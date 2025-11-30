use crate::db::Database;
use crate::models::{
    ChatCitation, ChatMessage, ChatMessageWithContext, ChatToolCall, Conversation,
    ConversationWithMessages, ConversationWithTags, Tag,
};
use rusqlite::Connection;
use tauri::State;

// ==================== Helper Functions ====================

/// Get tags for a conversation
fn get_conversation_tags(conn: &Connection, conversation_id: &str) -> Result<Vec<Tag>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.parent_id, t.created_at
             FROM tags t
             JOIN conversation_tags ct ON ct.tag_id = t.id
             WHERE ct.conversation_id = ?1
             ORDER BY t.name",
        )
        .map_err(|e| format!("Failed to prepare tags query: {}", e))?;

    let tags = stmt
        .query_map([conversation_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| format!("Failed to query tags: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect tags: {}", e))?;

    Ok(tags)
}

/// Get message count and last message preview for a conversation
fn get_conversation_summary(
    conn: &Connection,
    conversation_id: &str,
) -> Result<(i32, Option<String>), String> {
    let message_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM chat_messages WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to count messages: {}", e))?;

    let last_message_preview: Option<String> = conn
        .query_row(
            "SELECT content FROM chat_messages
             WHERE conversation_id = ?1
             ORDER BY message_index DESC
             LIMIT 1",
            [conversation_id],
            |row| {
                let content: String = row.get(0)?;
                // Truncate to first 100 chars
                Ok(if content.len() > 100 {
                    format!("{}...", &content[..100])
                } else {
                    content
                })
            },
        )
        .ok(); // Returns None if no messages

    Ok((message_count, last_message_preview))
}

/// Get tool calls for a message
fn get_message_tool_calls(conn: &Connection, message_id: &str) -> Result<Vec<ChatToolCall>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, message_id, tool_name, tool_input, tool_output, status, created_at, completed_at
             FROM chat_tool_calls
             WHERE message_id = ?1
             ORDER BY created_at",
        )
        .map_err(|e| format!("Failed to prepare tool calls query: {}", e))?;

    let tool_calls = stmt
        .query_map([message_id], |row| {
            let tool_input_str: String = row.get(3)?;
            let tool_output_str: Option<String> = row.get(4)?;

            Ok(ChatToolCall {
                id: row.get(0)?,
                message_id: row.get(1)?,
                tool_name: row.get(2)?,
                tool_input: serde_json::from_str(&tool_input_str).unwrap_or(serde_json::Value::Null),
                tool_output: tool_output_str
                    .map(|s| serde_json::from_str(&s).unwrap_or(serde_json::Value::Null)),
                status: row.get(5)?,
                created_at: row.get(6)?,
                completed_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("Failed to query tool calls: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect tool calls: {}", e))?;

    Ok(tool_calls)
}

/// Get citations for a message
fn get_message_citations(conn: &Connection, message_id: &str) -> Result<Vec<ChatCitation>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, message_id, citation_index, atom_id, chunk_index, excerpt, relevance_score
             FROM chat_citations
             WHERE message_id = ?1
             ORDER BY citation_index",
        )
        .map_err(|e| format!("Failed to prepare citations query: {}", e))?;

    let citations = stmt
        .query_map([message_id], |row| {
            Ok(ChatCitation {
                id: row.get(0)?,
                message_id: row.get(1)?,
                citation_index: row.get(2)?,
                atom_id: row.get(3)?,
                chunk_index: row.get(4)?,
                excerpt: row.get(5)?,
                relevance_score: row.get(6)?,
            })
        })
        .map_err(|e| format!("Failed to query citations: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect citations: {}", e))?;

    Ok(citations)
}

/// Get messages with context for a conversation
fn get_messages_with_context(
    conn: &Connection,
    conversation_id: &str,
) -> Result<Vec<ChatMessageWithContext>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, conversation_id, role, content, created_at, message_index
             FROM chat_messages
             WHERE conversation_id = ?1
             ORDER BY message_index",
        )
        .map_err(|e| format!("Failed to prepare messages query: {}", e))?;

    let messages: Vec<ChatMessage> = stmt
        .query_map([conversation_id], |row| {
            Ok(ChatMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                message_index: row.get(5)?,
            })
        })
        .map_err(|e| format!("Failed to query messages: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect messages: {}", e))?;

    // Get tool calls and citations for each message
    let mut messages_with_context = Vec::new();
    for message in messages {
        let tool_calls = get_message_tool_calls(conn, &message.id)?;
        let citations = get_message_citations(conn, &message.id)?;
        messages_with_context.push(ChatMessageWithContext {
            message,
            tool_calls,
            citations,
        });
    }

    Ok(messages_with_context)
}

