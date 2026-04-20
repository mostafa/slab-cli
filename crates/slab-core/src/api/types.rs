use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: String,
    pub title: String,
    pub content: Option<serde_json::Value>,
    pub inserted_at: Option<String>,
    pub updated_at: Option<String>,
    pub version: Option<i64>,
    pub topics: Option<Vec<TopicRef>>,
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
