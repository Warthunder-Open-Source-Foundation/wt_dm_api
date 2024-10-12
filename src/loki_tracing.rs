use std::process;

use reqwest::Url;
use tracing_loki::Layer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn spawn_loki() -> Layer {
	let (layer, task) = tracing_loki::builder()
		.label("host", "mine")
		.unwrap()
		.extra_field("pid", format!("{}", process::id()))
		.unwrap()
		.build_url(Url::parse("http://127.0.0.1:3100").unwrap())
		.unwrap();

	// The background task needs to be spawned so the logs actually get
	// delivered.
	tokio::spawn(task);
	layer
}
