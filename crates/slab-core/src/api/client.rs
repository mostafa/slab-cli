use anyhow::Context;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::Config;
use crate::api::types::*;

#[derive(Debug, Clone)]
pub struct SlabClient {
    http: reqwest::Client,
    endpoint: String,
}

#[derive(Serialize)]
struct GqlRequest<'a> {
    query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct GqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GqlError>>,
}

#[derive(Debug, Deserialize)]
struct GqlError {
    message: String,
}

impl SlabClient {
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let mut headers = HeaderMap::new();
        let auth_value = format!("Bearer {}", config.token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value).context("invalid token characters")?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            http,
            endpoint: config.endpoint.clone(),
        })
    }

    async fn query<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> anyhow::Result<T> {
        let body = GqlRequest { query, variables };
        debug!(query, "executing GraphQL query");

        let resp = self
            .http
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let gql: GqlResponse<T> = resp.json().await?;
        if let Some(errors) = gql.errors {
            let msgs: Vec<_> = errors.iter().map(|e| e.message.as_str()).collect();
            anyhow::bail!("GraphQL errors: {}", msgs.join("; "));
        }
        gql.data.context("no data in GraphQL response")
    }

    pub async fn get_post(&self, id: &str) -> anyhow::Result<Post> {
        #[derive(Deserialize)]
        struct Resp {
            post: Post,
        }
        let q = r#"
            query GetPost($id: ID!) {
                post(id: $id) {
                    id
                    title
                    content
                    insertedAt
                    updatedAt
                    version
                    topics { id name }
                }
            }
        "#;
        let vars = serde_json::json!({ "id": id });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.post)
    }

    pub async fn list_topics(&self) -> anyhow::Result<Vec<Topic>> {
        #[derive(Deserialize)]
        struct Resp {
            topics: Vec<Topic>,
        }
        let q = r#"
            query ListTopics {
                topics {
                    id
                    name
                    description
                    parentTopicId
                }
            }
        "#;
        let resp: Resp = self.query(q, None).await?;
        Ok(resp.topics)
    }

    pub async fn get_topic_posts(&self, topic_id: &str) -> anyhow::Result<Vec<Post>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            topic: TopicWithPosts,
        }
        #[derive(Deserialize)]
        struct TopicWithPosts {
            posts: Vec<Post>,
        }
        let q = r#"
            query GetTopicPosts($id: ID!) {
                topic(id: $id) {
                    posts {
                        id
                        title
                        content
                        insertedAt
                        updatedAt
                        version
                        topics { id name }
                    }
                }
            }
        "#;
        let vars = serde_json::json!({ "id": topic_id });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.topic.posts)
    }

    pub async fn search_posts(
        &self,
        query_str: &str,
        cursor: Option<&str>,
    ) -> anyhow::Result<(Vec<SearchResult>, Option<String>)> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            search: SearchConnection,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SearchConnection {
            results: Vec<SearchResult>,
            next_cursor: Option<String>,
        }
        let q = r#"
            query SearchPosts($query: String!, $cursor: String) {
                search(query: $query, after: $cursor) {
                    results {
                        post {
                            id
                            title
                            content
                            insertedAt
                            updatedAt
                            version
                            topics { id name }
                        }
                        highlight
                    }
                    nextCursor
                }
            }
        "#;
        let vars = serde_json::json!({
            "query": query_str,
            "cursor": cursor,
        });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok((resp.search.results, resp.search.next_cursor))
    }

    pub async fn list_all_posts(
        &self,
        cursor: Option<&str>,
    ) -> anyhow::Result<(Vec<Post>, Option<String>)> {
        #[derive(Deserialize)]
        struct Resp {
            organization: OrgPosts,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct OrgPosts {
            posts: Vec<Post>,
            next_cursor: Option<String>,
        }
        let q = r#"
            query ListAllPosts($cursor: String) {
                organization {
                    posts(after: $cursor) {
                        id
                        title
                        content
                        insertedAt
                        updatedAt
                        version
                        topics { id name }
                    }
                    nextCursor
                }
            }
        "#;
        let vars = serde_json::json!({ "cursor": cursor });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok((resp.organization.posts, resp.organization.next_cursor))
    }

    pub async fn update_post_content(
        &self,
        id: &str,
        content: &serde_json::Value,
    ) -> anyhow::Result<Post> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            update_post_content: Post,
        }
        let q = r#"
            mutation UpdatePostContent($id: ID!, $content: JSON!) {
                updatePostContent(postId: $id, content: $content) {
                    id
                    title
                    content
                    insertedAt
                    updatedAt
                    version
                    topics { id name }
                }
            }
        "#;
        let vars = serde_json::json!({
            "id": id,
            "content": content,
        });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.update_post_content)
    }

    pub async fn create_post(
        &self,
        title: &str,
        content: &serde_json::Value,
        topic_id: Option<&str>,
    ) -> anyhow::Result<Post> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            sync_post: Post,
        }
        let q = r#"
            mutation SyncPost($title: String!, $content: JSON!, $externalId: ID!, $topicId: ID) {
                syncPost(
                    title: $title,
                    content: $content,
                    externalId: $externalId,
                    topicId: $topicId
                ) {
                    id
                    title
                    content
                    insertedAt
                    updatedAt
                    version
                    topics { id name }
                }
            }
        "#;
        let external_id = format!("slab-cli-{}", uuid_v4_simple());
        let vars = serde_json::json!({
            "title": title,
            "content": content,
            "externalId": external_id,
            "topicId": topic_id,
        });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.sync_post)
    }

    pub async fn get_post_threads(&self, post_id: &str) -> anyhow::Result<PostThreads> {
        #[derive(Deserialize)]
        struct Resp {
            post: PostThreads,
        }
        let q = r#"
            query PostCommentThreads($postId: ID!) {
                post(id: $postId) {
                    id
                    threads {
                        id
                        ... on CommentThread {
                            comments {
                                id
                                content
                                author { id name avatarUrl }
                                insertedAt
                                updatedAt
                            }
                            resolvedAt
                        }
                    }
                }
            }
        "#;
        let vars = serde_json::json!({ "postId": post_id });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.post)
    }

    pub async fn create_comment(
        &self,
        post_id: &str,
        thread_id: &str,
        content: &str,
        version: i64,
        checksum: i64,
        mark: &serde_json::Value,
    ) -> anyhow::Result<CreatedComment> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            create_comment: CreatedComment,
        }
        let q = r#"
            mutation CreateComment(
                $threadId: ID!,
                $id: ID!,
                $version: Int!,
                $checksum: Int!,
                $mark: Json!,
                $content: String!,
                $notifyGroups: Boolean = false
            ) {
                createComment(
                    threadId: $threadId,
                    postId: $id,
                    version: $version,
                    checksum: $checksum,
                    mark: $mark,
                    content: $content,
                    notifyGroups: $notifyGroups
                ) { id }
            }
        "#;
        let vars = serde_json::json!({
            "threadId": thread_id,
            "id": post_id,
            "version": version,
            "checksum": checksum,
            "mark": mark,
            "content": content,
            "notifyGroups": true,
        });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.create_comment)
    }

    pub async fn update_comment(
        &self,
        comment_id: &str,
        content: &str,
    ) -> anyhow::Result<Comment> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            update_comment: Comment,
        }
        let q = r#"
            mutation UpdateComment($id: ID!, $content: String!) {
                updateComment(commentId: $id, content: $content) {
                    id
                    content
                    updatedAt
                }
            }
        "#;
        let vars = serde_json::json!({
            "id": comment_id,
            "content": content,
        });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.update_comment)
    }

    pub async fn react_to_comment(
        &self,
        comment_id: &str,
        emoji: &str,
    ) -> anyhow::Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Reaction {
            id: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            comment_reaction: Reaction,
        }
        let q = r#"
            mutation CreateCommentReaction($commentId: ID!, $emoji: String!) {
                commentReaction: createCommentReaction(commentId: $commentId, emoji: $emoji) {
                    id
                }
            }
        "#;
        let vars = serde_json::json!({
            "commentId": comment_id,
            "emoji": emoji,
        });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.comment_reaction.id)
    }

    pub async fn delete_comment(&self, comment_id: &str) -> anyhow::Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            delete_comment: CreatedComment,
        }
        let q = r#"
            mutation DeleteComment($id: ID!) {
                deleteComment(commentId: $id) { id }
            }
        "#;
        let vars = serde_json::json!({ "id": comment_id });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.delete_comment.id)
    }

    pub async fn delete_thread(&self, thread_id: &str) -> anyhow::Result<String> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            delete_thread: CreatedComment,
        }
        let q = r#"
            mutation DeleteThread($id: ID!) {
                deleteThread(threadId: $id) { id }
            }
        "#;
        let vars = serde_json::json!({ "id": thread_id });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.delete_thread.id)
    }

    pub async fn resolve_thread(&self, thread_id: &str) -> anyhow::Result<ResolvedThread> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            resolve_thread: ResolvedThread,
        }
        let q = r#"
            mutation ResolveThread($id: ID!) {
                resolveThread(threadId: $id) {
                    id
                    resolvedAt
                }
            }
        "#;
        let vars = serde_json::json!({ "id": thread_id });
        let resp: Resp = self.query(q, Some(vars)).await?;
        Ok(resp.resolve_thread)
    }

    pub async fn get_organization(&self) -> anyhow::Result<Organization> {
        #[derive(Deserialize)]
        struct Resp {
            organization: Organization,
        }
        let q = r#"
            query GetOrganization {
                organization {
                    id
                    host
                }
            }
        "#;
        let resp: Resp = self.query(q, None).await?;
        Ok(resp.organization)
    }
}

fn uuid_v4_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos:032x}")
}
