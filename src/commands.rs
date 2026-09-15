use crate::cli::Options;
use crate::client::Client;
use crate::mentor::MentorClient;
use crate::settings;
use anyhow::Result;
use serde_json::{json, Map, Value};
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

/// Find an app by exact `assetKey` or exact `name`. The asset-repository API identifies an
/// app by `assetKey` (not `key`).
fn find_app<'a>(
    apps: &'a [Map<String, Value>],
    identifier: &str,
) -> Option<&'a Map<String, Value>> {
    apps.iter().find(|app| match app.get("assetKey") {
        Some(Value::String(key)) if key == identifier => true,
        _ => matches!(app.get("name"), Some(Value::String(name)) if name == identifier),
    })
}

/// Return an error naming `what` if `positionals` is empty.
fn require_positional(positionals: &[String], command: &str, what: &str) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("{} requires {}", command, what));
    }
    Ok(())
}

/// Resolve a user-supplied app name/key to the app it refers to. Supports a GUID, an exact
/// name or key, or an unambiguous substring of the name/key — falling back to a "did you
/// mean" error listing the candidates when the input is ambiguous or matches nothing exactly
/// (via `resolve::resolve`).
pub(crate) fn resolve_app<'a>(
    apps: &'a [Map<String, Value>],
    identifier: &str,
) -> Result<&'a Map<String, Value>> {
    let asset_key = crate::resolve::resolve(identifier, "app", apps, "assetKey")?;
    find_app(apps, &asset_key).ok_or_else(|| anyhow::anyhow!("App not found: {}", identifier))
}

/// Resolve the revision to act on: an explicit `--revision`, else the app's current revision,
/// falling back to the latest revision if the app has none set.
pub(crate) fn resolve_revision(
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

/// Read a string status field off a result map.
pub(crate) fn status_str<'a>(map: &'a Map<String, Value>, field: &str) -> &'a str {
    map.get(field).and_then(|v| v.as_str()).unwrap_or("")
}

