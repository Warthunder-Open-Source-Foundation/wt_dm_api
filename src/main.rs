mod app_state;
mod endpoints;
mod error;
mod eyre_error_translation;
mod middleware;
mod unpacking;
mod vromf_enum;
mod wait_ready;

use std::{process::abort, sync::Arc, time::Duration};

use axum::{handler::Handler, response::Redirect, routing::get, Router};
use endpoints::{
	files::{Params, __path_get_files, get_files, FileRequest, UnpackedVromfs},
	get_vromfs::{get_latest, print_latest_version, update_cache_loop, VromfCache},
};
use octocrab::Octocrab;
use rayon::{ThreadPool, ThreadPoolBuilder};
use tokio::{
	signal,
	spawn,
	sync::{Mutex, RwLock},
	time::sleep,
};
use tracing::{error, level_filters::LevelFilter, log::info};
use tracing_subscriber::{fmt, EnvFilter};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};
use wt_version::Version;

use crate::{
	app_state::AppState,
	endpoints::{
		get_vromfs::find_version_sha,
		health::{__path_health, health},
		versions::{__path_list_versions, list_versions},
	},
	wait_ready::WaitReady,
};

#[derive(OpenApi)]
#[openapi(
	paths(get_files, health, list_versions),
	info(title = "WT Datamining API", version = "1.0")
)]
struct ApiDoc;

#[tokio::main]
async fn main() {
	if cfg!(feature = "tokio-console") {
		#[cfg(feature = "tokio-console")]
		console_subscriber::init();
	} else {
		let filter = EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into());
		fmt().with_env_filter(filter).try_init().unwrap(/*fine*/);
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
		.route("/health", get(health))
		.route("/metadata/versions", get(list_versions))
		.merge(Scalar::with_url("/docs", ApiDoc::openapi()))
		.with_state(state.clone());

	// run our app with hyper, listening globally on port 3000
	let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap(/*fine*/);

	update_cache_loop(state.clone(), wait_ready.register().await);

	// Ensure the commit cache is filled from the latest version to the latest in assets/commits.txt
	let mut octo = state.octocrab.lock().await;
	find_version_sha(
		state.clone(),
		&mut Some(Version::new(u16::MAX, u16::MAX, u16::MAX, u16::MAX)),
		&mut octo,
		None,
	)
	.await
	.unwrap();
	drop(octo);

	wait_ready.wait_ready().await;
	info!("Wait ready completed. Starting server...");
	axum::serve(listener, app).await.unwrap(/*fine*/);
	t.await.unwrap(/*fine*/);
}
