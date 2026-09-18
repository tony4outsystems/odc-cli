//! Revision management and producer graph commands.

use super::shared::*;
use crate::cli::Options;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

pub async fn cmd_latest_revision(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "latest-revision requires an asset name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let app_key = &positionals[0];

    let asset = resolve_asset(&client, app_key)?;
    let revision = asset
        .get("revision")
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no revision field", app_key))?;
    output.print_result(revision)?;
    Ok(())
}

pub async fn cmd_list_revisions(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "list-revisions requires an asset name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let app_key = &positionals[0];

    let asset_key = resolve_asset(&client, app_key)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", app_key))?
        .to_string();

    let listing = fetch_listing(
        options,
        |offset, limit| client.list_revisions_page(&asset_key, offset, limit),
        || crate::inspection::list_revisions(&client, app_key),
    )?;
    print_listing(&output, listing, REVISION_TABLE_COLUMNS)
}

pub async fn cmd_get_revision(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "get-revision requires an asset name or key"
        ));
    }
    let revision = options
        .revision
        .ok_or_else(|| anyhow::anyhow!("get-revision requires --revision <number>"))?;

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let app_key = &positionals[0];

    let found = crate::inspection::get_revision(&client, app_key, revision)?;

    output.print_result(&serde_json::Value::Object(found))?;
    Ok(())
}

pub async fn cmd_producer_graph(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "producer-graph requires an asset name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let app_key = &positionals[0];
    let asset = resolve_asset(&client, app_key)?;

    let asset_key = asset
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", app_key))?
        .to_string();

    let revision = match options.revision {
        Some(revision) => revision,
        None => asset
            .get("revision")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("Asset {} has no revision field", app_key))?
            as i32,
    };

    let env_key = if options.env.is_empty() {
        String::new()
    } else {
        let environments = client.list_environments()?;
        crate::resolve::resolve(&options.env, "environment", &environments, "key")?
    };

    let producer_type_filter = if options.all_producers {
        "All"
    } else {
        options.filter.as_str()
    };

    let producers = client.get_producer_graph(
        &asset_key,
        revision,
        options.max_depth,
        producer_type_filter,
        &env_key,
    )?;

    let mut root = asset.clone();
    root.insert("revision".to_string(), Value::from(revision));

    let graph = crate::mermaid::render_producer_graph(&root, &producers);

    let output_path = if options.output.is_empty() {
        crate::mermaid::default_mermaid_path(&asset_key, revision as i64)
    } else {
        options.output.clone()
    };
    std::fs::write(&output_path, &graph)?;

    if options.json {
        output.print_result(&serde_json::json!({"output": output_path}))?;
    } else {
        output.println_locked(&format!("Wrote producer graph to {}", output_path));
    }
    Ok(())
}
