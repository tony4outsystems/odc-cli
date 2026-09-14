pub mod cli;
pub mod commands;
pub mod client;
pub mod transport;
pub mod value;
pub mod output;
pub mod settings;
pub mod login;
pub mod inspection;
pub mod resolve;
pub mod upload;
pub mod mermaid;
pub mod workflows;
pub mod testutil;

pub fn run(_args: &[String]) -> anyhow::Result<()> {
    Err(anyhow::anyhow!("not implemented"))
}
