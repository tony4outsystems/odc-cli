#![cfg(test)]

use crate::transport::{HttpRequest, HttpResponse, Transport};
use anyhow::Result;
use serde_json::json;

/// Test helper: creates a fake transport that responds to requests based on URL/method patterns
pub fn test_transport<F>(handler: F) -> impl Transport
where
    F: Fn(HttpRequest) -> Result<HttpResponse> + Send + Sync,
{
    handler
}

/// Build a JSON response for testing
pub fn json_response(status: u16, body: serde_json::Value) -> Result<HttpResponse> {
    Ok(HttpResponse {
        status,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: serde_json::to_vec(&body)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_response() {
        let resp = json_response(200, json!({"key": "value"}));
        assert!(resp.is_ok());
    }
}
