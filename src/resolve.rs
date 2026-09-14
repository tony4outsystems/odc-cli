use anyhow::anyhow;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

fn guid_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
            .unwrap()
    })
}

fn contains(value: &Value, query: &str) -> bool {
    let value_str = crate::value::str(value).to_lowercase();
    value_str.contains(&query.to_lowercase())
}

/// Resolve a name or GUID to a key for a given kind (app, environment, user, role)
pub fn resolve(
    input: &str,
    kind: &str,
    items: &[serde_json::Map<String, Value>],
    key_field: &str,
) -> anyhow::Result<String> {
    if input.is_empty() {
        return Err(anyhow!("{} is required", kind));
    }
    
    // Short-circuit GUID
    if guid_pattern().is_match(input) {
        return Ok(input.to_string());
    }
    
    let mut matches = Vec::new();
    let mut exact = Vec::new();
    
    for item in items {
        // Check for name or key match (case-insensitive)
        if let Some(name) = item.get("name") {
            if contains(name, input) {
                matches.push(item.clone());
            }
            if crate::value::str(name).eq_ignore_ascii_case(input) {
                exact.push(item.clone());
            }
        }
        
        // Also check the key field
        if let Some(key) = item.get(key_field) {
            if contains(key, input) && !matches.iter().any(|m| m == item) {
                matches.push(item.clone());
            }
        }
    }
    
    // For apps, require exact match
    if kind == "app" && !exact.is_empty() && exact.len() == 1 {
        return crate::value::require_string(
            exact[0].get(key_field).unwrap_or(&Value::Null),
            &format!("{} key", kind),
        );
    }
    
    // For other kinds, allow single partial match if no exact match
    if exact.is_empty() && kind != "app" && matches.len() == 1 {
        return crate::value::require_string(
            matches[0].get(key_field).unwrap_or(&Value::Null),
            &format!("{} key", kind),
        );
    }
    
    // If we have exactly one exact match, return it
    if exact.len() == 1 {
        return crate::value::require_string(
            exact[0].get(key_field).unwrap_or(&Value::Null),
            &format!("{} key", kind),
        );
    }
    
    // Build error message
    let mut message = if exact.len() > 1 {
        format!("Multiple {}s match the name (ambiguous):", kind)
    } else if matches.is_empty() {
        format!("No {}s found matching {:?}", kind, input)
    } else {
        format!("No exact match for {:?}. Did you mean:", input)
    };
    
    let display_items = if exact.len() > 1 {
        exact
    } else if matches.is_empty() && kind == "environment" {
        message.push_str("\nAvailable environments:");
        items.to_vec()
    } else {
        matches
    };
    
    for item in display_items.iter().take(10) {
        let name = crate::value::str(item.get("name").unwrap_or(&Value::Null));
        let key = crate::value::str(item.get(key_field).unwrap_or(&Value::Null));
        message.push_str(&format!("\n  - {} ({})", name, key));
    }
    
    if display_items.len() > 10 {
        message.push_str(&format!("\n  ... and {} more", display_items.len() - 10));
    }
    
    Err(anyhow!(message))
}

/// Resolve a role name by name and optional app key
pub fn resolve_role(
    name: &str,
    roles: &[serde_json::Map<String, Value>],
) -> anyhow::Result<String> {
    if name.is_empty() {
        return Err(anyhow!("role name is required"));
    }
    
    // Short-circuit GUID
    if guid_pattern().is_match(name) {
        return Ok(name.to_string());
    }
    
    let mut matches = Vec::new();
    let mut exact = Vec::new();
    
    for role in roles {
        if let Some(role_name) = role.get("name") {
            if contains(role_name, name) {
                matches.push(role.clone());
            }
            if crate::value::str(role_name).eq_ignore_ascii_case(name) {
                exact.push(role.clone());
            }
        }
    }
    
    // If exactly one exact match, return it
    if exact.len() == 1 {
        return crate::value::require_string(
            exact[0].get("key").unwrap_or(&Value::Null),
            "role key",
        );
    }
    
    // If no exact match but one partial match, return it
    if exact.is_empty() && matches.len() == 1 {
        return crate::value::require_string(
            matches[0].get("key").unwrap_or(&Value::Null),
            "role key",
        );
    }
    
    // Build error message
    let mut message = if exact.len() > 1 {
        format!(
            "Multiple roles named {:?} (ambiguous); use --app to narrow down:",
            name
        )
    } else if matches.is_empty() {
        format!("No application roles found matching {:?}", name)
    } else {
        format!("No exact match for role {:?}. Did you mean:", name)
    };
    
    let display_items = if exact.len() > 1 { exact } else { matches };
    
    for role in display_items.iter().take(10) {
        let role_name = crate::value::str(role.get("name").unwrap_or(&Value::Null));
        let app_key = crate::value::str(role.get("assetKey").unwrap_or(&Value::Null));
        message.push_str(&format!("\n  - {} (app {})", role_name, app_key));
    }
    
    if display_items.len() > 10 {
        message.push_str(&format!("\n  ... and {} more", display_items.len() - 10));
    }
    
    Err(anyhow!(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_guid_pattern() {
        let valid_guid = "550e8400-e29b-41d4-a716-446655440000";
        assert!(guid_pattern().is_match(valid_guid));
        assert!(guid_pattern().is_match(&valid_guid.to_uppercase()));
        
        let invalid = "not-a-guid";
        assert!(!guid_pattern().is_match(invalid));
    }

    #[test]
    fn test_contains_case_insensitive() {
        let value = json!("TestValue");
        assert!(contains(&value, "test"));
        assert!(contains(&value, "VALUE"));
        assert!(!contains(&value, "notfound"));
    }

    #[test]
    fn test_resolve_exact_match() {
        let mut app = serde_json::Map::new();
        app.insert("name".to_string(), json!("MyApp"));
        app.insert("assetKey".to_string(), json!("app-key-123"));
        
        let result = resolve("MyApp", "app", &[app], "assetKey");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "app-key-123");
    }

    #[test]
    fn test_resolve_guid_short_circuit() {
        let guid = "550e8400-e29b-41d4-a716-446655440000";
        let result = resolve(guid, "app", &[], "assetKey");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), guid);
    }
}
