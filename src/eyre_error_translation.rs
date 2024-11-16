use std::fmt::{Debug, Display};

use http::StatusCode;
use tracing::error;

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
			Err(e) => conv_err(e),
		}
	}
}

pub trait OptionToApiError<T> {
	fn convert_err(self, msg: &str) -> Result<T, (StatusCode, String)>;
}

impl<T> OptionToApiError<T> for Option<T> {
	fn convert_err(self, msg: &str) -> Result<T, (StatusCode, String)> {
		match self {
			Some(e) => Ok(e),
			None => Err((StatusCode::INTERNAL_SERVER_ERROR, msg.to_owned())),
		}
	}
}

fn conv_err<T>(e: impl Debug) -> Result<T, (StatusCode, String)> {
	if cfg!(feature = "debug-err") {
		error!("{e:#?}");
		Err((StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#?}")))
	} else {
		Err((StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#?}")))
	}
}
