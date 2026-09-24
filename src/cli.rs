use clap::{CommandFactory, Parser, Subcommand};
use serde_json::Map;
use std::time::Duration;

/// Command names grouped by category, in display order, for the categorized top-level help.
const HELP_CATEGORIES: &[(&str, &[&str])] = &[
    ("Auth", &["discover", "login"]),
    ("Portfolios", &["list-portfolios"]),
    (
        "Assets & Environments",
        &[
            "list-environments",
            "list-assets",
            "list-deployed-assets",
            "get-asset",
            "delete-asset",
        ],
    ),
    (
        "Revisions & Source",
        &[
            "latest-revision",
            "list-revisions",
            "get-revision",
            "producer-graph",
            "download-source-code",
            "upload-source-code",
        ],
    ),
    (
        "Deployment",
        &[
            "analyze-deployment",
            "analyze-deletion",
            "deploy",
            "undeploy",
        ],
    ),
    (
        "Batch Operations",
        &[
            "batch-deploy",
            "batch-undeploy",
            "batch-delete",
            "dangerous-batch-undeploy-all",
        ],
    ),
    ("Users", &["get-user", "update-user"]),
    (
        "Groups",
        &[
            "list-groups",
            "get-group",
            "update-group",
            "list-group-members",
            "add-user-to-group",
            "remove-user-from-group",
        ],
    ),
    (
        "Roles",
        &[
            "list-roles",
            "list-role-assignments",
            "grant-role",
            "revoke-role",
            "grant-group-role",
            "revoke-group-role",
        ],
    ),
    (
        "Internal (Advanced)",
        &["internal-build", "internal-publish", "internal-deploy"],
    ),
    ("Mentor", &["mentor"]),
    (
        "Mentor (Advanced)",
        &[
            "mentor-start-session",
            "mentor-create-asset",
            "mentor-load-asset",
            "mentor-prompt",
            "mentor-get-run",
            "mentor-get-event",
            "mentor-cancel-prompt",
            "mentor-request-upload",
            "mentor-publish",
            "mentor-close-session",
        ],
    ),
    ("Misc", &["completion"]),
];

/// Prints the top-level `odc --help` output with commands grouped into categories, since
/// clap doesn't support heading grouping for subcommands.
pub fn print_categorized_help() {
    let app = Cli::command();
    println!("{}\n", app.get_about().unwrap());
    println!("Usage: odc [OPTIONS] <COMMAND>\n");

    let name_width = app
        .get_subcommands()
        .map(|c| c.get_name().len())
        .max()
        .unwrap_or(0)
        + 2;

    for (category, names) in HELP_CATEGORIES {
        println!("{category}:");
        for name in *names {
            if let Some(sub) = app.find_subcommand(name) {
                let about = sub.get_about().map(|s| s.to_string()).unwrap_or_default();
                println!("  {:<width$}  {}", name, about, width = name_width);
            }
        }
        println!();
    }

    println!("Options:");

    // Extract and print global arguments automatically
    for arg in app.get_arguments() {
        // Skip help and version (they're handled separately below)
        if matches!(arg.get_id().as_str(), "help" | "version") {
            continue;
        }

        let short = arg.get_short();
        let long = arg.get_long();
        let help = arg.get_help().map(|h| h.to_string()).unwrap_or_default();

        // Format flag part: "  -n, --no-resolve" or "      --json"
        let mut flag_str = match (short, long) {
            (Some(s), Some(l)) => format!("  -{}, --{}", s, l),
            (Some(s), None) => format!("  -{}", s),
            (None, Some(l)) => format!("      --{}", l),
            _ => continue,
        };

        // Add value placeholder only if the argument takes a value
        // Boolean flags don't need a value placeholder
        if !matches!(
            arg.get_action(),
            clap::ArgAction::SetTrue | clap::ArgAction::SetFalse
        ) {
            if let Some(value_names) = arg.get_value_names() {
                if !value_names.is_empty() {
                    let value_placeholder = value_names
                        .first()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "VALUE".to_string());
                    flag_str.push_str(&format!(" <{}>", value_placeholder));
                }
            }
        }

        println!("{:<35}  {}", flag_str, help);
    }

    // Print help and version explicitly at the end
    println!("  -h, --help           Print help");
    println!("  -V, --version        Print version");
}

/// odc: OutSystems Developer Cloud CLI
#[derive(Parser, Debug)]
#[command(
    name = "odc",
    version,
    about = "OutSystems ODC CLI",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    /// Control color output
    #[arg(long, global = true, default_value = "auto", value_parser = parse_color_arg)]
    pub color: crate::output::ColorMode,

    /// Disable resolution of external keys (environment names, role names, etc.) in user-friendly output
    #[arg(short = 'n', long, global = true)]
    pub no_resolve: bool,

    #[command(subcommand)]
    pub command: Commands,
}

fn parse_color_arg(s: &str) -> Result<crate::output::ColorMode, String> {
    parse_color(s).map_err(|e| e.to_string())
}

/// Recognized asset types, per the asset-repository API's `assetTypes` filter parameter.
#[derive(clap::ValueEnum, Debug, Clone, Copy, strum_macros::Display)]
pub enum AppType {
    #[value(name = "WebApplication")]
    #[strum(to_string = "WebApplication")]
    WebApplication,
    #[value(name = "MobileApplication")]
    #[strum(to_string = "MobileApplication")]
    MobileApplication,
    #[value(name = "LowCodeLibrary")]
    #[strum(to_string = "LowCodeLibrary")]
    LowCodeLibrary,
    #[value(name = "ExtensionLibrary")]
    #[strum(to_string = "ExtensionLibrary")]
    ExtensionLibrary,
    #[value(name = "ExternalConnection")]
    #[strum(to_string = "ExternalConnection")]
    ExternalConnection,
    #[value(name = "ExternalLibrary")]
    #[strum(to_string = "ExternalLibrary")]
    ExternalLibrary,
    #[value(name = "Workflow")]
    #[strum(to_string = "Workflow")]
    Workflow,
    #[value(name = "WidgetLibrary")]
    #[strum(to_string = "WidgetLibrary")]
    WidgetLibrary,
    #[value(name = "AIModelConnection")]
    #[strum(to_string = "AIModelConnection")]
    AiModelConnection,
    #[value(name = "SearchServiceConnection")]
    #[strum(to_string = "SearchServiceConnection")]
    SearchServiceConnection,
    #[value(name = "Agent")]
    #[strum(to_string = "Agent")]
    Agent,
    #[value(name = "MCPConnection")]
    #[strum(to_string = "MCPConnection")]
    McpConnection,
    #[value(name = "A2AConnection")]
    #[strum(to_string = "A2AConnection")]
    A2aConnection,
    #[value(name = "KnowledgeBase")]
    #[strum(to_string = "KnowledgeBase")]
    KnowledgeBase,
}

