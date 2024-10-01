use std::{
	collections::HashMap,
	num::NonZeroUsize,
	str::FromStr,
	sync::{Arc, LazyLock},
	time::Duration,
};

use axum::{
	extract::{Path, State},
	response::IntoResponse,
	Json,
};
use base64::{prelude::BASE64_STANDARD, Engine};
use http::StatusCode;
use lru::LruCache;
use strum::VariantArray;
use tokio::sync::oneshot::Sender;
use tokio::time::sleep;
use tracing::info;
use tracing_subscriber::fmt::format;
use wt_version::Version;

use crate::AppState;
use crate::error::ApiError;
use crate::eyre_error_translation::EyreToApiError;
use crate::vromf_enum::VromfType;

pub struct VromfCache {
	pub elems:                LruCache<Version, HashMap<VromfType, Vec<u8>>>,
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

pub async fn refresh_cache(state: Arc<AppState>) -> ApiError<()> {
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
		for vromf in VromfType::VARIANTS {
			let file = octo
				.repos("gszabi99", "War-Thunder-Datamine")
				.get_content()
				.path(&format!("raw/{vromf}"))
				.r#ref(&res.items.first().unwrap().sha) // Specify the commit SHA
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
		state.vromf_cache.write().await.elems.push(latest, reqs);
		info!("Pushed {latest} to cache");
	} else {
		info!("No newer version found");
	}
	Ok(())
}

pub fn update_cache_loop(state: Arc<AppState>, sender: Sender<()>) {
	tokio::spawn(async move {
		let mut s = Some(sender);
		loop {
			refresh_cache(state.clone()).await.unwrap();
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
