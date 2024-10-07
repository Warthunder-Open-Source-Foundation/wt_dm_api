use std::{collections::HashMap, num::NonZeroUsize, str::FromStr, sync::Arc, time::Duration};

use axum::extract::{Path, State};
use http::StatusCode;
use lru::LruCache;
use octocrab::Octocrab;
use strum::VariantArray;
use tokio::{sync::oneshot::Sender, time::sleep};
use tracing::{debug, info};
use wt_version::Version;

use crate::{
	error::ApiError,
	eyre_error_translation::{EyreToApiError, OptionToApiError},
	vromf_enum::VromfType,
	AppState,
};

pub struct VromfCache {
	elems:                LruCache<Version, HashMap<VromfType, Vec<u8>>>,
	latest_known_version: Version,
	commit_pages:         HashMap<Version, String>,
}

impl Default for VromfCache {
	fn default() -> Self {
		Self {
			elems:                LruCache::new(NonZeroUsize::new(100).unwrap(/*fine*/)),
			latest_known_version: Version::from_u64(0),
			commit_pages:         cached_shas(),
		}
	}
}

impl VromfCache {
	pub fn latest_known_version(&self) -> Version {
		self.latest_known_version
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
		state.vromf_cache.read().await.latest_known_version
	};

	// Validate if cache already has vromf
	if *ask_api {
		if state
			.vromf_cache
			.write()
			.await
			.elems
			.get(&version)
			.is_some()
		{
			*ask_api = false;
		}
	}

	// Only refresh when necessary
	if *ask_api {
		pull_vromf_to_cache(state.clone(), Some(version)).await?;
	}

	let res = state
		.vromf_cache
		.write()
		.await
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
	let mut r = state.vromf_cache.write().await;
	let v = r.latest_known_version.clone();
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
	let sha = find_version_sha(state.clone(), &mut version, &mut octo).await?;
	let version = version.convert_err("Version was not set by find_version_sha")?;
	if get_latest {
		if version > state.vromf_cache.read().await.latest_known_version {
			info!("Found newer version: {version}");
			state.vromf_cache.write().await.latest_known_version = version;

			#[cfg(feature = "dev-cache")]
			{
				use std::fs;
				let mut cache_intact = true;
				let mut files = vec![];
				for vromf in VromfType::VARIANTS {
					if let Ok(f) = fs::read(format!("target/vromf_cache/{vromf}.{version}")) {
						files.push((*vromf, f));
					} else {
						cache_intact = false;
					}
				}
				if cache_intact {
					info!("Got vromfs from disk");
					state
						.vromf_cache
						.write()
						.await
						.elems
						.push(version, HashMap::from_iter(files.into_iter()));
					return Ok(());
				}
			}
			let vromfs = get_vromfs(&sha, &mut octo).await?;
			state.vromf_cache.write().await.elems.push(version, vromfs);

			#[cfg(feature = "dev-cache")]
			{
				use std::fs;
				info!("Wrote cache to disk");
				fs::create_dir("target/vromf_cache").unwrap(/*fine*/);
				for (vromf, b) in state
					.vromf_cache
					.write()
					.await
					.elems
					.get(&version)
					.unwrap(/*fine*/)
					.iter()
				{
					fs::write(format!("target/vromf_cache/{vromf}.{version}"), b).unwrap(/*fine*/)
				}
			}

			info!("Pushed {version} to cache");
		} else {
			info!("No newer version found");
		}
	} else {
		if state
			.vromf_cache
			.write()
			.await
			.elems
			.get(&version)
			.is_none()
		{
			let vromfs = get_vromfs(&sha, &mut octo).await?;
			state.vromf_cache.write().await.elems.push(version, vromfs);
		}
	}
	Ok(())
}

async fn get_vromfs(sha: &str, octo: &mut Octocrab) -> ApiError<HashMap<VromfType, Vec<u8>>> {
	let mut reqs = HashMap::new();
	debug!("Downloading vromfs");
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

async fn find_version_sha(
	state: Arc<AppState>,
	v: &mut Option<Version>,
	octo: &mut Octocrab,
) -> ApiError<String> {
	let cache = state.vromf_cache.read().await;
	let latest_known_version = cache.latest_known_version;

	// Consult LUT for ancient vromfs
	if let Some(res) = cache.commit_pages.get(&v.unwrap_or(latest_known_version)) {
		return Ok(res.clone());
	}

	// Check if the version should have been in the LUT, if it has, the version does not exist
	if let Some(v) = *v {
		if latest_known_version > v {
			return Err((StatusCode::BAD_REQUEST, "Version is not valid".to_string()));
		}
	}
	drop(cache);
	// Else we look for newer versions than we currently know

	let mut page: u32 = 1;
	let mut checks = 0;
	debug!("Fetching SHAs from github");
	'outer: loop {
		let res = octo
			.repos("gszabi99", "War-Thunder-Datamine")
			.list_commits()
			.page(page)
			.send()
			.await
			.convert_err()?;
		let map = &mut state.vromf_cache.write().await.commit_pages;
		for commit in res {
			let parsed = Version::from_str(&commit.commit.message).convert_err()?;
			map.insert(parsed, commit.sha.clone());

			// Searching for version
			if let Some(v) = *v {
				if v == parsed {
					return Ok(commit.sha);
				} else {
					page += 1;
				}
			} else {
				// Return latest commit
				*v = Some(parsed);
				return Ok(commit.sha);
			}
			checks += 1;
			if checks > 60 {
				break 'outer;
			}
		}
	}
	if let Some(v) = *v {
		if v > latest_known_version {
			return Err((
				StatusCode::BAD_REQUEST,
				"Exceeded 60 searched versions into history. This version seems too new to exist"
					.to_string(),
			));
		}
	}
	Err((
		StatusCode::BAD_REQUEST,
		"Exceeded 60 searched versions into history. Are you sure this version exists?".to_string(),
	))
}

pub fn update_cache_loop(state: Arc<AppState>, sender: Sender<()>) {
	tokio::spawn(async move {
		let mut s = Some(sender);
		loop {
			pull_vromf_to_cache(state.clone(), None).await.unwrap();
			if let Some(s) = s.take() {
				s.send(()).unwrap();
			}

			sleep(Duration::from_secs(120)).await;
		}
	});
}

pub async fn print_latest_version(State(state): State<Arc<AppState>>) -> String {
	state
		.vromf_cache
		.read()
		.await
		.latest_known_version
		.to_string()
}

static CACHED_SHAS: &str = include_str!("../assets/commits.txt");
fn cached_shas() -> HashMap<Version, String> {
	CACHED_SHAS
		.lines()
		.map(|e| e.split(" "))
		.map(|mut e| (e.next().unwrap(/*fine*/), e.next().unwrap(/*fine*/)))
		.map(|(sha, version)| {
			(
				Version::from_str(&version).unwrap(/*fine*/),
				sha.to_string(),
			)
		})
		.collect()
}
