mod files;
mod get_vromfs;
mod eyre_error_translation;
mod vromf_enum;
mod error;
mod wait_ready;

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
use tracing::log::info;
use crate::{
	files::{get_files, UnpackedVromfs},
	get_vromfs::{get_latest, print_latest_version, update_cache_loop, VromfCache},
};
use crate::wait_ready::WaitReady;

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

	let mut wait_ready = WaitReady::new();

	let state = Arc::new(AppState::default());

	// See the routing_docs folder for more details on the router
	let app = Router::new()
		.route("/latest/*vromf", get(get_latest))
		.route("/metadata/latest", get(print_latest_version))
		.route("/files/*path", get(get_files))
		.with_state(state.clone());

	// run our app with hyper, listening globally on port 3000
	let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

	update_cache_loop(state, wait_ready.register().await);


	wait_ready.wait_ready().await;
	info!("Wait ready completed. Starting server...");
	axum::serve(listener, app).await.unwrap();
}
