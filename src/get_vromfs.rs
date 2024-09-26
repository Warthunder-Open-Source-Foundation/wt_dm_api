use std::num::NonZeroUsize;
use std::ops::Sub;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use axum::extract::State;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::info;
use wt_version::Version;
use crate::AppState;

const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

pub struct VromfCache {
	elems: LruCache<Version, Vec<u8>>,
	last_checked: Instant,
	latest_known_version: Version,
}

impl Default for VromfCache {
	fn default() -> Self {
		Self {
			elems: LruCache::new(NonZeroUsize::new(10).unwrap()),
			last_checked: Instant::now().sub(FETCH_TIMEOUT),
			latest_known_version: Version::from_u64(0),
		}
	}
}


pub async fn get_latest(State(state): State<Arc<AppState>>) -> Vec<u8> {
	refresh_cache(state.clone()).await;

	let mut r = state.vromf_cache.write().await;
	let v = r.latest_known_version.clone();
	r.elems.get(&v).cloned().unwrap()
}

pub async fn refresh_cache(state: Arc<AppState>) {
	if state.vromf_cache.read().await.last_checked.elapsed() < FETCH_TIMEOUT {
		return;
	}
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

		let dec = base64::decode(file.items.first().unwrap().clone().content.unwrap()).unwrap();

		state.vromf_cache.write().await.elems.push(latest, dec).unwrap();
		info!("Pushed {latest} to cache");
	}

}
