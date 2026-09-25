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
//! - [`context`]: per-invocation [`context::Ctx`] (output + no_resolve, plus client construction)

pub mod args;
pub mod assets;
pub mod auth;
pub mod context;
pub mod deployment;
pub mod environments;
pub mod mentor;
pub mod revisions;
pub mod roles;
pub mod shared;
pub mod source_code;
pub mod users;

use crate::cli::{Cli, Commands};
use anyhow::Result;
use context::Ctx;

/// Execute a command by matching on the Commands enum directly, dispatching to the appropriate module.
pub async fn execute(cli: &Cli) -> Result<()> {
    use Commands::*;

    let ctx = Ctx::new(cli.json, cli.color, cli.no_resolve);

    match &cli.command {
        // Auth & Portfolios
        Discover => auth::cmd_discover(&ctx).await,
        ListPortfolios(args) => auth::cmd_list_portfolios(&ctx, args).await,

        // Assets & Environments
        ListAssets(args) => assets::cmd_list_assets(&ctx, args).await,
        ListDeployedAssets(args) => assets::cmd_list_deployed_assets(&ctx, args).await,
        GetAsset(args) => assets::cmd_get_asset(&ctx, args).await,
        ListEnvironments => environments::cmd_list_environments(&ctx).await,

        // Revisions
        LatestRevision(args) => revisions::cmd_latest_revision(&ctx, args).await,
        ListRevisions(args) => revisions::cmd_list_revisions(&ctx, args).await,
        GetRevision(args) => revisions::cmd_get_revision(&ctx, args).await,
        ProducerGraph(args) => revisions::cmd_producer_graph(&ctx, args).await,

        // Deployment
        AnalyzeDeployment(args) => deployment::cmd_analyze_deployment(&ctx, args).await,
        AnalyzeDeletion(args) => deployment::cmd_analyze_deletion(&ctx, args).await,
        InternalBuild(args) => deployment::cmd_internal_build(&ctx, args).await,
        InternalPublish(args) => deployment::cmd_internal_publish(&ctx, args).await,
        InternalDeploy(args) => deployment::cmd_internal_deploy(&ctx, args).await,
        Deploy(args) => deployment::cmd_deploy(&ctx, args).await,
        Undeploy(args) => deployment::cmd_undeploy(&ctx, args).await,
        DeleteAsset(args) => deployment::cmd_delete_asset(&ctx, args).await,

        // Batch operations (handled by workflows module)
        BatchDeploy(args) => crate::workflows::batch_deploy(&ctx, args).await,
        BatchUndeploy(args) => crate::workflows::batch_undeploy(&ctx, args).await,
        BatchDelete(args) => crate::workflows::batch_delete(&ctx, args).await,
        DangerousBatchUndeployAll(args) => {
            crate::workflows::dangerous_batch_undeploy_all(&ctx, args).await
        }

        // Source Code
        DownloadSourceCode(args) => source_code::cmd_download_source_code(&ctx, args).await,
        UploadSourceCode(args) => source_code::cmd_upload_source_code(&ctx, args).await,

        // Users & Groups
        GetUser(args) => users::cmd_get_user(&ctx, args).await,
        UpdateUser(args) => users::cmd_update_user(&ctx, args).await,
        ListGroups(args) => users::cmd_list_groups(&ctx, args).await,
        GetGroup(args) => users::cmd_get_group(&ctx, args).await,
        UpdateGroup(args) => users::cmd_update_group(&ctx, args).await,
        ListGroupMembers(args) => users::cmd_list_group_members(&ctx, args).await,
        AddUserToGroup(args) => users::cmd_add_user_to_group(&ctx, args).await,
        RemoveUserFromGroup(args) => users::cmd_remove_user_from_group(&ctx, args).await,

        // Roles
        ListRoles(args) => roles::cmd_list_roles(&ctx, args).await,
        ListRoleAssignments(args) => roles::cmd_list_role_assignments(&ctx, args).await,
        GrantRole(args) => roles::cmd_grant_role(&ctx, args).await,
        RevokeRole(args) => roles::cmd_revoke_role(&ctx, args).await,
        GrantGroupRole(args) => roles::cmd_grant_group_role(&ctx, args).await,
        RevokeGroupRole(args) => roles::cmd_revoke_group_role(&ctx, args).await,

        // Mentor
        MentorStartSession => mentor::cmd_mentor_start_session(&ctx).await,
        MentorCreateAsset(args) => mentor::cmd_mentor_create_asset(&ctx, args).await,
        MentorLoadAsset(args) => mentor::cmd_mentor_load_asset(&ctx, args).await,
        Mentor(args) => mentor::cmd_mentor(&ctx, args).await,
        MentorPrompt(args) => mentor::cmd_mentor_prompt(&ctx, args).await,
        MentorGetRun(args) => mentor::cmd_mentor_get_run(&ctx, args).await,
        MentorGetEvent(args) => mentor::cmd_mentor_get_event(&ctx, args).await,
        MentorCancelPrompt(args) => mentor::cmd_mentor_cancel_prompt(&ctx, args).await,
        MentorCloseSession(args) => mentor::cmd_mentor_close_session(&ctx, args).await,
        MentorRequestUpload(args) => mentor::cmd_mentor_request_upload(&ctx, args).await,
        MentorPublish(args) => mentor::cmd_mentor_publish(&ctx, args).await,

        // These are handled specially and should not reach here
        Login(_) => Err(anyhow::anyhow!(
            "Login should be handled in main run() function"
        )),
        Completion(_) => Err(anyhow::anyhow!(
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
