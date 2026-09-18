//! Custom error types for ODC CLI.
//!
//! Defines [`OdcError`] for structured error handling across the CLI.
//! Enables type-safe error matching and recovery strategies.

use std::io;

/// ODC CLI errors with structured context for debugging and recovery.
#[derive(Debug, thiserror::Error)]
pub enum OdcError {
    /// HTTP API error with status code and response body
    #[error("API error from {endpoint}: {status} {body}")]
    ApiError {
        status: u16,
        body: String,
        endpoint: String,
    },
    /// OAuth2 or authentication failure
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    /// Invalid user input (missing field, bad format, etc.)
    #[error("Invalid {field}: {reason}")]
    ValidationError { field: String, reason: String },
    /// Invalid or missing configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    /// I/O error (file not found, permission denied, etc.)
    #[error(transparent)]
    Io(#[from] io::Error),
    /// URL parsing error
    #[error("Invalid URL: {0}")]
    UrlParse(#[from] url::ParseError),
    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    /// Generic error that doesn't fit other categories
    #[error("{0}")]
    Other(String),
}

/// Convert anyhow errors to OdcError during migration period
impl From<anyhow::Error> for OdcError {
    fn from(err: anyhow::Error) -> Self {
        OdcError::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_display() {
        let err = OdcError::ApiError {
            status: 500,
            body: "Internal server error".to_string(),
            endpoint: "/api/apps".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("500"));
        assert!(msg.contains("Internal server error"));
    }

    #[test]
    fn test_validation_error_display() {
        let err = OdcError::ValidationError {
            field: "app".to_string(),
            reason: "required".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Invalid app"));
        assert!(msg.contains("required"));
    }

    #[test]
    fn test_authentication_error_display() {
        let err = OdcError::AuthenticationError("invalid token".to_string());
        let msg = err.to_string();
        assert!(msg.contains("Authentication failed"));
        assert!(msg.contains("invalid token"));
    }
}
