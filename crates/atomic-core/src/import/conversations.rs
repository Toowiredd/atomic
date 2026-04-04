//! Conversation history import adapters.
//!
//! Supports importing conversation histories from:
//! - ChatGPT export (`conversations.json`)
//! - Claude export (JSON array with `chat_messages`)
//! - Generic Markdown files (headers like `## User` / `## Assistant`)

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A conversation parsed from an external source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedConversation {
    /// Conversation title (may be empty).
    pub title: Option<String>,
    /// ISO-8601 creation timestamp, if available.
    pub created_at: Option<String>,
    /// Ordered list of messages.
    pub messages: Vec<ImportedMessage>,
}

/// A single message in an imported conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedMessage {
    /// Normalised role: "user", "assistant", or "system".
    pub role: String,
    /// Plain-text content.
    pub content: String,
    /// ISO-8601 timestamp, if available.
    pub created_at: Option<String>,
}

// ==================== ChatGPT Export ====================

/// Parse the array-of-conversations found in ChatGPT's `conversations.json`.
///
/// Each element looks like:
/// ```json
/// { "title": "...", "create_time": 1234567890.0,
///   "mapping": { "nodeId": { "message": {...}, "parent": "...", "children": [...] } } }
/// ```
pub fn parse_chatgpt_export(json_str: &str) -> Result<Vec<ImportedConversation>, String> {
    let root: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Invalid JSON: {e}"))?;

    let conversations = root
        .as_array()
        .ok_or("Expected a JSON array at the top level")?;

    let mut result = Vec::with_capacity(conversations.len());
    for item in conversations {
        if let Some(conv) = parse_chatgpt_conversation(item) {
            if !conv.messages.is_empty() {
                result.push(conv);
            }
        }
    }
    Ok(result)
}

fn parse_chatgpt_conversation(item: &Value) -> Option<ImportedConversation> {
    let title = item["title"].as_str().map(String::from);

    let created_at = item["create_time"]
        .as_f64()
        .and_then(unix_timestamp_to_rfc3339);

    let mapping = item["mapping"].as_object()?;

    // Find the root node (parent is null or absent).
    let root_id = mapping
        .iter()
        .find(|(_, v)| v["parent"].is_null() || v["parent"].as_str().map(|s| s.is_empty()).unwrap_or(false))
        .map(|(k, _)| k.clone())?;

    // Walk the tree in order: root → first child → first grandchild → …
    // ChatGPT conversations are generally linear (one child per node), but we
    // fall back to DFS if there are multiple children (takes the first branch).
    let mut messages = Vec::new();
    let mut current = root_id;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current.clone()) {
            break; // cycle guard
        }
        let node = match mapping.get(&current) {
            Some(n) => n,
            None => break,
        };

        // Try to extract a message from this node.
        if let Some(msg) = extract_chatgpt_message(node) {
            messages.push(msg);
        }

        // Move to the first child.
        let children = node["children"].as_array();
        match children.and_then(|c| c.first()).and_then(|v| v.as_str()) {
            Some(child_id) => current = child_id.to_string(),
            None => break,
        }
    }

    Some(ImportedConversation { title, created_at, messages })
}

fn extract_chatgpt_message(node: &Value) -> Option<ImportedMessage> {
    let msg = &node["message"];
    if msg.is_null() {
        return None;
    }

    let role = match msg["author"]["role"].as_str()? {
        "user" => "user",
        "assistant" => "assistant",
        "system" => "system",
        "tool" => "assistant", // collapse tool responses to assistant
        _ => return None,
    };

    let content = &msg["content"];
    let text = match content["content_type"].as_str().unwrap_or("") {
        "text" => {
            let parts = content["parts"].as_array()?;
            parts
                .iter()
                .filter_map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        }
        "code" => {
            let code_text = content["text"].as_str().unwrap_or("");
            let lang = content["language"].as_str().unwrap_or("");
            format!("```{lang}\n{code_text}\n```")
        }
        _ => return None,
    };

    if text.trim().is_empty() {
        return None;
    }

    let created_at = msg["create_time"]
        .as_f64()
        .and_then(unix_timestamp_to_rfc3339);

    Some(ImportedMessage {
        role: role.to_string(),
        content: text,
        created_at,
    })
}

// ==================== Claude Export ====================

/// Parse the array exported by Claude (`claude_conversations.json` or similar).
///
/// Each element looks like:
/// ```json
/// { "uuid": "...", "name": "...", "created_at": "...",
///   "chat_messages": [ { "uuid": "...", "text": "...", "sender": "human"|"assistant", "created_at": "..." } ] }
/// ```
pub fn parse_claude_export(json_str: &str) -> Result<Vec<ImportedConversation>, String> {
    let root: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Invalid JSON: {e}"))?;

    let conversations = root
        .as_array()
        .ok_or("Expected a JSON array at the top level")?;

    let mut result = Vec::with_capacity(conversations.len());
    for item in conversations {
        let title = item["name"].as_str().map(String::from);
        let created_at = item["created_at"].as_str().map(String::from);

        let raw_messages = match item["chat_messages"].as_array() {
            Some(m) => m,
            None => continue,
        };

        let messages: Vec<ImportedMessage> = raw_messages
            .iter()
            .filter_map(|m| {
                let role = match m["sender"].as_str()? {
                    "human" => "user",
                    "assistant" => "assistant",
                    _ => return None,
                };
                let content = m["text"].as_str()?.trim();
                if content.is_empty() {
                    return None;
                }
                Some(ImportedMessage {
                    role: role.to_string(),
                    content: content.to_string(),
                    created_at: m["created_at"].as_str().map(String::from),
                })
            })
            .collect();

        if !messages.is_empty() {
            result.push(ImportedConversation { title, created_at, messages });
        }
    }
    Ok(result)
}

