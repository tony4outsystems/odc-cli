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

        // Assets & Environments
        ListAssets { .. } => assets::cmd_list_assets(options, positionals).await,
        ListDeployedAssets { .. } => assets::cmd_list_deployed_assets(options, positionals).await,
        GetAsset { .. } => assets::cmd_get_asset(options, positionals).await,
        ListEnvironments => environments::cmd_list_environments(options).await,

        // Revisions
        LatestRevision { .. } => revisions::cmd_latest_revision(options, positionals).await,
        ListRevisions { .. } => revisions::cmd_list_revisions(options, positionals).await,
        GetRevision { .. } => revisions::cmd_get_revision(options, positionals).await,
        ProducerGraph { .. } => revisions::cmd_producer_graph(options, positionals).await,

        // Deployment
        AnalyzeDeployment { .. } => deployment::cmd_analyze_deployment(options).await,
        AnalyzeDeletion { .. } => deployment::cmd_analyze_deletion(options).await,
        InternalBuild { .. } => deployment::cmd_internal_build(options).await,
        InternalPublish { .. } => deployment::cmd_internal_publish(options).await,
        InternalDeploy { .. } => deployment::cmd_internal_deploy(options).await,
        Deploy { .. } => deployment::cmd_deploy(options).await,
        Undeploy { .. } => deployment::cmd_undeploy(options).await,
        DeleteAsset { .. } => deployment::cmd_delete_asset(options).await,

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

        // Mentor
        MentorStartSession => mentor::cmd_mentor_start_session(options).await,
        MentorCreateAsset { .. } => mentor::cmd_mentor_create_asset(options).await,
        MentorLoadAsset { .. } => mentor::cmd_mentor_load_asset(options, positionals).await,
        MentorPrompt { .. } => mentor::cmd_mentor_prompt(options).await,
        MentorGetRun { .. } => mentor::cmd_mentor_get_run(options).await,
        MentorGetEvent { .. } => mentor::cmd_mentor_get_event(options).await,
        MentorCancelPrompt { .. } => mentor::cmd_mentor_cancel_prompt(options).await,
        MentorCloseSession { .. } => mentor::cmd_mentor_close_session(options).await,
        MentorRequestUpload { .. } => mentor::cmd_mentor_request_upload(options).await,
        MentorPublish { .. } => mentor::cmd_mentor_publish(options).await,

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