// ==================== Tauri Commands ====================

/// Create a new conversation
#[tauri::command]
pub fn create_conversation(
    db: State<Database>,
    tag_ids: Vec<String>,
    title: Option<String>,
) -> Result<ConversationWithTags, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();

    // Insert conversation
    conn.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at, is_archived)
         VALUES (?1, ?2, ?3, ?4, 0)",
        rusqlite::params![&id, &title, &now, &now],
    )
    .map_err(|e| format!("Failed to create conversation: {}", e))?;

    // Insert tag associations
    for tag_id in &tag_ids {
        conn.execute(
            "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![&id, tag_id],
        )
        .map_err(|e| format!("Failed to add tag to conversation: {}", e))?;
    }

    // Get the created conversation with tags
    let tags = get_conversation_tags(&conn, &id)?;

    Ok(ConversationWithTags {
        conversation: Conversation {
            id,
            title,
            created_at: now.clone(),
            updated_at: now,
            is_archived: false,
        },
        tags,
        message_count: 0,
        last_message_preview: None,
    })
}

/// Get all conversations, optionally filtered by tag
#[tauri::command]
pub fn get_conversations(
    db: State<Database>,
    filter_tag_id: Option<String>,
    limit: i32,
    offset: i32,
) -> Result<Vec<ConversationWithTags>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let conversations: Vec<Conversation> = if let Some(tag_id) = filter_tag_id {
        // Filter by tag
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT c.id, c.title, c.created_at, c.updated_at, c.is_archived
                 FROM conversations c
                 JOIN conversation_tags ct ON ct.conversation_id = c.id
                 WHERE ct.tag_id = ?1 AND c.is_archived = 0
                 ORDER BY c.updated_at DESC
                 LIMIT ?2 OFFSET ?3",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let results: Vec<Conversation> = stmt
            .query_map(rusqlite::params![&tag_id, limit, offset], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    is_archived: row.get::<_, i32>(4)? != 0,
                })
            })
            .map_err(|e| format!("Failed to query: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect: {}", e))?;
        results
    } else {
        // Get all conversations
        let mut stmt = conn
            .prepare(
                "SELECT id, title, created_at, updated_at, is_archived
                 FROM conversations
                 WHERE is_archived = 0
                 ORDER BY updated_at DESC
                 LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let results: Vec<Conversation> = stmt
            .query_map(rusqlite::params![limit, offset], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    is_archived: row.get::<_, i32>(4)? != 0,
                })
            })
            .map_err(|e| format!("Failed to query: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Failed to collect: {}", e))?;
        results
    };

    // Enrich with tags and summaries
    let mut result = Vec::new();
    for conversation in conversations {
        let tags = get_conversation_tags(&conn, &conversation.id)?;
        let (message_count, last_message_preview) =
            get_conversation_summary(&conn, &conversation.id)?;

        result.push(ConversationWithTags {
            conversation,
            tags,
            message_count,
            last_message_preview,
        });
    }

    Ok(result)
}

/// Get a single conversation with all messages
#[tauri::command]
pub fn get_conversation(
    db: State<Database>,
    conversation_id: String,
) -> Result<Option<ConversationWithMessages>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Get conversation
    let conversation: Option<Conversation> = conn
        .query_row(
            "SELECT id, title, created_at, updated_at, is_archived
             FROM conversations
             WHERE id = ?1",
            [&conversation_id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    is_archived: row.get::<_, i32>(4)? != 0,
                })
            },
        )
        .ok();

    match conversation {
        Some(conv) => {
            let tags = get_conversation_tags(&conn, &conv.id)?;
            let messages = get_messages_with_context(&conn, &conv.id)?;

            Ok(Some(ConversationWithMessages {
                conversation: conv,
                tags,
                messages,
            }))
        }
        None => Ok(None),
    }
}

/// Update a conversation (title, archive status)
#[tauri::command]
pub fn update_conversation(
    db: State<Database>,
    id: String,
    title: Option<String>,
    is_archived: Option<bool>,
) -> Result<Conversation, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    // Update only provided fields
    if let Some(t) = &title {
        conn.execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![t, &now, &id],
        )
        .map_err(|e| format!("Failed to update title: {}", e))?;
    }

    if let Some(archived) = is_archived {
        conn.execute(
            "UPDATE conversations SET is_archived = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![if archived { 1 } else { 0 }, &now, &id],
        )
        .map_err(|e| format!("Failed to update archive status: {}", e))?;
    }

    // Return updated conversation
    conn.query_row(
        "SELECT id, title, created_at, updated_at, is_archived
         FROM conversations
         WHERE id = ?1",
        [&id],
        |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
                is_archived: row.get::<_, i32>(4)? != 0,
            })
        },
    )
    .map_err(|e| format!("Conversation not found: {}", e))
}

