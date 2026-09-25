//! Deployment operations: analysis, deploy, undeploy, delete.

use super::args::*;
use super::context::Ctx;
use super::shared::*;
use crate::client::Client;
use crate::value::JsonMapExt;
use anyhow::Result;
use serde_json::Map;
use std::time::Duration;

fn analysis_is_terminal(map: &Map<std::string::String, serde_json::Value>) -> bool {
    matches!(status_str(map, "processStatus"), "Finished" | "Failed")
}

fn build_is_terminal(map: &Map<std::string::String, serde_json::Value>) -> bool {
    matches!(
        status_str(map, "status"),
        "Finished" | "FinishedWithErrors" | "Deleted" | "ToBeDeleted"
    )
}

fn operation_is_terminal(map: &Map<std::string::String, serde_json::Value>) -> bool {
    matches!(status_str(map, "status"), "Finished" | "FinishedWithError")
}

/// Poll an already-started analysis (deployment or deletion — both use `processStatus` with the
/// same terminal/failure values) until it finishes, unless `poll.no_wait`. `label` (e.g.
/// `"deployment analysis"`) is used in the timeout and failure error messages; on `--no-wait`,
/// prints just `{"analysisKey": analysis_key}` instead of waiting.
async fn wait_for_analysis(
    ctx: &Ctx,
    label: &str,
    poll: &PollArgs,
    analysis_key: &str,
    fetch: impl Fn() -> Result<Map<String, serde_json::Value>>,
) -> Result<()> {
    if poll.no_wait {
        return ctx
            .output
            .print_result(&serde_json::json!({ "analysisKey": analysis_key }));
    }

    let result = crate::workflows::wait_for(
        label,
        fetch,
        analysis_is_terminal,
        Duration::from_secs(poll.poll_interval),
        Duration::from_secs(poll.timeout),
    )
    .await?;

    if status_str(&result, "processStatus") == "Failed" {
        return Err(anyhow::anyhow!(
            "{}{} failed: {}",
            label[..1].to_uppercase(),
            &label[1..],
            serde_json::Value::Object(result)
        ));
    }

    ctx.output.print_result(&serde_json::Value::Object(result))
}

pub async fn cmd_analyze_deployment(ctx: &Ctx, args: &AnalyzeDeploymentArgs) -> Result<()> {
    let client = ctx.client()?;

    let (asset, asset_key) = resolve_asset_key(&client, &args.asset)?;
    let env_key = resolve_env(&client, &args.env)?;
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    let started = client.start_deployment_analysis(&asset_key, revision, &env_key)?;
    let analysis_key = started
        .str_field("analysisKey")
        .ok_or_else(|| anyhow::anyhow!("Deployment analysis response has no analysisKey"))?
        .to_string();

    wait_for_analysis(
        ctx,
        "deployment analysis",
        &args.poll,
        &analysis_key,
        || client.get_deployment_analysis(&analysis_key),
    )
    .await
}

pub async fn cmd_analyze_deletion(ctx: &Ctx, args: &AnalyzeDeletionArgs) -> Result<()> {
    let client = ctx.client()?;

    let (_, asset_key) = resolve_asset_key(&client, &args.asset)?;

    let started = client.start_deletion_analysis(&asset_key)?;
    let analysis_key = started
        .str_field("analysisKey")
        .ok_or_else(|| anyhow::anyhow!("Deletion analysis response has no analysisKey"))?
        .to_string();

    wait_for_analysis(ctx, "deletion analysis", &args.poll, &analysis_key, || {
        client.get_deletion_analysis(&analysis_key)
    })
    .await
}

/// Start a build for `asset_key`/`revision` and, unless `poll.no_wait`, poll until it finishes.
/// Returns the build key and, when waited for, errors out on `FinishedWithErrors`.
pub async fn run_build(
    client: &Client,
    build_type: &str,
    poll: &PollArgs,
    asset_key: &str,
    revision: i32,
) -> Result<(String, Option<Map<String, serde_json::Value>>)> {
    let started = client.start_build(asset_key, revision, build_type)?;
    let build_key = started
        .str_field("buildKey")
        .ok_or_else(|| anyhow::anyhow!("Build response has no buildKey"))?
        .to_string();

    if poll.no_wait {
        return Ok((build_key, None));
    }

    let result = crate::workflows::wait_for(
        "build",
        || client.get_build(&build_key),
        build_is_terminal,
        Duration::from_secs(poll.poll_interval),
        Duration::from_secs(poll.timeout),
    )
    .await?;

    if status_str(&result, "status") == "FinishedWithErrors" {
        return Err(anyhow::anyhow!(
            "Build failed: {}",
            serde_json::Value::Object(result)
        ));
    }

    Ok((build_key, Some(result)))
}

pub async fn cmd_internal_build(ctx: &Ctx, args: &InternalBuildArgs) -> Result<()> {
    let client = ctx.client()?;

    let (asset, asset_key) = resolve_asset_key(&client, &args.asset)?;
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    let (build_key, result) =
        run_build(&client, &args.build_type, &args.poll, &asset_key, revision).await?;

    match result {
        Some(result) => ctx.output.print_result(&serde_json::Value::Object(result)),
        None => ctx
            .output
            .print_result(&serde_json::json!({ "buildKey": build_key })),
    }
}

