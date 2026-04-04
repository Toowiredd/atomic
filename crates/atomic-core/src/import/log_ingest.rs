//! Log file ingestion — turns log content into atoms.
//!
//! Supports several common log formats and produces a markdown atom with a
//! hierarchical tag path so logs from different sources are easy to browse.

use serde::{Deserialize, Serialize};

/// How the log lines are formatted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    /// Automatically detect the format (default).
    Auto,
    /// One JSON object per line (structured logging).
    JsonLines,
    /// Standard syslog / journald text output.
    Syslog,
    /// Plain text — each line is a log entry.
    PlainText,
}

impl Default for LogFormat {
    fn default() -> Self {
        LogFormat::Auto
    }
}

/// Parameters for ingesting a log file as an atom.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestLogRequest {
    /// Raw log content (text).
    pub content: String,
    /// Format hint. Use `Auto` to let the parser decide.
    #[serde(default)]
    pub format: LogFormat,
    /// Human-readable source label (hostname, service name, filename, …).
    pub source_name: String,
    /// Override the tag hierarchy root.  Defaults to `Logs`.
    pub tag_root: Option<String>,
    /// Optional sub-category below the root, e.g. `System` or `Application`.
    pub tag_category: Option<String>,
}

/// Prepared log atom ready for insertion.
#[derive(Debug, Clone)]
pub struct PreparedLogAtom {
    /// Markdown-formatted content.
    pub content: String,
    /// Tag path segments from root to leaf (e.g. `["Logs", "System", "my-server"]`).
    pub tag_path: Vec<String>,
    /// Number of log lines included.
    pub line_count: usize,
}

/// Prepare a log atom from raw content without writing to the database.
///
/// Returns a `PreparedLogAtom` that can then be inserted by the caller.
pub fn prepare_log_atom(req: &IngestLogRequest) -> PreparedLogAtom {
    let format = if req.format == LogFormat::Auto {
        detect_format(&req.content)
    } else {
        req.format.clone()
    };

    let (formatted_content, line_count) = match format {
        LogFormat::JsonLines => format_json_lines(&req.content),
        LogFormat::Syslog => format_syslog(&req.content),
        LogFormat::PlainText | LogFormat::Auto => format_plain_text(&req.content),
    };

    let root = req
        .tag_root
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("Logs");

    let mut tag_path = vec![root.to_string()];
    if let Some(cat) = &req.tag_category {
        if !cat.is_empty() {
            tag_path.push(cat.clone());
        }
    }
    tag_path.push(req.source_name.clone());

    let header = format!(
        "# Log: {}\n\n**Source:** `{}`  \n**Lines:** {}  \n**Format:** {:?}\n\n---\n\n",
        req.source_name, req.source_name, line_count, req.format
    );
    let content = format!("{header}{formatted_content}");

    PreparedLogAtom { content, tag_path, line_count }
}

// ==================== Format detection ====================

/// Compiled once at first use; matches RFC-3164 (`Jan  1 00:00:00`) and ISO-8601
/// (`2024-01-01T00:00:00`) syslog-style line prefixes.
///
/// Using `OnceLock` avoids recompiling the regex on every `detect_format` call,
/// which would otherwise happen for each log file processed.  The lock ensures
/// the regex is initialized exactly once even under concurrent access.
static SYSLOG_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

fn syslog_re() -> &'static regex::Regex {
    SYSLOG_RE.get_or_init(|| {
        regex::Regex::new(
            r"^(?:[A-Z][a-z]{2}\s+\d+|\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})\s+\S+\s+\S+",
        )
        .expect("SYSLOG_RE is a valid regex")
    })
}

fn detect_format(content: &str) -> LogFormat {
    let first_non_empty = content.lines().find(|l| !l.trim().is_empty()).unwrap_or("");

    if first_non_empty.trim_start().starts_with('{') {
        return LogFormat::JsonLines;
    }

    // Syslog: lines beginning with RFC-3164 timestamp like "Jan  1 00:00:00" or
    // ISO-8601 timestamp "2024-01-01T00:00:00" followed by hostname.
    if syslog_re().is_match(first_non_empty) {
        return LogFormat::Syslog;
    }

    LogFormat::PlainText
}

// ==================== Formatters ====================

fn format_plain_text(content: &str) -> (String, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let count = lines.iter().filter(|l| !l.trim().is_empty()).count();
    let formatted = format!("```\n{}\n```", content.trim());
    (formatted, count)
}

fn format_json_lines(content: &str) -> (String, usize) {
    let mut output = String::from("```json\n");
    let mut count = 0usize;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Pretty-print each JSON object if possible, otherwise include as-is.
        let pretty = serde_json::from_str::<serde_json::Value>(trimmed)
            .ok()
            .and_then(|v| serde_json::to_string_pretty(&v).ok())
            .unwrap_or_else(|| trimmed.to_string());
        output.push_str(&pretty);
        output.push('\n');
        count += 1;
    }
    output.push_str("```");
    (output, count)
}

fn format_syslog(content: &str) -> (String, usize) {
    // Group lines into a fenced code block; syslog is already human-readable.
    let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
    let count = lines.len();
    let formatted = format!("```\n{}\n```", lines.join("\n"));
    (formatted, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text_log() {
        let req = IngestLogRequest {
            content: "error: something broke\nwarn: watch out\n".to_string(),
            format: LogFormat::PlainText,
            source_name: "myapp".to_string(),
            tag_root: None,
            tag_category: Some("Application".to_string()),
        };
        let atom = prepare_log_atom(&req);
        assert!(atom.content.contains("myapp"));
        assert_eq!(atom.line_count, 2);
        assert_eq!(atom.tag_path, vec!["Logs", "Application", "myapp"]);
    }

    #[test]
    fn test_json_lines_format() {
        let req = IngestLogRequest {
            content: r#"{"level":"info","msg":"started"}
{"level":"error","msg":"failed"}"#.to_string(),
            format: LogFormat::JsonLines,
            source_name: "service".to_string(),
            tag_root: Some("Logs".to_string()),
            tag_category: None,
        };
        let atom = prepare_log_atom(&req);
        assert_eq!(atom.line_count, 2);
        assert!(atom.content.contains("```json"));
    }

    #[test]
    fn test_auto_detect_json() {
        let req = IngestLogRequest {
            content: r#"{"ts":"2024-01-01","level":"info","msg":"ok"}"#.to_string(),
            format: LogFormat::Auto,
            source_name: "svc".to_string(),
            tag_root: None,
            tag_category: None,
        };
        let atom = prepare_log_atom(&req);
        assert!(atom.content.contains("```json"));
    }

    #[test]
    fn test_tag_path_defaults() {
        let req = IngestLogRequest {
            content: "hello".to_string(),
            format: LogFormat::PlainText,
            source_name: "host1".to_string(),
            tag_root: None,
            tag_category: None,
        };
        let atom = prepare_log_atom(&req);
        assert_eq!(atom.tag_path[0], "Logs");
        assert_eq!(atom.tag_path.last().unwrap(), "host1");
    }
}
