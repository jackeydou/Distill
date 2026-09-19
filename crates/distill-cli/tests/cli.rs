#![allow(clippy::unwrap_used)] // Test helpers panic on setup failure by design.

use assert_cmd::Command;
use serde_json::{Value, json};

struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        let env = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        let vault = env.dir.path().join("vault");
        env.ok(&["init", "--vault", vault.to_str().unwrap()]);
        env
    }

    fn cmd(&self, args: &[&str]) -> Command {
        let mut cmd = Command::cargo_bin("distill").unwrap();
        cmd.env("DISTILL_HOME", self.dir.path().join("home"))
            .env_remove("DISTILL_VAULT")
            .args(args);
        cmd
    }

    fn ok(&self, args: &[&str]) -> String {
        let out = self.cmd(args).output().unwrap();
        assert!(
            out.status.success(),
            "distill {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut args = args.to_vec();
        args.push("--json");
        serde_json::from_str(&self.ok(&args)).unwrap()
    }

    fn save(&self, request: &Value, extra: &[&str]) -> Value {
        let mut args = vec!["save", "--json"];
        args.extend_from_slice(extra);
        let out = self
            .cmd(&args)
            .write_stdin(request.to_string())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "save failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}

fn request(question: &str, tags: Value) -> Value {
    json!({
        "title": question,
        "question": question,
        "conclusion": "macOS 自带的 SQLite 禁止加载扩展，换成 rusqlite bundled。",
        "key_points": ["bun:sqlite 用系统 SQLite"],
        "topic": "new",
        "tags": tags["tags"],
        "new_tags": tags["new_tags"],
        "source": {
            "agent": "claude-code",
            "session_id": "133e9647-5436-4a71-9352-39b822dfb773",
            "cwd": "/Users/me/project"
        }
    })
}

/// P1 exit criterion: two notes on the same question show up as one topic asked twice.
#[test]
fn same_question_twice_is_counted_as_a_repeat() {
    let env = Env::new();
    let first = env.save(
        &request(
            "bun:sqlite 为什么加载不了扩展",
            json!({"tags": [], "new_tags": ["sqlite"]}),
        ),
        &[],
    );
    let topic = first["topic_id"].as_str().unwrap().to_string();

    let recall = env.json(&["recall", "bun", "加载扩展", "失败"]);
    assert_eq!(recall["topics"][0]["topic_id"], topic.as_str());

    let second = env.save(
        &request(
            "bun 里 sqlite-vec 加载失败",
            json!({"tags": ["sqlite"], "new_tags": []}),
        ),
        &["--topic", &topic],
    );
    assert_eq!(second["ask_count"], 2);

    let stats = env.json(&["stats"]);
    assert_eq!(stats["notes"], 2);
    assert_eq!(stats["top_topics"][0]["topic_id"], topic.as_str());
    assert_eq!(stats["top_topics"][0]["ask_count"], 2);

    let human = env.ok(&["stats"]);
    assert!(human.contains("2 note(s) in 1 topic(s)"), "{human}");
}

#[test]
fn search_tags_annotate_and_doctor() {
    let env = Env::new();
    let saved = env.save(
        &request(
            "WAL 模式下的并发写入",
            json!({"tags": [], "new_tags": ["sqlite"]}),
        ),
        &[],
    );
    let note_id = saved["note_id"].as_str().unwrap();

    let hits = env.json(&["search", "并发写入"]);
    assert_eq!(hits[0]["id"], note_id);
    let tags = env.json(&["tags"]);
    assert_eq!(tags[0]["tag"], "sqlite");

    env.ok(&["annotate", note_id, "一写多读，写之间要排队"]);
    let recall = env.json(&["recall", "并发写入"]);
    assert_eq!(
        recall["topics"][0]["notes"][0]["annotations"][0]["body"],
        "一写多读，写之间要排队"
    );

    let doctor = env.json(&["doctor"]);
    assert_eq!(doctor["ok"], true);
}

#[test]
fn save_errors_explain_what_to_do() {
    let env = Env::new();
    let out = env
        .cmd(&["save"])
        .write_stdin(request("q", json!({"tags": ["nope"], "new_tags": []})).to_string())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown tag \"nope\""), "{stderr}");
    assert!(stderr.contains("new_tags"), "{stderr}");
}

#[test]
fn config_round_trip() {
    let env = Env::new();
    env.ok(&["config", "set", "suggest.enabled", "false"]);
    assert_eq!(
        env.ok(&["config", "get", "suggest.enabled"]).trim(),
        "false"
    );
    let out = env
        .cmd(&["config", "set", "ui.port", "80"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn init_without_vault_needs_a_terminal_or_yes() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("distill")
        .unwrap()
        .env("DISTILL_HOME", dir.path().join("home"))
        .args(["init"])
        .write_stdin("")
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--vault"));
}

#[test]
fn commands_before_init_point_to_init() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("distill")
        .unwrap()
        .env("DISTILL_HOME", dir.path().join("home"))
        .args(["stats"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("distill init"));
}
