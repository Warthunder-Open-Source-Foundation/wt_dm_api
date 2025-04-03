use std::{
	collections::HashMap,
	env,
	env::current_exe,
	num::NonZeroUsize,
	str::FromStr,
	sync::{Arc, OnceLock},
	time::Duration,
};

use arc_swap::ArcSwap;
use axum::extract::{Path, State};
use dashmap::{mapref::multiple::RefMulti, DashMap};
use http::StatusCode;
use moka::ops::compute::Op;
use octocrab::Octocrab;
use strum::VariantArray;
use tokio::{
	sync::{oneshot::Sender, RwLock},
	time::sleep,
};
use tracing::{debug, error, info, warn};
use wt_version::Version;

use crate::{
	app_state::AppState,
	error::ApiError,
	eyre_error_translation::{EyreToApiError, OptionToApiError},
	vromf_enum::VromfType,
};

pub struct VromfCache {
	elems:        DashMap<Version, HashMap<VromfType, Vec<u8>>>,
	commit_pages: DashMap<Version, String>,
}

impl Default for VromfCache {
	fn default() -> Self {
		Self {
			elems:        DashMap::new(),
			commit_pages: cached_shas(),
		}
	}
}

impl VromfCache {
	pub fn latest_known_version(&self) -> Version {
		self.list_versions().map(|e| *e.key()).max().unwrap()
	}

	pub fn list_versions(&self) -> impl Iterator<Item = RefMulti<'_, Version, String>> {
		self.commit_pages.iter()
	}
}

pub async fn fetch_vromf(
	state: Arc<AppState>,
	version: Option<Version>,
	vromf_type: VromfType,
	// Helper for fetching many vromfs in a loop to only refresh the cache once
	ask_api: &mut bool,
) -> ApiError<Vec<u8>> {
	// Determine exact version
	let version = if let Some(v) = version {
		v
	} else {
		state.vromf_cache.latest_known_version()
	};

	// Validate if cache already has vromf
	if *ask_api {
		if state.vromf_cache.elems.get(&version).is_some() {
			*ask_api = false;
		}
	}

	// Only refresh when necessary
	if *ask_api {
		pull_vromf_to_cache(state.clone(), Some(version)).await?;
	}

	let res = state
		.vromf_cache
		.elems
		.get(&version)
		.convert_err("vromf cache does not have expected version")?
		.get(&vromf_type)
		.convert_err("vromf cache does not have expected type")?
		.clone();

	// When called in a loop we can avoid
	*ask_api = false;
	Ok(res)
}

pub async fn get_latest(
	State(state): State<Arc<AppState>>,
	Path(path): Path<String>,
) -> ApiError<Vec<u8>> {
	let r = &state.vromf_cache;
	let v = r.latest_known_version();
	match r.elems.get(&v) {
		None => Err((StatusCode::NOT_FOUND, format!("Version {v} is invalid"))),
		Some(c) => match c.get(&VromfType::from_str(&path).convert_err()?) {
			None => Err((StatusCode::NOT_FOUND, format!("Path {path} not found"))),
			Some(e) => Ok(e.clone()),
		},
	}
}

pub async fn pull_vromf_to_cache(
	state: Arc<AppState>,
	mut version: Option<Version>,
) -> ApiError<()> {
	info!("Refreshing vromf cache");

	let mut octo = state.octocrab.lock().await;
	let get_latest = version.is_none();
	let sha = find_version_sha(state.clone(), &mut version, &mut octo, Some(2)).await?;
	let version = version.convert_err("Version was not set by find_version_sha")?;
	if get_latest {
		if version > state.vromf_cache.latest_known_version() {
			info!("Found newer version: {version}");

			#[cfg(feature = "dev-cache")]
			{
				let out_dir = env::current_dir()
					.unwrap()
					.join("target/vromf_cache")
					.to_str()
					.unwrap()
					.to_string();
				use std::fs;
				let mut cache_intact = true;
				let mut files = vec![];
				for vromf in VromfType::VARIANTS {
					if let Ok(f) = fs::read(format!("{out_dir}/{vromf}.{version}")) {
						files.push((*vromf, f));
					} else {
						cache_intact = false;
					}
				}
				if cache_intact {
					info!("Got vromfs from disk");
					state
						.vromf_cache
						.elems
						.insert(version, HashMap::from_iter(files.into_iter()));
					return Ok(());
				}
			}
			let vromfs = get_vromfs(&sha, &mut octo).await?;
			state.vromf_cache.elems.insert(version, vromfs);

			#[cfg(feature = "dev-cache")]
			{
				let out_dir = env::current_dir()
					.unwrap()
					.join("target/vromf_cache")
					.to_str()
					.unwrap()
					.to_string();
				use std::fs;
				info!("Wrote cache to disk");
				fs::create_dir_all(&out_dir).unwrap();
				for (vromf, b) in state
					.vromf_cache
					.elems
					.get(&version)
					.unwrap(/*fine*/)
					.iter()
				{
					fs::write(format!("{out_dir}/{vromf}.{version}"), b).unwrap(/*fine*/)
				}
			}

			info!("Pushed {version} to cache");
		} else {
			info!("No newer version found");
		}
	} else {
		if state.vromf_cache.elems.get(&version).is_none() {
			let vromfs = get_vromfs(&sha, &mut octo).await?;
			state.vromf_cache.elems.insert(version, vromfs);
		}
	}
	state.vromf_cache.commit_pages.insert(version, sha);
	Ok(())
}