/// Delete a conversation
#[tauri::command]
pub fn delete_conversation(db: State<Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM conversations WHERE id = ?1", [&id])
        .map_err(|e| format!("Failed to delete conversation: {}", e))?;

    Ok(())
}

// ==================== Scope Management ====================

/// Set the full scope (replace all tags)
#[tauri::command]
pub fn set_conversation_scope(
    db: State<Database>,
    conversation_id: String,
    tag_ids: Vec<String>,
) -> Result<ConversationWithTags, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    // Remove existing tags
    conn.execute(
        "DELETE FROM conversation_tags WHERE conversation_id = ?1",
        [&conversation_id],
    )
    .map_err(|e| format!("Failed to clear tags: {}", e))?;

    // Add new tags
    for tag_id in &tag_ids {
        conn.execute(
            "INSERT INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![&conversation_id, tag_id],
        )
        .map_err(|e| format!("Failed to add tag: {}", e))?;
    }

    // Update conversation timestamp
    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![&now, &conversation_id],
    )
    .map_err(|e| format!("Failed to update timestamp: {}", e))?;

    // Get conversation with updated tags
    let conversation = conn
        .query_row(
            "SELECT id, title, created_at, updated_at, is_archived
             FROM conversations WHERE id = ?1",
            [&conversation_id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    is_archived: row.get::<_, i32>(4)? != 0,
                })
            },
        )
        .map_err(|e| format!("Conversation not found: {}", e))?;

    let tags = get_conversation_tags(&conn, &conversation_id)?;
    let (message_count, last_message_preview) =
        get_conversation_summary(&conn, &conversation_id)?;

    Ok(ConversationWithTags {
        conversation,
        tags,
        message_count,
        last_message_preview,
    })
}

/// Add a single tag to scope
#[tauri::command]
pub fn add_tag_to_scope(
    db: State<Database>,
    conversation_id: String,
    tag_id: String,
) -> Result<ConversationWithTags, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    // Insert tag (ignore if already exists)
    conn.execute(
        "INSERT OR IGNORE INTO conversation_tags (conversation_id, tag_id) VALUES (?1, ?2)",
        rusqlite::params![&conversation_id, &tag_id],
    )
    .map_err(|e| format!("Failed to add tag: {}", e))?;

    // Update conversation timestamp
    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![&now, &conversation_id],
    )
    .map_err(|e| format!("Failed to update timestamp: {}", e))?;

    // Get conversation with updated tags
    let conversation = conn
        .query_row(
            "SELECT id, title, created_at, updated_at, is_archived
             FROM conversations WHERE id = ?1",
            [&conversation_id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    is_archived: row.get::<_, i32>(4)? != 0,
                })
            },
        )
        .map_err(|e| format!("Conversation not found: {}", e))?;

    let tags = get_conversation_tags(&conn, &conversation_id)?;
    let (message_count, last_message_preview) =
        get_conversation_summary(&conn, &conversation_id)?;

    Ok(ConversationWithTags {
        conversation,
        tags,
        message_count,
        last_message_preview,
    })
}

/// Remove a single tag from scope
#[tauri::command]
pub fn remove_tag_from_scope(
    db: State<Database>,
    conversation_id: String,
    tag_id: String,
) -> Result<ConversationWithTags, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    // Remove tag
    conn.execute(
        "DELETE FROM conversation_tags WHERE conversation_id = ?1 AND tag_id = ?2",
        rusqlite::params![&conversation_id, &tag_id],
    )
    .map_err(|e| format!("Failed to remove tag: {}", e))?;

    // Update conversation timestamp
    conn.execute(
        "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
        rusqlite::params![&now, &conversation_id],
    )
    .map_err(|e| format!("Failed to update timestamp: {}", e))?;

    // Get conversation with updated tags
    let conversation = conn
        .query_row(
            "SELECT id, title, created_at, updated_at, is_archived
             FROM conversations WHERE id = ?1",
            [&conversation_id],
            |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                    is_archived: row.get::<_, i32>(4)? != 0,
                })
            },
        )
        .map_err(|e| format!("Conversation not found: {}", e))?;

    let tags = get_conversation_tags(&conn, &conversation_id)?;
    let (message_count, last_message_preview) =
        get_conversation_summary(&conn, &conversation_id)?;

    Ok(ConversationWithTags {
        conversation,
        tags,
        message_count,
        last_message_preview,
    })
}
