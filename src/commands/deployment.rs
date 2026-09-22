//! Deployment operations: analysis, deploy, undeploy, delete.

use super::args::*;
use super::shared::*;
use crate::client::Client;
use crate::output::Output;
use crate::settings;
use anyhow::Result;
use serde_json::Map;
use std::sync::Arc;
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

pub async fn cmd_analyze_deployment(args: DeploymentAnalysisArgs) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let asset = resolve_asset(&client, &args.asset)?;
    let asset_key = asset
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", args.asset))?
        .to_string();
    let env_key = resolve_env(&client, &args.env)?;
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    let started = client.start_deployment_analysis(&asset_key, revision, &env_key)?;
    let analysis_key = started
        .get("analysisKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Deployment analysis response has no analysisKey"))?
        .to_string();

    if args.no_wait {
        return output.print_result(&serde_json::json!({ "analysisKey": analysis_key }));
    }

    let result = crate::workflows::wait_for(
        "deployment analysis",
        || client.get_deployment_analysis(&analysis_key),
        analysis_is_terminal,
        Duration::from_secs(args.poll_interval),
        Duration::from_secs(args.timeout),
    )
    .await?;

    if status_str(&result, "processStatus") == "Failed" {
        return Err(anyhow::anyhow!(
            "Deployment analysis failed: {}",
            serde_json::Value::Object(result)
        ));
    }

    output.print_result(&serde_json::Value::Object(result))
}

pub async fn cmd_analyze_deletion(args: DeletionAnalysisArgs) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let asset_key = resolve_asset(&client, &args.asset)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", args.asset))?
        .to_string();

    let started = client.start_deletion_analysis(&asset_key)?;
    let analysis_key = started
        .get("analysisKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Deletion analysis response has no analysisKey"))?
        .to_string();

    if args.no_wait {
        return output.print_result(&serde_json::json!({ "analysisKey": analysis_key }));
    }

    let result = crate::workflows::wait_for(
        "deletion analysis",
        || client.get_deletion_analysis(&analysis_key),
        analysis_is_terminal,
        Duration::from_secs(args.poll_interval),
        Duration::from_secs(args.timeout),
    )
    .await?;

    if status_str(&result, "processStatus") == "Failed" {
        return Err(anyhow::anyhow!(
            "Deletion analysis failed: {}",
            serde_json::Value::Object(result)
        ));
    }

    output.print_result(&serde_json::Value::Object(result))
}

/// Start a build for `asset_key`/`revision` and, unless `no_wait`, poll until it finishes.
/// Returns the build key and, when waited for, errors out on `FinishedWithErrors`.
pub async fn run_build(
    client: &Client,
    build_type: &str,
    poll_interval: u64,
    timeout: u64,
    no_wait: bool,
    asset_key: &str,
    revision: i32,
) -> Result<(String, Option<Map<String, serde_json::Value>>)> {
    let started = client.start_build(asset_key, revision, build_type)?;
    let build_key = started
        .get("buildKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Build response has no buildKey"))?
        .to_string();

    if no_wait {
        return Ok((build_key, None));
    }

    let result = crate::workflows::wait_for(
        "build",
        || client.get_build(&build_key),
        build_is_terminal,
        Duration::from_secs(poll_interval),
        Duration::from_secs(timeout),
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

pub async fn cmd_internal_build(args: BuildArgs) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let asset = resolve_asset(&client, &args.asset)?;
    let asset_key = asset
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", args.asset))?
        .to_string();
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    let (build_key, result) = run_build(
        &client,
        &args.build_type,
        args.poll_interval,
        args.timeout,
        args.no_wait,
        &asset_key,
        revision,
    )
    .await?;

    match result {
        Some(result) => output.print_result(&serde_json::Value::Object(result)),
        None => output.print_result(&serde_json::json!({ "buildKey": build_key })),
    }
}

pub async fn cmd_internal_publish(args: PublishArgs) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let asset = resolve_asset(&client, &args.asset)?;
    let asset_key = asset
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", args.asset))?
        .to_string();
    let env_key = resolve_env(&client, &args.env)?;
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    let started = client.start_publish(&asset_key, revision, &env_key)?;
    let operation_key = started
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Publish response has no key"))?
        .to_string();

    if args.no_wait {
        return output.print_result(&serde_json::Value::Object(started));
    }

    let result = crate::workflows::wait_for(
        "publish",
        || client.get_publish(&operation_key),
        operation_is_terminal,
        Duration::from_secs(args.poll_interval),
        Duration::from_secs(args.timeout),
    )
    .await?;

    if status_str(&result, "status") == "FinishedWithError" {
        return Err(anyhow::anyhow!(
            "Publish failed: {}",
            serde_json::Value::Object(result)
        ));
    }

    output.print_result(&serde_json::Value::Object(result))
}

