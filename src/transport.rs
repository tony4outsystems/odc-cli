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

/// Reqwest-based HTTP transport
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;
        Ok(Self { client })
    }
}

impl Transport for ReqwestTransport {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse> {
        // Block in place to safely call async from sync context
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { send_request_async(&self.client, req).await })
        })
    }
}

async fn send_request_async(client: &reqwest::Client, req: HttpRequest) -> Result<HttpResponse> {
    let mut request = match req.method.to_uppercase().as_str() {
        "GET" => client.get(req.url.clone()),
        "POST" => client.post(req.url.clone()),
        "PUT" => client.put(req.url.clone()),
        "DELETE" => client.delete(req.url.clone()),
        "PATCH" => client.patch(req.url.clone()),
        m => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", m)),
    };

    // Add headers
    for (name, value) in &req.headers {
        request = request.header(name, value);
    }

    // Add body if present
    if !req.body.is_empty() {
        request = request.body(req.body.clone());
    }

    // Send request
    let resp = request.send().await?;
    let status = resp.status().as_u16();
    let headers = resp
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|v| (name.as_str().to_string(), v.to_string()))
        })
        .collect();
    let body = resp.bytes().await?.to_vec();

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
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

/// Lets a single transport instance be shared (e.g. between the API auth client and the
/// Mentor MCP client, which both need to issue requests through the same transport) without
/// requiring the underlying implementation to be `Clone`.
impl Transport for std::sync::Arc<dyn Transport> {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse> {
        (**self).send(req)
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
