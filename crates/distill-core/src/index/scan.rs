use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};
use std::time::UNIX_EPOCH;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};
use serde::Serialize;

use crate::error::{Error, IoContext, Result};
use crate::fsutil::content_hash;
use crate::model::{AnnotationMeta, NoteBody, NoteMeta, TagAlias, TopicMeta};
use crate::vault::{
    ALIASES_DIR, ANNOTATIONS_DIR, NOTES_DIR, TOPICS_DIR, check_schema, parse_md, parse_yaml,
};

#[derive(Debug, Default, Clone, Serialize)]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    /// Files that failed to parse this run. They stay listed in `distill doctor`.
    pub invalid: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Note,
    Topic,
    Alias,
    Annotation,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Note => "note",
            Kind::Topic => "topic",
            Kind::Alias => "alias",
            Kind::Annotation => "annotation",
        }
    }

    /// Classifies a vault-relative path. Anything else in the vault (sync-tool metadata,
    /// temp files, the user's own files) is ignored.
    fn of(rel: &str) -> Option<Self> {
        let parts: Vec<&str> = rel.split('/').collect();
        if parts.iter().any(|p| p.starts_with('.')) {
            return None;
        }
        let ext = rel.rsplit_once('.').map(|(_, e)| e)?;
        match (parts.first().copied()?, parts.len(), ext) {
            (NOTES_DIR, n, "md") if n >= 2 => Some(Kind::Note),
            (TOPICS_DIR, 2, "md") => Some(Kind::Topic),
            (ALIASES_DIR, 2, "yml" | "yaml") => Some(Kind::Alias),
            (ANNOTATIONS_DIR, 3, "md") => Some(Kind::Annotation),
            _ => None,
        }
    }
}

struct Seen {
    kind: Kind,
    mtime_ns: i64,
    size: i64,
}

pub(super) fn sync(conn: &mut Connection, root: &Path, rebuild: bool) -> Result<SyncReport> {
    let on_disk = walk(root)?;
    let tx = Transaction::new(conn, TransactionBehavior::Immediate)?;
    if rebuild {
        tx.execute("DELETE FROM note_fts", [])?;
        tx.execute("DELETE FROM file", [])?;
    }
    let known = known_files(&tx)?;
    let mut report = SyncReport::default();

    for (rel, seen) in &on_disk {
        match known.get(rel) {
            Some(&(mtime, size, _)) if mtime == seen.mtime_ns && size == seen.size => {
                report.unchanged += 1;
            }
            Some((_, _, hash)) => {
                let bytes = std::fs::read(root.join(rel)).at(root.join(rel))?;
                if content_hash(&bytes) == *hash {
                    tx.execute(
                        "UPDATE file SET mtime_ns = ?2, size = ?3 WHERE path = ?1",
                        params![rel, seen.mtime_ns, seen.size],
                    )?;
                    report.unchanged += 1;
                } else {
                    remove(&tx, rel)?;
                    ingest(&tx, root, rel, seen, &bytes, &mut report)?;
                    report.updated += 1;
                }
            }
            None => {
                let bytes = std::fs::read(root.join(rel)).at(root.join(rel))?;
                ingest(&tx, root, rel, seen, &bytes, &mut report)?;
                report.added += 1;
            }
        }
    }
    let on_disk_paths: HashSet<&String> = on_disk.keys().collect();
    for rel in known.keys().filter(|k| !on_disk_paths.contains(k)) {
        remove(&tx, rel)?;
        report.removed += 1;
    }
    tx.commit()?;
    Ok(report)
}

fn walk(root: &Path) -> Result<HashMap<String, Seen>> {
    let mut out = HashMap::new();
    let walker = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'));
    for entry in walker {
        let entry = entry.map_err(|e| Error::Io {
            path: e.path().unwrap_or(root).to_path_buf(),
            source: e.into(),
        })?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(rel) = relative(root, entry.path()) else {
            continue;
        };
        let Some(kind) = Kind::of(&rel) else {
            continue;
        };
        let meta = entry.metadata().map_err(|e| Error::Io {
            path: entry.path().to_path_buf(),
            source: e.into(),
        })?;
        let mtime_ns = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as i64);
        out.insert(
            rel,
            Seen {
                kind,
                mtime_ns,
                size: meta.len() as i64,
            },
        );
    }
    Ok(out)
}

/// Vault-relative path with `/` separators on every platform.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    Some(parts.join("/"))
}

fn known_files(tx: &Transaction) -> Result<HashMap<String, (i64, i64, String)>> {
    let mut stmt = tx.prepare("SELECT path, mtime_ns, size, hash FROM file")?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?)))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn remove(tx: &Transaction, rel: &str) -> Result<()> {
    tx.execute("DELETE FROM note_fts WHERE path = ?1", [rel])?;
    tx.execute("DELETE FROM file WHERE path = ?1", [rel])?;
    Ok(())
}

