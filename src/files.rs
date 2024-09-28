use std::sync::Arc;
use axum::extract::{Path, State};
use wt_blk::vromf::BlkOutputFormat;
use wt_version::Version;

use crate::AppState;

pub struct FileRequest {
	/// Defaults to latest
	version: Version,

	/// File path within vromf to return
	path: String,

	/// None if raw file
	unpack_format: Option<BlkOutputFormat>,
}

impl FileRequest {
	pub async fn default(state: Arc<AppState>) -> Self {
		Self {
			version:       state.vromf_cache.read().await.latest_known_version,
			path:          "/".to_string(),
			unpack_format: None,
		}
	}
}

pub async fn get_files(State(state): State<Arc<AppState>>, Path(path): Path<String>) -> String {
	dbg!(&path);

	"".to_string()
}