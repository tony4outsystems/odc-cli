//! Source code download and upload commands.

use super::args::*;
use super::context::Ctx;
use super::shared::resolve_asset;
use anyhow::Result;

pub async fn cmd_download_source_code(ctx: &Ctx, args: &DownloadSourceCodeArgs) -> Result<()> {
    let client = ctx.client()?;

    let asset = resolve_asset(&client, &args.asset)?;

    let revision = match args.revision {
        Some(revision) => revision,
        None => asset
            .get("revision")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("Asset {} has no revision field", args.asset))?
            as i32,
    };

    let output_arg = args.output.as_deref().unwrap_or("");
    let (output_path, _bytes) =
        crate::inspection::download_source_code(&client, &args.asset, revision, output_arg)?;

    if ctx.json() {
        ctx.output
            .print_result(&serde_json::json!({ "output": output_path }))?;
    } else {
        ctx.output
            .println_locked(&format!("Wrote source code to {}", output_path));
    }
    Ok(())
}

pub async fn cmd_upload_source_code(ctx: &Ctx, args: &UploadSourceCodeArgs) -> Result<()> {
    let client = ctx.client()?;

    let bytes = std::fs::read(&args.oml_file)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", args.oml_file, e))?;

    let upload_url = client.request_upload_url()?;
    client.upload_file_bytes(&upload_url, bytes)?;
    let created = client.create_asset_revision(&upload_url)?;

    ctx.output.print_result(&serde_json::Value::Object(created))
}
