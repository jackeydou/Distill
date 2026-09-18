use super::Index;
use crate::model::{Agent, NoteBody, NoteMeta, SCHEMA, Source, TagAlias, TopicMeta};
use crate::vault::{Vault, new_id};

struct Fixture {
    _dir: tempfile::TempDir,
    vault: Vault,
    index: Index,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let (vault, _) = Vault::open_or_create(&dir.path().join("vault"), false).unwrap();
    let index = Index::open(&dir.path().join("index.db"), vault.root()).unwrap();
    Fixture {
        _dir: dir,
        vault,
        index,
    }
}

fn source() -> Source {
    Source {
        agent: Agent::ClaudeCode,
        session_id: "133e9647-5436-4a71-9352-39b822dfb773".into(),
        cwd: "/Users/me/project".into(),
        git_repo: None,
    }
}

fn write_topic(vault: &Vault, id: &str, label: &str, merged_into: Option<&str>) {
    vault
        .write_topic(
            &TopicMeta {
                schema: SCHEMA,
                id: id.into(),
                label: label.into(),
                merged_into: merged_into.map(str::to_string),
                created: "2026-09-13T10:00:00+08:00".into(),
            },
            "",
        )
        .unwrap();
}

fn write_note(vault: &Vault, topic: &str, title: &str, conclusion: &str, tags: &[&str]) -> String {
    let id = new_id();
    let created = "2026-09-13T10:00:00+08:00";
    let body = NoteBody {
        title: title.into(),
        question: title.into(),
        conclusion: conclusion.into(),
        ..Default::default()
    };
    let meta = NoteMeta {
        schema: SCHEMA,
        id: id.clone(),
        topic: topic.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        source: source(),
        created: created.into(),
        updated: None,
    };
    vault
        .write_note(&vault.note_path(&id, title, created), &meta, &body)
        .unwrap();
    id
}

#[test]
fn sync_is_incremental() {
    let mut f = fixture();
    write_topic(&f.vault, "T1", "sqlite", None);
    write_note(
        &f.vault,
        "T1",
        "sqlite 扩展加载",
        "用 rusqlite",
        &["sqlite"],
    );
    let first = f.index.sync().unwrap();
    assert_eq!((first.added, first.unchanged), (2, 0));

    let second = f.index.sync().unwrap();
    assert_eq!((second.added, second.updated, second.unchanged), (0, 0, 2));

    write_note(&f.vault, "T1", "another", "c", &["sqlite"]);
    let third = f.index.sync().unwrap();
    assert_eq!((third.added, third.unchanged), (1, 2));
}

#[test]
fn deleted_files_leave_the_index() {
    let mut f = fixture();
    write_topic(&f.vault, "T1", "t", None);
    let id = write_note(&f.vault, "T1", "to be removed", "c", &["x"]);
    f.index.sync().unwrap();
    let path = f
        .vault
        .root()
        .join(f.index.note(&id).unwrap().unwrap().path);
    std::fs::remove_file(path).unwrap();
    let report = f.index.sync().unwrap();
    assert_eq!(report.removed, 1);
    assert!(f.index.note(&id).unwrap().is_none());
    assert!(f.index.search("removed", None, 10).unwrap().is_empty());
}

#[test]
fn duplicate_ids_are_conflicts_and_shortest_path_wins() {
    let mut f = fixture();
    write_topic(&f.vault, "T1", "t", None);
    let id = write_note(&f.vault, "T1", "original", "c", &["x"]);
    f.index.sync().unwrap();
    let original = f.index.note(&id).unwrap().unwrap().path;
    let copy = original.replace(".md", " (conflicted copy 2026-09-13).md");
    std::fs::copy(f.vault.root().join(&original), f.vault.root().join(&copy)).unwrap();
    f.index.sync().unwrap();

    let conflicts = f.index.conflicts().unwrap();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].paths, vec![original.clone(), copy]);
    assert_eq!(f.index.note(&id).unwrap().unwrap().path, original);
    assert_eq!(f.index.stats().unwrap().notes, 1);
}

