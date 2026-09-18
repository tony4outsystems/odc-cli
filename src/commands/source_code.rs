//! Source code download and upload commands.

use super::args::*;
use super::shared::resolve_asset;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use std::sync::Arc;

pub async fn cmd_download_source_code(
    args: DownloadSourceCodeArgs,
    positionals: &[String],
) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "download-source-code requires an app name or key"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let app_key = &positionals[0];
    let asset = resolve_asset(&client, app_key)?;

    let revision = match args.revision {
        Some(revision) => revision,
        None => asset
            .get("revision")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("Asset {} has no revision field", app_key))?
            as i32,
    };

    let (output_path, _bytes) =
        crate::inspection::download_source_code(&client, app_key, revision, &args.output)?;

    if args.json {
        output.print_result(&serde_json::json!({ "output": output_path }))?;
    } else {
        output.println_locked(&format!("Wrote source code to {}", output_path));
    }
    Ok(())
}

pub async fn cmd_upload_source_code(
    args: UploadSourceCodeArgs,
    positionals: &[String],
) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!(
            "upload-source-code requires an OML/XIF file"
        ));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let bytes = std::fs::read(&positionals[0])
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", positionals[0], e))?;

    let upload_url = client.request_upload_url()?;
    client.upload_file_bytes(&upload_url, bytes)?;
    let created = client.create_asset_revision(&upload_url)?;

    output.print_result(&serde_json::Value::Object(created))
}
