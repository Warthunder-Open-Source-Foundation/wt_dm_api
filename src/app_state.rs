use std::{env, sync::Arc, time::Duration};

use moka::future::{Cache, CacheBuilder};
use octocrab::Octocrab;
use rayon::{ThreadPool, ThreadPoolBuilder};
use tokio::{
	sync::{
		oneshot::{channel, Sender},
		Mutex,
	},
	time::sleep,
};
use tracing::{error, info};

use crate::{
	endpoints::{
		files::{FileRequest, UnpackedVromfs},
		get_vromfs,
		get_vromfs::VromfCache,
	},
	error::ApiError,
	eyre_error_translation::EyreToApiError,
};

pub struct AppState {
	// Contains binary VROMFs requested from github
	pub vromf_cache:     VromfCache,
	pub octocrab:        Mutex<Octocrab>,
	// Initialized unpackers per VROMF
	pub unpacked_vromfs: UnpackedVromfs,
	worker_pool:         Arc<ThreadPool>,
	// 	Request with content type and data
	pub files_cache:     Cache<FileRequest, (Vec<u8>, &'static str)>,
}

impl Default for AppState {
	fn default() -> Self {
		let worker_pool = Arc::new(
			ThreadPoolBuilder::new()
				.thread_name(|idx| format!("worker-pool-{}", idx))
				.build()
				.unwrap(/*fine*/),
		);
		let mut octocrab = Octocrab::builder();
		if let Ok(tok) = env::var("GH_TOKEN") {
			octocrab = octocrab.personal_token(tok);
		}

		Self {
			vromf_cache: Default::default(),
			octocrab: Mutex::new(octocrab.build().unwrap()),
			unpacked_vromfs: Default::default(),
			worker_pool,
			files_cache: CacheBuilder::new(100)
				.time_to_live(Duration::from_secs(60)) // 😡😡😡😡😡 https://github.com/rust-lang/rust/issues/120301
				.build(),
		}
	}
}

impl AppState {
	pub async fn spawn_worker<F, T>(self: Arc<Self>, f: F) -> ApiError<T>
	where
		F: FnOnce(Sender<T>) + Send + 'static,
		T: Send + 'static, {
		let (s, r) = channel();
		self.worker_pool.spawn(|| f(s));
		r.await.convert_err()
	}
}

pub fn cache_refresh_task(state: Arc<AppState>, sender: Sender<()>) {
	tokio::spawn(async move {
		let mut s = Some(sender);
		loop {
			{
				let e = get_vromfs::pull_vromf_to_cache(state.clone(), None)
					.await
					.err();
				if let Some(e) = e {
					error!("Failed to pull latest vromfs to cache. Reason: {}", e.1);
				}
			}

			if let Some(s) = s.take() {
				s.send(()).expect("main vromf thread to run");
			}
			info!("Updated vromfs to cache job");
			sleep(Duration::from_secs(120)).await;
		}
	});
}
