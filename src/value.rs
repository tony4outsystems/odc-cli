use anyhow::{anyhow, Result};
use serde_json::Value;

/// Extract a string from a value
pub fn str(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// Convenience accessors for `Map<String, Value>`, replacing the repeated
/// `.get(field).and_then(|v| v.as_str())...` chains scattered across `client.rs` and
/// `commands/*.rs`.
pub trait JsonMapExt {
    /// The field's value as a `&str`, or `None` if missing or not a string.
    fn str_field(&self, key: &str) -> Option<&str>;

    /// The field's value as a `&str`, or `""` if missing or not a string. Equivalent to the old
    /// `commands::shared::status_str`.
    fn str_or_empty(&self, key: &str) -> &str;

    /// The field's value as a `&str`, or an error naming `what` and `key` (matching the
    /// `"{what} has no {key} field"` wording used throughout `commands/*.rs`) if missing.
    fn require_str<'a>(&'a self, key: &str, what: &str) -> Result<&'a str>;

    /// Whether the field is present and equals `value` as a string.
    fn key_eq(&self, key: &str, value: &str) -> bool;
}

impl JsonMapExt for serde_json::Map<String, Value> {
    fn str_field(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    fn str_or_empty(&self, key: &str) -> &str {
        self.str_field(key).unwrap_or("")
    }

    fn require_str<'a>(&'a self, key: &str, what: &str) -> Result<&'a str> {
        self.str_field(key)
            .ok_or_else(|| anyhow!("{} has no {} field", what, key))
    }

    fn key_eq(&self, key: &str, value: &str) -> bool {
        self.str_field(key) == Some(value)
    }
}

/// Extract an array of objects from a value
pub fn objects(v: &Value) -> Vec<serde_json::Map<String, Value>> {
    match v {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|item| item.as_object().cloned())
            .collect(),
        Value::Object(obj) => vec![obj.clone()],
        _ => vec![],
    }
}

/// Keep only specified fields from a map if they are non-nil and non-empty
pub fn compact_map(
    payload: &serde_json::Map<String, Value>,
    fields: &[&str],
) -> serde_json::Map<String, Value> {
    let mut out = serde_json::Map::new();
    for field in fields {
        if let Some(value) = payload.get(*field) {
            match value {
                Value::Null => continue,
                Value::String(s) if s.is_empty() => continue,
                _ => {
                    out.insert(field.to_string(), value.clone());
                }
            }
        }
    }
    out
}

/// Require a string value with a label for error messages
pub fn require_string(value: &Value, label: &str) -> Result<String> {
    match value {
        Value::String(s) if !s.is_empty() => Ok(s.clone()),
        _ => Err(anyhow!("{} is required", label)),
    }
}

/// Format a value like Go's %v format specifier for maps
/// This is used for error messages and status formatting
pub fn go_fmt(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("\"{}\"", s),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(go_fmt).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            let mut items: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{}:{}", k, go_fmt(v)))
                .collect();
            items.sort(); // Go sorts keys
            format!("map[{}]", items.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_compact_map() {
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), json!("Alice"));
        map.insert("age".to_string(), json!(30));
        map.insert("city".to_string(), json!(""));
        map.insert("country".to_string(), Value::Null);

        let result = compact_map(&map, &["name", "age", "city", "country", "state"]);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("name").and_then(|v| v.as_str()), Some("Alice"));
        assert_eq!(result.get("age").and_then(|v| v.as_i64()), Some(30));
    }

    #[test]
    fn test_json_map_ext_str_field() {
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), json!("Alice"));
        map.insert("age".to_string(), json!(30));

        assert_eq!(map.str_field("name"), Some("Alice"));
        assert_eq!(map.str_field("age"), None); // not a string
        assert_eq!(map.str_field("missing"), None);
    }

    #[test]
    fn test_json_map_ext_str_or_empty() {
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), json!("Alice"));

        assert_eq!(map.str_or_empty("name"), "Alice");
        assert_eq!(map.str_or_empty("missing"), "");
    }

    #[test]
    fn test_json_map_ext_require_str() {
        let mut map = serde_json::Map::new();
        map.insert("assetKey".to_string(), json!("guid-1"));

        assert_eq!(map.require_str("assetKey", "Asset").unwrap(), "guid-1");

        let err = map.require_str("missingKey", "Asset").unwrap_err();
        assert_eq!(err.to_string(), "Asset has no missingKey field");
    }

    #[test]
    fn test_json_map_ext_key_eq() {
        let mut map = serde_json::Map::new();
        map.insert("status".to_string(), json!("Finished"));

        assert!(map.key_eq("status", "Finished"));
        assert!(!map.key_eq("status", "Failed"));
        assert!(!map.key_eq("missing", "Finished"));
    }

    #[test]
    fn test_go_fmt_map() {
        let mut map = serde_json::Map::new();
        map.insert("status".to_string(), json!("Running"));
        map.insert("count".to_string(), json!(5));

        let formatted = go_fmt(&Value::Object(map));
        assert!(formatted.contains("count:5"));
        assert!(formatted.contains("status:\"Running\""));
    }
}
