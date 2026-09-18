//! Shared utilities for command execution.
//!
//! This module provides common patterns used across multiple commands:
//! - [`Listing`]: paginated listing results
//! - [`fetch_listing()`]: fetch a single page or all results
//! - [`filter_by_substring()`]: filter items by text search
//! - [`print_listing()`]: output paginated or full results
//! - Table column definitions for various entity types

use crate::cli::Options;
use crate::client::Client;
use anyhow::Result;
use serde_json::{Map, Value};

/// The result of a listing command: either a single page (with pagination metadata) or
/// every page already combined.
pub struct Listing {
    pub items: Vec<Map<String, Value>>,
    /// (offset, limit, next_offset) when a single page was requested.
    pub page: Option<(i64, i64, Option<i64>)>,
}

/// Fetch a listing that supports offset-based pagination: a single page when
/// `options.offset` is set, or every page combined otherwise.
pub fn fetch_listing(
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

/// Return an error naming `what` if `positionals` is empty.
pub fn require_positional(positionals: &[String], command: &str, what: &str) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("{} requires {}", command, what));
    }
    Ok(())
}

/// Read a string status field off a result map.
pub fn status_str<'a>(map: &'a Map<String, Value>, field: &str) -> &'a str {
    map.get(field).and_then(|v| v.as_str()).unwrap_or("")
}

/// Find an app by exact `assetKey` or exact `name`. The asset-repository API identifies an
/// app by `assetKey` (not `key`).
pub fn find_app<'a>(
    apps: &'a [Map<String, Value>],
    identifier: &str,
) -> Option<&'a Map<String, Value>> {
    apps.iter().find(|app| match app.get("assetKey") {
        Some(Value::String(key)) if key == identifier => true,
        _ => matches!(app.get("name"), Some(Value::String(name)) if name == identifier),
    })
}

/// Resolve a user-supplied app name/key against an already-fetched list of apps. Supports a
/// GUID, an exact name or key, or an unambiguous substring of the name/key — falling back to a
/// "did you mean" error listing the candidates when the input is ambiguous or matches nothing
/// exactly (via `resolve::resolve`). Used when the caller already has the full app list handy
/// (e.g. resolving many apps from a workflow file against one shared listing).
pub fn resolve_app_in<'a>(
    apps: &'a [Map<String, Value>],
    identifier: &str,
) -> Result<&'a Map<String, Value>> {
    let asset_key = crate::resolve::resolve(identifier, "app", apps, "assetKey")?;
    find_app(apps, &asset_key).ok_or_else(|| anyhow::anyhow!("App not found: {}", identifier))
}

/// Resolve a user-supplied app name/key to the app it refers to, without fetching every app in
/// the tenant. A GUID is looked up directly by key; otherwise the asset-repository API's
/// server-side `nameContains` filter narrows the candidates before applying the same
/// exact/unambiguous-partial-match/"did you mean" contract as `resolve_app_in`.
pub fn resolve_app(client: &Client, identifier: &str) -> Result<Map<String, Value>> {
    if identifier.is_empty() {
        return Err(anyhow::anyhow!("app is required"));
    }
    if crate::resolve::is_guid(identifier) {
        return client.get_app(identifier);
    }
    let candidates = client.find_apps_by_name(identifier)?;
    resolve_app_in(&candidates, identifier).cloned()
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

// Table column definitions for various entity types

pub const PORTFOLIO_TABLE_COLUMNS: &[&str] = &["name", "key", "id"];
pub const APP_TABLE_COLUMNS: &[&str] = &["name", "assetKey", "assetType", "revision", "tag"];
pub const DEPLOYED_APP_TABLE_COLUMNS: &[&str] =
    &["name", "key", "type", "environment", "revision", "tag"];
pub const REVISION_TABLE_COLUMNS: &[&str] = &["revision", "tag", "createdAt", "createdBy"];
pub const ROLE_TABLE_COLUMNS: &[&str] = &["name", "key", "environment"];
pub const ROLE_ASSIGNMENT_TABLE_COLUMNS: &[&str] =
    &["role", "environment", "type", "name", "key"];
pub const GROUP_TABLE_COLUMNS: &[&str] = &["name", "key", "environmentKey", "description"];
pub const GROUP_USER_TABLE_COLUMNS: &[&str] = &["name", "email", "key", "status"];
pub const USER_TABLE_COLUMNS: &[&str] = &["key", "name", "email", "status"];
