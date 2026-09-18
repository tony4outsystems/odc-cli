//! Environment listing commands.

use crate::cli::Options;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

pub async fn cmd_list_environments(options: &Options) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());

    let envs = client.list_environments()?;
    let result: Vec<_> = envs.into_iter().map(Value::Object).collect();
    output.print_result(&Value::Array(result))?;
    Ok(())
}