// ==================== Generic Markdown ====================

/// Parse a single markdown file as a conversation.
///
/// Sections marked `## User`, `## Human`, `## You`, or `## Assistant` start
/// a new message. Everything between two such headers is the message body.
/// If no section headers are found the whole file is treated as a single user message.
pub fn parse_markdown_conversation(content: &str, filename: &str) -> ImportedConversation {
    let title = filename
        .trim_end_matches(".md")
        .trim_end_matches(".markdown")
        .to_string();

    let mut messages: Vec<ImportedMessage> = Vec::new();
    let mut current_role: Option<&str> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        let role = if trimmed.starts_with("## ") {
            let header = trimmed.trim_start_matches("## ").trim().to_lowercase();
            match header.as_str() {
                "user" | "human" | "you" | "me" => Some("user"),
                "assistant" | "ai" | "claude" | "chatgpt" | "gpt" | "bot" => Some("assistant"),
                "system" => Some("system"),
                _ => None,
            }
        } else {
            None
        };

        if let Some(r) = role {
            // Flush the previous message.
            if let Some(prev_role) = current_role {
                let text = current_lines.join("\n").trim().to_string();
                if !text.is_empty() {
                    messages.push(ImportedMessage {
                        role: prev_role.to_string(),
                        content: text,
                        created_at: None,
                    });
                }
            }
            current_role = Some(r);
            current_lines.clear();
        } else if current_role.is_some() {
            current_lines.push(line);
        }
    }

    // Flush last message.
    if let Some(role) = current_role {
        let text = current_lines.join("\n").trim().to_string();
        if !text.is_empty() {
            messages.push(ImportedMessage {
                role: role.to_string(),
                content: text,
                created_at: None,
            });
        }
    }

    // If no role headers were found, treat whole file as a user message.
    if messages.is_empty() {
        let text = content.trim().to_string();
        if !text.is_empty() {
            messages.push(ImportedMessage {
                role: "user".to_string(),
                content: text,
                created_at: None,
            });
        }
    }

    ImportedConversation {
        title: Some(title),
        created_at: None,
        messages,
    }
}

// ==================== Helpers ====================

fn unix_timestamp_to_rfc3339(ts: f64) -> Option<String> {
    use chrono::TimeZone;
    let secs = ts.trunc() as i64;
    let nanos = ((ts.fract()) * 1_000_000_000.0) as u32;
    chrono::Utc
        .timestamp_opt(secs, nanos)
        .single()
        .map(|dt| dt.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_chatgpt_basic() {
        let json = r#"[{
          "title": "Test chat",
          "create_time": 1700000000.0,
          "mapping": {
            "root": { "id": "root", "message": null, "parent": null, "children": ["msg1"] },
            "msg1": { "id": "msg1", "message": {
              "author": {"role": "user"},
              "content": {"content_type": "text", "parts": ["Hello"]},
              "create_time": 1700000001.0
            }, "parent": "root", "children": ["msg2"] },
            "msg2": { "id": "msg2", "message": {
              "author": {"role": "assistant"},
              "content": {"content_type": "text", "parts": ["Hi there!"]},
              "create_time": 1700000002.0
            }, "parent": "msg1", "children": [] }
          }
        }]"#;

        let convs = parse_chatgpt_export(json).unwrap();
        assert_eq!(convs.len(), 1);
        let conv = &convs[0];
        assert_eq!(conv.title.as_deref(), Some("Test chat"));
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[0].role, "user");
        assert_eq!(conv.messages[0].content, "Hello");
        assert_eq!(conv.messages[1].role, "assistant");
        assert_eq!(conv.messages[1].content, "Hi there!");
    }

    #[test]
    fn test_parse_claude_basic() {
        let json = r#"[{
          "uuid": "abc",
          "name": "My chat",
          "created_at": "2024-01-01T00:00:00Z",
          "chat_messages": [
            {"uuid": "m1", "sender": "human", "text": "Hello", "created_at": "2024-01-01T00:00:01Z"},
            {"uuid": "m2", "sender": "assistant", "text": "Hi!", "created_at": "2024-01-01T00:00:02Z"}
          ]
        }]"#;

        let convs = parse_claude_export(json).unwrap();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].messages[0].role, "user");
        assert_eq!(convs[0].messages[1].role, "assistant");
    }

    #[test]
    fn test_parse_markdown_conversation_with_headers() {
        let md = "## User\nHello there\n\n## Assistant\nHi! How can I help?";
        let conv = parse_markdown_conversation(md, "chat.md");
        assert_eq!(conv.title.as_deref(), Some("chat"));
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[0].role, "user");
        assert_eq!(conv.messages[1].role, "assistant");
    }

    #[test]
    fn test_parse_markdown_no_headers_treated_as_user() {
        let md = "Just a note without role headers.";
        let conv = parse_markdown_conversation(md, "note.md");
        assert_eq!(conv.messages.len(), 1);
        assert_eq!(conv.messages[0].role, "user");
    }

    #[test]
    fn test_chatgpt_empty_text_parts_skipped() {
        let json = r#"[{
          "title": "Empty",
          "create_time": 1700000000.0,
          "mapping": {
            "root": { "message": null, "parent": null, "children": ["m1"] },
            "m1": { "message": {
              "author": {"role": "user"},
              "content": {"content_type": "text", "parts": [""]},
              "create_time": null
            }, "parent": "root", "children": [] }
          }
        }]"#;
        let convs = parse_chatgpt_export(json).unwrap();
        // Empty-text message should be skipped → conv has 0 messages → filtered out
        assert_eq!(convs.len(), 0);
    }
}
