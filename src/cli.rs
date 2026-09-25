use crate::commands::args::*;
use clap::{CommandFactory, Parser, Subcommand};

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
    #[arg(long, global = true, default_value = "auto")]
    pub color: crate::output::ColorMode,

    /// Disable resolution of external keys (environment names, role names, etc.) in user-friendly output
    #[arg(short = 'n', long, global = true)]
    pub no_resolve: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Recognized asset types, per the asset-repository API's `assetTypes` filter parameter.
///
/// `rename_all = "verbatim"` means each clap possible-value is exactly the variant name, so the
/// enum variants below are spelled to match the API's `assetType` string exactly (including
/// acronym casing like `AIModelConnection`). This lets `as_str()` be `self.into()` via
/// `strum::IntoStaticStr`, without a per-variant `#[value(name = ...)]` attribute.
#[derive(clap::ValueEnum, Debug, Clone, Copy, strum::IntoStaticStr)]
#[value(rename_all = "verbatim")]
pub enum AppType {
    WebApplication,
    MobileApplication,
    LowCodeLibrary,
    ExtensionLibrary,
    ExternalConnection,
    ExternalLibrary,
    Workflow,
    WidgetLibrary,
    AIModelConnection,
    SearchServiceConnection,
    Agent,
    MCPConnection,
    A2AConnection,
    KnowledgeBase,
}

impl AppType {
    /// The exact `assetType` string this variant matches, as returned by the API.
    pub fn as_str(&self) -> &'static str {
        (*self).into()
    }
}

/// Asset types Mentor can create, per `mentor_create_asset`'s `assetType` parameter.
#[derive(clap::ValueEnum, Debug, Clone, Copy, strum::IntoStaticStr)]
#[value(rename_all = "verbatim")]
pub enum MentorAssetType {
    WebApplication,
    Agent,
    Library,
    Workflow,
}

impl MentorAssetType {
    pub fn as_str(&self) -> &'static str {
        (*self).into()
    }
}

/// Role assignee kinds, per `list-role-assignments`'s `--type` filter.
#[derive(clap::ValueEnum, Debug, Clone, Copy, strum::IntoStaticStr)]
#[value(rename_all = "verbatim")]
pub enum AssigneeType {
    User,
    Group,
}

impl AssigneeType {
    pub fn as_str(&self) -> &'static str {
        (*self).into()
    }
}

#[derive(Subcommand, Debug)]
#[command(rename_all = "kebab-case")]
pub enum Commands {
    /// Show the OAuth discovery document (issuer, endpoints, scopes).
    Discover,

    /// Save credentials in ~/.odc/config.json (prompts for client secret).
    Login(LoginArgs),

    /// List portfolios in the tenant.
    ListPortfolios(ListPortfoliosArgs),

    /// List environments in the tenant.
    ListEnvironments,

    /// List assets in the tenant, optionally filtered by name/key and/or type.
    ListAssets(ListAssetsArgs),

    /// List deployed assets, optionally filtered by environment and name/key.
    ListDeployedAssets(ListDeployedAssetsArgs),

    /// Retrieve asset metadata.
    GetAsset(GetAssetArgs),

    /// Print the latest revision number of an asset.
    LatestRevision(LatestRevisionArgs),

    /// List all revisions of an asset.
    ListRevisions(ListRevisionsArgs),

    /// Retrieve a specific asset revision.
    GetRevision(GetRevisionArgs),

    /// Render an asset's producer dependency graph as Mermaid.
    ProducerGraph(ProducerGraphArgs),

    /// Download the OML source code of an asset revision.
    DownloadSourceCode(DownloadSourceCodeArgs),

    /// Upload an OML/XIF file, creating a new asset or revision.
    UploadSourceCode(UploadSourceCodeArgs),

    /// Analyze the impact of deploying an asset revision.
    AnalyzeDeployment(AnalyzeDeploymentArgs),

    /// Analyze the impact of deleting an asset.
    AnalyzeDeletion(AnalyzeDeletionArgs),

    /// Deploy an asset to an environment.
    Deploy(DeployArgs),

    /// Undeploy an asset from an environment.
    Undeploy(UndeployArgs),

