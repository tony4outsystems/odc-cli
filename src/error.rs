//! Custom error types for ODC CLI.
//!
//! Defines [`OdcError`] for structured error handling across the CLI.
//! Enables type-safe error matching and recovery strategies.

use std::fmt;
use std::io;

/// ODC CLI errors with structured context for debugging and recovery.
#[derive(Debug)]
pub enum OdcError {
    /// HTTP API error with status code and response body
    ApiError {
        status: u16,
        body: String,
        endpoint: String,
    },
    /// OAuth2 or authentication failure
    AuthenticationError(String),
    /// Invalid user input (missing field, bad format, etc.)
    ValidationError { field: String, reason: String },
    /// Failed to resolve a user-supplied identifier (app, environment, user, etc.)
    ResolutionError {
        kind: String,
        input: String,
        suggestions: Vec<String>,
    },
    /// Invalid or missing configuration
    InvalidConfiguration(String),
    /// I/O error (file not found, permission denied, etc.)
    Io(io::Error),
    /// Generic error that doesn't fit other categories
    Other(String),
}

impl fmt::Display for OdcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OdcError::ApiError {
                status,
                body,
                endpoint,
            } => {
                write!(f, "API error from {}: {} ", endpoint, status)?;
                if !body.is_empty() {
                    write!(f, "{}", body)?;
                }
                Ok(())
            }
            OdcError::AuthenticationError(msg) => {
                write!(f, "Authentication failed: {}", msg)
            }
            OdcError::ValidationError { field, reason } => {
                write!(f, "Invalid {}: {}", field, reason)
            }
            OdcError::ResolutionError {
                kind,
                input,
                suggestions,
            } => {
                write!(f, "No {} found matching {:?}", kind, input)?;
                if !suggestions.is_empty() {
                    write!(f, ". Did you mean:")?;
                    for suggestion in suggestions.iter().take(10) {
                        write!(f, "\n  - {}", suggestion)?;
                    }
                    if suggestions.len() > 10 {
                        write!(f, "\n  ... and {} more", suggestions.len() - 10)?;
                    }
                }
                Ok(())
            }
            OdcError::InvalidConfiguration(msg) => {
                write!(f, "Invalid configuration: {}", msg)
            }
            OdcError::Io(err) => {
                write!(f, "I/O error: {}", err)
            }
            OdcError::Other(msg) => {
                write!(f, "{}", msg)
            }
        }
    }
}

impl std::error::Error for OdcError {}

impl From<io::Error> for OdcError {
    fn from(err: io::Error) -> Self {
        OdcError::Io(err)
    }
}

impl From<url::ParseError> for OdcError {
    fn from(err: url::ParseError) -> Self {
        OdcError::InvalidConfiguration(format!("Invalid URL: {}", err))
    }
}

impl From<serde_json::Error> for OdcError {
    fn from(err: serde_json::Error) -> Self {
        OdcError::Other(format!("JSON error: {}", err))
    }
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
    fn test_resolution_error_display() {
        let err = OdcError::ResolutionError {
            kind: "app".to_string(),
            input: "MyAp".to_string(),
            suggestions: vec!["MyApp".to_string(), "MyApplication".to_string()],
        };
        let msg = err.to_string();
        assert!(msg.contains("MyAp"));
        assert!(msg.contains("MyApp"));
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
}
