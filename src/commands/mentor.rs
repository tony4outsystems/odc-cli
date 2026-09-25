//! Mentor (AI) integration commands.

use super::args::*;
use super::context::Ctx;
use super::shared::resolve_asset;
use crate::mentor::MentorClient;
use crate::output::Output;
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::io::{self, Write};
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

/// Insert `field` into `args` as `value` if `value` is non-empty, matching the
/// `if let Some(x) = ... { if !x.is_empty() { ... } }` pattern repeated across the
/// `mentor-create-asset`/`mentor-publish` tool-arg builders.
fn insert_opt_nonempty(args: &mut Map<String, Value>, field: &str, value: &Option<String>) {
    if let Some(value) = value {
        if !value.is_empty() {
            args.insert(field.to_string(), json!(value));
        }
    }
}

/// Resolve `app_name` to an asset, start a Mentor session, and load the asset into it. Shared
/// by `cmd_mentor_single_shot` and `cmd_mentor_interactive` (steps 1-2 of the `mentor` flow).
/// Returns the session id and the asset's (key, name) for status messages.
async fn open_session_with_asset(
    ctx: &Ctx,
    mentor: &MentorClient,
    app_name: &str,
) -> Result<(String, String, String)> {
    let output = &ctx.output;

    let api_client = ctx.client()?;
    let asset = resolve_asset(&api_client, app_name)?;
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

    output.stderr("Starting Mentor session...");
    let session_result = mentor.call_tool("mentor_start_session", json!({}))?;
    let session_id = session_result
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get sessionId from mentor_start_session"))?
        .to_string();
    output.stderr(&format!("Session started: {}", session_id));

    output.stderr(&format!("Loading app: {} ({})", asset_name, asset_key));
    let mut load_args = Map::new();
    load_args.insert("sessionId".to_string(), json!(session_id.clone()));
    load_args.insert("assetKey".to_string(), json!(asset_key.clone()));
    let _load_result = mentor.call_tool("mentor_load_asset", Value::Object(load_args))?;
    output.stderr("App loaded successfully");

    Ok((session_id, asset_key, asset_name))
}

/// Send `message` to `session_id` and poll until the run completes. Shared by
/// `cmd_mentor_single_shot` and `cmd_mentor_interactive` (step 3 of the `mentor` flow).
/// Returns the final run result.
async fn send_prompt_and_wait(
    ctx: &Ctx,
    mentor: &MentorClient,
    session_id: &str,
    message: &str,
) -> Result<Value> {
    let output = &ctx.output;

    output.stderr("Sending prompt...");
    let mut prompt_args = Map::new();
    prompt_args.insert("sessionId".to_string(), json!(session_id));
    prompt_args.insert("message".to_string(), json!(message));
    let prompt_result = mentor.call_tool("mentor_prompt", Value::Object(prompt_args))?;
    let run_id = prompt_result
        .get("runId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Failed to get runId from mentor_prompt"))?
        .to_string();
    output.stderr(&format!("Prompt sent, run ID: {}", run_id));

    poll_mentor_run(mentor, session_id, &run_id, output).await
}

pub async fn cmd_mentor_start_session(ctx: &Ctx) -> Result<()> {
    let client = ctx.mentor()?;
    let result = client.call_tool("mentor_start_session", json!({}))?;
    ctx.output.print_result(&result)
}

pub async fn cmd_mentor_create_asset(ctx: &Ctx, args: &MentorCreateAssetArgs) -> Result<()> {
    let client = ctx.mentor()?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    tool_args.insert("assetType".to_string(), json!(args.asset_type.as_str()));
    tool_args.insert("name".to_string(), json!(args.name));
    tool_args.insert("portfolioKey".to_string(), json!(args.portfolio_key));
    insert_opt_nonempty(&mut tool_args, "description", &args.description);
    insert_opt_nonempty(&mut tool_args, "templateAssetKey", &args.template_asset_key);

    let result = client.call_tool("mentor_create_asset", Value::Object(tool_args))?;
    ctx.output.print_result(&result)
}

