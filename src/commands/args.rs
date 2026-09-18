//! Strongly-typed command arguments.
//!
//! This module defines argument structs for commands, replacing the generic `Options` bag.
//! Each command or command group has its own struct with only the fields it actually uses.
//!
//! This provides:
//! - Type safety: no silent defaults, no ambiguous field names
//! - Clarity: handlers document exactly what they need
//! - Maintainability: easier to add new fields without cluttering `Options`
//!
//! Extraction happens in `cli.rs` via `Commands::as_*_args()` methods.
//!
//! # Migration Pattern
//!
//! Over time, command handlers will transition from `(options: &Options, positionals: &[String])`
//! to `(args: TypedArgs)` or `(args: TypedArgs, positionals: &[String])`.
//! This can be done incrementally, command by command.

use crate::cli::AppType;
use crate::output::ColorMode;

/// Arguments for commands that list assets.
///
/// Shared by `list-assets` and similar list commands.
#[derive(Debug, Clone)]
pub struct ListAssetsArgs {
    pub json: bool,
    pub color: ColorMode,
    pub filter: Option<String>,
    pub asset_type: Option<AppType>,
    pub offset: Option<i64>,
    pub limit: i64,
}

/// Arguments for commands that list deployed assets.
#[derive(Debug, Clone)]
pub struct ListDeployedAssetsArgs {
    pub json: bool,
    pub color: ColorMode,
    pub filter: Option<String>,
    pub env: Option<String>,
    pub offset: Option<i64>,
    pub limit: i64,
}

/// Arguments for commands that fetch a single asset.
///
/// Shared by `get-asset`, `get-revision`, etc.
#[derive(Debug, Clone)]
pub struct GetAssetArgs {
    pub json: bool,
    pub color: ColorMode,
}

/// Arguments for commands that list environments.
#[derive(Debug, Clone)]
pub struct ListEnvironmentsArgs {
    pub json: bool,
    pub color: ColorMode,
}
