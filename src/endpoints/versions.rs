use std::{fmt::Write, sync::Arc};

use axum::{extract::State, Json};
use serde::Serialize;

use crate::{app_state::AppState, error::ApiError, eyre_error_translation::EyreToApiError};

#[utoipa::path(
	get,
	path = "/metadata/versions",
	responses(
        (status = 200, description = "Lists all versions available to fetch", content_type = ["text/plain"]),
	)
)]
pub async fn list_versions(State(state): State<Arc<AppState>>) -> ApiError<String> {
	let mut res = String::new();
	let vers = state.vromf_cache.list_versions();
	for v in vers {
		writeln!(res, "{}", v.key()).convert_err()?;
	}
	Ok(res)
}
