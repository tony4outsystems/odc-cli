//! Clap-derived argument structs for each subcommand.
//!
//! Each struct here is flattened directly into its `Commands` variant (see `cli.rs`), so clap
//! parses straight into the type each handler consumes — no separate conversion step. Global
//! flags (`--json`, `--color`, `--no-resolve`) live on `Cli` / `commands::context::Ctx`, not
//! here, since they're shared by every command.
//!
//! Struct-level doc comments are intentionally plain `//` (not `///`): a `///` doc comment on
//! an `Args`-derived struct can override the subcommand's `about` text taken from the enum
//! variant's doc comment in `cli.rs`, which would cause `--help` drift between the two.

use crate::cli::{AppType, AssigneeType, MentorAssetType};

// Shared flags for commands that start and poll a long-running operation.
#[derive(clap::Args, Debug, Clone)]
pub struct PollArgs {
    /// Seconds between status polls
    #[arg(long = "poll-interval", default_value_t = 10)]
    pub poll_interval: u64,
    /// Positive seconds to wait before giving up
    #[arg(long, default_value_t = 1800)]
    pub timeout: u64,
    /// Return after starting the operation instead of polling
    #[arg(long)]
    pub no_wait: bool,
}

// Shared flags for commands that run multiple assets in parallel.
#[derive(clap::Args, Debug, Clone)]
pub struct ParallelArgs {
    /// Maximum assets to process concurrently
    #[arg(long, default_value_t = 3)]
    pub max_parallel: usize,
    /// Keep going on remaining assets if one fails, instead of stopping
    #[arg(long)]
    pub continue_on_error: bool,
}

// login
#[derive(clap::Args, Debug, Clone)]
pub struct LoginArgs {
    pub tenant_url: String,
    pub client_id: String,
}

// list-portfolios
#[derive(clap::Args, Debug, Clone)]
pub struct ListPortfoliosArgs {
    /// Filter to portfolios whose name or key contains this (case-insensitive)
    #[arg(long)]
    pub asset: Option<String>,
    /// Fetch a single page starting at this result index (default: fetch every page)
    #[arg(long)]
    pub offset: Option<i64>,
    /// Page size to request from the API
    #[arg(long, default_value_t = 100)]
    pub limit: i64,
}

// list-assets
#[derive(clap::Args, Debug, Clone)]
pub struct ListAssetsArgs {
    /// Filter to assets whose name or key contains this (case-insensitive)
    #[arg(long)]
    pub asset: Option<String>,
    /// Filter to assets of this type
    #[arg(long = "type")]
    pub app_type: Option<AppType>,
    /// Fetch a single page starting at this result index (default: fetch every page)
    #[arg(long)]
    pub offset: Option<i64>,
    /// Page size to request from the API
    #[arg(long, default_value_t = 100)]
    pub limit: i64,
}

// list-deployed-assets
#[derive(clap::Args, Debug, Clone)]
pub struct ListDeployedAssetsArgs {
    /// Filter to assets whose name or key contains this (case-insensitive)
    #[arg(long)]
    pub asset: Option<String>,
    /// Environment name, key, or unambiguous partial name
    #[arg(long)]
    pub env: Option<String>,
    #[arg(long)]
    pub offset: Option<i64>,
    #[arg(long, default_value_t = 100)]
    pub limit: i64,
}

// get-asset
#[derive(clap::Args, Debug, Clone)]
pub struct GetAssetArgs {
    pub asset: String,
}

// latest-revision
#[derive(clap::Args, Debug, Clone)]
pub struct LatestRevisionArgs {
    pub asset: String,
}

// list-revisions
#[derive(clap::Args, Debug, Clone)]
pub struct ListRevisionsArgs {
    pub asset: String,
    #[arg(long)]
    pub offset: Option<i64>,
    #[arg(long, default_value_t = 100)]
    pub limit: i64,
}

// get-revision
#[derive(clap::Args, Debug, Clone)]
pub struct GetRevisionArgs {
    pub asset: String,
    /// Revision number to retrieve
    #[arg(long)]
    pub revision: i32,
}

// producer-graph
#[derive(clap::Args, Debug, Clone)]
pub struct ProducerGraphArgs {
    pub asset: String,
    /// Defaults to the latest revision
    #[arg(long)]
    pub revision: Option<i32>,
    /// Environment context for resolving producers
    #[arg(long)]
    pub env: Option<String>,
    /// Maximum producer depth to traverse; 0 means infinite
    #[arg(long, default_value_t = 0)]
    pub max_depth: i32,
    /// Deployable, Libraries, or All
    #[arg(long, default_value = "Deployable")]
    pub producer_type_filter: String,
    /// Shortcut for --producer-type-filter All
    #[arg(long)]
    pub all_producers: bool,
    /// Output path; defaults to producer-graph-<asset>-rev-<revision>.mmd
    #[arg(long)]
    pub output: Option<String>,
}

// download-source-code
#[derive(clap::Args, Debug, Clone)]
pub struct DownloadSourceCodeArgs {
    pub asset: String,
    /// Defaults to the latest revision
    #[arg(long)]
    pub revision: Option<i32>,
    /// Output file path; defaults to <asset-key>-rev-<revision>.oml
    #[arg(long)]
    pub output: Option<String>,
}

