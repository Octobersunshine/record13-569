use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("File too large: {0}")]
    FileTooLarge(String),

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("Invalid multipart request: {0}")]
    InvalidMultipart(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Compression error: {0}")]
    Compression(String),

    #[error("Model loading error: {0}")]
    ModelLoad(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_type) = match &self {
            AppError::FileTooLarge(_) => (StatusCode::PAYLOAD_TOO_LARGE, "file_too_large"),
            AppError::UnsupportedFormat(_) => (StatusCode::BAD_REQUEST, "unsupported_format"),
            AppError::InvalidMultipart(_) => (StatusCode::BAD_REQUEST, "invalid_multipart"),
            AppError::TaskNotFound(_) => (StatusCode::NOT_FOUND, "task_not_found"),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            AppError::Io(_)
            | AppError::Serde(_)
            | AppError::Compression(_)
            | AppError::ModelLoad(_)
            | AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };

        let body = Json(ErrorResponse {
            error: error_type.to_string(),
            message: self.to_string(),
        });

        (status, body).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
