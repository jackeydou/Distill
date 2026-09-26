#![allow(clippy::unwrap_used)] // Test helpers panic on setup failure by design.

mod common;

use common::{Setup, open, request, setup};
use distill_core::embed::Embedder;
use distill_core::ops::SaveRequest;

fn note(title: &str, question: &str, topic: &str, tag: &str) -> SaveRequest {
    let mut req = request(topic, &[], &[tag]);
    req.title = title.into();
    req.question = question.into();
    req.conclusion = format!("{title}：结论。");
    req.key_points = vec![];
    req.confirm_new = true;
    req
}

/// Points the setup at a model pulled beforehand with
/// `DISTILL_HOME=<dir> distill model pull`, so tests do not download 0.25 GB.
fn with_model(s: &Setup) -> bool {
    let Some(home) = std::env::var_os("DISTILL_TEST_MODEL_HOME") else {
        return false;
    };
    let pulled = std::path::PathBuf::from(home).join("data/models");
    assert!(pulled.is_dir(), "{} has no pulled model", pulled.display());
    std::fs::create_dir_all(&s.dirs.data_dir).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&pulled, Embedder::models_dir(&s.dirs)).unwrap();
    Embedder::installed(&s.dirs)
}

#[test]
fn without_the_model_recall_uses_keywords() {
    let s = setup();
    let mut d = open(&s);
    d.save(note(
        "git rebase 和 merge 怎么选",
        "rebase 还是 merge？",
        "new",
        "git",
    ))
    .unwrap();
    let recall = d.recall("git rebase 还是 merge", 5).unwrap();
    assert!(!recall.semantic);
    assert_eq!(recall.topics.len(), 1);
    let dup = d.duplicate_topics().unwrap();
    assert!(!dup.model_installed && dup.pairs.is_empty());
}

/// The P4 exit criterion: recall finds a reworded question that keyword matching misses.
#[test]
#[ignore = "needs a pulled model: DISTILL_TEST_MODEL_HOME=<DISTILL_HOME with `distill model pull` done>"]
fn recall_finds_reworded_questions_keywords_miss() {
    let s = setup();
    assert!(with_model(&s), "set DISTILL_TEST_MODEL_HOME");
    let mut d = open(&s);
    let git = d
        .save(note(
            "如何让 Git 忽略已经提交过的文件",
            "文件已经提交了，加进 .gitignore 为什么没用？",
            "new",
            "git",
        ))
        .unwrap();
    let effect = d
        .save(note(
            "React StrictMode 为什么让 useEffect 跑两次",
            "useEffect 为什么执行两次？",
            "new",
            "react",
        ))
        .unwrap();
    d.save(note(
        "SQLite WAL 模式下多进程并发写",
        "WAL 能不能多进程写？",
        "new",
        "sqlite",
    ))
    .unwrap();

    for (question, expected) in [
        (
            "stop tracking a file that's already in the repo",
            &git.topic_id,
        ),
        ("开发环境下 effect 执行了两遍", &effect.topic_id),
    ] {
        let keyword = d.index.recall(question, 5).unwrap();
        assert!(
            keyword.topics.iter().all(|t| &t.topic_id != expected),
            "keyword recall already finds {question:?}; pick a harder rewording"
        );
        let semantic = d.recall(question, 5).unwrap();
        assert!(semantic.semantic);
        assert_eq!(&semantic.topics[0].topic_id, expected, "{question:?}");
    }
}

#[test]
#[ignore = "needs a pulled model: DISTILL_TEST_MODEL_HOME=<DISTILL_HOME with `distill model pull` done>"]
fn near_identical_topics_are_suggested_until_dismissed() {
    let s = setup();
    assert!(with_model(&s), "set DISTILL_TEST_MODEL_HOME");
    let mut d = open(&s);
    let first = d
        .save(note(
            "Rust 里 Box<dyn Error> 和 anyhow 的区别",
            "Box<dyn Error> 和 anyhow 有什么区别？",
            "new",
            "rust",
        ))
        .unwrap();
    let again = d
        .save(note(
            "anyhow 和 Box<dyn Error> 该用哪个",
            "错误处理用 anyhow 还是 Box<dyn Error>？",
            "new",
            "rust",
        ))
        .unwrap();
    d.save(note(
        "git rebase 和 merge 怎么选",
        "rebase 还是 merge？",
        "new",
        "git",
    ))
    .unwrap();

    let dup = d.duplicate_topics().unwrap();
    assert_eq!(dup.pairs.len(), 1, "{:?}", dup.pairs);
    let ids = [&dup.pairs[0].a.topic_id, &dup.pairs[0].b.topic_id];
    assert!(ids.contains(&&first.topic_id) && ids.contains(&&again.topic_id));

    let similar = d.similar_topics(&first.topic_id, 3).unwrap();
    assert_eq!(similar[0].topic.topic_id, again.topic_id);

    d.index
        .dismiss_pair(&again.topic_id, &first.topic_id)
        .unwrap();
    assert!(d.duplicate_topics().unwrap().pairs.is_empty());
}
