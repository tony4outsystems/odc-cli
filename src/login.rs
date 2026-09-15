use crate::settings::Settings;
use anyhow::{anyhow, Result};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use url::Url;

/// Get the path to the config file
pub fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot determine home directory"))?;
    Ok(home.join(".odc").join("config.json"))
}

/// Prompt for and read the client secret from stdin
pub fn prompt_secret() -> Result<String> {
    use std::io::IsTerminal;

    if !std::io::stdin().is_terminal() {
        return Err(anyhow!(
            "login requires an interactive terminal to enter the client secret"
        ));
    }

    eprint!("Client secret: ");
    std::io::stderr().flush()?;

    let secret = rpassword::read_password()?;

    Ok(secret)
}

/// Execute the login command
pub fn login(tenant_url: &str, client_id: &str) -> Result<()> {
    // Validate tenant URL
    let url = Url::parse(tenant_url)?;

    if url.host().is_none() {
        return Err(anyhow!(
            "tenant URL must be an absolute HTTP or HTTPS URL without credentials, query, or fragment"
        ));
    }

    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(anyhow!(
            "tenant URL must be an absolute HTTP or HTTPS URL without credentials, query, or fragment"
        ));
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(anyhow!(
            "tenant URL must be an absolute HTTP or HTTPS URL without credentials, query, or fragment"
        ));
    }

    if url.query().is_some() || url.fragment().is_some() {
        return Err(anyhow!(
            "tenant URL must be an absolute HTTP or HTTPS URL without credentials, query, or fragment"
        ));
    }

    if client_id.trim().is_empty() {
        return Err(anyhow!("client ID must not be empty"));
    }

    // Prompt for secret
    let secret = prompt_secret()?;
    if secret.trim().is_empty() {
        return Err(anyhow!("client secret must not be empty"));
    }

    // Save settings
    // Preserve any existing mentor credentials already on disk.
    let existing = load_existing_settings().unwrap_or_default();

    let path = save_settings(&Settings {
        tenant_url: tenant_url.to_string(),
        client_id: client_id.to_string(),
        client_secret: secret,
        ..existing
    })?;

    // Output result
    let result = json!({"configuration": path.to_string_lossy().to_string()});
    let output = std::sync::Arc::new(crate::output::Output::new(
        false,
        crate::output::ColorMode::Never,
    ));
    output.print_result(&result)?;

    Ok(())
}

/// Load the settings currently on disk, if any, so a `login`/`login-mentor` call only
/// overwrites the fields it's responsible for.
fn load_existing_settings() -> Result<Settings> {
    let path = config_path()?;
    let data = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

/// Execute the login-mentor command: saves Mentor's own OAuth2 client credentials
/// (separate from the main ODC API client) alongside any existing configuration.
pub fn login_mentor(token_url: &str, client_id: &str) -> Result<()> {
    let url = Url::parse(token_url)?;
    if url.host().is_none() || (url.scheme() != "https" && url.scheme() != "http") {
        return Err(anyhow!("token URL must be an absolute HTTP or HTTPS URL"));
    }

    if client_id.trim().is_empty() {
        return Err(anyhow!("client ID must not be empty"));
    }

    let secret = prompt_secret()?;
    if secret.trim().is_empty() {
        return Err(anyhow!("client secret must not be empty"));
    }

    let existing = load_existing_settings().unwrap_or_default();

    let path = save_settings(&Settings {
        mentor_token_url: Some(token_url.to_string()),
        mentor_client_id: Some(client_id.to_string()),
        mentor_client_secret: Some(secret),
        ..existing
    })?;

    let result = json!({"configuration": path.to_string_lossy().to_string()});
    let output = std::sync::Arc::new(crate::output::Output::new(
        false,
        crate::output::ColorMode::Never,
    ));
    output.print_result(&result)?;

    Ok(())
}

/// Save settings to ~/.odc/config.json atomically
fn save_settings(settings: &Settings) -> Result<PathBuf> {
    let path = config_path()?;

    // Create directory with mode 0700
    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("Invalid config path"))?;
    fs::create_dir_all(dir)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }

    // Serialize settings
    let json = serde_json::to_string_pretty(&settings)?;
    let data = format!("{}\n", json);

    // Write to temp file and rename atomically
    let temp_file = tempfile::NamedTempFile::new_in(dir)?;
    let mut file = temp_file.as_file();
    file.write_all(data.as_bytes())?;
    file.flush()?;
    let _ = file;

    temp_file.persist(&path)?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path() {
        let path = config_path();
        assert!(path.is_ok());
        let p = path.unwrap();
        assert!(p.to_string_lossy().contains(".odc"));
        assert!(p.to_string_lossy().contains("config.json"));
    }

    #[test]
    fn test_tenant_url_validation() {
        let result = login("https://example.com", "");
        assert!(result.is_err());

        let result = login("not-a-url", "client-id");
        assert!(result.is_err());
    }
}
