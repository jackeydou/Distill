#![allow(clippy::unwrap_used)] // Test helpers panic on setup failure by design.

mod common;

use common::{open, request, setup};
use distill_core::Error;

#[test]
fn rename_rewrites_only_the_topic_file() {
    let s = setup();
    let mut d = open(&s);
    let saved = d.save(request("new", &[], &["sqlite"])).unwrap();
    let note_before = std::fs::read_to_string(&saved.path).unwrap();

    d.rename_topic(&saved.topic_id, "  为什么 SQLite 扩展加载不了  ")
        .unwrap();
    let topic = d.index.topic_detail(&saved.topic_id).unwrap().unwrap();
    assert_eq!(topic.label, "为什么 SQLite 扩展加载不了");
    assert_eq!(std::fs::read_to_string(&saved.path).unwrap(), note_before);

    assert!(matches!(
        d.rename_topic(&saved.topic_id, "two\nlines"),
        Err(Error::InvalidTopic { .. })
    ));
    assert!(matches!(
        d.rename_topic("01NOPE", "x"),
        Err(Error::UnknownTopic { .. })
    ));
}

#[test]
fn merge_moves_the_count_and_keeps_the_notes() {
    let s = setup();
    let mut d = open(&s);
    let a = d.save(request("new", &[], &["sqlite"])).unwrap();
    let b = d.save(request("new", &["sqlite"], &[])).unwrap();
    let c = d.save(request(&b.topic_id, &["sqlite"], &[])).unwrap();

    let target = d.merge_topic(&a.topic_id, &b.topic_id).unwrap();
    assert_eq!(target, b.topic_id);
    assert_eq!(d.index.ask_count(&a.topic_id).unwrap(), 3);

    // Asking for the merged-away topic lands on the one it was merged into.
    let topic = d.index.topic_detail(&a.topic_id).unwrap().unwrap();
    assert_eq!(topic.topic_id, b.topic_id);
    assert_eq!(topic.merged_from.len(), 1);
    assert_eq!(topic.merged_from[0].topic_id, a.topic_id);
    let ids: Vec<&str> = topic.notes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, vec![&a.note_id, &b.note_id, &c.note_id]);
    assert_eq!(topic.sessions, 1);
    assert_eq!(d.index.topics().unwrap().len(), 1);

    match d.merge_topic(&b.topic_id, &a.topic_id) {
        Err(Error::InvalidMerge { reason, .. }) => assert!(reason.contains("already")),
        other => panic!("expected InvalidMerge, got {other:?}"),
    }
}

#[test]
fn annotations_can_be_edited_and_deleted() {
    let s = setup();
    let mut d = open(&s);
    let saved = d.save(request("new", &[], &["sqlite"])).unwrap();
    d.annotate(&saved.note_id, "第一版理解").unwrap();
    let id = d.index.note(&saved.note_id).unwrap().unwrap().annotations[0]
        .id
        .clone();

    d.update_annotation(&id, "改过：token sk-proj-abcdefghijklmnopqrstuvwx 不该出现")
        .unwrap();
    let a = &d.index.note(&saved.note_id).unwrap().unwrap().annotations[0];
    assert!(a.body.starts_with("改过"));
    assert!(a.body.contains("[REDACTED:"), "{}", a.body);
    assert!(a.updated.is_some());
    assert!(matches!(
        d.update_annotation(&id, "  "),
        Err(Error::InvalidNote { .. })
    ));

    d.delete_annotation(&id).unwrap();
    let note = d.index.note(&saved.note_id).unwrap().unwrap();
    assert!(note.annotations.is_empty());
    assert!(
        !s.dir
            .path()
            .join("vault/annotations")
            .join(&saved.note_id)
            .exists(),
        "the empty per-note directory is removed"
    );
    assert!(matches!(
        d.delete_annotation(&id),
        Err(Error::UnknownAnnotation { .. })
    ));
}

#[test]
fn resolving_a_conflict_sets_the_other_copy_aside() {
    let s = setup();
    let mut d = open(&s);
    let saved = d.save(request("new", &[], &["sqlite"])).unwrap();
    let copy = saved
        .path
        .with_file_name(format!("{} (conflicted copy).md", saved.note_id));
    std::fs::copy(&saved.path, &copy).unwrap();
    d.index.sync().unwrap();
    let conflict = d.index.conflicts().unwrap().remove(0);
    assert_eq!(conflict.paths.len(), 2);
    let keep = conflict
        .paths
        .iter()
        .find(|p| p.contains("conflicted copy"))
        .unwrap()
        .clone();

    assert!(matches!(
        d.resolve_conflict("note", &saved.note_id, "notes/elsewhere.md"),
        Err(Error::NotAConflictCopy { .. })
    ));
    let moved = d.resolve_conflict("note", &saved.note_id, &keep).unwrap();
    assert_eq!(moved.len(), 1);
    assert!(moved[0].starts_with(s.dirs.discarded_dir(d.vault.id())));
    assert!(moved[0].is_file());
    assert!(!saved.path.exists());
    assert!(copy.exists());
    assert!(d.index.conflicts().unwrap().is_empty());
    assert!(d.index.note(&saved.note_id).unwrap().is_some());

    assert!(matches!(
        d.resolve_conflict("note", &saved.note_id, &keep),
        Err(Error::UnknownConflict { .. })
    ));
}

#[test]
fn note_detail_has_the_page_contents() {
    let s = setup();
    let mut d = open(&s);
    let saved = d.save(request("new", &[], &["sqlite"])).unwrap();
    let detail = d.index.note_detail(&saved.note_id).unwrap().unwrap();
    assert!(detail.markdown.starts_with("# bun:sqlite"));
    assert!(detail.markdown.contains("## 要点"));
    assert_eq!(detail.ask_count, 1);
    assert_eq!(detail.topic_label, "bun:sqlite 在 macOS 上加载扩展");
    assert_eq!(std::path::Path::new(&detail.file), saved.path);
    assert!(detail.reopen.link.starts_with("codex://threads/"));
    assert!(d.index.note_detail("01NOPE").unwrap().is_none());
}
