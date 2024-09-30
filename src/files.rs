use std::{collections::HashMap, str::FromStr, sync::Arc};

use axum::{
	extract::{Path, Query, State},
	response::IntoResponse,
};
use http::StatusCode;
use serde::Deserialize;
use serde_json::to_string;
use wt_blk::vromf::{BlkOutputFormat, VromfUnpacker};
use wt_version::Version;

use crate::{get_vromfs::VROMFS, AppState};

pub struct UnpackedVromfs {
	unpackers: HashMap<String, VromfUnpacker>,
}

impl Default for UnpackedVromfs {
	fn default() -> Self {
		Self {
			unpackers: Default::default(),
		}
	}
}

pub struct FileRequest {
	/// Defaults to latest
	version: Version,

	/// File path within vromf to return
	path: String,

	/// None if raw file
	unpack_format: BlkOutputFormat,

	/// Returns just one file directly, or a zip containing many requested files (folder)
	single_file: bool,

	/// Which vromf to get from
	vromf: String,
}

#[derive(Debug, Deserialize)]
pub struct Params {
	version:     Option<String>,
	format:      Option<String>,
	single_file: Option<bool>,
}

impl FileRequest {
	pub async fn from_path_and_query(
		state: Arc<AppState>,
		path: &str,
		query: &Params,
	) -> Result<Self, (StatusCode, String)> {
		let (path, vromf) = {
			let path_split = path.split_once('/');

			match path_split {
				// Means the entire vromf is requested, as long as its valid
				None => {
					if VROMFS.contains(&path) {
						(path.to_owned(), "/".to_owned())
					} else {
						return Err((StatusCode::NOT_FOUND, format!("Vromf not found: {}", path)));
					}
				},
				Some(e) => (e.0.to_owned(), e.1.to_owned()),
			}
		};
		let latest = state.vromf_cache.read().await.latest_known_version;
		let unpack_format = match &query.format {
			None => BlkOutputFormat::Json,
			Some(f) => match f.to_ascii_lowercase().as_str() {
				"blk" => BlkOutputFormat::BlkText,
				"json" => BlkOutputFormat::Json,
				_ => return Err((StatusCode::BAD_REQUEST, format!("unknown output format: {f}"))),
			},
		};

		Ok(Self {
			version: query
				.version
				.clone()
				.map(|e| Version::from_str(&e))
				.unwrap_or(Ok(latest))
				.expect("Infallible"),
			path,
			unpack_format,
			single_file: query.single_file.unwrap_or(true),
			vromf,
		})
	}
}

pub async fn get_files(
	State(state): State<Arc<AppState>>,
	Path(path): Path<String>,
	Query(params): Query<Params>,
) -> Result<String, (StatusCode, String)> {
	dbg!(&path);

	let req = FileRequest::from_path_and_query(state, &path, &params).await?;

	Ok("".to_string())
}
