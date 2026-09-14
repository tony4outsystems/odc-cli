use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub tenant_url: String,
    pub client_id: String,
    pub client_secret: String,
}

impl Settings {
    pub fn tenant_origin(&self) -> String {
        self.tenant_url.trim_end_matches('/').to_string()
    }
}

pub fn load_settings() -> Result<Settings> {
    load_settings_with_paths(None, None)
}

pub fn load_settings_with_paths(cwd: Option<PathBuf>, home: Option<PathBuf>) -> Result<Settings> {
    let mut env_vars = load_environment_with_paths(cwd, home)?;
    
    let required = ["ODC_TENANT_URL", "ODC_CLIENT_ID", "ODC_CLIENT_SECRET"];
    let mut missing = Vec::new();
    
    for name in &required {
        if env_vars.get(*name).map(|v| v.is_empty()).unwrap_or(true) {
            missing.push(*name);
        }
    }
    
    if !missing.is_empty() {
        let missing_str = missing.join(", ");
        return Err(anyhow!("Missing required environment variables: {}", missing_str));
    }
    
    Ok(Settings {
        tenant_url: env_vars.remove("ODC_TENANT_URL").unwrap_or_default(),
        client_id: env_vars.remove("ODC_CLIENT_ID").unwrap_or_default(),
        client_secret: env_vars.remove("ODC_CLIENT_SECRET").unwrap_or_default(),
    })
}

fn load_environment_with_paths(cwd: Option<PathBuf>, home: Option<PathBuf>) -> Result<HashMap<String, String>> {
    // Start with current environment
    let mut env_vars: HashMap<String, String> = std::env::vars().collect();
    
    let start_dir = cwd.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    
    // Walk upward looking for .env
    let mut dir = start_dir.clone();
    loop {
        let env_path = dir.join(".env");
        if env_path.exists() {
            load_dot_env(&env_path, &mut env_vars)?;
            return Ok(env_vars);
        }
        
        let parent = dir.parent();
        if parent.is_none() || parent == Some(dir.as_path()) {
            break;
        }
        dir = parent.unwrap().to_path_buf();
    }
    
    // No .env found, try config file
    let config_path = config_path(home)?;
    if config_path.exists() {
        let data = fs::read_to_string(&config_path)?;
        let settings: Settings = serde_json::from_str(&data)
            .map_err(|_| anyhow!("Invalid configuration in {}", config_path.display()))?;
        
        // Set env vars from config only if not already set
        if std::env::var("ODC_TENANT_URL").is_err() {
            env_vars.insert("ODC_TENANT_URL".to_string(), settings.tenant_url);
        }
        if std::env::var("ODC_CLIENT_ID").is_err() {
            env_vars.insert("ODC_CLIENT_ID".to_string(), settings.client_id);
        }
        if std::env::var("ODC_CLIENT_SECRET").is_err() {
            env_vars.insert("ODC_CLIENT_SECRET".to_string(), settings.client_secret);
        }
    }
    
    Ok(env_vars)
}

fn config_path(home: Option<PathBuf>) -> Result<PathBuf> {
    let home_dir = home.unwrap_or_else(|| {
        dirs::home_dir().unwrap_or_default()
    });
    Ok(home_dir.join(".odc").join("config.json"))
}

fn load_dot_env(path: &Path, env_vars: &mut HashMap<String, String>) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let env_ref_re = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}")?;
    
    let mut lines = content.lines().peekable();
    
    while let Some(mut line) = lines.next() {
        line = line.trim();
        
        // Skip blank and comment lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Strip export prefix
        line = if line.starts_with("export ") {
            line[7..].trim()
        } else {
            line
        };
        
        // Split on first =
        let (key, mut value) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.to_string()),
            None => continue,
        };
        
        if key.is_empty() {
            continue;
        }
        
        value = value.trim().to_string();
        
        // Handle quoted values
        if !value.is_empty() {
            let first_char = value.chars().next().unwrap();
            if first_char == '\'' || first_char == '"' {
                let quote = first_char;
                let mut end_pos = find_quoted_end(&value, quote);
                
                while end_pos < 0 {
                    if let Some(next_line) = lines.next() {
                        value.push('\n');
                        value.push_str(next_line);
                        end_pos = find_quoted_end(&value, quote);
                    } else {
                        return Err(anyhow!("Unterminated quoted value for {} in {}", key, path.display()));
                    }
                }
                
                let end_pos = end_pos as usize;
                value = value[1..end_pos].to_string();
                
                // Unescape based on quote type
                if quote == '"' {
                    value = value
                        .replace(r#"\n"#, "\n")
                        .replace(r#"\r"#, "\r")
                        .replace(r#"\t"#, "\t")
                        .replace(r#"\""#, "\"")
                        .replace(r#"\\"#, "\\");
                } else {
                    value = value
                        .replace(r#"\'"#, "'")
                        .replace(r#"\\"#, "\\");
                }
            } else {
                // Unquoted: handle inline comments
                if let Some(pos) = value.find('#') {
                    if pos > 0 && (value.chars().nth(pos - 1) == Some(' ') || value.chars().nth(pos - 1) == Some('\t')) {
                        value = value[..pos].trim().to_string();
                    }
                }
            }
        }
        
        // Expand ${VAR} and ${VAR:-default}
        value = env_ref_re.replace_all(&value, |caps: &regex::Captures| {
            let var_name = caps.get(1).unwrap().as_str();
            if let Ok(v) = std::env::var(var_name) {
                v
            } else if let Some(v) = env_vars.get(var_name) {
                v.clone()
            } else {
                caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string()
            }
        }).to_string();
        
        // Only set if not already in environment
        if !env_vars.contains_key(key) && std::env::var(key).is_err() {
            env_vars.insert(key.to_string(), value);
        }
    }
    
    Ok(())
}

fn find_quoted_end(value: &str, quote: char) -> i32 {
    let mut escaped = false;
    for (i, ch) in value.chars().enumerate().skip(1) {
        if !escaped && ch == quote {
            return i as i32;
        }
        escaped = !escaped && ch == '\\';
    }
    -1
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_find_quoted_end_double() {
        let value = r#""hello world""#;
        assert_eq!(find_quoted_end(value, '"'), 12);
    }

    #[test]
    fn test_find_quoted_end_single() {
        let value = r"'hello world'";
        assert_eq!(find_quoted_end(value, '\''), 12);
    }

    #[test]
    fn test_find_quoted_end_with_escape() {
        let value = r#""hello \"world"""#;
        assert_eq!(find_quoted_end(value, '"'), 14);
    }
}