/// Keep only items where `fields` contains `filter` as a case-insensitive substring.
/// A `None`/empty filter is a no-op.
fn filter_by_substring(
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

/// Columns shown for `list-apps` table output. `--json` still returns every field the API
/// sent; this only narrows what the human-readable table displays, since showing all ~20
/// asset fields (guids, digests, tagging metadata, ...) makes the table unreadable.
const APP_TABLE_COLUMNS: &[&str] = &["name", "assetKey", "assetType", "revision", "tag"];

/// Columns shown for `list-deployed-apps` table output; rows are one per app/environment
/// deployment (see `inspection::deployed_app_rows`), enriched with a resolved `environment`
/// name column alongside the raw `environmentKey` guid.
const DEPLOYED_APP_TABLE_COLUMNS: &[&str] =
    &["name", "key", "type", "environment", "revision", "tag"];

/// Columns shown for `list-revisions` table output; see `APP_TABLE_COLUMNS`.
const REVISION_TABLE_COLUMNS: &[&str] = &["revision", "tag", "createdAt", "createdBy"];

/// Print a `Listing`: a plain JSON/table array when every page was fetched, or a
/// `{results, page}` envelope (in `--json` mode) carrying `page.nextOffset` when a single
/// page was requested, so the next page can be fetched with `--offset <nextOffset>`.
///
/// In table mode, rows are narrowed to `table_columns` first so wide/unreadable fields (guids,
/// digests, tagging metadata, ...) don't blow up the table; `--json` output is unaffected and
/// always includes every field the API returned.
fn print_listing(
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
        "producer-graph" => cmd_producer_graph(options, positionals).await,
        "download-source-code" => cmd_download_source_code(options, positionals).await,
        "upload-source-code" => cmd_upload_source_code(options, positionals).await,
        "analyze-deployment" => cmd_analyze_deployment(options).await,
        "analyze-deletion" => cmd_analyze_deletion(options).await,
        "deploy" => cmd_deploy(options).await,
        "undeploy" => cmd_undeploy(options).await,
        "delete-app" => cmd_delete_app(options).await,
        "batch-deploy" => {
            require_positional(positionals, "batch-deploy", "an apps file")?;
            crate::workflows::batch_deploy(options, &positionals[0]).await
        }
        "batch-undeploy" => {
            require_positional(positionals, "batch-undeploy", "an apps file")?;
            crate::workflows::batch_undeploy(options, &positionals[0]).await
        }
        "batch-delete" => {
            require_positional(positionals, "batch-delete", "an apps file")?;
            crate::workflows::batch_delete(options, &positionals[0]).await
        }
        "dangerous-batch-undeploy-all" => {
            crate::workflows::dangerous_batch_undeploy_all(options).await
        }
        "get-user" => cmd_get_user(options, positionals).await,
        "update-user" => cmd_update_user(options, positionals).await,
        "list-roles" => cmd_list_roles(options, positionals).await,
        "list-app-role-users" => cmd_list_app_role_users(options, positionals).await,
        "grant-role" => cmd_grant_role(options, positionals).await,
        "revoke-role" => cmd_revoke_role(options, positionals).await,
        "internal-build" => cmd_internal_build(options).await,
        "internal-publish" => cmd_internal_publish(options).await,
        "internal-deploy" => cmd_internal_deploy(options).await,
        "mentor-start-session" => cmd_mentor_start_session(options).await,
        "mentor-create-asset" => cmd_mentor_create_asset(options).await,
        "mentor-load-asset" => cmd_mentor_load_asset(options, positionals).await,
        "mentor-prompt" => cmd_mentor_prompt(options).await,
        "mentor-get-run" => cmd_mentor_get_run(options).await,
        "mentor-get-event" => cmd_mentor_get_event(options).await,
        "mentor-cancel-prompt" => cmd_mentor_cancel_prompt(options).await,
        "mentor-close-session" => cmd_mentor_close_session(options).await,
        "mentor-request-upload" => cmd_mentor_request_upload(options).await,
        "mentor-publish" => cmd_mentor_publish(options).await,
        _ => Err(anyhow::anyhow!("Unknown command: {}", cmd)),
    }
}

fn mentor_client(options: &Options) -> Result<(MentorClient, Arc<crate::output::Output>)> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    Ok((MentorClient::new(settings), output))
}

async fn cmd_mentor_start_session(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;
    let result = client.call_tool("mentor_start_session", json!({}))?;
    output.print_result(&result)
}

async fn cmd_mentor_create_asset(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let mut args = Map::new();
    args.insert("sessionId".to_string(), json!(options.session_id));
    args.insert("assetType".to_string(), json!(options.mentor_asset_type));
    args.insert("name".to_string(), json!(options.name));
    args.insert("portfolioKey".to_string(), json!(options.portfolio_key));
    if !options.description.is_empty() {
        args.insert("description".to_string(), json!(options.description));
    }
    if !options.template_asset_key.is_empty() {
        args.insert(
            "templateAssetKey".to_string(),
            json!(options.template_asset_key),
        );
    }

    let result = client.call_tool("mentor_create_asset", Value::Object(args))?;
    output.print_result(&result)
}

async fn cmd_mentor_load_asset(options: &Options, positionals: &[String]) -> Result<()> {
    require_positional(positionals, "mentor-load-asset", "an asset key")?;
    let (client, output) = mentor_client(options)?;

    let mut args = Map::new();
    args.insert("sessionId".to_string(), json!(options.session_id));
    args.insert("assetKey".to_string(), json!(positionals[0]));
    if let Some(revision) = options.revision {
        args.insert("revision".to_string(), json!(revision));
    }

    let result = client.call_tool("mentor_load_asset", Value::Object(args))?;
    output.print_result(&result)
}

