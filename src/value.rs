use anyhow::{anyhow, Result};
use serde_json::Value;

/// Extract a string from a value
pub fn str(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// Extract an integer from a value, accepting integers, string numbers, and floats
/// Returns (value, ok)
pub fn integer(v: &Value) -> (i64, bool) {
    match v {
        Value::Number(n) => {
            // Try to parse as i64
            if let Some(i) = n.as_i64() {
                (i, true)
            } else if let Ok(s) = n.to_string().parse::<i64>() {
                (s, true)
            } else {
                (0, false)
            }
        }
        Value::String(s) => {
            if let Ok(i) = s.parse::<i64>() {
                (i, true)
            } else {
                (0, false)
            }
        }
        _ => (0, false),
    }
}

/// Get a string field from a map, returning empty string if not found or not a string.
pub fn get_string(map: &serde_json::Map<String, Value>, key: &str) -> String {
    map.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Get an optional i64 field from a map.
pub fn get_i64(map: &serde_json::Map<String, Value>, key: &str) -> Option<i64> {
    map.get(key).and_then(|v| v.as_i64())
}

/// Get an optional boolean field from a map.
pub fn get_bool(map: &serde_json::Map<String, Value>, key: &str) -> Option<bool> {
    map.get(key).and_then(|v| v.as_bool())
}

/// Get an array field from a map, returning empty vector if not found or not an array.
pub fn get_array(map: &serde_json::Map<String, Value>, key: &str) -> Vec<Value> {
    map.get(key)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
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

/// Return the first non-nil, non-empty value from a list
pub fn first(values: &[&Value]) -> Option<Value> {
    for v in values {
        if !v.is_null() {
            match v {
                Value::String(s) if s.is_empty() => continue,
                _ => return Some((*v).clone()),
            }
        }
    }
    None
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

/// Check if any entry in a summary has "failed" status
pub fn has_failed(summary: &[serde_json::Map<String, Value>]) -> bool {
    for entry in summary {
        if entry.get("status").and_then(|v| v.as_str()) == Some("failed") {
            return true;
        }
    }
    false
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
    fn test_integer_from_number() {
        let v = json!(42);
        let (n, ok) = integer(&v);
        assert!(ok);
        assert_eq!(n, 42);
    }

    #[test]
    fn test_integer_from_string() {
        let v = json!("123");
        let (n, ok) = integer(&v);
        assert!(ok);
        assert_eq!(n, 123);
    }

    #[test]
    fn test_integer_from_invalid() {
        let v = json!("not a number");
        let (_n, ok) = integer(&v);
        assert!(!ok);
    }

    #[test]
    fn test_first_with_values() {
        let v1 = json!(null);
        let v2 = json!("hello");
        let v3 = json!("world");
        let result = first(&[&v1, &v2, &v3]);
        assert_eq!(result, Some(json!("hello")));
    }

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
    fn test_has_failed() {
        let mut map1 = serde_json::Map::new();
        map1.insert("status".to_string(), json!("success"));

        let mut map2 = serde_json::Map::new();
        map2.insert("status".to_string(), json!("failed"));

        assert!(!has_failed(&[map1.clone()]));
        assert!(has_failed(&[map1, map2]));
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
