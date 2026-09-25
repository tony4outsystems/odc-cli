//! Environment listing commands.

use super::context::Ctx;
use anyhow::Result;
use serde_json::Value;

pub async fn cmd_list_environments(ctx: &Ctx) -> Result<()> {
    let client = ctx.client()?;

    let envs = client.list_environments()?;
    let result: Vec<_> = envs.into_iter().map(Value::Object).collect();
    ctx.output.print_result(&Value::Array(result))?;
    Ok(())
}
