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

use crate::cli::Options;
use anyhow::Result;

/// Execute a command based on its name, dispatching to the appropriate module.
pub async fn execute(cmd: &str, options: &Options, positionals: &[String]) -> Result<()> {
    match cmd {
        "login" => Err(anyhow::anyhow!(
            "Login should be handled in main run() function"
        )),

        // Auth & Portfolios
        "discover" => auth::cmd_discover(options).await,
        "list-portfolios" => auth::cmd_list_portfolios(options, positionals).await,

        // Assets & Environments
        "list-assets" => assets::cmd_list_assets(options, positionals).await,
        "list-deployed-assets" => assets::cmd_list_deployed_assets(options, positionals).await,
        "get-asset" => assets::cmd_get_asset(options, positionals).await,
        "list-environments" => environments::cmd_list_environments(options).await,

        // Revisions
        "latest-revision" => revisions::cmd_latest_revision(options, positionals).await,
        "list-revisions" => revisions::cmd_list_revisions(options, positionals).await,
        "get-revision" => revisions::cmd_get_revision(options, positionals).await,
        "producer-graph" => revisions::cmd_producer_graph(options, positionals).await,

        // Deployment
        "analyze-deployment" => deployment::cmd_analyze_deployment(options).await,
        "analyze-deletion" => deployment::cmd_analyze_deletion(options).await,
        "internal-build" => deployment::cmd_internal_build(options).await,
        "internal-publish" => deployment::cmd_internal_publish(options).await,
        "internal-deploy" => deployment::cmd_internal_deploy(options).await,
        "deploy" => deployment::cmd_deploy(options).await,
        "undeploy" => deployment::cmd_undeploy(options).await,
        "delete-asset" => deployment::cmd_delete_asset(options).await,

        // Batch operations (handled by workflows module)
        "batch-deploy" => {
            shared::require_positional(positionals, "batch-deploy", "an assets file")?;
            crate::workflows::batch_deploy(options, &positionals[0]).await
        }
        "batch-undeploy" => {
            shared::require_positional(positionals, "batch-undeploy", "an assets file")?;
            crate::workflows::batch_undeploy(options, &positionals[0]).await
        }
        "batch-delete" => {
            shared::require_positional(positionals, "batch-delete", "an assets file")?;
            crate::workflows::batch_delete(options, &positionals[0]).await
        }
        "dangerous-batch-undeploy-all" => {
            crate::workflows::dangerous_batch_undeploy_all(options).await
        }

        // Source Code
        "download-source-code" => source_code::cmd_download_source_code(options, positionals).await,
        "upload-source-code" => source_code::cmd_upload_source_code(options, positionals).await,

        // Users & Groups
        "get-user" => users::cmd_get_user(options, positionals).await,
        "update-user" => users::cmd_update_user(options, positionals).await,
        "list-groups" => users::cmd_list_groups(options, positionals).await,
        "get-group" => users::cmd_get_group(options, positionals).await,
        "update-group" => users::cmd_update_group(options, positionals).await,
        "list-group-members" => users::cmd_list_group_members(options, positionals).await,
        "add-user-to-group" => users::cmd_add_user_to_group(options, positionals).await,
        "remove-user-from-group" => users::cmd_remove_user_from_group(options, positionals).await,

        // Roles
        "list-roles" => roles::cmd_list_roles(options, positionals).await,
        "list-role-assignments" => roles::cmd_list_role_assignments(options, positionals).await,
        "grant-role" => roles::cmd_grant_role(options, positionals).await,
        "revoke-role" => roles::cmd_revoke_role(options, positionals).await,
        "grant-group-role" => roles::cmd_grant_group_role(options, positionals).await,
        "revoke-group-role" => roles::cmd_revoke_group_role(options, positionals).await,

        // Mentor
        "mentor-start-session" => mentor::cmd_mentor_start_session(options).await,
        "mentor-create-asset" => mentor::cmd_mentor_create_asset(options).await,
        "mentor-load-asset" => mentor::cmd_mentor_load_asset(options, positionals).await,
        "mentor-prompt" => mentor::cmd_mentor_prompt(options).await,
        "mentor-get-run" => mentor::cmd_mentor_get_run(options).await,
        "mentor-get-event" => mentor::cmd_mentor_get_event(options).await,
        "mentor-cancel-prompt" => mentor::cmd_mentor_cancel_prompt(options).await,
        "mentor-close-session" => mentor::cmd_mentor_close_session(options).await,
        "mentor-request-upload" => mentor::cmd_mentor_request_upload(options).await,
        "mentor-publish" => mentor::cmd_mentor_publish(options).await,

        _ => Err(anyhow::anyhow!("Unknown command: {}", cmd)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    #[tokio::test]
    async fn test_unknown_command() {
        let opts = Options::default();
        let result = execute("nonexistent", &opts, &[]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }

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
