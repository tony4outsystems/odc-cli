//! Shared utilities for command execution.
//!
//! This module provides common patterns used across multiple commands:
//! - [`Listing`]: paginated listing results
//! - [`fetch_listing()`]: fetch a single page or all results
//! - [`filter_by_substring()`]: filter items by text search
//! - [`print_listing()`]: output paginated or full results
//! - Table column definitions for various entity types
//!
//! Client/output construction lives on `commands::context::Ctx` (`ctx.client()`), not here.

use crate::client::Client;
use crate::value::JsonMapExt;
use anyhow::Result;
use serde_json::{Map, Value};

/// The result of a listing command: either a single page (with pagination metadata) or
/// every page already combined.
pub struct Listing {
    pub items: Vec<Map<String, Value>>,
    /// (offset, limit, next_offset) when a single page was requested.
    pub page: Option<(i64, i64, Option<i64>)>,
}

/// Fetch a listing that supports offset-based pagination: a single page when `offset` is set,
/// or every page combined otherwise.
pub fn fetch_listing(
    offset: Option<i64>,
    limit: i64,
    page_fn: impl FnOnce(i64, i64) -> Result<(Vec<Map<String, Value>>, Option<i64>)>,
    all_fn: impl FnOnce() -> Result<Vec<Map<String, Value>>>,
) -> Result<Listing> {
    match offset {
        Some(offset) => {
            let (items, next_offset) = page_fn(offset, limit)?;
            Ok(Listing {
                items,
                page: Some((offset, limit, next_offset)),
            })
        }
        None => Ok(Listing {
            items: all_fn()?,
            page: None,
        }),
    }
}

/// Keep only items where `fields` contains `filter` as a case-insensitive substring.
/// A `None`/empty filter is a no-op.
pub fn filter_by_substring(
    items: Vec<Map<String, Value>>,
    filter: Option<&str>,
    fields: &[&str],
) -> Vec<Map<String, Value>> {
    let filter = match filter {
        Some(f) if !f.is_empty() => f.to_lowercase(),
        _ => return items,
    };

    items
        .into_iter()
        .filter(|item| {
            fields.iter().any(|field| {
                item.get(*field)
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s.to_lowercase().contains(&filter))
            })
        })
        .collect()
}

/// Print a `Listing`: a plain JSON/table array when every page was fetched, or a
/// `{results, page}` envelope (in `--json` mode) carrying `page.nextOffset` when a single
/// page was requested, so the next page can be fetched with `--offset <nextOffset>`.
///
/// In table mode, rows are narrowed to `table_columns` first so wide/unreadable fields (guids,
/// digests, tagging metadata, ...) don't blow up the table; `--json` output is unaffected and
/// always includes every field the API returned.
pub fn print_listing(
    output: &crate::output::Output,
    listing: Listing,
    table_columns: &[&str],
) -> Result<()> {
    let items = if output.json {
        listing.items
    } else {
        listing
            .items
            .iter()
            .map(|item| crate::value::compact_map(item, table_columns))
            .collect()
    };
    let results: Vec<Value> = items.into_iter().map(Value::Object).collect();

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

/// Read a string status field off a result map.
pub fn status_str<'a>(map: &'a Map<String, Value>, field: &str) -> &'a str {
    map.str_or_empty(field)
}

/// Find an asset by exact `assetKey` or exact `name`. The asset-repository API identifies an
/// asset by `assetKey` (not `key`).
pub fn find_asset<'a>(
    assets: &'a [Map<String, Value>],
    identifier: &str,
) -> Option<&'a Map<String, Value>> {
    assets.iter().find(|asset| match asset.get("assetKey") {
        Some(Value::String(key)) if key == identifier => true,
        _ => matches!(asset.get("name"), Some(Value::String(name)) if name == identifier),
    })
}

/// Resolve a user-supplied asset name/key against an already-fetched list of assets. Supports a
/// GUID, an exact name or key, or an unambiguous substring of the name/key — falling back to a
/// "did you mean" error listing the candidates when the input is ambiguous or matches nothing
/// exactly (via `resolve::resolve`). Used when the caller already has the full asset list handy
/// (e.g. resolving many assets from a workflow file against one shared listing).
pub fn resolve_asset_in<'a>(
    assets: &'a [Map<String, Value>],
    identifier: &str,
) -> Result<&'a Map<String, Value>> {
    let asset_key = crate::resolve::resolve(identifier, "asset", assets, "assetKey")?;
    find_asset(assets, &asset_key).ok_or_else(|| anyhow::anyhow!("Asset not found: {}", identifier))
}