impl AppType {
    /// The exact `assetType` string this variant matches, as returned by the API.
    pub fn as_str(&self) -> &'static str {
        match self {
            AppType::WebApplication => "WebApplication",
            AppType::MobileApplication => "MobileApplication",
            AppType::LowCodeLibrary => "LowCodeLibrary",
            AppType::ExtensionLibrary => "ExtensionLibrary",
            AppType::ExternalConnection => "ExternalConnection",
            AppType::ExternalLibrary => "ExternalLibrary",
            AppType::Workflow => "Workflow",
            AppType::WidgetLibrary => "WidgetLibrary",
            AppType::AiModelConnection => "AIModelConnection",
            AppType::SearchServiceConnection => "SearchServiceConnection",
            AppType::Agent => "Agent",
            AppType::McpConnection => "MCPConnection",
            AppType::A2aConnection => "A2AConnection",
            AppType::KnowledgeBase => "KnowledgeBase",
        }
    }
}

/// Asset types Mentor can create, per `mentor_create_asset`'s `assetType` parameter.
#[derive(clap::ValueEnum, Debug, Clone, Copy, strum_macros::Display)]
pub enum MentorAssetType {
    #[value(name = "WebApplication")]
    #[strum(to_string = "WebApplication")]
    WebApplication,
    #[value(name = "Agent")]
    #[strum(to_string = "Agent")]
    Agent,
    #[value(name = "Library")]
    #[strum(to_string = "Library")]
    Library,
    #[value(name = "Workflow")]
    #[strum(to_string = "Workflow")]
    Workflow,
}

impl MentorAssetType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MentorAssetType::WebApplication => "WebApplication",
            MentorAssetType::Agent => "Agent",
            MentorAssetType::Library => "Library",
            MentorAssetType::Workflow => "Workflow",
        }
    }
}

/// Role assignee kinds, per `list-role-assignments`'s `--type` filter.
#[derive(clap::ValueEnum, Debug, Clone, Copy, strum_macros::Display)]
pub enum AssigneeType {
    #[value(name = "User")]
    #[strum(to_string = "User")]
    User,
    #[value(name = "Group")]
    #[strum(to_string = "Group")]
    Group,
}

impl AssigneeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssigneeType::User => "User",
            AssigneeType::Group => "Group",
        }
    }
}

/// Shared polling flags for commands that start and wait on an operation.
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

