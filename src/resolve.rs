use anyhow::anyhow;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

fn guid_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap()
    })
}

/// Whether `input` looks like a GUID (the API's key format).
pub fn is_guid(input: &str) -> bool {
    guid_pattern().is_match(input)
}

fn contains(value: &Value, query: &str) -> bool {
    let value_str = crate::value::str(value).to_lowercase();
    value_str.contains(&query.to_lowercase())
}

/// Configuration for generic resolution behavior.
pub struct ResolveConfig {
    /// Kind name for error messages (e.g., "app", "role", "environment")
    pub kind: String,
    /// Allow matching on partial name/key (lenient), or require exact match (strict)
    pub allow_partial_match: bool,
    /// API field name that contains the identifier key (e.g., "assetKey", "key")
    pub key_field: String,
    /// Maximum suggestions to show in error messages
    pub max_suggestions: usize,
    /// If true, exact name matches have highest priority. If false, treat name and key equally.
    pub prefer_exact_name: bool,
}

impl ResolveConfig {
    /// Configuration for strict app resolution (exact match required, name-preferred)
    pub fn for_app() -> Self {
        ResolveConfig {
            kind: "app".to_string(),
            allow_partial_match: false,
            key_field: "assetKey".to_string(),
            max_suggestions: 10,
            prefer_exact_name: true,
        }
    }

    /// Configuration for lenient resolution (partial match allowed)
    pub fn for_kind(kind: &str, key_field: &str) -> Self {
        ResolveConfig {
            kind: kind.to_string(),
            allow_partial_match: true,
            key_field: key_field.to_string(),
            max_suggestions: 10,
            prefer_exact_name: false,
        }
    }
}

/// Generic resolver: resolve a user-supplied identifier to an API key.
///
/// Supports exact name matching, partial matching, GUID pass-through, and suggestions.
///
/// # Matching Strategy
///
/// 1. If `input` is a GUID, return it directly (short-circuit)
/// 2. Find exact matches on `name` field (case-insensitive)
/// 3. Find partial matches on `name` and `key_field` (case-insensitive substring)
/// 4. Apply resolution rules based on `config`:
///    - Strict (apps): require exactly one exact match
///    - Lenient (others): allow single partial match if no exact match
/// 5. Return key of unique match, or error with suggestions
///
/// # Returns
///
/// The value of `config.key_field` for the matched item.
///
/// # Errors
///
/// - If no matches found
/// - If multiple exact matches (ambiguous)
/// - If strict mode and only partial matches exist
pub fn resolve_generic(
    input: &str,
    config: &ResolveConfig,
    items: &[serde_json::Map<String, Value>],
) -> anyhow::Result<String> {
    if input.is_empty() {
        return Err(anyhow!("{} is required", config.kind));
    }

    // Short-circuit GUID
    if guid_pattern().is_match(input) {
        return Ok(input.to_string());
    }

    let mut matches = Vec::new();
    let mut exact_name = Vec::new();

    for item in items {
        // Check name field
        if let Some(name) = item.get("name") {
            let name_str = crate::value::str(name);
            if contains(name, input) {
                matches.push(item.clone());
            }
            if name_str.eq_ignore_ascii_case(input) {
                exact_name.push(item.clone());
            }
        }

        // Check key field (avoid duplicates from name check)
        if let Some(key) = item.get(&config.key_field) {
            if contains(key, input) && !matches.iter().any(|m| m == item) {
                matches.push(item.clone());
            }
        }
    }

    // Apply strict/lenient resolution rules
    let mut result: Option<String> = None;

    // Prefer exact name match if found
    if exact_name.len() == 1 {
        result = crate::value::require_string(
            exact_name[0].get(&config.key_field).unwrap_or(&Value::Null),
            &format!("{} key", config.kind),
        )
        .ok();
    } else if exact_name.len() > 1 {
        // Multiple exact name matches: error
        return Err(anyhow!(
            "Multiple {}s match the name (ambiguous): {}",
            config.kind,
            exact_name.len()
        ));
    }

    // Strict mode: require exact match
    if result.is_none() && !config.allow_partial_match && exact_name.is_empty() {
        let suggestions: Vec<String> = matches
            .iter()
            .take(config.max_suggestions)
            .map(|item| {
                format!(
                    "{} ({})",
                    crate::value::str(item.get("name").unwrap_or(&Value::Null)),
                    crate::value::str(item.get(&config.key_field).unwrap_or(&Value::Null))
                )
            })
            .collect();
        return Err(anyhow!(
            "No exact match for {:?}. Did you mean: {}",
            input,
            suggestions.len()
        ));
    }

    // Lenient mode: allow single partial match if no exact match
    if result.is_none() && config.allow_partial_match && matches.len() == 1 {
        result = crate::value::require_string(
            matches[0].get(&config.key_field).unwrap_or(&Value::Null),
            &format!("{} key", config.kind),
        )
        .ok();
    }

    if let Some(key) = result {
        return Ok(key);
    }

    // Build error response with suggestions
    let display_items = if matches.is_empty() {
        items.to_vec()
    } else {
        matches
    };

    let suggestions: Vec<String> = display_items
        .iter()
        .take(config.max_suggestions)
        .map(|item| {
            format!(
                "{} ({})",
                crate::value::str(item.get("name").unwrap_or(&Value::Null)),
                crate::value::str(item.get(&config.key_field).unwrap_or(&Value::Null))
            )
        })
        .collect();

    Err(anyhow!(
        "No {}s found matching {:?}. {}",
        config.kind,
        input,
        if suggestions.is_empty() {
            "No suggestions available.".to_string()
        } else {
            format!("Did you mean: {}", suggestions.join(", "))
        }
    ))
}

/// Resolve a name or GUID to a key for a given kind (app, environment, user, role)
pub fn resolve(
    input: &str,
    kind: &str,
    items: &[serde_json::Map<String, Value>],
    key_field: &str,
) -> anyhow::Result<String> {
    let config = if kind == "app" {
        ResolveConfig::for_app()
    } else {
        ResolveConfig::for_kind(kind, key_field)
    };
    resolve_generic(input, &config, items)
}

/// Resolve a role name by name and optional app key
///
/// Uses lenient matching: returns single partial match if no exact match found.
/// See [`resolve_generic`] for matching strategy details.
pub fn resolve_role(
    name: &str,
    roles: &[serde_json::Map<String, Value>],
) -> anyhow::Result<String> {
    let config = ResolveConfig::for_kind("role", "key");
    resolve_generic(name, &config, roles)
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
