use anyhow::Context;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::Config;
use crate::api::types::*;

/// Post fields selected in every query (web endpoint schema).
const POST_FIELDS: &str = "id title insertedAt editedAt \
     content { delta markdown text version } topics { id name }";

/// Raw post shape returned by the team web endpoint.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebPost {
    id: String,
    title: String,
    content: Option<WebContent>,
    inserted_at: Option<String>,
    edited_at: Option<String>,
    topics: Option<Vec<TopicRef>>,
}

#[derive(Deserialize)]
struct WebContent {
    delta: Option<serde_json::Value>,
    markdown: Option<String>,
    text: Option<String>,
    version: Option<i64>,
}

impl From<WebPost> for Post {
    fn from(p: WebPost) -> Self {
        let (delta, markdown, text, version) = match p.content {
            Some(c) => (c.delta, c.markdown, c.text, c.version),
            None => (None, None, None, None),
        };
        Post {
            id: p.id,
            title: p.title,
            content: delta,
            markdown,
            text,
            inserted_at: p.inserted_at,
            updated_at: p.edited_at,
            version,
            topics: p.topics,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebTopic {
    id: String,
    name: String,
    description: Option<serde_json::Value>,
    parent: Option<IdRef>,
}

#[derive(Deserialize)]
struct IdRef {
    id: String,
}

impl From<WebTopic> for Topic {
    fn from(t: WebTopic) -> Self {
        Topic {
            id: t.id,
            name: t.name,
            description: t.description.as_ref().map(json_to_text),
            parent_topic_id: t.parent.map(|p| p.id),
            posts: None,
        }
    }
}

#[derive(Deserialize)]
struct Edges<N> {
    edges: Vec<Edge<N>>,
    #[serde(rename = "pageInfo")]
    page_info: PageInfo,
}

#[derive(Deserialize)]
struct Edge<N> {
    node: N,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    end_cursor: Option<String>,
    has_next_page: bool,
}

/// Flatten a Json value (Quill Delta or plain string) into readable text.
/// Json scalars often arrive as strings containing serialized Delta ops.
fn json_to_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => match serde_json::from_str::<serde_json::Value>(s) {
            Ok(inner) if inner.is_array() || inner.is_object() => json_to_text(&inner),
            _ => s.clone(),
        },
        serde_json::Value::Array(_) => {
            let wrapped = serde_json::json!({ "ops": v });
            crate::delta::delta_to_markdown(&wrapped).trim().to_string()
        }
        serde_json::Value::Object(_) => crate::delta::delta_to_markdown(v).trim().to_string(),
        _ => String::new(),
    }
}

#[derive(Debug, Clone)]
pub struct SlabClient {
    http: reqwest::Client,
    endpoint: String,
    team: String,
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
            team: config.team.clone(),
        })
    }

    /// True when talking to the public API (`api.slab.com`), which has a
    /// slightly different schema than the team-specific web endpoint.
    fn is_public_api(&self) -> bool {
        self.endpoint.contains("api.slab.com")
    }

    /// Execute a raw GraphQL query and return the full response body
    /// (including any errors) as JSON.
    pub async fn raw_query(
        &self,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> anyhow::Result<serde_json::Value> {
        let body = GqlRequest { query, variables };
        let resp = self
            .http
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
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
            post: WebPost,
        }
        let q = format!(r#"query GetPost($id: ID!) {{ post(id: $id) {{ {POST_FIELDS} }} }}"#);
        let vars = serde_json::json!({ "id": id });
        let resp: Resp = self.query(&q, Some(vars)).await?;
        Ok(resp.post.into())
    }

    /// Fetch the full topic tree. `listTopics` only returns one level
    /// (roots when `parentId` is omitted), so we BFS the hierarchy,
    /// batching each level's children into a single aliased query.
    pub async fn list_topics(&self) -> anyhow::Result<Vec<Topic>> {
        let roots = self.list_topics_level(None).await?;
        let mut frontier: Vec<String> = roots.iter().map(|t| t.id.clone()).collect();
        let mut all = roots;

        while !frontier.is_empty() {
            let mut found = Vec::new();
            // Server caps query complexity at 200; ~16 per aliased lookup.
            for chunk in frontier.chunks(10) {
                found.extend(self.topics_children_batch(chunk).await?);
            }
            frontier = found.iter().map(|t| t.id.clone()).collect();
            all.extend(found);
        }
        Ok(all)
    }

    /// Fetch all topics of one level (children of `parent`, or roots).
    async fn list_topics_level(&self, parent: Option<&str>) -> anyhow::Result<Vec<Topic>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            list_topics: Edges<WebTopic>,
        }
        let q = r#"
            query ListTopics($parentId: ID, $cursor: String) {
                listTopics(first: 100, parentId: $parentId, after: $cursor) {
                    edges {
                        node { id name description parent { id } }
                    }
                    pageInfo { endCursor hasNextPage }
                }
            }
        "#;
        let mut topics = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let vars = serde_json::json!({ "parentId": parent, "cursor": cursor });
            let resp: Resp = self.query(q, Some(vars)).await?;
            topics.extend(resp.list_topics.edges.into_iter().map(|e| e.node.into()));
            if !resp.list_topics.page_info.has_next_page {
                break;
            }
            cursor = resp.list_topics.page_info.end_cursor;
        }
        Ok(topics)
    }

    /// Fetch the children of several parent topics in one aliased query.
    async fn topics_children_batch(&self, parents: &[String]) -> anyhow::Result<Vec<Topic>> {
        let mut q = String::from("query(");
        for i in 0..parents.len() {
            if i > 0 {
                q.push_str(", ");
            }
            q.push_str(&format!("$p{i}: ID"));
        }
        q.push_str(") {");
        for i in 0..parents.len() {
            q.push_str(&format!(
                " t{i}: listTopics(first: 100, parentId: $p{i}) {{ \
                   edges {{ node {{ id name description parent {{ id }} }} }} \
                   pageInfo {{ endCursor hasNextPage }} }}"
            ));
        }
        q.push('}');

        let mut vars = serde_json::Map::new();
        for (i, p) in parents.iter().enumerate() {
            vars.insert(format!("p{i}"), serde_json::Value::String(p.clone()));
        }

        let resp: std::collections::HashMap<String, Edges<WebTopic>> = self
            .query(&q, Some(serde_json::Value::Object(vars)))
            .await?;

        let mut topics = Vec::new();
        for (alias, conn) in resp {
            let truncated = conn.page_info.has_next_page;
            topics.extend(conn.edges.into_iter().map(|e| Topic::from(e.node)));
            if truncated {
                // Rare: >100 children. Re-fetch that parent with pagination.
                let idx: usize = alias.trim_start_matches('t').parse().unwrap_or(0);
                if let Some(parent) = parents.get(idx) {
                    topics.retain(|t| t.parent_topic_id.as_deref() != Some(parent.as_str()));
                    topics.extend(self.list_topics_level(Some(parent)).await?);
                }
            }
        }
        Ok(topics)
    }

    /// Fetch all posts in a topic, including its subtopics.
    /// `listPosts(topicId:)` only returns posts directly in a topic, so we
    /// walk the topic tree and collect posts from every descendant.
    pub async fn get_topic_posts(&self, topic_id: &str) -> anyhow::Result<Vec<Post>> {
        use std::collections::{HashMap, HashSet};

        let topics = self.list_topics().await?;
        let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
        for t in &topics {
            if let Some(parent) = t.parent_topic_id.as_deref() {
                children.entry(parent).or_default().push(t.id.as_str());
            }
        }

        let mut queue = vec![topic_id.to_string()];
        let mut subtree = Vec::new();
        while let Some(id) = queue.pop() {
            if let Some(kids) = children.get(id.as_str()) {
                queue.extend(kids.iter().map(|s| s.to_string()));
            }
            subtree.push(id);
        }

        let mut seen = HashSet::new();
        let mut posts = Vec::new();
        for id in &subtree {
            let mut cursor: Option<String> = None;
            loop {
                let (page, next) = self.list_posts_page(Some(id), cursor.as_deref()).await?;
                for p in page {
                    if seen.insert(p.id.clone()) {
                        posts.push(p);
                    }
                }
                match next {
                    Some(c) => cursor = Some(c),
                    None => break,
                }
            }
        }
        Ok(posts)
    }

    pub async fn list_all_posts(
        &self,
        cursor: Option<&str>,
    ) -> anyhow::Result<(Vec<Post>, Option<String>)> {
        self.list_posts_page(None, cursor).await
    }

    async fn list_posts_page(
        &self,
        topic_id: Option<&str>,
        cursor: Option<&str>,
    ) -> anyhow::Result<(Vec<Post>, Option<String>)> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            list_posts: Edges<WebPost>,
        }
        let q = format!(
            r#"query ListPosts($topicId: ID, $cursor: String) {{
                listPosts(first: 50, after: $cursor, topicId: $topicId) {{
                    edges {{ node {{ {POST_FIELDS} }} }}
                    pageInfo {{ endCursor hasNextPage }}
                }}
            }}"#
        );
        let vars = serde_json::json!({ "topicId": topic_id, "cursor": cursor });
        let resp: Resp = self.query(&q, Some(vars)).await?;
        let posts: Vec<Post> = resp
            .list_posts
            .edges
            .into_iter()
            .map(|e| e.node.into())
            .collect();
        let next = if resp.list_posts.page_info.has_next_page {
            resp.list_posts.page_info.end_cursor
        } else {
            None
        };
        Ok((posts, next))
    }

    pub async fn search_posts(
        &self,
        query_str: &str,
        cursor: Option<&str>,
    ) -> anyhow::Result<(Vec<SearchResult>, Option<String>)> {
        #[derive(Deserialize)]
        struct Resp {
            search: Edges<SearchNode>,
        }
        #[derive(Deserialize)]
        struct SearchNode {
            post: Option<WebPost>,
            highlight: Option<serde_json::Value>,
        }
        let q = format!(
            r#"query SearchPosts($query: String!, $cursor: String) {{
                search(query: $query, first: 30, after: $cursor) {{
                    edges {{
                        node {{
                            ... on PostSearchResult {{
                                highlight
                                post {{ {POST_FIELDS} }}
                            }}
                        }}
                    }}
                    pageInfo {{ endCursor hasNextPage }}
                }}
            }}"#
        );
        let vars = serde_json::json!({ "query": query_str, "cursor": cursor });
        let resp: Resp = self.query(&q, Some(vars)).await?;
        let results: Vec<SearchResult> = resp
            .search
            .edges
            .into_iter()
            .filter_map(|e| {
                let post = e.node.post?;
                Some(SearchResult {
                    post: post.into(),
                    highlight: e.node.highlight.as_ref().map(json_to_text),
                })
            })
            .collect();
        let next = if resp.search.page_info.has_next_page {
            resp.search.page_info.end_cursor
        } else {
            None
        };
        Ok((results, next))
    }

    /// Update a post's content. Fetches the current version and text first
    /// to compute the OT consistency checksum the server requires.
    pub async fn update_post_content(
        &self,
        id: &str,
        content: &serde_json::Value,
    ) -> anyhow::Result<Post> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Resp {
            update_post_content: WebPost,
        }
        let current = self.get_post(id).await?;
        let version = current.version.context("post has no content version")?;
        let checksum = current.checksum();

        let q = format!(
            r#"mutation UpdatePostContent($id: ID!, $delta: Json!, $version: Int!, $checksum: Int!) {{
                updatePostContent(id: $id, delta: $delta, version: $version, checksum: $checksum) {{
                    {POST_FIELDS}
                }}
            }}"#
        );
        let vars = serde_json::json!({
            "id": id,
            "delta": content,
            "version": version,
            "checksum": checksum,
        });
        let resp: Resp = self.query(&q, Some(vars)).await?;
        Ok(resp.update_post_content.into())
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
            create_post: WebPost,
        }
        let q = format!(
            r#"mutation CreatePost($title: String, $content: Json, $topicId: ID) {{
                createPost(title: $title, content: $content, topicId: $topicId) {{
                    {POST_FIELDS}
                }}
            }}"#
        );
        let vars = serde_json::json!({
            "title": title,
            "content": content,
            "topicId": topic_id,
        });
        let resp: Resp = self.query(&q, Some(vars)).await?;
        Ok(resp.create_post.into())
    }

    /// Verify the configured token by querying the current session.
    pub async fn verify_auth(&self) -> anyhow::Result<()> {
        let _: serde_json::Value = self
            .query("query VerifyAuth { currentUser { hasPassword } }", None)
            .await
            .context("token rejected by Slab — check SLAB_API_TOKEN")?;
        Ok(())
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

    pub async fn update_comment(&self, comment_id: &str, content: &str) -> anyhow::Result<Comment> {
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

    pub async fn react_to_comment(&self, comment_id: &str, emoji: &str) -> anyhow::Result<String> {
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
        if self.is_public_api() {
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
        } else {
            // The team web endpoint requires the org host as an argument.
            let q = r#"
                query GetOrganization($host: String!) {
                    organization(host: $host) {
                        id
                        host
                    }
                }
            "#;
            let vars = serde_json::json!({ "host": format!("{}.slab.com", self.team) });
            let resp: Resp = self.query(q, Some(vars)).await?;
            Ok(resp.organization)
        }
    }
}
