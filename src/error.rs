use http::StatusCode;

pub type ApiError<T> = Result<T, (StatusCode, String)>;
