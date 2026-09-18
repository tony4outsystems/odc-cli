//! Batch operation workflows and dependency management.
//!
//! This module handles multi-app batch operations (deploy, undeploy, delete) with automatic
//! dependency resolution. It provides:
//!
//! # Dependency-Aware Batch Deployment
//!
//! - [`parse_apps_file()`]: Parse app list from file (format: `app-key[@revision]`)
//! - [`dependency_levels()`]: Compute deployment order using topological sort (Kahn's algorithm)
//! - Dependencies are deployed before dependents
//! - Cycles are detected and reported
//!
//! # Polling and Waiting
//!
//! - [`wait_for()`]: Poll a status check until terminal state or timeout
//! - Used for builds, deployments, and other long-running operations
//!
//! # Parallelization
//!
//! Batch operations run multiple apps concurrently (configurable via `--max-parallel`).
//! Each app runs through: resolve → build → deploy/undeploy/delete.

use crate::cli::Options;
use crate::commands::shared::{resolve_asset_in, resolve_env, resolve_revision};
use anyhow::{anyhow, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct App {
    pub key: String,
    pub revision: Option<i32>,
}

/// Parse an apps file listing apps to deploy.
///
/// File format: one app per line, optionally with `@revision` suffix.
/// - `MyApp` → deploy latest revision of MyApp
/// - `MyApp@5` → deploy specific revision 5 of MyApp
/// - Blank lines and lines starting with `#` are ignored
///
/// # Errors
///
/// Returns error if file doesn't exist, contains invalid lines, or revisions are non-positive.
pub fn parse_apps_file(path: &Path) -> Result<Vec<App>> {
    let content = std::fs::read_to_string(path).map_err(|e| anyhow!("Apps file: {}", e))?;

    let mut apps = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();

        // Skip blank and comment lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Split on first @
        let (key, revision_str) = match line.split_once('@') {
            Some((k, r)) => (k.trim().to_string(), Some(r.trim())),
            None => (line.trim().to_string(), None),
        };

        if key.is_empty() {
            return Err(anyhow!("Empty app at line {}", line_num + 1));
        }

        let revision = if let Some(rev_str) = revision_str {
            let n: i32 = rev_str
                .parse()
                .map_err(|_| anyhow!("Invalid revision {:?} for app {:?}", rev_str, key))?;
            if n < 1 {
                return Err(anyhow!("Invalid revision {:?} for app {:?}", rev_str, key));
            }
            Some(n)
        } else {
            None
        };

        apps.push(App { key, revision });
    }

    if apps.is_empty() {
        return Err(anyhow!("Apps file is empty: {}", path.display()));
    }

    Ok(apps)
}

/// Compute deployment order using topological sort (Kahn's algorithm).
///
/// Groups apps into levels such that:
/// - Each level contains only apps whose dependencies are in earlier levels
/// - Levels can be deployed in order: level 0, then level 1, etc.
/// - Producers (dependencies) deploy before consumers
///
/// # Arguments
///
/// - `apps`: Apps to deploy (potentially a subset of all apps)
/// - `deps`: Map from app key to list of app keys it depends on
///
/// # Returns
///
/// A vector of deployment levels, where each level is a vector of apps.
/// Apps with no dependencies are in level 0.
///
/// # Errors
///
/// Returns error if a cycle is detected in dependencies.
/// External dependencies (not in `apps` list) are ignored.
pub fn dependency_levels(
    apps: &[App],
    deps: &HashMap<String, Vec<String>>,
) -> Result<Vec<Vec<App>>> {
    use std::collections::HashSet;

    let keys: HashSet<&str> = apps.iter().map(|a| a.key.as_str()).collect();

    let mut indegree: HashMap<&str, usize> = keys.iter().map(|&k| (k, 0)).collect();
    for key in &keys {
        if let Some(dep_list) = deps.get(*key) {
            let count = dep_list
                .iter()
                .filter(|d| keys.contains(d.as_str()))
                .count();
            *indegree.get_mut(key).unwrap() = count;
        }
    }

    let mut remaining: HashSet<&str> = keys.clone();
    let mut levels = Vec::new();

    while !remaining.is_empty() {
        let ready: Vec<&str> = remaining
            .iter()
            .copied()
            .filter(|k| indegree[k] == 0)
            .collect();

        if ready.is_empty() {
            return Err(anyhow!(
                "Cycle detected among apps: {}",
                remaining.iter().copied().collect::<Vec<_>>().join(", ")
            ));
        }

        let ready_set: HashSet<&str> = ready.iter().copied().collect();
        let level: Vec<App> = apps
            .iter()
            .filter(|a| ready_set.contains(a.key.as_str()))
            .cloned()
            .collect();
        levels.push(level);

        for k in &ready {
            remaining.remove(k);
        }
        for key in &remaining {
            if let Some(dep_list) = deps.get(*key) {
                let resolved = dep_list
                    .iter()
                    .filter(|d| ready_set.contains(d.as_str()))
                    .count();
                if resolved > 0 {
                    *indegree.get_mut(key).unwrap() -= resolved;
                }
            }
        }
    }

    Ok(levels)
}