async fn get_vromfs(sha: &str, octo: &mut Octocrab) -> ApiError<HashMap<VromfType, Vec<u8>>> {
	let mut reqs = HashMap::new();
	info!("Downloading vromfs from: {sha}");
	for vromf in VromfType::VARIANTS {
		let file = octo
			.repos("gszabi99", "War-Thunder-Datamine")
			.get_content()
			.path(&format!("raw/{vromf}"))
			.r#ref(sha)
			.send()
			.await
			.convert_err()?;

		let dec = reqwest::get(
			file.items
				.first()
				.convert_err("commit has no elements")?
				.clone()
				.download_url
				.convert_err("no download URL on commit")?,
		)
		.await
		.convert_err()?
		.bytes()
		.await
		.convert_err()?
		.to_vec();
		reqs.insert(*vromf, dec);
	}
	Ok(reqs)
}

pub async fn find_version_sha(
	state: Arc<AppState>,
	v: &mut Option<Version>,
	octo: &mut Octocrab,
	// Set to none when performing unbounded cache warmup
	maximum_pages_request_limit: Option<u64>,
) -> ApiError<String> {
	let cache = &state.vromf_cache;
	let latest_known_version = cache.latest_known_version();

	// Consult LUT for ancient vromfs
	if let Some(res) = cache.commit_pages.get(&v.unwrap_or(latest_known_version)) {
		*v = Some(*res.key());
		return Ok(res.clone());
	}

	// Check if the version should have been in the LUT, if it has, the version does not exist
	if let Some(v) = *v {
		if latest_known_version > v {
			return Err((StatusCode::BAD_REQUEST, "Version is not valid".to_string()));
		}
	}
	// Else we look for newer versions than we currently know

	let mut checks = 0;
	info!("Fetching SHAs from github for version: {v:?}");
	'outer: for page in 1_u32.. {
		let res = octo
			.repos("gszabi99", "War-Thunder-Datamine")
			.list_commits()
			.page(page)
			.send()
			.await
			.convert_err()?;
		let commit_pages = &state.vromf_cache.commit_pages;

		for commit in res {
			let parsed = Version::from_str(&commit.commit.message).convert_err()?;
			let before = commit_pages.insert(parsed, commit.sha.clone());
			if before.is_none() {
				warn!("discovered {parsed}");
			}

			// If a specific version is desired, then check if we found it
			if let Some(v) = *v {
				if v == parsed {
					return Ok(commit.sha);
				}
			}
			// Otherwise just return whatever is the latest
			else {
				*v = Some(parsed);
				return Ok(commit.sha);
			}

			// Also check if we have reached the latest statically known version
			if parsed <= *LATEST_MAPPED.get().unwrap(/*fine*/) {
				// Make an exception for unbounded check, in this case, we have reached our goal
				if maximum_pages_request_limit.is_some() {
					break 'outer;
				} else {
					return Ok(commit.sha);
				}
			}
		}

		checks += 1;
		if let Some(check_limit) = maximum_pages_request_limit {
			if checks > check_limit {
				break 'outer;
			}
		}
	}
	if let Some(v) = *v {
		if v > latest_known_version {
			return Err((
				StatusCode::BAD_REQUEST,
				format!("Exceeded {maximum_pages_request_limit:?} searched versions into history. This version seems too new to exist"),
			));
		}
	}
	Err((
		StatusCode::BAD_REQUEST,
		format!("Exceeded {maximum_pages_request_limit:?} searched versions into history. Are you sure this version exists?"),
	))
}

pub async fn print_latest_version(State(state): State<Arc<AppState>>) -> String {
	state.vromf_cache.latest_known_version().to_string()
}

static CACHED_SHAS: &str = include_str!("../../assets/commits.txt");
const EARLIEST_VERSION: Version = Version::new(2, 27, 2, 20);
static LATEST_MAPPED: OnceLock<Version> = OnceLock::new();
fn cached_shas() -> DashMap<Version, String> {
	let it: DashMap<Version, String> = CACHED_SHAS
		.lines()
		.map(|e| e.split(" "))
		.map(|mut e| (e.next().unwrap(/*fine*/), e.next().unwrap(/*fine*/)))
		.map(|(sha, version)| {
			(
				Version::from_str(&version).unwrap(/*fine*/),
				sha.to_string(),
			)
		})
		.filter(|&(v, _)| v >= EARLIEST_VERSION)
		.collect();

	{
		let max = it.iter().max_by_key(|e| *e.key()).unwrap();
		LATEST_MAPPED.set(*max.key()).unwrap();
	}
	it
}
