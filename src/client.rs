use crate::settings::Settings;
use crate::transport::Transport;
use serde_json::{Map, Value};
use std::sync::Mutex;

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
        // TODO: Implement
        Err(anyhow::anyhow!("Not yet implemented"))
    }

    /// Get an access token
    pub fn token(&self) -> anyhow::Result<String> {
        // TODO: Implement
        Err(anyhow::anyhow!("Not yet implemented"))
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
