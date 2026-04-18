use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub(crate) struct AppHttpError {
    pub(crate) code: StatusCode,
    pub(crate) message: String,
}

impl AppHttpError {
    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

impl IntoResponse for AppHttpError {
    fn into_response(self) -> Response {
        (self.code, self.message).into_response()
    }
}
