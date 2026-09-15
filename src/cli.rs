use clap::{CommandFactory, Parser, Subcommand};
use serde_json::Map;
use std::time::Duration;

/// Command names grouped by category, in display order, for the categorized top-level help.
const HELP_CATEGORIES: &[(&str, &[&str])] = &[
    ("Auth", &["discover", "login", "login-mentor"]),
    (
        "Apps & Environments",
        &[
            "list-environments",
            "list-apps",
            "list-deployed-apps",
            "get-app",
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
            "delete-app",
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
    (
        "Users & Roles",
        &["get-user", "update-user", "grant-role", "revoke-role"],
    ),
    (
        "Internal (Advanced)",
        &["internal-build", "internal-publish", "internal-deploy"],
    ),
    (
        "Mentor",
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
    println!("      --json           Output in JSON format");
    println!("      --color <COLOR>  Control color output [default: auto]");
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

    #[command(subcommand)]
    pub command: Commands,
}

fn parse_color_arg(s: &str) -> Result<crate::output::ColorMode, String> {
    parse_color(s).map_err(|e| e.to_string())
}

/// Recognized asset types, per the asset-repository API's `assetTypes` filter parameter.
#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum AppType {
    #[value(name = "WebApplication")]
    WebApplication,
    #[value(name = "MobileApplication")]
    MobileApplication,
    #[value(name = "LowCodeLibrary")]
    LowCodeLibrary,
    #[value(name = "ExtensionLibrary")]
    ExtensionLibrary,
    #[value(name = "ExternalConnection")]
    ExternalConnection,
    #[value(name = "ExternalLibrary")]
    ExternalLibrary,
    #[value(name = "Workflow")]
    Workflow,
    #[value(name = "WidgetLibrary")]
    WidgetLibrary,
    #[value(name = "AIModelConnection")]
    AiModelConnection,
    #[value(name = "SearchServiceConnection")]
    SearchServiceConnection,
    #[value(name = "Agent")]
    Agent,
    #[value(name = "MCPConnection")]
    McpConnection,
    #[value(name = "A2AConnection")]
    A2aConnection,
    #[value(name = "KnowledgeBase")]
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
#[derive(clap::ValueEnum, Debug, Clone, Copy)]
pub enum MentorAssetType {
    #[value(name = "WebApplication")]
    WebApplication,
    #[value(name = "Agent")]
    Agent,
    #[value(name = "Library")]
    Library,
    #[value(name = "Workflow")]
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

/// Shared flags for commands that run multiple apps in parallel.
#[derive(clap::Args, Debug, Clone)]
pub struct ParallelArgs {
    /// Maximum apps to process concurrently
    #[arg(long, default_value_t = 3)]
    pub max_parallel: usize,
    /// Keep going on remaining apps if one fails, instead of stopping
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

    /// List environments in the tenant.
    ListEnvironments,

    /// List apps in the tenant, optionally filtered by name/key and/or type.
    ListApps {
        /// Filter to apps whose name or key contains this (case-insensitive)
        filter: Option<String>,
        /// Filter to apps of this type
        #[arg(long = "type")]
        app_type: Option<AppType>,
        /// Fetch a single page starting at this result index (default: fetch every page)
        #[arg(long)]
        offset: Option<i64>,
        /// Page size to request from the API
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },

    /// List deployed apps, optionally filtered by environment and name/key.
    ListDeployedApps {
        /// Filter to apps whose name or key contains this (case-insensitive)
        filter: Option<String>,
        /// Environment name, key, or unambiguous partial name
        #[arg(long)]
        env: Option<String>,
        #[arg(long)]
        offset: Option<i64>,
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },

    /// Retrieve app metadata.
    GetApp { app: String },

    /// Print the latest revision number of an app.
    LatestRevision { app: String },

    /// List all revisions of an app.
    ListRevisions {
        app: String,
        #[arg(long)]
        offset: Option<i64>,
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },

    /// Retrieve a specific app revision.
    GetRevision {
        app: String,
        /// Revision number to retrieve
        #[arg(long)]
        revision: i32,
    },

    /// Render an app's producer dependency graph as Mermaid.
    ProducerGraph {
        app: String,
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
        /// Output path; defaults to producer-graph-<app>-rev-<revision>.mmd
        #[arg(long)]
        output: Option<String>,
    },

    /// Download the OML source code of an app revision.
    DownloadSourceCode {
        app: String,
        /// Defaults to the latest revision
        #[arg(long)]
        revision: Option<i32>,
        /// Output file path; defaults to <app-key>-rev-<revision>.oml
        #[arg(long)]
        output: Option<String>,
    },

    /// Upload an OML/XIF file, creating a new asset or revision.
    UploadSourceCode { oml_file: String },

    /// Analyze the impact of deploying an app revision.
    AnalyzeDeployment {
        #[arg(long)]
        app: String,
        #[arg(long)]
        env: String,
        /// Defaults to the latest revision
        #[arg(long)]
        revision: Option<i32>,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Analyze the impact of deleting an app.
    AnalyzeDeletion {
        #[arg(long)]
        app: String,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Deploy an app to an environment.
    Deploy {
        #[arg(long)]
        app: String,
        #[arg(long)]
        env: String,
        /// Defaults to the app's current revision (falls back to the latest)
        #[arg(long)]
        revision: Option<i32>,
        /// Debug or Release
        #[arg(long, default_value = "Release")]
        build_type: String,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Undeploy an app from an environment.
    Undeploy {
        #[arg(long)]
        app: String,
        #[arg(long)]
        env: String,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Delete an app.
    DeleteApp {
        #[arg(long)]
        app: String,
    },

    /// Deploy multiple apps listed in a file.
    BatchDeploy {
        apps_file: String,
        #[arg(long)]
        env: String,
        /// Debug or Release
        #[arg(long, default_value = "Release")]
        build_type: String,
        /// Deploy only the apps listed in the file, without their producer dependencies
        #[arg(long)]
        skip_dependencies: bool,
        #[command(flatten)]
        poll: PollArgs,
        #[command(flatten)]
        parallel: ParallelArgs,
    },

    /// Undeploy multiple apps listed in a file.
    BatchUndeploy {
        apps_file: String,
        #[arg(long)]
        env: String,
        #[command(flatten)]
        poll: PollArgs,
        #[command(flatten)]
        parallel: ParallelArgs,
    },

    /// Delete multiple apps listed in a file.
    BatchDelete {
        apps_file: String,
        #[command(flatten)]
        parallel: ParallelArgs,
    },

    /// Undeploy all apps from an environment.
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

    /// Grant an application role to a user.
    GrantRole {
        /// App the role belongs to (name or key)
        app: String,
        /// Role name or key
        role: String,
        /// User key or email
        user: String,
    },

    /// Revoke an application role from a user.
    RevokeRole {
        app: String,
        role: String,
        user: String,
    },

    /// Start a build for an app revision.
    InternalBuild {
        #[arg(long)]
        app: String,
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
        app: String,
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
        app: String,
        #[arg(long)]
        env: String,
        #[arg(long)]
        revision: Option<i32>,
        #[arg(long)]
        build_key: String,
        #[command(flatten)]
        poll: PollArgs,
    },

    /// Save Mentor's OAuth2 client credentials (prompts for client secret).
    LoginMentor {
        token_url: String,
        client_id: String,
    },

    /// Start a new Mentor session; prints the sessionId used for follow-up commands.
    MentorStartSession,

    /// Create a new asset in a Mentor session, so `mentor-prompt` can edit it.
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

    /// Load an existing app into a Mentor session, so `mentor-prompt` can edit it.
    MentorLoadAsset {
        session_id: String,
        asset_key: String,
        /// Defaults to the latest revision
        #[arg(long)]
        revision: Option<i32>,
    },

    /// Send a prompt to a Mentor session; returns a runId to poll with `mentor-get-run`.
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
    /// The kebab-case command name used for dispatch (matches `commands::execute`).
    pub fn name(&self) -> &'static str {
        match self {
            Commands::Discover => "discover",
            Commands::Login { .. } => "login",
            Commands::ListEnvironments => "list-environments",
            Commands::ListApps { .. } => "list-apps",
            Commands::ListDeployedApps { .. } => "list-deployed-apps",
            Commands::GetApp { .. } => "get-app",
            Commands::LatestRevision { .. } => "latest-revision",
            Commands::ListRevisions { .. } => "list-revisions",
            Commands::GetRevision { .. } => "get-revision",
            Commands::ProducerGraph { .. } => "producer-graph",
            Commands::DownloadSourceCode { .. } => "download-source-code",
            Commands::UploadSourceCode { .. } => "upload-source-code",
            Commands::AnalyzeDeployment { .. } => "analyze-deployment",
            Commands::AnalyzeDeletion { .. } => "analyze-deletion",
            Commands::Deploy { .. } => "deploy",
            Commands::Undeploy { .. } => "undeploy",
            Commands::DeleteApp { .. } => "delete-app",
            Commands::BatchDeploy { .. } => "batch-deploy",
            Commands::BatchUndeploy { .. } => "batch-undeploy",
            Commands::BatchDelete { .. } => "batch-delete",
            Commands::DangerousBatchUndeployAll { .. } => "dangerous-batch-undeploy-all",
            Commands::GetUser { .. } => "get-user",
            Commands::UpdateUser { .. } => "update-user",
            Commands::GrantRole { .. } => "grant-role",
            Commands::RevokeRole { .. } => "revoke-role",
            Commands::InternalBuild { .. } => "internal-build",
            Commands::InternalPublish { .. } => "internal-publish",
            Commands::InternalDeploy { .. } => "internal-deploy",
            Commands::Completion { .. } => "completion",
            Commands::LoginMentor { .. } => "login-mentor",
            Commands::MentorStartSession => "mentor-start-session",
            Commands::MentorCreateAsset { .. } => "mentor-create-asset",
            Commands::MentorLoadAsset { .. } => "mentor-load-asset",
            Commands::MentorPrompt { .. } => "mentor-prompt",
            Commands::MentorGetRun { .. } => "mentor-get-run",
            Commands::MentorGetEvent { .. } => "mentor-get-event",
            Commands::MentorCancelPrompt { .. } => "mentor-cancel-prompt",
            Commands::MentorCloseSession { .. } => "mentor-close-session",
            Commands::MentorRequestUpload { .. } => "mentor-request-upload",
            Commands::MentorPublish { .. } => "mentor-publish",
        }
    }

    /// Convert the parsed command into the `(Options, positionals)` shape that
    /// `commands::execute` dispatches on, so command implementations don't need to change.
    pub fn into_dispatch(self) -> (Options, Vec<String>) {
        let mut options = Options::default();
        let mut positionals = Vec::new();

        match self {
            Commands::Discover | Commands::ListEnvironments | Commands::Completion { .. } => {}
            Commands::Login {
                tenant_url,
                client_id,
            } => {
                positionals = vec![tenant_url, client_id];
            }
            Commands::ListApps {
                filter,
                app_type,
                offset,
                limit,
            } => {
                options.offset = offset;
                options.limit = limit;
                if let Some(t) = app_type {
                    options.app_type = t.as_str().to_string();
                }
                if let Some(f) = filter {
                    positionals.push(f);
                }
            }
            Commands::ListDeployedApps {
                filter,
                env,
                offset,
                limit,
            } => {
                options.offset = offset;
                options.limit = limit;
                options.env = env.unwrap_or_default();
                if let Some(f) = filter {
                    positionals.push(f);
                }
            }
            Commands::GetApp { app } | Commands::LatestRevision { app } => {
                positionals = vec![app];
            }
            Commands::ListRevisions { app, offset, limit } => {
                options.offset = offset;
                options.limit = limit;
                positionals = vec![app];
            }
            Commands::GetRevision { app, revision } => {
                options.revision = Some(revision);
                positionals = vec![app];
            }
            Commands::ProducerGraph {
                app,
                revision,
                env,
                max_depth,
                producer_type_filter,
                all_producers,
                output,
            } => {
                options.revision = revision;
                options.env = env.unwrap_or_default();
                options.max_depth = max_depth;
                options.filter = producer_type_filter;
                options.all_producers = all_producers;
                options.output = output.unwrap_or_default();
                positionals = vec![app];
            }
            Commands::DownloadSourceCode {
                app,
                revision,
                output,
            } => {
                options.revision = revision;
                options.output = output.unwrap_or_default();
                positionals = vec![app];
            }
            Commands::UploadSourceCode { oml_file } => {
                positionals = vec![oml_file];
            }
            Commands::AnalyzeDeployment {
                app,
                env,
                revision,
                poll,
            } => {
                options.app = app;
                options.env = env;
                options.revision = revision;
                apply_poll(&mut options, poll);
            }
            Commands::AnalyzeDeletion { app, poll } => {
                options.app = app;
                apply_poll(&mut options, poll);
            }
            Commands::Deploy {
                app,
                env,
                revision,
                build_type,
                poll,
            } => {
                options.app = app;
                options.env = env;
                options.revision = revision;
                options.build_type = build_type;
                apply_poll(&mut options, poll);
            }
            Commands::Undeploy { app, env, poll } => {
                options.app = app;
                options.env = env;
                apply_poll(&mut options, poll);
            }
            Commands::DeleteApp { app } => {
                options.app = app;
            }
            Commands::BatchDeploy {
                apps_file,
                env,
                build_type,
                skip_dependencies,
                poll,
                parallel,
            } => {
                options.env = env;
                options.build_type = build_type;
                options.skip_dependencies = skip_dependencies;
                apply_poll(&mut options, poll);
                apply_parallel(&mut options, parallel);
                positionals = vec![apps_file];
            }
            Commands::BatchUndeploy {
                apps_file,
                env,
                poll,
                parallel,
            } => {
                options.env = env;
                apply_poll(&mut options, poll);
                apply_parallel(&mut options, parallel);
                positionals = vec![apps_file];
            }
            Commands::BatchDelete {
                apps_file,
                parallel,
            } => {
                apply_parallel(&mut options, parallel);
                positionals = vec![apps_file];
            }
            Commands::DangerousBatchUndeployAll {
                env,
                poll,
                parallel,
            } => {
                options.env = env;
                apply_poll(&mut options, poll);
                apply_parallel(&mut options, parallel);
            }
            Commands::GetUser { user } => {
                positionals = vec![user];
            }
            Commands::UpdateUser {
                user,
                name,
                is_active,
                photo_url,
            } => {
                positionals = vec![user];
                if let Some(name) = name {
                    options.updates.insert("name".to_string(), name.into());
                }
                if let Some(is_active) = is_active {
                    options
                        .updates
                        .insert("isActive".to_string(), is_active.into());
                }
                if let Some(photo_url) = photo_url {
                    options
                        .updates
                        .insert("photoUrl".to_string(), photo_url.into());
                }
            }
            Commands::GrantRole { app, role, user } | Commands::RevokeRole { app, role, user } => {
                options.app = app;
                positionals = vec![user, role];
            }
            Commands::InternalBuild {
                app,
                env,
                revision,
                build_type,
                poll,
            } => {
                options.app = app;
                options.env = env;
                options.revision = revision;
                options.build_type = build_type;
                apply_poll(&mut options, poll);
            }
            Commands::InternalPublish {
                app,
                env,
                revision,
                poll,
            } => {
                options.app = app;
                options.env = env;
                options.revision = revision;
                apply_poll(&mut options, poll);
            }
            Commands::InternalDeploy {
                app,
                env,
                revision,
                build_key,
                poll,
            } => {
                options.app = app;
                options.env = env;
                options.revision = revision;
                options.build_key = build_key;
                apply_poll(&mut options, poll);
            }
            Commands::LoginMentor {
                token_url,
                client_id,
            } => {
                positionals = vec![token_url, client_id];
            }
            Commands::MentorStartSession => {}
            Commands::MentorCreateAsset {
                session_id,
                asset_type,
                name,
                portfolio_key,
                description,
                template_asset_key,
            } => {
                options.session_id = session_id;
                options.mentor_asset_type = asset_type.as_str().to_string();
                options.name = name;
                options.portfolio_key = portfolio_key;
                options.description = description.unwrap_or_default();
                options.template_asset_key = template_asset_key.unwrap_or_default();
            }
            Commands::MentorLoadAsset {
                session_id,
                asset_key,
                revision,
            } => {
                options.session_id = session_id;
                options.revision = revision;
                positionals = vec![asset_key];
            }
            Commands::MentorPrompt {
                session_id,
                message,
                attachment_refs,
            } => {
                options.session_id = session_id;
                options.message = message;
                options.attachment_refs = attachment_refs;
            }
            Commands::MentorGetRun {
                session_id,
                run_id,
                cursor,
            } => {
                options.session_id = session_id;
                options.run_id = run_id;
                options.cursor = cursor;
            }
            Commands::MentorGetEvent {
                session_id,
                run_id,
                event_id,
            } => {
                options.session_id = session_id;
                options.run_id = run_id;
                options.event_id = event_id;
            }
            Commands::MentorCancelPrompt { session_id, run_id } => {
                options.session_id = session_id;
                options.run_id = run_id;
            }
            Commands::MentorCloseSession { session_id } => {
                options.session_id = session_id;
            }
            Commands::MentorRequestUpload {
                session_id,
                file_name,
                size_bytes,
            } => {
                options.session_id = session_id;
                options.file_name = file_name;
                options.size_bytes = size_bytes;
            }
            Commands::MentorPublish {
                session_id,
                comment,
            } => {
                options.session_id = session_id;
                options.comment = comment.unwrap_or_default();
            }
        }

        (options, positionals)
    }
}

fn apply_poll(options: &mut Options, poll: PollArgs) {
    options.interval = Duration::from_secs(poll.poll_interval);
    options.timeout = Duration::from_secs(poll.timeout);
    options.no_wait = poll.no_wait;
}

fn apply_parallel(options: &mut Options, parallel: ParallelArgs) {
    options.max_parallel = parallel.max_parallel;
    options.continue_on_error = parallel.continue_on_error;
}

/// Options bag threaded through to command implementations in `commands.rs`. Built from a
/// parsed `Commands` variant via `Commands::into_dispatch`.
#[derive(Debug, Clone)]
pub struct Options {
    pub json: bool,
    pub color: crate::output::ColorMode,
    pub app: String,
    pub env: String,
    pub build_type: String,
    pub build_key: String,
    pub filter: String,
    pub app_type: String,
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
            app: String::new(),
            env: String::new(),
            build_type: "Release".to_string(),
            build_key: String::new(),
            filter: String::new(),
            app_type: String::new(),
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
    fn test_parses_list_apps_with_filter_and_pagination() {
        let cli = Cli::try_parse_from(["odc", "list-apps", "eGov", "--offset", "10"]).unwrap();
        match cli.command {
            Commands::ListApps {
                filter,
                app_type,
                offset,
                limit,
            } => {
                assert_eq!(filter, Some("eGov".to_string()));
                assert!(app_type.is_none());
                assert_eq!(offset, Some(10));
                assert_eq!(limit, 100);
            }
            _ => panic!("expected ListApps"),
        }
    }

    #[test]
    fn test_parses_list_deployed_apps_env() {
        let cli =
            Cli::try_parse_from(["odc", "list-deployed-apps", "eGov", "--env", "prod"]).unwrap();
        let (options, positionals) = cli.command.into_dispatch();
        assert_eq!(options.env, "prod");
        assert_eq!(positionals, vec!["eGov".to_string()]);
    }

    #[test]
    fn test_list_apps_type_filter_maps_to_exact_api_string() {
        let cli = Cli::try_parse_from(["odc", "list-apps", "--type", "Agent"]).unwrap();
        let (options, _) = cli.command.into_dispatch();
        assert_eq!(options.app_type, "Agent");
    }

    #[test]
    fn test_list_apps_type_filter_rejects_unknown_value() {
        let result = Cli::try_parse_from(["odc", "list-apps", "--type", "Bogus"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_apps_without_type_leaves_app_type_empty() {
        let cli = Cli::try_parse_from(["odc", "list-apps"]).unwrap();
        let (options, _) = cli.command.into_dispatch();
        assert_eq!(options.app_type, "");
    }

    #[test]
    fn test_get_revision_requires_revision_flag() {
        let result = Cli::try_parse_from(["odc", "get-revision", "MyApp"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_global_json_flag_before_subcommand() {
        let cli = Cli::try_parse_from(["odc", "--json", "list-apps"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn test_command_name_matches_dispatch() {
        let cli = Cli::try_parse_from(["odc", "get-user", "demo@example.com"]).unwrap();
        assert_eq!(cli.command.name(), "get-user");
    }

    #[test]
    fn test_grant_role_requires_app_role_user_in_order() {
        let cli = Cli::try_parse_from(["odc", "grant-role", "MyApp", "Admin", "demo@example.com"])
            .unwrap();
        let (options, positionals) = cli.command.into_dispatch();
        assert_eq!(options.app, "MyApp");
        assert_eq!(
            positionals,
            vec!["demo@example.com".to_string(), "Admin".to_string()]
        );
    }

    #[test]
    fn test_grant_role_missing_user_is_rejected() {
        let result = Cli::try_parse_from(["odc", "grant-role", "MyApp", "Admin"]);
        assert!(result.is_err());
    }
}