// upload-source-code
#[derive(clap::Args, Debug, Clone)]
pub struct UploadSourceCodeArgs {
    pub oml_file: String,
}

// analyze-deployment
#[derive(clap::Args, Debug, Clone)]
pub struct AnalyzeDeploymentArgs {
    /// Asset name or key
    pub asset: String,
    /// Environment name or key
    pub env: String,
    /// Defaults to the latest revision
    #[arg(long)]
    pub revision: Option<i32>,
    #[command(flatten)]
    pub poll: PollArgs,
}

// analyze-deletion
#[derive(clap::Args, Debug, Clone)]
pub struct AnalyzeDeletionArgs {
    /// Asset name or key
    pub asset: String,
    #[command(flatten)]
    pub poll: PollArgs,
}

// deploy
#[derive(clap::Args, Debug, Clone)]
pub struct DeployArgs {
    /// Asset name or key
    pub asset: String,
    /// Environment name or key
    pub env: String,
    /// Defaults to the asset's current revision (falls back to the latest)
    #[arg(long)]
    pub revision: Option<i32>,
    /// Debug or Release
    #[arg(long, default_value = "Release")]
    pub build_type: String,
    #[command(flatten)]
    pub poll: PollArgs,
}

// undeploy
#[derive(clap::Args, Debug, Clone)]
pub struct UndeployArgs {
    /// Asset name or key
    pub asset: String,
    /// Environment name or key
    pub env: String,
    #[command(flatten)]
    pub poll: PollArgs,
}

// delete-asset
#[derive(clap::Args, Debug, Clone)]
pub struct DeleteAssetArgs {
    /// Asset name or key
    pub asset: String,
}

// batch-deploy
#[derive(clap::Args, Debug, Clone)]
pub struct BatchDeployArgs {
    pub assets_file: String,
    #[arg(long)]
    pub env: String,
    /// Debug or Release
    #[arg(long, default_value = "Release")]
    pub build_type: String,
    /// Deploy only the assets listed in the file, without their producer dependencies
    #[arg(long)]
    pub skip_dependencies: bool,
    #[command(flatten)]
    pub poll: PollArgs,
    #[command(flatten)]
    pub parallel: ParallelArgs,
}

// batch-undeploy
#[derive(clap::Args, Debug, Clone)]
pub struct BatchUndeployArgs {
    pub assets_file: String,
    #[arg(long)]
    pub env: String,
    #[command(flatten)]
    pub poll: PollArgs,
    #[command(flatten)]
    pub parallel: ParallelArgs,
}

// batch-delete
#[derive(clap::Args, Debug, Clone)]
pub struct BatchDeleteArgs {
    pub assets_file: String,
    #[command(flatten)]
    pub parallel: ParallelArgs,
}

// dangerous-batch-undeploy-all
#[derive(clap::Args, Debug, Clone)]
pub struct DangerousBatchUndeployAllArgs {
    #[arg(long)]
    pub env: String,
    #[command(flatten)]
    pub poll: PollArgs,
    #[command(flatten)]
    pub parallel: ParallelArgs,
}

// get-user
#[derive(clap::Args, Debug, Clone)]
pub struct GetUserArgs {
    pub user: String,
}

// update-user
#[derive(clap::Args, Debug, Clone)]
pub struct UpdateUserArgs {
    pub user: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub is_active: Option<bool>,
    #[arg(long)]
    pub photo_url: Option<String>,
}

// list-roles
#[derive(clap::Args, Debug, Clone)]
pub struct ListRolesArgs {
    /// Asset name or key (app the roles belong to)
    pub asset: String,
    /// Environment name, key, or unambiguous partial name (optional)
    #[arg(long)]
    pub env: Option<String>,
}

// list-role-assignments
#[derive(clap::Args, Debug, Clone)]
pub struct ListRoleAssignmentsArgs {
    /// Asset name or key (app the roles belong to)
    pub asset: String,
    /// Environment name, key, or unambiguous partial name (optional)
    #[arg(long)]
    pub env: Option<String>,
    /// Only show assignments of this type (default: both)
    #[arg(long, value_enum)]
    pub r#type: Option<AssigneeType>,
}

// grant-role
#[derive(clap::Args, Debug, Clone)]
pub struct GrantRoleArgs {
    /// App the role belongs to (name or key)
    pub asset: String,
    /// Environment name, key, or unambiguous partial name
    pub env: String,
    /// Role name or key
    pub role: String,
    /// User key or email
    pub user: String,
}

// revoke-role
#[derive(clap::Args, Debug, Clone)]
pub struct RevokeRoleArgs {
    pub asset: String,
    pub env: String,
    pub role: String,
    pub user: String,
}

// list-groups
#[derive(clap::Args, Debug, Clone)]
pub struct ListGroupsArgs {
    /// Substring to filter groups by name
    #[arg(long)]
    pub group: Option<String>,
    /// Environment name, key, or unambiguous partial name
    #[arg(long)]
    pub env: Option<String>,
}

