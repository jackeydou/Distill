//! Edits made from the web UI: rename and merge topics, change and delete annotations, and
//! settle sync conflicts. Each one rewrites or removes only the file it is about, so the
//! vault stays safe for file-sync tools (spec D2).

use std::path::{Path, PathBuf};

use crate::error::{Error, IoContext, Result};
use crate::fsutil::now_rfc3339;
use crate::model::{AnnotationMeta, TopicMeta};
use crate::ops::Distill;
use crate::redact::redact;
use crate::vault::parse_md;

impl Distill {
    pub fn rename_topic(&mut self, id: &str, label: &str) -> Result<()> {
        let label = label.trim();
        if label.is_empty() || label.contains('\n') {
            return Err(Error::InvalidTopic {
                id: id.to_string(),
                reason: "the label must be one non-empty line".into(),
            });
        }
        let path = self.topic_path(id)?;
        let (mut meta, description): (TopicMeta, String) = read_md(&path)?;
        meta.label = redact(label);
        self.vault.rewrite_md(&path, &meta, &description)?;
        self.index.sync()?;
        Ok(())
    }

    /// Merges the topic `from` belongs to into the topic `into` belongs to, by writing
    /// `merged_into` into the first topic's file. Notes are not touched. Returns the topic
    /// both now resolve to.
    pub fn merge_topic(&mut self, from: &str, into: &str) -> Result<String> {
        for id in [from, into] {
            if !self.index.topic_exists(id)? {
                return Err(Error::UnknownTopic { id: id.to_string() });
            }
        }
        let source = self.index.resolve_topic(from)?;
        let target = self.index.resolve_topic(into)?;
        if source == target {
            return Err(Error::InvalidMerge {
                from: from.to_string(),
                into: into.to_string(),
                reason: format!("both already resolve to topic {target}"),
            });
        }
        let path = self.topic_path(&source)?;
        let (mut meta, description): (TopicMeta, String) = read_md(&path)?;
        meta.merged_into = Some(target.clone());
        self.vault.rewrite_md(&path, &meta, &description)?;
        self.index.sync()?;
        Ok(target)
    }

    pub fn update_annotation(&mut self, id: &str, text: &str) -> Result<()> {
        if text.trim().is_empty() {
            return Err(Error::InvalidNote {
                reason: format!(
                    "annotation {id} would be empty; delete it instead of saving no text"
                ),
            });
        }
        let path = self.annotation_path(id)?;
        let (mut meta, _): (AnnotationMeta, String) = read_md(&path)?;
        meta.updated = Some(now_rfc3339());
        self.vault
            .rewrite_md(&path, &meta, &format!("{}\n", redact(text.trim())))?;
        self.index.sync()?;
        Ok(())
    }

    pub fn delete_annotation(&mut self, id: &str) -> Result<()> {
        let path = self.annotation_path(id)?;
        std::fs::remove_file(&path).at(&path)?;
        if let Some(dir) = path.parent()
            && std::fs::read_dir(dir).at(dir)?.next().is_none()
        {
            std::fs::remove_dir(dir).at(dir)?;
        }
        self.index.sync()?;
        Ok(())
    }

    /// Keeps `keep`, one of the files a sync tool left for the same `kind` and `id`, and
    /// moves the other copies out of the vault into this device's data directory, where
    /// they can still be recovered. Returns where the copies went.
    pub fn resolve_conflict(&mut self, kind: &str, id: &str, keep: &str) -> Result<Vec<PathBuf>> {
        let conflict = self
            .index
            .conflicts()?
            .into_iter()
            .find(|c| c.kind == kind && c.id == id)
            .ok_or_else(|| Error::UnknownConflict {
                kind: kind.to_string(),
                id: id.to_string(),
            })?;
        if !conflict.paths.iter().any(|p| p == keep) {
            return Err(Error::NotAConflictCopy {
                kind: kind.to_string(),
                id: id.to_string(),
                path: keep.to_string(),
                paths: conflict.paths,
            });
        }
        let dest_root = self
            .dirs
            .discarded_dir(self.vault.id())
            .join(now_rfc3339().replace(':', ""));
        let mut moved = Vec::new();
        for rel in conflict.paths.iter().filter(|p| *p != keep) {
            let src = self.vault.root().join(rel);
            let dest = dest_root.join(rel);
            move_file(&src, &dest)?;
            moved.push(dest);
        }
        self.index.sync()?;
        Ok(moved)
    }

    fn topic_path(&self, id: &str) -> Result<PathBuf> {
        let rel = self
            .index
            .topic_file(id)?
            .ok_or_else(|| Error::UnknownTopic { id: id.to_string() })?;
        Ok(self.vault.root().join(rel))
    }

    fn annotation_path(&self, id: &str) -> Result<PathBuf> {
        let (rel, _) = self
            .index
            .annotation_file(id)?
            .ok_or_else(|| Error::UnknownAnnotation { id: id.to_string() })?;
        Ok(self.vault.root().join(rel))
    }
}

fn read_md<T: serde::de::DeserializeOwned>(path: &Path) -> Result<(T, String)> {
    let text = std::fs::read_to_string(path).at(path)?;
    parse_md(path, &text)
}

/// Renames, or copies and removes when the destination is on another volume (a vault on
/// a sync drive, the data directory on the system disk).
fn move_file(src: &Path, dest: &Path) -> Result<()> {
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).at(dir)?;
    }
    match std::fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            std::fs::copy(src, dest).at(dest)?;
            std::fs::remove_file(src).at(src)
        }
        Err(e) => Err(Error::Io {
            path: src.to_path_buf(),
            source: e,
        }),
    }
}
