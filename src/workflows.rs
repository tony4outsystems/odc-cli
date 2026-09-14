use crate::cli::Options;
use anyhow::{anyhow, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct App {
    pub key: String,
    pub revision: Option<i32>,
}

/// Parse an apps file with format: app-name[@revision]
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

/// Compute dependency levels using topological sort
pub fn dependency_levels(
    apps: &[App],
    _deps: &HashMap<String, Vec<String>>,
) -> Result<Vec<Vec<App>>> {
    // TODO: Implement topological sort
    // For now, return all apps in one level
    if apps.is_empty() {
        Ok(vec![])
    } else {
        Ok(vec![apps.to_vec()])
    }
}

/// Wait for an operation to complete with polling
pub async fn wait_for(
    _label: &str,
    _fetch: impl Fn() -> Result<Map<String, Value>>,
    _is_build: bool,
    interval: Duration,
    timeout: Duration,
) -> Result<Map<String, Value>> {
    let deadline = Instant::now() + timeout;

    if Instant::now() >= deadline {
        return Err(anyhow!("Timed out after {:?}", timeout));
    }

    let remaining = deadline - Instant::now();
    let sleep_duration = interval.min(remaining);

    tokio::time::sleep(sleep_duration).await;

    // TODO: Fetch status and check if terminal
    Err(anyhow!("Not yet implemented"))
}

/// Batch deploy apps from a file
pub async fn batch_deploy(_options: &Options, _apps_file: &str) -> Result<()> {
    // TODO: Implement batch deploy pipeline
    Err(anyhow!("batch-deploy: not yet implemented"))
}

/// Batch undeploy apps from a file
pub async fn batch_undeploy(_options: &Options, _apps_file: &str) -> Result<()> {
    // TODO: Implement batch undeploy pipeline
    Err(anyhow!("batch-undeploy: not yet implemented"))
}

/// Batch delete apps from a file
pub async fn batch_delete(_options: &Options, _apps_file: &str) -> Result<()> {
    // TODO: Implement batch delete pipeline
    Err(anyhow!("batch-delete: not yet implemented"))
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
}
