use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration};
use axum::extract::State;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use lru::LruCache;
use tokio::time::sleep;
use tracing::info;
use wt_version::Version;
use crate::AppState;

pub struct VromfCache {
	elems: LruCache<Version, Vec<u8>>,
	pub latest_known_version: Version,
}

impl Default for VromfCache {
	fn default() -> Self {
		Self {
			elems: LruCache::new(NonZeroUsize::new(10).unwrap()),
			latest_known_version: Version::from_u64(0),
		}
	}
}


pub async fn get_latest(State(state): State<Arc<AppState>>) -> Vec<u8> {
	let mut r = state.vromf_cache.write().await;
	let v = r.latest_known_version.clone();
	r.elems.get(&v).cloned().unwrap()
}

pub async fn refresh_cache(state: Arc<AppState>) {
	info!("Refreshing vromf cache");

	let octo = state.octocrab.lock().await;
	let res = octo.repos("gszabi99", "War-Thunder-Datamine")
		.list_commits()
		.send()
		.await
		.unwrap();
	let latest = Version::from_str(&res.items.first().unwrap().commit.message).unwrap();

	if latest > state.vromf_cache.read().await.latest_known_version {
		info!("Found newer version: {latest}");
		state.vromf_cache.write().await.latest_known_version = latest;

		let file = octo
			.repos("gszabi99", "War-Thunder-Datamine")
			.get_content()
			.path("raw/aces.vromfs.bin")
			.r#ref(&res.items.first().unwrap().sha) // Specify the commit SHA
			.send()
			.await.unwrap();

		let dec = BASE64_STANDARD.decode(file.items.first().unwrap().clone().content.unwrap().as_bytes()).unwrap();

		state.vromf_cache.write().await.elems.push(latest, dec);
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
	state.vromf_cache.read().await.latest_known_version.to_string()
}