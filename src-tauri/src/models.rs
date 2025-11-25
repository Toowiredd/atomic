use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Atom {
    pub id: String,
    pub content: String,
    pub source_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub embedding_status: String, // 'pending', 'processing', 'complete', 'failed'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomWithTags {
    #[serde(flatten)]
    pub atom: Atom,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagWithCount {
    #[serde(flatten)]
    pub tag: Tag,
    pub atom_count: i32,
    pub children: Vec<TagWithCount>,
}

