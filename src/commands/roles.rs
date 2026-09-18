//! Role management commands.

use super::shared::*;
use crate::cli::Options;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use serde_json::{Map, Value};
use std::sync::Arc;

/// Resolve a role name/key to its key, optionally disambiguated by app (name/key).
fn resolve_role_key(client: &Client, role_input: &str, app_filter: &str) -> Result<String> {
    let mut roles = client.list_application_roles(role_input)?;

    if !app_filter.is_empty() {
        let asset_key = resolve_asset(client, app_filter)?
            .get("assetKey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", app_filter))?
            .to_string();
        roles.retain(|r| r.get("assetKey").and_then(|v| v.as_str()) == Some(asset_key.as_str()));
    }

    crate::resolve::resolve_role(role_input, &roles)
}

/// Resolve a group by key (GUID) or name (exact/unambiguous substring).
fn resolve_group(
    client: &Client,
    identifier: &str,
) -> Result<Map<String, Value>> {
    if crate::resolve::is_guid(identifier) {
        return client.get_group(identifier);
    }

    let candidates = client.list_groups(identifier, "")?;
    let key = crate::resolve::resolve(identifier, "group", &candidates, "key")?;
    candidates
        .into_iter()
        .find(|g| g.get("key").and_then(|v| v.as_str()) == Some(key.as_str()))
        .ok_or_else(|| anyhow::anyhow!("Group not found: {}", identifier))
}

/// Resolve a user by key (GUID) or email.
fn resolve_user(
    client: &Client,
    identifier: &str,
) -> Result<Map<String, Value>> {
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

/// Resolve an app's application roles, optionally narrowed to one environment, with each
/// role's `environment` name filled in from its `environmentKey`. Shared by `list-roles` and
/// `list-role-assignments`.
fn resolve_app_roles(client: &Client, app: &str, env: &str) -> Result<Vec<Map<String, Value>>> {
    let asset_key = resolve_asset(client, app)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", app))?
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

pub async fn cmd_list_roles(options: &Options, positionals: &[String]) -> Result<()> {
    require_positional(positionals, "list-roles", "an app name or key")?;

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let roles = resolve_app_roles(&client, &positionals[0], &options.env)?;

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

pub async fn cmd_list_role_assignments(options: &Options, positionals: &[String]) -> Result<()> {
    require_positional(positionals, "list-role-assignments", "an app name or key")?;

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let roles = resolve_app_roles(&client, &positionals[0], &options.env)?;

    let want_users = options.filter.is_empty() || options.filter == "User";
    let want_groups = options.filter.is_empty() || options.filter == "Group";

    let mut rows: Vec<Map<String, Value>> = Vec::new();

    if want_users {
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
                user.insert("type".to_string(), Value::String("User".to_string()));
                if let Some(env) = role.get("environment").cloned() {
                    user.insert("environment".to_string(), env);
                }
                rows.push(user);
            }
        }
    }

    if want_groups {
        // No `/application-roles/{key}/groups` endpoint exists, so for each distinct
        // environment among the resolved roles, list that environment's groups once and
        // check each group's own assigned roles for a match.
        let mut groups_by_env: std::collections::HashMap<String, Vec<Map<String, Value>>> =
            std::collections::HashMap::new();
        for role in &roles {
            let Some(env_key) = role.get("environmentKey").and_then(|v| v.as_str()) else {
                continue;
            };
            if !groups_by_env.contains_key(env_key) {
                groups_by_env.insert(env_key.to_string(), client.list_groups("", env_key)?);
            }
        }

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
            let Some(env_key) = role.get("environmentKey").and_then(|v| v.as_str()) else {
                continue;
            };

            for group in groups_by_env.get(env_key).into_iter().flatten() {
                let group_key = group
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Group has no key field"))?;
                let group_roles = client.list_group_application_roles(group_key)?;
                let has_role = group_roles
                    .iter()
                    .any(|r| r.get("key").and_then(|v| v.as_str()) == Some(role_key));
                if !has_role {
                    continue;
                }

                let mut row = group.clone();
                row.insert("role".to_string(), Value::String(role_name.clone()));
                row.insert("type".to_string(), Value::String("Group".to_string()));
                if let Some(env) = role.get("environment").cloned() {
                    row.insert("environment".to_string(), env);
                }
                rows.push(row);
            }
        }
    }

    let items = if output.json {
        rows
    } else {
        rows.iter()
            .map(|item| crate::value::compact_map(item, ROLE_ASSIGNMENT_TABLE_COLUMNS))
            .collect()
    };
    let results: Vec<Value> = items.into_iter().map(Value::Object).collect();
    output.print_result(&Value::Array(results))
}

pub async fn cmd_grant_role(options: &Options, positionals: &[String]) -> Result<()> {
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
    let role_key = resolve_role_key(&client, &positionals[1], &options.asset)?;

    client.grant_role(&user_key, &role_key)?;
    output.println_locked(&format!(
        "Granted role {} to {}",
        positionals[1], positionals[0]
    ));
    Ok(())
}

pub async fn cmd_revoke_role(options: &Options, positionals: &[String]) -> Result<()> {
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
    let role_key = resolve_role_key(&client, &positionals[1], &options.asset)?;

    client.revoke_role(&user_key, &role_key)?;
    output.println_locked(&format!(
        "Revoked role {} from {}",
        positionals[1], positionals[0]
    ));
    Ok(())
}

pub async fn cmd_grant_group_role(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.len() < 2 {
        return Err(anyhow::anyhow!(
            "grant-group-role requires a group and a role"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let group = resolve_group(&client, &positionals[0])?;
    let group_key = group
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Group {} has no key field", positionals[0]))?
        .to_string();
    let role_key = resolve_role_key(&client, &positionals[1], &options.asset)?;

    client.patch_group_application_roles(&group_key, &[role_key], &[])?;
    output.println_locked(&format!(
        "Granted role {} to group {}",
        positionals[1], positionals[0]
    ));
    Ok(())
}

pub async fn cmd_revoke_group_role(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.len() < 2 {
        return Err(anyhow::anyhow!(
            "revoke-group-role requires a group and a role"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let group = resolve_group(&client, &positionals[0])?;
    let group_key = group
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Group {} has no key field", positionals[0]))?
        .to_string();
    let role_key = resolve_role_key(&client, &positionals[1], &options.asset)?;

    client.patch_group_application_roles(&group_key, &[], &[role_key])?;
    output.println_locked(&format!(
        "Revoked role {} from group {}",
        positionals[1], positionals[0]
    ));
    Ok(())
}
