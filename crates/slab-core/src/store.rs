use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostState {
    pub slab_id: String,
    pub path: String,
    pub title: String,
    pub remote_updated_at: Option<String>,
    pub remote_content_hash: String,
    pub local_content_hash: String,
    pub version: Option<i64>,
}

pub struct StateDb {
    conn: Connection,
}

impl StateDb {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS posts (
                slab_id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                title TEXT NOT NULL,
                remote_updated_at TEXT,
                remote_content_hash TEXT NOT NULL,
                local_content_hash TEXT NOT NULL,
                version INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_posts_path ON posts(path);",
        )?;
        Ok(Self { conn })
    }

    pub fn upsert(&self, state: &PostState) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO posts
             (slab_id, path, title, remote_updated_at, remote_content_hash, local_content_hash, version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                state.slab_id,
                state.path,
                state.title,
                state.remote_updated_at,
                state.remote_content_hash,
                state.local_content_hash,
                state.version,
            ],
        )?;
        Ok(())
    }

    pub fn get_by_id(&self, slab_id: &str) -> anyhow::Result<Option<PostState>> {
        let mut stmt = self.conn.prepare(
            "SELECT slab_id, path, title, remote_updated_at, remote_content_hash, local_content_hash, version
             FROM posts WHERE slab_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![slab_id], |row| {
            Ok(PostState {
                slab_id: row.get(0)?,
                path: row.get(1)?,
                title: row.get(2)?,
                remote_updated_at: row.get(3)?,
                remote_content_hash: row.get(4)?,
                local_content_hash: row.get(5)?,
                version: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(Ok(state)) => Ok(Some(state)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn get_by_path(&self, path: &str) -> anyhow::Result<Option<PostState>> {
        let mut stmt = self.conn.prepare(
            "SELECT slab_id, path, title, remote_updated_at, remote_content_hash, local_content_hash, version
             FROM posts WHERE path = ?1",
        )?;
        let mut rows = stmt.query_map(params![path], |row| {
            Ok(PostState {
                slab_id: row.get(0)?,
                path: row.get(1)?,
                title: row.get(2)?,
                remote_updated_at: row.get(3)?,
                remote_content_hash: row.get(4)?,
                local_content_hash: row.get(5)?,
                version: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(Ok(state)) => Ok(Some(state)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn list_all(&self) -> anyhow::Result<Vec<PostState>> {
        let mut stmt = self.conn.prepare(
            "SELECT slab_id, path, title, remote_updated_at, remote_content_hash, local_content_hash, version
             FROM posts ORDER BY path",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PostState {
                slab_id: row.get(0)?,
                path: row.get(1)?,
                title: row.get(2)?,
                remote_updated_at: row.get(3)?,
                remote_content_hash: row.get(4)?,
                local_content_hash: row.get(5)?,
                version: row.get(6)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn delete_by_id(&self, slab_id: &str) -> anyhow::Result<()> {
        self.conn
            .execute("DELETE FROM posts WHERE slab_id = ?1", params![slab_id])?;
        Ok(())
    }
}