    /// Delete an asset.
    DeleteAsset(DeleteAssetArgs),

    /// Deploy multiple assets listed in a file.
    BatchDeploy(BatchDeployArgs),

    /// Undeploy multiple assets listed in a file.
    BatchUndeploy(BatchUndeployArgs),

    /// Delete multiple assets listed in a file.
    BatchDelete(BatchDeleteArgs),

    /// Undeploy all assets from an environment.
    DangerousBatchUndeployAll(DangerousBatchUndeployAllArgs),

    /// Retrieve a user's details.
    GetUser(GetUserArgs),

    /// Update a user's name, active status, or photo URL.
    UpdateUser(UpdateUserArgs),

    /// List application roles defined for an asset.
    ListRoles(ListRolesArgs),

    /// List, for each application role of an asset, the users and/or groups assigned to it.
    ListRoleAssignments(ListRoleAssignmentsArgs),

    /// Grant an application role to a user.
    GrantRole(GrantRoleArgs),

    /// Revoke an application role from a user.
    RevokeRole(RevokeRoleArgs),

    /// List end-user groups, optionally filtered by name and/or environment.
    ListGroups(ListGroupsArgs),

    /// Retrieve an end-user group's details.
    GetGroup(GetGroupArgs),

    /// Update a group's name or description.
    UpdateGroup(UpdateGroupArgs),

    /// List the members of an end-user group.
    ListGroupMembers(ListGroupMembersArgs),

    /// Add a user to an end-user group.
    AddUserToGroup(AddUserToGroupArgs),

    /// Remove a user from an end-user group.
    RemoveUserFromGroup(RemoveUserFromGroupArgs),

    /// Grant an application role to an end-user group.
    GrantGroupRole(GrantGroupRoleArgs),

    /// Revoke an application role from an end-user group.
    RevokeGroupRole(RevokeGroupRoleArgs),

    /// Start a build for an asset revision.
    InternalBuild(InternalBuildArgs),

    /// Publish a build to an environment.
    InternalPublish(InternalPublishArgs),

    /// Deploy an existing build to an environment.
    InternalDeploy(InternalDeployArgs),

    /// Start a new Mentor session; prints the sessionId used for follow-up commands.
    MentorStartSession,

    /// Create a new asset in a Mentor session, so `mentor` can edit it.
    MentorCreateAsset(MentorCreateAssetArgs),

    /// Load an existing asset into a Mentor session, so `mentor` can edit it.
    MentorLoadAsset(MentorLoadAssetArgs),

    /// Send a prompt to Mentor and wait for completion, auto-publishing the result; or start interactive mode if no prompt is given.
    Mentor(MentorArgs),

    /// Send a prompt to a Mentor session; returns a runId to poll with `mentor-get-run` (low-level).
    MentorPrompt(MentorPromptArgs),

    /// Poll a Mentor run's progress events and status.
    MentorGetRun(MentorGetRunArgs),

    /// Fetch the full body of a truncated Mentor run event.
    MentorGetEvent(MentorGetEventArgs),

    /// Cancel the in-flight prompt for a Mentor session.
    MentorCancelPrompt(MentorCancelPromptArgs),

    /// Close a Mentor session and release its resources.
    MentorCloseSession(MentorCloseSessionArgs),

    /// Mint a presigned upload URL for a Mentor session attachment.
    MentorRequestUpload(MentorRequestUploadArgs),

    /// Publish the asset loaded in a Mentor session to the connected dev environment.
    MentorPublish(MentorPublishArgs),

    /// Generate shell completion scripts.
    Completion(CompletionArgs),
}

#[cfg(test)]
mod tests {
    use super::*;

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
            Commands::ListAssets(args) => {
                assert_eq!(args.asset, Some("eGov".to_string()));
                assert!(args.app_type.is_none());
                assert_eq!(args.offset, Some(10));
                assert_eq!(args.limit, 100);
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
            Commands::ListPortfolios(args) => {
                assert_eq!(args.asset, Some("MyPortfolio".to_string()));
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
            Commands::ListDeployedAssets(args) => {
                assert_eq!(args.asset, Some("MyAsset".to_string()));
                assert_eq!(args.env, Some("dev".to_string()));
            }
            _ => panic!("expected ListDeployedAssets"),
        }
    }

