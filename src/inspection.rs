//! Result formatting and inspection utilities.
//!
//! This module provides helpers for transforming and formatting API responses for display:
//! - Converting raw API objects into display rows
//! - Filtering and selecting fields for table output
//! - Enriching sparse data (e.g., resolving environment keys to names)

use crate::client::Client;
use crate::commands::shared::resolve_asset;
use crate::value::compact_map;
use anyhow::Result;
use serde_json::{Map, Value};

/// Resolve `app_identifier` (name, key, or unambiguous substring) to its `assetKey`.
fn asset_key_of(client: &Client, app_identifier: &str) -> Result<String> {
    resolve_asset(client, app_identifier)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", app_identifier))
}

/// List all revisions of an app, following pagination until exhausted.
pub fn list_revisions(client: &Client, app_identifier: &str) -> Result<Vec<Map<String, Value>>> {
    let asset_key = asset_key_of(client, app_identifier)?;
    client.list_revisions(&asset_key)
}

/// Get a specific revision of an app.
pub fn get_revision(
    client: &Client,
    app_identifier: &str,
    revision: i32,
) -> Result<Map<String, Value>> {
    let asset_key = asset_key_of(client, app_identifier)?;
    client
        .list_revisions(&asset_key)?
        .into_iter()
        .find(|r| r.get("revision").and_then(|v| v.as_i64()) == Some(revision as i64))
        .ok_or_else(|| {
            anyhow::anyhow!("Revision {} not found for app {}", revision, app_identifier)
        })
}

/// Default output path for a downloaded revision's source code.
fn default_source_code_path(asset_key: &str, revision: i32) -> String {
    format!(
        "{}-rev-{}.oml",
        crate::mermaid::safe_file_token(asset_key),
        revision
    )
}

/// Download an app revision's OML/XIF source code to `output` (or a generated default path
/// when empty), returning the number of bytes written.
pub fn download_source_code(
    client: &Client,
    app_identifier: &str,
    revision: i32,
    output: &str,
) -> Result<(String, u64)> {
    let asset_key = asset_key_of(client, app_identifier)?;
    let url = client.get_source_code_url(&asset_key, revision)?;
    let bytes = client.download_file_bytes(&url)?;

    let output_path = if output.is_empty() {
        default_source_code_path(&asset_key, revision)
    } else {
        output.to_string()
    };
    std::fs::write(&output_path, &bytes)?;

    Ok((output_path, bytes.len() as u64))
}

