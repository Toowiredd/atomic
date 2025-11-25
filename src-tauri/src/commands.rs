use crate::db::Database;
use crate::models::{Atom, AtomWithTags, Tag, TagWithCount};
use chrono::Utc;
use tauri::State;
use uuid::Uuid;

// Helper function to get tags for an atom
fn get_tags_for_atom(conn: &rusqlite::Connection, atom_id: &str) -> Result<Vec<Tag>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.parent_id, t.created_at 
             FROM tags t 
             INNER JOIN atom_tags at ON t.id = at.tag_id 
             WHERE at.atom_id = ?1",
        )
        .map_err(|e| e.to_string())?;

    let tags = stmt
        .query_map([atom_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(tags)
}

// Atom operations
#[tauri::command]
pub fn get_all_atoms(db: State<Database>) -> Result<Vec<AtomWithTags>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, content, source_url, created_at, updated_at, COALESCE(embedding_status, 'pending') FROM atoms ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let atoms: Vec<Atom> = stmt
        .query_map([], |row| {
            Ok(Atom {
                id: row.get(0)?,
                content: row.get(1)?,
                source_url: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                embedding_status: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for atom in atoms {
        let tags = get_tags_for_atom(&conn, &atom.id)?;
        result.push(AtomWithTags { atom, tags });
    }

    Ok(result)
}

#[tauri::command]
pub fn get_atom(db: State<Database>, id: String) -> Result<AtomWithTags, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let atom: Atom = conn
        .query_row(
            "SELECT id, content, source_url, created_at, updated_at, COALESCE(embedding_status, 'pending') FROM atoms WHERE id = ?1",
            [&id],
            |row| {
                Ok(Atom {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    source_url: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    embedding_status: row.get(5)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let tags = get_tags_for_atom(&conn, &atom.id)?;
    Ok(AtomWithTags { atom, tags })
}

#[tauri::command]
pub fn create_atom(
    db: State<Database>,
    content: String,
    source_url: Option<String>,
    tag_ids: Vec<String>,
) -> Result<AtomWithTags, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let embedding_status = "pending";

    conn.execute(
        "INSERT INTO atoms (id, content, source_url, created_at, updated_at, embedding_status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (&id, &content, &source_url, &now, &now, &embedding_status),
    )
    .map_err(|e| e.to_string())?;

    // Add tags
    for tag_id in &tag_ids {
        conn.execute(
            "INSERT INTO atom_tags (atom_id, tag_id) VALUES (?1, ?2)",
            (&id, tag_id),
        )
        .map_err(|e| e.to_string())?;
    }

    let atom = Atom {
        id: id.clone(),
        content,
        source_url,
        created_at: now.clone(),
        updated_at: now,
        embedding_status: embedding_status.to_string(),
    };

    let tags = get_tags_for_atom(&conn, &id)?;
    Ok(AtomWithTags { atom, tags })
}

#[tauri::command]
pub fn update_atom(
    db: State<Database>,
    id: String,
    content: String,
    source_url: Option<String>,
    tag_ids: Vec<String>,
) -> Result<AtomWithTags, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let now = Utc::now().to_rfc3339();
    let embedding_status = "pending"; // Reset to pending when content changes

    conn.execute(
        "UPDATE atoms SET content = ?1, source_url = ?2, updated_at = ?3, embedding_status = ?4 WHERE id = ?5",
        (&content, &source_url, &now, &embedding_status, &id),
    )
    .map_err(|e| e.to_string())?;

    // Remove existing tags and add new ones
    conn.execute("DELETE FROM atom_tags WHERE atom_id = ?1", [&id])
        .map_err(|e| e.to_string())?;

    for tag_id in &tag_ids {
        conn.execute(
            "INSERT INTO atom_tags (atom_id, tag_id) VALUES (?1, ?2)",
            (&id, tag_id),
        )
        .map_err(|e| e.to_string())?;
    }

    // Get the updated atom
    let atom: Atom = conn
        .query_row(
            "SELECT id, content, source_url, created_at, updated_at, COALESCE(embedding_status, 'pending') FROM atoms WHERE id = ?1",
            [&id],
            |row| {
                Ok(Atom {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    source_url: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    embedding_status: row.get(5)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let tags = get_tags_for_atom(&conn, &id)?;
    Ok(AtomWithTags { atom, tags })
}

#[tauri::command]
pub fn delete_atom(db: State<Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM atoms WHERE id = ?1", [&id])
        .map_err(|e| e.to_string())?;

    Ok(())
}

// Tag operations
#[tauri::command]
pub fn get_all_tags(db: State<Database>) -> Result<Vec<TagWithCount>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Get all tags with their atom counts
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.parent_id, t.created_at, 
                    (SELECT COUNT(*) FROM atom_tags WHERE tag_id = t.id) as atom_count
             FROM tags t
             ORDER BY t.name",
        )
        .map_err(|e| e.to_string())?;

    let tags: Vec<(Tag, i32)> = stmt
        .query_map([], |row| {
            Ok((
                Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    created_at: row.get(3)?,
                },
                row.get(4)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    // Build hierarchical structure
    fn build_tree(tags: &[(Tag, i32)], parent_id: Option<&str>) -> Vec<TagWithCount> {
        tags.iter()
            .filter(|(tag, _)| tag.parent_id.as_deref() == parent_id)
            .map(|(tag, count)| TagWithCount {
                tag: tag.clone(),
                atom_count: *count,
                children: build_tree(tags, Some(&tag.id)),
            })
            .collect()
    }

    Ok(build_tree(&tags, None))
}

#[tauri::command]
pub fn create_tag(
    db: State<Database>,
    name: String,
    parent_id: Option<String>,
) -> Result<Tag, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO tags (id, name, parent_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        (&id, &name, &parent_id, &now),
    )
    .map_err(|e| e.to_string())?;

    Ok(Tag {
        id,
        name,
        parent_id,
        created_at: now,
    })
}

#[tauri::command]
pub fn update_tag(
    db: State<Database>,
    id: String,
    name: String,
    parent_id: Option<String>,
) -> Result<Tag, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE tags SET name = ?1, parent_id = ?2 WHERE id = ?3",
        (&name, &parent_id, &id),
    )
    .map_err(|e| e.to_string())?;

    // Get the updated tag
    let tag: Tag = conn
        .query_row(
            "SELECT id, name, parent_id, created_at FROM tags WHERE id = ?1",
            [&id],
            |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    parent_id: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(tag)
}

#[tauri::command]
pub fn delete_tag(db: State<Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM tags WHERE id = ?1", [&id])
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_atoms_by_tag(db: State<Database>, tag_id: String) -> Result<Vec<AtomWithTags>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.content, a.source_url, a.created_at, a.updated_at, COALESCE(a.embedding_status, 'pending')
             FROM atoms a
             INNER JOIN atom_tags at ON a.id = at.atom_id
             WHERE at.tag_id = ?1
             ORDER BY a.updated_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let atoms: Vec<Atom> = stmt
        .query_map([&tag_id], |row| {
            Ok(Atom {
                id: row.get(0)?,
                content: row.get(1)?,
                source_url: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                embedding_status: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for atom in atoms {
        let tags = get_tags_for_atom(&conn, &atom.id)?;
        result.push(AtomWithTags { atom, tags });
    }

    Ok(result)
}

// sqlite-vec verification command
#[tauri::command]
pub fn check_sqlite_vec(db: State<Database>) -> Result<String, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let version: String = conn
        .query_row("SELECT vec_version()", [], |row| row.get(0))
        .map_err(|e| format!("sqlite-vec not loaded: {}", e))?;

    Ok(version)
}

