use std::fmt::{Debug, Display};

use http::StatusCode;

pub trait EyreToApiError<T> {
	fn convert_err(self) -> Result<T, (StatusCode, String)>;
}

impl<T, E> EyreToApiError<T> for Result<T, E>
where
	E: Debug,
{
	fn convert_err(self) -> Result<T, (StatusCode, String)> {
		match self {
			Ok(e) => Ok(e),
			Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#?}"))),
		}
	}
}