fn ingest(
    tx: &Transaction,
    root: &Path,
    rel: &str,
    seen: &Seen,
    bytes: &[u8],
    report: &mut SyncReport,
) -> Result<()> {
    let hash = content_hash(bytes);
    tx.execute(
        "INSERT INTO file (path, kind, mtime_ns, size, hash) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![rel, seen.kind.as_str(), seen.mtime_ns, seen.size, hash],
    )?;
    let path = root.join(rel);
    let parsed = std::str::from_utf8(bytes)
        .map_err(|e| Error::InvalidFile {
            path: path.clone(),
            reason: format!("not UTF-8: {e}"),
        })
        .and_then(|text| match seen.kind {
            Kind::Note => insert_note(tx, &path, rel, text),
            Kind::Topic => insert_topic(tx, &path, rel, text),
            Kind::Alias => insert_alias(tx, &path, rel, text),
            Kind::Annotation => insert_annotation(tx, &path, rel, text),
        });
    match parsed {
        Ok(()) => Ok(()),
        Err(Error::InvalidFile { reason, .. }) => {
            tx.execute(
                "UPDATE file SET error = ?2 WHERE path = ?1",
                params![rel, reason],
            )?;
            report.invalid.push(rel.to_string());
            Ok(())
        }
        Err(other) => Err(other),
    }
}

fn to_utc(path: &Path, rfc3339: &str) -> Result<String> {
    let ts: jiff::Timestamp = rfc3339.parse().map_err(|e| Error::InvalidFile {
        path: path.to_path_buf(),
        reason: format!("`created` is not an RFC 3339 timestamp: {e}"),
    })?;
    Ok(ts.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
}

fn insert_note(tx: &Transaction, path: &Path, rel: &str, text: &str) -> Result<()> {
    let (meta, body_md): (NoteMeta, String) = parse_md(path, text)?;
    check_schema(path, meta.schema)?;
    meta.source.validate().map_err(|e| Error::InvalidFile {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    let body = NoteBody::parse(&body_md);
    let created_utc = to_utc(path, &meta.created)?;
    tx.execute(
        "INSERT INTO note (path, id, topic_id, title, question, conclusion, body, agent,
             session_id, cwd, git_repo, created, created_utc)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            rel,
            meta.id,
            meta.topic,
            body.title,
            body.question,
            body.conclusion,
            body_md,
            meta.source.agent.as_str(),
            meta.source.session_id,
            meta.source.cwd,
            meta.source.git_repo,
            meta.created,
            created_utc,
        ],
    )?;
    for tag in &meta.tags {
        tx.execute(
            "INSERT OR IGNORE INTO note_tag (note_path, tag) VALUES (?1, ?2)",
            params![rel, tag],
        )?;
    }
    tx.execute(
        "INSERT INTO note_fts (path, title, question, body) VALUES (?1, ?2, ?3, ?4)",
        params![rel, body.title, body.question, body_md],
    )?;
    Ok(())
}

fn insert_topic(tx: &Transaction, path: &Path, rel: &str, text: &str) -> Result<()> {
    let (meta, _description): (TopicMeta, String) = parse_md(path, text)?;
    check_schema(path, meta.schema)?;
    tx.execute(
        "INSERT INTO topic (path, id, label, merged_into, created) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![rel, meta.id, meta.label, meta.merged_into, meta.created],
    )?;
    Ok(())
}

fn insert_alias(tx: &Transaction, path: &Path, rel: &str, text: &str) -> Result<()> {
    let alias: TagAlias = parse_yaml(path, text)?;
    check_schema(path, alias.schema)?;
    tx.execute(
        "INSERT INTO tag_alias (path, from_tag, to_tag, created) VALUES (?1, ?2, ?3, ?4)",
        params![rel, alias.from, alias.to, alias.created],
    )?;
    Ok(())
}

fn insert_annotation(tx: &Transaction, path: &Path, rel: &str, text: &str) -> Result<()> {
    let (meta, body): (AnnotationMeta, String) = parse_md(path, text)?;
    check_schema(path, meta.schema)?;
    tx.execute(
        "INSERT INTO annotation (path, id, note_id, body, created, updated)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            rel,
            meta.id,
            meta.note,
            body.trim(),
            meta.created,
            meta.updated
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Kind;

    #[test]
    fn classifies_vault_paths() {
        assert_eq!(Kind::of("notes/2026/09/01AB-x.md"), Some(Kind::Note));
        assert_eq!(Kind::of("topics/01AB.md"), Some(Kind::Topic));
        assert_eq!(Kind::of("tag-aliases/01AB.yml"), Some(Kind::Alias));
        assert_eq!(
            Kind::of("annotations/01NOTE/01AB.md"),
            Some(Kind::Annotation)
        );
        assert_eq!(Kind::of("notes/2026/.distill-tmp-abc"), None);
        assert_eq!(Kind::of(".stfolder/x.md"), None);
        assert_eq!(Kind::of("README.md"), None);
        assert_eq!(Kind::of("topics/nested/x.md"), None);
    }
}