async fn cmd_mentor_prompt(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let mut args = Map::new();
    args.insert("sessionId".to_string(), json!(options.session_id));
    args.insert("message".to_string(), json!(options.message));
    if !options.attachment_refs.is_empty() {
        args.insert("attachmentRefs".to_string(), json!(options.attachment_refs));
    }

    let result = client.call_tool("mentor_prompt", Value::Object(args))?;
    output.print_result(&result)
}

async fn cmd_mentor_get_run(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let mut args = Map::new();
    args.insert("sessionId".to_string(), json!(options.session_id));
    args.insert("runId".to_string(), json!(options.run_id));
    if let Some(cursor) = options.cursor {
        args.insert("cursor".to_string(), json!(cursor));
    }

    let result = client.call_tool("mentor_get_run", Value::Object(args))?;
    output.print_result(&result)
}

async fn cmd_mentor_get_event(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let args = json!({
        "sessionId": options.session_id,
        "runId": options.run_id,
        "eventId": options.event_id,
    });

    let result = client.call_tool("mentor_get_event", args)?;
    output.print_result(&result)
}

async fn cmd_mentor_cancel_prompt(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let args = json!({
        "sessionId": options.session_id,
        "runId": options.run_id,
    });

    let result = client.call_tool("mentor_cancel_prompt", args)?;
    output.print_result(&result)
}

async fn cmd_mentor_close_session(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;
    let args = json!({ "sessionId": options.session_id });
    let result = client.call_tool("mentor_close_session", args)?;
    output.print_result(&result)
}

async fn cmd_mentor_request_upload(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let args = json!({
        "sessionId": options.session_id,
        "fileName": options.file_name,
        "sizeBytes": options.size_bytes,
    });

    let result = client.call_tool("mentor_request_upload", args)?;
    output.print_result(&result)
}

async fn cmd_mentor_publish(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let mut args = Map::new();
    args.insert("sessionId".to_string(), json!(options.session_id));
    if !options.comment.is_empty() {
        args.insert("comment".to_string(), json!(options.comment));
    }

    let result = client.call_tool("mentor_publish", Value::Object(args))?;
    output.print_result(&result)
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

async fn cmd_list_apps(options: &Options, positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let mut listing = fetch_listing(
        options,
        |offset, limit| client.list_apps_page(offset, limit),
        || client.list_apps(),
    )?;
    listing.items = filter_by_substring(
        listing.items,
        positionals.first().map(String::as_str),
        &["name", "assetKey"],
    );
    if !options.app_type.is_empty() {
        listing.items.retain(|app| {
            app.get("assetType").and_then(|v| v.as_str()) == Some(options.app_type.as_str())
        });
    }
    print_listing(&output, listing, APP_TABLE_COLUMNS)
}

async fn cmd_list_deployed_apps(options: &Options, positionals: &[String]) -> Result<()> {
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
        |offset, limit| client.list_deployed_apps_page(offset, limit),
        || client.list_deployed_apps(),
    )?;

    let search = positionals.first().map(String::as_str).unwrap_or("");
    let mut rows = crate::inspection::deployed_app_rows(&listing.items, &env_key, search);
    for row in &mut rows {
        if let Some(Value::String(key)) = row.get("environmentKey").cloned() {
            let name = env_names.get(key.as_str()).copied().unwrap_or(&key);
            row.insert("environment".to_string(), Value::String(name.to_string()));
        }
    }
    listing.items = rows;

    print_listing(&output, listing, DEPLOYED_APP_TABLE_COLUMNS)
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

    let app = resolve_app(&apps, app_key)?;
    output.print_result(&serde_json::Value::Object(app.clone()))?;
    Ok(())
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

    let app = resolve_app(&apps, app_key)?;
    let revision = app
        .get("revision")
        .ok_or_else(|| anyhow::anyhow!("App {} has no revision field", app_key))?;
    output.print_result(revision)?;
    Ok(())
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

    let asset_key = resolve_app(&apps, app_key)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", app_key))?
        .to_string();

    let listing = fetch_listing(
        options,
        |offset, limit| client.list_revisions_page(&asset_key, offset, limit),
        || crate::inspection::list_revisions(&client, &apps, app_key),
    )?;
    print_listing(&output, listing, REVISION_TABLE_COLUMNS)
}

