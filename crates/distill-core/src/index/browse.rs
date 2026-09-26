//! Reads behind the web UI's pages: one note with everything its page shows, one topic with
//! its timeline, every topic, and the vault files behind topics and annotations.

use std::collections::{HashMap, HashSet};

use rusqlite::OptionalExtension;
use serde::Serialize;

use super::Index;
use super::query::{NoteView, TopicCount, note_view, resolve_merge, topic_counts};
use crate::error::Result;
use crate::model::Source;
use crate::sources::{Reopen, reopen};
use crate::tags::resolve_alias;

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

/// One note on the timeline: the question and the start of the answer, plus where it sits
/// in its topic.
#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TimelineItem {
    pub id: String,
    pub topic_id: String,
    pub topic_label: String,
    pub title: String,
    pub question: String,
    pub conclusion: String,
    pub tags: Vec<String>,
    pub created: String,
    pub source: Source,
    /// This note is the Nth in its topic, oldest first: "the Nth time you asked".
    pub ask_index: usize,
    /// Notes in the topic.
    pub ask_count: usize,
    pub annotations: usize,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TimelinePage {
    /// Newest first.
    pub items: Vec<TimelineItem>,
    /// Pass back as `before` for the next page; absent after the last page.
    #[ts(optional)]
    pub next: Option<String>,
}

impl Index {
    /// Notes newest first, optionally only those with `tag`, in pages of `limit`. `before`
    /// is the `next` cursor of the previous page.
    pub fn timeline(
        &self,
        tag: Option<&str>,
        before: Option<&str>,
        limit: usize,
    ) -> Result<TimelinePage> {
        let mut rows = self.rows()?;
        rows.sort_by(|a, b| {
            b.created_utc
                .cmp(&a.created_utc)
                .then_with(|| b.id.cmp(&a.id))
        });
        let mut position: HashMap<&str, usize> = HashMap::new();
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for row in rows.iter().rev() {
            let n = counts.entry(row.topic_id.as_str()).or_default();
            *n += 1;
            position.insert(row.id.as_str(), *n);
        }
        let aliases = self.aliases()?;
        let tag = tag.map(|t| resolve_alias(t, &aliases));
        let labels = self.topic_labels()?;
        let annotations = self.annotations_by_note()?;
        let mut matching = rows
            .iter()
            .filter(|r| tag.as_ref().is_none_or(|t| r.tags.contains(t)))
            .filter(|r| before.is_none_or(|b| cursor(&r.created_utc, &r.id).as_str() < b));
        let chosen: Vec<_> = matching.by_ref().take(limit).collect();
        let next = match (matching.next(), chosen.last()) {
            (Some(_), Some(last)) => Some(cursor(&last.created_utc, &last.id)),
            _ => None,
        };
        let page = chosen
            .into_iter()
            .map(|row| TimelineItem {
                id: row.id.clone(),
                topic_id: row.topic_id.clone(),
                topic_label: labels.get(&row.topic_id).cloned().unwrap_or_default(),
                title: row.title.clone(),
                question: row.question.clone(),
                conclusion: row.conclusion.clone(),
                tags: row.tags.clone(),
                created: row.created.clone(),
                source: row.source.clone(),
                ask_index: position[row.id.as_str()],
                ask_count: counts[row.topic_id.as_str()],
                annotations: annotations.get(&row.id).map_or(0, Vec::len),
            })
            .collect();
        Ok(TimelinePage { items: page, next })
    }

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

/// Sorts like the timeline: by UTC creation time, then id.
fn cursor(created_utc: &str, id: &str) -> String {
    format!("{created_utc} {id}")
}
