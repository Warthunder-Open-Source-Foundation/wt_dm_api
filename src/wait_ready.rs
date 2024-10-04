use tokio::sync::{
	oneshot,
	oneshot::{Receiver, Sender},
	RwLock,
};

pub struct WaitReady {
	tasks: RwLock<Vec<Receiver<()>>>,
}

impl WaitReady {
	pub fn new() -> Self {
		WaitReady {
			tasks: Default::default(),
		}
	}

	pub async fn register(&mut self) -> Sender<()> {
		let (send, recv) = oneshot::channel();
		self.tasks.write().await.push(recv);
		send
	}

	pub async fn wait_ready(self) {
		for task in self.tasks.into_inner() {
			task.await.unwrap();
		}
	}
}
