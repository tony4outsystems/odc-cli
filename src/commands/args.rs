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

// ============================================================================
// DEPLOYMENT COMMANDS
// ============================================================================

/// Arguments for deployment analysis commands.
#[derive(Debug, Clone)]
pub struct DeploymentAnalysisArgs {
    pub json: bool,
    pub color: ColorMode,
    pub asset: String,
    pub env: String,
    pub revision: Option<i32>,
    pub poll_interval: u64,
    pub timeout: u64,
    pub no_wait: bool,
}

/// Arguments for deletion analysis command.
#[derive(Debug, Clone)]
pub struct DeletionAnalysisArgs {
    pub json: bool,
    pub color: ColorMode,
    pub asset: String,
    pub poll_interval: u64,
    pub timeout: u64,
    pub no_wait: bool,
}

/// Arguments for build commands (internal-build).
#[derive(Debug, Clone)]
pub struct BuildArgs {
    pub json: bool,
    pub color: ColorMode,
    pub asset: String,
    pub env: String,
    pub revision: Option<i32>,
    pub build_type: String,
    pub poll_interval: u64,
    pub timeout: u64,
    pub no_wait: bool,
}

/// Arguments for publish commands (internal-publish).
#[derive(Debug, Clone)]
pub struct PublishArgs {
    pub json: bool,
    pub color: ColorMode,
    pub asset: String,
    pub env: String,
    pub revision: Option<i32>,
    pub poll_interval: u64,
    pub timeout: u64,
    pub no_wait: bool,
}

/// Arguments for internal-deploy command.
#[derive(Debug, Clone)]
pub struct InternalDeployArgs {
    pub json: bool,
    pub color: ColorMode,
    pub asset: String,
    pub env: String,
    pub revision: Option<i32>,
    pub build_key: String,
    pub poll_interval: u64,
    pub timeout: u64,
    pub no_wait: bool,
}

/// Arguments for deploy/undeploy commands.
#[derive(Debug, Clone)]
pub struct DeploymentOperationArgs {
    pub json: bool,
    pub color: ColorMode,
    pub asset: String,
    pub env: String,
    pub revision: Option<i32>,
    pub build_type: String,
    pub poll_interval: u64,
    pub timeout: u64,
    pub no_wait: bool,
}

/// Arguments for delete-asset command.
#[derive(Debug, Clone)]
pub struct DeleteAssetArgs {
    pub json: bool,
    pub color: ColorMode,
    pub asset: String,
}

// ============================================================================
// MENTOR COMMANDS
// ============================================================================

/// Arguments for mentor-create-asset command.
#[derive(Debug, Clone)]
pub struct MentorCreateAssetArgs {
    pub json: bool,
    pub color: ColorMode,
    pub session_id: String,
    pub asset_type: String,
    pub name: String,
    pub portfolio_key: String,
    pub description: Option<String>,
    pub template_asset_key: Option<String>,
}

/// Arguments for mentor-load-asset command.
#[derive(Debug, Clone)]
pub struct MentorLoadAssetArgs {
    pub json: bool,
    pub color: ColorMode,
    pub session_id: String,
    pub revision: Option<i32>,
}

/// Arguments for mentor-prompt command.
#[derive(Debug, Clone)]
pub struct MentorPromptArgs {
    pub json: bool,
    pub color: ColorMode,
    pub app_name: String,
    pub prompt: String,
}

/// Arguments for mentor-prompt-raw command (low-level).
#[derive(Debug, Clone)]
pub struct MentorPromptRawArgs {
    pub json: bool,
    pub color: ColorMode,
    pub session_id: String,
    pub message: String,
    pub attachment_refs: Vec<String>,
}

/// Arguments for mentor-get-run command.
#[derive(Debug, Clone)]
pub struct MentorGetRunArgs {
    pub json: bool,
    pub color: ColorMode,
    pub session_id: String,
    pub run_id: String,
    pub cursor: Option<i64>,
}

/// Arguments for mentor-get-event command.
#[derive(Debug, Clone)]
pub struct MentorGetEventArgs {
    pub json: bool,
    pub color: ColorMode,
    pub session_id: String,
    pub run_id: String,
    pub event_id: i64,
}

/// Arguments for mentor session commands (start, close, cancel-prompt).
#[derive(Debug, Clone)]
pub struct MentorSessionArgs {
    pub json: bool,
    pub color: ColorMode,
    pub session_id: String,
    pub run_id: Option<String>, // Used by cancel-prompt
}

/// Arguments for mentor-request-upload command.
#[derive(Debug, Clone)]
pub struct MentorRequestUploadArgs {
    pub json: bool,
    pub color: ColorMode,
    pub session_id: String,
    pub file_name: String,
    pub size_bytes: i64,
}

/// Arguments for mentor-publish command.
#[derive(Debug, Clone)]
pub struct MentorPublishArgs {
    pub json: bool,
    pub color: ColorMode,
    pub session_id: String,
    pub comment: Option<String>,
}

// ============================================================================
// REVISION COMMANDS
// ============================================================================

/// Arguments for revision listing and query commands.
#[derive(Debug, Clone)]
pub struct LatestRevisionArgs {
    pub json: bool,
    pub color: ColorMode,
}

/// Arguments for list-revisions command.
#[derive(Debug, Clone)]
pub struct ListRevisionsArgs {
    pub json: bool,
    pub color: ColorMode,
    pub offset: Option<i64>,
    pub limit: i64,
}

