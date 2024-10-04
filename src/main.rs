mod error;
mod eyre_error_translation;
mod files;
mod get_vromfs;
mod vromf_enum;
mod wait_ready;

use std::sync::Arc;

use axum::{routing::get, Router};
use octocrab::Octocrab;
use tokio::sync::{Mutex, RwLock};
use tracing::log::info;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::{
	files::{Params, __path_get_files, get_files, FileRequest, UnpackedVromfs},
	get_vromfs::{get_latest, print_latest_version, update_cache_loop, VromfCache},
	wait_ready::WaitReady,
};

#[derive(Default)]
pub struct AppState {
	vromf_cache:     RwLock<VromfCache>,
	octocrab:        Mutex<Octocrab>,
	unpacked_vromfs: UnpackedVromfs,
}

#[derive(OpenApi)]
#[openapi(paths(get_files), info(title = "WT Datamining API", version = "1.0"))]
struct ApiDoc;

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
		.merge(Scalar::with_url("/docs", ApiDoc::openapi()))
		.with_state(state.clone());

	// run our app with hyper, listening globally on port 3000
	let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

	update_cache_loop(state, wait_ready.register().await);

	wait_ready.wait_ready().await;
	info!("Wait ready completed. Starting server...");
	axum::serve(listener, app).await.unwrap();
}
