use crate::value::compact_map;
use anyhow::Result;
use serde_json::{Map, Value};

/// List all revisions of an app
pub fn list_revisions(_key: &str) -> Result<Vec<Map<String, Value>>> {
    // TODO: Implement via client.paginated("asset-repository", ...)
    Err(anyhow::anyhow!("Not yet implemented"))
}

/// Get a specific revision
pub fn get_revision(_key: &str, _revision: i32) -> Result<Map<String, Value>> {
    // TODO: Implement via client.call
    Err(anyhow::anyhow!("Not yet implemented"))
}

/// Get revision source code metadata
pub fn get_revision_source_code(_key: &str, _revision: i32) -> Result<Map<String, Value>> {
    // TODO: Implement via client.call
    Err(anyhow::anyhow!("Not yet implemented"))
}

/// Download source code from a presigned URL
pub async fn download_source_code(_key: &str, _revision: i32, _output: &str) -> Result<u64> {
    // TODO: Implement via HTTP GET to presigned URL
    Err(anyhow::anyhow!("Not yet implemented"))
}

/// Build deployed app rows from the API response, filtering by environment and search
pub fn deployed_app_rows(
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
    use serde_json::json;

    #[test]
    fn test_deployed_app_rows_filtering() {
        let mut app = serde_json::Map::new();
        app.insert("key".to_string(), json!("app1"));
        app.insert("type".to_string(), json!("WebApplication"));

        let mut deployment = serde_json::Map::new();
        deployment.insert("name".to_string(), json!("prod-deployment"));
        deployment.insert("environmentKey".to_string(), json!("prod"));
        deployment.insert("revision".to_string(), json!(5));

        let mut app_with_deploy = app.clone();
        app_with_deploy.insert("deployments".to_string(), json!(vec![deployment.clone()]));

        let rows = deployed_app_rows(&[app_with_deploy], "prod", "");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("key").and_then(|v| v.as_str()), Some("app1"));
    }

    #[test]
    fn test_deployed_app_rows_env_filter() {
        let mut app = serde_json::Map::new();
        app.insert("key".to_string(), json!("app1"));
        app.insert("type".to_string(), json!("WebApplication"));

        let mut deployment = serde_json::Map::new();
        deployment.insert("name".to_string(), json!("prod-deployment"));
        deployment.insert("environmentKey".to_string(), json!("staging"));

        let mut app_with_deploy = app.clone();
        app_with_deploy.insert("deployments".to_string(), json!(vec![deployment]));

        let rows = deployed_app_rows(&[app_with_deploy], "prod", "");
        assert_eq!(rows.len(), 0); // Filtered out by environment
    }

    #[test]
    fn test_deployed_app_rows_search_filter() {
        let mut app = serde_json::Map::new();
        app.insert("key".to_string(), json!("app1"));
        app.insert("type".to_string(), json!("WebApplication"));

        let mut deployment = serde_json::Map::new();
        deployment.insert("name".to_string(), json!("prod-deployment"));
        deployment.insert("environmentKey".to_string(), json!("prod"));

        let mut app_with_deploy = app.clone();
        app_with_deploy.insert("deployments".to_string(), json!(vec![deployment]));

        let rows = deployed_app_rows(&[app_with_deploy.clone()], "", "nomatch");
        assert_eq!(rows.len(), 0); // Filtered out by search

        let rows = deployed_app_rows(&[app_with_deploy], "", "prod");
        assert_eq!(rows.len(), 1); // Matches environment name
    }
}
