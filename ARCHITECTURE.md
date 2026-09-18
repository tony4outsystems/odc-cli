# ODC CLI Architecture Guide

This document describes the architectural patterns and conventions used in the ODC CLI project.

---

## Error Handling

### Pattern: `thiserror` Derive Macro

All CLI errors use the `thiserror` crate for clean, maintainable error definitions.

**Location:** [`src/error.rs`](src/error.rs)

**Benefits:**
- Automatic `Display` and `Error` trait implementations
- Type-safe error conversions with `#[from]`
- Error messages defined inline with variants
- No manual boilerplate

**Example:**
```rust
use thiserror::Error;
use std::io;

#[derive(Debug, Error)]
pub enum OdcError {
    #[error("API error from {endpoint}: {status} {body}")]
    ApiError {
        status: u16,
        body: String,
        endpoint: String,
    },
    
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
    
    #[error(transparent)]
    Io(#[from] io::Error),
    
    #[error("Invalid URL: {0}")]
    UrlParse(#[from] url::ParseError),
}

// Usage: Automatically converts io::Error to OdcError
fn read_file(path: &str) -> Result<String, OdcError> {
    let content = std::fs::read_to_string(path)?; // io::Error converted automatically
    Ok(content)
}
```

**Adding a New Error:**
1. Add a new variant to `OdcError` enum
2. Use `#[error("...")]` attribute with the message pattern
3. If it wraps another error type, use `#[from]` to auto-convert
4. Done—no impl blocks needed

---

## Enum String Conversions

### Pattern: `strum` Display Derive

Enums that need string representations use `strum` for automatic `Display` and conversion methods.

**Location:** [`src/cli.rs`](src/cli.rs) — see `AppType`, `MentorAssetType`, `AssigneeType`

**Benefits:**
- No manual match blocks duplicating strings
- Automatic `Display` impl for easy `to_string()` calls
- Single source of truth for enum values

**Example:**
```rust
use strum_macros::Display;
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum, Display)]
pub enum AppType {
    #[value(name = "WebApplication")]
    #[strum(to_string = "WebApplication")]
    WebApplication,
    
    #[value(name = "Agent")]
    #[strum(to_string = "Agent")]
    Agent,
}

impl AppType {
    /// Get the exact API string for this type (always matches #[strum(to_string = ...)])
    pub fn as_str(&self) -> &'static str {
        match self {
            AppType::WebApplication => "WebApplication",
            AppType::Agent => "Agent",
        }
    }
}

// Usage:
let app_type = AppType::WebApplication;
println!("{}", app_type);           // "WebApplication" (via Display)
println!("{}", app_type.as_str());  // "WebApplication"
```

**Adding a New Enum Variant:**
1. Add the variant with both `#[value(name = "...")]` and `#[strum(to_string = "...")]`
2. Add matching arm to `as_str()` method
3. Both attributes must have identical strings—compiler catches mismatches

---

## Client Initialization

### Pattern: `make_client()` Helper

All command handlers initialize a client the same way. Use the `make_client()` helper function.

**Location:** [`src/commands/shared.rs`](src/commands/shared.rs)

**Benefits:**
- Eliminates repetitive 3-line setup in every command
- Single point of control for initialization logic
- Easy to extend (add logging, caching, etc. in one place)

**Example:**
```rust
use crate::commands::shared::make_client;
use crate::cli::Options;

pub async fn cmd_list_assets(options: &Options, positionals: &[String]) -> Result<()> {
    let (output, client) = make_client(options)?;
    // ... rest of command
}
```

**Implementation:**
```rust
pub fn make_client(options: &Options) -> Result<(Arc<Output>, Client)> {
    let settings = crate::settings::load_settings()?;
    let output = Arc::new(Output::new(options.json, options.color));
    let client = Client::new(settings, output.clone());
    Ok((output, client))
}
```

**Why it matters:**
- If you forget this pattern, you duplicate setup logic
- Future changes (e.g., telemetry) only need to happen in one place
- Tests can mock this function instead of recreating settings/client

