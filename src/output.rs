//! Output formatting for CLI results.
//!
//! This module handles formatting ODC API responses for terminal display:
//!
//! # Color Support
//!
//! - [`paint()`]: Apply ANSI color codes to strings
//! - [`scalar()`]: Format values with status-based colors (green=success, red=failed, yellow=pending)
//! - [`ColorMode`]: Auto/Always/Never color output
//!
//! # Table Rendering
//!
//! - [`render_table()`]: Format homogeneous object arrays as aligned tab-delimited tables
//! - [`field_order()`]: Determine field display order (name, key, status first, then alphabetical)
//!
//! # Pretty Printing
//!
//! - [`render_pretty()`]: Format JSON with proper indentation and type-aware rendering
//! - [`write_result()`]: Output formatter dispatch (JSON or human-readable)

use anyhow::Result;
use serde_json::Value;
use std::io::{self, IsTerminal, Write};
use std::sync::Mutex;
use tabwriter::TabWriter;

/// Terminal color mode preference.
///
/// Derives `clap::ValueEnum` so `--color` parsing (and its `auto`/`always`/`never` possible
/// values) comes straight from clap instead of a hand-written parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorMode {
    /// Use colors if stdout is a terminal and NO_COLOR is not set
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

pub struct Output {
    pub json: bool,
    pub color: ColorMode,
    lock: Mutex<()>,
}

impl Output {
    pub fn new(json: bool, color: ColorMode) -> Self {
        Self {
            json,
            color,
            lock: Mutex::new(()),
        }
    }

    pub fn color_enabled(&self) -> bool {
        match self.color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => {
                if std::env::var("NO_COLOR").is_ok() {
                    return false;
                }
                if std::env::var("TERM").map(|t| t == "dumb").unwrap_or(false) {
                    return false;
                }
                io::stdout().is_terminal()
            }
        }
    }

    pub fn print_result(&self, value: &Value) -> Result<()> {
        let _guard = self.lock.lock().unwrap();
        let color = self.color_enabled();
        write_result(&mut io::stdout(), value, self.json, color)?;
        Ok(())
    }

    pub fn println_locked(&self, msg: &str) {
        let _guard = self.lock.lock().unwrap();
        if self.json {
            eprintln!("{}", msg);
        } else {
            println!("{}", msg);
        }
    }

    pub fn stderr(&self, msg: &str) {
        let _guard = self.lock.lock().unwrap();
        eprintln!("{}", msg);
    }
}

/// Apply ANSI color code to a string if colors are enabled.
///
/// # Arguments
///
/// - `s`: Text to colorize
/// - `code`: ANSI color code (e.g., "32" for green, "1;36" for bright cyan)
/// - `apply_color`: Whether to actually apply color or return plain text
///
/// # Examples
///
/// ```ignore
/// paint("success", "32", true)  // "\x1b[32msuccess\x1b[0m"
/// paint("success", "32", false) // "success"
/// ```
pub fn paint(s: &str, code: &str, apply_color: bool) -> String {
    if apply_color {
        format!("\x1b[{}m{}\x1b[0m", code, s)
    } else {
        s.to_string()
    }
}

/// Transform a field name to human-readable label.
///
/// Converts:
/// - snake_case/kebab-case to Title Case
/// - assetKey/Asset → appKey/App
///
/// # Examples
///
/// ```ignore
/// label("assetKey")         // "App Key"
/// label("deploymentTime")   // "Deployment Time"
/// ```
pub fn label(s: &str) -> String {
    // Replace asset -> app, Asset -> App
    let s = s.replace("asset", "app").replace("Asset", "App");

    let mut result = String::new();
    let mut prev = '\0';

    for (i, ch) in s.chars().enumerate() {
        match ch {
            '_' | '-' => {
                result.push(' ');
                prev = ch;
                continue;
            }
            _ => {
                if i > 0 && ch.is_uppercase() && (prev.is_lowercase() || prev.is_numeric()) {
                    result.push(' ');
                }
                if i == 0 {
                    result.push_str(&ch.to_uppercase().to_string());
                } else {
                    result.push(ch);
                }
                prev = ch;
            }
        }
    }

    result
}

