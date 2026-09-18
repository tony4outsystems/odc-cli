//! Command execution handlers for ODC CLI.
//!
//! This module dispatches CLI commands to their implementation handlers organized by domain:
//! - [`auth`]: authentication and portfolio commands
//! - [`assets`]: asset listing and details
//! - [`environments`]: environment listing
//! - [`revisions`]: revision management and producer graphs
//! - [`deployment`]: deployment operations (analyze, build, deploy, undeploy, delete)
//! - [`source_code`]: source code download/upload
//! - [`users`]: user and group management
//! - [`roles`]: role assignment and management
//! - [`mentor`]: AI mentor integration
//! - [`shared`]: common utilities (listing, filtering, resolution)

pub mod args;
pub mod assets;
pub mod auth;
pub mod deployment;
pub mod environments;
pub mod mentor;
pub mod revisions;
pub mod roles;
pub mod shared;
pub mod source_code;
pub mod users;

use crate::cli::{Commands, Options};
use anyhow::Result;

/// Execute a command by matching on the Commands enum directly, dispatching to the appropriate module.
pub async fn execute(command: &Commands, options: &Options, positionals: &[String]) -> Result<()> {
    use Commands::*;

    match command {
        // Auth & Portfolios
        Discover => auth::cmd_discover(options).await,
        ListPortfolios { .. } => auth::cmd_list_portfolios(options, positionals).await,

        // Assets & Environments (now using typed args)
        ListAssets { .. } => {
            let args = command.as_list_assets_args(options.json, options.color);
            assets::cmd_list_assets(args, positionals).await
        }
        ListDeployedAssets { .. } => {
            let args = command.as_list_deployed_assets_args(options.json, options.color);
            assets::cmd_list_deployed_assets(args, positionals).await
        }
        GetAsset { .. } => {
            let args = command.as_get_asset_args(options.json, options.color);
            assets::cmd_get_asset(args, positionals).await
        }
        ListEnvironments => {
            let args = command.as_list_environments_args(options.json, options.color);
            environments::cmd_list_environments(args).await
        }

        // Revisions
        LatestRevision { .. } => revisions::cmd_latest_revision(options, positionals).await,
        ListRevisions { .. } => revisions::cmd_list_revisions(options, positionals).await,
        GetRevision { .. } => revisions::cmd_get_revision(options, positionals).await,
        ProducerGraph { .. } => revisions::cmd_producer_graph(options, positionals).await,

        // Deployment (now using typed args)
        AnalyzeDeployment { .. } => {
            let args = command.as_deployment_analysis_args(options.json, options.color);
            deployment::cmd_analyze_deployment(args).await
        }
        AnalyzeDeletion { .. } => {
            let args = command.as_deletion_analysis_args(options.json, options.color);
            deployment::cmd_analyze_deletion(args).await
        }
        InternalBuild { .. } => {
            let args = command.as_build_args(options.json, options.color);
            deployment::cmd_internal_build(args).await
        }
        InternalPublish { .. } => {
            let args = command.as_publish_args(options.json, options.color);
            deployment::cmd_internal_publish(args).await
        }
        InternalDeploy { .. } => {
            let args = command.as_internal_deploy_args(options.json, options.color);
            deployment::cmd_internal_deploy(args).await
        }
        Deploy { .. } => {
            let args = command.as_deploy_args(options.json, options.color);
            deployment::cmd_deploy(args).await
        }
        Undeploy { .. } => {
            let args = command.as_undeploy_args(options.json, options.color);
            deployment::cmd_undeploy(args).await
        }
        DeleteAsset { .. } => {
            let args = command.as_delete_asset_args(options.json, options.color);
            deployment::cmd_delete_asset(args).await
        }

        // Batch operations (handled by workflows module)
        BatchDeploy { .. } => {
            shared::require_positional(positionals, "batch-deploy", "an assets file")?;
            crate::workflows::batch_deploy(options, &positionals[0]).await
        }
        BatchUndeploy { .. } => {
            shared::require_positional(positionals, "batch-undeploy", "an assets file")?;
            crate::workflows::batch_undeploy(options, &positionals[0]).await
        }
        BatchDelete { .. } => {
            shared::require_positional(positionals, "batch-delete", "an assets file")?;
            crate::workflows::batch_delete(options, &positionals[0]).await
        }
        DangerousBatchUndeployAll { .. } => {
            crate::workflows::dangerous_batch_undeploy_all(options).await
        }

        // Source Code
        DownloadSourceCode { .. } => source_code::cmd_download_source_code(options, positionals).await,
        UploadSourceCode { .. } => source_code::cmd_upload_source_code(options, positionals).await,

        // Users & Groups
        GetUser { .. } => users::cmd_get_user(options, positionals).await,
        UpdateUser { .. } => users::cmd_update_user(options, positionals).await,
        ListGroups { .. } => users::cmd_list_groups(options, positionals).await,
        GetGroup { .. } => users::cmd_get_group(options, positionals).await,
        UpdateGroup { .. } => users::cmd_update_group(options, positionals).await,
        ListGroupMembers { .. } => users::cmd_list_group_members(options, positionals).await,
        AddUserToGroup { .. } => users::cmd_add_user_to_group(options, positionals).await,
        RemoveUserFromGroup { .. } => users::cmd_remove_user_from_group(options, positionals).await,

        // Roles
        ListRoles { .. } => roles::cmd_list_roles(options, positionals).await,
        ListRoleAssignments { .. } => roles::cmd_list_role_assignments(options, positionals).await,
        GrantRole { .. } => roles::cmd_grant_role(options, positionals).await,
        RevokeRole { .. } => roles::cmd_revoke_role(options, positionals).await,
        GrantGroupRole { .. } => roles::cmd_grant_group_role(options, positionals).await,
        RevokeGroupRole { .. } => roles::cmd_revoke_group_role(options, positionals).await,

        // Mentor (now using typed args)
        MentorStartSession => {
            let args = command.as_mentor_start_session_args(options.json, options.color);
            mentor::cmd_mentor_start_session(args).await
        }
        MentorCreateAsset { .. } => {
            let args = command.as_mentor_create_asset_args(options.json, options.color);
            mentor::cmd_mentor_create_asset(args).await
        }
        MentorLoadAsset { .. } => {
            let args = command.as_mentor_load_asset_args(options.json, options.color);
            mentor::cmd_mentor_load_asset(args, positionals).await
        }
        MentorPrompt { .. } => {
            let args = command.as_mentor_prompt_args(options.json, options.color);
            mentor::cmd_mentor_prompt(args).await
        }
        MentorGetRun { .. } => {
            let args = command.as_mentor_get_run_args(options.json, options.color);
            mentor::cmd_mentor_get_run(args).await
        }
        MentorGetEvent { .. } => {
            let args = command.as_mentor_get_event_args(options.json, options.color);
            mentor::cmd_mentor_get_event(args).await
        }
        MentorCancelPrompt { .. } => {
            let args = command.as_mentor_cancel_prompt_args(options.json, options.color);
            mentor::cmd_mentor_cancel_prompt(args).await
        }
        MentorCloseSession { .. } => {
            let args = command.as_mentor_close_session_args(options.json, options.color);
            mentor::cmd_mentor_close_session(args).await
        }
        MentorRequestUpload { .. } => {
            let args = command.as_mentor_request_upload_args(options.json, options.color);
            mentor::cmd_mentor_request_upload(args).await
        }
        MentorPublish { .. } => {
            let args = command.as_mentor_publish_args(options.json, options.color);
            mentor::cmd_mentor_publish(args).await
        }

        // These are handled specially and should not reach here
        Login { .. } => Err(anyhow::anyhow!(
            "Login should be handled in main run() function"
        )),
        Completion { .. } => Err(anyhow::anyhow!(
            "Completion should be handled in main run() function"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    #[test]
    fn test_asset_table_columns_drop_wide_fields() {
        use serde_json::Map;

        let mut asset = Map::new();
        asset.insert("name".to_string(), serde_json::json!("MyAsset"));
        asset.insert("assetKey".to_string(), serde_json::json!("guid-1"));
        asset.insert("assetType".to_string(), serde_json::json!("WebApplication"));
        asset.insert("revision".to_string(), serde_json::json!(3));
        asset.insert("tag".to_string(), serde_json::json!("1.0.0"));
        asset.insert("modelDigest".to_string(), serde_json::json!("digest-guid"));
        asset.insert(
            "description".to_string(),
            serde_json::json!("a very long description"),
        );

        let compacted = crate::value::compact_map(&asset, shared::ASSET_TABLE_COLUMNS);

        assert_eq!(compacted.len(), 5);
        assert!(!compacted.contains_key("modelDigest"));
        assert!(!compacted.contains_key("description"));
    }

    fn sample_assets() -> Vec<Map<String, serde_json::Value>> {
        let mut asset1 = Map::new();
        asset1.insert("assetKey".to_string(), serde_json::json!("guid-1"));
        asset1.insert("name".to_string(), serde_json::json!("Zip"));

        let mut asset2 = Map::new();
        asset2.insert("assetKey".to_string(), serde_json::json!("guid-2"));
        asset2.insert("name".to_string(), serde_json::json!("MyAsset"));

        vec![asset1, asset2]
    }

    #[test]
    fn test_find_asset_by_asset_key() {
        let assets = sample_assets();
        let found = shared::find_asset(&assets, "guid-2").unwrap();
        assert_eq!(found.get("name").unwrap(), "MyAsset");
    }

    #[test]
    fn test_find_asset_by_name() {
        let assets = sample_assets();
        let found = shared::find_asset(&assets, "Zip").unwrap();
        assert_eq!(found.get("assetKey").unwrap(), "guid-1");
    }

    #[test]
    fn test_find_asset_not_found() {
        let assets = sample_assets();
        assert!(shared::find_asset(&assets, "does-not-exist").is_none());
    }
}