// get-group
#[derive(clap::Args, Debug, Clone)]
pub struct GetGroupArgs {
    pub group: String,
}

// update-group
#[derive(clap::Args, Debug, Clone)]
pub struct UpdateGroupArgs {
    pub group: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
}

// list-group-members
#[derive(clap::Args, Debug, Clone)]
pub struct ListGroupMembersArgs {
    pub group: String,
}

// add-user-to-group
#[derive(clap::Args, Debug, Clone)]
pub struct AddUserToGroupArgs {
    pub group: String,
    pub user: String,
}

// remove-user-from-group
#[derive(clap::Args, Debug, Clone)]
pub struct RemoveUserFromGroupArgs {
    pub group: String,
    pub user: String,
}

// grant-group-role
#[derive(clap::Args, Debug, Clone)]
pub struct GrantGroupRoleArgs {
    /// App the role belongs to (name or key)
    pub asset: String,
    /// Environment name, key, or unambiguous partial name
    pub env: String,
    /// Role name or key
    pub role: String,
    /// Group name or key
    pub group: String,
}

// revoke-group-role
#[derive(clap::Args, Debug, Clone)]
pub struct RevokeGroupRoleArgs {
    pub asset: String,
    pub env: String,
    pub role: String,
    pub group: String,
}

// internal-build
#[derive(clap::Args, Debug, Clone)]
pub struct InternalBuildArgs {
    #[arg(long)]
    pub asset: String,
    #[arg(long)]
    pub env: String,
    #[arg(long)]
    pub revision: Option<i32>,
    #[arg(long, default_value = "Release")]
    pub build_type: String,
    #[command(flatten)]
    pub poll: PollArgs,
}

// internal-publish
#[derive(clap::Args, Debug, Clone)]
pub struct InternalPublishArgs {
    #[arg(long)]
    pub asset: String,
    #[arg(long)]
    pub env: String,
    #[arg(long)]
    pub revision: Option<i32>,
    #[command(flatten)]
    pub poll: PollArgs,
}

// internal-deploy
#[derive(clap::Args, Debug, Clone)]
pub struct InternalDeployArgs {
    #[arg(long)]
    pub asset: String,
    #[arg(long)]
    pub env: String,
    #[arg(long)]
    pub revision: Option<i32>,
    #[arg(long)]
    pub build_key: String,
    #[command(flatten)]
    pub poll: PollArgs,
}

// mentor-create-asset
#[derive(clap::Args, Debug, Clone)]
pub struct MentorCreateAssetArgs {
    pub session_id: String,
    #[arg(long = "type", value_enum)]
    pub asset_type: MentorAssetType,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub portfolio_key: String,
    #[arg(long)]
    pub description: Option<String>,
    /// Clone from an existing AVS application instead of the built-in template
    #[arg(long)]
    pub template_asset_key: Option<String>,
}

// mentor-load-asset
#[derive(clap::Args, Debug, Clone)]
pub struct MentorLoadAssetArgs {
    pub session_id: String,
    pub asset_key: String,
    /// Defaults to the latest revision
    #[arg(long)]
    pub revision: Option<i32>,
}

// mentor
#[derive(clap::Args, Debug, Clone)]
pub struct MentorArgs {
    /// Name or key of the app to edit
    pub app_name: String,
    /// The prompt message (optional; if not provided, starts interactive mode)
    pub prompt: Option<String>,
}

// mentor-prompt
#[derive(clap::Args, Debug, Clone)]
pub struct MentorPromptArgs {
    pub session_id: String,
    pub message: String,
    /// Attachment id(s) from a prior `mentor-request-upload`
    #[arg(long = "attachment-ref")]
    pub attachment_refs: Vec<String>,
}

// mentor-get-run
#[derive(clap::Args, Debug, Clone)]
pub struct MentorGetRunArgs {
    pub session_id: String,
    pub run_id: String,
    /// Re-read from this position instead of only new events (0 = from the start)
    #[arg(long)]
    pub cursor: Option<i64>,
}

// mentor-get-event
#[derive(clap::Args, Debug, Clone)]
pub struct MentorGetEventArgs {
    pub session_id: String,
    pub run_id: String,
    pub event_id: i64,
}

// mentor-cancel-prompt
#[derive(clap::Args, Debug, Clone)]
pub struct MentorCancelPromptArgs {
    pub session_id: String,
    pub run_id: String,
}

// mentor-close-session
#[derive(clap::Args, Debug, Clone)]
pub struct MentorCloseSessionArgs {
    pub session_id: String,
}

// mentor-request-upload
#[derive(clap::Args, Debug, Clone)]
pub struct MentorRequestUploadArgs {
    pub session_id: String,
    pub file_name: String,
    pub size_bytes: i64,
}

// mentor-publish
#[derive(clap::Args, Debug, Clone)]
pub struct MentorPublishArgs {
    pub session_id: String,
    #[arg(long)]
    pub comment: Option<String>,
}

// completion
#[derive(clap::Args, Debug, Clone)]
pub struct CompletionArgs {
    pub shell: clap_complete::Shell,
}
