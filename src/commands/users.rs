//! User and group management commands.

use super::args::*;
use super::context::Ctx;
use super::shared::*;
use crate::value::JsonMapExt;
use anyhow::Result;
use serde_json::{Map, Value};

pub async fn cmd_get_user(ctx: &Ctx, args: &GetUserArgs) -> Result<()> {
    let client = ctx.client()?;

    let identifier = &args.user;

    // A GUID or exact email unambiguously names one user; anything else searches by
    // name/email/username and returns every match, since "tony" could be several people.
    if crate::resolve::is_guid(identifier) {
        let user = client.get_user(identifier)?;
        return ctx.output.print_result(&serde_json::Value::Object(user));
    }
    if identifier.contains('@') {
        let user = client.find_user_by_email(identifier)?;
        return ctx.output.print_result(&serde_json::Value::Object(user));
    }

    let users = client.search_users(identifier)?;
    let items: Vec<Value> = if ctx.json() {
        users.into_iter().map(Value::Object).collect()
    } else {
        users
            .iter()
            .map(|u| Value::Object(crate::value::compact_map(u, USER_TABLE_COLUMNS)))
            .collect()
    };
    ctx.output.print_result(&Value::Array(items))
}

/// Split a single `--name` flag into `givenName`/`surname` on the first space, matching the
/// behavior this used to have as part of `Commands::as_update_user_args()` in `cli.rs`.
fn split_name(name: &str) -> (Option<String>, Option<String>) {
    let parts: Vec<&str> = name.splitn(2, ' ').collect();
    match parts.as_slice() {
        [first, last] => (Some(first.to_string()), Some(last.to_string())),
        [only] => (Some(only.to_string()), None),
        _ => (None, None),
    }
}

pub async fn cmd_update_user(ctx: &Ctx, args: &UpdateUserArgs) -> Result<()> {
    if args.name.is_none() {
        return Err(anyhow::anyhow!(
            "update-user requires at least one of --name"
        ));
    }

    let client = ctx.client()?;

    let user = resolve_user(&client, &args.user)?;
    let user_key = user.require_str("key", &format!("User {}", args.user))?;

    let (given_name, surname) = args.name.as_deref().map(split_name).unwrap_or((None, None));

    let mut updates = Map::new();
    if let Some(name) = given_name {
        updates.insert("givenName".to_string(), Value::String(name));
    }
    if let Some(name) = surname {
        updates.insert("surname".to_string(), Value::String(name));
    }

    let updated = client.update_user(user_key, &updates)?;
    ctx.output.print_result(&serde_json::Value::Object(updated))
}

pub async fn cmd_list_groups(ctx: &Ctx, args: &ListGroupsArgs) -> Result<()> {
    let client = ctx.client()?;

    let env_key = if let Some(ref env_input) = args.env {
        if env_input.is_empty() {
            String::new()
        } else {
            resolve_env(&client, env_input)?
        }
    } else {
        String::new()
    };
    let filter = args.group.as_deref().unwrap_or("");
    let mut groups = client.list_groups(filter, &env_key)?;

    // For table output, always populate "environment" field
    // Either with resolved name (default) or raw key (with -n flag)
    if !ctx.json() {
        annotate_env_names(&client, &mut groups, ctx.no_resolve)?;
    }

    let items = if ctx.json() {
        groups
    } else {
        groups
            .iter()
            .map(|item| crate::value::compact_map(item, GROUP_TABLE_COLUMNS))
            .collect()
    };
    let results: Vec<Value> = items.into_iter().map(Value::Object).collect();
    ctx.output.print_result(&Value::Array(results))
}

pub async fn cmd_get_group(ctx: &Ctx, args: &GetGroupArgs) -> Result<()> {
    let client = ctx.client()?;

    let group = resolve_group(&client, &args.group, "")?;
    ctx.output.print_result(&Value::Object(group))
}

pub async fn cmd_update_group(ctx: &Ctx, args: &UpdateGroupArgs) -> Result<()> {
    let client = ctx.client()?;

    let group = resolve_group(&client, &args.group, "")?;
    let group_key = group
        .require_str("key", &format!("Group {}", args.group))?
        .to_string();

    let mut updates = Map::new();
    if let Some(name) = &args.name {
        if !name.is_empty() {
            updates.insert("name".to_string(), Value::String(name.clone()));
        }
    }
    if let Some(desc) = &args.description {
        if !desc.is_empty() {
            updates.insert("description".to_string(), Value::String(desc.clone()));
        }
    }

    if !updates.is_empty() {
        client.update_group(&group_key, &updates)?;
    }
    let updated = client.get_group(&group_key)?;
    ctx.output.print_result(&Value::Object(updated))
}

pub async fn cmd_list_group_members(ctx: &Ctx, args: &ListGroupMembersArgs) -> Result<()> {
    let client = ctx.client()?;

    let group = resolve_group(&client, &args.group, "")?;
    let group_key = group.require_str("key", &format!("Group {}", args.group))?;
    let members = client.list_group_users(group_key)?;

    let items = if ctx.json() {
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
    ctx.output.print_result(&Value::Array(results))
}

pub async fn cmd_add_user_to_group(ctx: &Ctx, args: &AddUserToGroupArgs) -> Result<()> {
    let client = ctx.client()?;

    let group = resolve_group(&client, &args.group, "")?;
    let group_key = group
        .require_str("key", &format!("Group {}", args.group))?
        .to_string();
    let user = resolve_user(&client, &args.user)?;
    let user_key = user
        .require_str("key", &format!("User {}", args.user))?
        .to_string();

    client.patch_group_users(&group_key, &[user_key], &[])?;
    ctx.output
        .println_locked(&format!("Added {} to group {}", args.user, args.group));
    Ok(())
}

pub async fn cmd_remove_user_from_group(ctx: &Ctx, args: &RemoveUserFromGroupArgs) -> Result<()> {
    let client = ctx.client()?;

    let group = resolve_group(&client, &args.group, "")?;
    let group_key = group
        .require_str("key", &format!("Group {}", args.group))?
        .to_string();
    let user = resolve_user(&client, &args.user)?;
    let user_key = user
        .require_str("key", &format!("User {}", args.user))?
        .to_string();

    client.patch_group_users(&group_key, &[], &[user_key])?;
    ctx.output
        .println_locked(&format!("Removed {} from group {}", args.user, args.group));
    Ok(())
}
