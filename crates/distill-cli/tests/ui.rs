#![allow(clippy::unwrap_used)] // Test helpers panic on setup failure by design.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

struct Server {
    dir: tempfile::TempDir,
    port: u16,
    child: Child,
    /// The one-time authorization link the server printed at start.
    link: String,
}

impl Server {
    fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let port = free_port();
        let vault = dir.path().join("vault");
        run(&dir, &["init", "--vault", vault.to_str().unwrap()]);
        run(&dir, &["config", "set", "ui.port", &port.to_string()]);
        let mut child = command(&dir, &["ui", "--no-open"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut out = BufReader::new(child.stdout.take().unwrap());
        let mut link = String::new();
        while !link.starts_with("http://") {
            link.clear();
            assert!(out.read_line(&mut link).unwrap() > 0, "server exited early");
        }
        Self {
            dir,
            port,
            child,
            link: link.trim().to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn host(&self) -> String {
        format!("distill.localhost:{}", self.port)
    }

    fn origin(&self) -> String {
        format!("http://{}", self.host())
    }

    /// Redeems the printed link and returns the session cookie it sets.
    fn login(&self) -> String {
        let path = self.link.split_once(&self.host()).unwrap().1;
        let res = agent()
            .get(self.url(path))
            .header("Host", self.host())
            .call()
            .unwrap();
        assert_eq!(res.status(), 303);
        let cookie = res.headers()["set-cookie"].to_str().unwrap();
        assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
        cookie.split(';').next().unwrap().to_string()
    }

    fn get(&self, path: &str, cookie: &str) -> (u16, Value) {
        let mut res = agent()
            .get(self.url(path))
            .header("Host", self.host())
            .header("Cookie", cookie)
            .call()
            .unwrap();
        let status = res.status().as_u16();
        (
            status,
            serde_json::from_str(&res.body_mut().read_to_string().unwrap()).unwrap(),
        )
    }

    fn post(&self, path: &str, cookie: &str, origin: &str, body: &Value) -> u16 {
        agent()
            .post(self.url(path))
            .header("Host", self.host())
            .header("Cookie", cookie)
            .header("Origin", origin)
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .unwrap()
            .status()
            .as_u16()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn command(dir: &tempfile::TempDir, args: &[&str]) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("distill"));
    cmd.env("DISTILL_HOME", dir.path().join("home"))
        .env("CODEX_HOME", dir.path().join("codex"))
        .env("CLAUDE_CONFIG_DIR", dir.path().join("claude"))
        .env_remove("DISTILL_VAULT")
        .env_remove("PORT")
        .args(args);
    cmd
}

fn run(dir: &tempfile::TempDir, args: &[&str]) -> String {
    let out = command(dir, args).output().unwrap();
    assert!(
        out.status.success(),
        "distill {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn save_note(dir: &tempfile::TempDir) -> Value {
    let request = json!({
        "title": "bun:sqlite 为什么加载不了扩展",
        "question": "为什么 bun:sqlite 加载不了 sqlite-vec？",
        "conclusion": "macOS 系统 SQLite 禁止加载扩展。",
        "topic": "new",
        "new_tags": ["sqlite"],
        "source": {
            "agent": "claude-code",
            "session_id": "133e9647-5436-4a71-9352-39b822dfb773",
            "cwd": "/Users/me/project"
        }
    });
    let mut child = command(dir, &["save", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(5)))
        .build()
        .into()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Sends a raw request so the test controls the Host header exactly.
fn raw_status(port: u16, host: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response.lines().next().unwrap().to_string()
}

#[test]
fn api_needs_a_loopback_host_and_the_session() {
    let server = Server::start();
    assert!(raw_status(server.port, &server.host(), "/api/health").contains("200"));
    assert!(raw_status(server.port, "evil.example:80", "/api/health").contains("403"));
    assert!(raw_status(server.port, &server.host(), "/api/stats").contains("401"));

    let (status, _) = server.get("/api/stats", "distill_session=guess");
    assert_eq!(status, 401);
}

#[test]
fn an_authorized_browser_reads_and_annotates() {
    let server = Server::start();
    let saved = save_note(&server.dir);
    let note_id = saved["note_id"].as_str().unwrap();
    let cookie = server.login();

    let (status, notes) = server.get("/api/notes?tag=sqlite", &cookie);
    assert_eq!(status, 200);
    assert_eq!(notes[0]["id"], note_id);

    let (status, detail) = server.get(&format!("/api/notes/{note_id}"), &cookie);
    assert_eq!(status, 200);
    assert_eq!(detail["ask_count"], 1);
    assert!(
        detail["reopen"]["command"]
            .as_str()
            .unwrap()
            .contains("claude --resume")
    );

    let path = format!("/api/notes/{note_id}/annotations");
    let body = json!({ "text": "我的理解" });
    assert_eq!(
        server.post(&path, &cookie, "http://localhost:3000", &body),
        403,
        "another local site must not write with the cookie"
    );
    assert_eq!(server.post(&path, &cookie, &server.origin(), &body), 200);
    let (_, detail) = server.get(&format!("/api/notes/{note_id}"), &cookie);
    assert_eq!(detail["note"]["annotations"][0]["body"], "我的理解");

    let (status, missing) = server.get("/api/notes/01NOPE", &cookie);
    assert_eq!(status, 404);
    assert!(missing["error"].as_str().unwrap().contains("01NOPE"));

    // The printed link works once.
    let path = server.link.split_once(&server.host()).unwrap().1;
    let again = agent()
        .get(server.url(path))
        .header("Host", server.host())
        .call()
        .unwrap();
    assert_eq!(again.status(), 403);
}

#[test]
fn a_second_ui_reuses_the_server_and_stop_ends_it() {
    let mut server = Server::start();
    let out = run(&server.dir, &["ui", "--no-open"]);
    assert!(out.contains("/auth?token="), "{out}");

    let out = run(&server.dir, &["ui", "stop"]);
    assert!(out.contains("Stopped"), "{out}");
    let status = server.child.wait().unwrap();
    assert!(status.success());
    let out = run(&server.dir, &["ui", "stop"]);
    assert!(out.contains("not running"), "{out}");
}