/// Poll an operation until it reaches a terminal state or timeout occurs.
///
/// Repeatedly calls `fetch()` on `interval` until `is_terminal()` returns true,
/// or until `timeout` elapses. Used for waiting on builds, deployments, and async operations.
///
/// # Arguments
///
/// - `label`: Human-readable operation name (used in timeout error message)
/// - `fetch`: Async function to fetch current status; should fetch fresh state each call
/// - `is_terminal`: Function to check if a status is complete (build finished, deployment done, etc.)
/// - `interval`: Time to wait between polls
/// - `timeout`: Maximum time to wait before returning timeout error
///
/// # Returns
///
/// The last fetched state if it reached terminal, or timeout error.
///
/// # Example
///
/// ```ignore
/// let build = wait_for(
///     "build",
///     || client.get_build(build_key),
///     |b| matches!(b.get("status").and_then(|v| v.as_str()), Some("succeeded" | "failed")),
///     Duration::from_secs(10),
///     Duration::from_secs(1800),
/// ).await?;
/// ```
pub async fn wait_for(
    label: &str,
    fetch: impl Fn() -> Result<Map<String, Value>>,
    is_terminal: impl Fn(&Map<String, Value>) -> bool,
    interval: Duration,
    timeout: Duration,
) -> Result<Map<String, Value>> {
    let deadline = Instant::now() + timeout;

    loop {
        let result = fetch()?;
        if is_terminal(&result) {
            return Ok(result);
        }

        if Instant::now() >= deadline {
            return Err(anyhow!(
                "Timed out after {:?} waiting for {}",
                timeout,
                label
            ));
        }

        let remaining = deadline - Instant::now();
        let sleep_duration = interval.min(remaining);
        tokio::time::sleep(sleep_duration).await;
    }
}

type BoxFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/// Run multiple items concurrently with error handling.
///
/// Executes `make_task(item)` for each item, with at most `max_parallel` tasks running concurrently.
/// If a task fails:
/// - If `continue_on_error` is false: aborts immediately and returns the error
/// - If `continue_on_error` is true: continues running remaining items and collects errors
///
/// Used for batch deploy, batch undeploy, and batch delete operations.
async fn run_concurrent<T, F>(
    mut items: Vec<T>,
    max_parallel: usize,
    continue_on_error: bool,
    make_task: F,
) -> Result<()>
where
    T: Send + 'static,
    F: Fn(T) -> BoxFuture + Send + Sync + 'static,
{
    let make_task = Arc::new(make_task);
    let chunk_size = max_parallel.max(1);
    let mut failures: Vec<String> = Vec::new();

    while !items.is_empty() {
        let chunk: Vec<T> = items.drain(..chunk_size.min(items.len())).collect();
        let mut set = tokio::task::JoinSet::new();
        for item in chunk {
            let make_task = make_task.clone();
            set.spawn(async move { make_task(item).await });
        }

        while let Some(joined) = set.join_next().await {
            let result = match joined {
                Ok(result) => result,
                Err(join_err) => Err(anyhow!(join_err.to_string())),
            };
            if let Err(e) = result {
                if continue_on_error {
                    failures.push(e.to_string());
                } else {
                    return Err(e);
                }
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "{} of the operations failed:\n{}",
            failures.len(),
            failures.join("\n")
        ))
    }
}