/// Determine the display order for object fields in tables and formatted output.
///
/// Prioritizes commonly-used fields (name, key, status) first, then sorts remaining fields
/// alphabetically. This makes human-readable output more scannable.
///
/// # Priority Order
///
/// 1. name
/// 2. key / assetKey
/// 3. type
/// 4. status
/// 5. Remaining fields (alphabetical)
pub fn field_order(map: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();

    let mut ordered = Vec::new();
    for key in &["name", "key", "assetKey", "type", "status"] {
        if map.contains_key(*key) {
            ordered.push(key.to_string());
        }
    }

    for key in keys {
        if !ordered.contains(&key) {
            ordered.push(key);
        }
    }

    ordered
}

/// Format a JSON value as a human-readable string with status-based coloring.
///
/// Applies ANSI color codes based on value content:
/// - Green: "success", "succeeded", "completed", "true", "active"
/// - Red: "failed", "failure", "error", "false"
/// - Yellow: "pending", "running", "inprogress", "queued"
///
/// Null values display as "—" (em dash).
pub fn scalar(v: &Value, apply_color: bool) -> String {
    let mut s = match v {
        Value::Null => "—".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(v).unwrap_or_default(),
    };

    // Replace control characters with spaces
    s = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();

    let color_code = match s.to_lowercase().as_str() {
        "success" | "succeeded" | "completed" | "true" | "active" => Some("32"),
        "failed" | "failure" | "error" | "false" => Some("31"),
        "pending" | "running" | "inprogress" | "in progress" | "queued" => Some("33"),
        _ => None,
    };

    if let Some(code) = color_code {
        paint(&s, code, apply_color)
    } else {
        s
    }
}

/// Render a JSON value in pretty-printed human-readable format.
///
/// - Objects: Display fields with labels and indentation
/// - Arrays: Render as tables if homogeneous objects, otherwise as lists
/// - Scalars: Display with status-based coloring
pub fn render_pretty(w: &mut dyn Write, v: &Value, indent: &str, apply_color: bool) -> Result<()> {
    match v {
        Value::Object(map) => {
            if map.is_empty() {
                writeln!(w, "{}(empty)", indent)?;
                return Ok(());
            }

            for key in field_order(map) {
                if let Some(value) = map.get(&key) {
                    let heading = format!("{}{}", indent, paint(&label(&key), "1;36", apply_color));
                    match value {
                        Value::Object(_) | Value::Array(_) => {
                            writeln!(w, "{}:", heading)?;
                            render_pretty(w, value, &format!("{}  ", indent), apply_color)?;
                        }
                        _ => {
                            writeln!(w, "{}: {}", heading, scalar(value, apply_color))?;
                        }
                    }
                }
            }
        }
        Value::Array(arr) => {
            if arr.is_empty() {
                writeln!(w, "{}No results.", indent)?;
                return Ok(());
            }

            // Check if this is a table (homogeneous array of flat maps)
            let mut union = serde_json::Map::new();
            let mut is_table = true;

            for row in arr {
                match row {
                    Value::Object(m) if !m.is_empty() => {
                        for (k, cell) in m {
                            match cell {
                                Value::Object(_) | Value::Array(_) => is_table = false,
                                _ => {}
                            }
                            union.insert(k.clone(), Value::Null);
                        }
                    }
                    _ => {
                        is_table = false;
                        break;
                    }
                }
            }

            if is_table && !union.is_empty() {
                render_table(w, arr, &union, indent, apply_color)?;
            } else {
                // Render as list
                for (i, row) in arr.iter().enumerate() {
                    match row {
                        Value::Object(_) | Value::Array(_) => {
                            writeln!(w, "{}[{}]", indent, i + 1)?;
                            render_pretty(w, row, &format!("{}  ", indent), apply_color)?;
                        }
                        _ => {
                            writeln!(w, "{}• {}", indent, scalar(row, apply_color))?;
                        }
                    }
                }
            }
        }
        _ => {
            writeln!(w, "{}{}", indent, scalar(v, apply_color))?;
        }
    }
    Ok(())
}

