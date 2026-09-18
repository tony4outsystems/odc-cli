//! Mentor (AI) integration commands.

use crate::cli::Options;
use crate::mentor::MentorClient;
use crate::settings;
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::sync::Arc;

fn mentor_client(options: &Options) -> Result<(MentorClient, Arc<crate::output::Output>)> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    Ok((MentorClient::new(settings), output))
}

pub async fn cmd_mentor_start_session(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;
    let result = client.call_tool("mentor_start_session", json!({}))?;
    output.print_result(&result)
}

pub async fn cmd_mentor_create_asset(options: &Options) -> Result<()> {
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

pub async fn cmd_mentor_load_asset(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("mentor-load-asset requires an asset key"));
    }
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

pub async fn cmd_mentor_prompt(options: &Options) -> Result<()> {
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

pub async fn cmd_mentor_get_run(options: &Options) -> Result<()> {
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

pub async fn cmd_mentor_get_event(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let args = json!({
        "sessionId": options.session_id,
        "runId": options.run_id,
        "eventId": options.event_id,
    });

    let result = client.call_tool("mentor_get_event", args)?;
    output.print_result(&result)
}

pub async fn cmd_mentor_cancel_prompt(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let args = json!({
        "sessionId": options.session_id,
        "runId": options.run_id,
    });

    let result = client.call_tool("mentor_cancel_prompt", args)?;
    output.print_result(&result)
}

pub async fn cmd_mentor_close_session(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;
    let args = json!({ "sessionId": options.session_id });
    let result = client.call_tool("mentor_close_session", args)?;
    output.print_result(&result)
}

pub async fn cmd_mentor_request_upload(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let args = json!({
        "sessionId": options.session_id,
        "fileName": options.file_name,
        "sizeBytes": options.size_bytes,
    });

    let result = client.call_tool("mentor_request_upload", args)?;
    output.print_result(&result)
}

pub async fn cmd_mentor_publish(options: &Options) -> Result<()> {
    let (client, output) = mentor_client(options)?;

    let mut args = Map::new();
    args.insert("sessionId".to_string(), json!(options.session_id));
    if !options.comment.is_empty() {
        args.insert("comment".to_string(), json!(options.comment));
    }

    let result = client.call_tool("mentor_publish", Value::Object(args))?;
    output.print_result(&result)
}
