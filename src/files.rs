use std::{path::Path as StdPath, str::FromStr, sync::Arc, time::Instant};

use axum::{
	body::Body,
	extract::{Path, Query, State},
	response::{IntoResponse, Response},
};
use dashmap::DashMap;
use http::StatusCode;
use serde::Deserialize;
use strum::VariantArray;
use tokio::task::spawn_blocking;
use utoipa::{IntoParams, ToSchema};
use wt_blk::vromf::{BlkOutputFormat, File, VromfUnpacker, ZipFormat};
use wt_version::Version;

use crate::{
	app_state::AppState,
	error::ApiError,
	eyre_error_translation::{EyreToApiError, OptionToApiError},
	get_vromfs::fetch_vromf,
	vromf_enum::VromfType,
};

pub struct UnpackedVromfs {
	unpackers: DashMap<(Version, VromfType), VromfUnpacker>,
}

impl UnpackedVromfs {
	pub async fn unpack_one(state: Arc<AppState>, req: Arc<FileRequest>) -> ApiError<Vec<u8>> {
		Self::cache_unpacker(&state.unpacked_vromfs, state.clone(), &req).await?;

		let vromf = req.vromf;
		let res = state
			.clone()
			.spawn_worker(move |s| {
				let res = || {
					let unpacker = state
						.unpacked_vromfs
						.unpackers
						.get(&(req.version, vromf))
						.convert_err("cache unpacker did not insert requested vromf")?;

					let res = unpacker
						.unpack_one(StdPath::new(&req.path), req.unpack_format, true)
						.convert_err()?;
					Ok(res)
				};
				s.send(res()).expect("channel to remain open after work");
			})
			.await??;

		Ok(res.split().1)
	}

	pub async fn unpack_zip(state: Arc<AppState>, req: Arc<FileRequest>) -> ApiError<Vec<u8>> {
		Self::cache_unpacker(&state.unpacked_vromfs, state.clone(), &req).await?;

		let vromf = req.vromf;
		let res = state
			.clone()
			.spawn_worker(move |s| {
				let res = || {
					let unpacker = state
						.unpacked_vromfs
						.unpackers
						.get(&(req.version, vromf))
						.convert_err("cache unpacker did not insert requested vromf")?;

					let res = unpacker
						.unpack_subfolder_to_zip(
							&req.path,
							true,
							ZipFormat::Uncompressed,
							req.unpack_format,
							true,
							true, // TODO: Set this false when the system is under very high load
						)
						.convert_err();
					Ok(res)
				};
				s.send(res()).expect("channel to remain open after work");
			})
			.await???;

		Ok(res)
	}

	/// Ensures that unpacker is cached
	pub async fn cache_unpacker(
		&self,
		state: Arc<AppState>,
		req: &FileRequest,
	) -> Result<(), (StatusCode, String)> {
		if state
			.unpacked_vromfs
			.unpackers
			.contains_key(&(req.version, req.vromf))
		{
			return Ok(());
		}

		let mut ask_api = true;
		for vromf in VromfType::VARIANTS {
			let buf =
				fetch_vromf(state.clone(), Some(req.version), req.vromf, &mut ask_api).await?;
			state.unpacked_vromfs.unpackers.insert(
				(req.version, *vromf),
				VromfUnpacker::from_file(&File::from_raw(vromf.into(), buf), false)
					.convert_err()?,
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

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
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

#[derive(Debug, Deserialize, IntoParams)]
pub struct Params {
	#[param(example = "latest", default = "Latest available")]
	/// Either version string or literal "latest"
	version: Option<String>,
	#[param(example = "json", default = "json")]
	/// Format to convert BLK to. One of: [raw, blk, json]
	format:  Option<String>,
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
		let latest = state.vromf_cache.latest_known_version();
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
		let single_file = path.contains('.');

		Ok(Self {
			version: query
				.version
				.clone()
				.filter(|v| v != "latest") // Latest string just gets turned into none
				.map(|e| {
					Version::from_str(&e).convert_err().map_err(|e| {
						(
							e.0,
							format!(
								"Invalid version: {}",
								query.version.clone().unwrap_or_default()
							),
						)
					})
				})
				.transpose()?
				.unwrap_or(latest),
			path,
			unpack_format,
			single_file,
			vromf,
		})
	}
}

#[utoipa::path(
	get,
	path = "/files/{path}",
	params(
		("path" = String, description = "The file/folder path to retrieve from the vromf", example = "aces.vromfs.bin/gamedata/weapons/rocketguns/fr_mica_em.blk"),
		Params
	),
	responses(
        (status = 200, description = "Plaintext or binary depending on format and file", content_type = ["text/plain", "application/octet-stream"]),
		(status = 404, description = "Provided path is not in vromf"),
		(status = 400, description = "Format specifier invalid"),
	)
)]
pub async fn get_files(
	State(state): State<Arc<AppState>>,
	Path(path): Path<String>,
	Query(params): Query<Params>,
) -> ApiError<impl IntoResponse> {
	let req = FileRequest::from_path_and_query(state.clone(), &path, &params).await?;

	let return_body = |(res, content_type): (Vec<u8>, _)| {
		Ok(Response::builder()
			.header("Content-Type", content_type)
			.body(Body::from(res))
			.convert_err()?)
	};

	if let Some(res) = state.files_cache.get(&req).await {
		return return_body(res);
	}

	// From here on req gets passed to a bunch of threads so we share it
	let req = Arc::new(req);

	let (res, content_type) = if req.single_file {
		let t = match &req.unpack_format {
			None => "application/octet-stream",
			Some(f) => {
				if req.path.ends_with("blk") {
					match f {
						BlkOutputFormat::Json => "application/json",
						BlkOutputFormat::BlkText => "text/plain",
					}
				} else {
					"application/octet-stream"
				}
			},
		};
		let res = UnpackedVromfs::unpack_one(state.clone(), req.clone()).await?;
		(res, t)
	} else {
		let res = UnpackedVromfs::unpack_zip(state.clone(), req.clone()).await?;
		(res, "application/zip")
	};

	state
		.files_cache
		.insert(
			Arc::<FileRequest>::unwrap_or_clone(req),
			(res.clone(), content_type),
		)
		.await;

	return_body((res, content_type))
}