---

## Command Dispatch

### Pattern: Typed Enum Matching

Commands are dispatched using direct enum pattern matching, not string conversion.

**Location:** [`src/lib.rs`](src/lib.rs) (entry point) and [`src/commands/mod.rs`](src/commands/mod.rs) (dispatch)

**Benefits:**
- Type-safe: compiler ensures all variants are handled
- No string conversion bugs (no typos like "lsit-assets")
- Clear what each command requires

**Example:**
```rust
pub async fn execute(command: &Commands, options: &Options, positionals: &[String]) -> Result<()> {
    use Commands::*;
    
    match command {
        ListAssets { .. } => {
            let args = command.as_list_assets_args(options.json, options.color);
            assets::cmd_list_assets(args, positionals).await
        }
        Deploy { .. } => {
            // ... deployment handling
        }
        _ => Err(anyhow!("Unknown command")),
    }
}
```

**Why not strings?**
- ❌ Old way: `if cmd == "list-assets"` → typos cause silent failures
- ✅ New way: `Commands::ListAssets { .. }` → compiler error if misspelled

---

## Typed Command Arguments

### Pattern: Command-Specific Argument Structs

Instead of passing a generic `Options` bag with 30 fields (most unused), commands use minimal, typed argument structs.

**Location:** [`src/commands/args.rs`](src/commands/args.rs)

**Status:** Partially implemented (POC for Assets commands)

**Benefits:**
- Type safety: no silent defaults or wrong field access
- Clarity: handlers document exactly what they need
- Maintainability: no ambiguous field names used for multiple purposes
- Testability: easier to construct test data

### Current Implementation

**Assets commands use typed args:**

```rust
#[derive(Debug, Clone)]
pub struct ListAssetsArgs {
    pub json: bool,
    pub color: ColorMode,
    pub filter: Option<String>,
    pub asset_type: Option<AppType>,
    pub offset: Option<i64>,
    pub limit: i64,
}

// Handler is clear about its dependencies:
pub async fn cmd_list_assets(args: ListAssetsArgs, positionals: &[String]) -> Result<()> {
    // All 6 fields are directly used, no "empty defaults" confusion
}
```

**Other commands still use generic `Options`:**

```rust
// Old pattern (still used by most commands)
pub async fn cmd_deploy(options: &Options, positionals: &[String]) -> Result<()> {
    // options has 30 fields; only 8 are relevant
    // Silent bugs possible: accessing unused fields
}
```

### Extraction Pattern

Commands are extracted from the enum into typed args via helper methods:

```rust
impl Commands {
    pub fn as_list_assets_args(&self, json: bool, color: ColorMode) -> ListAssetsArgs {
        match self {
            Commands::ListAssets { filter, app_type, offset, limit } => {
                ListAssetsArgs {
                    json,
                    color,
                    filter: filter.clone(),
                    asset_type: *app_type,
                    offset: *offset,
                    limit: *limit,
                }
            }
            _ => panic!("Expected ListAssets command"),
        }
    }
}
```

### Migration Path (Future Work)

To gradually migrate other commands to typed args:

1. **Define arg struct** in `commands/args.rs`:
   ```rust
   #[derive(Debug, Clone)]
   pub struct DeployArgs {
       pub json: bool,
       pub color: ColorMode,
       pub asset: String,
       pub env: String,
       pub revision: Option<i32>,
       pub build_type: String,
       pub poll_interval: Duration,
       pub timeout: Duration,
       pub no_wait: bool,
   }
   ```

2. **Add extraction method** in `Commands::as_deploy_args()`:
   ```rust
   pub fn as_deploy_args(&self, json: bool, color: ColorMode) -> DeployArgs {
       match self {
           Commands::Deploy { asset, env, revision, build_type, poll } => {
               DeployArgs {
                   json,
                   color,
                   asset: asset.clone(),
                   env: env.clone(),
                   revision: *revision,
                   build_type: build_type.clone(),
                   poll_interval: Duration::from_secs(poll.poll_interval),
                   timeout: Duration::from_secs(poll.timeout),
                   no_wait: poll.no_wait,
               }
           }
           _ => panic!("Expected Deploy command"),
       }
   }
   ```

