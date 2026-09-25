//! Asset listing and detail commands.

use super::args::{GetAssetArgs, ListAssetsArgs, ListDeployedAssetsArgs};
use super::context::Ctx;
use super::shared::*;
use anyhow::Result;
use serde_json::Value;

/// List assets with optional filtering by name/key and/or asset type.
pub async fn cmd_list_assets(ctx: &Ctx, args: &ListAssetsArgs) -> Result<()> {
    let client = ctx.client()?;

    let mut listing = fetch_listing(
        args.offset,
        args.limit,
        |offset, limit| client.list_assets_page(offset, limit),
        || client.list_assets(),
    )?;

    listing.items =
        filter_by_substring(listing.items, args.asset.as_deref(), &["name", "assetKey"]);

    if let Some(asset_type) = args.app_type {
        let type_str = asset_type.as_str();
        listing
            .items
            .retain(|item| item.get("assetType").and_then(|v| v.as_str()) == Some(type_str));
    }

    print_listing(&ctx.output, listing, ASSET_TABLE_COLUMNS)
}

/// List deployed assets, optionally filtered by environment and name/key.
pub async fn cmd_list_deployed_assets(ctx: &Ctx, args: &ListDeployedAssetsArgs) -> Result<()> {
    let client = ctx.client()?;

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

    let mut listing = fetch_listing(
        args.offset,
        args.limit,
        |offset, limit| client.list_deployed_assets_page(offset, limit),
        || client.list_deployed_assets(),
    )?;

    let search = args.asset.as_deref().unwrap_or("");
    let mut rows = crate::inspection::deployed_asset_rows(&listing.items, &env_key, search);
    for row in &mut rows {
        if let Some(Value::String(key)) = row.get("environmentKey").cloned() {
            let name = env_names.get(key.as_str()).copied().unwrap_or(&key);
            row.insert("environment".to_string(), Value::String(name.to_string()));
        }
    }
    listing.items = rows;

    print_listing(&ctx.output, listing, DEPLOYED_ASSET_TABLE_COLUMNS)
}

/// Retrieve asset metadata by name or key.
pub async fn cmd_get_asset(ctx: &Ctx, args: &GetAssetArgs) -> Result<()> {
    let client = ctx.client()?;

    let asset = resolve_asset(&client, &args.asset)?;
    ctx.output.print_result(&serde_json::Value::Object(asset))?;
    Ok(())
}
