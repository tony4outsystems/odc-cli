pub mod cli;
pub mod client;
pub mod commands;  // Now a module directory with submodules organized by domain
pub mod error;
pub mod inspection;
pub mod login;
pub mod mentor;
pub mod mermaid;
pub mod output;
pub mod resolve;
pub mod settings;
pub mod testutil;
pub mod transport;
pub mod value;
pub mod workflows;

use anyhow::Result;
use clap::{CommandFactory, Parser};
use cli::{Cli, Commands};

/// Main entry point for the CLI
pub async fn run(args: &[String]) -> Result<()> {
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        cli::print_categorized_help();
        return Ok(());
    }

    let cli =
        match Cli::try_parse_from(std::iter::once("odc".to_string()).chain(args.iter().cloned())) {
            Ok(cli) => cli,
            // clap formats --help/--version/usage errors for us, routes them to the right
            // stream, and exits with the right code (0 for help/version, 2 for a real error).
            Err(err) => err.exit(),
        };

    if let Commands::Completion { shell } = cli.command {
        let mut app = Cli::command();
        clap_complete::generate(shell, &mut app, "odc", &mut std::io::stdout());
        return Ok(());
    }

    let cmd = cli.command.name();
    let (mut options, positionals) = cli.command.into_dispatch();
    options.json = cli.json;
    options.color = cli.color;

    // Handle login specially (doesn't need auth)
    if cmd == "login" {
        return login::login(&positionals[0], &positionals[1]);
    }

    commands::execute(cmd, &options, &positionals).await
}
