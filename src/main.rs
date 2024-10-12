mod app_state;
mod error;
mod eyre_error_translation;
mod files;
mod get_vromfs;
mod loki_tracing;
mod vromf_enum;
mod wait_ready;

use std::{process::abort, sync::Arc, time::Duration};

use axum::{routing::get, Router};
use octocrab::Octocrab;
use rayon::{ThreadPool, ThreadPoolBuilder};
use tokio::{
	signal,
	spawn,
	sync::{Mutex, RwLock},
	time::sleep,
};
use tracing::{error, level_filters::LevelFilter, log::info};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::{
	app_state::AppState,
	files::{Params, __path_get_files, get_files, FileRequest, UnpackedVromfs},
	get_vromfs::{get_latest, print_latest_version, update_cache_loop, VromfCache},
	loki_tracing::spawn_loki,
	wait_ready::WaitReady,
};

#[derive(OpenApi)]
#[openapi(paths(get_files), info(title = "WT Datamining API", version = "1.0"))]
struct ApiDoc;

#[tokio::main]
async fn main() {
	if cfg!(feature = "tokio-console") {
		#[cfg(feature = "tokio-console")]
		console_subscriber::init();
	} else {
		let filter = EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into());
		let layer = spawn_loki();
		tracing_subscriber::registry()
			.with(layer) // Register the custom layer
			.with(fmt::layer().with_filter(filter)) // fmt layer with env filter
			.init(); // Register the layers with tracing
	}

	color_eyre::install().unwrap(/*fine*/);

	let t = spawn(async {
		signal::ctrl_c().await.unwrap(/*fine*/);
		error!("Got CTRL-C signal. Aborting in 1000ms");
		#[cfg(not(debug_assertions))]
		sleep(Duration::from_millis(1000)).await;
		error!("Aborting after CTRL-C...");
		abort();
	});

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
	let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap(/*fine*/);

	update_cache_loop(state, wait_ready.register().await);

	wait_ready.wait_ready().await;
	info!("Wait ready completed. Starting server...");
	axum::serve(listener, app).await.unwrap(/*fine*/);
	t.await.unwrap(/*fine*/);
}