/// Arguments for get-revision command.
#[derive(Debug, Clone)]
pub struct GetRevisionArgs {
    pub json: bool,
    pub color: ColorMode,
    pub revision: i32,
}

/// Arguments for producer-graph command.
#[derive(Debug, Clone)]
pub struct ProducerGraphArgs {
    pub json: bool,
    pub color: ColorMode,
    pub revision: Option<i32>,
    pub env: String,
    pub filter: String,
    pub all_producers: bool,
    pub max_depth: i32,
    pub output: String,
}

// ============================================================================
// AUTH & PORTFOLIO COMMANDS
// ============================================================================

/// Arguments for discover command.
#[derive(Debug, Clone)]
pub struct DiscoverArgs {
    pub json: bool,
    pub color: ColorMode,
}

/// Arguments for list-portfolios command.
#[derive(Debug, Clone)]
pub struct ListPortfoliosArgs {
    pub json: bool,
    pub color: ColorMode,
}

// ============================================================================
// USER & GROUP COMMANDS
// ============================================================================

/// Arguments for user/group query commands.
#[derive(Debug, Clone)]
pub struct GetUserArgs {
    pub json: bool,
    pub color: ColorMode,
}

/// Arguments for update-user command.
#[derive(Debug, Clone)]
pub struct UpdateUserArgs {
    pub json: bool,
    pub color: ColorMode,
    pub given_name: Option<String>,
    pub surname: Option<String>,
}

/// Arguments for list-groups command.
#[derive(Debug, Clone)]
pub struct ListGroupsArgs {
    pub json: bool,
    pub color: ColorMode,
    pub filter: String,
    pub no_resolve: bool,
}

/// Arguments for group query/update commands.
#[derive(Debug, Clone)]
pub struct GetGroupArgs {
    pub json: bool,
    pub color: ColorMode,
}

/// Arguments for update-group command.
#[derive(Debug, Clone)]
pub struct UpdateGroupArgs {
    pub json: bool,
    pub color: ColorMode,
    pub env: String,
    pub description: Option<String>,
}

/// Arguments for list-group-members command.
#[derive(Debug, Clone)]
pub struct ListGroupMembersArgs {
    pub json: bool,
    pub color: ColorMode,
    pub env: String,
}

/// Arguments for user-group membership commands.
#[derive(Debug, Clone)]
pub struct UserGroupArgs {
    pub json: bool,
    pub color: ColorMode,
    pub env: String,
}

// ============================================================================
// ROLE COMMANDS
// ============================================================================

/// Arguments for role query commands.
#[derive(Debug, Clone)]
pub struct ListRolesArgs {
    pub json: bool,
    pub color: ColorMode,
    pub asset: String,
    pub env: String,
    pub no_resolve: bool,
}

/// Arguments for list-role-assignments command.
#[derive(Debug, Clone)]
pub struct ListRoleAssignmentsArgs {
    pub json: bool,
    pub color: ColorMode,
    pub asset: String,
    pub env: String,
    pub no_resolve: bool,
}

/// Arguments for role grant/revoke commands.
#[derive(Debug, Clone)]
pub struct RoleGrantArgs {
    pub json: bool,
    pub color: ColorMode,
    pub role: String,
    pub user: String,
    pub asset: String,
}

/// Arguments for role revoke commands.
#[derive(Debug, Clone)]
pub struct RoleRevokeArgs {
    pub json: bool,
    pub color: ColorMode,
    pub role: String,
    pub user: String,
    pub asset: String,
}

/// Arguments for group role grant commands.
#[derive(Debug, Clone)]
pub struct GroupRoleGrantArgs {
    pub json: bool,
    pub color: ColorMode,
    pub role: String,
    pub group: String,
    pub asset: String,
    pub env: String,
}

/// Arguments for group role revoke commands.
#[derive(Debug, Clone)]
pub struct GroupRoleRevokeArgs {
    pub json: bool,
    pub color: ColorMode,
    pub role: String,
    pub group: String,
    pub asset: String,
    pub env: String,
}

// ============================================================================
// SOURCE CODE COMMANDS
// ============================================================================

/// Arguments for download-source-code command.
#[derive(Debug, Clone)]
pub struct DownloadSourceCodeArgs {
    pub json: bool,
    pub color: ColorMode,
    pub revision: Option<i32>,
    pub output: String,
}

/// Arguments for upload-source-code command.
#[derive(Debug, Clone)]
pub struct UploadSourceCodeArgs {
    pub json: bool,
    pub color: ColorMode,
}

// ============================================================================
// BATCH OPERATION COMMANDS
// ============================================================================

/// Arguments for batch deployment commands.
#[derive(Debug, Clone)]
pub struct BatchDeployArgs {
    pub json: bool,
    pub color: ColorMode,
    pub poll_interval: u64,
    pub timeout: u64,
    pub build_type: String,
}

/// Arguments for batch undeploy command.
#[derive(Debug, Clone)]
pub struct BatchUndeployArgs {
    pub json: bool,
    pub color: ColorMode,
    pub poll_interval: u64,
    pub timeout: u64,
}

/// Arguments for batch delete command.
#[derive(Debug, Clone)]
pub struct BatchDeleteArgs {
    pub json: bool,
    pub color: ColorMode,
}

/// Arguments for dangerous-batch-undeploy-all command.
#[derive(Debug, Clone)]
pub struct DangerousBatchUndeployAllArgs {
    pub json: bool,
    pub color: ColorMode,
    pub poll_interval: u64,
    pub timeout: u64,
}
