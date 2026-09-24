//! Environment listing commands.

use super::args::ListEnvironmentsArgs;
use anyhow::Result;
use serde_json::Value;

pub async fn cmd_list_environments(args: ListEnvironmentsArgs) -> Result<()> {
    let (output, client) = super::shared::make_client(args.json, args.color)?;

    let envs = client.list_environments()?;
    let result: Vec<_> = envs.into_iter().map(Value::Object).collect();
    output.print_result(&Value::Array(result))?;
    Ok(())
}
