use crate::settings::Settings;
use crate::transport::{HttpRequest, HttpResponse, Transport};
use serde_json::{Map, Value};
use std::sync::Mutex;
use url::Url;

/// HTTP client for ODC API
#[allow(dead_code)]
pub struct Client {
    pub settings: Settings,
    pub output: std::sync::Arc<crate::output::Output>,
    transport: Box<dyn Transport>,
    auth_mutex: Mutex<AuthState>,
    apps_cache: Mutex<Option<Vec<Map<String, Value>>>>,
}

#[allow(dead_code)]
#[derive(Default)]
struct AuthState {
    discovery: Option<Map<String, Value>>,
}

impl Client {
    /// Create a new client with production transport
    pub fn new(settings: Settings, output: std::sync::Arc<crate::output::Output>) -> Self {
        use crate::transport::UreqTransport;
        Self {
            settings,
            output,
            transport: Box::new(UreqTransport::new()),
            auth_mutex: Mutex::new(AuthState::default()),
            apps_cache: Mutex::new(None),
        }
    }

    /// Create a client with a custom transport (for testing)
    pub fn with_transport<T>(
        settings: Settings,
        output: std::sync::Arc<crate::output::Output>,
        transport: T,
    ) -> Self
    where
        T: Transport + 'static,
    {
        Self {
            settings,
            output,
            transport: Box::new(transport),
            auth_mutex: Mutex::new(AuthState::default()),
            apps_cache: Mutex::new(None),
        }
    }

    /// Get the OpenID discovery document
    pub fn discover(&self) -> anyhow::Result<Map<String, Value>> {
        // TODO: HTTP discover call
        Err(anyhow::anyhow!(
            "discover: HTTP transport not yet implemented"
        ))
    }

    /// Get an access token
    pub fn token(&self) -> anyhow::Result<String> {
        // TODO: OAuth token request
        Err(anyhow::anyhow!("token: HTTP transport not yet implemented"))
    }

    /// List all apps in the tenant
    pub fn list_apps(&self) -> anyhow::Result<Vec<Map<String, Value>>> {
        // TODO: HTTP /assets call
        Err(anyhow::anyhow!(
            "list_apps: HTTP transport not yet implemented"
        ))
    }

    /// List environments in the tenant
    pub fn list_environments(&self) -> anyhow::Result<Vec<Map<String, Value>>> {
        // TODO: HTTP /environments call
        Err(anyhow::anyhow!(
            "list_environments: HTTP transport not yet implemented"
        ))
    }

    /// Low-level HTTP call (for future implementation)
    #[allow(dead_code)]
    fn call_raw(
        &self,
        method: &str,
        url: &str,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    ) -> anyhow::Result<HttpResponse> {
        let url = Url::parse(url)?;

        let req = HttpRequest {
            method: method.to_string(),
            url,
            headers,
            body: body.unwrap_or_default(),
        };

        self.transport.send(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let settings = Settings {
            tenant_url: "https://example.com".to_string(),
            client_id: "test-id".to_string(),
            client_secret: "test-secret".to_string(),
        };
        let output = std::sync::Arc::new(crate::output::Output::new(
            false,
            crate::output::ColorMode::Never,
        ));
        let _client = Client::new(settings, output);
    }
}
