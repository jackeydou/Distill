//! Reads behind the web UI's pages: one note with everything its page shows, one topic with
//! its timeline, every topic, and the vault files behind topics and annotations.

use std::collections::HashSet;

use rusqlite::OptionalExtension;
use serde::Serialize;

use super::Index;
use super::query::{NoteView, TopicCount, note_view, resolve_merge, topic_counts};
use crate::error::Result;
use crate::sources::{Reopen, reopen};

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct NoteDetail {
    pub note: NoteView,
    /// The note body as written in the file, starting with `# <title>`.
    pub markdown: String,
    /// Absolute path of the note file, for opening it in an editor.
    pub file: String,
    pub topic_label: String,
    /// Notes in the note's topic, this one included.
    pub ask_count: usize,
    pub reopen: Reopen,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TopicRef {
    pub topic_id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TopicDetail {
    /// The topic after following `merged_into`; differs from the requested id when that
    /// topic was merged away.
    pub topic_id: String,
    pub label: String,
    pub created: String,
    pub ask_count: usize,
    /// Distinct agent sessions the question was distilled in.
    pub sessions: usize,
    /// Topics merged into this one.
    pub merged_from: Vec<TopicRef>,
    /// Oldest first, so the page reads as a timeline.
    pub notes: Vec<NoteView>,
}

impl Index {
    pub fn note_detail(&self, id: &str) -> Result<Option<NoteDetail>> {
        let Some(note) = self.note(id)? else {
            return Ok(None);
        };
        let markdown: String =
            self.conn
                .query_row("SELECT body FROM note_current WHERE id = ?1", [id], |r| {
                    r.get(0)
                })?;
        let label = self.topic_labels()?.remove(&note.topic_id);
        Ok(Some(NoteDetail {
            markdown,
            file: self.vault_root.join(&note.path).display().to_string(),
            topic_label: label.unwrap_or_default(),
            ask_count: self.ask_count(&note.topic_id)?,
            reopen: reopen(&note.source),
            note,
        }))
    }

    pub fn topic_detail(&self, id: &str) -> Result<Option<TopicDetail>> {
        if !self.topic_exists(id)? {
            return Ok(None);
        }
        let merges = self.merges()?;
        let topic_id = resolve_merge(id, &merges);
        let (label, created): (String, String) = self.conn.query_row(
            "SELECT label, created FROM topic_current WHERE id = ?1",
            [&topic_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let labels = self.topic_labels()?;
        let mut merged_from: Vec<TopicRef> = merges
            .keys()
            .filter(|from| resolve_merge(from, &merges) == topic_id)
            .map(|from| TopicRef {
                label: labels.get(from).cloned().unwrap_or_default(),
                topic_id: from.clone(),
            })
            .collect();
        merged_from.sort_by(|a, b| a.topic_id.cmp(&b.topic_id));
        let annotations = self.annotations_by_note()?;
        let mut notes: Vec<NoteView> = self
            .rows()?
            .iter()
            .filter(|r| r.topic_id == topic_id)
            .map(|r| note_view(r, &annotations))
            .collect();
        notes.sort_by(|a, b| a.created.cmp(&b.created));
        let sessions: HashSet<&str> = notes.iter().map(|n| n.source.session_id.as_str()).collect();
        Ok(Some(TopicDetail {
            label,
            created,
            ask_count: notes.len(),
            sessions: sessions.len(),
            merged_from,
            notes,
            topic_id,
        }))
    }

    /// Every topic that has notes, most asked first.
    pub fn topics(&self) -> Result<Vec<TopicCount>> {
        Ok(topic_counts(
            &self.rows()?,
            &self.topic_labels()?,
            &self.annotations_by_note()?,
        ))
    }

    /// Vault-relative path of the file that defines topic `id`.
    pub fn topic_file(&self, id: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT path FROM topic_current WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?)
    }

    /// Vault-relative path of annotation `id` and the note it belongs to.
    pub fn annotation_file(&self, id: &str) -> Result<Option<(String, String)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT path, note_id FROM annotation_current WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }
}
