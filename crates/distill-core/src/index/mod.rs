//! The per-device SQLite index. It is derived from the vault and safe to delete.
//!
//! Several processes open it at once (one `distill mcp` per agent session, the CLI, the
//! web server), so it runs in WAL mode with a busy timeout, and every sync writes inside
//! one immediate transaction.

mod browse;
mod query;
mod recall;
mod scan;
mod semantic;
mod vec;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

use crate::error::{IoContext, Result};

pub use browse::{NoteDetail, TopicDetail, TopicRef};
pub use query::{
    AnnotationView, Conflict, InvalidFile, NoteHit, NoteView, ProjectCount, RecallResult,
    RecallTopic, Stats, TagCount, TopicCount, WeekCount,
};
pub use scan::SyncReport;
pub use semantic::{DUPLICATE_MIN, SimilarTopic, TopicPair};

static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(include_str!("../../migrations/0001_init.sql")),
        M::up(include_str!("../../migrations/0002_embeddings.sql")),
    ])
});

pub struct Index {
    conn: Connection,
    vault_root: PathBuf,
}

impl Index {
    pub fn open(db_path: &Path, vault_root: &Path) -> Result<Self> {
        if let Some(dir) = db_path.parent() {
            std::fs::create_dir_all(dir).at(dir)?;
        }
        vec::register()?;
        let mut conn = Connection::open(db_path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        MIGRATIONS.to_latest(&mut conn)?;
        Ok(Self {
            conn,
            vault_root: vault_root.to_path_buf(),
        })
    }

    /// Brings the index up to date with the vault: new and changed files are parsed,
    /// deleted files are dropped. Unchanged files cost one `stat`.
    pub fn sync(&mut self) -> Result<SyncReport> {
        scan::sync(&mut self.conn, &self.vault_root, false)
    }

    /// Drops every row and re-reads the whole vault.
    pub fn rebuild(&mut self) -> Result<SyncReport> {
        scan::sync(&mut self.conn, &self.vault_root, true)
    }
}

#[cfg(test)]
mod tests;