/// Build deployed asset rows from the API response, filtering by environment and search
pub fn deployed_asset_rows(
    items: &[Map<String, Value>],
    env: &str,
    search: &str,
) -> Vec<Map<String, Value>> {
    let mut rows = Vec::new();

    for app in items {
        if let Some(Value::Array(deployments)) = app.get("deployments") {
            for deployment in deployments {
                if let Value::Object(dep_map) = deployment {
                    // Filter by environment
                    if !env.is_empty() {
                        if let Some(Value::String(dep_env)) = dep_map.get("environmentKey") {
                            if dep_env != env {
                                continue;
                            }
                        }
                    }

                    // Filter by search
                    if !search.is_empty() {
                        let app_key = crate::value::str(app.get("key").unwrap_or(&Value::Null));
                        let dep_name =
                            crate::value::str(dep_map.get("name").unwrap_or(&Value::Null));

                        if !app_key.to_lowercase().contains(&search.to_lowercase())
                            && !dep_name.to_lowercase().contains(&search.to_lowercase())
                        {
                            continue;
                        }
                    }

                    // Build row
                    let row = compact_map(
                        dep_map,
                        &[
                            "name",
                            "revision",
                            "tag",
                            "url",
                            "environmentKey",
                            "deploymentDateTime",
                        ],
                    );

                    let mut final_row = row;
                    final_row.insert(
                        "key".to_string(),
                        app.get("key").unwrap_or(&Value::Null).clone(),
                    );
                    final_row.insert(
                        "type".to_string(),
                        app.get("type").unwrap_or(&Value::Null).clone(),
                    );

                    rows.push(final_row);
                }
            }
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{ColorMode, Output};
    use crate::settings::Settings;
    use serde_json::json;

    fn test_settings() -> Settings {
        Settings {
            tenant_url: "https://example.com".to_string(),
            client_id: "test-id".to_string(),
            client_secret: "test-secret".to_string(),
        }
    }

    fn test_output() -> std::sync::Arc<Output> {
        std::sync::Arc::new(Output::new(false, ColorMode::Never))
    }

    fn mock_client() -> Client {
        let transport = crate::testutil::test_transport(|req: crate::transport::HttpRequest| {
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
            if url.contains("/source-code") {
                return crate::testutil::json_response(
                    200,
                    json!({"sourceCodeBinaryUrl": "https://storage.example.com/app1-rev-3.oml"}),
                );
            }
            if url.contains("storage.example.com") {
                return Ok(crate::transport::HttpResponse {
                    status: 200,
                    headers: vec![],
                    body: b"<oml/>".to_vec(),
                });
            }
            if url.contains("/assets?nameContains=") {
                return crate::testutil::json_response(
                    200,
                    json!({
                        "results": [{"assetKey": "app1", "name": "App One"}],
                        "page": {"nextPageOffset": 0, "totalResults": 1},
                    }),
                );
            }
            if url.contains("/revisions") {
                return crate::testutil::json_response(
                    200,
                    json!({
                        "results": [{"revision": 1}, {"revision": 3}],
                        "page": {"nextPageOffset": 0, "totalResults": 2},
                    }),
                );
            }

            crate::testutil::json_response(404, json!({"error": "not found"}))
        });
        Client::with_transport(test_settings(), test_output(), transport)
    }

    #[test]
    fn test_list_revisions_resolves_app_and_fetches() {
        let client = mock_client();
        let revisions = list_revisions(&client, "App One").unwrap();
        assert_eq!(revisions.len(), 2);
    }

    #[test]
    fn test_get_revision_finds_matching_revision() {
        let client = mock_client();
        let revision = get_revision(&client, "App One", 3).unwrap();
        assert_eq!(revision.get("revision").unwrap(), 3);
    }

    #[test]
    fn test_get_revision_not_found() {
        let client = mock_client();
        let result = get_revision(&client, "App One", 99);
        assert!(result.is_err());
    }

    #[test]
    fn test_download_source_code_writes_file() {
        let client = mock_client();
        let dir = tempfile::tempdir().unwrap();
        let output_path = dir.path().join("out.oml");

        let (path, bytes) =
            download_source_code(&client, "App One", 3, output_path.to_str().unwrap()).unwrap();

        assert_eq!(path, output_path.to_str().unwrap());
        assert_eq!(bytes, 6);
        assert_eq!(std::fs::read(&output_path).unwrap(), b"<oml/>");
    }

    #[test]
    fn test_default_source_code_path() {
        assert_eq!(default_source_code_path("app1", 3), "app1-rev-3.oml");
    }

    #[test]
    fn test_deployed_asset_rows_filtering() {
        let mut app = serde_json::Map::new();
        app.insert("key".to_string(), json!("app1"));
        app.insert("type".to_string(), json!("WebApplication"));

        let mut deployment = serde_json::Map::new();
        deployment.insert("name".to_string(), json!("prod-deployment"));
        deployment.insert("environmentKey".to_string(), json!("prod"));
        deployment.insert("revision".to_string(), json!(5));

        let mut app_with_deploy = app.clone();
        app_with_deploy.insert("deployments".to_string(), json!(vec![deployment.clone()]));

        let rows = deployed_asset_rows(&[app_with_deploy], "prod", "");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("key").and_then(|v| v.as_str()), Some("app1"));
    }

    #[test]
    fn test_deployed_asset_rows_env_filter() {
        let mut app = serde_json::Map::new();
        app.insert("key".to_string(), json!("app1"));
        app.insert("type".to_string(), json!("WebApplication"));

        let mut deployment = serde_json::Map::new();
        deployment.insert("name".to_string(), json!("prod-deployment"));
        deployment.insert("environmentKey".to_string(), json!("staging"));

        let mut app_with_deploy = app.clone();
        app_with_deploy.insert("deployments".to_string(), json!(vec![deployment]));

        let rows = deployed_asset_rows(&[app_with_deploy], "prod", "");
        assert_eq!(rows.len(), 0); // Filtered out by environment
    }

    #[test]
    fn test_deployed_asset_rows_search_filter() {
        let mut app = serde_json::Map::new();
        app.insert("key".to_string(), json!("app1"));
        app.insert("type".to_string(), json!("WebApplication"));

        let mut deployment = serde_json::Map::new();
        deployment.insert("name".to_string(), json!("prod-deployment"));
        deployment.insert("environmentKey".to_string(), json!("prod"));

        let mut app_with_deploy = app.clone();
        app_with_deploy.insert("deployments".to_string(), json!(vec![deployment]));

        let rows = deployed_asset_rows(&[app_with_deploy.clone()], "", "nomatch");
        assert_eq!(rows.len(), 0); // Filtered out by search

        let rows = deployed_asset_rows(&[app_with_deploy], "", "prod");
        assert_eq!(rows.len(), 1); // Matches environment name
    }
}