/// Resolve a user-supplied asset name/key to the asset it refers to, without fetching every asset in
/// the tenant. A GUID is looked up directly by key; otherwise the asset-repository API's
/// server-side `nameContains` filter narrows the candidates before applying the same
/// exact/unambiguous-partial-match/"did you mean" contract as `resolve_asset_in`.
pub fn resolve_asset(client: &Client, identifier: &str) -> Result<Map<String, Value>> {
    if identifier.is_empty() {
        return Err(anyhow::anyhow!("asset is required"));
    }
    if crate::resolve::is_guid(identifier) {
        return client.get_asset(identifier);
    }
    let candidates = client.find_assets_by_name(identifier)?;
    resolve_asset_in(&candidates, identifier).cloned()
}

/// Resolve a user-supplied asset name/key to the asset it refers to and its `assetKey`,
/// erroring with `"Asset {identifier} has no assetKey field"` if the resolved asset is somehow
/// missing one. Replaces the `resolve_asset(...)... .get("assetKey")... .ok_or_else(...)`
/// pattern repeated across `commands/deployment.rs`, `commands/revisions.rs`,
/// `commands/roles.rs`, and `inspection.rs`.
pub fn resolve_asset_key(
    client: &Client,
    identifier: &str,
) -> Result<(Map<String, Value>, String)> {
    let asset = resolve_asset(client, identifier)?;
    let asset_key = asset
        .require_str("assetKey", &format!("Asset {}", identifier))?
        .to_string();
    Ok((asset, asset_key))
}

/// Resolve the revision to act on: an explicit `--revision`, else the app's current revision,
/// falling back to the latest revision if the app has none set.
pub fn resolve_revision(
    client: &Client,
    app: &Map<String, Value>,
    asset_key: &str,
    explicit: Option<i32>,
) -> Result<i32> {
    if let Some(revision) = explicit {
        return Ok(revision);
    }
    if let Some(revision) = app.get("revision").and_then(|v| v.as_i64()) {
        return Ok(revision as i32);
    }
    client
        .list_revisions(asset_key)?
        .iter()
        .filter_map(|r| r.get("revision").and_then(|v| v.as_i64()))
        .max()
        .map(|r| r as i32)
        .ok_or_else(|| anyhow::anyhow!("No revisions found for asset {}", asset_key))
}

/// Resolve `--env` (name, key, or unambiguous partial name) to an environment key.
pub fn resolve_env(client: &Client, env_input: &str) -> Result<String> {
    let environments = client.list_environments()?;
    crate::resolve::resolve(env_input, "environment", &environments, "key")
}

/// Resolve an environment key to its human-readable name.
/// If the key is empty or cannot be resolved, returns the original key unchanged.
pub fn resolve_environment_key(client: &Client, env_key: &str) -> Result<String> {
    if env_key.is_empty() {
        return Ok(env_key.to_string());
    }

    let environments = client.list_environments()?;
    for env in environments {
        if let Some(Value::String(key)) = env.get("key") {
            if key == env_key {
                if let Some(Value::String(name)) = env.get("name") {
                    return Ok(name.clone());
                }
                break;
            }
        }
    }

    // Graceful fallback: return the original key if not found
    Ok(env_key.to_string())
}

/// Populate an `"environment"` field on each row from its `"environmentKey"`, resolving to the
/// human-readable name unless `no_resolve` is set (in which case the raw key is copied as-is).
/// Rows without an `environmentKey` string field are left untouched. Used for table output only
/// (`--json` output always carries `environmentKey` as-is and doesn't need this); shared by
/// `list-roles`, `list-role-assignments`, and `list-groups`.
pub fn annotate_env_names(
    client: &Client,
    rows: &mut [Map<String, Value>],
    no_resolve: bool,
) -> Result<()> {
    for row in rows.iter_mut() {
        if let Some(Value::String(env_key)) = row.get("environmentKey") {
            let env_value = if no_resolve {
                env_key.clone()
            } else {
                resolve_environment_key(client, env_key)?
            };
            row.insert("environment".to_string(), Value::String(env_value));
        }
    }
    Ok(())
}

// Table column definitions for various entity types

pub const PORTFOLIO_TABLE_COLUMNS: &[&str] = &["name", "key", "id"];
pub const ASSET_TABLE_COLUMNS: &[&str] = &["name", "assetKey", "assetType", "revision", "tag"];
pub const DEPLOYED_ASSET_TABLE_COLUMNS: &[&str] =
    &["name", "key", "type", "environment", "revision", "tag"];
pub const REVISION_TABLE_COLUMNS: &[&str] = &["revision", "tag", "createdAt", "createdBy"];
pub const ROLE_TABLE_COLUMNS: &[&str] = &["name", "key", "environment"];
pub const ROLE_ASSIGNMENT_TABLE_COLUMNS: &[&str] = &["role", "environment", "type", "name", "key"];
pub const GROUP_TABLE_COLUMNS: &[&str] = &["name", "key", "environment", "description"];
pub const GROUP_USER_TABLE_COLUMNS: &[&str] = &["name", "email", "key", "status"];
pub const USER_TABLE_COLUMNS: &[&str] = &["key", "name", "email", "status"];