async fn cmd_get_revision(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("get-revision requires an app name or key"));
    }
    let revision = options
        .revision
        .ok_or_else(|| anyhow::anyhow!("get-revision requires --revision <number>"))?;

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];

    let found = crate::inspection::get_revision(&client, &apps, app_key, revision)?;

    output.print_result(&serde_json::Value::Object(found))?;
    Ok(())
}

async fn cmd_producer_graph(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "producer-graph requires an app name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];
    let app = resolve_app(&apps, app_key)?;

    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", app_key))?
        .to_string();

    let revision = match options.revision {
        Some(revision) => revision,
        None => app
            .get("revision")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("App {} has no revision field", app_key))?
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

    let mut root = app.clone();
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

/// Resolve `--env` (name, key, or unambiguous partial name) to an environment key.
pub(crate) fn resolve_env(client: &Client, env_input: &str) -> Result<String> {
    let environments = client.list_environments()?;
    crate::resolve::resolve(env_input, "environment", &environments, "key")
}

/// Resolve a user by key (GUID), exact email, or name/email search — following the same
/// exact-match/unambiguous-partial-match/"did you mean" contract as `resolve_app`/`resolve_env`.
fn resolve_user(client: &Client, identifier: &str) -> Result<Map<String, Value>> {
    if crate::resolve::is_guid(identifier) {
        return client.get_user(identifier);
    }

    let candidates = client.search_users(identifier)?;

    if identifier.contains('@') {
        if let Some(exact) = candidates.iter().find(|u| {
            u.get("email")
                .and_then(|v| v.as_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(identifier))
        }) {
            return Ok(exact.clone());
        }
    }

    let key = crate::resolve::resolve(identifier, "user", &candidates, "key")?;
    candidates
        .into_iter()
        .find(|u| u.get("key").and_then(|v| v.as_str()) == Some(key.as_str()))
        .ok_or_else(|| anyhow::anyhow!("User not found: {}", identifier))
}

/// Resolve a role name/key to its key, optionally disambiguated by app (name/key).
fn resolve_role_key(client: &Client, role_input: &str, app_filter: &str) -> Result<String> {
    let mut roles = client.list_application_roles(role_input)?;

    if !app_filter.is_empty() {
        let apps = client.list_apps()?;
        let asset_key = resolve_app(&apps, app_filter)?
            .get("assetKey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", app_filter))?
            .to_string();
        roles.retain(|r| r.get("assetKey").and_then(|v| v.as_str()) == Some(asset_key.as_str()));
    }

    crate::resolve::resolve(role_input, "role", &roles, "key")
}

fn analysis_is_terminal(map: &Map<String, Value>) -> bool {
    matches!(status_str(map, "processStatus"), "Finished" | "Failed")
}

async fn cmd_analyze_deployment(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app = resolve_app(&apps, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;
    let revision = resolve_revision(&client, app, &asset_key, options.revision)?;

    let started = client.start_deployment_analysis(&asset_key, revision, &env_key)?;
    let analysis_key = started
        .get("analysisKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Deployment analysis response has no analysisKey"))?
        .to_string();

    if options.no_wait {
        return output.print_result(&serde_json::json!({ "analysisKey": analysis_key }));
    }

    let result = crate::workflows::wait_for(
        "deployment analysis",
        || client.get_deployment_analysis(&analysis_key),
        analysis_is_terminal,
        options.interval,
        options.timeout,
    )
    .await?;

    if status_str(&result, "processStatus") == "Failed" {
        return Err(anyhow::anyhow!(
            "Deployment analysis failed: {}",
            serde_json::Value::Object(result)
        ));
    }

    output.print_result(&serde_json::Value::Object(result))
}

