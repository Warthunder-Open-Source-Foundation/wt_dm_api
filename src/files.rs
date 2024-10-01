use std::{
	path::{Path as StdPath, PathBuf},
	str::FromStr,
	sync::Arc,
};

use axum::{
	extract::{Path, Query, State},
};
use dashmap::DashMap;
use http::StatusCode;
use serde::Deserialize;
use strum::VariantArray;
use wt_blk::vromf::{BlkOutputFormat, File, VromfUnpacker};
use wt_version::Version;
use crate::eyre_error_translation::EyreToApiError;

use crate::{AppState};
use crate::vromf_enum::VromfType;

pub struct UnpackedVromfs {
	unpackers: DashMap<(Version, VromfType), VromfUnpacker>,
}

impl UnpackedVromfs {
	pub async fn unpack_one(
		state: Arc<AppState>,
		req: FileRequest,
	) -> Result<Vec<u8>, (StatusCode, String)> {
		Self::refresh_cache(&state.unpacked_vromfs, state.clone(), &req).await?;

		let vromf = req.vromf;
		let unpacker = state
			.unpacked_vromfs
			.unpackers
			.get_mut(&(req.version, vromf))
			.expect("Vromfs should be validated and present");
		let res = unpacker.unpack_one(StdPath::new(&req.path), req.unpack_format, true);

		match res {
			Ok(res) => Ok(res.split().1),
			Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
		}
	}

	pub async fn refresh_cache(&self, state: Arc<AppState>, req: &FileRequest) -> Result<(), (StatusCode, String)> {
		for vromf in VromfType::VARIANTS {
			// TODO: Replace expects
			let buf = state
				.vromf_cache
				.write()
				.await
				.elems
				.get(&req.version)
				.expect("vromfs should have latest version")
				.get(vromf)
				.expect("vromfs should be in map")
				.to_owned();
			state.unpacked_vromfs.unpackers.insert(
				(req.version, *vromf),
				VromfUnpacker::from_file(
					&File::from_raw(vromf.into(), buf),
					true,
				).convert_err()?,
			);
		}
		Ok(())
	}
}

impl Default for UnpackedVromfs {
	fn default() -> Self {
		Self {
			unpackers: Default::default(),
		}
	}
}

#[derive(Debug)]
pub struct FileRequest {
	/// Defaults to latest
	version: Version,

	/// File path within vromf to return
	path: String,

	/// None if raw file
	unpack_format: Option<BlkOutputFormat>,

	/// Returns just one file directly, or a zip containing many requested files (folder)
	single_file: bool,

	/// Which vromf to get from
	vromf: VromfType,
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
		let (vromf, path) = {
			let path_split = path.split_once('/');

			match path_split {
				// Means the entire vromf is requested, as long as its valid
				None => {
					if let Ok(v) = VromfType::from_str(path) {
						(v, "/".to_owned())
					} else {
						return Err((StatusCode::NOT_FOUND, format!("Vromf not found: {}", path)));
					}
				},
				Some(e) => (VromfType::from_str(e.0).convert_err()?, e.1.to_owned()),
			}
		};
		let latest = state.vromf_cache.read().await.latest_known_version;
		let unpack_format = match &query.format {
			None => Some(BlkOutputFormat::Json),
			Some(f) => match f.to_ascii_lowercase().as_str() {
				"blk" => Some(BlkOutputFormat::BlkText),
				"json" => Some(BlkOutputFormat::Json),
				"raw" => None,
				_ => {
					return Err((
						StatusCode::BAD_REQUEST,
						format!("unknown output format: {f}"),
					))
				},
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
) -> Result<Vec<u8>, (StatusCode, String)> {
	let req = FileRequest::from_path_and_query(state.clone(), &path, &params).await?;
	let res = UnpackedVromfs::unpack_one(state.clone(), dbg!(req)).await;

	Ok(res?)
}