    #[test]
    fn test_list_roles_without_env() {
        // env is optional for list-roles
        let cli = Cli::try_parse_from(["odc", "list-roles", "MyApp"]).unwrap();
        match cli.command {
            Commands::ListRoles(args) => {
                assert_eq!(args.asset, "MyApp");
                assert_eq!(args.env, None);
            }
            _ => panic!("expected ListRoles"),
        }
    }

    #[test]
    fn test_list_roles_with_optional_env() {
        let cli = Cli::try_parse_from(["odc", "list-roles", "MyApp", "--env", "dev"]).unwrap();
        match cli.command {
            Commands::ListRoles(args) => {
                assert_eq!(args.asset, "MyApp");
                assert_eq!(args.env, Some("dev".to_string()));
            }
            _ => panic!("expected ListRoles"),
        }
    }

    #[test]
    fn test_list_role_assignments_without_env() {
        let cli = Cli::try_parse_from(["odc", "list-role-assignments", "MyApp"]).unwrap();
        match cli.command {
            Commands::ListRoleAssignments(args) => {
                assert_eq!(args.asset, "MyApp");
                assert_eq!(args.env, None);
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
            Commands::ListRoleAssignments(args) => {
                assert_eq!(args.asset, "MyApp");
                assert_eq!(args.env, Some("dev".to_string()));
                assert!(args.r#type.is_some());
            }
            _ => panic!("expected ListRoleAssignments"),
        }
    }

    #[test]
    fn test_list_groups_with_optional_filter_and_env() {
        let cli = Cli::try_parse_from(["odc", "list-groups", "--group", "MyGroup", "--env", "dev"])
            .unwrap();
        match cli.command {
            Commands::ListGroups(args) => {
                assert_eq!(args.group, Some("MyGroup".to_string()));
                assert_eq!(args.env, Some("dev".to_string()));
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
            Commands::GrantRole(args) => {
                assert_eq!(args.asset, "MyApp");
                assert_eq!(args.env, "dev");
                assert_eq!(args.role, "Admin");
                assert_eq!(args.user, "user@example.com");
            }
            _ => panic!("expected GrantRole"),
        }
    }

    #[test]
    fn test_all_app_type_variants_round_trip_to_expected_api_string() {
        let cases: &[(&str, &str)] = &[
            ("WebApplication", "WebApplication"),
            ("MobileApplication", "MobileApplication"),
            ("LowCodeLibrary", "LowCodeLibrary"),
            ("ExtensionLibrary", "ExtensionLibrary"),
            ("ExternalConnection", "ExternalConnection"),
            ("ExternalLibrary", "ExternalLibrary"),
            ("Workflow", "Workflow"),
            ("WidgetLibrary", "WidgetLibrary"),
            ("AIModelConnection", "AIModelConnection"),
            ("SearchServiceConnection", "SearchServiceConnection"),
            ("Agent", "Agent"),
            ("MCPConnection", "MCPConnection"),
            ("A2AConnection", "A2AConnection"),
            ("KnowledgeBase", "KnowledgeBase"),
        ];
        for (clap_name, api_str) in cases {
            let cli = Cli::try_parse_from(["odc", "list-assets", "--type", clap_name])
                .unwrap_or_else(|e| panic!("--type {clap_name} should parse: {e}"));
            match cli.command {
                Commands::ListAssets(args) => {
                    assert_eq!(args.app_type.unwrap().as_str(), *api_str, "for {clap_name}");
                }
                _ => panic!("expected ListAssets"),
            }
        }
    }

    #[test]
    fn test_color_mode_parses_all_values() {
        for value in ["auto", "always", "never"] {
            let cli = Cli::try_parse_from(["odc", "--color", value, "list-assets"])
                .unwrap_or_else(|e| panic!("--color {value} should parse: {e}"));
            let _ = cli.color;
        }
        assert!(Cli::try_parse_from(["odc", "--color", "bogus", "list-assets"]).is_err());
    }
}
