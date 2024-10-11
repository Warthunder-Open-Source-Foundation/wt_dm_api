use std::sync::Arc;

use octocrab::Octocrab;
use rayon::{ThreadPool, ThreadPoolBuilder};
use tokio::{
	sync::{
		oneshot::{channel, Sender},
		Mutex,
	},
	task::spawn_blocking,
};

use crate::{
	error::ApiError,
	eyre_error_translation::EyreToApiError,
	files::UnpackedVromfs,
	get_vromfs::VromfCache,
};

pub struct AppState {
	pub vromf_cache:     VromfCache,
	pub octocrab:        Mutex<Octocrab>,
	pub unpacked_vromfs: UnpackedVromfs,
	worker_pool:         Arc<ThreadPool>,
}

impl Default for AppState {
	fn default() -> Self {
		let worker_pool = Arc::new(
			ThreadPoolBuilder::new()
				.thread_name(|idx| format!("worker-pool-{}", idx))
				.build()
				.unwrap(),
		);

		Self {
			vromf_cache: Default::default(),
			octocrab: Default::default(),
			unpacked_vromfs: Default::default(),
			worker_pool,
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
