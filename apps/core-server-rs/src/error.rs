use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::fmt::Display;

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

impl AppError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

pub fn internal_error(err: impl Display) -> (StatusCode, String) {
    tracing::error!(error = %err, "internal error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal server error".to_string(),
    )
}

pub fn map_db_error(err: sqlx::Error) -> (StatusCode, String) {
    let status = match &err {
        sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
        sqlx::Error::Database(db) => match db.code().as_deref() {
            Some("23505") => StatusCode::CONFLICT,    // unique_violation
            Some("23503") => StatusCode::BAD_REQUEST, // foreign_key_violation
            Some("23502") => StatusCode::BAD_REQUEST, // not_null_violation
            Some("22P02") => StatusCode::BAD_REQUEST, // invalid_text_representation
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        },
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    tracing::error!(error = %err, status = %status, "database error");

    let message = match status {
        StatusCode::NOT_FOUND => "Resource not found",
        StatusCode::CONFLICT => "Resource already exists",
        StatusCode::BAD_REQUEST => "Invalid request",
        _ => "Database error",
    };

    (status, message.to_string())
}

pub fn map_db_conflict(err: sqlx::Error, message: &str) -> (StatusCode, String) {
    if let sqlx::Error::Database(db) = &err {
        if db.code().as_deref() == Some("23505") {
            tracing::warn!(error = %err, "database conflict");
            return (StatusCode::CONFLICT, message.to_string());
        }
    }
    map_db_error(err)
}
