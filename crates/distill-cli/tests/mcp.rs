#![allow(clippy::unwrap_used)] // Test helpers panic on setup failure by design.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

const SESSION: &str = "01a0d530-42ae-7731-8a1d-b4e07d9b837d";

struct Env {
    dir: tempfile::TempDir,
}

impl Env {
    /// A fresh vault plus a fake Codex home holding one rollout for `SESSION`.
    fn new() -> Self {
        let env = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        let day = env.dir.path().join("codex/sessions/2026/09/13");
        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(
            day.join(format!("rollout-2026-09-13T22-48-00-{SESSION}.jsonl")),
            "",
        )
        .unwrap();
        let vault = env.dir.path().join("vault");
        let status = env
            .command(&["init", "--vault", vault.to_str().unwrap()])
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        // A port of its own: saving starts the web UI in the background.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let status = env
            .command(&["config", "set", "ui.port", &port.to_string()])
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        env
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("distill"));
        cmd.env("DISTILL_HOME", self.dir.path().join("home"))
            .env("CODEX_HOME", self.dir.path().join("codex"))
            .env("CLAUDE_CONFIG_DIR", self.dir.path().join("claude"))
            .env_remove("DISTILL_VAULT")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env_remove("CLAUDECODE")
            .args(args);
        cmd
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = self
            .command(&["ui", "stop"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Mcp {
    fn start(env: &Env) -> Self {
        Self::start_args(env, &["mcp"])
    }

    fn start_args(env: &Env, args: &[&str]) -> Self {
        let mut child = env
            .command(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut mcp = Self {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        let init = mcp.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "0" }
            }),
        );
        assert_eq!(init["serverInfo"]["name"], "distill");
        mcp.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        mcp
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).unwrap() > 0,
                "server closed stdout"
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                assert!(message.get("error").is_none(), "protocol error: {message}");
                return message["result"].clone();
            }
        }
    }

    /// Calls a tool; returns (is_error, payload). The payload is parsed JSON on success
    /// and the error text otherwise.
    fn call(&mut self, name: &str, arguments: Value) -> (bool, Value) {
        let result = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let text = result["content"][0]["text"].as_str().unwrap().to_string();
        let is_error = result["isError"].as_bool().unwrap_or(false);
        if is_error {
            (true, Value::String(text))
        } else {
            (false, serde_json::from_str(&text).unwrap())
        }
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn save_args(topic: &str, session: &str) -> Value {
    json!({
        "title": "bun:sqlite 为什么加载不了扩展",
        "question": "bun:sqlite 在 macOS 上加载 sqlite-vec 报错",
        "conclusion": "macOS 自带 SQLite 关闭了扩展加载。",
        "topic": topic,
        "new_tags": ["sqlite"],
        "source": { "agent": "codex", "session_id": session, "cwd": "/Users/me/project" }
    })
}

#[test]
fn lists_the_four_tools() {
    let env = Env::new();
    let mut mcp = Mcp::start(&env);
    let tools = mcp.request("tools/list", json!({}));
    let mut names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "distill_recall",
            "distill_save",
            "distill_search",
            "distill_stats"
        ]
    );
}

