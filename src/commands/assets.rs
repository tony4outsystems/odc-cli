//! Asset listing and detail commands.

use super::args::{GetAssetArgs, ListAssetsArgs, ListDeployedAssetsArgs};
use super::shared::*;
use anyhow::Result;
use serde_json::Value;

/// List assets with optional filtering by name/key and/or asset type.
pub async fn cmd_list_assets(args: ListAssetsArgs, positionals: &[String]) -> Result<()> {
    let (output, client) = super::shared::make_client(args.json, args.color)?;

    // Create a minimal Options struct just for fetch_listing compatibility
    let options = crate::cli::Options {
        json: args.json,
        color: args.color,
        offset: args.offset,
        limit: args.limit,
        ..Default::default()
    };

    let mut listing = fetch_listing(
        &options,
        |offset, limit| client.list_assets_page(offset, limit),
        || client.list_assets(),
    )?;

    listing.items = filter_by_substring(
        listing.items,
        args.filter
            .as_deref()
            .or_else(|| positionals.first().map(String::as_str)),
        &["name", "assetKey"],
    );

    if let Some(asset_type) = args.asset_type {
        let type_str = asset_type.as_str();
        listing
            .items
            .retain(|item| item.get("assetType").and_then(|v| v.as_str()) == Some(type_str));
    }

    print_listing(&output, listing, ASSET_TABLE_COLUMNS)
}

/// List deployed assets, optionally filtered by environment and name/key.
pub async fn cmd_list_deployed_assets(
    args: ListDeployedAssetsArgs,
    positionals: &[String],
) -> Result<()> {
    let (output, client) = super::shared::make_client(args.json, args.color)?;

    let environments = client.list_environments()?;

    // Resolve --env (name or key) up front so a typo fails loudly instead of silently
    // matching nothing.
    let env_key = if let Some(ref env_input) = args.env {
        if env_input.is_empty() {
            String::new()
        } else {
            crate::resolve::resolve(env_input, "environment", &environments, "key")?
        }
    } else {
        String::new()
    };

    let env_names: std::collections::HashMap<&str, &str> = environments
        .iter()
        .filter_map(|e| Some((e.get("key")?.as_str()?, e.get("name")?.as_str()?)))
        .collect();

    // Create a minimal Options struct just for fetch_listing compatibility
    let options = crate::cli::Options {
        json: args.json,
        color: args.color,
        offset: args.offset,
        limit: args.limit,
        ..Default::default()
    };

    let mut listing = fetch_listing(
        &options,
        |offset, limit| client.list_deployed_assets_page(offset, limit),
        || client.list_deployed_assets(),
    )?;

    let search = args
        .filter
        .as_deref()
        .or_else(|| positionals.first().map(String::as_str))
        .unwrap_or("");
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

/// Retrieve asset metadata by name or key.
pub async fn cmd_get_asset(args: GetAssetArgs, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("get-asset requires an asset name or key"));
    }

    let (output, client) = super::shared::make_client(args.json, args.color)?;

    let asset_key = &positionals[0];
    let asset = resolve_asset(&client, asset_key)?;
    output.print_result(&serde_json::Value::Object(asset))?;
    Ok(())
}