async fn cmd_analyze_deletion(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let asset_key = resolve_app(&apps, &options.app)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();

    let started = client.start_deletion_analysis(&asset_key)?;
    let analysis_key = started
        .get("analysisKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Deletion analysis response has no analysisKey"))?
        .to_string();

    if options.no_wait {
        return output.print_result(&serde_json::json!({ "analysisKey": analysis_key }));
    }

    let result = crate::workflows::wait_for(
        "deletion analysis",
        || client.get_deletion_analysis(&analysis_key),
        analysis_is_terminal,
        options.interval,
        options.timeout,
    )
    .await?;

    if status_str(&result, "processStatus") == "Failed" {
        return Err(anyhow::anyhow!(
            "Deletion analysis failed: {}",
            serde_json::Value::Object(result)
        ));
    }

    output.print_result(&serde_json::Value::Object(result))
}

fn build_is_terminal(map: &Map<String, Value>) -> bool {
    matches!(
        status_str(map, "status"),
        "Finished" | "FinishedWithErrors" | "Deleted" | "ToBeDeleted"
    )
}

fn operation_is_terminal(map: &Map<String, Value>) -> bool {
    matches!(status_str(map, "status"), "Finished" | "FinishedWithError")
}

/// Start a build for `asset_key`/`revision` and, unless `--no-wait`, poll until it finishes.
/// Returns the build key and, when waited for, errors out on `FinishedWithErrors`.
pub(crate) async fn run_build(
    client: &Client,
    options: &Options,
    asset_key: &str,
    revision: i32,
) -> Result<(String, Option<Map<String, Value>>)> {
    let started = client.start_build(asset_key, revision, &options.build_type)?;
    let build_key = started
        .get("buildKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Build response has no buildKey"))?
        .to_string();

    if options.no_wait {
        return Ok((build_key, None));
    }

    let result = crate::workflows::wait_for(
        "build",
        || client.get_build(&build_key),
        build_is_terminal,
        options.interval,
        options.timeout,
    )
    .await?;

    if status_str(&result, "status") == "FinishedWithErrors" {
        return Err(anyhow::anyhow!(
            "Build failed: {}",
            serde_json::Value::Object(result)
        ));
    }

    Ok((build_key, Some(result)))
}