fn render_table(
    w: &mut dyn Write,
    rows: &[Value],
    union: &serde_json::Map<String, Value>,
    indent: &str,
    apply_color: bool,
) -> Result<()> {
    let keys = field_order(union);

    // First render to buffer without colors to get proper tab alignment
    let mut table_buf = Vec::new();
    let mut tw = TabWriter::new(&mut table_buf);

    // Write header
    let headers: Vec<String> = keys.iter().map(|k| label(k).to_uppercase()).collect();
    writeln!(tw, "{}", headers.join("\t"))?;

    // Write rows with color disabled for alignment
    for row in rows {
        if let Value::Object(m) = row {
            let cells: Vec<String> = keys
                .iter()
                .map(|k| scalar(m.get(k).unwrap_or(&Value::Null), false))
                .collect();
            writeln!(tw, "{}", cells.join("\t"))?;
        }
    }
    tw.flush()?;

    // Now render the table with colors applied after alignment
    let table_str = String::from_utf8(table_buf)?;
    let lines: Vec<&str> = table_str.trim_end().split('\n').collect();

    for (i, line) in lines.iter().enumerate() {
        let formatted_line = if i == 0 {
            // Header line: paint it
            paint(line, "1;36", apply_color)
        } else if apply_color && rows.len() > i - 1 {
            // Data line: apply status color if present
            if let Value::Object(m) = &rows[i - 1] {
                if let Some(status_val) = m.get("status") {
                    let status = scalar(status_val, false);
                    let styled = scalar(status_val, true);
                    if styled != status {
                        line.replacen(&status, &styled, 1)
                    } else {
                        line.to_string()
                    }
                } else {
                    line.to_string()
                }
            } else {
                line.to_string()
            }
        } else {
            line.to_string()
        };

        writeln!(w, "{}{}", indent, formatted_line)?;
    }

    writeln!(w, "{}{} results", indent, rows.len())?;
    Ok(())
}

/// Output a result using either JSON or human-readable format.
///
/// - If `as_json` is true: Pretty-print JSON
/// - Otherwise: Use [`render_pretty()`] for human-readable format
pub fn write_result(
    w: &mut dyn Write,
    value: &Value,
    as_json: bool,
    apply_color: bool,
) -> Result<()> {
    let encoded = serde_json::to_string_pretty(value)?;

    if as_json {
        writeln!(w, "{}", encoded)?;
        return Ok(());
    }

    // Parse with arbitrary precision preservation
    let normalized: Value = serde_json::from_str(&encoded)?;

    render_pretty(w, &normalized, "", apply_color)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_label_transforms() {
        assert_eq!(label("assetKey"), "App Key");
        assert_eq!(label("deploymentDateTime"), "Deployment Date Time");
        assert_eq!(label("name"), "Name");
    }

    #[test]
    fn test_scalar_colors() {
        let success = json!("success");
        let colored = scalar(&success, true);
        assert!(colored.contains("\x1b[32m"));

        let failed = json!("failed");
        let colored = scalar(&failed, true);
        assert!(colored.contains("\x1b[31m"));
    }

    #[test]
    fn test_scalar_null() {
        let null_val = json!(null);
        assert_eq!(scalar(&null_val, false), "—");
    }

    #[test]
    fn test_field_order() {
        let mut map = serde_json::Map::new();
        map.insert("status".to_string(), json!("success"));
        map.insert("name".to_string(), json!("test"));
        map.insert("zebra".to_string(), json!(1));
        map.insert("key".to_string(), json!("k1"));

        let order = field_order(&map);
        assert_eq!(&order[0], "name");
        assert_eq!(&order[1], "key");
        assert_eq!(&order[2], "status");
        assert_eq!(&order[3], "zebra");
    }
}
