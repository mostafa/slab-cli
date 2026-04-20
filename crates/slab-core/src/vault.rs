use std::path::PathBuf;

use sha2::{Digest, Sha256};
use tracing::info;

use crate::api::types::Post;
use crate::config::Config;
use crate::delta::delta_to_markdown;
use crate::store::{PostState, StateDb};

pub struct Vault {
    pub config: Config,
    pub db: StateDb,
}

#[derive(Debug, Clone)]
pub enum FileStatus {
    /// Local matches remote (clean).
    Clean,
    /// Local file modified vs what was pulled.
    Modified,
    /// New local file not yet pushed.
    Added,
    /// Remote post has no local file (deleted locally).
    Deleted,
    /// Both local and remote changed since last sync.
    Conflict,
}

impl std::fmt::Display for FileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileStatus::Clean => write!(f, " "),
            FileStatus::Modified => write!(f, "M"),
            FileStatus::Added => write!(f, "A"),
            FileStatus::Deleted => write!(f, "D"),
            FileStatus::Conflict => write!(f, "C"),
        }
    }
}

impl Vault {
    pub fn open(config: Config) -> anyhow::Result<Self> {
        let db_path = config.state_db_path();
        std::fs::create_dir_all(config.slab_dir())?;
        let db = StateDb::open(&db_path)?;
        Ok(Self { config, db })
    }

    /// Initialize a new vault directory.
    pub fn init(config: &Config) -> anyhow::Result<()> {
        let slab_dir = config.slab_dir();
        std::fs::create_dir_all(&slab_dir)?;
        config.save()?;
        info!(vault = %config.vault_path.display(), "vault initialized");
        Ok(())
    }

    /// Write a post to the vault filesystem and update state db.
    pub fn write_post(&self, post: &Post) -> anyhow::Result<PathBuf> {
        let md_content = match &post.content {
            Some(content) => delta_to_markdown(content),
            None => String::new(),
        };

        let topic_name = post
            .topics
            .as_ref()
            .and_then(|t| t.first())
            .map(|t| sanitize_filename(&t.name))
            .unwrap_or_else(|| "Uncategorized".to_string());

        let filename = sanitize_filename(&post.title);
        let rel_dir = PathBuf::from("Topics").join(&topic_name);
        let rel_path = rel_dir.join(format!("{filename}.md"));

        let abs_dir = self.config.vault_path.join(&rel_dir);
        std::fs::create_dir_all(&abs_dir)?;

        let topics_list: Vec<String> = post
            .topics
            .as_ref()
            .map(|ts| ts.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default();

        let frontmatter = Frontmatter {
            slab_id: &post.id,
            title: &post.title,
            topics: &topics_list,
            version: post.version,
            updated_at: post.updated_at.as_deref(),
        };
        let full_content = format!("{}{md_content}", frontmatter.to_yaml());

        let abs_path = self.config.vault_path.join(&rel_path);
        std::fs::write(&abs_path, &full_content)?;

        let content_hash = hash_content(&full_content);
        let state = PostState {
            slab_id: post.id.clone(),
            path: rel_path.to_string_lossy().to_string(),
            title: post.title.clone(),
            remote_updated_at: post.updated_at.clone(),
            remote_content_hash: content_hash.clone(),
            local_content_hash: content_hash,
            version: post.version,
        };
        self.db.upsert(&state)?;

        Ok(rel_path)
    }

    /// Compute status for all tracked posts.
    pub fn status(&self) -> anyhow::Result<Vec<(String, FileStatus)>> {
        let all = self.db.list_all()?;
        let mut results = Vec::new();

        for state in &all {
            let abs_path = self.config.vault_path.join(&state.path);
            if !abs_path.exists() {
                results.push((state.path.clone(), FileStatus::Deleted));
                continue;
            }
            let content = std::fs::read_to_string(&abs_path)?;
            let current_hash = hash_content(&content);
            if current_hash == state.local_content_hash {
                results.push((state.path.clone(), FileStatus::Clean));
            } else {
                results.push((state.path.clone(), FileStatus::Modified));
            }
        }

        // Check for untracked files in Inbox/
        let inbox_dir = self.config.vault_path.join("Inbox");
        if inbox_dir.exists() {
            for entry in std::fs::read_dir(&inbox_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "md") {
                    let rel = path
                        .strip_prefix(&self.config.vault_path)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    if self.db.get_by_path(&rel)?.is_none() {
                        results.push((rel, FileStatus::Added));
                    }
                }
            }
        }

        results.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(results)
    }

    /// Read a post file and strip frontmatter, returning the markdown body.
    pub fn read_post_body(&self, rel_path: &str) -> anyhow::Result<String> {
        let abs_path = self.config.vault_path.join(rel_path);
        let content = std::fs::read_to_string(&abs_path)?;
        Ok(strip_frontmatter(&content))
    }

    pub fn list_tracked_posts(&self) -> anyhow::Result<Vec<PostState>> {
        self.db.list_all()
    }
}

struct Frontmatter<'a> {
    slab_id: &'a str,
    title: &'a str,
    topics: &'a [String],
    version: Option<i64>,
    updated_at: Option<&'a str>,
}

impl<'a> Frontmatter<'a> {
    fn to_yaml(&self) -> String {
        let mut yaml = String::from("---\n");
        yaml.push_str(&format!("slab_id: \"{}\"\n", self.slab_id));
        yaml.push_str(&format!("title: \"{}\"\n", self.title));
        if !self.topics.is_empty() {
            yaml.push_str("topics:\n");
            for t in self.topics {
                yaml.push_str(&format!("  - \"{t}\"\n"));
            }
        }
        if let Some(v) = self.version {
            yaml.push_str(&format!("version: {v}\n"));
        }
        if let Some(u) = self.updated_at {
            yaml.push_str(&format!("updated_at: \"{u}\"\n"));
        }
        yaml.push_str("---\n");
        yaml
    }
}

pub fn strip_frontmatter(content: &str) -> String {
    if content.starts_with("---\n")
        && let Some(end) = content[4..].find("---\n")
    {
        return content[4 + end + 4..].to_string();
    }
    content.to_string()
}

pub fn parse_frontmatter_slab_id(content: &str) -> Option<String> {
    if !content.starts_with("---\n") {
        return None;
    }
    let end = content[4..].find("---\n")?;
    let fm = &content[4..4 + end];
    for line in fm.lines() {
        if let Some(rest) = line.strip_prefix("slab_id:") {
            let id = rest.trim().trim_matches('"');
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02x}")).collect()
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
