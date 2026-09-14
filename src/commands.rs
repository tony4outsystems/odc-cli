use crate::cli::Options;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use std::sync::Arc;
use serde_json::json;

/// Execute a command based on its name
pub async fn execute(cmd: &str, options: &Options, positionals: &[String]) -> Result<()> {
    match cmd {
        "login" => Err(anyhow::anyhow!(
            "Login should be handled in main run() function"
        )),
        "discover" => cmd_discover(options).await,
        "list-environments" => cmd_list_environments(options).await,
        "list-apps" => cmd_list_apps(options, positionals).await,
        "list-deployed-apps" => cmd_list_deployed_apps(options, positionals).await,
        "get-app" => cmd_get_app(options, positionals).await,
        "latest-revision" => cmd_latest_revision(options, positionals).await,
        "list-revisions" => cmd_list_revisions(options, positionals).await,
        "get-revision" => cmd_get_revision(options, positionals).await,
        "producer-graph" => Err(anyhow::anyhow!("producer-graph: not yet implemented")),
        "download-source-code" => Err(anyhow::anyhow!("download-source-code: not yet implemented")),
        "upload-source-code" => Err(anyhow::anyhow!("upload-source-code: not yet implemented")),
        "validate" => Err(anyhow::anyhow!("validate: not yet implemented")),
        "analyze-deployment" => Err(anyhow::anyhow!("analyze-deployment: not yet implemented")),
        "analyze-deletion" => Err(anyhow::anyhow!("analyze-deletion: not yet implemented")),
        "deploy" => Err(anyhow::anyhow!("deploy: not yet implemented")),
        "undeploy" => Err(anyhow::anyhow!("undeploy: not yet implemented")),
        "delete-app" => Err(anyhow::anyhow!("delete-app: not yet implemented")),
        "batch-deploy" => Err(anyhow::anyhow!("batch-deploy: not yet implemented")),
        "batch-undeploy" => Err(anyhow::anyhow!("batch-undeploy: not yet implemented")),
        "batch-delete" => Err(anyhow::anyhow!("batch-delete: not yet implemented")),
        "dangerous-batch-undeploy-all" => Err(anyhow::anyhow!(
            "dangerous-batch-undeploy-all: not yet implemented"
        )),
        "get-user" => Err(anyhow::anyhow!("get-user: not yet implemented")),
        "update-user" => Err(anyhow::anyhow!("update-user: not yet implemented")),
        "grant-role" => Err(anyhow::anyhow!("grant-role: not yet implemented")),
        "revoke-role" => Err(anyhow::anyhow!("revoke-role: not yet implemented")),
        "internal-build" => Err(anyhow::anyhow!("internal-build: not yet implemented")),
        "internal-publish" => Err(anyhow::anyhow!("internal-publish: not yet implemented")),
        "internal-deploy" => Err(anyhow::anyhow!("internal-deploy: not yet implemented")),
        _ => Err(anyhow::anyhow!("Unknown command: {}", cmd)),
    }
}

async fn cmd_discover(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let discovery = client.discover()?;
    output.print_result(&serde_json::Value::Object(discovery))?;
    Ok(())
}

async fn cmd_list_environments(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let envs = client.list_environments()?;
    let result: Vec<_> = envs.into_iter().map(serde_json::Value::Object).collect();
    output.print_result(&serde_json::Value::Array(result))?;
    Ok(())
}

async fn cmd_list_apps(options: &Options, _positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let result: Vec<_> = apps.into_iter().map(serde_json::Value::Object).collect();
    output.print_result(&serde_json::Value::Array(result))?;
    Ok(())
}

async fn cmd_list_deployed_apps(options: &Options, _positionals: &[String]) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let result: Vec<_> = apps.into_iter().map(serde_json::Value::Object).collect();
    output.print_result(&serde_json::Value::Array(result))?;
    Ok(())
}

async fn cmd_get_app(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("get-app requires an app name or key"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];

    for app in apps {
        if let Some(serde_json::Value::String(key)) = app.get("key") {
            if key == app_key {
                output.print_result(&serde_json::Value::Object(app))?;
                return Ok(());
            }
        }
    }

    Err(anyhow::anyhow!("App not found: {}", app_key))
}

async fn cmd_latest_revision(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("latest-revision requires an app name or key"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];

    for app in apps {
        if let Some(serde_json::Value::String(key)) = app.get("key") {
            if key == app_key {
                if let Some(revision) = app.get("revision") {
                    output.print_result(revision)?;
                    return Ok(());
                }
            }
        }
    }

    Err(anyhow::anyhow!("App not found: {}", app_key))
}

async fn cmd_list_revisions(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.is_empty() {
        return Err(anyhow::anyhow!("list-revisions requires an app name or key"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];

    for app in apps {
        if let Some(serde_json::Value::String(key)) = app.get("key") {
            if key == app_key {
                if let Some(serde_json::Value::Array(revisions)) = app.get("revisions") {
                    output.print_result(&serde_json::Value::Array(revisions.clone()))?;
                    return Ok(());
                }
            }
        }
    }

    Err(anyhow::anyhow!("App not found: {}", app_key))
}

async fn cmd_get_revision(options: &Options, positionals: &[String]) -> Result<()> {
    if positionals.len() < 2 {
        return Err(anyhow::anyhow!("get-revision requires <app> --revision <number>"));
    }

    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let apps = client.list_apps()?;
    let app_key = &positionals[0];

    for app in apps {
        if let Some(serde_json::Value::String(key)) = app.get("key") {
            if key == app_key {
                output.print_result(&serde_json::Value::Object(app))?;
                return Ok(());
            }
        }
    }

    Err(anyhow::anyhow!("App not found: {}", app_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unknown_command() {
        let opts = Options::default();
        let result = execute("nonexistent", &opts, &[]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }
}