3. **Update dispatch** in `commands::execute()`:
   ```rust
   Deploy { .. } => {
       let args = command.as_deploy_args(options.json, options.color);
       deployment::cmd_deploy(args).await
   }
   ```

4. **Refactor handler**:
   ```rust
   pub async fn cmd_deploy(args: DeployArgs) -> Result<()> {
       let (output, client) = make_client(/*...*/)?;
       // Use args.asset, args.env, args.revision, etc.
       // All fields are relevant, clear intent
   }
   ```

**Priority order for future migration:**
1. Deployment commands (heavy use of Options fields)
2. Mentor commands (many session_id fields)
3. Batch operations (parallel/poll args)
4. Everything else incrementally

---

## Pagination Helpers

### Pattern: `fetch_listing()` and `print_listing()`

Commands that support pagination use helpers in [`src/commands/shared.rs`](src/commands/shared.rs).

**`fetch_listing(options, page_fn, all_fn)`**
- Fetches a single page if `options.offset` is set
- Fetches all pages if `options.offset` is None
- Returns a `Listing` struct with items and pagination metadata

**`print_listing(output, listing, table_columns)`**
- Outputs JSON (full results) or table (filtered columns)
- Includes pagination metadata in JSON mode (next offset)

**Example:**
```rust
let mut listing = fetch_listing(
    &options,
    |offset, limit| client.list_assets_page(offset, limit),
    || client.list_assets(),
)?;

// Apply filters
listing.items = filter_by_substring(
    listing.items,
    positionals.first().map(String::as_str),
    &["name", "assetKey"],
);

// Output
print_listing(&output, listing, ASSET_TABLE_COLUMNS)
```

---

## Resolution Helpers

### Pattern: Identifier Resolution

Commands that need to resolve user-supplied identifiers (asset names, environment names, etc.) use helpers in [`src/resolve.rs`](src/resolve.rs).

**`resolve(input, kind, items, key_field)`**
- Supports GUID pass-through (short-circuit)
- Matches exact name first (case-insensitive)
- Falls back to substring matching (lenient mode)
- Returns suggestions on ambiguity

**Example:**
```rust
let env_key = crate::resolve::resolve(
    &options.env,          // User's input: "prod" or "550e8400-..."
    "environment",         // Kind (for error messages)
    &environments,         // List of all environments from API
    "key"                  // Field to return (the key to use in next call)
)?;
```

**Why it matters:**
- Users can type "prod" instead of GUID
- Typos get helpful suggestions: "Did you mean: production?"
- Compiler verifies all resolution sites use same logic

---

## Testing

### Pattern: Structured Test Organization

Tests live in the same module as the code they test, organized by concern.

**Examples:**
- `src/error.rs` — error display tests
- `src/cli.rs` — CLI parsing tests
- `src/commands/mod.rs` — table column tests
- `src/resolve.rs` — resolution logic tests

**Running tests:**
```bash
cargo test                 # All tests
cargo test resolve::       # Just resolve module
cargo test test_guid       # Just that test
cargo test -- --nocapture # Show println! output
```

---

## Code Organization

### Directory Structure

```
src/
├── main.rs                    # Entry point (minimal)
├── lib.rs                     # Public API + run()
├── cli.rs                     # clap command definitions + Options
├── error.rs                   # Error types (thiserror)
├── client.rs                  # HTTP client (reqwest)
├── settings.rs                # Config file handling
├── output.rs                  # JSON/table formatting
├── resolve.rs                 # Identifier resolution
├── transport.rs               # HTTP abstraction
├── workflows.rs               # Batch operations
├── commands/
│   ├── mod.rs                 # Dispatch + shared tests
│   ├── args.rs                # Typed argument structs (POC)
│   ├── shared.rs              # Helpers: make_client, fetch_listing, etc.
│   ├── assets.rs              # Asset commands
│   ├── auth.rs                # Auth & portfolio commands
│   ├── deployment.rs          # Deploy operations
│   ├── environments.rs        # Environment commands
│   ├── revisions.rs           # Revision commands
│   ├── roles.rs               # Role management
│   ├── users.rs               # User & group management
│   ├── mentor.rs              # AI mentor commands
│   └── source_code.rs         # Upload/download operations
└── ...
```

