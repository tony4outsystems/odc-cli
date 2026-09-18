//! Asset listing and detail commands.

use super::shared::*;
use crate::cli::Options;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

pub async fn cmd_list_assets(options: &Options, positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let mut listing = fetch_listing(
        options,
        |offset, limit| client.list_assets_page(offset, limit),
        || client.list_assets(),
    )?;
    listing.items = filter_by_substring(
        listing.items,
        positionals.first().map(String::as_str),
        &["name", "assetKey"],
    );
    if !options.asset_type.is_empty() {
        listing.items.retain(|asset| {
            asset.get("assetType").and_then(|v| v.as_str()) == Some(options.asset_type.as_str())
        });
    }
    print_listing(&output, listing, ASSET_TABLE_COLUMNS)
}

pub async fn cmd_list_deployed_assets(options: &Options, positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let environments = client.list_environments()?;

    // Resolve --env (name or key) up front so a typo fails loudly instead of silently
    // matching nothing.
    let env_key = if options.env.is_empty() {
        String::new()
    } else {
        crate::resolve::resolve(&options.env, "environment", &environments, "key")?
    };
    let env_names: std::collections::HashMap<&str, &str> = environments
        .iter()
        .filter_map(|e| Some((e.get("key")?.as_str()?, e.get("name")?.as_str()?)))
        .collect();

    let mut listing = fetch_listing(
        options,
        |offset, limit| client.list_deployed_assets_page(offset, limit),
        || client.list_deployed_assets(),
    )?;

    let search = positionals.first().map(String::as_str).unwrap_or("");
    let mut rows = crate::inspection::deployed_asset_rows(&listing.items, &env_key, search);
    for row in &mut rows {
        if let Some(Value::String(key)) = row.get("environmentKey").cloned() {
            let name = env_names.get(key.as_str()).copied().unwrap_or(&key);
            row.insert("environment".to_string(), Value::String(name.to_string()));
        }
    }
    listing.items = rows;

    print_listing(&output, listing, DEPLOYED_ASSET_TABLE_COLUMNS)
}

pub async fn cmd_get_asset(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("get-asset requires an asset name or key"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let asset_key = &positionals[0];

    let asset = resolve_asset(&client, asset_key)?;
    output.print_result(&serde_json::Value::Object(asset))?;
    Ok(())
}