/// Shared flags for commands that run multiple assets in parallel.
#[derive(clap::Args, Debug, Clone)]
pub struct ParallelArgs {
    /// Maximum assets to process concurrently
    #[arg(long, default_value_t = 3)]
    pub max_parallel: usize,
    /// Keep going on remaining assets if one fails, instead of stopping
    #[arg(long)]
    pub continue_on_error: bool,
}

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum Commands {
    /// Show the OAuth discovery document (issuer, endpoints, scopes).
    Discover,

    /// Save credentials in ~/.odc/config.json (prompts for client secret).
    Login {
        tenant_url: String,
        client_id: String,
    },

    /// List portfolios in the tenant.
    ListPortfolios {
        /// Filter to portfolios whose name or key contains this (case-insensitive)
        #[arg(long)]
        asset: Option<String>,
        /// Fetch a single page starting at this result index (default: fetch every page)
        #[arg(long)]
        offset: Option<i64>,
        /// Page size to request from the API
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },

    /// List environments in the tenant.
    ListEnvironments,

    /// List assets in the tenant, optionally filtered by name/key and/or type.
    ListAssets {
        /// Filter to assets whose name or key contains this (case-insensitive)
        #[arg(long)]
        asset: Option<String>,
        /// Filter to assets of this type
        #[arg(long = "type")]
        app_type: Option<AppType>,
        /// Fetch a single page starting at this result index (default: fetch every page)
        #[arg(long)]
        offset: Option<i64>,
        /// Page size to request from the API
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },

    /// List deployed assets, optionally filtered by environment and name/key.
    ListDeployedAssets {
        /// Filter to assets whose name or key contains this (case-insensitive)
        #[arg(long)]
        asset: Option<String>,
        /// Environment name, key, or unambiguous partial name
        #[arg(long)]
        env: Option<String>,
        #[arg(long)]
        offset: Option<i64>,
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },

    /// Retrieve asset metadata.
    GetAsset { asset: String },

    /// Print the latest revision number of an asset.
    LatestRevision { asset: String },

    /// List all revisions of an asset.
    ListRevisions {
        asset: String,
        #[arg(long)]
        offset: Option<i64>,
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },

    /// Retrieve a specific asset revision.
    GetRevision {
        asset: String,
        /// Revision number to retrieve
        #[arg(long)]
        revision: i32,
    },

    /// Render an asset's producer dependency graph as Mermaid.
    ProducerGraph {
        asset: String,
        /// Defaults to the latest revision
        #[arg(long)]
        revision: Option<i32>,
        /// Environment context for resolving producers
        #[arg(long)]
        env: Option<String>,
        /// Maximum producer depth to traverse; 0 means infinite
        #[arg(long, default_value_t = 0)]
        max_depth: i32,
        /// Deployable, Libraries, or All
        #[arg(long, default_value = "Deployable")]
        producer_type_filter: String,
        /// Shortcut for --producer-type-filter All
        #[arg(long)]
        all_producers: bool,
        /// Output path; defaults to producer-graph-<asset>-rev-<revision>.mmd
        #[arg(long)]
        output: Option<String>,
    },

    /// Download the OML source code of an asset revision.
    DownloadSourceCode {
        asset: String,
        /// Defaults to the latest revision
        #[arg(long)]
        revision: Option<i32>,
        /// Output file path; defaults to <asset-key>-rev-<revision>.oml
        #[arg(long)]
        output: Option<String>,
    },

    /// Upload an OML/XIF file, creating a new asset or revision.
    UploadSourceCode { oml_file: String },

    /// Analyze the impact of deploying an asset revision.
    AnalyzeDeployment {
        /// Asset name or key
        asset: String,
        /// Environment name or key
        env: String,
        /// Defaults to the latest revision
        #[arg(long)]
        revision: Option<i32>,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Analyze the impact of deleting an asset.
    AnalyzeDeletion {
        /// Asset name or key
        asset: String,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Deploy an asset to an environment.
    Deploy {
        /// Asset name or key
        asset: String,
        /// Environment name or key
        env: String,
        /// Defaults to the asset's current revision (falls back to the latest)
        #[arg(long)]
        revision: Option<i32>,
        /// Debug or Release
        #[arg(long, default_value = "Release")]
        build_type: String,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Undeploy an asset from an environment.
    Undeploy {
        /// Asset name or key
        asset: String,
        /// Environment name or key
        env: String,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Delete an asset.
    DeleteAsset {
        /// Asset name or key
        asset: String,
    },

    /// Deploy multiple assets listed in a file.
    BatchDeploy {
        assets_file: String,
        #[arg(long)]
        env: String,
        /// Debug or Release
        #[arg(long, default_value = "Release")]
        build_type: String,
        /// Deploy only the assets listed in the file, without their producer dependencies
        #[arg(long)]
        skip_dependencies: bool,
        #[command(flatten)]
        poll: PollArgs,
        #[command(flatten)]
        parallel: ParallelArgs,
    },

    /// Undeploy multiple assets listed in a file.
    BatchUndeploy {
        assets_file: String,
        #[arg(long)]
        env: String,
        #[command(flatten)]
        poll: PollArgs,
        #[command(flatten)]
        parallel: ParallelArgs,
    },

    /// Delete multiple assets listed in a file.
    BatchDelete {
        assets_file: String,
        #[command(flatten)]
        parallel: ParallelArgs,
    },

    /// Undeploy all assets from an environment.
    DangerousBatchUndeployAll {
        #[arg(long)]
        env: String,
        #[command(flatten)]
        poll: PollArgs,
        #[command(flatten)]
        parallel: ParallelArgs,
    },

    /// Retrieve a user's details.
    GetUser { user: String },

    /// Update a user's name, active status, or photo URL.
    UpdateUser {
        user: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        is_active: Option<bool>,
        #[arg(long)]
        photo_url: Option<String>,
    },

    /// List application roles defined for an asset.
    ListRoles {
        /// Asset name or key (app the roles belong to)
        asset: String,
        /// Environment name, key, or unambiguous partial name (optional)
        #[arg(long)]
        env: Option<String>,
    },

    /// List, for each application role of an asset, the users and/or groups assigned to it.
    ListRoleAssignments {
        /// Asset name or key (app the roles belong to)
        asset: String,
        /// Environment name, key, or unambiguous partial name (optional)
        #[arg(long)]
        env: Option<String>,
        /// Only show assignments of this type (default: both)
        #[arg(long, value_enum)]
        r#type: Option<AssigneeType>,
    },

    /// Grant an application role to a user.
    GrantRole {
        /// App the role belongs to (name or key)
        asset: String,
        /// Environment name, key, or unambiguous partial name
        env: String,
        /// Role name or key
        role: String,
        /// User key or email
        user: String,
    },

    /// Revoke an application role from a user.
    RevokeRole {
        asset: String,
        env: String,
        role: String,
        user: String,
    },

    /// List end-user groups, optionally filtered by name and/or environment.
    ListGroups {
        /// Substring to filter groups by name
        #[arg(long)]
        group: Option<String>,
        /// Environment name, key, or unambiguous partial name
        #[arg(long)]
        env: Option<String>,
    },

    /// Retrieve an end-user group's details.
    GetGroup { group: String },

    /// Update a group's name or description.
    UpdateGroup {
        group: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
    },

    /// List the members of an end-user group.
    ListGroupMembers { group: String },

    /// Add a user to an end-user group.
    AddUserToGroup { group: String, user: String },

    /// Remove a user from an end-user group.
    RemoveUserFromGroup { group: String, user: String },

    /// Grant an application role to an end-user group.
    GrantGroupRole {
        /// App the role belongs to (name or key)
        asset: String,
        /// Environment name, key, or unambiguous partial name
        env: String,
        /// Role name or key
        role: String,
        /// Group name or key
        group: String,
    },

    /// Revoke an application role from an end-user group.
    RevokeGroupRole {
        asset: String,
        env: String,
        role: String,
        group: String,
    },

    /// Start a build for an asset revision.
    InternalBuild {
        #[arg(long)]
        asset: String,
        #[arg(long)]
        env: String,
        #[arg(long)]
        revision: Option<i32>,
        #[arg(long, default_value = "Release")]
        build_type: String,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Publish a build to an environment.
    InternalPublish {
        #[arg(long)]
        asset: String,
        #[arg(long)]
        env: String,
        #[arg(long)]
        revision: Option<i32>,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Deploy an existing build to an environment.
    InternalDeploy {
        #[arg(long)]
        asset: String,
        #[arg(long)]
        env: String,
        #[arg(long)]
        revision: Option<i32>,
        #[arg(long)]
        build_key: String,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Start a new Mentor session; prints the sessionId used for follow-up commands.
    MentorStartSession,

    /// Create a new asset in a Mentor session, so `mentor` can edit it.
    MentorCreateAsset {
        session_id: String,
        #[arg(long = "type", value_enum)]
        asset_type: MentorAssetType,
        #[arg(long)]
        name: String,
        #[arg(long)]
        portfolio_key: String,
        #[arg(long)]
        description: Option<String>,
        /// Clone from an existing AVS application instead of the built-in template
        #[arg(long)]
        template_asset_key: Option<String>,
    },

    /// Load an existing asset into a Mentor session, so `mentor` can edit it.
    MentorLoadAsset {
        session_id: String,
        asset_key: String,
        /// Defaults to the latest revision
        #[arg(long)]
        revision: Option<i32>,
    },

    /// Send a prompt to Mentor and wait for completion, auto-publishing the result; or start interactive mode if no prompt is given.
    Mentor {
        /// Name or key of the app to edit
        app_name: String,
        /// The prompt message (optional; if not provided, starts interactive mode)
        prompt: Option<String>,
    },

    /// Send a prompt to a Mentor session; returns a runId to poll with `mentor-get-run` (low-level).
    MentorPrompt {
        session_id: String,
        message: String,
        /// Attachment id(s) from a prior `mentor-request-upload`
        #[arg(long = "attachment-ref")]
        attachment_refs: Vec<String>,
    },

    /// Poll a Mentor run's progress events and status.
    MentorGetRun {
        session_id: String,
        run_id: String,
        /// Re-read from this position instead of only new events (0 = from the start)
        #[arg(long)]
        cursor: Option<i64>,
    },

    /// Fetch the full body of a truncated Mentor run event.
    MentorGetEvent {
        session_id: String,
        run_id: String,
        event_id: i64,
    },

    /// Cancel the in-flight prompt for a Mentor session.
    MentorCancelPrompt { session_id: String, run_id: String },

    /// Close a Mentor session and release its resources.
    MentorCloseSession { session_id: String },

    /// Mint a presigned upload URL for a Mentor session attachment.
    MentorRequestUpload {
        session_id: String,
        file_name: String,
        size_bytes: i64,
    },

    /// Publish the asset loaded in a Mentor session to the connected dev environment.
    MentorPublish {
        session_id: String,
        #[arg(long)]
        comment: Option<String>,
    },

    /// Generate shell completion scripts.
    Completion { shell: clap_complete::Shell },
}

impl Commands {
    /// Extract typed arguments for `list-assets` command.
    ///
    /// # Panics
    /// If called on a command variant other than `ListAssets`.
    pub fn as_list_assets_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::ListAssetsArgs {
        match self {
            Commands::ListAssets {
                asset: filter,
                app_type,
                offset,
                limit,
            } => crate::commands::args::ListAssetsArgs {
                json,
                color,
                filter: filter.clone(),
                asset_type: *app_type,
                offset: *offset,
                limit: *limit,
            },
            _ => panic!("Expected ListAssets command"),
        }
    }

    /// Extract typed arguments for `list-deployed-assets` command.
    pub fn as_list_deployed_assets_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::ListDeployedAssetsArgs {
        match self {
            Commands::ListDeployedAssets {
                asset: filter,
                env,
                offset,
                limit,
            } => crate::commands::args::ListDeployedAssetsArgs {
                json,
                color,
                filter: filter.clone(),
                env: env.clone(),
                offset: *offset,
                limit: *limit,
            },
            _ => panic!("Expected ListDeployedAssets command"),
        }
    }

    /// Extract typed arguments for `get-asset` command.
    pub fn as_get_asset_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::GetAssetArgs {
        match self {
            Commands::GetAsset { .. } => crate::commands::args::GetAssetArgs { json, color },
            _ => panic!("Expected GetAsset command"),
        }
    }

    /// Extract typed arguments for `list-environments` command.
    pub fn as_list_environments_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::ListEnvironmentsArgs {
        match self {
            Commands::ListEnvironments => {
                crate::commands::args::ListEnvironmentsArgs { json, color }
            }
            _ => panic!("Expected ListEnvironments command"),
        }
    }

    // ========================================================================
    // DEPLOYMENT COMMAND EXTRACTION METHODS
    // ========================================================================

    pub fn as_deployment_analysis_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::DeploymentAnalysisArgs {
        match self {
            Commands::AnalyzeDeployment {
                asset,
                env,
                revision,
                poll,
            } => crate::commands::args::DeploymentAnalysisArgs {
                json,
                color,
                asset: asset.clone(),
                env: env.clone(),
                revision: *revision,
                poll_interval: poll.poll_interval,
                timeout: poll.timeout,
                no_wait: poll.no_wait,
            },
            _ => panic!("Expected AnalyzeDeployment command"),
        }
    }

    pub fn as_deletion_analysis_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::DeletionAnalysisArgs {
        match self {
            Commands::AnalyzeDeletion { asset, poll } => {
                crate::commands::args::DeletionAnalysisArgs {
                    json,
                    color,
                    asset: asset.clone(),
                    poll_interval: poll.poll_interval,
                    timeout: poll.timeout,
                    no_wait: poll.no_wait,
                }
            }
            _ => panic!("Expected AnalyzeDeletion command"),
        }
    }

    pub fn as_build_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::BuildArgs {
        match self {
            Commands::InternalBuild {
                asset,
                env,
                revision,
                build_type,
                poll,
            } => crate::commands::args::BuildArgs {
                json,
                color,
                asset: asset.clone(),
                env: env.clone(),
                revision: *revision,
                build_type: build_type.clone(),
                poll_interval: poll.poll_interval,
                timeout: poll.timeout,
                no_wait: poll.no_wait,
            },
            _ => panic!("Expected InternalBuild command"),
        }
    }

    pub fn as_publish_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::PublishArgs {
        match self {
            Commands::InternalPublish {
                asset,
                env,
                revision,
                poll,
            } => crate::commands::args::PublishArgs {
                json,
                color,
                asset: asset.clone(),
                env: env.clone(),
                revision: *revision,
                poll_interval: poll.poll_interval,
                timeout: poll.timeout,
                no_wait: poll.no_wait,
            },
            _ => panic!("Expected InternalPublish command"),
        }
    }

    pub fn as_internal_deploy_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::InternalDeployArgs {
        match self {
            Commands::InternalDeploy {
                asset,
                env,
                revision,
                build_key,
                poll,
            } => crate::commands::args::InternalDeployArgs {
                json,
                color,
                asset: asset.clone(),
                env: env.clone(),
                revision: *revision,
                build_key: build_key.clone(),
                poll_interval: poll.poll_interval,
                timeout: poll.timeout,
                no_wait: poll.no_wait,
            },
            _ => panic!("Expected InternalDeploy command"),
        }
    }

    pub fn as_deploy_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::DeploymentOperationArgs {
        match self {
            Commands::Deploy {
                asset,
                env,
                revision,
                build_type,
                poll,
            } => crate::commands::args::DeploymentOperationArgs {
                json,
                color,
                asset: asset.clone(),
                env: env.clone(),
                revision: *revision,
                build_type: build_type.clone(),
                poll_interval: poll.poll_interval,
                timeout: poll.timeout,
                no_wait: poll.no_wait,
            },
            _ => panic!("Expected Deploy command"),
        }
    }

    pub fn as_undeploy_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::DeploymentOperationArgs {
        match self {
            Commands::Undeploy { asset, env, poll } => {
                crate::commands::args::DeploymentOperationArgs {
                    json,
                    color,
                    asset: asset.clone(),
                    env: env.clone(),
                    revision: None,
                    build_type: String::new(), // Not used for undeploy
                    poll_interval: poll.poll_interval,
                    timeout: poll.timeout,
                    no_wait: poll.no_wait,
                }
            }
            _ => panic!("Expected Undeploy command"),
        }
    }

    pub fn as_delete_asset_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::DeleteAssetArgs {
        match self {
            Commands::DeleteAsset { asset } => crate::commands::args::DeleteAssetArgs {
                json,
                color,
                asset: asset.clone(),
            },
            _ => panic!("Expected DeleteAsset command"),
        }
    }

    // ========================================================================
    // MENTOR COMMAND EXTRACTION METHODS
    // ========================================================================

    pub fn as_mentor_create_asset_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorCreateAssetArgs {
        match self {
            Commands::MentorCreateAsset {
                session_id,
                asset_type,
                name,
                portfolio_key,
                description,
                template_asset_key,
            } => crate::commands::args::MentorCreateAssetArgs {
                json,
                color,
                session_id: session_id.clone(),
                asset_type: asset_type.as_str().to_string(),
                name: name.clone(),
                portfolio_key: portfolio_key.clone(),
                description: description.clone(),
                template_asset_key: template_asset_key.clone(),
            },
            _ => panic!("Expected MentorCreateAsset command"),
        }
    }

    pub fn as_mentor_load_asset_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorLoadAssetArgs {
        match self {
            Commands::MentorLoadAsset {
                session_id,
                revision,
                ..
            } => crate::commands::args::MentorLoadAssetArgs {
                json,
                color,
                session_id: session_id.clone(),
                revision: *revision,
            },
            _ => panic!("Expected MentorLoadAsset command"),
        }
    }

    pub fn as_mentor_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorArgs {
        match self {
            Commands::Mentor { app_name, prompt } => crate::commands::args::MentorArgs {
                json,
                color,
                app_name: app_name.clone(),
                prompt: prompt.clone(),
            },
            _ => panic!("Expected Mentor command"),
        }
    }

    pub fn as_mentor_prompt_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorPromptArgs {
        match self {
            Commands::MentorPrompt {
                session_id,
                message,
                attachment_refs,
            } => crate::commands::args::MentorPromptArgs {
                json,
                color,
                session_id: session_id.clone(),
                message: message.clone(),
                attachment_refs: attachment_refs.clone(),
            },
            _ => panic!("Expected MentorPrompt command"),
        }
    }

    pub fn as_mentor_get_run_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorGetRunArgs {
        match self {
            Commands::MentorGetRun {
                session_id,
                run_id,
                cursor,
            } => crate::commands::args::MentorGetRunArgs {
                json,
                color,
                session_id: session_id.clone(),
                run_id: run_id.clone(),
                cursor: *cursor,
            },
            _ => panic!("Expected MentorGetRun command"),
        }
    }

    pub fn as_mentor_get_event_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorGetEventArgs {
        match self {
            Commands::MentorGetEvent {
                session_id,
                run_id,
                event_id,
            } => crate::commands::args::MentorGetEventArgs {
                json,
                color,
                session_id: session_id.clone(),
                run_id: run_id.clone(),
                event_id: *event_id,
            },
            _ => panic!("Expected MentorGetEvent command"),
        }
    }

    pub fn as_mentor_start_session_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorSessionArgs {
        match self {
            Commands::MentorStartSession => {
                crate::commands::args::MentorSessionArgs {
                    json,
                    color,
                    session_id: String::new(), // Not used
                    run_id: None,
                }
            }
            _ => panic!("Expected MentorStartSession command"),
        }
    }

    pub fn as_mentor_cancel_prompt_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorSessionArgs {
        match self {
            Commands::MentorCancelPrompt { session_id, run_id } => {
                crate::commands::args::MentorSessionArgs {
                    json,
                    color,
                    session_id: session_id.clone(),
                    run_id: Some(run_id.clone()),
                }
            }
            _ => panic!("Expected MentorCancelPrompt command"),
        }
    }

    pub fn as_mentor_close_session_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorSessionArgs {
        match self {
            Commands::MentorCloseSession { session_id } => {
                crate::commands::args::MentorSessionArgs {
                    json,
                    color,
                    session_id: session_id.clone(),
                    run_id: None,
                }
            }
            _ => panic!("Expected MentorCloseSession command"),
        }
    }

    pub fn as_mentor_request_upload_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorRequestUploadArgs {
        match self {
            Commands::MentorRequestUpload {
                session_id,
                file_name,
                size_bytes,
            } => crate::commands::args::MentorRequestUploadArgs {
                json,
                color,
                session_id: session_id.clone(),
                file_name: file_name.clone(),
                size_bytes: *size_bytes,
            },
            _ => panic!("Expected MentorRequestUpload command"),
        }
    }

    pub fn as_mentor_publish_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::MentorPublishArgs {
        match self {
            Commands::MentorPublish {
                session_id,
                comment,
            } => crate::commands::args::MentorPublishArgs {
                json,
                color,
                session_id: session_id.clone(),
                comment: comment.clone(),
            },
            _ => panic!("Expected MentorPublish command"),
        }
    }

    // ========================================================================
    // REVISION COMMAND EXTRACTION METHODS
    // ========================================================================

    pub fn as_latest_revision_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::LatestRevisionArgs {
        match self {
            Commands::LatestRevision { .. } => {
                crate::commands::args::LatestRevisionArgs { json, color }
            }
            _ => panic!("Expected LatestRevision command"),
        }
    }

    pub fn as_list_revisions_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::ListRevisionsArgs {
        match self {
            Commands::ListRevisions { offset, limit, .. } => {
                crate::commands::args::ListRevisionsArgs {
                    json,
                    color,
                    offset: *offset,
                    limit: *limit,
                }
            }
            _ => panic!("Expected ListRevisions command"),
        }
    }

    pub fn as_get_revision_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::GetRevisionArgs {
        match self {
            Commands::GetRevision { revision, .. } => crate::commands::args::GetRevisionArgs {
                json,
                color,
                revision: *revision,
            },
            _ => panic!("Expected GetRevision command"),
        }
    }

    pub fn as_producer_graph_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::ProducerGraphArgs {
        match self {
            Commands::ProducerGraph {
                revision,
                env,
                max_depth,
                producer_type_filter,
                all_producers,
                output,
                ..
            } => crate::commands::args::ProducerGraphArgs {
                json,
                color,
                revision: *revision,
                env: env.clone().unwrap_or_default(),
                filter: producer_type_filter.clone(),
                all_producers: *all_producers,
                max_depth: *max_depth,
                output: output.clone().unwrap_or_default(),
            },
            _ => panic!("Expected ProducerGraph command"),
        }
    }

    // ========================================================================
    // AUTH & PORTFOLIO COMMAND EXTRACTION METHODS
    // ========================================================================

    pub fn as_discover_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::DiscoverArgs {
        match self {
            Commands::Discover => crate::commands::args::DiscoverArgs { json, color },
            _ => panic!("Expected Discover command"),
        }
    }

    pub fn as_list_portfolios_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::ListPortfoliosArgs {
        match self {
            Commands::ListPortfolios { asset, .. } => crate::commands::args::ListPortfoliosArgs {
                json,
                color,
                filter: asset.clone(),
            },
            _ => panic!("Expected ListPortfolios command"),
        }
    }

    // ========================================================================
    // USER & GROUP COMMAND EXTRACTION METHODS
    // ========================================================================

    pub fn as_get_user_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::GetUserArgs {
        match self {
            Commands::GetUser { .. } => crate::commands::args::GetUserArgs { json, color },
            _ => panic!("Expected GetUser command"),
        }
    }

    pub fn as_update_user_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::UpdateUserArgs {
        match self {
            Commands::UpdateUser {
                user: _,
                name,
                is_active: _,
                photo_url: _,
            } => {
                let (given_name, surname) = if let Some(n) = name {
                    let parts: Vec<&str> = n.splitn(2, ' ').collect();
                    match parts.as_slice() {
                        [first, last] => (Some(first.to_string()), Some(last.to_string())),
                        [only] => (Some(only.to_string()), None),
                        _ => (None, None),
                    }
                } else {
                    (None, None)
                };
                crate::commands::args::UpdateUserArgs {
                    json,
                    color,
                    given_name,
                    surname,
                }
            }
            _ => panic!("Expected UpdateUser command"),
        }
    }

    pub fn as_list_groups_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
        no_resolve: bool,
    ) -> crate::commands::args::ListGroupsArgs {
        match self {
            Commands::ListGroups { group, env } => crate::commands::args::ListGroupsArgs {
                json,
                color,
                filter: group.clone(),
                env: env.clone(),
                no_resolve,
            },
            _ => panic!("Expected ListGroups command"),
        }
    }

    pub fn as_get_group_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::GetGroupArgs {
        match self {
            Commands::GetGroup { .. } => crate::commands::args::GetGroupArgs { json, color },
            _ => panic!("Expected GetGroup command"),
        }
    }

    pub fn as_update_group_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::UpdateGroupArgs {
        match self {
            Commands::UpdateGroup {
                group: _,
                name: _,
                description,
            } => crate::commands::args::UpdateGroupArgs {
                json,
                color,
                env: String::new(),
                description: description.clone(),
            },
            _ => panic!("Expected UpdateGroup command"),
        }
    }

    pub fn as_list_group_members_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::ListGroupMembersArgs {
        match self {
            Commands::ListGroupMembers { group: _ } => {
                crate::commands::args::ListGroupMembersArgs {
                    json,
                    color,
                    env: String::new(),
                }
            }
            _ => panic!("Expected ListGroupMembers command"),
        }
    }

    pub fn as_add_user_to_group_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::UserGroupArgs {
        match self {
            Commands::AddUserToGroup { group: _, user: _ } => {
                crate::commands::args::UserGroupArgs {
                    json,
                    color,
                    env: String::new(),
                }
            }
            _ => panic!("Expected AddUserToGroup command"),
        }
    }

    pub fn as_remove_user_from_group_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::UserGroupArgs {
        match self {
            Commands::RemoveUserFromGroup { group: _, user: _ } => {
                crate::commands::args::UserGroupArgs {
                    json,
                    color,
                    env: String::new(),
                }
            }
            _ => panic!("Expected RemoveUserFromGroup command"),
        }
    }

    // ========================================================================
    // ROLE COMMAND EXTRACTION METHODS
    // ========================================================================

    pub fn as_list_roles_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
        no_resolve: bool,
    ) -> crate::commands::args::ListRolesArgs {
        match self {
            Commands::ListRoles { asset, env } => crate::commands::args::ListRolesArgs {
                json,
                color,
                asset: asset.clone(),
                env: env.clone().unwrap_or_default(),
                no_resolve,
            },
            _ => panic!("Expected ListRoles command"),
        }
    }

    pub fn as_list_role_assignments_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
        no_resolve: bool,
    ) -> crate::commands::args::ListRoleAssignmentsArgs {
        match self {
            Commands::ListRoleAssignments { asset, env, .. } => {
                crate::commands::args::ListRoleAssignmentsArgs {
                    json,
                    color,
                    asset: asset.clone(),
                    env: env.clone().unwrap_or_default(),
                    no_resolve,
                }
            }
            _ => panic!("Expected ListRoleAssignments command"),
        }
    }

    pub fn as_grant_role_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::RoleGrantArgs {
        match self {
            Commands::GrantRole {
                asset,
                env,
                role,
                user,
            } => crate::commands::args::RoleGrantArgs {
                json,
                color,
                role: role.clone(),
                user: user.clone(),
                asset: asset.clone(),
                env: env.clone(),
            },
            _ => panic!("Expected GrantRole command"),
        }
    }

    pub fn as_revoke_role_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::RoleRevokeArgs {
        match self {
            Commands::RevokeRole {
                asset,
                env,
                role,
                user,
            } => crate::commands::args::RoleRevokeArgs {
                json,
                color,
                role: role.clone(),
                user: user.clone(),
                asset: asset.clone(),
                env: env.clone(),
            },
            _ => panic!("Expected RevokeRole command"),
        }
    }

    pub fn as_grant_group_role_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::GroupRoleGrantArgs {
        match self {
            Commands::GrantGroupRole {
                asset,
                env,
                role,
                group: _,
            } => crate::commands::args::GroupRoleGrantArgs {
                json,
                color,
                role: role.clone(),
                group: String::new(),
                asset: asset.clone(),
                env: env.clone(),
            },
            _ => panic!("Expected GrantGroupRole command"),
        }
    }

    pub fn as_revoke_group_role_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::GroupRoleRevokeArgs {
        match self {
            Commands::RevokeGroupRole {
                asset,
                env,
                role,
                group: _,
            } => crate::commands::args::GroupRoleRevokeArgs {
                json,
                color,
                role: role.clone(),
                group: String::new(),
                asset: asset.clone(),
                env: env.clone(),
            },
            _ => panic!("Expected RevokeGroupRole command"),
        }
    }

    // ========================================================================
    // SOURCE CODE COMMAND EXTRACTION METHODS
    // ========================================================================

    pub fn as_download_source_code_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::DownloadSourceCodeArgs {
        match self {
            Commands::DownloadSourceCode {
                revision, output, ..
            } => crate::commands::args::DownloadSourceCodeArgs {
                json,
                color,
                revision: *revision,
                output: output.clone().unwrap_or_default(),
            },
            _ => panic!("Expected DownloadSourceCode command"),
        }
    }

    pub fn as_upload_source_code_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::UploadSourceCodeArgs {
        match self {
            Commands::UploadSourceCode { .. } => {
                crate::commands::args::UploadSourceCodeArgs { json, color }
            }
            _ => panic!("Expected UploadSourceCode command"),
        }
    }

    // ========================================================================
    // BATCH OPERATION COMMAND EXTRACTION METHODS
    // ========================================================================

    pub fn as_batch_deploy_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::BatchDeployArgs {
        match self {
            Commands::BatchDeploy {
                build_type, poll, ..
            } => crate::commands::args::BatchDeployArgs {
                json,
                color,
                poll_interval: poll.poll_interval,
                timeout: poll.timeout,
                build_type: build_type.clone(),
            },
            _ => panic!("Expected BatchDeploy command"),
        }
    }

    pub fn as_batch_undeploy_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::BatchUndeployArgs {
        match self {
            Commands::BatchUndeploy { poll, .. } => crate::commands::args::BatchUndeployArgs {
                json,
                color,
                poll_interval: poll.poll_interval,
                timeout: poll.timeout,
            },
            _ => panic!("Expected BatchUndeploy command"),
        }
    }

    pub fn as_batch_delete_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::BatchDeleteArgs {
        match self {
            Commands::BatchDelete { .. } => crate::commands::args::BatchDeleteArgs { json, color },
            _ => panic!("Expected BatchDelete command"),
        }
    }

    pub fn as_dangerous_batch_undeploy_all_args(
        &self,
        json: bool,
        color: crate::output::ColorMode,
    ) -> crate::commands::args::DangerousBatchUndeployAllArgs {
        match self {
            Commands::DangerousBatchUndeployAll { poll, .. } => {
                crate::commands::args::DangerousBatchUndeployAllArgs {
                    json,
                    color,
                    poll_interval: poll.poll_interval,
                    timeout: poll.timeout,
                }
            }
            _ => panic!("Expected DangerousBatchUndeployAll command"),
        }
    }
}

/// Options bag threaded through to command implementations in `commands.rs`.
/// All commands now use typed argument structs instead of the Options property bag.
#[derive(Debug, Clone)]
pub struct Options {
    pub json: bool,
    pub color: crate::output::ColorMode,
    pub no_resolve: bool,
    pub asset: String,
    pub env: String,
    pub build_type: String,
    pub build_key: String,
    pub filter: String,
    pub asset_type: String,
    pub output: String,
    pub revision: Option<i32>,
    pub offset: Option<i64>,
    pub limit: i64,
    pub interval: Duration,
    pub timeout: Duration,
    pub max_parallel: usize,
    pub max_depth: i32,
    pub no_wait: bool,
    pub continue_on_error: bool,
    pub skip_dependencies: bool,
    pub all_producers: bool,
    pub updates: Map<String, serde_json::Value>,
    // Mentor
    pub session_id: String,
    pub run_id: String,
    pub message: String,
    pub attachment_refs: Vec<String>,
    pub cursor: Option<i64>,
    pub event_id: i64,
    pub mentor_asset_type: String,
    pub name: String,
    pub portfolio_key: String,
    pub description: String,
    pub template_asset_key: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub comment: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            json: false,
            color: crate::output::ColorMode::Auto,
            no_resolve: false,
            asset: String::new(),
            env: String::new(),
            build_type: "Release".to_string(),
            build_key: String::new(),
            filter: String::new(),
            asset_type: String::new(),
            output: String::new(),
            revision: None,
            offset: None,
            limit: 100,
            interval: Duration::from_secs(10),
            timeout: Duration::from_secs(1800),
            max_parallel: 3,
            max_depth: -1,
            no_wait: false,
            continue_on_error: false,
            skip_dependencies: false,
            all_producers: false,
            updates: Map::new(),
            session_id: String::new(),
            run_id: String::new(),
            message: String::new(),
            attachment_refs: Vec::new(),
            cursor: None,
            event_id: 0,
            mentor_asset_type: String::new(),
            name: String::new(),
            portfolio_key: String::new(),
            description: String::new(),
            template_asset_key: String::new(),
            file_name: String::new(),
            size_bytes: 0,
            comment: String::new(),
        }
    }
}

pub fn parse_color(s: &str) -> anyhow::Result<crate::output::ColorMode> {
    match s {
        "auto" => Ok(crate::output::ColorMode::Auto),
        "always" => Ok(crate::output::ColorMode::Always),
        "never" => Ok(crate::output::ColorMode::Never),
        _ => Err(anyhow::anyhow!("--color must be auto, always, or never")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_options_default() {
        let opts = Options::default();
        assert_eq!(opts.build_type, "Release");
        assert_eq!(opts.interval, Duration::from_secs(10));
        assert_eq!(opts.timeout, Duration::from_secs(1800));
        assert_eq!(opts.max_parallel, 3);
    }

    #[test]
    fn test_parse_color() {
        assert!(parse_color("auto").is_ok());
        assert!(parse_color("always").is_ok());
        assert!(parse_color("never").is_ok());
        assert!(parse_color("invalid").is_err());
    }

    #[test]
    fn test_cli_verifies() {
        // Catches any invalid clap definition (duplicate args, conflicting ids, etc.)
        Cli::command().debug_assert();
    }

    #[test]
    fn test_parses_list_assets_with_filter_and_pagination() {
        let cli = Cli::try_parse_from(["odc", "list-assets", "--asset", "eGov", "--offset", "10"])
            .unwrap();
        match cli.command {
            Commands::ListAssets {
                asset: filter,
                app_type,
                offset,
                limit,
            } => {
                assert_eq!(filter, Some("eGov".to_string()));
                assert!(app_type.is_none());
                assert_eq!(offset, Some(10));
                assert_eq!(limit, 100);
            }
            _ => panic!("expected ListAssets"),
        }
    }

    #[test]
    fn test_list_portfolios_requires_flag() {
        // --asset flag is required for portfolios filter
        let cli =
            Cli::try_parse_from(["odc", "list-portfolios", "--asset", "MyPortfolio"]).unwrap();
        match cli.command {
            Commands::ListPortfolios { asset, .. } => {
                assert_eq!(asset, Some("MyPortfolio".to_string()));
            }
            _ => panic!("expected ListPortfolios"),
        }
    }

    #[test]
    fn test_list_deployed_assets_with_filter_and_env() {
        let cli = Cli::try_parse_from([
            "odc",
            "list-deployed-assets",
            "--asset",
            "MyAsset",
            "--env",
            "dev",
        ])
        .unwrap();
        match cli.command {
            Commands::ListDeployedAssets { asset, env, .. } => {
                assert_eq!(asset, Some("MyAsset".to_string()));
                assert_eq!(env, Some("dev".to_string()));
            }
            _ => panic!("expected ListDeployedAssets"),
        }
    }

    #[test]
    fn test_list_roles_without_env() {
        // env is optional for list-roles
        let cli = Cli::try_parse_from(["odc", "list-roles", "MyApp"]).unwrap();
        match cli.command {
            Commands::ListRoles { asset, env } => {
                assert_eq!(asset, "MyApp");
                assert_eq!(env, None);
            }
            _ => panic!("expected ListRoles"),
        }
    }

    #[test]
    fn test_list_roles_with_optional_env() {
        let cli = Cli::try_parse_from(["odc", "list-roles", "MyApp", "--env", "dev"]).unwrap();
        match cli.command {
            Commands::ListRoles { asset, env } => {
                assert_eq!(asset, "MyApp");
                assert_eq!(env, Some("dev".to_string()));
            }
            _ => panic!("expected ListRoles"),
        }
    }

    #[test]
    fn test_list_role_assignments_without_env() {
        let cli = Cli::try_parse_from(["odc", "list-role-assignments", "MyApp"]).unwrap();
        match cli.command {
            Commands::ListRoleAssignments { asset, env, .. } => {
                assert_eq!(asset, "MyApp");
                assert_eq!(env, None);
            }
            _ => panic!("expected ListRoleAssignments"),
        }
    }

    #[test]
    fn test_list_role_assignments_with_optional_env_and_type() {
        let cli = Cli::try_parse_from([
            "odc",
            "list-role-assignments",
            "MyApp",
            "--env",
            "dev",
            "--type",
            "User",
        ])
        .unwrap();
        match cli.command {
            Commands::ListRoleAssignments { asset, env, r#type } => {
                assert_eq!(asset, "MyApp");
                assert_eq!(env, Some("dev".to_string()));
                assert!(r#type.is_some());
            }
            _ => panic!("expected ListRoleAssignments"),
        }
    }

    #[test]
    fn test_list_groups_with_optional_filter_and_env() {
        let cli = Cli::try_parse_from(["odc", "list-groups", "--group", "MyGroup", "--env", "dev"])
            .unwrap();
        match cli.command {
            Commands::ListGroups { group, env } => {
                assert_eq!(group, Some("MyGroup".to_string()));
                assert_eq!(env, Some("dev".to_string()));
            }
            _ => panic!("expected ListGroups"),
        }
    }

    #[test]
    fn test_list_assets_type_filter_rejects_unknown_value() {
        let result = Cli::try_parse_from(["odc", "list-assets", "--type", "Bogus"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_revision_requires_revision_flag() {
        let result = Cli::try_parse_from(["odc", "get-revision", "MyApp"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_global_json_flag_before_subcommand() {
        let cli = Cli::try_parse_from(["odc", "--json", "list-assets"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn test_grant_role_missing_user_is_rejected() {
        let result = Cli::try_parse_from(["odc", "grant-role", "MyApp", "dev", "Admin"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_grant_role_complete() {
        let cli = Cli::try_parse_from([
            "odc",
            "grant-role",
            "MyApp",
            "dev",
            "Admin",
            "user@example.com",
        ])
        .unwrap();
        match cli.command {
            Commands::GrantRole {
                asset,
                env,
                role,
                user,
            } => {
                assert_eq!(asset, "MyApp");
                assert_eq!(env, "dev");
                assert_eq!(role, "Admin");
                assert_eq!(user, "user@example.com");
            }
            _ => panic!("expected GrantRole"),
        }
    }
}