#[test]
fn invalid_files_are_reported_not_fatal() {
    let mut f = fixture();
    let bad = f.vault.root().join("notes/2026/09/broken.md");
    std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
    std::fs::write(&bad, "no frontmatter here").unwrap();
    let report = f.index.sync().unwrap();
    assert_eq!(report.invalid, vec!["notes/2026/09/broken.md"]);
    let invalid = f.index.invalid_files().unwrap();
    assert!(invalid[0].error.contains("frontmatter"));
}

#[test]
fn ignores_sync_tool_files() {
    let mut f = fixture();
    let root = f.vault.root();
    std::fs::create_dir_all(root.join(".stfolder")).unwrap();
    std::fs::write(root.join(".stfolder/x.md"), "x").unwrap();
    std::fs::write(root.join("notes/.distill-tmp-abc"), "x").unwrap();
    std::fs::write(root.join("README.md"), "x").unwrap();
    let report = f.index.sync().unwrap();
    assert_eq!((report.added, report.invalid.len()), (0, 0));
}

#[test]
fn recall_matches_chinese_and_groups_by_topic() {
    let mut f = fixture();
    write_topic(&f.vault, "T1", "bun 扩展", None);
    write_topic(&f.vault, "T2", "wal", None);
    write_note(
        &f.vault,
        "T1",
        "bun:sqlite 加载扩展失败",
        "系统 SQLite 禁止加载扩展",
        &["sqlite"],
    );
    write_note(
        &f.vault,
        "T1",
        "为什么 bun 加载不了扩展",
        "换 rusqlite",
        &["sqlite"],
    );
    write_note(&f.vault, "T2", "SQLite WAL 并发", "一写多读", &["sqlite"]);
    f.index.sync().unwrap();

    let result = f.index.recall("bun 为什么加载扩展会失败", 5).unwrap();
    assert_eq!(result.topics[0].topic_id, "T1");
    assert_eq!(result.topics[0].ask_count, 2);
    assert_eq!(result.topics[0].label, "bun 扩展");
    assert_eq!(result.tags[0].tag, "sqlite");
    assert_eq!(result.tags[0].notes, 3);
}

#[test]
fn search_falls_back_to_like_for_short_pieces() {
    let mut f = fixture();
    write_topic(&f.vault, "T1", "t", None);
    write_note(&f.vault, "T1", "加载扩展", "c", &["x"]);
    write_note(&f.vault, "T1", "并发写入", "c", &["x"]);
    f.index.sync().unwrap();
    let hits = f.index.search("扩展", None, 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "加载扩展");
    assert_eq!(f.index.search("", None, 10).unwrap().len(), 2);
}

#[test]
fn merges_and_aliases_resolve() {
    let mut f = fixture();
    write_topic(&f.vault, "OLD", "old", Some("NEW"));
    write_topic(&f.vault, "NEW", "new", None);
    write_note(&f.vault, "OLD", "a", "c", &["sqlite3"]);
    write_note(&f.vault, "NEW", "b", "c", &["sqlite"]);
    f.vault
        .write_alias(&TagAlias {
            schema: SCHEMA,
            from: "sqlite3".into(),
            to: "sqlite".into(),
            created: "2026-09-13T10:00:00+08:00".into(),
        })
        .unwrap();
    f.index.sync().unwrap();

    assert_eq!(f.index.resolve_topic("OLD").unwrap(), "NEW");
    assert_eq!(f.index.ask_count("OLD").unwrap(), 2);
    let tags = f.index.tags().unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!((tags[0].tag.as_str(), tags[0].notes), ("sqlite", 2));
    assert_eq!(f.index.search("", Some("sqlite3"), 10).unwrap().len(), 2);
}

#[test]
fn rebuild_matches_incremental() {
    let mut f = fixture();
    write_topic(&f.vault, "T1", "t", None);
    write_note(&f.vault, "T1", "one", "c", &["x"]);
    f.index.sync().unwrap();
    let report = f.index.rebuild().unwrap();
    assert_eq!(report.added, 2);
    assert_eq!(f.index.stats().unwrap().notes, 1);
}