/// Resolve every listed app to its asset key, deduplicating and erroring on a revision
/// conflict for the same app between two lines in the file.
fn resolve_file_apps(
    client: &crate::client::Client,
    file_apps: &[App],
    resolve_revision: impl Fn(
        &crate::client::Client,
        &Map<String, Value>,
        &str,
        Option<i32>,
    ) -> Result<i32>,
) -> Result<HashMap<String, i32>> {
    let apps_list = client.list_assets()?;
    let mut resolved: HashMap<String, i32> = HashMap::new();

    for file_app in file_apps {
        let app = resolve_asset_in(&apps_list, &file_app.key)?;
        let asset_key = app
            .get("assetKey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("App {} has no assetKey field", file_app.key))?
            .to_string();
        let revision = resolve_revision(client, app, &asset_key, file_app.revision)?;

        match resolved.get(&asset_key) {
            Some(existing) if *existing != revision => {
                return Err(anyhow!(
                    "Conflicting revision for {}: {} vs {}",
                    asset_key,
                    existing,
                    revision
                ));
            }
            _ => {
                resolved.insert(asset_key, revision);
            }
        }
    }

    Ok(resolved)
}

/// Walk a producer dependency tree (as returned in one call by the producer-graph endpoint),
/// recording each producer's resolved revision and a `child -> parent` dependency edge for
/// `dependency_levels`. Errors on a revision conflict with an already-recorded app.
fn merge_producer_tree(
    parent_key: &str,
    producers: &[Map<String, Value>],
    revisions: &mut HashMap<String, i32>,
    deps: &mut HashMap<String, Vec<String>>,
) -> Result<()> {
    for producer in producers {
        let key = producer
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Producer has no key"))?
            .to_string();

        if let Some(revision) = producer.get("revision").and_then(|v| v.as_i64()) {
            let revision = revision as i32;
            match revisions.get(&key) {
                Some(existing) if *existing != revision => {
                    return Err(anyhow!(
                        "Conflicting revision for {}: {} vs {}",
                        key,
                        existing,
                        revision
                    ));
                }
                _ => {
                    revisions.entry(key.clone()).or_insert(revision);
                }
            }
        }

        deps.entry(parent_key.to_string())
            .or_default()
            .push(key.clone());

        let children: Vec<Map<String, Value>> = producer
            .get("producers")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| match v {
                Value::Object(map) => Some(map),
                _ => None,
            })
            .collect();
        merge_producer_tree(&key, &children, revisions, deps)?;
    }

    Ok(())
}

