//! Client for the Mentor MCP server.
//!
//! Mentor is exposed as an MCP server (JSON-RPC 2.0 over the "Streamable HTTP" transport) at
//! `<tenant origin>/mcp`, authenticated with its own OAuth2 client-credentials grant — a
//! separate client id/secret and token endpoint from the main ODC REST API client in
//! `client.rs`, configured via `odc login-mentor`. Each Mentor capability (start a session,
//! send a prompt, poll a run, ...) is one `tools/call` request naming a `mentor_*` tool.

use crate::settings::Settings;
use crate::transport::{HttpRequest, HttpResponse, Transport};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use url::Url;

pub struct MentorClient {
    settings: Settings,
    transport: Box<dyn Transport>,
    token: Mutex<Option<String>>,
    /// The `Mcp-Session-Id` the server assigned during `initialize`, echoed back on every
    /// subsequent request in the same CLI invocation.
    mcp_session_id: Mutex<Option<String>>,
    initialized: Mutex<bool>,
    next_id: AtomicI64,
}

impl MentorClient {
    pub fn new(settings: Settings) -> Self {
        use crate::transport::ReqwestTransport;
        Self::with_transport(settings, ReqwestTransport::new())
    }

    pub fn with_transport<T>(settings: Settings, transport: T) -> Self
    where
        T: Transport + 'static,
    {
        Self {
            settings,
            transport: Box::new(transport),
            token: Mutex::new(None),
            mcp_session_id: Mutex::new(None),
            initialized: Mutex::new(false),
            next_id: AtomicI64::new(1),
        }
    }

    fn require_credentials(&self) -> anyhow::Result<(&str, &str, &str)> {
        let token_url = self.settings.mentor_token_url.as_deref().unwrap_or("");
        let client_id = self.settings.mentor_client_id.as_deref().unwrap_or("");
        let client_secret = self.settings.mentor_client_secret.as_deref().unwrap_or("");

        if token_url.is_empty() || client_id.is_empty() || client_secret.is_empty() {
            return Err(anyhow::anyhow!(
                "Mentor is not configured; run `odc login-mentor <token-url> <client-id>` first"
            ));
        }

        Ok((token_url, client_id, client_secret))
    }

