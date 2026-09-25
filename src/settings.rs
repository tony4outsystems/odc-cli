use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub tenant_url: String,
    pub client_id: String,
    pub client_secret: String,
}

impl Settings {
    /// Create a new Settings with validation.
    ///
    /// # Validation
    ///
    /// - `tenant_url` must be a valid `https://` URL
    /// - `client_id` must not be empty
    /// - `client_secret` must not be empty
    ///
    /// # Returns
    ///
    /// Settings if all fields are valid, or error if validation fails.
    pub fn new(tenant_url: String, client_id: String, client_secret: String) -> Result<Self> {
        // Validate URL format
        let url = url::Url::parse(&tenant_url).map_err(|e| anyhow!("Invalid tenant_url: {}", e))?;
        if url.scheme() != "https" {
            return Err(anyhow!(
                "Invalid tenant_url: must use https:// (got {}://)",
                url.scheme()
            ));
        }

        // Validate non-empty fields
        if client_id.is_empty() {
            return Err(anyhow!("client_id cannot be empty"));
        }
        if client_secret.is_empty() {
            return Err(anyhow!("client_secret cannot be empty"));
        }

        Ok(Settings {
            tenant_url,
            client_id,
            client_secret,
        })
    }

    pub fn tenant_origin(&self) -> String {
        self.tenant_url.trim_end_matches('/').to_string()
    }

    /// The Mentor MCP server endpoint, derived from the tenant origin.
    pub fn mentor_url(&self) -> String {
        format!("{}/mcp", self.tenant_origin())
    }
}

/// Load settings from the process environment, falling back to `~/.odc/config.json` for any
/// value not set in the environment. Each of `ODC_TENANT_URL`/`ODC_CLIENT_ID`/`ODC_CLIENT_SECRET`
/// is resolved independently, so a partial environment can be completed by the saved config.
pub fn load_settings() -> Result<Settings> {
    load_settings_with_home(None)
}

/// Like [`load_settings`], but reads the saved config from under `home` instead of the real
/// home directory (for tests).
pub fn load_settings_with_home(home: Option<PathBuf>) -> Result<Settings> {
    let config = load_saved_config(home)?;

    let tenant_url = required_var("ODC_TENANT_URL", config.as_ref().map(|c| &c.tenant_url))?;
    let client_id = required_var("ODC_CLIENT_ID", config.as_ref().map(|c| &c.client_id))?;
    let client_secret = required_var(
        "ODC_CLIENT_SECRET",
        config.as_ref().map(|c| &c.client_secret),
    )?;

    Settings::new(tenant_url, client_id, client_secret)
}

/// Resolve a single required setting: the process environment variable `name` if it is set and
/// non-empty, else `fallback` (from the saved config) if non-empty, else an error.
fn required_var(name: &str, fallback: Option<&String>) -> Result<String> {
    if let Ok(value) = std::env::var(name) {
        if !value.is_empty() {
            return Ok(value);
        }
    }
    if let Some(value) = fallback {
        if !value.is_empty() {
            return Ok(value.clone());
        }
    }
    Err(anyhow!("Missing required environment variable: {}", name))
}

/// Read `~/.odc/config.json` (or under `home` when given), returning `None` if it doesn't exist.
fn load_saved_config(home: Option<PathBuf>) -> Result<Option<Settings>> {
    let path = config_path(home)?;
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path)?;
    let settings: Settings = serde_json::from_str(&data)
        .map_err(|_| anyhow!("Invalid configuration in {}", path.display()))?;
    Ok(Some(settings))
}

/// Path to `~/.odc/config.json` (or under `home` when given, for tests).
pub fn config_path(home: Option<PathBuf>) -> Result<PathBuf> {
    let home_dir = match home {
        Some(h) => h,
        None => dirs::home_dir().ok_or_else(|| anyhow!("Cannot determine home directory"))?,
    };
    Ok(home_dir.join(".odc").join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_rejects_plain_http_tenant_url() {
        let err =
            Settings::new("http://example.com".into(), "id".into(), "secret".into()).unwrap_err();
        assert!(err.to_string().contains("https"));
        assert!(Settings::new("https://example.com".into(), "id".into(), "secret".into()).is_ok());
    }

    #[test]
    fn test_tenant_origin_trim() {
        let settings = Settings {
            tenant_url: "https://example.com/".to_string(),
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
        };
        assert_eq!(settings.tenant_origin(), "https://example.com");
    }

    #[test]
    fn test_tenant_origin_no_trailing_slash() {
        let settings = Settings {
            tenant_url: "https://example.com".to_string(),
            client_id: "id".to_string(),
            client_secret: "secret".to_string(),
        };
        assert_eq!(settings.tenant_origin(), "https://example.com");
    }

    #[test]
    fn test_required_var_uses_fallback_when_env_unset() {
        // Use a name that is certainly not set in the test process environment.
        let value =
            required_var("ODC_TEST_VAR_NOT_IN_ENV", Some(&"from-config".to_string())).unwrap();
        assert_eq!(value, "from-config");
    }

    #[test]
    fn test_required_var_errors_without_env_or_fallback() {
        let err = required_var("ODC_TEST_VAR_NOT_IN_ENV", None).unwrap_err();
        assert!(err.to_string().contains("ODC_TEST_VAR_NOT_IN_ENV"));
    }

    #[test]
    fn test_required_var_errors_on_empty_fallback() {
        let err = required_var("ODC_TEST_VAR_NOT_IN_ENV", Some(&String::new())).unwrap_err();
        assert!(err.to_string().contains("Missing required"));
    }

    #[test]
    fn test_load_saved_config_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let config = load_saved_config(Some(dir.path().to_path_buf())).unwrap();
        assert!(config.is_none());
    }

    #[test]
    fn test_load_saved_config_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let odc_dir = dir.path().join(".odc");
        fs::create_dir_all(&odc_dir).unwrap();
        fs::write(
            odc_dir.join("config.json"),
            r#"{"tenant_url":"https://example.com","client_id":"id","client_secret":"secret"}"#,
        )
        .unwrap();

        let config = load_saved_config(Some(dir.path().to_path_buf()))
            .unwrap()
            .unwrap();
        assert_eq!(config.tenant_url, "https://example.com");
        assert_eq!(config.client_id, "id");
        assert_eq!(config.client_secret, "secret");
    }
}
