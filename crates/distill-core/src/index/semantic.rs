//! Embedding-backed reads: recall that finds reworded questions keyword matching misses,
//! and topic pairs that are probably the same question (merge suggestions, spec P4).
//!
//! Vectors live in sqlite-vec `vec0` tables in the index (`0002_embeddings.sql`), and every
//! nearest-neighbour search is a `vec0` KNN query with cosine distance. Topic vectors are the
//! mean of their notes' vectors and are rebuilt only when the notes or topics change.

use std::collections::{HashMap, HashSet};

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use super::Index;
use super::browse::TopicRef;
use super::query::{RecallResult, Row};
use crate::embed::{Embedder, MODEL_KEY, centroid, from_blob, note_text, to_blob};
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
/// How many nearest notes take part in fusion.
const SEMANTIC_CANDIDATES: usize = 30;
/// Nearest topics checked per topic when looking for duplicates.
const DUPLICATE_NEIGHBOURS: usize = 5;

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
    /// Computes vectors for notes that have none under the current model, drops vectors no
    /// note uses any more, and refreshes topic vectors when anything changed. Returns how
    /// many notes were embedded.
    pub fn embed_missing(&self, embedder: &Embedder) -> Result<usize> {
        let rows = self.rows()?;
        let known: HashSet<String> = {
            let mut stmt = self.conn.prepare("SELECT key FROM note_vec")?;
            let keys = stmt.query_map([], |r| r.get(0))?;
            keys.collect::<rusqlite::Result<_>>()?
        };
        let wanted: HashMap<String, String> = rows
            .iter()
            .map(|r| (note_key(r), note_text(&r.title, &r.question)))
            .collect();
        let missing: Vec<(&String, &String)> =
            wanted.iter().filter(|(k, _)| !known.contains(*k)).collect();
        let vectors = if missing.is_empty() {
            Vec::new()
        } else {
            let texts: Vec<String> = missing.iter().map(|(_, t)| (*t).clone()).collect();
            embedder.embed(&texts)?
        };
        let tx = self.conn.unchecked_transaction()?;
        for ((key, _), vector) in missing.iter().zip(&vectors) {
            tx.execute(
                "INSERT INTO note_vec (key, vector) VALUES (?1, ?2)",
                params![key, to_blob(vector)],
            )?;
        }
        for stale in known.iter().filter(|k| !wanted.contains_key(*k)) {
            tx.execute("DELETE FROM note_vec WHERE key = ?1", [stale])?;
        }
        tx.commit()?;
        self.refresh_topic_vectors(&rows)?;
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
        let query = to_blob(&embedder.embed_one(question)?);
        let mut stmt = self.conn.prepare(
            "SELECT key, distance FROM note_vec WHERE vector MATCH ?1 AND k = ?2
             ORDER BY distance",
        )?;
        let hits = stmt.query_map(params![query, SEMANTIC_CANDIDATES as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?;
        let mut keys: Vec<String> = Vec::new();
        for hit in hits {
            let (key, distance) = hit?;
            if similarity(distance) >= RECALL_MIN {
                keys.push(key);
            }
        }
        // Several notes can share one key when their title and question are identical.
        let rows = self.rows()?;
        let semantic: Vec<String> = keys
            .iter()
            .flat_map(|key| rows.iter().filter(move |r| note_key(r) == *key))
            .map(|r| r.id.clone())
            .collect();
        self.recall_with(question, limit, Some(&semantic))
    }

    /// Topic pairs that are probably the same question, most similar first. Pairs the user
    /// dismissed are left out.
    pub fn duplicate_topics(&self, embedder: &Embedder) -> Result<Vec<TopicPair>> {
        self.embed_missing(embedder)?;
        let topics = self.topic_info()?;
        let dismissed = self.dismissed()?;
        let mut seen: HashSet<(String, String)> = HashSet::new();
        let mut pairs = Vec::new();
        for id in topics.keys() {
            for (other, similarity) in self.nearest_topics(id, DUPLICATE_NEIGHBOURS)? {
                let key = ordered(id, &other);
                if similarity < DUPLICATE_MIN || dismissed.contains(&key) || !seen.insert(key) {
                    continue;
                }
                let (Some(a), Some(b)) = (topics.get(id), topics.get(&other)) else {
                    continue;
                };
                // The more-asked topic is `b`, the natural merge target.
                let (a, b) = if a.count > b.count { (b, a) } else { (a, b) };
                pairs.push(TopicPair {
                    a: a.topic.clone(),
                    b: b.topic.clone(),
                    similarity,
                    a_count: a.count,
                    b_count: b.count,
                });
            }
        }
        pairs.sort_by(|x, y| {
            y.similarity
                .total_cmp(&x.similarity)
                .then(x.a.topic_id.cmp(&y.a.topic_id))
        });
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
        let topics = self.topic_info()?;
        Ok(self
            .nearest_topics(&topic_id, limit)?
            .into_iter()
            .filter(|(_, similarity)| *similarity >= RECALL_MIN)
            .filter_map(|(id, similarity)| {
                let info = topics.get(&id)?;
                Some(SimilarTopic {
                    topic: info.topic.clone(),
                    similarity,
                    ask_count: info.count,
                })
            })
            .collect())
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

    /// The `limit` topics nearest to `topic_id` by a `vec0` KNN query, with their cosine
    /// similarity. Empty when the topic has no vector.
    fn nearest_topics(&self, topic_id: &str, limit: usize) -> Result<Vec<(String, f32)>> {
        let vector: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT vector FROM topic_vec WHERE topic_id = ?1",
                [topic_id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(vector) = vector else {
            return Ok(Vec::new());
        };
        let mut stmt = self.conn.prepare(
            "SELECT topic_id, distance FROM topic_vec WHERE vector MATCH ?1 AND k = ?2
             ORDER BY distance",
        )?;
        let hits = stmt.query_map(params![vector, (limit + 1) as i64], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?))
        })?;
        let mut out = Vec::new();
        for hit in hits {
            let (id, distance) = hit?;
            if id != topic_id {
                out.push((id, similarity(distance)));
            }
        }
        out.truncate(limit);
        Ok(out)
    }

    /// Rewrites `topic_vec` when the topics or their notes' vectors changed since the last
    /// time. The check is one hash over every (topic, note key) pair.
    fn refresh_topic_vectors(&self, rows: &[Row]) -> Result<()> {
        let mut members: Vec<(&str, String)> = rows
            .iter()
            .map(|r| (r.topic_id.as_str(), note_key(r)))
            .collect();
        members.sort();
        let signature = content_hash(
            members
                .iter()
                .map(|(t, k)| format!("{t} {k}\n"))
                .collect::<String>()
                .as_bytes(),
        );
        let stored: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM vec_state WHERE name = 'topic_vec'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if stored.as_deref() == Some(signature.as_str()) {
            return Ok(());
        }
        let vectors: HashMap<String, Vec<f32>> = {
            let mut stmt = self.conn.prepare("SELECT key, vector FROM note_vec")?;
            let rows = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, from_blob(&r.get::<_, Vec<u8>>(1)?)))
            })?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut by_topic: HashMap<&str, Vec<&[f32]>> = HashMap::new();
        for (topic, key) in &members {
            if let Some(v) = vectors.get(key) {
                by_topic.entry(topic).or_default().push(v);
            }
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM topic_vec", [])?;
        for (topic, vs) in by_topic {
            if let Some(mean) = centroid(vs) {
                tx.execute(
                    "INSERT INTO topic_vec (topic_id, vector) VALUES (?1, ?2)",
                    params![topic, to_blob(&mean)],
                )?;
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO vec_state (name, value) VALUES ('topic_vec', ?1)",
            [signature],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn dismissed(&self) -> Result<HashSet<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT topic_a, topic_b FROM dismissed_pair")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Label and note count of every topic that has notes.
    fn topic_info(&self) -> Result<HashMap<String, TopicInfo>> {
        let labels = self.topic_labels()?;
        let mut out: HashMap<String, TopicInfo> = HashMap::new();
        for row in self.rows()? {
            out.entry(row.topic_id.clone())
                .or_insert_with(|| TopicInfo {
                    topic: TopicRef {
                        label: labels.get(&row.topic_id).cloned().unwrap_or_default(),
                        topic_id: row.topic_id.clone(),
                    },
                    count: 0,
                })
                .count += 1;
        }
        Ok(out)
    }
}

struct TopicInfo {
    topic: TopicRef,
    count: usize,
}

/// `vec0` cosine distance is `1 - cosine similarity`.
fn similarity(distance: f64) -> f32 {
    (1.0 - distance) as f32
}

/// What a note's vector is stored under: the model and the hash of the embedded text.
fn note_key(row: &Row) -> String {
    format!(
        "{MODEL_KEY}:{}",
        content_hash(note_text(&row.title, &row.question).as_bytes())
    )
}

fn ordered(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn unit(v: &[f32]) -> Vec<f32> {
        let mut v = v.to_vec();
        v.resize(384, 0.0);
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter().map(|x| x / norm).collect()
    }

    #[test]
    fn nearest_topics_is_a_vec0_knn_query() {
        let dir = tempfile::tempdir().unwrap();
        let index = Index::open(&dir.path().join("index.db"), dir.path()).unwrap();
        let version: String = index
            .conn
            .query_row("SELECT vec_version()", [], |r| r.get(0))
            .unwrap();
        assert!(version.starts_with('v'), "{version}");
        for (id, v) in [
            ("A", unit(&[1.0, 0.0])),
            ("B", unit(&[0.9, 0.1])),
            ("C", unit(&[0.0, 1.0])),
        ] {
            index
                .conn
                .execute(
                    "INSERT INTO topic_vec (topic_id, vector) VALUES (?1, ?2)",
                    params![id, to_blob(&v)],
                )
                .unwrap();
        }
        let near = index.nearest_topics("A", 2).unwrap();
        assert_eq!(near[0].0, "B");
        assert!(near[0].1 > 0.99, "{near:?}");
        assert_eq!(near[1].0, "C");
        assert!(near[1].1.abs() < 1e-3, "{near:?}");
        assert!(index.nearest_topics("missing", 2).unwrap().is_empty());
    }
}