    /// Get (and cache) an access token for the Mentor OAuth2 client.
    fn token(&self) -> anyhow::Result<String> {
        {
            let token = self.token.lock().unwrap();
            if let Some(token) = &*token {
                return Ok(token.clone());
            }
        }

        let (token_url, client_id, client_secret) = self.require_credentials()?;

        let body = format!(
            "grant_type=client_credentials&client_id={}&client_secret={}",
            client_id, client_secret
        );

        let resp = self.send_raw(
            "POST",
            token_url,
            vec![(
                "Content-Type".to_string(),
                "application/x-www-form-urlencoded".to_string(),
            )],
            Some(body.into_bytes()),
        )?;

        if resp.status >= 400 {
            let body_str = String::from_utf8(resp.body).unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Mentor token request failed with {}: {}",
                resp.status,
                body_str
            ));
        }

        let body_str = String::from_utf8(resp.body)?;
        let token_resp: Value = serde_json::from_str(&body_str)?;
        let access_token = crate::value::require_string(
            token_resp.get("access_token").unwrap_or(&Value::Null),
            "access_token",
        )?;

        *self.token.lock().unwrap() = Some(access_token.clone());
        Ok(access_token)
    }

    fn send_raw(
        &self,
        method: &str,
        url: &str,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
    ) -> anyhow::Result<HttpResponse> {
        let req = HttpRequest {
            method: method.to_string(),
            url: Url::parse(url)?,
            headers,
            body: body.unwrap_or_default(),
        };
        self.transport.send(req)
    }

    /// Send a single JSON-RPC request (or notification, when `id` is `None`) to the Mentor
    /// MCP endpoint and return the parsed response body, handling both a plain JSON response
    /// and a `text/event-stream` one.
    fn rpc(&self, method: &str, params: Value, id: Option<i64>) -> anyhow::Result<Option<Value>> {
        let token = self.token()?;

        let mut payload = serde_json::Map::new();
        payload.insert("jsonrpc".to_string(), json!("2.0"));
        payload.insert("method".to_string(), json!(method));
        payload.insert("params".to_string(), params);
        if let Some(id) = id {
            payload.insert("id".to_string(), json!(id));
        }

        let mut headers = vec![
            ("Authorization".to_string(), format!("Bearer {}", token)),
            ("Content-Type".to_string(), "application/json".to_string()),
            (
                "Accept".to_string(),
                "application/json, text/event-stream".to_string(),
            ),
        ];
        if let Some(session_id) = self.mcp_session_id.lock().unwrap().clone() {
            headers.push(("Mcp-Session-Id".to_string(), session_id));
        }

        let resp = self.send_raw(
            "POST",
            &self.settings.mentor_url(),
            headers,
            Some(serde_json::to_vec(&Value::Object(payload))?),
        )?;

        if let Some(session_id) = find_header(&resp.headers, "mcp-session-id") {
            *self.mcp_session_id.lock().unwrap() = Some(session_id);
        }

        if resp.status >= 400 {
            let body_str = String::from_utf8(resp.body).unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Mentor request ({}) failed with {}: {}",
                method,
                resp.status,
                body_str
            ));
        }

        // A notification (no id) has no response body to parse.
        if id.is_none() {
            return Ok(None);
        }

        if resp.body.is_empty() {
            return Ok(None);
        }

        let body_str = String::from_utf8(resp.body)?;
        let content_type = find_header(&resp.headers, "content-type").unwrap_or_default();
        let message = if content_type.contains("text/event-stream") {
            parse_sse_last_json(&body_str)?
        } else {
            serde_json::from_str(&body_str)?
        };

        if let Some(error) = message.get("error") {
            return Err(anyhow::anyhow!(
                "Mentor request ({}) failed: {}",
                method,
                error
            ));
        }

        Ok(message.get("result").cloned())
    }

    /// Perform the MCP handshake once per client instance: `initialize`, then the
    /// `notifications/initialized` notification.
    fn ensure_initialized(&self) -> anyhow::Result<()> {
        {
            let initialized = self.initialized.lock().unwrap();
            if *initialized {
                return Ok(());
            }
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.rpc(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "odc-cli", "version": env!("CARGO_PKG_VERSION")},
            }),
            Some(id),
        )?;

        // Best-effort: some servers require this notification before accepting tool calls.
        let _ = self.rpc("notifications/initialized", json!({}), None);

        *self.initialized.lock().unwrap() = true;
        Ok(())
    }

    /// Call a `mentor_*` tool and return its result.
    ///
    /// On success, returns the tool's `structuredContent` when present, otherwise the raw
    /// `content` array from the MCP `tools/call` result.
    pub fn call_tool(&self, name: &str, arguments: Value) -> anyhow::Result<Value> {
        self.ensure_initialized()?;

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let result = self
            .rpc(
                "tools/call",
                json!({"name": name, "arguments": arguments}),
                Some(id),
            )?
            .ok_or_else(|| anyhow::anyhow!("Mentor tool {} returned no result", name))?;

        if result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return Err(anyhow::anyhow!(
                "Mentor tool {} failed: {}",
                name,
                extract_text(&result)
            ));
        }

        if let Some(structured) = result.get("structuredContent") {
            return Ok(structured.clone());
        }

        Ok(result.get("content").cloned().unwrap_or(result))
    }
}

fn find_header(headers: &[(String, String)], name: &str) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.clone())
}

/// Extract the concatenated text of an MCP `content` array, for error messages.
fn extract_text(result: &Value) -> String {
    result
        .get("content")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| result.to_string())
}

