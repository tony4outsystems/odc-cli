//! Environment listing commands.

use super::args::ListEnvironmentsArgs;
use crate::output::Output;
use crate::client::Client;
use crate::settings;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

pub async fn cmd_list_environments(args: ListEnvironmentsArgs) -> Result<()> {
    let settings = settings::load_settings()?;
    let output = Arc::new(Output::new(args.json, args.color));
    let client = Client::new(settings, output.clone());

    let envs = client.list_environments()?;
    let result: Vec<_> = envs.into_iter().map(Value::Object).collect();
    output.print_result(&Value::Array(result))?;
    Ok(())
}
