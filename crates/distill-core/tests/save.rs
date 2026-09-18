#![allow(clippy::unwrap_used)] // Test helpers panic on setup failure by design.

use distill_core::config::Dirs;
use distill_core::model::{Agent, Source};
use distill_core::ops::{InitOptions, SaveRequest, init};
use distill_core::{Distill, Error};

struct Setup {
    dir: tempfile::TempDir,
    dirs: Dirs,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let dirs = Dirs {
        config_dir: dir.path().join("home/config"),
        data_dir: dir.path().join("home/data"),
    };
    init(
        &dirs,
        InitOptions {
            vault: dir.path().join("vault"),
            adopt: false,
            bin_path: None,
        },
    )
    .unwrap();
    Setup { dir, dirs }
}

fn open(s: &Setup) -> Distill {
    Distill::open_in(s.dirs.clone()).unwrap()
}

fn request(topic: &str, tags: &[&str], new_tags: &[&str]) -> SaveRequest {
    SaveRequest {
        title: "bun:sqlite 在 macOS 上加载扩展".into(),
        question: "为什么 bun:sqlite 加载不了 sqlite-vec？".into(),
        conclusion: "macOS 系统 SQLite 禁止加载扩展。".into(),
        key_points: vec!["换 rusqlite bundled".into()],
        open_questions: vec![],
        topic: topic.into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        new_tags: new_tags.iter().map(|t| t.to_string()).collect(),
        confirm_new: false,
        source: Source {
            agent: Agent::Codex,
            session_id: "01a0d530-42ae-7731-8a1d-b4e07d9b837d".into(),
            cwd: "/Users/me/project".into(),
            git_repo: Some("github.com/me/project".into()),
        },
    }
}

#[test]
fn second_save_on_same_topic_counts_as_a_repeat() {
    let s = setup();
    let mut d = open(&s);
    let first = d.save(request("new", &[], &["sqlite"])).unwrap();
    assert!(first.topic_created);
    assert_eq!(first.ask_count, 1);
    assert_eq!(first.tags_created, vec!["sqlite"]);
    assert!(first.url.ends_with(&format!("/notes/{}", first.note_id)));

    let second = d.save(request(&first.topic_id, &["sqlite"], &[])).unwrap();
    assert!(!second.topic_created);
    assert_eq!(second.topic_id, first.topic_id);
    assert_eq!(second.ask_count, 2);

    let stats = open(&s).index.stats().unwrap();
    assert_eq!(
        (stats.notes, stats.topics, stats.repeated_topics),
        (2, 1, 1)
    );
    assert_eq!(stats.top_topics[0].ask_count, 2);
}

#[test]
fn tags_must_exist_unless_declared_new() {
    let s = setup();
    let mut d = open(&s);
    d.save(request("new", &[], &["sqlite"])).unwrap();

    match d.save(request("new", &["sqlit"], &[])) {
        Err(Error::UnknownTag { suggestions, .. }) => assert_eq!(suggestions, vec!["sqlite"]),
        other => panic!("expected UnknownTag, got {other:?}"),
    }
    match d.save(request("new", &[], &["SQLite3"])) {
        Err(Error::SimilarTag { tag, similar }) => {
            assert_eq!(tag, "sqlite3");
            assert_eq!(similar, vec!["sqlite"]);
        }
        other => panic!("expected SimilarTag, got {other:?}"),
    }
    let mut confirmed = request("new", &[], &["sqlite3"]);
    confirmed.confirm_new = true;
    assert_eq!(d.save(confirmed).unwrap().tags_created, vec!["sqlite3"]);

    // A "new" tag that normalizes to an existing one is reused, not created.
    let reused = d.save(request("new", &[], &[" SQLite "])).unwrap();
    assert!(reused.tags_created.is_empty());
    assert_eq!(reused.tags, vec!["sqlite"]);
}

#[test]
fn rejects_bad_input() {
    let s = setup();
    let mut d = open(&s);
    assert!(matches!(
        d.save(request("new", &[], &[])),
        Err(Error::TagCount { count: 0, .. })
    ));
    assert!(matches!(
        d.save(request("01UNKNOWNTOPIC", &[], &["x"])),
        Err(Error::UnknownTopic { .. })
    ));
    let mut bad_source = request("new", &[], &["x"]);
    bad_source.source.session_id = "last".into();
    assert!(matches!(
        d.save(bad_source),
        Err(Error::InvalidSource { .. })
    ));
    let mut empty = request("new", &[], &["x"]);
    empty.conclusion = "  ".into();
    assert!(matches!(d.save(empty), Err(Error::InvalidNote { .. })));
}

#[test]
fn secrets_never_reach_the_vault() {
    let s = setup();
    let mut d = open(&s);
    let mut req = request("new", &[], &["x"]);
    req.conclusion = "用 OPENAI_API_KEY=sk-proj-abcdefghijklmnopqrstuvwx 调用".into();
    let saved = d.save(req).unwrap();
    let text = std::fs::read_to_string(saved.path).unwrap();
    assert!(!text.contains("sk-proj-abcdefghijklmnopqrstuvwx"), "{text}");
    assert!(text.contains("[REDACTED:"), "{text}");
}

#[test]
fn annotations_come_back_with_recall() {
    let s = setup();
    let mut d = open(&s);
    let saved = d.save(request("new", &[], &["sqlite"])).unwrap();
    d.annotate(&saved.note_id, "关键是系统 SQLite 编译时关了扩展加载")
        .unwrap();
    let recall = d.index.recall("bun 加载扩展", 5).unwrap();
    let note = &recall.topics[0].notes[0];
    assert_eq!(note.annotations.len(), 1);
    assert!(note.annotations[0].body.contains("编译时"));
    assert!(matches!(
        d.annotate("01NOPE", "x"),
        Err(Error::UnknownNote { .. })
    ));
}

#[test]
fn init_is_idempotent_and_guards_locations() {
    let s = setup();
    let before = open(&s).config;
    let again = init(
        &s.dirs,
        InitOptions {
            vault: s.dir.path().join("vault"),
            adopt: false,
            bin_path: None,
        },
    )
    .unwrap();
    assert!(!again.created_vault);
    assert_eq!(again.device_id, before.device_id);
    assert_eq!(again.ui_port, before.ui.port);

    let inside = init(
        &s.dirs,
        InitOptions {
            vault: s.dirs.data_dir.join("vault"),
            adopt: false,
            bin_path: None,
        },
    );
    assert!(matches!(inside, Err(Error::InvalidVaultLocation { .. })));
}

#[test]
fn move_vault_keeps_notes() {
    let s = setup();
    let mut d = open(&s);
    let saved = d.save(request("new", &[], &["sqlite"])).unwrap();
    let dest = s.dir.path().join("moved");
    let copied = d.move_vault(&dest).unwrap();
    assert!(copied >= 3);
    assert!(s.dir.path().join("vault").exists(), "original is kept");

    let reopened = open(&s);
    assert_eq!(reopened.vault.root(), dest);
    assert!(reopened.index.note(&saved.note_id).unwrap().is_some());
}
