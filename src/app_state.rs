use std::{env, sync::Arc, time::Duration};

use moka::future::{Cache, CacheBuilder};
use octocrab::Octocrab;
use rayon::{ThreadPool, ThreadPoolBuilder};
use tokio::sync::{
	oneshot::{channel, Sender},
	Mutex,
};

use crate::{
	endpoints::{
		files::{FileRequest, UnpackedVromfs},
		get_vromfs::VromfCache,
	},
	error::ApiError,
	eyre_error_translation::EyreToApiError,
	unpacking::Vromfs,
};

pub struct AppState {
	vromfs:          Vromfs,
	worker_pool:     Arc<ThreadPool>,
	// 	Request with content type and data
	pub files_cache: Cache<FileRequest, (Vec<u8>, &'static str)>,
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
			vromfs: Vromfs::new(octocrab.build().unwrap()),
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
