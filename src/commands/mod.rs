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

use crate::cli::Commands;
use crate::output::ColorMode;
use anyhow::Result;

/// Execute a command by matching on the Commands enum directly, dispatching to the appropriate module.
pub async fn execute(command: &Commands, json: bool, color: ColorMode, no_resolve: bool) -> Result<()> {
    use Commands::*;

    match command {
        // Auth & Portfolios (now using typed args)
        Discover => {
            let args = command.as_discover_args(json, color);
            auth::cmd_discover(args).await
        }
        ListPortfolios { .. } => {
            let args = command.as_list_portfolios_args(json, color);
            auth::cmd_list_portfolios(args, &[]).await
        }

        // Assets & Environments (now using typed args)
        ListAssets { .. } => {
            let args = command.as_list_assets_args(json, color);
            assets::cmd_list_assets(args, &[]).await
        }
        ListDeployedAssets { .. } => {
            let args = command.as_list_deployed_assets_args(json, color);
            assets::cmd_list_deployed_assets(args, &[]).await
        }
        GetAsset { asset } => {
            let args = command.as_get_asset_args(json, color);
            assets::cmd_get_asset(args, std::slice::from_ref(asset)).await
        }
        ListEnvironments => {
            let args = command.as_list_environments_args(json, color);
            environments::cmd_list_environments(args).await
        }

        // Revisions (now using typed args)
        LatestRevision { asset } => {
            let args = command.as_latest_revision_args(json, color);
            revisions::cmd_latest_revision(args, std::slice::from_ref(asset)).await
        }
        ListRevisions { asset, .. } => {
            let args = command.as_list_revisions_args(json, color);
            revisions::cmd_list_revisions(args, std::slice::from_ref(asset)).await
        }
        GetRevision { asset, .. } => {
            let args = command.as_get_revision_args(json, color);
            revisions::cmd_get_revision(args, std::slice::from_ref(asset)).await
        }
        ProducerGraph { asset, .. } => {
            let args = command.as_producer_graph_args(json, color);
            revisions::cmd_producer_graph(args, std::slice::from_ref(asset)).await
        }

        // Deployment (now using typed args)
        AnalyzeDeployment { .. } => {
            let args = command.as_deployment_analysis_args(json, color);
            deployment::cmd_analyze_deployment(args).await
        }
        AnalyzeDeletion { .. } => {
            let args = command.as_deletion_analysis_args(json, color);
            deployment::cmd_analyze_deletion(args).await
        }
        InternalBuild { .. } => {
            let args = command.as_build_args(json, color);
            deployment::cmd_internal_build(args).await
        }
        InternalPublish { .. } => {
            let args = command.as_publish_args(json, color);
            deployment::cmd_internal_publish(args).await
        }
        InternalDeploy { .. } => {
            let args = command.as_internal_deploy_args(json, color);
            deployment::cmd_internal_deploy(args).await
        }
        Deploy { .. } => {
            let args = command.as_deploy_args(json, color);
            deployment::cmd_deploy(args).await
        }
        Undeploy { .. } => {
            let args = command.as_undeploy_args(json, color);
            deployment::cmd_undeploy(args).await
        }
        DeleteAsset { .. } => {
            let args = command.as_delete_asset_args(json, color);
            deployment::cmd_delete_asset(args).await
        }

        // Batch operations (handled by workflows module)
        BatchDeploy {
            assets_file,
            env,
            build_type,
            skip_dependencies,
            poll,
            parallel,
        } => {
            shared::require_positional(
                std::slice::from_ref(assets_file),
                "batch-deploy",
                "an assets file",
            )?;
            let options = crate::cli::Options {
                json,
                color,
                env: env.clone(),
                build_type: build_type.clone(),
                skip_dependencies: *skip_dependencies,
                interval: std::time::Duration::from_secs(poll.poll_interval),
                timeout: std::time::Duration::from_secs(poll.timeout),
                no_wait: poll.no_wait,
                max_parallel: parallel.max_parallel,
                continue_on_error: parallel.continue_on_error,
                ..Default::default()
            };
            crate::workflows::batch_deploy(&options, assets_file).await
        }
        BatchUndeploy {
            assets_file,
            env,
            poll,
            parallel,
        } => {
            shared::require_positional(
                std::slice::from_ref(assets_file),
                "batch-undeploy",
                "an assets file",
            )?;
            let options = crate::cli::Options {
                json,
                color,
                env: env.clone(),
                interval: std::time::Duration::from_secs(poll.poll_interval),
                timeout: std::time::Duration::from_secs(poll.timeout),
                no_wait: poll.no_wait,
                max_parallel: parallel.max_parallel,
                continue_on_error: parallel.continue_on_error,
                ..Default::default()
            };
            crate::workflows::batch_undeploy(&options, assets_file).await
        }
        BatchDelete {
            assets_file,
            parallel,
        } => {
            shared::require_positional(
                std::slice::from_ref(assets_file),
                "batch-delete",
                "an assets file",
            )?;
            let options = crate::cli::Options {
                json,
                color,
                max_parallel: parallel.max_parallel,
                continue_on_error: parallel.continue_on_error,
                ..Default::default()
            };
            crate::workflows::batch_delete(&options, assets_file).await
        }
        DangerousBatchUndeployAll {
            env,
            poll,
            parallel,
        } => {
            let options = crate::cli::Options {
                json,
                color,
                env: env.clone(),
                interval: std::time::Duration::from_secs(poll.poll_interval),
                timeout: std::time::Duration::from_secs(poll.timeout),
                no_wait: poll.no_wait,
                max_parallel: parallel.max_parallel,
                continue_on_error: parallel.continue_on_error,
                ..Default::default()
            };
            crate::workflows::dangerous_batch_undeploy_all(&options).await
        }

        // Source Code (now using typed args)
        DownloadSourceCode { asset, .. } => {
            let args = command.as_download_source_code_args(json, color);
            source_code::cmd_download_source_code(args, std::slice::from_ref(asset)).await
        }
        UploadSourceCode { oml_file } => {
            let args = command.as_upload_source_code_args(json, color);
            source_code::cmd_upload_source_code(args, std::slice::from_ref(oml_file)).await
        }

        // Users & Groups (now using typed args)
        GetUser { user } => {
            let args = command.as_get_user_args(json, color);
            users::cmd_get_user(args, std::slice::from_ref(user)).await
        }
        UpdateUser { user, .. } => {
            let args = command.as_update_user_args(json, color);
            users::cmd_update_user(args, std::slice::from_ref(user)).await
        }
        ListGroups { .. } => {
            let args = command.as_list_groups_args(json, color, no_resolve);
            users::cmd_list_groups(args, &[]).await
        }
        GetGroup { group } => {
            let args = command.as_get_group_args(json, color);
            users::cmd_get_group(args, std::slice::from_ref(group)).await
        }
        UpdateGroup { group, .. } => {
            let args = command.as_update_group_args(json, color);
            users::cmd_update_group(args, std::slice::from_ref(group)).await
        }
        ListGroupMembers { group } => {
            let args = command.as_list_group_members_args(json, color);
            users::cmd_list_group_members(args, std::slice::from_ref(group)).await
        }
        AddUserToGroup { group, user } => {
            let args = command.as_add_user_to_group_args(json, color);
            users::cmd_add_user_to_group(args, &[group.clone(), user.clone()]).await
        }
        RemoveUserFromGroup { group, user } => {
            let args = command.as_remove_user_from_group_args(json, color);
            users::cmd_remove_user_from_group(args, &[group.clone(), user.clone()]).await
        }

        // Roles (now using typed args)
        ListRoles { asset, .. } => {
            let args = command.as_list_roles_args(json, color, no_resolve);
            roles::cmd_list_roles(args, std::slice::from_ref(asset)).await
        }
        ListRoleAssignments { asset, .. } => {
            let args = command.as_list_role_assignments_args(json, color, no_resolve);
            roles::cmd_list_role_assignments(args, std::slice::from_ref(asset)).await
        }
        GrantRole {
            asset: _,
            role,
            user,
        } => {
            let args = command.as_grant_role_args(json, color);
            roles::cmd_grant_role(args, &[user.clone(), role.clone()]).await
        }
        RevokeRole {
            asset: _,
            role,
            user,
        } => {
            let args = command.as_revoke_role_args(json, color);
            roles::cmd_revoke_role(args, &[user.clone(), role.clone()]).await
        }
        GrantGroupRole {
            asset: _,
            group,
            role,
        } => {
            let args = command.as_grant_group_role_args(json, color);
            roles::cmd_grant_group_role(args, &[group.clone(), role.clone()]).await
        }
        RevokeGroupRole {
            asset: _,
            group,
            role,
        } => {
            let args = command.as_revoke_group_role_args(json, color);
            roles::cmd_revoke_group_role(args, &[group.clone(), role.clone()]).await
        }

        // Mentor (now using typed args)
        MentorStartSession => {
            let args = command.as_mentor_start_session_args(json, color);
            mentor::cmd_mentor_start_session(args).await
        }
        MentorCreateAsset { .. } => {
            let args = command.as_mentor_create_asset_args(json, color);
            mentor::cmd_mentor_create_asset(args).await
        }
        MentorLoadAsset { .. } => {
            let args = command.as_mentor_load_asset_args(json, color);
            mentor::cmd_mentor_load_asset(args, &[]).await
        }
        MentorPrompt { .. } => {
            let args = command.as_mentor_prompt_args(json, color);
            mentor::cmd_mentor_prompt(args).await
        }
        MentorGetRun { .. } => {
            let args = command.as_mentor_get_run_args(json, color);
            mentor::cmd_mentor_get_run(args).await
        }
        MentorGetEvent { .. } => {
            let args = command.as_mentor_get_event_args(json, color);
            mentor::cmd_mentor_get_event(args).await
        }
        MentorCancelPrompt { .. } => {
            let args = command.as_mentor_cancel_prompt_args(json, color);
            mentor::cmd_mentor_cancel_prompt(args).await
        }
        MentorCloseSession { .. } => {
            let args = command.as_mentor_close_session_args(json, color);
            mentor::cmd_mentor_close_session(args).await
        }
        MentorRequestUpload { .. } => {
            let args = command.as_mentor_request_upload_args(json, color);
            mentor::cmd_mentor_request_upload(args).await
        }
        MentorPublish { .. } => {
            let args = command.as_mentor_publish_args(json, color);
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