async fn cmd_internal_build(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app = resolve_app(&apps, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let revision = resolve_revision(&client, app, &asset_key, options.revision)?;

    let (build_key, result) = run_build(&client, options, &asset_key, revision).await?;

    match result {
        Some(result) => output.print_result(&serde_json::Value::Object(result)),
        None => output.print_result(&serde_json::json!({ "buildKey": build_key })),
    }
}

async fn cmd_internal_publish(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app = resolve_app(&apps, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;
    let revision = resolve_revision(&client, app, &asset_key, options.revision)?;

    let started = client.start_publish(&asset_key, revision, &env_key)?;
    let operation_key = started
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Publish response has no key"))?
        .to_string();

    if options.no_wait {
        return output.print_result(&serde_json::Value::Object(started));
    }

    let result = crate::workflows::wait_for(
        "publish",
        || client.get_publish(&operation_key),
        operation_is_terminal,
        options.interval,
        options.timeout,
    )
    .await?;

    if status_str(&result, "status") == "FinishedWithError" {
        return Err(anyhow::anyhow!(
            "Publish failed: {}",
            serde_json::Value::Object(result)
        ));
    }

    output.print_result(&serde_json::Value::Object(result))
}

async fn cmd_internal_deploy(options: &Options) -> Result<()> {
    if options.build_key.is_empty() {
        return Err(anyhow::anyhow!("internal-deploy requires --build-key"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app = resolve_app(&apps, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;
    let revision = resolve_revision(&client, app, &asset_key, options.revision)?;

    let (operation_key, result) = run_deployment_operation(
        &client,
        options,
        "Deploy",
        &asset_key,
        &env_key,
        Some(revision),
        Some(&options.build_key),
    )
    .await?;

    match result {
        Some(result) => output.print_result(&serde_json::Value::Object(result)),
        None => output.print_result(&serde_json::json!({ "operationKey": operation_key })),
    }
}

/// Start a deployment operation (`Deploy`/`Undeploy`) and, unless `--no-wait`, poll until it
/// finishes, erroring out on `FinishedWithError`.
pub(crate) async fn run_deployment_operation(
    client: &Client,
    options: &Options,
    operation: &str,
    asset_key: &str,
    env_key: &str,
    revision: Option<i32>,
    build_key: Option<&str>,
) -> Result<(String, Option<Map<String, Value>>)> {
    let started =
        client.start_deployment_operation(operation, asset_key, env_key, revision, build_key)?;
    let operation_key = started
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{} response has no key", operation))?
        .to_string();

    if options.no_wait {
        return Ok((operation_key, None));
    }

    let result = crate::workflows::wait_for(
        operation,
        || client.get_deployment_operation(&operation_key),
        operation_is_terminal,
        options.interval,
        options.timeout,
    )
    .await?;

    if status_str(&result, "status") == "FinishedWithError" {
        return Err(anyhow::anyhow!(
            "{} failed: {}",
            operation,
            serde_json::Value::Object(result)
        ));
    }

    Ok((operation_key, Some(result)))
}

async fn cmd_deploy(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app = resolve_app(&apps, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;
    let revision = resolve_revision(&client, app, &asset_key, options.revision)?;

    if options.no_wait {
        // --no-wait doesn't make sense for a multi-step composite command: we always need
        // the build to finish before we know it's safe to deploy it.
        return Err(anyhow::anyhow!(
            "deploy does not support --no-wait; use internal-build/internal-deploy instead"
        ));
    }

    let (build_key, _) = run_build(&client, options, &asset_key, revision).await?;

    let (_, deploy_result) = run_deployment_operation(
        &client,
        options,
        "Deploy",
        &asset_key,
        &env_key,
        Some(revision),
        Some(&build_key),
    )
    .await?;

    output.print_result(&serde_json::Value::Object(
        deploy_result.unwrap_or_default(),
    ))
}

async fn cmd_undeploy(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let asset_key = resolve_app(&apps, &options.app)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;

    let (operation_key, result) = run_deployment_operation(
        &client, options, "Undeploy", &asset_key, &env_key, None, None,
    )
    .await?;

    match result {
        Some(result) => output.print_result(&serde_json::Value::Object(result)),
        None => output.print_result(&serde_json::json!({ "operationKey": operation_key })),
    }
}

async fn cmd_delete_app(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let asset_key = resolve_app(&apps, &options.app)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();

    client.delete_asset(&asset_key)?;
    output.println_locked(&format!("Deleted app {}", options.app));
    Ok(())
}

async fn cmd_update_user(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("update-user requires a user key or email"));
    }
    if options.updates.is_empty() {
        return Err(anyhow::anyhow!(
            "update-user requires at least one of --name, --is-active, or --photo-url"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let user = resolve_user(&client, &positionals[0])?;
    let user_key = user
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("User {} has no key field", positionals[0]))?;

    let updated = client.update_user(user_key, &options.updates)?;
    output.print_result(&serde_json::Value::Object(updated))
}

const ROLE_TABLE_COLUMNS: &[&str] = &["name", "key", "assetKey", "environment"];
const ROLE_USER_TABLE_COLUMNS: &[&str] = &["role", "name", "email", "key", "status"];

/// Resolve an app's application roles, optionally narrowed to one environment, with each
/// role's `environment` name filled in from its `environmentKey`. Shared by `list-roles` and
/// `list-app-role-users`.
fn resolve_app_roles(
    client: &Client,
    apps: &[Map<String, Value>],
    app: &str,
    env: &str,
) -> Result<Vec<Map<String, Value>>> {
    let asset_key = resolve_app(apps, app)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", app))?
        .to_string();

    let environments = client.list_environments()?;

    // Resolve --env (name or key) up front so a typo fails loudly instead of silently
    // matching nothing.
    let env_key = if env.is_empty() {
        String::new()
    } else {
        crate::resolve::resolve(env, "environment", &environments, "key")?
    };
    let env_names: std::collections::HashMap<&str, &str> = environments
        .iter()
        .filter_map(|e| Some((e.get("key")?.as_str()?, e.get("name")?.as_str()?)))
        .collect();

    let mut roles = client.list_application_roles("")?;
    roles.retain(|r| r.get("assetKey").and_then(|v| v.as_str()) == Some(asset_key.as_str()));
    if !env_key.is_empty() {
        roles
            .retain(|r| r.get("environmentKey").and_then(|v| v.as_str()) == Some(env_key.as_str()));
    }

    for role in &mut roles {
        if let Some(Value::String(key)) = role.get("environmentKey").cloned() {
            let name = env_names.get(key.as_str()).copied().unwrap_or(&key);
            role.insert("environment".to_string(), Value::String(name.to_string()));
        }
    }

    Ok(roles)
}

async fn cmd_list_roles(options: &Options, positionals: &[String]) -> Result<()> {
    require_positional(positionals, "list-roles", "an app name or key")?;

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let roles = resolve_app_roles(&client, &apps, &positionals[0], &options.env)?;

    let items = if output.json {
        roles
    } else {
        roles
            .iter()
            .map(|item| crate::value::compact_map(item, ROLE_TABLE_COLUMNS))
            .collect()
    };
    let results: Vec<Value> = items.into_iter().map(Value::Object).collect();
    output.print_result(&Value::Array(results))
}

async fn cmd_list_app_role_users(options: &Options, positionals: &[String]) -> Result<()> {
    require_positional(positionals, "list-app-role-users", "an app name or key")?;

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let roles = resolve_app_roles(&client, &apps, &positionals[0], &options.env)?;

    let mut rows: Vec<Map<String, Value>> = Vec::new();
    for role in &roles {
        let role_key = role
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Role has no key field"))?;
        let role_name = role
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(role_key)
            .to_string();

        let users = client.list_application_role_users(role_key)?;
        for mut user in users {
            user.insert("role".to_string(), Value::String(role_name.clone()));
            if let Some(env) = role.get("environment").cloned() {
                user.insert("environment".to_string(), env);
            }
            rows.push(user);
        }
    }

    let items = if output.json {
        rows
    } else {
        rows.iter()
            .map(|item| crate::value::compact_map(item, ROLE_USER_TABLE_COLUMNS))
            .collect()
    };
    let results: Vec<Value> = items.into_iter().map(Value::Object).collect();
    output.print_result(&Value::Array(results))
}

async fn cmd_grant_role(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.len() < 2 {
        return Err(anyhow::anyhow!("grant-role requires a user and a role"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let user = resolve_user(&client, &positionals[0])?;
    let user_key = user
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("User {} has no key field", positionals[0]))?
        .to_string();
    let role_key = resolve_role_key(&client, &positionals[1], &options.app)?;

    client.grant_role(&user_key, &role_key)?;
    output.println_locked(&format!(
        "Granted role {} to {}",
        positionals[1], positionals[0]
    ));
    Ok(())
}

async fn cmd_revoke_role(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.len() < 2 {
        return Err(anyhow::anyhow!("revoke-role requires a user and a role"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let user = resolve_user(&client, &positionals[0])?;
    let user_key = user
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("User {} has no key field", positionals[0]))?
        .to_string();
    let role_key = resolve_role_key(&client, &positionals[1], &options.app)?;

    client.revoke_role(&user_key, &role_key)?;
    output.println_locked(&format!(
        "Revoked role {} from {}",
        positionals[1], positionals[0]
    ));
    Ok(())
}

async fn cmd_download_source_code(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "download-source-code requires an app name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];
    let app = resolve_app(&apps, app_key)?;

    let revision = match options.revision {
        Some(revision) => revision,
        None => app
            .get("revision")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("App {} has no revision field", app_key))?
            as i32,
    };

    let (output_path, _bytes) = crate::inspection::download_source_code(
        &client,
        &apps,
        app_key,
        revision,
        &options.output,
    )?;

    if options.json {
        output.print_result(&serde_json::json!({ "output": output_path }))?;
    } else {
        output.println_locked(&format!("Wrote source code to {}", output_path));
    }
    Ok(())
}

async fn cmd_upload_source_code(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "upload-source-code requires an OML/XIF file"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let bytes = std::fs::read(&positionals[0])
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", positionals[0], e))?;

    let upload_url = client.request_upload_url()?;
    client.upload_file_bytes(&upload_url, bytes)?;
    let created = client.create_asset_revision(&upload_url)?;

    output.print_result(&serde_json::Value::Object(created))
}

/// Columns shown for `get-user`'s search-list table output; see `APP_TABLE_COLUMNS`.
const USER_TABLE_COLUMNS: &[&str] = &["key", "name", "email", "status"];

async fn cmd_get_user(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "get-user requires a user key, email, or name"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let identifier = &positionals[0];

    // A GUID or exact email unambiguously names one user; anything else searches by
    // name/email/username and returns every match, since "tony" could be several people.
    if crate::resolve::is_guid(identifier) {
        let user = client.get_user(identifier)?;
        return output.print_result(&serde_json::Value::Object(user));
    }
    if identifier.contains('@') {
        let user = client.find_user_by_email(identifier)?;
        return output.print_result(&serde_json::Value::Object(user));
    }

    let users = client.search_users(identifier)?;
    let items: Vec<Value> = if output.json {
        users.into_iter().map(Value::Object).collect()
    } else {
        users
            .iter()
            .map(|u| Value::Object(crate::value::compact_map(u, USER_TABLE_COLUMNS)))
            .collect()
    };
    output.print_result(&Value::Array(items))
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

    #[test]
    fn test_app_table_columns_drop_wide_fields() {
        let mut app = Map::new();
        app.insert("name".to_string(), serde_json::json!("MyApp"));
        app.insert("assetKey".to_string(), serde_json::json!("guid-1"));
        app.insert("assetType".to_string(), serde_json::json!("WebApplication"));
        app.insert("revision".to_string(), serde_json::json!(3));
        app.insert("tag".to_string(), serde_json::json!("1.0.0"));
        app.insert("modelDigest".to_string(), serde_json::json!("digest-guid"));
        app.insert(
            "description".to_string(),
            serde_json::json!("a very long description"),
        );

        let compacted = crate::value::compact_map(&app, APP_TABLE_COLUMNS);

        assert_eq!(compacted.len(), 5);
        assert!(!compacted.contains_key("modelDigest"));
        assert!(!compacted.contains_key("description"));
    }

    fn sample_apps() -> Vec<Map<String, Value>> {
        let mut app1 = Map::new();
        app1.insert("assetKey".to_string(), serde_json::json!("guid-1"));
        app1.insert("name".to_string(), serde_json::json!("Zip"));

        let mut app2 = Map::new();
        app2.insert("assetKey".to_string(), serde_json::json!("guid-2"));
        app2.insert("name".to_string(), serde_json::json!("MyApp"));

        vec![app1, app2]
    }

    #[test]
    fn test_find_app_by_asset_key() {
        let apps = sample_apps();
        let found = find_app(&apps, "guid-2").unwrap();
        assert_eq!(found.get("name").unwrap(), "MyApp");
    }

    #[test]
    fn test_find_app_by_name() {
        let apps = sample_apps();
        let found = find_app(&apps, "Zip").unwrap();
        assert_eq!(found.get("assetKey").unwrap(), "guid-1");
    }

    #[test]
    fn test_find_app_not_found() {
        let apps = sample_apps();
        assert!(find_app(&apps, "does-not-exist").is_none());
    }
}
