//! Mentor (AI) integration commands.

use super::args::*;
use super::shared::resolve_asset;
use crate::client::Client;
use crate::mentor::MentorClient;
use crate::output::Output;
use crate::settings;
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::sync::Arc;

/// Flush accumulated streaming text to stderr, rendering it as markdown via termimad.
/// Clears the buffer after printing.
fn flush_text_buf(buf: &mut String) {
    let text = buf.trim().to_string();
    if !text.is_empty() && text != "null" {
        termimad::print_text(&text);
    }
    buf.clear();
}

fn mentor_client(
    json: bool,
    color: crate::output::ColorMode,
) -> Result<(MentorClient, Arc<Output>)> {
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

pub async fn cmd_mentor_load_asset(
    args: MentorLoadAssetArgs,
    positionals: &[String],
) -> Result<()> {
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
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));

    // Resolve the app name/key to an actual asset (shows "did you mean" on mismatch)
    let api_client = Client::new(settings.clone(), output.clone());
    let asset = resolve_asset(&api_client, &args.app_name)?;
    let asset_key = asset
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset has no assetKey"))?
        .to_string();
    let asset_name = asset
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(&asset_key)
        .to_string();

    let mentor = MentorClient::new(settings);

    // 1. Start a session
    output.stderr("Starting Mentor session...");
    let session_result = mentor.call_tool("mentor_start_session", json!({}))?;
    let session_id = session_result
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get sessionId from mentor_start_session"))?
        .to_string();
    output.stderr(&format!("Session started: {}", session_id));

    // 2. Load the asset into the session
    output.stderr(&format!("Loading app: {} ({})", asset_name, asset_key));
    let mut load_args = Map::new();
    load_args.insert("sessionId".to_string(), json!(session_id.clone()));
    load_args.insert("assetKey".to_string(), json!(asset_key.clone()));
    let _load_result = mentor.call_tool("mentor_load_asset", Value::Object(load_args))?;
    output.stderr("App loaded successfully");

    // 3. Send the prompt
    output.stderr("Sending prompt...");
    let mut prompt_args = Map::new();
    prompt_args.insert("sessionId".to_string(), json!(session_id.clone()));
    prompt_args.insert("message".to_string(), json!(args.prompt));
    let prompt_result = mentor.call_tool("mentor_prompt", Value::Object(prompt_args))?;
    let run_id = prompt_result
        .get("runId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get runId from mentor_prompt"))?
        .to_string();
    output.stderr(&format!("Prompt sent, run ID: {}", run_id));

    // 4. Poll until completion
    output.stderr("Waiting for prompt to complete (this may take a few minutes)...");
    let mut poll_cursor: Option<i64> = None;
    let mut run_finished = false;
    let mut run_failed = false;
    let max_polls = 30; // 5 minutes with 10-second interval
    let mut poll_count = 0;
    // Buffer for streaming "text" chunks — flushed when a non-text event arrives or polling ends.
    let mut text_buf = String::new();
    // Track the final run result to check for changes
    let mut final_run_result = json!({});

    while !run_finished && poll_count < max_polls {
        std::thread::sleep(std::time::Duration::from_secs(10));
        poll_count += 1;

        let mut run_args = Map::new();
        run_args.insert("sessionId".to_string(), json!(session_id.clone()));
        run_args.insert("runId".to_string(), json!(run_id.clone()));
        if let Some(cursor) = poll_cursor {
            run_args.insert("cursor".to_string(), json!(cursor));
        }

        let run_result = mentor.call_tool("mentor_get_run", Value::Object(run_args))?;
        final_run_result = run_result.clone();

        // Check top-level status field
        if let Some(status) = run_result.get("status").and_then(|v| v.as_str()) {
            match status {
                "succeeded" | "completed" => run_finished = true,
                "failed" => {
                    run_finished = true;
                    run_failed = true;
                }
                _ => {}
            }
        }

        // Display events
        if let Some(events) = run_result.get("events").and_then(|v| v.as_array()) {
            for event in events {
                let event_obj = if let Some(s) = event.as_str() {
                    serde_json::from_str::<serde_json::Value>(s).ok()
                } else {
                    Some(event.clone())
                };

                if let Some(obj) = event_obj {
                    let msg_type = obj
                        .get("MsgType")
                        .or_else(|| obj.get("msgType"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    match msg_type {
                        "text" => {
                            // Accumulate streaming text chunks into a single buffer.
                            if let Some(chunk) = obj.get("text").and_then(|v| v.as_str()) {
                                text_buf.push_str(chunk);
                            }
                        }
                        "conversationInfoUpdated" => {
                            // Flush accumulated text before printing a status line.
                            flush_text_buf(&mut text_buf);
                            if let Some(title) = obj.get("title").and_then(|v| v.as_str()) {
                                if !title.is_empty() && title != "null" {
                                    output.stderr(&format!("  → {}", title));
                                }
                            }
                        }
                        "reasoning" => {
                            flush_text_buf(&mut text_buf);
                            let label = obj
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Thinking...");
                            output.stderr(&format!("  → {}", label));
                        }
                        _ => {}
                    }

                    if let Some(status) = obj.get("status").and_then(|v| v.as_str()) {
                        match status {
                            "completed" | "succeeded" => run_finished = true,
                            "failed" => {
                                run_finished = true;
                                run_failed = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if let Some(cursor_val) = run_result.get("cursor").and_then(|v| v.as_i64()) {
            poll_cursor = Some(cursor_val);
        }
    }

    // Flush any remaining buffered text after polling completes.
    flush_text_buf(&mut text_buf);

    if !run_finished {
        // Close session before returning error
        let _ = mentor.call_tool("mentor_close_session", json!({ "sessionId": session_id }));
        return Err(anyhow::anyhow!("Timeout waiting for prompt completion"));
    }

    if run_failed {
        let _ = mentor.call_tool("mentor_close_session", json!({ "sessionId": session_id }));
        return Err(anyhow::anyhow!("Mentor prompt run failed"));
    }

    output.stderr("Prompt completed");

    // 5. Check if changes were actually applied before publishing
    let change_applied = final_run_result
        .get("result")
        .and_then(|r| r.get("changeApplied"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if change_applied {
        output.stderr("Changes detected, publishing...");
        // Auto-publish
        let mut publish_args = Map::new();
        publish_args.insert("sessionId".to_string(), json!(session_id.clone()));
        let publish_result = mentor.call_tool("mentor_publish", Value::Object(publish_args))?;
        output.stderr("Asset published successfully");
        output.print_result(&publish_result)?;
    } else {
        output.stderr("No changes applied, skipping publish");
        output.print_result(&json!({"status": "completed", "changeApplied": false}))?;
    }

    // 6. Close the session
    output.stderr("Closing session...");
    let close_args = json!({ "sessionId": session_id });
    let _close_result = mentor.call_tool("mentor_close_session", close_args)?;
    output.stderr("Session closed");

    Ok(())
}

pub async fn cmd_mentor_prompt_raw(args: MentorPromptRawArgs) -> Result<()> {
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
