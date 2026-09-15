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
        self.call_with_body(method, path, None)
    }

    /// Call an API endpoint with a JSON request body and return parsed JSON.
    pub fn call_with_body(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> anyhow::Result<Value> {
        let token = self.token()?;
        let tenant_origin = self.settings.tenant_origin();
        let url = format!("{}{}", tenant_origin, path);

        let headers = vec![
            ("Authorization".to_string(), format!("Bearer {}", token)),
            ("Content-Type".to_string(), "application/json".to_string()),
        ];

        let body_bytes = body.map(|v| serde_json::to_vec(&v)).transpose()?;
        let resp = self.call_raw(method, &url, headers, body_bytes)?;

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

    /// Default page size used when fetching every page of a paginated listing.
    const DEFAULT_PAGE_SIZE: i64 = 100;

    /// Fetch a single page from an endpoint that follows the ODC API's `results` /
    /// `page.nextPageOffset` pagination convention (used by `/assets`,
    /// `/assets/{key}/revisions`, `/deployed-assets`, and similar list endpoints).
    /// `path` may already contain a `?query`; `limit`/`offset` are appended to it.
    /// Returns the page's items along with the offset of the next page, if any.
    fn fetch_page(&self, path: &str, offset: i64, limit: i64) -> anyhow::Result<AppsPage> {
        let separator = if path.contains('?') { '&' } else { '?' };
        let full_path = format!("{}{}limit={}&offset={}", path, separator, limit, offset);
        let resp = self.call("GET", &full_path)?;

        let (results, page) = match resp {
            Value::Object(mut map) => {
                let results = map.remove("results").unwrap_or(Value::Array(Vec::new()));
                let page = map.remove("page");
                (results, page)
            }
            Value::Array(items) => (Value::Array(items), None),
            _ => return Err(anyhow::anyhow!("Expected object or array from {}", path)),
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

    /// Fetch every page from an endpoint that follows the `results` / `page.nextPageOffset`
    /// pagination convention, combining them into a single list.
    fn fetch_all_pages(&self, path: &str) -> anyhow::Result<Vec<Map<String, Value>>> {
        let mut items = Vec::new();
        let mut offset: i64 = 0;

        loop {
            let (page_items, next_offset) =
                self.fetch_page(path, offset, Self::DEFAULT_PAGE_SIZE)?;
            items.extend(page_items);

            match next_offset {
                Some(next) => offset = next,
                None => break,
            }
        }

        Ok(items)
    }

    /// Fetch a single page of apps starting at `offset`, up to `limit` results.
    /// Returns the page's items along with the offset of the next page, if any.
    pub fn list_apps_page(&self, offset: i64, limit: i64) -> anyhow::Result<AppsPage> {
        self.fetch_page("/api/asset-repository/v1/assets", offset, limit)
    }

    /// List all apps in the tenant, following pagination until exhausted.
    pub fn list_apps(&self) -> anyhow::Result<Vec<Map<String, Value>>> {
        {
            let cache = self.apps_cache.lock().unwrap();
            if let Some(apps) = &*cache {
                return Ok(apps.clone());
            }
        }

        let apps = self.fetch_all_pages("/api/asset-repository/v1/assets")?;

        let mut cache = self.apps_cache.lock().unwrap();
        *cache = Some(apps.clone());

        Ok(apps)
    }

    /// Fetch a single page of an app's revisions starting at `offset`, up to `limit` results.
    /// Returns the page's items along with the offset of the next page, if any.
    pub fn list_revisions_page(
        &self,
        asset_key: &str,
        offset: i64,
        limit: i64,
    ) -> anyhow::Result<AppsPage> {
        let path = format!("/api/asset-repository/v1/assets/{}/revisions", asset_key);
        self.fetch_page(&path, offset, limit)
    }

    /// List all revisions of an app, following pagination until exhausted.
    pub fn list_revisions(&self, asset_key: &str) -> anyhow::Result<Vec<Map<String, Value>>> {
        let path = format!("/api/asset-repository/v1/assets/{}/revisions", asset_key);
        self.fetch_all_pages(&path)
    }

    /// List environments in the tenant
    pub fn list_environments(&self) -> anyhow::Result<Vec<Map<String, Value>>> {
        let resp = self.call("GET", "/api/portfolios/v2/environments")?;

        let items = match resp {
            // EnvironmentListResponse: {"results": [...]}
            Value::Object(mut map) => map.remove("results").unwrap_or(Value::Array(Vec::new())),
            Value::Array(items) => Value::Array(items),
            _ => {
                return Err(anyhow::anyhow!(
                    "Expected object or array from /environments"
                ))
            }
        };

        match items {
            Value::Array(items) => Ok(items
                .into_iter()
                .filter_map(|item| match item {
                    Value::Object(map) => Some(map),
                    _ => None,
                })
                .collect()),
            _ => Err(anyhow::anyhow!("Expected array from /environments results")),
        }
    }

    /// Fetch a single page of deployed assets starting at `offset`, up to `limit` results.
    /// Returns the page's items along with the offset of the next page, if any.
    pub fn list_deployed_apps_page(&self, offset: i64, limit: i64) -> anyhow::Result<AppsPage> {
        self.fetch_page("/api/portfolios/v2/deployed-assets", offset, limit)
    }

    /// List all deployed assets in the tenant, following pagination until exhausted.
    pub fn list_deployed_apps(&self) -> anyhow::Result<Vec<Map<String, Value>>> {
        self.fetch_all_pages("/api/portfolios/v2/deployed-assets")
    }

    /// Fetch a user by key (guid) via the identity API.
    pub fn get_user(&self, key: &str) -> anyhow::Result<Map<String, Value>> {
        let path = format!("/api/identity/v1/users/{}", key);
        let resp = self.call("GET", &path)?;
        match resp {
            Value::Object(map) => Ok(map),
            _ => Err(anyhow::anyhow!("Expected object from {}", path)),
        }
    }

    /// Fetch the producer dependency tree for an asset revision.
    ///
    /// Returns the top-level producers; each producer's own `producers` field (if present)
    /// already carries its nested dependencies, since the API returns the full tree in one call.
    pub fn get_producer_graph(
        &self,
        asset_key: &str,
        revision: i32,
        max_depth: i32,
        producer_type_filter: &str,
        environment_key: &str,
    ) -> anyhow::Result<Vec<Map<String, Value>>> {
        let mut path = format!(
            "/api/dependency-management/v1/assets/{}/revisions/{}/producer-graph?maxDepth={}&producerTypeFilter={}",
            asset_key, revision, max_depth, producer_type_filter
        );
        if !environment_key.is_empty() {
            path.push_str(&format!("&environmentKey={}", environment_key));
        }

        let resp = self.call("GET", &path)?;
        match resp {
            Value::Object(mut map) => match map.remove("results") {
                Some(Value::Array(items)) => Ok(items
                    .into_iter()
                    .filter_map(|item| match item {
                        Value::Object(map) => Some(map),
                        _ => None,
                    })
                    .collect()),
                _ => Ok(Vec::new()),
            },
            _ => Err(anyhow::anyhow!("Expected object from {}", path)),
        }
    }

    /// Get the source code binary URL for an asset revision.
    pub fn get_source_code_url(&self, asset_key: &str, revision: i32) -> anyhow::Result<String> {
        let path = format!(
            "/api/asset-repository/v1/assets/{}/revisions/{}/source-code",
            asset_key, revision
        );
        let resp = self.call("GET", &path)?;
        Self::expect_object(resp, &path)?
            .remove("sourceCodeBinaryUrl")
            .and_then(|v| v.as_str().map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("{} has no sourceCodeBinaryUrl", path))
    }

    /// Download the raw bytes at `url` (a pre-signed storage URL; no API token is sent).
    pub fn download_file_bytes(&self, url: &str) -> anyhow::Result<Vec<u8>> {
        let resp = self.call_raw("GET", url, Vec::new(), None)?;
        if resp.status >= 400 {
            let body_str = String::from_utf8(resp.body).unwrap_or_default();
            return Err(anyhow::anyhow!(
                "GET {} failed with {}: {}",
                url,
                resp.status,
                body_str
            ));
        }
        Ok(resp.body)
    }

    /// Request a pre-signed URL to upload a file to.
    pub fn request_upload_url(&self) -> anyhow::Result<String> {
        let resp = self.call_with_body("POST", "/api/asset-repository/v1/uploads", None)?;
        Self::expect_object(resp, "/api/asset-repository/v1/uploads")?
            .remove("uploadUrl")
            .and_then(|v| v.as_str().map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("/uploads response has no uploadUrl"))
    }

    /// Upload raw bytes to a pre-signed storage URL (no API token is sent).
    pub fn upload_file_bytes(&self, url: &str, bytes: Vec<u8>) -> anyhow::Result<()> {
        let headers = vec![(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        )];
        let resp = self.call_raw("PUT", url, headers, Some(bytes))?;
        if resp.status >= 400 {
            let body_str = String::from_utf8(resp.body).unwrap_or_default();
            return Err(anyhow::anyhow!(
                "PUT {} failed with {}: {}",
                url,
                resp.status,
                body_str
            ));
        }
        Ok(())
    }

    /// Create a new asset, or a new revision of an existing one, from an uploaded file's URI.
    /// The asset key is derived server-side from the file's embedded module key.
    pub fn create_asset_revision(&self, file_uri: &str) -> anyhow::Result<Map<String, Value>> {
        let body = json!({ "fileUri": file_uri });
        let resp = self.call_with_body("POST", "/api/asset-repository/v1/assets", Some(body))?;
        Self::expect_object(resp, "/api/asset-repository/v1/assets")
    }

    /// Find a user by exact email (case-insensitive) via the identity API's substring search.
    pub fn find_user_by_email(&self, email: &str) -> anyhow::Result<Map<String, Value>> {
        self.search_users(email)?
            .into_iter()
            .find(|u| {
                u.get("email")
                    .and_then(|v| v.as_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case(email))
            })
            .ok_or_else(|| anyhow::anyhow!("User not found: {}", email))
    }

    /// Search users whose name, email, or username contains `query` (case-insensitive,
    /// server-side), following pagination until exhausted.
    pub fn search_users(&self, query: &str) -> anyhow::Result<Vec<Map<String, Value>>> {
        let encoded =
            percent_encoding::utf8_percent_encode(query, percent_encoding::NON_ALPHANUMERIC);
        let path = format!(
            "/api/identity/v1/users?nameOrEmailOrUsernameContains={}",
            encoded
        );
        self.fetch_all_pages(&path)
    }

    /// Update a user's mutable fields (name, isActive, photoUrl).
    pub fn update_user(
        &self,
        key: &str,
        updates: &Map<String, Value>,
    ) -> anyhow::Result<Map<String, Value>> {
        let path = format!("/api/identity/v1/users/{}", key);
        let resp = self.call_with_body("PATCH", &path, Some(Value::Object(updates.clone())))?;
        Self::expect_object(resp, &path)
    }

    /// List application roles whose name contains `name_contains` (case-insensitive, server-side).
    pub fn list_application_roles(
        &self,
        name_contains: &str,
    ) -> anyhow::Result<Vec<Map<String, Value>>> {
        let encoded = percent_encoding::utf8_percent_encode(
            name_contains,
            percent_encoding::NON_ALPHANUMERIC,
        );
        let path = format!(
            "/api/identity/v1/application-roles?nameContains={}",
            encoded
        );
        self.fetch_all_pages(&path)
    }

    /// Grant an application role to a user.
    pub fn grant_role(&self, user_key: &str, role_key: &str) -> anyhow::Result<()> {
        let path = format!(
            "/api/identity/v1/users/{}/application-roles/{}",
            user_key, role_key
        );
        self.call("POST", &path)?;
        Ok(())
    }

    /// Revoke an application role from a user.
    pub fn revoke_role(&self, user_key: &str, role_key: &str) -> anyhow::Result<()> {
        let path = format!(
            "/api/identity/v1/users/{}/application-roles/{}",
            user_key, role_key
        );
        self.call("DELETE", &path)?;
        Ok(())
    }

    /// Permanently delete an asset (app).
    pub fn delete_asset(&self, asset_key: &str) -> anyhow::Result<()> {
        let path = format!("/api/asset-repository/v1/assets/{}", asset_key);
        self.call("DELETE", &path)?;
        Ok(())
    }

    /// Start a build for an asset revision. Returns the `BuildResponse` (`buildKey`, `isSuccessful`).
    pub fn start_build(
        &self,
        asset_key: &str,
        revision: i32,
        build_type: &str,
    ) -> anyhow::Result<Map<String, Value>> {
        let body = json!({
            "assetKey": asset_key,
            "assetRevision": revision,
            "buildType": build_type,
        });
        let resp = self.call_with_body("POST", "/api/builds/v1/build-operations", Some(body))?;
        Self::expect_object(resp, "/api/builds/v1/build-operations")
    }

    /// Fetch a build's current status (`BuildDetails`).
    pub fn get_build(&self, build_key: &str) -> anyhow::Result<Map<String, Value>> {
        let path = format!("/api/builds/v1/build-operations/{}", build_key);
        let resp = self.call("GET", &path)?;
        Self::expect_object(resp, &path)
    }

    /// Start publishing an asset revision to an environment (omit `environment_key` for the
    /// hotfix scenario). Returns the `PublishOperationResponse`.
    pub fn start_publish(
        &self,
        asset_key: &str,
        revision: i32,
        environment_key: &str,
    ) -> anyhow::Result<Map<String, Value>> {
        let mut body = json!({
            "operation": "Publish",
            "assetKey": asset_key,
            "revision": revision,
        });
        if !environment_key.is_empty() {
            body["environmentKey"] = json!(environment_key);
        }
        let resp =
            self.call_with_body("POST", "/api/deployments/v1/publish-operations", Some(body))?;
        Self::expect_object(resp, "/api/deployments/v1/publish-operations")
    }

    /// Fetch a publish operation's current status.
    pub fn get_publish(&self, operation_key: &str) -> anyhow::Result<Map<String, Value>> {
        let path = format!("/api/deployments/v1/publish-operations/{}", operation_key);
        let resp = self.call("GET", &path)?;
        Self::expect_object(resp, &path)
    }

    /// Start a deployment operation (`Deploy` or `Undeploy`). `revision`/`build_key` are
    /// required for `Deploy`, and unused for `Undeploy`. Returns the `DeploymentOperationResponse`.
    pub fn start_deployment_operation(
        &self,
        operation: &str,
        asset_key: &str,
        environment_key: &str,
        revision: Option<i32>,
        build_key: Option<&str>,
    ) -> anyhow::Result<Map<String, Value>> {
        let mut body = json!({
            "operation": operation,
            "assetKey": asset_key,
            "environmentKey": environment_key,
        });
        if let Some(revision) = revision {
            body["revision"] = json!(revision);
        }
        if let Some(build_key) = build_key {
            body["buildKey"] = json!(build_key);
        }
        let resp = self.call_with_body(
            "POST",
            "/api/deployments/v1/deployment-operations",
            Some(body),
        )?;
        Self::expect_object(resp, "/api/deployments/v1/deployment-operations")
    }

    /// Fetch a deployment operation's current status.
    pub fn get_deployment_operation(
        &self,
        operation_key: &str,
    ) -> anyhow::Result<Map<String, Value>> {
        let path = format!(
            "/api/deployments/v1/deployment-operations/{}",
            operation_key
        );
        let resp = self.call("GET", &path)?;
        Self::expect_object(resp, &path)
    }

    /// Launch a deployment impact analysis for an asset revision against an environment.
    /// Returns the `ReferenceKeyResponse` (`analysisKey`).
    pub fn start_deployment_analysis(
        &self,
        asset_key: &str,
        revision: i32,
        environment_key: &str,
    ) -> anyhow::Result<Map<String, Value>> {
        let body = json!({
            "assetKey": asset_key,
            "revision": revision,
            "environmentKey": environment_key,
        });
        let resp = self.call_with_body(
            "POST",
            "/api/dependency-management/v1/deployment-analyses",
            Some(body),
        )?;
        Self::expect_object(resp, "/api/dependency-management/v1/deployment-analyses")
    }

    /// Fetch a deployment analysis's current status/report.
    pub fn get_deployment_analysis(
        &self,
        analysis_key: &str,
    ) -> anyhow::Result<Map<String, Value>> {
        let path = format!(
            "/api/dependency-management/v1/deployment-analyses/{}",
            analysis_key
        );
        let resp = self.call("GET", &path)?;
        Self::expect_object(resp, &path)
    }

    /// Launch a deletion impact analysis for an asset. Returns the `ReferenceKeyResponse`
    /// (`analysisKey`).
    pub fn start_deletion_analysis(&self, asset_key: &str) -> anyhow::Result<Map<String, Value>> {
        let body = json!({ "assetKey": asset_key });
        let resp = self.call_with_body(
            "POST",
            "/api/dependency-management/v1/deletion-analyses",
            Some(body),
        )?;
        Self::expect_object(resp, "/api/dependency-management/v1/deletion-analyses")
    }

    /// Fetch a deletion analysis's current status/report.
    pub fn get_deletion_analysis(&self, analysis_key: &str) -> anyhow::Result<Map<String, Value>> {
        let path = format!(
            "/api/dependency-management/v1/deletion-analyses/{}",
            analysis_key
        );
        let resp = self.call("GET", &path)?;
        Self::expect_object(resp, &path)
    }

    fn expect_object(value: Value, path: &str) -> anyhow::Result<Map<String, Value>> {
        match value {
            Value::Object(map) => Ok(map),
            _ => Err(anyhow::anyhow!("Expected object from {}", path)),
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
            ..Default::default()
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
            ..Default::default()
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

            if url.contains("/producer-graph") {
                return crate::testutil::json_response(
                    200,
                    json!({
                        "results": [
                            {"key": "producer1", "name": "Producer One", "revision": 3}
                        ]
                    }),
                );
            }

            if url.contains("/source-code") {
                return crate::testutil::json_response(
                    200,
                    json!({"sourceCodeBinaryUrl": "https://storage.example.com/app1-rev-3.oml"}),
                );
            }

            if url.contains("storage.example.com") && req.method == "GET" {
                return Ok(crate::transport::HttpResponse {
                    status: 200,
                    headers: vec![],
                    body: b"<oml/>".to_vec(),
                });
            }

            if url.contains("storage.example.com") && req.method == "PUT" {
                return Ok(crate::transport::HttpResponse {
                    status: 200,
                    headers: vec![],
                    body: Vec::new(),
                });
            }

            if url.contains("/uploads") {
                return crate::testutil::json_response(
                    201,
                    json!({"uploadUrl": "https://storage.example.com/upload-1"}),
                );
            }

            if url.contains("/revisions") {
                if url.contains("offset=0") {
                    return crate::testutil::json_response(
                        200,
                        json!({
                            "results": [{"revision": 1}],
                            "page": {"nextPageOffset": 1, "totalResults": 2},
                        }),
                    );
                }
                if url.contains("offset=1") {
                    return crate::testutil::json_response(
                        200,
                        json!({
                            "results": [{"revision": 2}],
                            "page": {"nextPageOffset": 0, "totalResults": 2},
                        }),
                    );
                }
            }

            if url.contains("/assets/app1") && req.method == "DELETE" {
                return Ok(crate::transport::HttpResponse {
                    status: 204,
                    headers: vec![],
                    body: Vec::new(),
                });
            }

            if url.ends_with("/assets") && req.method == "POST" {
                return crate::testutil::json_response(
                    201,
                    json!({"assetKey": "app1", "revision": 4}),
                );
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

            if url.contains("/environments") {
                return crate::testutil::json_response(
                    200,
                    json!({"results": [{"key": "env1", "name": "Development"}]}),
                );
            }

            if url.contains("/deployed-assets") {
                return crate::testutil::json_response(
                    200,
                    json!({
                        "results": [{"key": "app1", "type": "WebApplication"}],
                        "page": {"nextPageOffset": 0, "totalResults": 1},
                    }),
                );
            }

            if url.contains("/build-operations") && req.method == "POST" {
                return crate::testutil::json_response(
                    201,
                    json!({"buildKey": "build-1", "isSuccessful": true}),
                );
            }
            if url.contains("/build-operations/build-1") {
                return crate::testutil::json_response(
                    200,
                    json!({"buildKey": "build-1", "status": "Finished"}),
                );
            }

            if url.contains("/deployment-operations") && req.method == "POST" {
                return crate::testutil::json_response(
                    201,
                    json!({"key": "op-1", "status": "Running"}),
                );
            }
            if url.contains("/deployment-operations/op-1") {
                return crate::testutil::json_response(
                    200,
                    json!({"key": "op-1", "status": "Finished"}),
                );
            }

            if url.contains("/publish-operations") && req.method == "POST" {
                return crate::testutil::json_response(
                    201,
                    json!({"key": "pub-1", "status": "Running"}),
                );
            }
            if url.contains("/publish-operations/pub-1") {
                return crate::testutil::json_response(
                    200,
                    json!({"key": "pub-1", "status": "Finished"}),
                );
            }

            if url.contains("/deployment-analyses") && req.method == "POST" {
                return crate::testutil::json_response(201, json!({"analysisKey": "analysis-1"}));
            }
            if url.contains("/deployment-analyses/analysis-1") {
                return crate::testutil::json_response(
                    200,
                    json!({"analysisKey": "analysis-1", "processStatus": "Finished", "report": {"status": "NoIssuesFound"}}),
                );
            }

            if url.contains("/deletion-analyses") && req.method == "POST" {
                return crate::testutil::json_response(
                    201,
                    json!({"analysisKey": "del-analysis-1"}),
                );
            }
            if url.contains("/deletion-analyses/del-analysis-1") {
                return crate::testutil::json_response(
                    200,
                    json!({"analysisKey": "del-analysis-1", "processStatus": "Finished", "report": {"status": "NoIssuesFound"}}),
                );
            }

            if url.contains("/application-roles/role-1")
                && (req.method == "POST" || req.method == "DELETE")
            {
                return Ok(crate::transport::HttpResponse {
                    status: 204,
                    headers: vec![],
                    body: Vec::new(),
                });
            }

            if url.contains("/application-roles") {
                return crate::testutil::json_response(
                    200,
                    json!({
                        "results": [{"key": "role-1", "name": "Admin", "assetKey": "app1"}],
                        "page": {"nextPageOffset": 0, "totalResults": 1},
                    }),
                );
            }

            if url.contains("/users/user-1") && req.method == "PATCH" {
                return crate::testutil::json_response(
                    200,
                    json!({"key": "user-1", "name": "New Name"}),
                );
            }

            if url.contains("/users/user-1") {
                return crate::testutil::json_response(
                    200,
                    json!({"key": "user-1", "name": "Demo", "email": "demo@example.com"}),
                );
            }

            if url.contains("/users?") {
                return crate::testutil::json_response(
                    200,
                    json!({
                        "results": [{"key": "user-1", "name": "Demo", "email": "demo@example.com"}],
                        "page": {"nextPageOffset": 0, "totalResults": 1},
                    }),
                );
            }

            crate::testutil::json_response(404, json!({"error": "not found"}))
        })
    }

    #[test]
    fn test_list_environments_parses_results_envelope() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let envs = client.list_environments().unwrap();

        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].get("name").unwrap(), "Development");
    }

    #[test]
    fn test_list_deployed_apps() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let apps = client.list_deployed_apps().unwrap();

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].get("key").unwrap(), "app1");
    }

    #[test]
    fn test_get_user_by_key() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let user = client.get_user("user-1").unwrap();

        assert_eq!(user.get("email").unwrap(), "demo@example.com");
    }

    #[test]
    fn test_find_user_by_email() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let user = client.find_user_by_email("demo@example.com").unwrap();

        assert_eq!(user.get("key").unwrap(), "user-1");
    }

    #[test]
    fn test_find_user_by_email_not_found() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let result = client.find_user_by_email("nope@example.com");

        assert!(result.is_err());
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
    fn test_get_producer_graph() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let producers = client
            .get_producer_graph("app1", 3, 0, "Deployable", "")
            .unwrap();

        assert_eq!(producers.len(), 1);
        assert_eq!(producers[0].get("name").unwrap(), "Producer One");
    }

    #[test]
    fn test_start_and_get_build() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let started = client.start_build("app1", 3, "Release").unwrap();
        assert_eq!(started.get("buildKey").unwrap(), "build-1");

        let status = client.get_build("build-1").unwrap();
        assert_eq!(status.get("status").unwrap(), "Finished");
    }

    #[test]
    fn test_start_and_get_deployment_operation() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let started = client
            .start_deployment_operation("Deploy", "app1", "env1", Some(3), Some("build-1"))
            .unwrap();
        assert_eq!(started.get("key").unwrap(), "op-1");

        let status = client.get_deployment_operation("op-1").unwrap();
        assert_eq!(status.get("status").unwrap(), "Finished");
    }

    #[test]
    fn test_start_and_get_publish() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let started = client.start_publish("app1", 3, "env1").unwrap();
        assert_eq!(started.get("key").unwrap(), "pub-1");

        let status = client.get_publish("pub-1").unwrap();
        assert_eq!(status.get("status").unwrap(), "Finished");
    }

    #[test]
    fn test_start_and_get_deployment_analysis() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let started = client.start_deployment_analysis("app1", 3, "env1").unwrap();
        assert_eq!(started.get("analysisKey").unwrap(), "analysis-1");

        let status = client.get_deployment_analysis("analysis-1").unwrap();
        assert_eq!(status.get("processStatus").unwrap(), "Finished");
    }

    #[test]
    fn test_start_and_get_deletion_analysis() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let started = client.start_deletion_analysis("app1").unwrap();
        assert_eq!(started.get("analysisKey").unwrap(), "del-analysis-1");

        let status = client.get_deletion_analysis("del-analysis-1").unwrap();
        assert_eq!(status.get("processStatus").unwrap(), "Finished");
    }

    #[test]
    fn test_delete_asset() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        assert!(client.delete_asset("app1").is_ok());
    }

    #[test]
    fn test_list_application_roles() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let roles = client.list_application_roles("Admin").unwrap();
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].get("name").unwrap(), "Admin");
    }

    #[test]
    fn test_grant_and_revoke_role() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        assert!(client.grant_role("user-1", "role-1").is_ok());
        assert!(client.revoke_role("user-1", "role-1").is_ok());
    }

    #[test]
    fn test_update_user() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let mut updates = Map::new();
        updates.insert("name".to_string(), json!("New Name"));
        let updated = client.update_user("user-1", &updates).unwrap();
        assert_eq!(updated.get("name").unwrap(), "New Name");
    }

    #[test]
    fn test_get_source_code_url() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let url = client.get_source_code_url("app1", 3).unwrap();
        assert_eq!(url, "https://storage.example.com/app1-rev-3.oml");
    }

    #[test]
    fn test_download_file_bytes() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let bytes = client
            .download_file_bytes("https://storage.example.com/app1-rev-3.oml")
            .unwrap();
        assert_eq!(bytes, b"<oml/>");
    }

    #[test]
    fn test_request_upload_url_and_upload_bytes() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let url = client.request_upload_url().unwrap();
        assert_eq!(url, "https://storage.example.com/upload-1");
        assert!(client.upload_file_bytes(&url, b"<oml/>".to_vec()).is_ok());
    }

    #[test]
    fn test_create_asset_revision() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let created = client
            .create_asset_revision("https://storage.example.com/upload-1")
            .unwrap();
        assert_eq!(created.get("revision").unwrap(), 4);
    }

    #[test]
    fn test_list_revisions_follows_pagination() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let revisions = client.list_revisions("app1").unwrap();

        assert_eq!(revisions.len(), 2);
        assert_eq!(revisions[0].get("revision").unwrap(), 1);
        assert_eq!(revisions[1].get("revision").unwrap(), 2);
    }

    #[test]
    fn test_list_revisions_page_returns_next_offset() {
        let client = Client::with_transport(test_settings(), test_output(), mock_transport());
        let (items, next_offset) = client.list_revisions_page("app1", 0, 1).unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(next_offset, Some(1));
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
