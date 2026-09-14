pub mod cli;
pub mod client;
pub mod commands;
pub mod inspection;
pub mod login;
pub mod mermaid;
pub mod output;
pub mod resolve;
pub mod settings;
pub mod testutil;
pub mod transport;
pub mod value;
pub mod workflows;

use anyhow::Result;
use std::sync::Arc;

/// Main entry point for the CLI
pub async fn run(args: &[String]) -> Result<()> {
    // Parse arguments
    if args.is_empty() {
        return Err(anyhow::anyhow!(
            "A command is required; use odc --help for usage"
        ));
    }

    // Handle special commands that don't need auth
    if args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
        print_help();
        return Ok(());
    }

    if args[0] == "completion" {
        // TODO: Implement shell completion
        return Err(anyhow::anyhow!("completion: not yet implemented"));
    }

    // Parse color and JSON flags
    let mut json = false;
    let mut color_str = "auto";
    let mut offset: Option<i64> = None;
    let mut limit: i64 = 100;
    let mut cmd_args = Vec::new();
    let mut skip_next = false;

    for (i, arg) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }

        match arg.as_str() {
            "--json" => json = true,
            "--color" => {
                if i + 1 < args.len() {
                    color_str = &args[i + 1];
                    skip_next = true;
                }
            }
            "--offset" => {
                if i + 1 < args.len() {
                    offset =
                        Some(args[i + 1].parse().map_err(|_| {
                            anyhow::anyhow!("--offset must be a non-negative integer")
                        })?);
                    skip_next = true;
                }
            }
            "--limit" => {
                if i + 1 < args.len() {
                    limit = args[i + 1]
                        .parse()
                        .map_err(|_| anyhow::anyhow!("--limit must be a positive integer"))?;
                    skip_next = true;
                }
            }
            _ => cmd_args.push(arg.clone()),
        }
    }

    // Parse color mode
    let color = cli::parse_color(color_str)?;

    // Create output context
    let _output = Arc::new(output::Output::new(json, color));

    // Get the command name
    if cmd_args.is_empty() {
        return Err(anyhow::anyhow!(
            "A command is required; use odc --help for usage"
        ));
    }

    let cmd = &cmd_args[0];

    // `odc <command> --help` / `-h`: show that command's usage instead of running it.
    if cmd_args[1..].iter().any(|a| a == "--help" || a == "-h") {
        return print_command_help(cmd);
    }

    // Handle login specially (doesn't need auth)
    if cmd == "login" {
        if cmd_args.len() != 3 {
            return Err(anyhow::anyhow!("login requires <tenant-url> <client-id>"));
        }
        return login::login(&cmd_args[1], &cmd_args[2]);
    }

    // All other commands: dispatch to command handler
    let positionals = if cmd_args.len() > 1 {
        cmd_args[1..].to_vec()
    } else {
        Vec::new()
    };

    let options = cli::Options {
        json,
        color,
        offset,
        limit,
        ..Default::default()
    };

    // Dispatch to command executor
    commands::execute(cmd, &options, &positionals).await
}

/// Print usage for a single command in response to `odc <command> --help`/`-h`.
fn print_command_help(cmd: &str) -> Result<()> {
    let def = cli::find_command(cmd).ok_or_else(|| anyhow::anyhow!("Unknown command: {}", cmd))?;

    if def.positional.is_empty() {
        println!("Usage: odc {}", def.name);
    } else {
        println!("Usage: odc {} {}", def.name, def.positional);
    }
    println!();
    println!("{}", def.short);

    if matches!(
        def.name,
        "list-apps" | "list-deployed-apps" | "list-revisions"
    ) {
        println!();
        println!("Flags:");
        println!(
            "  --offset <n>            Fetch a single page starting at this result index (default: fetch every page)"
        );
        println!("  --limit <n>             Page size to request from the API (default: 100)");
    }

    println!();
    println!("Global flags: --json, --color auto|always|never");

    Ok(())
}

fn print_help() {
    println!("Usage: odc <command> [flags]");
    println!();
    println!("Global Flags:");
    println!("  --json            Output in JSON format");
    println!("  --color auto|always|never");
    println!("                    Control color output (default: auto)");
    println!();
    println!("Commands:");

    let groups_seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut current_group = "";

    for cmd in cli::COMMANDS {
        if cmd.group != current_group {
            if !groups_seen.is_empty() {
                println!();
            }
            println!("  {}:", cmd.group.to_uppercase());
            current_group = cmd.group;
        }
        println!("    {:<30} {}", cmd.name, cmd.short);
    }

    println!();
    println!("Run 'odc <command> --help' for command-specific flags.");
}
