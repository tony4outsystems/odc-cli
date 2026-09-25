# Development

This document covers building, testing, and releasing this repo. For installing and using the `odc` CLI, see [README.md](README.md).

During development, use `cargo run -- ...`. 

## Architecture

The Rust codebase is organized as follows:

| Module | Purpose |
|--------|---------|
| `main.rs` | Entry point and error handling (exit code 1 on failure) |
| `lib.rs` | Argument parsing and command dispatch |
| `cli.rs` | clap command definitions, categorized help text, and conversion to typed args |
| `commands/` | Command handlers, one module per domain (`assets`, `deployment`, `roles`, `users`, `mentor`, ...); `args.rs` holds the typed argument structs and `shared.rs` the common helpers (`make_client`, listing/pagination, asset/env resolution) |
| `client.rs` | HTTP client for ODC API with discovery, token caching, and pagination |
| `mentor.rs` | Client for the Mentor MCP endpoint |
| `transport.rs` | HTTP abstraction layer using `reqwest` with async/sync bridging, timeouts |
| `value.rs` | JSON helpers for number fidelity and value extraction |
| `output.rs` | Pretty-printing engine with color codes and table alignment |
| `settings.rs` | Configuration loading from the process environment (each variable falling back independently to `~/.odc/config.json`) |
| `login.rs` | Interactive `login` and config persistence |
| `resolve.rs` | Name-to-GUID resolution with partial matching and suggestions |
| `workflows.rs`, `inspection.rs`, `mermaid.rs` | Batch workflows, asset inspection, and dependency graphs |

## Testing

Run tests with `cargo test --lib`. Unit tests live next to the code in `#[cfg(test)]` modules. HTTP is mocked through the `Transport` trait (`testutil::test_transport` + `testutil::json_response`); the API client, Mentor client, and inspection logic are tested this way. Command handlers in `commands/` are not yet covered by tests.

CI (`.github/workflows/rust.yml`) runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --lib` on Linux, macOS, and Windows.

## Releasing

Push a version tag to build and publish binaries for Linux (x86_64), macOS (arm64), and Windows (x86_64):

```bash
git tag v0.1.2
git push origin v0.1.2
```

## TODO
