//! Revision management and producer graph commands.

use super::args::*;
use super::context::Ctx;
use super::shared::*;
use anyhow::Result;
use serde_json::Value;

pub async fn cmd_latest_revision(ctx: &Ctx, args: &LatestRevisionArgs) -> Result<()> {
    let client = ctx.client()?;

    let asset = resolve_asset(&client, &args.asset)?;
    let revision = asset
        .get("revision")
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no revision field", args.asset))?;
    ctx.output.print_result(revision)?;
    Ok(())
}

pub async fn cmd_list_revisions(ctx: &Ctx, args: &ListRevisionsArgs) -> Result<()> {
    let client = ctx.client()?;

    let (_, asset_key) = resolve_asset_key(&client, &args.asset)?;

    let offset = args.offset.unwrap_or(0);
    let (items, has_more) = client.list_revisions_page(&asset_key, offset, args.limit)?;

    let result = Listing {
        items,
        page: Some((offset, args.limit, has_more)),
    };

    print_listing(&ctx.output, result, REVISION_TABLE_COLUMNS)
}

pub async fn cmd_get_revision(ctx: &Ctx, args: &GetRevisionArgs) -> Result<()> {
    let client = ctx.client()?;

    let found = crate::inspection::get_revision(&client, &args.asset, args.revision)?;

    ctx.output.print_result(&serde_json::Value::Object(found))?;
    Ok(())
}

pub async fn cmd_producer_graph(ctx: &Ctx, args: &ProducerGraphArgs) -> Result<()> {
    let client = ctx.client()?;

    let (asset, asset_key) = resolve_asset_key(&client, &args.asset)?;

    let revision = match args.revision {
        Some(revision) => revision,
        None => asset
            .get("revision")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("Asset {} has no revision field", args.asset))?
            as i32,
    };

    let env_key = match args.env.as_deref() {
        Some(env) if !env.is_empty() => {
            let environments = client.list_environments()?;
            crate::resolve::resolve(env, "environment", &environments, "key")?
        }
        _ => String::new(),
    };

    let producer_type_filter = if args.all_producers {
        "All"
    } else {
        args.producer_type_filter.as_str()
    };

    let producers = client.get_producer_graph(
        &asset_key,
        revision,
        args.max_depth,
        producer_type_filter,
        &env_key,
    )?;

    let mut root = asset.clone();
    root.insert("revision".to_string(), Value::from(revision));

    let graph = crate::mermaid::render_producer_graph(&root, &producers);

    let output_path = match args.output.as_deref() {
        Some(output) if !output.is_empty() => output.to_string(),
        _ => crate::mermaid::default_mermaid_path(&asset_key, revision as i64),
    };
    std::fs::write(&output_path, &graph)?;

    if ctx.json() {
        ctx.output
            .print_result(&serde_json::json!({"output": output_path}))?;
    } else {
        ctx.output
            .println_locked(&format!("Wrote producer graph to {}", output_path));
    }
    Ok(())
}
