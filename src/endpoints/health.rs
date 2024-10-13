use axum::Json;
use serde::Serialize;
use time::OffsetDateTime;

use crate::error::ApiError;

#[derive(Serialize, Clone, Debug)]
pub struct HealthResponse {
	time: String,
}

impl Default for HealthResponse {
	fn default() -> Self {
		Self {
			time: time::OffsetDateTime::now_utc().to_string(),
		}
	}
}

#[utoipa::path(
	get,
	path = "/health",
	responses(
        (status = 200, description = "UTC time of the server", content_type = ["text/json"]),
	)
)]
pub async fn health() -> ApiError<Json<HealthResponse>> {
	Ok(Json(HealthResponse::default()))
}
