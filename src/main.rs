mod files;
mod get_vromfs;

use std::sync::Arc;

use axum::{
	http::StatusCode,
	routing::{get, post},
	Json,
	Router,
};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};

use crate::{
	files::{get_files, UnpackedVromfs},
	get_vromfs::{get_latest, print_latest_version, update_cache_loop, VromfCache},
};

#[derive(Default)]
pub struct AppState {
	vromf_cache:     RwLock<VromfCache>,
	octocrab:        Mutex<Octocrab>,
	unpacked_vromfs: UnpackedVromfs,
}

#[tokio::main]
async fn main() {
	// initialize tracing
	tracing_subscriber::fmt::init();
	color_eyre::install().unwrap();

	let state = Arc::new(AppState::default());

	// See the routing_docs folder for more details on the router
	let app = Router::new()
		.route("/latest/*vromf", get(get_latest))
		.route("/metadata/latest", get(print_latest_version))
		.route("/files/*path", get(get_files))
		.with_state(state.clone());

	// run our app with hyper, listening globally on port 3000
	let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

	update_cache_loop(state);

	axum::serve(listener, app).await.unwrap();
}
