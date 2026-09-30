#![allow(clippy::unwrap_used)] // Test fixtures panic on setup failure.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

struct Dev {
    dir: tempfile::TempDir,
}

impl Dev {
    fn new() -> Self {
        let dev = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        dev.build();
        dev
    }

    fn output(&self) -> PathBuf {
        self.dir.path().join("output")
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("dev data's home")
    }

    fn plugin(&self, agent: &str) -> PathBuf {
        self.output().join(agent).join("plugins/distill-dev")
    }

    fn build(&self) {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let status = Command::new(repo.join("scripts/build-plugins.sh"))
            .arg("--dev")
            .env("DISTILL_PLUGIN_OUT", self.output())
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        for agent in ["codex", "claude-code"] {
            let bin = self.plugin(agent).join("bin");
            std::fs::write(bin.join("distill-dev-home"), self.home().to_str().unwrap()).unwrap();
            std::fs::write(
                bin.join("distill-dev-binary"),
                assert_cmd::cargo::cargo_bin("distill").to_str().unwrap(),
            )
            .unwrap();
        }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(self.plugin("codex").join("bin/distill-launch"));
        cmd.args(args)
            .env("DISTILL_HOME", self.dir.path().join("regular"))
            .env("DISTILL_VAULT", self.dir.path().join("regular-vault"))
            .env("DISTILL_BIN", "/does/not/exist")
            .env("PORT", "4777")
            .env_remove("CLAUDECODE");
        cmd
    }

    fn run(&self, args: &[&str]) -> Value {
        let out = self.command(args).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap()
    }
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn dev_marketplaces_have_their_own_identity_and_tools() {
    let dev = Dev::new();
    for (agent, manifest, marketplace) in [
        (
            "codex",
            ".codex-plugin/plugin.json",
            ".agents/plugins/marketplace.json",
        ),
        (
            "claude-code",
            ".claude-plugin/plugin.json",
            ".claude-plugin/marketplace.json",
        ),
    ] {
        let plugin = dev.plugin(agent);
        let manifest = read_json(plugin.join(manifest));
        assert_eq!(manifest["name"], "distill-dev");
        assert_eq!(manifest["interface"]["displayName"], "Distill dev");
        assert_eq!(manifest["version"], "0.0.1");
        let market = read_json(dev.output().join(agent).join(marketplace));
        assert_eq!(market["name"], "distill-dev");
        assert_eq!(market["plugins"][0]["name"], "distill-dev");
        let source = &market["plugins"][0]["source"];
        assert_eq!(
            source["path"].as_str().or(source.as_str()).unwrap(),
            "./plugins/distill-dev"
        );
        let mcp = read_json(plugin.join(".mcp.json"));
        assert!(mcp["mcpServers"].get("distill").is_none());
        assert_eq!(mcp["mcpServers"]["distill-dev"]["title"], "Distill dev");
        assert!(!plugin.join("bin/distill-version").exists());
        assert!(!plugin.join("skills/distill").exists());
        let skill = std::fs::read_to_string(plugin.join("skills/distill-dev/SKILL.md")).unwrap();
        assert!(skill.contains("name: distill-dev\n"));
        assert!(skill.contains("distill-dev-source:"));
        assert!(skill.contains("Use only tools from the `distill-dev` MCP server"));
        assert!(
            plugin
                .join("skills/distill-dev/references/note-format.md")
                .exists()
        );
    }
}

#[test]
fn dev_launcher_isolates_notes_and_keeps_them_across_rebuilds_and_cache_copies() {
    let dev = Dev::new();
    assert_eq!(dev.run(&["stats", "--json"])["notes"], 0);
    let request = json!({
        "title": "Dev note", "question": "Does the dev vault stay isolated?",
        "conclusion": "The dev launcher pins its own home and vault.",
        "topic": "new", "new_tags": ["debugging"],
        "source": { "agent": "codex", "session_id": "01a0d530-42ae-7731-8a1d-b4e07d9b837d", "cwd": "/test" }
    });
    let input = dev.dir.path().join("note.json");
    std::fs::write(&input, request.to_string()).unwrap();
    let saved = dev.run(&["save", "--json", "--input", input.to_str().unwrap()]);
    assert!(Path::new(saved["path"].as_str().unwrap()).starts_with(dev.home().join("vault")));
    let config = std::fs::read_to_string(dev.home().join("config/config.toml")).unwrap();
    assert!(config.contains("port = 4778"));
    assert!(dev.home().join("data/index").is_dir());
    assert!(!dev.dir.path().join("regular").exists());
    assert!(!dev.dir.path().join("regular-vault").exists());

    dev.build();
    assert_eq!(dev.run(&["stats", "--json"])["notes"], 1);
    let cached = dev.dir.path().join("agent cache's plugin");
    for entry in walkdir::WalkDir::new(dev.plugin("codex")) {
        let entry = entry.unwrap();
        let target = cached.join(entry.path().strip_prefix(dev.plugin("codex")).unwrap());
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(target).unwrap();
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
    let out = Command::new(cached.join("bin/distill-launch"))
        .args(["stats", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["notes"],
        1
    );
}

#[test]
fn dev_hook_uses_the_dev_skill_and_does_not_initialize() {
    let dev = Dev::new();
    let out = dev
        .command(&["hook", "user-prompt-submit"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    assert!(!dev.home().exists());

    dev.run(&["stats", "--json"]);
    let mut child = dev
        .command(&["hook", "user-prompt-submit"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"session_id":"test","turn_id":"test","cwd":"/test"}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    let context = output["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.contains("distill-dev-source: codex test /test"));
    assert!(context.contains("distill-dev-suggest: on"));
    assert!(context.contains("read the distill-dev skill"));
    assert!(context.contains("Use only the distill-dev MCP server"));
}

#[test]
fn missing_local_binary_never_falls_back_to_an_installed_release() {
    let dev = Dev::new();
    std::fs::write(
        dev.plugin("codex").join("bin/distill-dev-binary"),
        "/missing/dev/distill",
    )
    .unwrap();
    let out = dev.command(&["mcp"]).output().unwrap();
    assert_eq!(out.status.code(), Some(127));
    assert!(String::from_utf8_lossy(&out.stderr).contains("mise run build:plugins:dev"));
    assert!(out.stdout.is_empty());
    let hook = dev
        .command(&["hook", "user-prompt-submit"])
        .output()
        .unwrap();
    assert!(hook.status.success());
    assert!(hook.stdout.is_empty());
    assert!(!dev.home().exists());
}