pub async fn cmd_mentor_load_asset(ctx: &Ctx, args: &MentorLoadAssetArgs) -> Result<()> {
    let client = ctx.mentor()?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    tool_args.insert("assetKey".to_string(), json!(args.asset_key));
    if let Some(revision) = args.revision {
        tool_args.insert("revision".to_string(), json!(revision));
    }

    let result = client.call_tool("mentor_load_asset", Value::Object(tool_args))?;
    ctx.output.print_result(&result)
}

pub async fn cmd_mentor(ctx: &Ctx, args: &MentorArgs) -> Result<()> {
    match &args.prompt {
        Some(prompt) => cmd_mentor_single_shot(ctx, &args.app_name, prompt).await,
        None => cmd_mentor_interactive(ctx, &args.app_name).await,
    }
}

async fn cmd_mentor_single_shot(ctx: &Ctx, app_name: &str, prompt: &str) -> Result<()> {
    let output = &ctx.output;
    let mentor = ctx.mentor()?;

    // 1-2. Resolve the app, start a session, and load it in
    let (session_id, _asset_key, _asset_name) =
        open_session_with_asset(ctx, &mentor, app_name).await?;

    // 3-4. Send the prompt and poll until completion
    let final_run_result = send_prompt_and_wait(ctx, &mentor, &session_id, prompt).await?;

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

/// Helper function to poll a mentor run until completion, handling events and text streaming.
/// Returns the final run result.
async fn poll_mentor_run(
    mentor: &MentorClient,
    session_id: &str,
    run_id: &str,
    output: &Arc<Output>,
) -> Result<Value> {
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
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        poll_count += 1;

        let mut run_args = Map::new();
        run_args.insert("sessionId".to_string(), json!(session_id));
        run_args.insert("runId".to_string(), json!(run_id));
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
        return Err(anyhow::anyhow!("Timeout waiting for prompt completion"));
    }

    if run_failed {
        return Err(anyhow::anyhow!("Mentor prompt run failed"));
    }

    Ok(final_run_result)
}

/// Interactive prompt selector using termimad.
/// Returns true if "Publish" was selected, false if "Continue" was selected.
fn select_publish_option() -> Result<bool> {
    let skin = termimad::MadSkin::default();

    loop {
        let choice = termimad::ask!(&skin, "Would you like to publish the changes?", ('n') {
            ('y', "**Y**es, publish the changes") => {
                1u8
            }
            ('n', "**N**o, continue without publishing") => {
                0u8
            }
            ('d', "**D**iff - Open the diff in ODC Studio") => {
                2u8
            }
        });

        match choice {
            1 => return Ok(true),
            0 => return Ok(false),
            _ => {
                termimad::print_text("*This feature is not implemented!* 🚀\n“We are all in the gutter, but some of us are looking at the stars.” — Oscar Wilde\n");
            }
        }
    }
}