#[test]
fn save_then_recall_reports_repeats_and_reopen_links() {
    let env = Env::new();
    let mut mcp = Mcp::start(&env);

    let (err, first) = mcp.call("distill_save", save_args("new", SESSION));
    assert!(!err, "{first}");
    assert_eq!(first["ask_count"], 1);
    assert_eq!(
        first["warnings"],
        json!([]),
        "the web UI should have started"
    );
    let url = first["url"].as_str().unwrap();
    let port = url.rsplit_once(':').unwrap().1.split('/').next().unwrap();
    let mut health = String::new();
    std::io::Read::read_to_string(
        &mut ureq::get(format!("http://127.0.0.1:{port}/api/health"))
            .call()
            .unwrap()
            .into_body()
            .into_reader(),
        &mut health,
    )
    .unwrap();
    assert!(health.contains("\"app\":\"distill\""), "{health}");

    let (_, recall) = mcp.call("distill_recall", json!({ "question": "bun 加载扩展失败" }));
    let topic = &recall["topics"][0];
    assert_eq!(topic["topic_id"], first["topic_id"]);
    assert_eq!(recall["tags"][0]["tag"], "sqlite");
    let note = &topic["notes"][0];
    assert_eq!(note["reopen"]["link"], format!("codex://threads/{SESSION}"));
    assert!(std::path::Path::new(note["file"].as_str().unwrap()).is_file());
    assert_eq!(note["url"], url);

    let mut again = save_args(first["topic_id"].as_str().unwrap(), SESSION);
    again["new_tags"] = json!([]);
    again["tags"] = json!(["sqlite"]);
    let (err, second) = mcp.call("distill_save", again);
    assert!(!err, "{second}");
    assert_eq!(second["ask_count"], 2);

    let (_, stats) = mcp.call("distill_stats", json!({}));
    assert_eq!(stats["repeated_topics"], 1);
}

#[test]
fn invalid_calls_come_back_as_tool_errors() {
    let env = Env::new();
    let mut mcp = Mcp::start(&env);

    let (err, text) = mcp.call(
        "distill_save",
        save_args("new", "11111111-2222-3333-4444-555555555555"),
    );
    assert!(err);
    assert!(
        text.as_str().unwrap().contains("no Codex session"),
        "{text}"
    );

    let mut unknown_tag = save_args("new", SESSION);
    unknown_tag["tags"] = json!(["nope"]);
    unknown_tag["new_tags"] = json!([]);
    let (err, text) = mcp.call("distill_save", unknown_tag);
    assert!(err);
    assert!(
        text.as_str().unwrap().contains("unknown tag \"nope\""),
        "{text}"
    );
}

fn hook(env: &Env, stdin: &str, home: Option<&str>) -> String {
    let mut cmd = env.command(&["hook", "user-prompt-submit"]);
    if let Some(home) = home {
        cmd.env("DISTILL_HOME", home);
    }
    hook_with(cmd, stdin)
}

fn hook_with(mut cmd: Command, stdin: &str) -> String {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "the hook must always exit 0");
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        json["hookSpecificOutput"]["hookEventName"],
        "UserPromptSubmit"
    );
    json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string()
}

/// A binary the plugin downloaded is not on PATH, so "run `distill init`" would not work.
#[cfg(unix)]
#[test]
fn hook_names_an_init_command_that_runs() {
    let env = Env::new();
    let bin = std::fs::canonicalize(assert_cmd::cargo::cargo_bin("distill")).unwrap();
    let not_set_up = |path: &std::path::Path| {
        let mut cmd = env.command(&["hook", "user-prompt-submit"]);
        cmd.env("DISTILL_HOME", "/nonexistent/distill")
            .env("PATH", path);
        hook_with(cmd, "{}")
    };

    let elsewhere = env.dir.path().join("empty");
    std::fs::create_dir(&elsewhere).unwrap();
    let text = not_set_up(&elsewhere);
    let expected = if bin.to_str().unwrap().contains(' ') {
        format!("`'{}' init --vault", bin.display())
    } else {
        format!("`{} init --vault", bin.display())
    };
    assert!(text.contains(&expected), "{text}");

    let on_path = env.dir.path().join("on-path");
    std::fs::create_dir(&on_path).unwrap();
    std::os::unix::fs::symlink(&bin, on_path.join("distill")).unwrap();
    assert!(not_set_up(&on_path).contains("`distill init --vault"));
}

#[test]
fn hook_injects_source_and_points_at_the_skill() {
    let env = Env::new();
    let codex_input = json!({
        "session_id": SESSION, "cwd": "/Users/me/project", "transcript_path": null,
        "turn_id": "t1", "prompt": "hi", "hook_event_name": "UserPromptSubmit",
        "model": "m", "permission_mode": "default"
    });
    let context = hook(&env, &codex_input.to_string(), None);
    assert!(context.starts_with(&format!(
        "distill-source: codex {SESSION} /Users/me/project\n"
    )));
    assert!(context.contains("distill-suggest: on"));
    assert!(context.contains("distill skill"));

    let not_set_up = hook(&env, &codex_input.to_string(), Some("/nonexistent/distill"));
    assert!(not_set_up.contains("distill init"));

    let garbage = hook(&env, "not json", None);
    assert!(!garbage.contains("distill-source"));
}

