//! Embedding-backed reads: recall that finds reworded questions keyword matching misses,
//! and topic pairs that are probably the same question (merge suggestions, spec P4).
//!
//! Vectors live in the index next to everything else and are compared in memory: a
//! personal vault has thousands of notes, which a linear scan over 384-dimension vectors
//! handles in milliseconds. `sqlite-vec` is not used; loading an extension needs `unsafe`,
//! which this workspace forbids.

use std::collections::{HashMap, HashSet};

use rusqlite::params;
use serde::Serialize;

use super::Index;
use super::browse::TopicRef;
use super::query::{RecallResult, Row};
use crate::embed::{Embedder, MODEL_KEY, centroid, cosine, from_blob, note_text, to_blob};
use crate::error::Result;
use crate::fsutil::content_hash;

/// Below this cosine similarity a note is not a recall candidate. Measured on this model
/// (2026-09-26, 12 Chinese/English pairs): rewordings of one question scored 0.40 to 0.74,
/// different questions on the same subject up to 0.54. The ranges overlap, so recall keeps
/// a low floor and leaves the "same question?" call to the agent, as keyword recall does.
const RECALL_MIN: f32 = 0.4;
/// Topic pairs at or above this are suggested for merging. Above every different-question
/// pair measured; the user confirms each merge or dismisses the pair.
pub const DUPLICATE_MIN: f32 = 0.65;
/// How many semantic hits take part in fusion.
const SEMANTIC_CANDIDATES: usize = 30;

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct TopicPair {
    pub a: TopicRef,
    pub b: TopicRef,
    /// Cosine similarity of the two topics' mean note vectors, 0 to 1.
    pub similarity: f32,
    pub a_count: usize,
    pub b_count: usize,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct SimilarTopic {
    pub topic: TopicRef,
    pub similarity: f32,
    pub ask_count: usize,
}

impl Index {
    /// Computes vectors for notes that have none under the current model, and drops
    /// vectors no note uses any more. Returns how many notes were embedded.
    pub fn embed_missing(&self, embedder: &Embedder) -> Result<usize> {
        let rows = self.rows()?;
        let known: HashSet<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT text_hash FROM embedding WHERE model = ?1")?;
            let hashes = stmt.query_map([MODEL_KEY], |r| r.get(0))?;
            hashes.collect::<rusqlite::Result<_>>()?
        };
        let mut wanted: HashMap<String, String> = HashMap::new();
        for row in &rows {
            let text = note_text(&row.title, &row.question);
            wanted.insert(content_hash(text.as_bytes()), text);
        }
        let missing: Vec<(&String, &String)> =
            wanted.iter().filter(|(h, _)| !known.contains(*h)).collect();
        if !missing.is_empty() {
            let texts: Vec<String> = missing.iter().map(|(_, t)| (*t).clone()).collect();
            let vectors = embedder.embed(&texts)?;
            for ((hash, _), vector) in missing.iter().zip(vectors) {
                self.conn.execute(
                    "INSERT OR REPLACE INTO embedding (model, text_hash, vector) VALUES (?1, ?2, ?3)",
                    params![MODEL_KEY, hash, to_blob(&vector)],
                )?;
            }
        }
        for stale in known.iter().filter(|h| !wanted.contains_key(*h)) {
            self.conn.execute(
                "DELETE FROM embedding WHERE model = ?1 AND text_hash = ?2",
                params![MODEL_KEY, stale],
            )?;
        }
        Ok(missing.len())
    }

    /// Keyword and embedding recall fused. Embeds any notes that lack vectors first.
    pub fn recall_semantic(
        &self,
        question: &str,
        limit: usize,
        embedder: &Embedder,
    ) -> Result<RecallResult> {
        self.embed_missing(embedder)?;
        let query = embedder.embed_one(question)?;
        let vectors = self.note_vectors()?;
        let mut scored: Vec<(f32, &String)> = vectors
            .iter()
            .map(|(id, v)| (cosine(&query, v), id))
            .filter(|(s, _)| *s >= RECALL_MIN)
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
        let semantic: Vec<String> = scored
            .into_iter()
            .take(SEMANTIC_CANDIDATES)
            .map(|(_, id)| id.clone())
            .collect();
        self.recall_with(question, limit, Some(&semantic))
    }

    /// Topic pairs that are probably the same question, most similar first. Pairs the user
    /// dismissed are left out.
    pub fn duplicate_topics(&self, embedder: &Embedder) -> Result<Vec<TopicPair>> {
        self.embed_missing(embedder)?;
        let topics = self.topic_vectors()?;
        let dismissed = self.dismissed()?;
        let mut pairs = Vec::new();
        for (i, a) in topics.iter().enumerate() {
            for b in &topics[i + 1..] {
                let similarity = cosine(&a.vector, &b.vector);
                if similarity < DUPLICATE_MIN || dismissed.contains(&ordered(&a.id, &b.id)) {
                    continue;
                }
                // The more-asked topic is `b`, the natural merge target.
                let (a, b) = if a.count > b.count { (b, a) } else { (a, b) };
                pairs.push(TopicPair {
                    a: a.topic_ref(),
                    b: b.topic_ref(),
                    similarity,
                    a_count: a.count,
                    b_count: b.count,
                });
            }
        }
        pairs.sort_by(|x, y| y.similarity.total_cmp(&x.similarity));
        Ok(pairs)
    }

    /// Topics closest to `topic_id`, most similar first, at most `limit`.
    pub fn similar_topics(
        &self,
        topic_id: &str,
        limit: usize,
        embedder: &Embedder,
    ) -> Result<Vec<SimilarTopic>> {
        self.embed_missing(embedder)?;
        let topic_id = self.resolve_topic(topic_id)?;
        let topics = self.topic_vectors()?;
        let Some(me) = topics.iter().find(|t| t.id == topic_id) else {
            return Ok(Vec::new());
        };
        let mut out: Vec<SimilarTopic> = topics
            .iter()
            .filter(|t| t.id != topic_id)
            .map(|t| SimilarTopic {
                similarity: cosine(&me.vector, &t.vector),
                ask_count: t.count,
                topic: t.topic_ref(),
            })
            .filter(|s| s.similarity >= RECALL_MIN)
            .collect();
        out.sort_by(|x, y| y.similarity.total_cmp(&x.similarity));
        out.truncate(limit);
        Ok(out)
    }

    /// Records that two topics are not the same question.
    pub fn dismiss_pair(&self, a: &str, b: &str) -> Result<()> {
        let (a, b) = ordered(a, b);
        self.conn.execute(
            "INSERT OR IGNORE INTO dismissed_pair (topic_a, topic_b) VALUES (?1, ?2)",
            params![a, b],
        )?;
        Ok(())
    }

    fn dismissed(&self) -> Result<HashSet<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT topic_a, topic_b FROM dismissed_pair")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Note id to vector, for every note that has one.
    fn note_vectors(&self) -> Result<HashMap<String, Vec<f32>>> {
        let by_hash = self.vectors_by_hash()?;
        Ok(self
            .rows()?
            .into_iter()
            .filter_map(|r| {
                let hash = content_hash(note_text(&r.title, &r.question).as_bytes());
                by_hash.get(&hash).map(|v| (r.id, v.clone()))
            })
            .collect())
    }

    fn vectors_by_hash(&self) -> Result<HashMap<String, Vec<f32>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT text_hash, vector FROM embedding WHERE model = ?1")?;
        let rows = stmt.query_map([MODEL_KEY], |r| {
            Ok((r.get::<_, String>(0)?, from_blob(&r.get::<_, Vec<u8>>(1)?)))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn topic_vectors(&self) -> Result<Vec<TopicVector>> {
        let by_hash = self.vectors_by_hash()?;
        let labels = self.topic_labels()?;
        let mut notes: HashMap<String, Vec<&Row>> = HashMap::new();
        let rows = self.rows()?;
        for row in &rows {
            notes.entry(row.topic_id.clone()).or_default().push(row);
        }
        let mut out: Vec<TopicVector> = notes
            .into_iter()
            .filter_map(|(id, rows)| {
                let vectors: Vec<&Vec<f32>> = rows
                    .iter()
                    .filter_map(|r| {
                        by_hash.get(&content_hash(note_text(&r.title, &r.question).as_bytes()))
                    })
                    .collect();
                let vector = centroid(vectors.iter().map(|v| v.as_slice()))?;
                Some(TopicVector {
                    label: labels.get(&id).cloned().unwrap_or_default(),
                    count: rows.len(),
                    id,
                    vector,
                })
            })
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }
}

struct TopicVector {
    id: String,
    label: String,
    count: usize,
    vector: Vec<f32>,
}

impl TopicVector {
    fn topic_ref(&self) -> TopicRef {
        TopicRef {
            topic_id: self.id.clone(),
            label: self.label.clone(),
        }
    }
}

fn ordered(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}
