use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: String,
    pub title: String,
    /// Quill Delta ops (JSON).
    pub content: Option<serde_json::Value>,
    /// Server-rendered markdown of the post content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    /// Plain text of the post content (used for OT checksum).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub inserted_at: Option<String>,
    pub updated_at: Option<String>,
    pub version: Option<i64>,
    pub topics: Option<Vec<TopicRef>>,
}

impl Post {
    /// OT checksum as computed by Slab's web client: the UTF-16 length of
    /// the document text (JavaScript string length semantics).
    pub fn checksum(&self) -> i64 {
        self.text
            .as_deref()
            .map(|t| t.encode_utf16().count() as i64)
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Topic {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub parent_topic_id: Option<String>,
    pub posts: Option<Vec<Post>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub post: Post,
    pub highlight: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: String,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentThread {
    pub id: String,
    pub comments: Option<Vec<Comment>>,
    #[serde(default)]
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: String,
    pub content: Option<String>,
    pub author: Option<CommentAuthor>,
    pub inserted_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentAuthor {
    pub id: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostThreads {
    pub id: String,
    pub threads: Option<Vec<CommentThread>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedComment {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedThread {
    pub id: String,
    pub resolved_at: Option<String>,
}
