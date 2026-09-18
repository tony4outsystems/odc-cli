//! Mentor (AI) integration commands.

use super::args::*;
use crate::mentor::MentorClient;
use crate::output::Output;
use crate::settings;
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::sync::Arc;

fn mentor_client(json: bool, color: crate::output::ColorMode) -> Result<(MentorClient, Arc<Output>)> {
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(json, color));
    Ok((MentorClient::new(settings), output))
}

pub async fn cmd_mentor_start_session(args: MentorSessionArgs) -> Result<()> {
    let (client, output) = mentor_client(args.json, args.color)?;
    let result = client.call_tool("mentor_start_session", json!({}))?;
    output.print_result(&result)
}

pub async fn cmd_mentor_create_asset(args: MentorCreateAssetArgs) -> Result<()> {
    let (client, output) = mentor_client(args.json, args.color)?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    tool_args.insert("assetType".to_string(), json!(args.asset_type));
    tool_args.insert("name".to_string(), json!(args.name));
    tool_args.insert("portfolioKey".to_string(), json!(args.portfolio_key));
    if let Some(description) = args.description {
        if !description.is_empty() {
            tool_args.insert("description".to_string(), json!(description));
        }
    }
    if let Some(template_asset_key) = args.template_asset_key {
        if !template_asset_key.is_empty() {
            tool_args.insert("templateAssetKey".to_string(), json!(template_asset_key));
        }
    }

    let result = client.call_tool("mentor_create_asset", Value::Object(tool_args))?;
    output.print_result(&result)
}

pub async fn cmd_mentor_load_asset(args: MentorLoadAssetArgs, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("mentor-load-asset requires an asset key"));
    }
    let (client, output) = mentor_client(args.json, args.color)?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    tool_args.insert("assetKey".to_string(), json!(positionals[0]));
    if let Some(revision) = args.revision {
        tool_args.insert("revision".to_string(), json!(revision));
    }

    let result = client.call_tool("mentor_load_asset", Value::Object(tool_args))?;
    output.print_result(&result)
}

pub async fn cmd_mentor_prompt(args: MentorPromptArgs) -> Result<()> {
    let (client, output) = mentor_client(args.json, args.color)?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    tool_args.insert("message".to_string(), json!(args.message));
    if !args.attachment_refs.is_empty() {
        tool_args.insert("attachmentRefs".to_string(), json!(args.attachment_refs));
    }

    let result = client.call_tool("mentor_prompt", Value::Object(tool_args))?;
    output.print_result(&result)
}

pub async fn cmd_mentor_get_run(args: MentorGetRunArgs) -> Result<()> {
    let (client, output) = mentor_client(args.json, args.color)?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    tool_args.insert("runId".to_string(), json!(args.run_id));
    if let Some(cursor) = args.cursor {
        tool_args.insert("cursor".to_string(), json!(cursor));
    }

    let result = client.call_tool("mentor_get_run", Value::Object(tool_args))?;
    output.print_result(&result)
}

pub async fn cmd_mentor_get_event(args: MentorGetEventArgs) -> Result<()> {
    let (client, output) = mentor_client(args.json, args.color)?;

    let tool_args = json!({
        "sessionId": args.session_id,
        "runId": args.run_id,
        "eventId": args.event_id,
    });

    let result = client.call_tool("mentor_get_event", tool_args)?;
    output.print_result(&result)
}

pub async fn cmd_mentor_cancel_prompt(args: MentorSessionArgs) -> Result<()> {
    let (client, output) = mentor_client(args.json, args.color)?;

    let tool_args = json!({
        "sessionId": args.session_id,
        "runId": args.run_id.as_ref().unwrap_or(&String::new()),
    });

    let result = client.call_tool("mentor_cancel_prompt", tool_args)?;
    output.print_result(&result)
}

pub async fn cmd_mentor_close_session(args: MentorSessionArgs) -> Result<()> {
    let (client, output) = mentor_client(args.json, args.color)?;
    let tool_args = json!({ "sessionId": args.session_id });
    let result = client.call_tool("mentor_close_session", tool_args)?;
    output.print_result(&result)
}

pub async fn cmd_mentor_request_upload(args: MentorRequestUploadArgs) -> Result<()> {
    let (client, output) = mentor_client(args.json, args.color)?;

    let tool_args = json!({
        "sessionId": args.session_id,
        "fileName": args.file_name,
        "sizeBytes": args.size_bytes,
    });

    let result = client.call_tool("mentor_request_upload", tool_args)?;
    output.print_result(&result)
}

pub async fn cmd_mentor_publish(args: MentorPublishArgs) -> Result<()> {
    let (client, output) = mentor_client(args.json, args.color)?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    if let Some(comment) = args.comment {
        if !comment.is_empty() {
            tool_args.insert("comment".to_string(), json!(comment));
        }
    }

    let result = client.call_tool("mentor_publish", Value::Object(tool_args))?;
    output.print_result(&result)
}
