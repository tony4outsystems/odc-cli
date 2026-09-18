//! Revision management and producer graph commands.

use super::args::*;
use super::shared::*;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

pub async fn cmd_latest_revision(args: LatestRevisionArgs, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "latest-revision requires an asset name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let app_key = &positionals[0];

    let asset = resolve_asset(&client, app_key)?;
    let revision = asset
        .get("revision")
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no revision field", app_key))?;
    output.print_result(revision)?;
    Ok(())
}

pub async fn cmd_list_revisions(args: ListRevisionsArgs, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "list-revisions requires an asset name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let app_key = &positionals[0];

    let asset_key = resolve_asset(&client, app_key)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", app_key))?
        .to_string();

    let offset = args.offset.unwrap_or(0);
    let (items, has_more) = client.list_revisions_page(&asset_key, offset, args.limit)?;

    let result = Listing {
        items,
        page: Some((offset, args.limit, has_more)),
    };

    print_listing(&output, result, REVISION_TABLE_COLUMNS)
}

pub async fn cmd_get_revision(args: GetRevisionArgs, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "get-revision requires an asset name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let app_key = &positionals[0];

    let found = crate::inspection::get_revision(&client, app_key, args.revision)?;

    output.print_result(&serde_json::Value::Object(found))?;
    Ok(())
}

pub async fn cmd_producer_graph(args: ProducerGraphArgs, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "producer-graph requires an asset name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let app_key = &positionals[0];
    let asset = resolve_asset(&client, app_key)?;

    let asset_key = asset
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", app_key))?
        .to_string();

    let revision = match args.revision {
        Some(revision) => revision,
        None => asset
            .get("revision")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("Asset {} has no revision field", app_key))?
            as i32,
    };

    let env_key = if args.env.is_empty() {
        String::new()
    } else {
        let environments = client.list_environments()?;
        crate::resolve::resolve(&args.env, "environment", &environments, "key")?
    };

    let producer_type_filter = if args.all_producers {
        "All"
    } else {
        args.filter.as_str()
    };

    let producers = client.get_producer_graph(
        &asset_key,
        revision,
        args.max_depth,
        producer_type_filter,
        &env_key,
    )?;

    let mut root = asset.clone();
    root.insert("revision".to_string(), Value::from(revision));

    let graph = crate::mermaid::render_producer_graph(&root, &producers);

    let output_path = if args.output.is_empty() {
        crate::mermaid::default_mermaid_path(&asset_key, revision as i64)
    } else {
        args.output.clone()
    };
    std::fs::write(&output_path, &graph)?;

    if args.json {
        output.print_result(&serde_json::json!({"output": output_path}))?;
    } else {
        output.println_locked(&format!("Wrote producer graph to {}", output_path));
    }
    Ok(())
}
