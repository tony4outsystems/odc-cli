use crate::cli::Options;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use serde_json::{Map, Value};
use std::sync::Arc;

/// The result of a listing command: either a single page (with pagination metadata) or
/// every page already combined.
struct Listing {
    items: Vec<Map<String, Value>>,
    /// (offset, limit, next_offset) when a single page was requested.
    page: Option<(i64, i64, Option<i64>)>,
}

/// Fetch a listing that supports offset-based pagination: a single page when
/// `options.offset` is set, or every page combined otherwise.
fn fetch_listing(
    options: &Options,
    page_fn: impl FnOnce(i64, i64) -> Result<(Vec<Map<String, Value>>, Option<i64>)>,
    all_fn: impl FnOnce() -> Result<Vec<Map<String, Value>>>,
) -> Result<Listing> {
    match options.offset {
        Some(offset) => {
            let (items, next_offset) = page_fn(offset, options.limit)?;
            Ok(Listing {
                items,
                page: Some((offset, options.limit, next_offset)),
            })
        }
        None => Ok(Listing {
            items: all_fn()?,
            page: None,
        }),
    }
}

/// Print a `Listing`: a plain JSON/table array when every page was fetched, or a
/// `{results, page}` envelope (in `--json` mode) carrying `page.nextOffset` when a single
/// page was requested, so the next page can be fetched with `--offset <nextOffset>`.
fn print_listing(output: &crate::output::Output, listing: Listing) -> Result<()> {
    let results: Vec<Value> = listing.items.into_iter().map(Value::Object).collect();

    match listing.page {
        Some((offset, limit, next_offset)) if output.json => {
            let mut page = Map::new();
            page.insert("offset".to_string(), Value::from(offset));
            page.insert("limit".to_string(), Value::from(limit));
            page.insert("nextOffset".to_string(), Value::from(next_offset));

            let mut body = Map::new();
            body.insert("results".to_string(), Value::Array(results));
            body.insert("page".to_string(), Value::Object(page));
            output.print_result(&Value::Object(body))
        }
        _ => output.print_result(&Value::Array(results)),
    }
}

/// Execute a command based on its name
pub async fn execute(cmd: &str, options: &Options, positionals: &[String]) -> Result<()> {
    match cmd {
        "login" => Err(anyhow::anyhow!(
            "Login should be handled in main run() function"
        )),
        "discover" => cmd_discover(options).await,
        "list-environments" => cmd_list_environments(options).await,
        "list-apps" => cmd_list_apps(options, positionals).await,
        "list-deployed-apps" => cmd_list_deployed_apps(options, positionals).await,
        "get-app" => cmd_get_app(options, positionals).await,
        "latest-revision" => cmd_latest_revision(options, positionals).await,
        "list-revisions" => cmd_list_revisions(options, positionals).await,
        "get-revision" => cmd_get_revision(options, positionals).await,
        "producer-graph" => Err(anyhow::anyhow!("producer-graph: not yet implemented")),
        "download-source-code" => Err(anyhow::anyhow!("download-source-code: not yet implemented")),
        "upload-source-code" => Err(anyhow::anyhow!("upload-source-code: not yet implemented")),
        "validate" => Err(anyhow::anyhow!("validate: not yet implemented")),
        "analyze-deployment" => Err(anyhow::anyhow!("analyze-deployment: not yet implemented")),
        "analyze-deletion" => Err(anyhow::anyhow!("analyze-deletion: not yet implemented")),
        "deploy" => Err(anyhow::anyhow!("deploy: not yet implemented")),
        "undeploy" => Err(anyhow::anyhow!("undeploy: not yet implemented")),
        "delete-app" => Err(anyhow::anyhow!("delete-app: not yet implemented")),
        "batch-deploy" => Err(anyhow::anyhow!("batch-deploy: not yet implemented")),
        "batch-undeploy" => Err(anyhow::anyhow!("batch-undeploy: not yet implemented")),
        "batch-delete" => Err(anyhow::anyhow!("batch-delete: not yet implemented")),
        "dangerous-batch-undeploy-all" => Err(anyhow::anyhow!(
            "dangerous-batch-undeploy-all: not yet implemented"
        )),
        "get-user" => Err(anyhow::anyhow!("get-user: not yet implemented")),
        "update-user" => Err(anyhow::anyhow!("update-user: not yet implemented")),
        "grant-role" => Err(anyhow::anyhow!("grant-role: not yet implemented")),
        "revoke-role" => Err(anyhow::anyhow!("revoke-role: not yet implemented")),
        "internal-build" => Err(anyhow::anyhow!("internal-build: not yet implemented")),
        "internal-publish" => Err(anyhow::anyhow!("internal-publish: not yet implemented")),
        "internal-deploy" => Err(anyhow::anyhow!("internal-deploy: not yet implemented")),
        _ => Err(anyhow::anyhow!("Unknown command: {}", cmd)),
    }
}

async fn cmd_discover(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let discovery = client.discover()?;
    output.print_result(&serde_json::Value::Object(discovery))?;
    Ok(())
}

async fn cmd_list_environments(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let envs = client.list_environments()?;
    let result: Vec<_> = envs.into_iter().map(serde_json::Value::Object).collect();
    output.print_result(&serde_json::Value::Array(result))?;
    Ok(())
}

async fn cmd_list_apps(options: &Options, _positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let listing = fetch_listing(
        options,
        |offset, limit| client.list_apps_page(offset, limit),
        || client.list_apps(),
    )?;
    print_listing(&output, listing)
}

async fn cmd_list_deployed_apps(options: &Options, _positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let listing = fetch_listing(
        options,
        |offset, limit| client.list_apps_page(offset, limit),
        || client.list_apps(),
    )?;
    print_listing(&output, listing)
}

async fn cmd_get_app(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("get-app requires an app name or key"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];

    for app in apps {
        if let Some(serde_json::Value::String(key)) = app.get("key") {
            if key == app_key {
                output.print_result(&serde_json::Value::Object(app))?;
                return Ok(());
            }
        }
    }

    Err(anyhow::anyhow!("App not found: {}", app_key))
}

async fn cmd_latest_revision(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "latest-revision requires an app name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];

    for app in apps {
        if let Some(serde_json::Value::String(key)) = app.get("key") {
            if key == app_key {
                if let Some(revision) = app.get("revision") {
                    output.print_result(revision)?;
                    return Ok(());
                }
            }
        }
    }

    Err(anyhow::anyhow!("App not found: {}", app_key))
}

async fn cmd_list_revisions(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "list-revisions requires an app name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];

    let asset_key = apps
        .iter()
        .find_map(|app| match app.get("key") {
            Some(serde_json::Value::String(key)) if key == app_key => Some(key.clone()),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("App not found: {}", app_key))?;

    let listing = fetch_listing(
        options,
        |offset, limit| client.list_revisions_page(&asset_key, offset, limit),
        || client.list_revisions(&asset_key),
    )?;
    print_listing(&output, listing)
}

async fn cmd_get_revision(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.len() < 2 {
        return Err(anyhow::anyhow!(
            "get-revision requires <app> --revision <number>"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];

    for app in apps {
        if let Some(serde_json::Value::String(key)) = app.get("key") {
            if key == app_key {
                output.print_result(&serde_json::Value::Object(app))?;
                return Ok(());
            }
        }
    }

    Err(anyhow::anyhow!("App not found: {}", app_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unknown_command() {
        let opts = Options::default();
        let result = execute("nonexistent", &opts, &[]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }
}
