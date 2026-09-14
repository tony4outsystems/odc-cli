use crate::settings::Settings;
use crate::transport::{HttpRequest, HttpResponse, Transport};
use serde_json::{json, Map, Value};
use std::sync::Mutex;
use url::Url;

/// A single page of apps: items plus the offset of the next page, if any.
pub type AppsPage = (Vec<Map<String, Value>>, Option<i64>);

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

    /// Fetch a single page of apps starting at `offset`, up to `limit` results.
    /// Returns the page's items along with the offset of the next page, if any.
    pub fn list_apps_page(&self, offset: i64, limit: i64) -> anyhow::Result<AppsPage> {
        let path = format!(
            "/api/asset-repository/v1/assets?limit={}&offset={}",
            limit, offset
        );
        let resp = self.call("GET", &path)?;

        let (results, page) = match resp {
            Value::Object(mut map) => {
                let results = map.remove("results").unwrap_or(Value::Array(Vec::new()));
                let page = map.remove("page");
                (results, page)
            }
            Value::Array(items) => (Value::Array(items), None),
            _ => return Err(anyhow::anyhow!("Expected object or array from /assets")),
        };

        let items = match results {
            Value::Array(items) => items
                .into_iter()
                .filter_map(|item| match item {
                    Value::Object(map) => Some(map),
                    _ => None,
                })
                .collect(),
            Value::Object(map) => vec![map],
            _ => Vec::new(),
        };

        let next_offset = page
            .as_ref()
            .and_then(|p| p.get("nextPageOffset"))
            .and_then(|v| v.as_i64())
            .filter(|&next| next > offset);

        Ok((items, next_offset))
    }

    /// List all apps in the tenant, following pagination until exhausted.
    pub fn list_apps(&self) -> anyhow::Result<Vec<Map<String, Value>>> {
        {
            let cache = self.apps_cache.lock().unwrap();
            if let Some(apps) = &*cache {
                return Ok(apps.clone());
            }
        }

        let mut apps = Vec::new();
        let mut offset: i64 = 0;
        const PAGE_SIZE: i64 = 100;

        loop {
            let (items, next_offset) = self.list_apps_page(offset, PAGE_SIZE)?;
            apps.extend(items);

            match next_offset {
                Some(next) => offset = next,
                None => break,
            }
        }

        let mut cache = self.apps_cache.lock().unwrap();
        *cache = Some(apps.clone());

        Ok(apps)
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

    fn test_settings() -> Settings {
        Settings {
            tenant_url: "https://example.com".to_string(),
            client_id: "test-id".to_string(),
            client_secret: "test-secret".to_string(),
        }
    }

    fn test_output() -> std::sync::Arc<crate::output::Output> {
        std::sync::Arc::new(crate::output::Output::new(
            false,
            crate::output::ColorMode::Never,
        ))
    }

    fn mock_transport() -> impl crate::transport::Transport {
        crate::testutil::test_transport(|req: HttpRequest| {
            let url = req.url.as_str();

            if url.contains("openid-configuration") {
                return crate::testutil::json_response(
                    200,
                    json!({"token_endpoint": "https://example.com/oauth/token"}),
                );
            }

            if url.contains("/oauth/token") {
                return crate::testutil::json_response(200, json!({"access_token": "test-token"}));
            }

            if url.contains("/assets") {
                if url.contains("offset=0") {
                    return crate::testutil::json_response(
                        200,
                        json!({
                            "results": [{"key": "app1"}, {"key": "app2"}],
                            "page": {"nextPageOffset": 2, "totalResults": 3},
                        }),
                    );
                }
                if url.contains("offset=2") {
                    return crate::testutil::json_response(
                        200,
                        json!({
                            "results": [{"key": "app3"}],
                            "page": {"nextPageOffset": 0, "totalResults": 3},
                        }),
                    );
                }
            }

            crate::testutil::json_response(404, json!({"error": "not found"}))
        })
    }

    #[test]
    fn test_list_apps_page_returns_next_offset() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let (items, next_offset) = client.list_apps_page(0, 2).unwrap();

        assert_eq!(items.len(), 2);
        assert_eq!(next_offset, Some(2));
    }

    #[test]
    fn test_list_apps_page_last_page_has_no_next_offset() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let (items, next_offset) = client.list_apps_page(2, 2).unwrap();

        assert_eq!(items.len(), 1);
        // nextPageOffset of 0 (<= current offset) means there is no next page.
        assert_eq!(next_offset, None);
    }

    #[test]
    fn test_list_apps_follows_pagination() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let apps = client.list_apps().unwrap();

        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0].get("key").unwrap(), "app1");
        assert_eq!(apps[2].get("key").unwrap(), "app3");
    }
}