### Module Naming

- **Commands:** Named after what they operate on (assets, revisions, users)
- **Shared:** `src/commands/shared.rs` for utilities used by multiple command modules
- **Args:** `src/commands/args.rs` for typed argument structs (new pattern)

---

## Adding a New Command

### Checklist

1. **Define in `src/cli.rs`:**
   - Add variant to `Commands` enum with `#[command(...)]` attributes
   - Add to appropriate `HELP_CATEGORIES`

2. **Create typed args** (if following new pattern):
   - Add struct to `src/commands/args.rs`
   - Add extraction method to `Commands` impl

3. **Implement handler:**
   - Create or update relevant module (`src/commands/deploy.rs`, etc.)
   - Use `make_client()` helper
   - Use `fetch_listing()` if paginated
   - Use `resolve()` if handling identifiers

4. **Add dispatch:**
   - Update `src/commands/mod.rs` execute() match statement

5. **Test:**
   - Add unit tests in the command module
   - Run `cargo test`
   - Manual test: `cargo run -- <command> --help`

---

## Dependencies

### Why Each Dependency

| Crate | Purpose | Why We Use It |
|-------|---------|---------------|
| `clap` | CLI parsing | Industry standard, strong derive support |
| `reqwest` | HTTP client | Async, modern, maintains good backwards compat |
| `tokio` | Async runtime | Reqwest requires it, Rust async standard |
| `serde_json` | JSON handling | Standard for Rust JSON, good error messages |
| `thiserror` | Error types | Eliminates error boilerplate (this project) |
| `strum` | Enum utils | Eliminates string conversion boilerplate (this project) |
| `dirs` | Config paths | Cross-platform home directory handling |
| `anyhow` | Error handling | General error propagation (being phased out for `thiserror`) |
| `tempfile` | Temp files | Cross-platform temp file handling |
| `tabwriter` | Table formatting | Clean ASCII table output |
| `regex` | Pattern matching | GUID validation |

---

## Performance Considerations

### Caching Strategy

**Token caching:**
- OAuth2 tokens cached per Client instance
- No automatic TTL refresh—tokens cached until expiration or Client dropped

**Discovery caching:**
- OIDC discovery document cached per Client instance
- Lifetime is per command invocation (no cross-command cache)

**Assets caching:**
- Full asset list cached for single command execution
- Useful for batch operations that need multiple lookups

### Network Calls

Each command makes the minimum necessary network calls:
- `list-assets` → 1 call to list all (or paginate)
- `list-deployed-assets` → 1 call to list environments + 1 to list deployed assets
- `get-asset` → 1 call (lookup by key)
- `deploy` → 1 call (start) + N polls (check status)

---

## Future Improvements

### Roadmap

- [ ] **Complete typed args migration** (Priorities: Deployment → Mentor → Batch)
- [ ] **Remove `into_dispatch()`** entirely (once all commands migrated)
- [ ] **Add structured logging** (replace eprintln! with tracing)
- [ ] **Add config file validation** (schema, examples)
- [ ] **Improve error messages** (suggest common typos)
- [ ] **Add shell completions** (via clap_complete)

### Known Limitations

- No automatic token refresh (tokens expire mid-session)
- No concurrent command execution (single CLI invocation per process)
- No interactive mode (everything is stateless one-shot commands)

---

## Questions?

Refer to:
- **Patterns:** This file
- **Error handling:** `src/error.rs`
- **CLI structure:** `src/cli.rs`
- **Command dispatch:** `src/commands/mod.rs`
- **POC typed args:** `src/commands/args.rs` and `src/commands/assets.rs`

