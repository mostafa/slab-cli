use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not authenticated — run `slab auth login` first")]
    NotAuthenticated,

    #[error("vault not initialized — run `slab vault init` first")]
    VaultNotInitialized,

    #[error("post not found: {0}")]
    PostNotFound(String),

    #[error("topic not found: {0}")]
    TopicNotFound(String),

    #[error("conflict: remote post changed since last pull (use --force to overwrite)")]
    Conflict,

    #[error("API error: {0}")]
    Api(String),

    #[error("GraphQL errors: {0}")]
    GraphQL(String),

    #[error("delta conversion error: {0}")]
    DeltaConversion(String),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