#[test]
fn ui_entrypoints_advertise_a_self_contained_app_and_icon() {
    let env = Env::new();
    let mut mcp = Mcp::start_args(&env, &["mcp", "--ui"]);
    let tools = mcp.request("tools/list", json!({}));
    let entry = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "distill_open_ui")
        .unwrap();
    assert_eq!(entry["title"], "Distill");
    assert_eq!(
        entry["_meta"]["openai/ui"]["entrypoints"],
        json!([{"type":"global"}, {"type":"thread"}])
    );
    assert_eq!(
        entry["_meta"]["ui"]["resourceUri"],
        "ui://distill/library.html"
    );
    assert!(
        entry["icons"][0]["src"]
            .as_str()
            .unwrap()
            .starts_with("data:image/svg+xml;base64,")
    );
    let (err, result) = mcp.call("distill_open_ui", json!({}));
    assert!(!err, "{result}");
    assert_eq!(result["status"], 200);
    assert_eq!(
        result["data"]["vault"],
        env.dir.path().join("vault").to_str().unwrap()
    );
    let list = mcp.request("resources/list", json!({}));
    let uri = list["resources"][0]["uri"].as_str().unwrap();
    let resource = mcp.request("resources/read", json!({"uri": uri}));
    let content = &resource["contents"][0];
    assert_eq!(content["mimeType"], "text/html;profile=mcp-app");
    assert_eq!(content["_meta"]["ui"]["csp"]["resourceDomains"], json!([]));
    assert_eq!(
        content["_meta"]["openai/ui"]["preferredDisplayMode"],
        "fullscreen"
    );
    let html = content["text"].as_str().unwrap();
    assert!(html.contains("data-transport=\"mcp\""));
    assert!(html.contains("data:font/"), "fonts must be embedded");
    assert!(!html.contains("src=\"/assets/"));
    for name in ["distill_ui_read", "distill_ui_write"] {
        let tool = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == name)
            .unwrap();
        assert_eq!(tool["_meta"]["ui"]["visibility"], json!(["app"]));
    }
}

#[test]
fn ui_bridge_edits_the_same_vault_and_excludes_server_control_routes() {
    let env = Env::new();
    let mut mcp = Mcp::start_args(&env, &["mcp", "--ui"]);
    let (_, saved) = mcp.call("distill_save", save_args("new", SESSION));
    let id = saved["note_id"].as_str().unwrap();
    let (_, result) = mcp.call("distill_ui_write", json!({"method":"POST", "path":format!("/notes/{id}/annotations"), "body":{"text":"Through MCP App"}}));
    assert_eq!(result["status"], 200, "{result}");
    let annotation = result["data"]["note"]["annotations"][0]["id"]
        .as_str()
        .unwrap();
    let (_, note) = mcp.call("distill_ui_read", json!({"path":format!("/notes/{id}")}));
    assert_eq!(
        note["data"]["note"]["annotations"][0]["body"],
        "Through MCP App"
    );
    let (_, deleted) = mcp.call(
        "distill_ui_write",
        json!({"method":"DELETE", "path":format!("/annotations/{annotation}")}),
    );
    assert_eq!(deleted["status"], 204);
    for path in ["/shutdown", "/auth", "/events"] {
        let (_, result) = mcp.call("distill_ui_read", json!({"path":path}));
        assert_eq!(result["status"], 404, "{path}: {result}");
    }
    for path in ["https://example.com/notes", "//example.com/notes", "notes"] {
        let (error, _) = mcp.call("distill_ui_read", json!({"path":path}));
        assert!(error, "{path} must not cross the API boundary");
    }
    let (_, result) = mcp.call("distill_ui_write", json!({"method":"POST","path":"/stats"}));
    assert_eq!(result["status"], 405);
}
