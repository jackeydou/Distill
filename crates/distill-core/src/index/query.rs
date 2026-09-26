use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, params_from_iter};
use serde::Serialize;

use super::Index;
use super::recall::{Doc, rank};
use crate::error::Result;
use crate::model::{Agent, Source};
use crate::tags::resolve_alias;

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TagCount {
    pub tag: String,
    pub notes: usize,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct AnnotationView {
    pub id: String,
    pub body: String,
    pub created: String,
    pub updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct NoteView {
    pub id: String,
    pub topic_id: String,
    pub title: String,
    pub question: String,
    pub conclusion: String,
    pub tags: Vec<String>,
    pub created: String,
    pub source: Source,
    pub path: String,
    pub annotations: Vec<AnnotationView>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct NoteHit {
    pub id: String,
    pub topic_id: String,
    pub title: String,
    pub conclusion: String,
    pub tags: Vec<String>,
    pub created: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct RecallTopic {
    pub topic_id: String,
    pub label: String,
    /// Notes in this topic, i.e. how many times the question was distilled.
    pub ask_count: usize,
    pub notes: Vec<NoteView>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct RecallResult {
    pub topics: Vec<RecallTopic>,
    /// Every tag in use, so the agent can reuse one instead of inventing a near-duplicate.
    pub tags: Vec<TagCount>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TopicCount {
    pub topic_id: String,
    pub label: String,
    pub ask_count: usize,
    pub first_asked: String,
    pub last_asked: String,
    pub annotated: bool,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct WeekCount {
    pub week: String,
    pub notes: usize,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ProjectCount {
    pub project: String,
    pub notes: usize,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Stats {
    pub notes: usize,
    pub topics: usize,
    pub repeated_topics: usize,
    pub annotations: usize,
    pub top_topics: Vec<TopicCount>,
    pub tags: Vec<TagCount>,
    pub by_week: Vec<WeekCount>,
    pub by_project: Vec<ProjectCount>,
    pub conflicts: usize,
    pub invalid_files: usize,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Conflict {
    pub kind: String,
    pub id: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct InvalidFile {
    pub path: String,
    pub error: String,
}

/// One row of `note_current`, with topic and tags already resolved through merges and
/// aliases.
#[derive(Debug, Clone)]
pub(super) struct Row {
    pub id: String,
    pub path: String,
    pub topic_id: String,
    pub title: String,
    pub question: String,
    pub conclusion: String,
    pub created: String,
    pub created_utc: String,
    pub source: Source,
    pub tags: Vec<String>,
}

const TOP_TOPICS: usize = 20;
const RECALL_CANDIDATES: usize = 30;

impl Index {
    pub fn search(&self, query: &str, tag: Option<&str>, limit: usize) -> Result<Vec<NoteHit>> {
        let rows = self.rows()?;
        let ids = search_ids(&self.conn, query)?;
        let by_id: HashMap<&str, &Row> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
        let aliases = self.aliases()?;
        let tag = tag.map(|t| resolve_alias(t, &aliases));
        Ok(ids
            .iter()
            .filter_map(|id| by_id.get(id.as_str()))
            .filter(|r| tag.as_ref().is_none_or(|t| r.tags.contains(t)))
            .take(limit)
            .map(|r| NoteHit {
                id: r.id.clone(),
                topic_id: r.topic_id.clone(),
                title: r.title.clone(),
                conclusion: r.conclusion.clone(),
                tags: r.tags.clone(),
                created: r.created.clone(),
                path: r.path.clone(),
            })
            .collect())
    }

    /// Topics whose notes resemble `question`, best match first, each with all of its
    /// notes and their annotations.
    pub fn recall(&self, question: &str, limit: usize) -> Result<RecallResult> {
        let rows = self.rows()?;
        let by_id: HashMap<&str, &Row> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
        let bodies = self.bodies()?;
        let docs: Vec<Doc> = rows
            .iter()
            .map(|r| Doc {
                id: &r.id,
                head: format!("{}\n{}", r.title, r.question),
                body: bodies.get(&r.id).map_or("", String::as_str),
            })
            .collect();
        let ids = rank(question, &docs);
        let labels = self.topic_labels()?;
        let annotations = self.annotations_by_note()?;
        let mut topics: Vec<String> = Vec::new();
        for id in ids.iter().take(RECALL_CANDIDATES) {
            if let Some(row) = by_id.get(id.as_str())
                && !topics.contains(&row.topic_id)
            {
                topics.push(row.topic_id.clone());
            }
        }
        topics.truncate(limit);
        let topics = topics
            .into_iter()
            .map(|topic_id| {
                let mut notes: Vec<NoteView> = rows
                    .iter()
                    .filter(|r| r.topic_id == topic_id)
                    .map(|r| note_view(r, &annotations))
                    .collect();
                notes.sort_by(|a, b| a.created.cmp(&b.created));
                RecallTopic {
                    label: labels.get(&topic_id).cloned().unwrap_or_default(),
                    ask_count: notes.len(),
                    topic_id,
                    notes,
                }
            })
            .collect();
        Ok(RecallResult {
            topics,
            tags: tag_counts(&rows),
        })
    }

    pub fn tags(&self) -> Result<Vec<TagCount>> {
        Ok(tag_counts(&self.rows()?))
    }

    /// Tags a new note may use without going through `new_tags`: every tag on a note and
    /// every alias target.
    pub fn known_tags(&self) -> Result<Vec<String>> {
        let mut tags: HashSet<String> = self.rows()?.into_iter().flat_map(|r| r.tags).collect();
        tags.extend(self.aliases()?.into_values());
        let mut tags: Vec<String> = tags.into_iter().collect();
        tags.sort();
        Ok(tags)
    }

    pub fn aliases(&self) -> Result<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT from_tag, to_tag FROM tag_alias ORDER BY created, path")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn topic_exists(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT 1 FROM topic_current WHERE id = ?1",
                [id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Follows `merged_into` to the topic a note effectively belongs to.
    pub fn resolve_topic(&self, id: &str) -> Result<String> {
        Ok(resolve_merge(id, &self.merges()?))
    }

    pub fn ask_count(&self, topic_id: &str) -> Result<usize> {
        let topic_id = self.resolve_topic(topic_id)?;
        Ok(self
            .rows()?
            .iter()
            .filter(|r| r.topic_id == topic_id)
            .count())
    }

    pub fn note(&self, id: &str) -> Result<Option<NoteView>> {
        let annotations = self.annotations_by_note()?;
        Ok(self
            .rows()?
            .iter()
            .find(|r| r.id == id)
            .map(|r| note_view(r, &annotations)))
    }

    pub fn stats(&self) -> Result<Stats> {
        let rows = self.rows()?;
        let annotations = self.annotations_by_note()?;
        let mut top = topic_counts(&rows, &self.topic_labels()?, &annotations);
        let repeated_topics = top.iter().filter(|t| t.ask_count >= 2).count();
        let topics = top.len();
        top.truncate(TOP_TOPICS);

        let mut weeks: BTreeMap<String, usize> = BTreeMap::new();
        let mut projects: HashMap<String, usize> = HashMap::new();
        for row in &rows {
            if let Some(week) = iso_week(&row.created) {
                *weeks.entry(week).or_default() += 1;
            }
            let project = row
                .source
                .git_repo
                .clone()
                .unwrap_or(row.source.cwd.clone());
            *projects.entry(project).or_default() += 1;
        }
        let mut by_project: Vec<ProjectCount> = projects
            .into_iter()
            .map(|(project, notes)| ProjectCount { project, notes })
            .collect();
        by_project.sort_by(|a, b| b.notes.cmp(&a.notes).then(a.project.cmp(&b.project)));

        Ok(Stats {
            notes: rows.len(),
            topics,
            repeated_topics,
            annotations: annotations.values().map(Vec::len).sum(),
            top_topics: top,
            tags: tag_counts(&rows),
            by_week: weeks
                .into_iter()
                .map(|(week, notes)| WeekCount { week, notes })
                .collect(),
            by_project,
            conflicts: self.conflicts()?.len(),
            invalid_files: self.invalid_files()?.len(),
        })
    }

    /// Frontmatter ids that appear in more than one file: copies a sync tool left behind
    /// after two devices edited the same file.
    pub fn conflicts(&self) -> Result<Vec<Conflict>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, id, group_concat(path, char(10) ORDER BY length(path), path) FROM (
                 SELECT 'note' AS kind, id, path FROM note
                 UNION ALL SELECT 'topic', id, path FROM topic
                 UNION ALL SELECT 'annotation', id, path FROM annotation
             ) GROUP BY kind, id HAVING count(*) > 1 ORDER BY kind, id",
        )?;
        let rows = stmt.query_map([], |r| {
            let paths: String = r.get(2)?;
            Ok(Conflict {
                kind: r.get(0)?,
                id: r.get(1)?,
                paths: paths.split('\n').map(str::to_string).collect(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn invalid_files(&self) -> Result<Vec<InvalidFile>> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, error FROM file WHERE error IS NOT NULL ORDER BY path")?;
        let rows = stmt.query_map([], |r| {
            Ok(InvalidFile {
                path: r.get(0)?,
                error: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub(super) fn merges(&self) -> Result<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, merged_into FROM topic_current WHERE merged_into IS NOT NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub(super) fn topic_labels(&self) -> Result<HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT id, label FROM topic_current")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub(super) fn bodies(&self) -> Result<HashMap<String, String>> {
        let mut stmt = self.conn.prepare("SELECT id, body FROM note_current")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub(super) fn annotations_by_note(&self) -> Result<HashMap<String, Vec<AnnotationView>>> {
        let mut stmt = self.conn.prepare(
            "SELECT note_id, id, body, created, updated FROM annotation_current ORDER BY created",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                AnnotationView {
                    id: r.get(1)?,
                    body: r.get(2)?,
                    created: r.get(3)?,
                    updated: r.get(4)?,
                },
            ))
        })?;
        let mut out: HashMap<String, Vec<AnnotationView>> = HashMap::new();
        for row in rows {
            let (note_id, view) = row?;
            out.entry(note_id).or_default().push(view);
        }
        Ok(out)
    }

    pub(super) fn rows(&self) -> Result<Vec<Row>> {
        let merges = self.merges()?;
        let aliases = self.aliases()?;
        let mut tags: HashMap<String, Vec<String>> = HashMap::new();
        {
            let mut stmt = self.conn.prepare("SELECT note_path, tag FROM note_tag")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (path, tag) = row?;
                let tag = resolve_alias(&tag, &aliases);
                let entry = tags.entry(path).or_default();
                if !entry.contains(&tag) {
                    entry.push(tag);
                }
            }
        }
        let mut stmt = self.conn.prepare(
            "SELECT id, path, topic_id, title, question, conclusion, created, created_utc,
                    agent, session_id, cwd, git_repo
             FROM note_current ORDER BY created_utc DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            let agent: String = r.get(8)?;
            // Rows only reach the index after Source::validate, so this fails only on a
            // corrupted index; `distill reindex` fixes that.
            let agent = Agent::parse(&agent).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    e.to_string().into(),
                )
            })?;
            Ok(Row {
                id: r.get(0)?,
                path: r.get(1)?,
                topic_id: r.get(2)?,
                title: r.get(3)?,
                question: r.get(4)?,
                conclusion: r.get(5)?,
                created: r.get(6)?,
                created_utc: r.get(7)?,
                source: Source {
                    agent,
                    session_id: r.get(9)?,
                    cwd: r.get(10)?,
                    git_repo: r.get(11)?,
                },
                tags: Vec::new(),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            let mut row = row?;
            row.topic_id = resolve_merge(&row.topic_id, &merges);
            row.tags = tags.remove(&row.path).unwrap_or_default();
            out.push(row);
        }
        Ok(out)
    }
}

pub(super) fn note_view(row: &Row, annotations: &HashMap<String, Vec<AnnotationView>>) -> NoteView {
    NoteView {
        id: row.id.clone(),
        topic_id: row.topic_id.clone(),
        title: row.title.clone(),
        question: row.question.clone(),
        conclusion: row.conclusion.clone(),
        tags: row.tags.clone(),
        created: row.created.clone(),
        source: row.source.clone(),
        path: row.path.clone(),
        annotations: annotations.get(&row.id).cloned().unwrap_or_default(),
    }
}

/// One entry per topic that has notes, most asked first, then most recently asked.
pub(super) fn topic_counts(
    rows: &[Row],
    labels: &HashMap<String, String>,
    annotations: &HashMap<String, Vec<AnnotationView>>,
) -> Vec<TopicCount> {
    let mut by_topic: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for row in rows {
        by_topic.entry(&row.topic_id).or_default().push(row);
    }
    let mut out: Vec<TopicCount> = by_topic
        .iter()
        .map(|(topic_id, notes)| TopicCount {
            topic_id: topic_id.to_string(),
            label: labels.get(*topic_id).cloned().unwrap_or_default(),
            ask_count: notes.len(),
            first_asked: min_created(notes, |a, b| a < b),
            last_asked: min_created(notes, |a, b| a > b),
            annotated: notes.iter().any(|n| annotations.contains_key(&n.id)),
        })
        .collect();
    out.sort_by(|a, b| {
        b.ask_count
            .cmp(&a.ask_count)
            .then(b.last_asked.cmp(&a.last_asked))
    });
    out
}

pub(super) fn tag_counts(rows: &[Row]) -> Vec<TagCount> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for tag in rows.iter().flat_map(|r| &r.tags) {
        *counts.entry(tag).or_default() += 1;
    }
    let mut out: Vec<TagCount> = counts
        .into_iter()
        .map(|(tag, notes)| TagCount {
            tag: tag.to_string(),
            notes,
        })
        .collect();
    out.sort_by(|a, b| b.notes.cmp(&a.notes).then(a.tag.cmp(&b.tag)));
    out
}

fn min_created(notes: &[&Row], better: impl Fn(&str, &str) -> bool) -> String {
    let mut best: Option<&Row> = None;
    for n in notes {
        if best.is_none_or(|b| better(&n.created_utc, &b.created_utc)) {
            best = Some(n);
        }
    }
    best.map(|n| n.created.clone()).unwrap_or_default()
}

pub(super) fn resolve_merge(id: &str, merges: &HashMap<String, String>) -> String {
    let mut current = id.to_string();
    let mut seen = HashSet::new();
    while let Some(next) = merges.get(&current) {
        if !seen.insert(current.clone()) {
            break;
        }
        current = next.clone();
    }
    current
}

/// ISO week of the note's local creation date, e.g. `2026-W37`.
fn iso_week(created: &str) -> Option<String> {
    let dt: jiff::civil::DateTime = created.get(..19)?.parse().ok()?;
    let week = dt.date().iso_week_date();
    Some(format!("{}-W{:02}", week.year(), week.week()))
}

/// Note ids matching every whitespace-separated piece of `query`, best first. Pieces too
/// short for the trigram index fall back to `LIKE`. An empty query lists every note.
fn search_ids(conn: &Connection, query: &str) -> Result<Vec<String>> {
    let pieces: Vec<&str> = query.split_whitespace().collect();
    let (long, short): (Vec<&str>, Vec<&str>) =
        pieces.into_iter().partition(|p| p.chars().count() >= 3);
    if long.is_empty() {
        return like_ids(conn, &short);
    }
    let fts = long
        .iter()
        .map(|p| format!("\"{}\"", p.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ");
    fts_ids(conn, &fts, &short)
}

fn fts_ids(conn: &Connection, fts: &str, also_like: &[&str]) -> Result<Vec<String>> {
    let mut sql = String::from(
        "SELECT n.id FROM note_fts JOIN note_current n ON n.path = note_fts.path
         WHERE note_fts MATCH ?1",
    );
    let mut args: Vec<String> = vec![fts.to_string()];
    for piece in also_like {
        sql.push_str(&like_clause(args.len() + 1));
        args.push(like_pattern(piece));
    }
    sql.push_str(" ORDER BY bm25(note_fts, 0.0, 3.0, 2.0, 1.0)");
    collect_ids(conn, &sql, args)
}

fn like_ids(conn: &Connection, pieces: &[&str]) -> Result<Vec<String>> {
    let mut sql = String::from("SELECT n.id FROM note_current n WHERE 1 = 1");
    let mut args: Vec<String> = Vec::new();
    for piece in pieces.iter().filter(|p| !p.is_empty()) {
        sql.push_str(&like_clause(args.len() + 1));
        args.push(like_pattern(piece));
    }
    sql.push_str(" ORDER BY n.created_utc DESC");
    collect_ids(conn, &sql, args)
}

fn like_clause(n: usize) -> String {
    format!(" AND (n.title LIKE ?{n} ESCAPE '\\' OR n.body LIKE ?{n} ESCAPE '\\')")
}

fn like_pattern(piece: &str) -> String {
    let escaped = piece
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn collect_ids(conn: &Connection, sql: &str, args: Vec<String>) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params_from_iter(args), |r| r.get::<_, String>(0))?;
    let mut ids: Vec<String> = Vec::new();
    for id in rows {
        let id = id?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}
