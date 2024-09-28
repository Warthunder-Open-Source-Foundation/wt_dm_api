use std::{num::NonZeroUsize, str::FromStr, sync::Arc, time::Duration};
use std::collections::HashMap;
use std::sync::LazyLock;
use axum::extract::{Path, State};
use axum::Json;
use axum::response::IntoResponse;
use base64::{prelude::BASE64_STANDARD, Engine};
use http::StatusCode;
use lru::LruCache;
use tokio::time::sleep;
use tracing::info;
use tracing_subscriber::fmt::format;
use wt_version::Version;

use crate::AppState;

static VROMF_NAMES: [&'static str; 8] = ["aces", "char", "game", "gui", "lang", "mis", "regional", "wwdata"];
static VROMFS: LazyLock<[Box<str>; 8]> = LazyLock::new(|| {
	VROMF_NAMES.into_iter().map(|e|format!("{e}.vromfs.bin").into_boxed_str()).collect::<Vec<_>>().try_into().unwrap()
});

pub struct VromfCache {
	elems:                    LruCache<Version, HashMap<String, Vec<u8>>>,
	pub latest_known_version: Version,
}

impl Default for VromfCache {
	fn default() -> Self {
		Self {
			elems:                LruCache::new(NonZeroUsize::new(10).unwrap()),
			latest_known_version: Version::from_u64(0),
		}
	}
}

pub async fn get_latest(State(state): State<Arc<AppState>>, Path(path): Path<String>) -> impl IntoResponse {
	let mut r = state.vromf_cache.write().await;
	let v = r.latest_known_version.clone();
	match r.elems.get(&v) {
		None => {
			(StatusCode::NOT_FOUND, format!("Version {v} is invalid")).into_response()
		}
		Some(c) => {
			match c.get(&path) {
				None => {
					(StatusCode::NOT_FOUND, format!("Path {path} not found")).into_response()
				}
				Some(e) => {
					(StatusCode::OK, e.clone()).into_response()
				}
			}
		}
	}
}

pub async fn refresh_cache(state: Arc<AppState>) {
	info!("Refreshing vromf cache");

	let octo = state.octocrab.lock().await;
	let res = octo
		.repos("gszabi99", "War-Thunder-Datamine")
		.list_commits()
		.send()
		.await
		.unwrap();
	let latest = Version::from_str(&res.items.first().unwrap().commit.message).unwrap();

	if latest > state.vromf_cache.read().await.latest_known_version {
		info!("Found newer version: {latest}");
		state.vromf_cache.write().await.latest_known_version = latest;

		let mut reqs = HashMap::new();
		for VROMF in VROMFS.iter() {
				let file = octo
					.repos("gszabi99", "War-Thunder-Datamine")
					.get_content()
					.path(&format!("raw/{VROMF}"))
					.r#ref(&res.items.first().unwrap().sha) // Specify the commit SHA
					.send()
					.await
					.unwrap();

				let dec = reqwest::get(file.items.first().unwrap().clone().download_url.unwrap())
					.await
					.unwrap()
					.bytes()
					.await
					.unwrap()
					.to_vec();
			dbg!(VROMF);
			reqs.insert(VROMF.to_string(), dec);
		}
		state.vromf_cache.write().await.elems.push(latest, reqs);
		info!("Pushed {latest} to cache");
	} else {
		info!("No newer version found");
	}
}

pub fn update_cache_loop(state: Arc<AppState>) {
	tokio::spawn(async move {
		loop {
			refresh_cache(state.clone()).await;
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