/// Build and deploy every app listed in `apps_file` to `--env`. By default, each app's
/// producer dependencies are resolved via the producer graph, deduplicated, and deployed
/// first (see `merge_producer_tree`/`dependency_levels`); `--skip-dependencies` deploys only
/// the listed apps. Apps within a level run up to `--max-parallel` at a time.
pub async fn batch_deploy(options: &Options, apps_file: &str) -> Result<()> {
    let settings = crate::settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Arc::new(crate::client::Client::new(settings, output.clone()));

    let file_apps = parse_apps_file(Path::new(apps_file))?;
    let env_key = resolve_env(&client, &options.env)?;

    let mut revisions = resolve_file_apps(&client, &file_apps, resolve_revision)?;
    let mut deps: HashMap<String, Vec<String>> = HashMap::new();

    if !options.skip_dependencies {
        let roots: Vec<(String, i32)> = revisions.iter().map(|(k, v)| (k.clone(), *v)).collect();
        for (asset_key, revision) in roots {
            let producers = client.get_producer_graph(&asset_key, revision, 0, "Deployable", "")?;
            merge_producer_tree(&asset_key, &producers, &mut revisions, &mut deps)?;
        }
    }

    let apps: Vec<App> = revisions
        .into_iter()
        .map(|(key, revision)| App {
            key,
            revision: Some(revision),
        })
        .collect();
    let levels = dependency_levels(&apps, &deps)?;

    for level in levels {
        let client = client.clone();
        let options = options.clone();
        let env_key = env_key.clone();
        run_concurrent(
            level,
            options.max_parallel,
            options.continue_on_error,
            move |app: App| {
                let client = client.clone();
                let options = options.clone();
                let env_key = env_key.clone();
                Box::pin(async move {
                    let revision = app
                        .revision
                        .ok_or_else(|| anyhow!("Missing resolved revision for {}", app.key))?;
                    let (build_key, _) = crate::commands::deployment::run_build(
                        &client,
                        &options.build_type,
                        options.interval.as_secs(),
                        options.timeout.as_secs(),
                        options.no_wait,
                        &app.key,
                        revision,
                    )
                    .await?;
                    crate::commands::deployment::run_deployment_operation(
                        &client,
                        "Deploy",
                        &app.key,
                        &env_key,
                        Some(revision),
                        Some(build_key.as_str()),
                        options.interval.as_secs(),
                        options.timeout.as_secs(),
                        options.no_wait,
                    )
                    .await?;
                    Ok(())
                }) as BoxFuture
            },
        )
        .await?;
    }

    output.println_locked("Batch deploy finished");
    Ok(())
}

