//! Deployment operations: analysis, deploy, undeploy, delete.

use super::shared::*;
use crate::cli::Options;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use serde_json::Map;
use std::sync::Arc;

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

pub async fn cmd_analyze_deployment(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let app = resolve_app(&client, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;
    let revision = resolve_revision(&client, &app, &asset_key, options.revision)?;

    let started = client.start_deployment_analysis(&asset_key, revision, &env_key)?;
    let analysis_key = started
        .get("analysisKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Deployment analysis response has no analysisKey"))?
        .to_string();

    if options.no_wait {
        return output.print_result(&serde_json::json!({ "analysisKey": analysis_key }));
    }

    let result = crate::workflows::wait_for(
        "deployment analysis",
        || client.get_deployment_analysis(&analysis_key),
        analysis_is_terminal,
        options.interval,
        options.timeout,
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

pub async fn cmd_analyze_deletion(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let asset_key = resolve_app(&client, &options.app)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();

    let started = client.start_deletion_analysis(&asset_key)?;
    let analysis_key = started
        .get("analysisKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Deletion analysis response has no analysisKey"))?
        .to_string();

    if options.no_wait {
        return output.print_result(&serde_json::json!({ "analysisKey": analysis_key }));
    }

    let result = crate::workflows::wait_for(
        "deletion analysis",
        || client.get_deletion_analysis(&analysis_key),
        analysis_is_terminal,
        options.interval,
        options.timeout,
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

/// Start a build for `asset_key`/`revision` and, unless `--no-wait`, poll until it finishes.
/// Returns the build key and, when waited for, errors out on `FinishedWithErrors`.
pub async fn run_build(
    client: &Client,
    options: &Options,
    asset_key: &str,
    revision: i32,
) -> Result<(String, Option<Map<String, serde_json::Value>>)> {
    let started = client.start_build(asset_key, revision, &options.build_type)?;
    let build_key = started
        .get("buildKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Build response has no buildKey"))?
        .to_string();

    if options.no_wait {
        return Ok((build_key, None));
    }

    let result = crate::workflows::wait_for(
        "build",
        || client.get_build(&build_key),
        build_is_terminal,
        options.interval,
        options.timeout,
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

pub async fn cmd_internal_build(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let app = resolve_app(&client, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let revision = resolve_revision(&client, &app, &asset_key, options.revision)?;

    let (build_key, result) = run_build(&client, options, &asset_key, revision).await?;

    match result {
        Some(result) => output.print_result(&serde_json::Value::Object(result)),
        None => output.print_result(&serde_json::json!({ "buildKey": build_key })),
    }
}

pub async fn cmd_internal_publish(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let app = resolve_app(&client, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;
    let revision = resolve_revision(&client, &app, &asset_key, options.revision)?;

    let started = client.start_publish(&asset_key, revision, &env_key)?;
    let operation_key = started
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Publish response has no key"))?
        .to_string();

    if options.no_wait {
        return output.print_result(&serde_json::Value::Object(started));
    }

    let result = crate::workflows::wait_for(
        "publish",
        || client.get_publish(&operation_key),
        operation_is_terminal,
        options.interval,
        options.timeout,
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

/// Start a deployment operation (`Deploy`/`Undeploy`) and, unless `--no-wait`, poll until it
/// finishes, erroring out on `FinishedWithError`.
pub async fn run_deployment_operation(
    client: &Client,
    options: &Options,
    operation: &str,
    asset_key: &str,
    env_key: &str,
    revision: Option<i32>,
    build_key: Option<&str>,
) -> Result<(String, Option<Map<String, serde_json::Value>>)> {
    let started =
        client.start_deployment_operation(operation, asset_key, env_key, revision, build_key)?;
    let operation_key = started
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("{} response has no key", operation))?
        .to_string();

    if options.no_wait {
        return Ok((operation_key, None));
    }

    let result = crate::workflows::wait_for(
        operation,
        || client.get_deployment_operation(&operation_key),
        operation_is_terminal,
        options.interval,
        options.timeout,
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

pub async fn cmd_internal_deploy(options: &Options) -> Result<()> {
    if options.build_key.is_empty() {
        return Err(anyhow::anyhow!("internal-deploy requires --build-key"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let app = resolve_app(&client, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;
    let revision = resolve_revision(&client, &app, &asset_key, options.revision)?;

    let (operation_key, result) = run_deployment_operation(
        &client,
        options,
        "Deploy",
        &asset_key,
        &env_key,
        Some(revision),
        Some(&options.build_key),
    )
    .await?;

    match result {
        Some(result) => output.print_result(&serde_json::Value::Object(result)),
        None => output.print_result(&serde_json::json!({ "operationKey": operation_key })),
    }
}

pub async fn cmd_deploy(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let app = resolve_app(&client, &options.app)?;
    let asset_key = app
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;
    let revision = resolve_revision(&client, &app, &asset_key, options.revision)?;

    if options.no_wait {
        // --no-wait doesn't make sense for a multi-step composite command: we always need
        // the build to finish before we know it's safe to deploy it.
        return Err(anyhow::anyhow!(
            "deploy does not support --no-wait; use internal-build/internal-deploy instead"
        ));
    }

    let (build_key, _) = run_build(&client, options, &asset_key, revision).await?;

    let (_, deploy_result) = run_deployment_operation(
        &client,
        options,
        "Deploy",
        &asset_key,
        &env_key,
        Some(revision),
        Some(&build_key),
    )
    .await?;

    output.print_result(&serde_json::Value::Object(
        deploy_result.unwrap_or_default(),
    ))
}

pub async fn cmd_undeploy(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let asset_key = resolve_app(&client, &options.app)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();
    let env_key = resolve_env(&client, &options.env)?;

    let (operation_key, result) = run_deployment_operation(
        &client, options, "Undeploy", &asset_key, &env_key, None, None,
    )
    .await?;

    match result {
        Some(result) => output.print_result(&serde_json::Value::Object(result)),
        None => output.print_result(&serde_json::json!({ "operationKey": operation_key })),
    }
}

pub async fn cmd_delete_app(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let asset_key = resolve_app(&client, &options.app)?
        .get("assetKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("App {} has no assetKey field", options.app))?
        .to_string();

    client.delete_asset(&asset_key)?;
    output.println_locked(&format!("Deleted app {}", options.app));
    Ok(())
}
