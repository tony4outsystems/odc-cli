//! User and group management commands.

use super::shared::*;
use crate::cli::Options;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use serde_json::{Map, Value};
use std::sync::Arc;

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

/// Resolve a group by key (GUID) or name (exact/unambiguous substring), optionally
/// disambiguated by environment (name/key), following the same contract as `resolve_app`.
fn resolve_group(
    client: &Client,
    identifier: &str,
    env_filter: &str,
) -> Result<Map<String, Value>> {
    if crate::resolve::is_guid(identifier) {
        return client.get_group(identifier);
    }

    let env_key = if env_filter.is_empty() {
        String::new()
    } else {
        resolve_env(client, env_filter)?
    };

    let candidates = client.list_groups(identifier, &env_key)?;
    let key = crate::resolve::resolve(identifier, "group", &candidates, "key")?;
    candidates
        .into_iter()
        .find(|g| g.get("key").and_then(|v| v.as_str()) == Some(key.as_str()))
        .ok_or_else(|| anyhow::anyhow!("Group not found: {}", identifier))
}

pub async fn cmd_get_user(options: &Options, positionals: &[String]) -> Result<()> {
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

pub async fn cmd_update_user(options: &Options, positionals: &[String]) -> Result<()> {
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

pub async fn cmd_list_groups(options: &Options, positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let env_key = if options.env.is_empty() {
        String::new()
    } else {
        resolve_env(&client, &options.env)?
    };
    let filter = positionals.first().map(String::as_str).unwrap_or("");
    let groups = client.list_groups(filter, &env_key)?;

    let items = if output.json {
        groups
    } else {
        groups
            .iter()
            .map(|item| crate::value::compact_map(item, GROUP_TABLE_COLUMNS))
            .collect()
    };
    let results: Vec<Value> = items.into_iter().map(Value::Object).collect();
    output.print_result(&Value::Array(results))
}

pub async fn cmd_get_group(options: &Options, positionals: &[String]) -> Result<()> {
    require_positional(positionals, "get-group", "a group name or key")?;

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let group = resolve_group(&client, &positionals[0], "")?;
    output.print_result(&Value::Object(group))
}

pub async fn cmd_update_group(options: &Options, positionals: &[String]) -> Result<()> {
    require_positional(positionals, "update-group", "a group name or key")?;
    if options.updates.is_empty() {
        return Err(anyhow::anyhow!(
            "update-group requires at least one of --name or --description"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let group = resolve_group(&client, &positionals[0], "")?;
    let group_key = group
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Group {} has no key field", positionals[0]))?
        .to_string();

    client.update_group(&group_key, &options.updates)?;
    let updated = client.get_group(&group_key)?;
    output.print_result(&Value::Object(updated))
}

pub async fn cmd_list_group_members(options: &Options, positionals: &[String]) -> Result<()> {
    require_positional(positionals, "list-group-members", "a group name or key")?;

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let group = resolve_group(&client, &positionals[0], "")?;
    let group_key = group
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Group {} has no key field", positionals[0]))?;
    let members = client.list_group_users(group_key)?;

    let items = if output.json {
        members
    } else {
        members
            .iter()
            .map(|item| {
                let mut flat = item
                    .get("user")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();
                if let Some(membership) = item.get("membershipTypes") {
                    flat.insert("membershipTypes".to_string(), membership.clone());
                }
                crate::value::compact_map(&flat, GROUP_USER_TABLE_COLUMNS)
            })
            .collect()
    };
    let results: Vec<Value> = items.into_iter().map(Value::Object).collect();
    output.print_result(&Value::Array(results))
}

pub async fn cmd_add_user_to_group(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.len() < 2 {
        return Err(anyhow::anyhow!(
            "add-user-to-group requires a group and a user"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let group = resolve_group(&client, &positionals[0], "")?;
    let group_key = group
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Group {} has no key field", positionals[0]))?
        .to_string();
    let user = resolve_user(&client, &positionals[1])?;
    let user_key = user
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("User {} has no key field", positionals[1]))?
        .to_string();

    client.patch_group_users(&group_key, &[user_key], &[])?;
    output.println_locked(&format!(
        "Added {} to group {}",
        positionals[1], positionals[0]
    ));
    Ok(())
}

pub async fn cmd_remove_user_from_group(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.len() < 2 {
        return Err(anyhow::anyhow!(
            "remove-user-from-group requires a group and a user"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let group = resolve_group(&client, &positionals[0], "")?;
    let group_key = group
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Group {} has no key field", positionals[0]))?
        .to_string();
    let user = resolve_user(&client, &positionals[1])?;
    let user_key = user
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("User {} has no key field", positionals[1]))?
        .to_string();

    client.patch_group_users(&group_key, &[], &[user_key])?;
    output.println_locked(&format!(
        "Removed {} from group {}",
        positionals[1], positionals[0]
    ));
    Ok(())
}
