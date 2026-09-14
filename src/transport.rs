use anyhow::Result;
use url::Url;

/// An HTTP request to be sent by the transport
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: Url,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// An HTTP response received from the transport
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Transport trait for making HTTP requests
pub trait Transport: Send + Sync {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse>;
}

/// Ureq-based transport implementation
pub struct UreqTransport;

impl UreqTransport {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl Transport for UreqTransport {
    fn send(&self, _req: HttpRequest) -> Result<HttpResponse> {
        // TODO: Implement ureq integration
        Err(anyhow::anyhow!("Not yet implemented"))
    }
}

/// A closure-based transport for testing
impl<F> Transport for F
where
    F: Fn(HttpRequest) -> Result<HttpResponse> + Send + Sync,
{
    fn send(&self, req: HttpRequest) -> Result<HttpResponse> {
        self(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_request_creation() {
        let req = HttpRequest {
            method: "GET".to_string(),
            url: "https://example.com/api".parse().unwrap(),
            headers: vec![("Authorization".to_string(), "Bearer token".to_string())],
            body: vec![],
        };
        assert_eq!(req.method, "GET");
    }
}