/// Parse a `text/event-stream` body for its JSON-RPC message: the last `data:` line that
/// parses as JSON (a single request's stream carries exactly one JSON-RPC response).
fn parse_sse_last_json(body: &str) -> anyhow::Result<Value> {
    let mut last = None;
    for line in body.lines() {
        if let Some(data) = line.strip_prefix("data:") {
            if let Ok(value) = serde_json::from_str::<Value>(data.trim()) {
                last = Some(value);
            }
        }
    }
    last.ok_or_else(|| anyhow::anyhow!("No JSON-RPC message found in event stream"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    fn test_settings() -> Settings {
        Settings {
            tenant_url: "https://example.com".to_string(),
            mentor_token_url: Some("https://example.com/auth/token".to_string()),
            mentor_client_id: Some("mentor-client".to_string()),
            mentor_client_secret: Some("mentor-secret".to_string()),
            ..Default::default()
        }
    }

    fn mock_transport(
        handler: impl Fn(&HttpRequest, &Value) -> Option<HttpResponse> + Send + Sync + 'static,
    ) -> impl Transport {
        crate::testutil::test_transport(move |req: HttpRequest| {
            let url = req.url.as_str();

            if url.contains("/auth/token") {
                return crate::testutil::json_response(
                    200,
                    json!({"access_token": "mentor-token"}),
                );
            }

            let parsed: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let method = parsed.get("method").and_then(|v| v.as_str()).unwrap_or("");

            if method == "initialize" {
                let mut resp = crate::testutil::json_response(
                    200,
                    json!({"jsonrpc": "2.0", "id": parsed.get("id"), "result": {}}),
                )?;
                resp.headers
                    .push(("Mcp-Session-Id".to_string(), "session-abc".to_string()));
                return Ok(resp);
            }

            if method == "notifications/initialized" {
                return Ok(HttpResponse {
                    status: 202,
                    headers: vec![],
                    body: Vec::new(),
                });
            }

            if let Some(resp) = handler(&req, &parsed) {
                return Ok(resp);
            }

            crate::testutil::json_response(404, json!({"error": "unhandled"}))
        })
    }

    #[test]
    fn test_call_tool_returns_structured_content() {
        let transport = mock_transport(|_req, parsed| {
            if parsed.get("method").and_then(|v| v.as_str()) == Some("tools/call") {
                return Some(
                    crate::testutil::json_response(
                        200,
                        json!({
                            "jsonrpc": "2.0",
                            "id": parsed.get("id"),
                            "result": {
                                "isError": false,
                                "content": [{"type": "text", "text": "{\"sessionId\":\"s1\"}"}],
                                "structuredContent": {"sessionId": "s1"},
                            },
                        }),
                    )
                    .unwrap(),
                );
            }
            None
        });

        let client = MentorClient::with_transport(test_settings(), transport);
        let result = client
            .call_tool("mentor_start_session", Value::Object(Map::new()))
            .unwrap();

        assert_eq!(result.get("sessionId").unwrap(), "s1");
    }

    #[test]
    fn test_call_tool_error_result_is_err() {
        let transport = mock_transport(|_req, parsed| {
            if parsed.get("method").and_then(|v| v.as_str()) == Some("tools/call") {
                return Some(
                    crate::testutil::json_response(
                        200,
                        json!({
                            "jsonrpc": "2.0",
                            "id": parsed.get("id"),
                            "result": {
                                "isError": true,
                                "content": [{"type": "text", "text": "session not found"}],
                            },
                        }),
                    )
                    .unwrap(),
                );
            }
            None
        });

        let client = MentorClient::with_transport(test_settings(), transport);
        let err = client
            .call_tool(
                "mentor_prompt",
                json!({"sessionId": "bogus", "message": "hi"}),
            )
            .unwrap_err();

        assert!(err.to_string().contains("session not found"));
    }

    #[test]
    fn test_missing_credentials_error() {
        let settings = Settings {
            tenant_url: "https://example.com".to_string(),
            ..Default::default()
        };
        let client = MentorClient::with_transport(
            settings,
            crate::testutil::test_transport(|_req| crate::testutil::json_response(200, json!({}))),
        );

        let err = client
            .call_tool("mentor_start_session", Value::Object(Map::new()))
            .unwrap_err();
        assert!(err.to_string().contains("login-mentor"));
    }

    #[test]
    fn test_sse_response_is_parsed() {
        let transport = mock_transport(|_req, parsed| {
            if parsed.get("method").and_then(|v| v.as_str()) == Some("tools/call") {
                let body = format!(
                    "event: message\ndata: {}\n\n",
                    json!({
                        "jsonrpc": "2.0",
                        "id": parsed.get("id"),
                        "result": {"isError": false, "content": [], "structuredContent": {"ok": true}},
                    })
                );
                return Some(HttpResponse {
                    status: 200,
                    headers: vec![("Content-Type".to_string(), "text/event-stream".to_string())],
                    body: body.into_bytes(),
                });
            }
            None
        });

        let client = MentorClient::with_transport(test_settings(), transport);
        let result = client
            .call_tool("mentor_publish", json!({"sessionId": "s1"}))
            .unwrap();
        assert_eq!(result.get("ok").unwrap(), true);
    }
}