/// Start a deployment operation (`Deploy`/`Undeploy`) and, unless `no_wait`, poll until it
/// finishes, erroring out on `FinishedWithError`.
#[allow(clippy::too_many_arguments)]
pub async fn run_deployment_operation(
    client: &Client,
    operation: &str,
    asset_key: &str,
    env_key: &str,
    revision: Option<i32>,
    build_key: Option<&str>,
    poll_interval: u64,
    timeout: u64,
    no_wait: bool,
) -> Result<(String, Option<Map<String, serde_json::Value>>)> {
    let started =
        client.start_deployment_operation(operation, asset_key, env_key, revision, build_key)?;
    let operation_key = started
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{} response has no key", operation))?
        .to_string();

    if no_wait {
        return Ok((operation_key, None));
    }

    let result = crate::workflows::wait_for(
        operation,
        || client.get_deployment_operation(&operation_key),
        operation_is_terminal,
        Duration::from_secs(poll_interval),
        Duration::from_secs(timeout),
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

pub async fn cmd_internal_deploy(args: InternalDeployArgs) -> Result<()> {
    if args.build_key.is_empty() {
        return Err(anyhow::anyhow!("internal-deploy requires --build-key"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let asset = resolve_asset(&client, &args.asset)?;
    let asset_key = asset
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", args.asset))?
        .to_string();
    let env_key = resolve_env(&client, &args.env)?;
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    let (operation_key, result) = run_deployment_operation(
        &client,
        "Deploy",
        &asset_key,
        &env_key,
        Some(revision),
        Some(&args.build_key),
        args.poll_interval,
        args.timeout,
        args.no_wait,
    )
    .await?;

    match result {
        Some(result) => output.print_result(&serde_json::Value::Object(result)),
        None => output.print_result(&serde_json::json!({ "operationKey": operation_key })),
    }
}

pub async fn cmd_deploy(args: DeploymentOperationArgs) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let asset = resolve_asset(&client, &args.asset)?;
    let asset_key = asset
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", args.asset))?
        .to_string();
    let env_key = resolve_env(&client, &args.env)?;
    let revision = resolve_revision(&client, &asset, &asset_key, args.revision)?;

    // --no-wait doesn't make sense for a multi-step composite command: we always need
    // the build to finish before we know it's safe to deploy it.
    if args.no_wait {
        return Err(anyhow::anyhow!(
            "deploy does not support --no-wait; use internal-build/internal-deploy instead"
        ));
    }

    output.stderr(&format!(
        "Building {} revision {}...",
        args.asset, revision
    ));
    let (build_key, _) = run_build(
        &client,
        &args.build_type,
        args.poll_interval,
        args.timeout,
        false, // deploy always waits
        &asset_key,
        revision,
    )
    .await?;
    output.stderr(&format!("Build completed (buildKey: {})", build_key));

    output.stderr(&format!(
        "Deploying {} to {}...",
        args.asset, args.env
    ));
    let (_, deploy_result) = run_deployment_operation(
        &client,
        "Deploy",
        &asset_key,
        &env_key,
        Some(revision),
        Some(&build_key),
        args.poll_interval,
        args.timeout,
        false, // deploy always waits
    )
    .await?;
    output.stderr("Deployment completed");

    output.print_result(&serde_json::Value::Object(
        deploy_result.unwrap_or_default(),
    ))
}

pub async fn cmd_undeploy(args: DeploymentOperationArgs) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let asset_key = resolve_asset(&client, &args.asset)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", args.asset))?
        .to_string();
    let env_key = resolve_env(&client, &args.env)?;

    let (operation_key, result) = run_deployment_operation(
        &client,
        "Undeploy",
        &asset_key,
        &env_key,
        None,
        None,
        args.poll_interval,
        args.timeout,
        args.no_wait,
    )
    .await?;

    match result {
        Some(result) => output.print_result(&serde_json::Value::Object(result)),
        None => output.print_result(&serde_json::json!({ "operationKey": operation_key })),
    }
}

pub async fn cmd_delete_asset(args: DeleteAssetArgs) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let asset_key = resolve_asset(&client, &args.asset)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Asset {} has no assetKey field", args.asset))?
        .to_string();

    client.delete_asset(&asset_key)?;
    output.println_locked(&format!("Deleted asset {}", args.asset));
    Ok(())
}