/// Undeploy every app listed in `apps_file` from `--env`, up to `--max-parallel` at a time.
pub async fn batch_undeploy(options: &Options, apps_file: &str) -> Result<()> {
    let settings = crate::settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Arc::new(crate::client::Client::new(settings, output.clone()));

    let file_apps = parse_apps_file(Path::new(apps_file))?;
    let apps_list = client.list_assets()?;
    let env_key = resolve_env(&client, &options.env)?;

    let mut asset_keys = Vec::new();
    for file_app in &file_apps {
        let asset_key = resolve_asset_in(&apps_list, &file_app.key)?
            .get("assetKey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("App {} has no assetKey field", file_app.key))?
            .to_string();
        asset_keys.push(asset_key);
    }

    let options = options.clone();
    run_concurrent(
        asset_keys,
        options.max_parallel,
        options.continue_on_error,
        move |asset_key: String| {
            let client = client.clone();
            let options = options.clone();
            let env_key = env_key.clone();
            Box::pin(async move {
                crate::commands::deployment::run_deployment_operation(
                    &client,
                    "Undeploy",
                    &asset_key,
                    &env_key,
                    None,
                    None,
                    options.interval.as_secs(),
                    options.timeout.as_secs(),
                    options.no_wait,
                )
                .await?;
                Ok(())
            }) as BoxFuture
        },
    )
    .await?;

    output.println_locked("Batch undeploy finished");
    Ok(())
}

/// Delete every app listed in `apps_file`, up to `--max-parallel` at a time.
pub async fn batch_delete(options: &Options, apps_file: &str) -> Result<()> {
    let settings = crate::settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Arc::new(crate::client::Client::new(settings, output.clone()));

    let file_apps = parse_apps_file(Path::new(apps_file))?;
    let apps_list = client.list_assets()?;

    let mut asset_keys = Vec::new();
    for file_app in &file_apps {
        let asset_key = resolve_asset_in(&apps_list, &file_app.key)?
            .get("assetKey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("App {} has no assetKey field", file_app.key))?
            .to_string();
        asset_keys.push(asset_key);
    }

    run_concurrent(
        asset_keys,
        options.max_parallel,
        options.continue_on_error,
        move |asset_key: String| {
            let client = client.clone();
            Box::pin(async move {
                client.delete_asset(&asset_key)?;
                Ok(())
            }) as BoxFuture
        },
    )
    .await?;

    output.println_locked("Batch delete finished");
    Ok(())
}

/// Undeploy every app currently deployed to `--env`, up to `--max-parallel` at a time.
pub async fn dangerous_batch_undeploy_all(options: &Options) -> Result<()> {
    let settings = crate::settings::load_settings()?;
    let output = Arc::new(crate::output::Output::new(options.json, options.color));
    let client = Arc::new(crate::client::Client::new(settings, output.clone()));

    let env_key = resolve_env(&client, &options.env)?;
    let deployed = client.list_deployed_assets()?;
    let rows = crate::inspection::deployed_asset_rows(&deployed, &env_key, "");

    let asset_keys: Vec<String> = rows
        .iter()
        .filter_map(|row| row.get("key").and_then(|v| v.as_str()).map(str::to_string))
        .collect();

    let options = options.clone();
    run_concurrent(
        asset_keys,
        options.max_parallel,
        options.continue_on_error,
        move |asset_key: String| {
            let client = client.clone();
            let options = options.clone();
            let env_key = env_key.clone();
            Box::pin(async move {
                crate::commands::deployment::run_deployment_operation(
                    &client,
                    "Undeploy",
                    &asset_key,
                    &env_key,
                    None,
                    None,
                    options.interval.as_secs(),
                    options.timeout.as_secs(),
                    options.no_wait,
                )
                .await?;
                Ok(())
            }) as BoxFuture
        },
    )
    .await?;

    output.println_locked("Undeployed every app from the environment");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_apps_file_simple() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "app1\napp2@5\n# comment\n\napp3")?;
        file.flush()?;

        let apps = parse_apps_file(file.path())?;
        assert_eq!(apps.len(), 3);
        assert_eq!(apps[0].key, "app1");
        assert_eq!(apps[0].revision, None);
        assert_eq!(apps[1].key, "app2");
        assert_eq!(apps[1].revision, Some(5));
        assert_eq!(apps[2].key, "app3");
        assert_eq!(apps[2].revision, None);
        Ok(())
    }

    #[test]
    fn test_parse_apps_file_empty() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "# just comments")?;
        file.flush()?;

        let result = parse_apps_file(file.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
        Ok(())
    }

    #[test]
    fn test_parse_apps_file_invalid_revision() -> Result<()> {
        let mut file = NamedTempFile::new()?;
        writeln!(file, "app@invalid")?;
        file.flush()?;

        let result = parse_apps_file(file.path());
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_dependency_levels_empty() -> Result<()> {
        let levels = dependency_levels(&[], &HashMap::new())?;
        assert_eq!(levels.len(), 0);
        Ok(())
    }

    #[test]
    fn test_dependency_levels_single() -> Result<()> {
        let app = App {
            key: "test".to_string(),
            revision: Some(1),
        };
        let levels = dependency_levels(std::slice::from_ref(&app), &HashMap::new())?;
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].len(), 1);
        assert_eq!(levels[0][0].key, "test");
        Ok(())
    }

    #[test]
    fn test_dependency_levels_orders_by_dependency() -> Result<()> {
        let apps = vec![
            App {
                key: "consumer".to_string(),
                revision: Some(1),
            },
            App {
                key: "producer".to_string(),
                revision: Some(1),
            },
        ];
        let mut deps = HashMap::new();
        deps.insert("consumer".to_string(), vec!["producer".to_string()]);

        let levels = dependency_levels(&apps, &deps)?;
        assert_eq!(levels.len(), 2);
        assert_eq!(levels[0][0].key, "producer");
        assert_eq!(levels[1][0].key, "consumer");
        Ok(())
    }

    #[test]
    fn test_dependency_levels_detects_cycle() {
        let apps = vec![
            App {
                key: "a".to_string(),
                revision: Some(1),
            },
            App {
                key: "b".to_string(),
                revision: Some(1),
            },
        ];
        let mut deps = HashMap::new();
        deps.insert("a".to_string(), vec!["b".to_string()]);
        deps.insert("b".to_string(), vec!["a".to_string()]);

        let result = dependency_levels(&apps, &deps);
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_producer_tree_collects_nested_revisions() -> Result<()> {
        let mut revisions = HashMap::new();
        let mut deps = HashMap::new();
        let producers = vec![{
            let mut child = Map::new();
            child.insert("key".to_string(), Value::String("lib1".to_string()));
            child.insert("revision".to_string(), Value::from(2));
            let mut grandchild = Map::new();
            grandchild.insert("key".to_string(), Value::String("lib2".to_string()));
            grandchild.insert("revision".to_string(), Value::from(5));
            child.insert(
                "producers".to_string(),
                Value::Array(vec![Value::Object(grandchild)]),
            );
            child
        }];

        merge_producer_tree("app1", &producers, &mut revisions, &mut deps)?;

        assert_eq!(revisions.get("lib1"), Some(&2));
        assert_eq!(revisions.get("lib2"), Some(&5));
        assert_eq!(deps.get("app1"), Some(&vec!["lib1".to_string()]));
        assert_eq!(deps.get("lib1"), Some(&vec!["lib2".to_string()]));
        Ok(())
    }

    #[test]
    fn test_merge_producer_tree_rejects_revision_conflict() {
        let mut revisions = HashMap::new();
        revisions.insert("lib1".to_string(), 1);
        let mut deps = HashMap::new();
        let mut child = Map::new();
        child.insert("key".to_string(), Value::String("lib1".to_string()));
        child.insert("revision".to_string(), Value::from(2));

        let result = merge_producer_tree("app1", &[child], &mut revisions, &mut deps);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_concurrent_runs_every_item() -> Result<()> {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let items: Vec<u32> = (0..5).collect();
        let counter_clone = counter.clone();

        run_concurrent(items, 2, false, move |_| {
            let counter = counter_clone.clone();
            Box::pin(async move {
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }) as BoxFuture
        })
        .await?;

        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 5);
        Ok(())
    }

    #[tokio::test]
    async fn test_run_concurrent_stops_on_first_error_by_default() {
        let items = vec![1, 2, 3];
        let result = run_concurrent(items, 1, false, |n: i32| {
            Box::pin(async move {
                if n == 2 {
                    Err(anyhow!("boom"))
                } else {
                    Ok(())
                }
            }) as BoxFuture
        })
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_run_concurrent_continues_on_error() -> Result<()> {
        let items = vec![1, 2, 3];
        let result = run_concurrent(items, 3, true, |n: i32| {
            Box::pin(async move {
                if n == 2 {
                    Err(anyhow!("boom"))
                } else {
                    Ok(())
                }
            }) as BoxFuture
        })
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("1 of the operations failed"));
        Ok(())
    }

    #[tokio::test]
    async fn test_wait_for_polls_until_terminal() -> Result<()> {
        let calls = std::sync::atomic::AtomicU32::new(0);
        let result = wait_for(
            "test",
            || {
                let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut map = Map::new();
                map.insert(
                    "status".to_string(),
                    Value::String(if n < 2 { "Running" } else { "Finished" }.to_string()),
                );
                Ok(map)
            },
            |m| m.get("status").and_then(|v| v.as_str()) == Some("Finished"),
            Duration::from_millis(1),
            Duration::from_secs(5),
        )
        .await?;

        assert_eq!(result.get("status").unwrap(), "Finished");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
        Ok(())
    }

    #[tokio::test]
    async fn test_wait_for_times_out() {
        let result = wait_for(
            "test",
            || Ok(Map::new()),
            |_| false,
            Duration::from_millis(1),
            Duration::from_millis(5),
        )
        .await;

        assert!(result.is_err());
    }
}
