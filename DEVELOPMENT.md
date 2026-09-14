# Development

This document covers building, testing, and releasing this repo. For installing and using the `odc` CLI, see [README.md](README.md).

During development, use `cargo run -- ...`. 

## Architecture

The Rust codebase is organized into 12 modules:

| Module | Purpose |
|--------|---------|
| `main.rs` | Entry point and error handling (exit code 1 on failure) |
| `lib.rs` | Argument parsing and command dispatch |
| `cli.rs` | Command table (30 commands in 6 help groups), option flags, and help text |
| `commands.rs` | Command handlers for all 30 commands |
| `client.rs` | HTTP client for ODC API with discovery, token caching, and pagination |
| `transport.rs` | HTTP abstraction layer using `reqwest` with async/sync bridging |
| `value.rs` | JSON helpers for number fidelity and value extraction |
| `output.rs` | Pretty-printing engine with color codes and table alignment |
| `settings.rs` | Configuration loading from `.env` with variable expansion |
| `login.rs` | OAuth2 client credentials flow and config persistence |
| `resolve.rs` | Name-to-GUID resolution with partial matching and suggestions |
| `workflows.rs`, `inspection.rs`, `mermaid.rs`, `upload.rs` | Command-specific logic and helpers |

## Testing

The codebase includes 42 unit tests covering:
- JSON value parsing and formatting
- Output formatting and field ordering
- Settings loading and `.env` expansion
- OAuth2 discovery and token flow
- App/environment resolution and API pagination
- Mermaid diagram generation

Run tests with `cargo test`. All tests use the `Transport` trait for mocking HTTP responses, allowing end-to-end command testing without network calls.

## Releasing

Push a version tag to build and publish binaries for Windows, macOS, and Linux, each for `amd64` and `arm64`:

```bash
git tag v0.1.2
git push origin v0.1.2
```

The `Release binaries` workflow runs tests and clippy checks, then uses cargo-dist (configured in `Cargo.toml.dist`) to build all six archives and publish a GitHub release with checksums and generated release notes. Tags containing a hyphen (for example, `v0.2.0-rc.1`) produce prereleases. Use a new version tag for each release, after the release configuration has been committed and pushed. Pushing `main` alone does not publish a release or add assets to existing releases. Skill releases use separate `skill-*` tags and do not trigger binary releases.

To validate the release locally without publishing (requires [cargo-dist](https://github.com/axo-dev/cargo-dist)):

```bash
cargo dist plan
cargo dist build --artifacts=local
```

Artifacts are written to `target/distrib/`.

## TODO

* Support Portfolio
