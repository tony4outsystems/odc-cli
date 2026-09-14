use crate::settings::Settings;
use crate::transport::{HttpRequest, HttpResponse, Transport};
use serde_json::{json, Map, Value};
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

#[derive(Default)]
struct AuthState {
    token: Option<String>,
    discovery: Option<Map<String, Value>>,
}

impl Client {
    /// Create a new client with production transport
    pub fn new(settings: Settings, output: std::sync::Arc<crate::output::Output>) -> Self {
        use crate::transport::ReqwestTransport;
        Self {
            settings,
            output,
            transport: Box::new(ReqwestTransport::new()),
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
        let mut auth = self.auth_mutex.lock().unwrap();

        if let Some(disc) = &auth.discovery {
            return Ok(disc.clone());
        }

        let tenant_origin = self.settings.tenant_origin();
        let url = format!(
            "{}/identity/.well-known/openid-configuration",
            tenant_origin
        );
        let resp = self.call_raw("GET", &url, Vec::new(), None)?;

        if resp.status >= 400 {
            let body_str = String::from_utf8(resp.body).unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Discovery failed with {}: {}",
                resp.status,
                body_str
            ));
        }

        let body_str = String::from_utf8(resp.body)?;
        let discovery: Map<String, Value> = serde_json::from_str(&body_str)?;
        auth.discovery = Some(discovery.clone());

        Ok(discovery)
    }

    /// Get an access token
    pub fn token(&self) -> anyhow::Result<String> {
        let auth = self.auth_mutex.lock().unwrap();

        if let Some(token) = &auth.token {
            return Ok(token.clone());
        }

        drop(auth); // Release lock before calling discover

        let discovery = self.discover()?;

        let token_endpoint =
            crate::value::str(discovery.get("token_endpoint").unwrap_or(&Value::Null));
        if token_endpoint.is_empty() {
            return Err(anyhow::anyhow!("token_endpoint not found in discovery"));
        }

        let body = format!(
            "grant_type=client_credentials&client_id={}&client_secret={}",
            self.settings.client_id, self.settings.client_secret
        );

        let resp = self.call_raw(
            "POST",
            &token_endpoint,
            vec![(
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            )],
            Some(body.into_bytes()),
        )?;

        if resp.status >= 400 {
            let body_str = String::from_utf8(resp.body).unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Token request failed with {}: {}",
                resp.status,
                body_str
            ));
        }

        let body_str = String::from_utf8(resp.body)?;
        let token_resp: Value = serde_json::from_str(&body_str)?;

        let token = crate::value::require_string(
            token_resp.get("access_token").unwrap_or(&Value::Null),
            "access_token",
        )?;

        let mut auth = self.auth_mutex.lock().unwrap();
        auth.token = Some(token.clone());

        Ok(token)
    }

    /// Call an API endpoint and return parsed JSON
    pub fn call(&self, method: &str, path: &str) -> anyhow::Result<Value> {
        let token = self.token()?;
        let tenant_origin = self.settings.tenant_origin();
        let url = format!("{}{}", tenant_origin, path);

        let headers = vec![
            ("Authorization".to_string(), format!("Bearer {}", token)),
            ("Content-Type".to_string(), "application/json".to_string()),
        ];

        let resp = self.call_raw(method, &url, headers, None)?;

        if resp.status >= 400 {
            let body_str = String::from_utf8(resp.body).unwrap_or_default();
            return Err(anyhow::anyhow!(
                "{} {} failed with {}: {}",
                method,
                path,
                resp.status,
                body_str
            ));
        }

        if resp.body.is_empty() {
            return Ok(json!({}));
        }

        let body_str = String::from_utf8(resp.body)?;
        serde_json::from_str(&body_str)
            .map_err(|_| anyhow::anyhow!("Invalid JSON response from {}", path))
    }

    /// Low-level HTTP call
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

    /// List all apps in the tenant
    pub fn list_apps(&self) -> anyhow::Result<Vec<Map<String, Value>>> {
        let resp = self.call("GET", "/api/asset-repository/v1/assets")?;

        match resp {
            Value::Array(items) => {
                let mut result = Vec::new();
                for item in items {
                    if let Value::Object(map) = item {
                        result.push(map);
                    }
                }
                Ok(result)
            }
            Value::Object(map) => Ok(vec![map]),
            _ => Err(anyhow::anyhow!("Expected array or object from /assets")),
        }
    }

    /// List environments in the tenant
    pub fn list_environments(&self) -> anyhow::Result<Vec<Map<String, Value>>> {
        let resp = self.call("GET", "/api/portfolios/v2/environments")?;

        match resp {
            Value::Array(items) => {
                let mut result = Vec::new();
                for item in items {
                    if let Value::Object(map) = item {
                        result.push(map);
                    }
                }
                Ok(result)
            }
            _ => Err(anyhow::anyhow!("Expected array from /environments")),
        }
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