pub async fn cmd_internal_publish(ctx: &Ctx, args: &InternalPublishArgs) -> Result<()> {
    let client = ctx.client()?;

    let (asset, asset_key) = resolve_asset_key(&client, &args.asset)?;
    let env_key = resolve_env(&client, &args.env)?;
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    let started = client.start_publish(&asset_key, revision, &env_key)?;
    let operation_key = started
        .str_field("key")
        .ok_or_else(|| anyhow::anyhow!("Publish response has no key"))?
        .to_string();

    if args.poll.no_wait {
        return ctx.output.print_result(&serde_json::Value::Object(started));
    }

    let result = crate::workflows::wait_for(
        "publish",
        || client.get_publish(&operation_key),
        operation_is_terminal,
        Duration::from_secs(args.poll.poll_interval),
        Duration::from_secs(args.poll.timeout),
    )
    .await?;

    if status_str(&result, "status") == "FinishedWithError" {
        return Err(anyhow::anyhow!(
            "Publish failed: {}",
            serde_json::Value::Object(result)
        ));
    }

    ctx.output.print_result(&serde_json::Value::Object(result))
}

/// Start a deployment operation (`Deploy`/`Undeploy`) and, unless `poll.no_wait`, poll until it
/// finishes, erroring out on `FinishedWithError`.
pub async fn run_deployment_operation(
    client: &Client,
    operation: &str,
    asset_key: &str,
    env_key: &str,
    revision: Option<i32>,
    build_key: Option<&str>,
    poll: &PollArgs,
) -> Result<(String, Option<Map<String, serde_json::Value>>)> {
    let started =
        client.start_deployment_operation(operation, asset_key, env_key, revision, build_key)?;
    let operation_key = started
        .str_field("key")
        .ok_or_else(|| anyhow::anyhow!("{} response has no key", operation))?
        .to_string();

    if poll.no_wait {
        return Ok((operation_key, None));
    }

    let result = crate::workflows::wait_for(
        operation,
        || client.get_deployment_operation(&operation_key),
        operation_is_terminal,
        Duration::from_secs(poll.poll_interval),
        Duration::from_secs(poll.timeout),
    )
    .await?;

    if status_str(&result, "status") == "FinishedWithError" {
        return Err(anyhow::anyhow!(
            "{} failed: {}",
            operation,
            serde_json::Value::Object(result)
        ));
    }

    Ok((operation_key, Some(result)))
}

pub async fn cmd_internal_deploy(ctx: &Ctx, args: &InternalDeployArgs) -> Result<()> {
    if args.build_key.is_empty() {
        return Err(anyhow::anyhow!("internal-deploy requires --build-key"));
    }

    let client = ctx.client()?;

    let (asset, asset_key) = resolve_asset_key(&client, &args.asset)?;
    let env_key = resolve_env(&client, &args.env)?;
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    let (operation_key, result) = run_deployment_operation(
        &client,
        "Deploy",
        &asset_key,
        &env_key,
        Some(revision),
        Some(&args.build_key),
        &args.poll,
    )
    .await?;

    match result {
        Some(result) => ctx.output.print_result(&serde_json::Value::Object(result)),
        None => ctx
            .output
            .print_result(&serde_json::json!({ "operationKey": operation_key })),
    }
}

pub async fn cmd_deploy(ctx: &Ctx, args: &DeployArgs) -> Result<()> {
    let client = ctx.client()?;

    let (asset, asset_key) = resolve_asset_key(&client, &args.asset)?;
    let env_key = resolve_env(&client, &args.env)?;
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    // --no-wait doesn't make sense for a multi-step composite command: we always need
    // the build to finish before we know it's safe to deploy it.
    if args.poll.no_wait {
        return Err(anyhow::anyhow!(
            "deploy does not support --no-wait; use internal-build/internal-deploy instead"
        ));
    }

    ctx.output
        .stderr(&format!("Building {} revision {}...", args.asset, revision));
    let always_wait = PollArgs {
        no_wait: false, // deploy always waits
        ..args.poll.clone()
    };
    let (build_key, _) = run_build(
        &client,
        &args.build_type,
        &always_wait,
        &asset_key,
        revision,
    )
    .await?;
    ctx.output
        .stderr(&format!("Build completed (buildKey: {})", build_key));

    ctx.output
        .stderr(&format!("Deploying {} to {}...", args.asset, args.env));
    let (_, deploy_result) = run_deployment_operation(
        &client,
        "Deploy",
        &asset_key,
        &env_key,
        Some(revision),
        Some(&build_key),
        &always_wait,
    )
    .await?;
    ctx.output.stderr("Deployment completed");

    ctx.output.print_result(&serde_json::Value::Object(
        deploy_result.unwrap_or_default(),
    ))
}

pub async fn cmd_undeploy(ctx: &Ctx, args: &UndeployArgs) -> Result<()> {
    let client = ctx.client()?;

    let (_, asset_key) = resolve_asset_key(&client, &args.asset)?;
    let env_key = resolve_env(&client, &args.env)?;

    let (operation_key, result) = run_deployment_operation(
        &client, "Undeploy", &asset_key, &env_key, None, None, &args.poll,
    )
    .await?;

    match result {
        Some(result) => ctx.output.print_result(&serde_json::Value::Object(result)),
        None => ctx
            .output
            .print_result(&serde_json::json!({ "operationKey": operation_key })),
    }
}

pub async fn cmd_delete_asset(ctx: &Ctx, args: &DeleteAssetArgs) -> Result<()> {
    let client = ctx.client()?;

    let (_, asset_key) = resolve_asset_key(&client, &args.asset)?;

    client.delete_asset(&asset_key)?;
    ctx.output
        .println_locked(&format!("Deleted asset {}", args.asset));
    Ok(())
}
