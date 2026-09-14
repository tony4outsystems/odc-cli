use crate::value::objects;
use serde_json::{Value};
use sha1_smol::Sha1;
use std::collections::{BTreeMap, HashSet};

pub fn mermaid_node_id(app: &serde_json::Map<String, Value>) -> String {
    let revision = if let Some(Value::Number(n)) = app.get("revision") {
        n.to_string()
    } else {
        String::new()
    };
    
    let key_val = app.get("key").and_then(|v| v.as_str()).unwrap_or("unknown");
    
    let digest = Sha1::from(format!("{}:{}", key_val, revision).as_bytes());
    let bytes = digest.digest().bytes();
    let hex: String = bytes.iter().take(6).map(|b| format!("{:02x}", b)).collect();
    format!("app_{}", hex)
}

pub fn mermaid_label(app: &serde_json::Map<String, Value>) -> String {
    let label = if let Some(Value::String(s)) = app.get("name") {
        s.clone()
    } else if let Some(Value::String(s)) = app.get("key") {
        s.clone()
    } else {
        "Unknown app".to_string()
    };
    
    let mut details = Vec::new();
    if app.contains_key("revision") {
        details.push(format!("rev {}", crate::value::go_fmt(app.get("revision").unwrap())));
    }
    if let Some(Value::String(s)) = app.get("type") {
        if !s.is_empty() {
            details.push(s.clone());
        }
    }
    
    let full_label = if !details.is_empty() {
        format!("{}\n{}", label, details.join(" / "))
    } else {
        label
    };
    
    // Escape backslash and quotes
    full_label.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn render_producer_graph(
    root: &serde_json::Map<String, Value>,
    producers: &[serde_json::Map<String, Value>],
) -> String {
    let mut nodes = BTreeMap::new();
    let mut edges = HashSet::new();
    
    fn visit(
        parent: &serde_json::Map<String, Value>,
        children: &[serde_json::Map<String, Value>],
        nodes: &mut BTreeMap<String, String>,
        edges: &mut HashSet<String>,
    ) {
        let id = mermaid_node_id(parent);
        nodes.insert(id.clone(), mermaid_label(parent));
        
        for child in children {
            let child_id = mermaid_node_id(child);
            edges.insert(format!("{} --> {}", id, child_id));
            
            let child_producers = objects(child.get("producers").unwrap_or(&Value::Null));
            visit(child, &child_producers, nodes, edges);
        }
    }
    
    visit(root, producers, &mut nodes, &mut edges);
    
    let mut lines = vec![
        "---".to_string(),
        "title: Producer dependency graph".to_string(),
        "---".to_string(),
        "flowchart LR".to_string(),
    ];
    
    for (id, label) in nodes {
        lines.push(format!("    {}[\"{}\" ]", id, label));
    }
    
    let mut edges_sorted: Vec<_> = edges.into_iter().collect();
    edges_sorted.sort();
    for edge in edges_sorted {
        lines.push(format!("    {}", edge));
    }
    
    lines.join("\n") + "\n"
}

pub fn safe_file_token(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub fn default_mermaid_path(key: &str, revision: i64) -> String {
    format!(
        "producer-graph-{}-rev-{}.mmd",
        safe_file_token(key),
        revision
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_mermaid_node_id() {
        let mut app = serde_json::Map::new();
        app.insert("key".to_string(), json!("test-app"));
        app.insert("revision".to_string(), json!(1));
        
        let id = mermaid_node_id(&app);
        assert!(id.starts_with("app_"));
        assert_eq!(id.len(), 4 + 12); // "app_" + 12 hex chars for 6 bytes
    }

    #[test]
    fn test_safe_file_token() {
        assert_eq!(safe_file_token("My-App_123"), "My-App_123");
        assert_eq!(safe_file_token("My App!@#"), "My_App___");
    }

    #[test]
    fn test_default_mermaid_path() {
        let path = default_mermaid_path("my-app", 42);
        assert_eq!(path, "producer-graph-my-app-rev-42.mmd");
    }

    #[test]
    fn test_mermaid_label_escaping() {
        let mut app = serde_json::Map::new();
        app.insert("name".to_string(), json!("App \"Test\""));
        app.insert("type".to_string(), json!("Type\\Path"));
        
        let label = mermaid_label(&app);
        assert!(label.contains("\\\""));
        assert!(label.contains("\\\\"));
    }
}
