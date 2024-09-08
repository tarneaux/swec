use crate::state_actor::{ServiceNotFoundError, WriteError};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::{error::Error, fmt::Display};

#[derive(Debug, Clone, Copy)]
pub enum ApiError {
    WriteError(WriteError),
    ServiceNotFoundError,
}

impl From<WriteError> for ApiError {
    fn from(value: WriteError) -> Self {
        Self::WriteError(value)
    }
}

impl From<ServiceNotFoundError> for ApiError {
    fn from(_: ServiceNotFoundError) -> Self {
        Self::ServiceNotFoundError
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WriteError(e) => e.fmt(f),
            Self::ServiceNotFoundError => ServiceNotFoundError.fmt(f),
        }
    }
}

impl Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::WriteError(WriteError::NameConflict) => {
                (StatusCode::CONFLICT, WriteError::NameConflict.to_string())
            }
            Self::ServiceNotFoundError | Self::WriteError(WriteError::NotFound) => {
                (StatusCode::NOT_FOUND, ServiceNotFoundError.to_string())
            }
        }
        .into_response()
    }
}
