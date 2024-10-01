use http::StatusCode;

pub trait EyreToApiError<T> {
	fn convert_err(self) -> Result<T, (StatusCode, String)>;
}

impl<T> EyreToApiError<T> for Result<T, color_eyre::Report> {
	fn convert_err(self) -> Result<T, (StatusCode, std::string::String)> {
		match self {
			Ok(e) => {Ok(e)}
			Err(e) => {Err((StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")))}
		}
	}
}