use std::{collections::HashMap, num::NonZeroUsize, str::FromStr, sync::Arc, time::Duration};

use axum::extract::{Path, State};
use http::StatusCode;
use lru::LruCache;
use octocrab::Octocrab;
use strum::VariantArray;
use tokio::{sync::oneshot::Sender, time::sleep};
use tracing::info;
use wt_version::Version;

use crate::{
	error::ApiError,
	eyre_error_translation::EyreToApiError,
	vromf_enum::VromfType,
	AppState,
};

pub struct VromfCache {
	elems:                LruCache<Version, HashMap<VromfType, Vec<u8>>>,
	latest_known_version: Version,
}

impl Default for VromfCache {
	fn default() -> Self {
		Self {
			elems:                LruCache::new(NonZeroUsize::new(100).unwrap()),
			latest_known_version: Version::from_u64(0),
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
		refresh_cache(state.clone(), Some(version)).await?;
	}

	let res = state
		.vromf_cache
		.write()
		.await
		.elems
		.get(&version)
		.expect("vromf cache is intact")
		.get(&vromf_type)
		.expect("vromf cache misses vromf type")
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

pub async fn refresh_cache(state: Arc<AppState>, mut version: Option<Version>) -> ApiError<()> {
	info!("Refreshing vromf cache");

	let mut octo = state.octocrab.lock().await;
	let get_latest = version.is_none();
	let sha = find_version_sha(&mut version, &mut octo).await?;
	let version = version.expect("version sha should have set version");
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
				fs::create_dir("target/vromf_cache").unwrap();
				for (vromf, b) in state
					.vromf_cache
					.write()
					.await
					.elems
					.get(&version)
					.unwrap()
					.iter()
				{
					fs::write(format!("target/vromf_cache/{vromf}.{version}"), b).unwrap()
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
	for vromf in VromfType::VARIANTS {
		let file = octo
			.repos("gszabi99", "War-Thunder-Datamine")
			.get_content()
			.path(&format!("raw/{vromf}"))
			.r#ref(sha)
			.send()
			.await
			.convert_err()?;

		let dec = reqwest::get(file.items.first().unwrap().clone().download_url.unwrap())
			.await
			.unwrap()
			.bytes()
			.await
			.unwrap()
			.to_vec();
		reqs.insert(*vromf, dec);
	}
	Ok(reqs)
}

async fn find_version_sha(v: &mut Option<Version>, octo: &mut Octocrab) -> ApiError<String> {
	let mut page: u32 = 1;
	let mut checks = 0;
	loop {
		let res = octo
			.repos("gszabi99", "War-Thunder-Datamine")
			.list_commits()
			.page(page)
			.send()
			.await
			.unwrap();
		for commit in res {
			let parsed = Version::from_str(&commit.commit.message).convert_err()?;

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
			if checks > 500 {
				break;
			}
		}
	}
	Err((
		StatusCode::BAD_REQUEST,
		"Exceeded 500 searched versions into history.".to_string(),
	))
}

pub fn update_cache_loop(state: Arc<AppState>, sender: Sender<()>) {
	tokio::spawn(async move {
		let mut s = Some(sender);
		loop {
			refresh_cache(state.clone(), None).await.unwrap();
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