async fn cmd_mentor_interactive(ctx: &Ctx, app_name: &str) -> Result<()> {
    let output = &ctx.output;
    let mentor = ctx.mentor()?;

    // 1-2. Resolve the app, start a session, and load it in
    let (session_id, _asset_key, _asset_name) =
        open_session_with_asset(ctx, &mentor, app_name).await?;
    output.stderr("Entering interactive mode. Type your prompts below (Enter to send, Shift+Enter for new line).");
    output.stderr("Press Ctrl+D to exit.\n");

    // 3. Interactive loop
    loop {
        // Read multi-line input
        eprint!("Ask Mentor: ");
        io::stderr().flush()?;

        let mut input = String::new();
        // Read lines until we get a complete input (for now, just read one line at a time)
        // TODO: Support Shift+Enter for multi-line later if needed
        match io::stdin().read_line(&mut input) {
            Ok(0) => {
                // EOF (Ctrl+D)
                output.stderr("\nExiting interactive mode...");
                break;
            }
            Ok(_) => {
                let input = input.trim();
                if input.is_empty() {
                    continue;
                }

                // Send the prompt and poll until completion
                let final_run_result =
                    send_prompt_and_wait(ctx, &mentor, &session_id, input).await?;

                // Check if changes were applied
                let change_applied = final_run_result
                    .get("result")
                    .and_then(|r| r.get("changeApplied"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if change_applied {
                    match select_publish_option() {
                        Ok(true) => {
                            output.stderr("Publishing...");
                            let mut publish_args = Map::new();
                            publish_args.insert("sessionId".to_string(), json!(session_id.clone()));
                            let _publish_result =
                                mentor.call_tool("mentor_publish", Value::Object(publish_args))?;
                            output.stderr("Asset published successfully.\n");
                        }
                        Ok(false) => {
                            output.stderr("Continuing without publishing.\n");
                        }
                        Err(e) => {
                            output.stderr(&format!("Error during publish prompt: {}\n", e));
                        }
                    }
                }
            }
            Err(e) => {
                output.stderr(&format!("Error reading input: {}", e));
                break;
            }
        }
    }

    // 4. Close the session
    output.stderr("Closing session...");
    let close_args = json!({ "sessionId": session_id });
    let _close_result = mentor.call_tool("mentor_close_session", close_args)?;
    output.stderr("Session closed");

    Ok(())
}

pub async fn cmd_mentor_prompt(ctx: &Ctx, args: &MentorPromptArgs) -> Result<()> {
    let client = ctx.mentor()?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    tool_args.insert("message".to_string(), json!(args.message));
    if !args.attachment_refs.is_empty() {
        tool_args.insert("attachmentRefs".to_string(), json!(args.attachment_refs));
    }

    let result = client.call_tool("mentor_prompt", Value::Object(tool_args))?;
    ctx.output.print_result(&result)
}

pub async fn cmd_mentor_get_run(ctx: &Ctx, args: &MentorGetRunArgs) -> Result<()> {
    let client = ctx.mentor()?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    tool_args.insert("runId".to_string(), json!(args.run_id));
    if let Some(cursor) = args.cursor {
        tool_args.insert("cursor".to_string(), json!(cursor));
    }

    let result = client.call_tool("mentor_get_run", Value::Object(tool_args))?;
    ctx.output.print_result(&result)
}

pub async fn cmd_mentor_get_event(ctx: &Ctx, args: &MentorGetEventArgs) -> Result<()> {
    let client = ctx.mentor()?;

    let tool_args = json!({
        "sessionId": args.session_id,
        "runId": args.run_id,
        "eventId": args.event_id,
    });

    let result = client.call_tool("mentor_get_event", tool_args)?;
    ctx.output.print_result(&result)
}

pub async fn cmd_mentor_cancel_prompt(ctx: &Ctx, args: &MentorCancelPromptArgs) -> Result<()> {
    let client = ctx.mentor()?;

    let tool_args = json!({
        "sessionId": args.session_id,
        "runId": args.run_id,
    });

    let result = client.call_tool("mentor_cancel_prompt", tool_args)?;
    ctx.output.print_result(&result)
}

pub async fn cmd_mentor_close_session(ctx: &Ctx, args: &MentorCloseSessionArgs) -> Result<()> {
    let client = ctx.mentor()?;
    let tool_args = json!({ "sessionId": args.session_id });
    let result = client.call_tool("mentor_close_session", tool_args)?;
    ctx.output.print_result(&result)
}

pub async fn cmd_mentor_request_upload(ctx: &Ctx, args: &MentorRequestUploadArgs) -> Result<()> {
    let client = ctx.mentor()?;

    let tool_args = json!({
        "sessionId": args.session_id,
        "fileName": args.file_name,
        "sizeBytes": args.size_bytes,
    });

    let result = client.call_tool("mentor_request_upload", tool_args)?;
    ctx.output.print_result(&result)
}

pub async fn cmd_mentor_publish(ctx: &Ctx, args: &MentorPublishArgs) -> Result<()> {
    let client = ctx.mentor()?;

    let mut tool_args = Map::new();
    tool_args.insert("sessionId".to_string(), json!(args.session_id));
    insert_opt_nonempty(&mut tool_args, "comment", &args.comment);

    let result = client.call_tool("mentor_publish", Value::Object(tool_args))?;
    ctx.output.print_result(&result)
}
