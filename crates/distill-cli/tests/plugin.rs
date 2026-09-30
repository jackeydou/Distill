#![allow(clippy::unwrap_used)] // Test helpers panic on setup failure by design.

//! Guards the plugin: it is not Rust, so nothing else notices when a manifest points at a file
//! that moved. The tests check what agents install, the output of `scripts/build-plugins.sh`,
//! not the source tree.
#![cfg(unix)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

const AGENTS: [Agent; 2] = [
    Agent {
        name: "claude-code",
        manifest: ".claude-plugin/plugin.json",
        marketplace: ".claude-plugin/marketplace.json",
        root_var: "${CLAUDE_PLUGIN_ROOT}",
    },
    Agent {
        name: "codex",
        manifest: ".codex-plugin/plugin.json",
        marketplace: ".agents/plugins/marketplace.json",
        root_var: "${PLUGIN_ROOT}",
    },
];

struct Agent {
    name: &'static str,
    manifest: &'static str,
    marketplace: &'static str,
    root_var: &'static str,
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Files under `root`, as `/`-separated paths relative to it.
fn files(root: &Path) -> BTreeSet<String> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file() && e.file_name() != ".DS_Store")
        .map(|e| {
            let rel = e.path().strip_prefix(root).unwrap();
            rel.to_str().unwrap().to_string()
        })
        .collect()
}

/// Runs the build script into a fresh directory and returns it.
fn build(args: &[&str]) -> tempfile::TempDir {
    let out = tempfile::tempdir().unwrap();
    let status = Command::new(repo().join("scripts/build-plugins.sh"))
        .args(args)
        .env("DISTILL_PLUGIN_OUT", out.path())
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "build-plugins.sh {args:?} failed");
    out
}

#[test]
fn each_output_is_a_native_marketplace() {
    let out = build(&[]);
    let mut versions = BTreeSet::new();
    for agent in &AGENTS {
        let root = out.path().join(agent.name);
        let plugin = root.join("plugins/distill");

        let marketplace = json(&root.join(agent.marketplace));
        assert_eq!(marketplace["name"], "distill", "{}", agent.name);
        let source = &marketplace["plugins"][0]["source"];
        let source = source["path"].as_str().or(source.as_str()).unwrap();
        assert_eq!(source, "./plugins/distill", "{}", agent.name);

        let manifest = json(&plugin.join(agent.manifest));
        assert_eq!(manifest["name"], "distill", "{}", agent.name);
        versions.insert(manifest["version"].as_str().unwrap().to_string());

        let mcp = json(&plugin.join(".mcp.json"));
        assert_eq!(mcp["mcpServers"]["distill"]["args"][0], "mcp");

        let hooks = json(&plugin.join("hooks/hooks.json"));
        let command = hooks["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(
            command.contains(&format!("{}/bin/distill-launch", agent.root_var)),
            "{}: hook must use {}",
            agent.name,
            agent.root_var
        );
        assert!(command.ends_with("hook user-prompt-submit"));

        assert!(plugin.join("skills/distill/SKILL.md").is_file());
        use std::os::unix::fs::PermissionsExt;
        let launcher = plugin.join("bin/distill-launch");
        let mode = std::fs::metadata(&launcher).unwrap().permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "{}: distill-launch lost its executable bit",
            agent.name
        );
    }
    assert_eq!(versions.len(), 1, "plugin versions differ: {versions:?}");

    let codex = json(
        &out.path()
            .join("codex/plugins/distill/.codex-plugin/plugin.json"),
    );
    assert_eq!(codex["skills"], "./skills/");
    assert_eq!(codex["mcpServers"], "./.mcp.json");
}

#[test]
fn outputs_hold_only_their_agents_files() {
    let out = build(&[]);
    let src = repo().join("plugins/distill");
    let shared: BTreeSet<String> = files(&src)
        .into_iter()
        .filter(|f| {
            !AGENTS
                .iter()
                .any(|a| f.starts_with(&format!("{}/", a.name)))
        })
        .filter(|f| !["README.md", "CHANGELOG.md", "BUGFIX.md"].contains(&f.as_str()))
        .collect();
    for agent in &AGENTS {
        let mut expected = shared.clone();
        expected.extend(files(&src.join(agent.name)));
        expected.insert("bin/distill-version".into());
        assert_eq!(
            files(&out.path().join(agent.name).join("plugins/distill")),
            expected,
            "{}: plugin files differ from shared + plugins/distill/{}; update scripts/build-plugins.sh",
            agent.name,
            agent.name
        );
        let mut marketplace = files(&out.path().join(agent.name));
        marketplace.retain(|f| !f.starts_with("plugins/"));
        assert_eq!(
            marketplace,
            files(&repo().join("packaging").join(agent.name)),
            "{}",
            agent.name
        );
    }
}

#[test]
fn build_id_goes_into_the_version() {
    let out = build(&["--build", "abc1234", "codex"]);
    assert!(!out.path().join("claude-code").exists());
    let manifest = json(
        &out.path()
            .join("codex/plugins/distill/.codex-plugin/plugin.json"),
    );
    let source = json(&repo().join("plugins/distill/codex/.codex-plugin/plugin.json"));
    assert_eq!(
        manifest["version"].as_str().unwrap(),
        format!("{}+codex.abc1234", source["version"].as_str().unwrap())
    );
}

/// The `version` in `[workspace.package]`, the only unindented `version` line in Cargo.toml.
fn workspace_version() -> String {
    let manifest = std::fs::read_to_string(repo().join("Cargo.toml")).unwrap();
    manifest
        .lines()
        .find_map(|l| l.strip_prefix("version = \""))
        .and_then(|v| v.strip_suffix('"'))
        .unwrap()
        .to_string()
}

#[test]
fn launcher_pins_the_workspace_version() {
    let out = build(&["claude-code"]);
    let pinned = std::fs::read_to_string(
        out.path()
            .join("claude-code/plugins/distill/bin/distill-version"),
    )
    .unwrap();
    assert_eq!(pinned.trim(), workspace_version());
}

const TARGETS: [&str; 4] = [
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
];

/// A built plugin, a fake release served over `file://`, and an empty home, so the launcher
/// finds no installed `distill` and has to download one.
struct Launch {
    dir: tempfile::TempDir,
    plugin: tempfile::TempDir,
}

impl Launch {
    fn new() -> Option<Self> {
        if ["/opt/homebrew/bin/distill", "/usr/local/bin/distill"]
            .iter()
            .any(|p| Path::new(p).exists())
        {
            eprintln!("skipped: a system-wide distill would be found before any download");
            return None;
        }
        let launch = Self {
            dir: tempfile::tempdir().unwrap(),
            plugin: build(&["claude-code"]),
        };
        launch.release("#!/bin/sh\necho \"fake distill: $*\"\n");
        Some(launch)
    }

    fn releases(&self) -> PathBuf {
        self.dir.path().join("releases")
    }

    fn data(&self) -> PathBuf {
        self.dir.path().join("distill-home/data")
    }

    /// Publishes `script` as the `distill` binary of the pinned release, for every target.
    fn release(&self, script: &str) {
        let stage = self.dir.path().join("stage");
        std::fs::create_dir_all(&stage).unwrap();
        std::fs::write(stage.join("distill"), script).unwrap();
        let dest = self.releases().join(format!("v{}", workspace_version()));
        std::fs::create_dir_all(&dest).unwrap();
        for target in TARGETS {
            let asset = format!("distill-{target}.tar.gz");
            let status = Command::new("sh")
                .arg("-c")
                .arg(
                    "chmod +x \"$1/distill\" && tar -czf \"$2/$3\" -C \"$1\" distill && cd \"$2\" \
                     && { sha256sum \"$3\" 2>/dev/null || shasum -a 256 \"$3\"; } >\"$3.sha256\"",
                )
                .args([
                    "sh",
                    stage.to_str().unwrap(),
                    dest.to_str().unwrap(),
                    &asset,
                ])
                .status()
                .unwrap();
            assert!(status.success());
        }
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        let home = self.dir.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        Command::new(
            self.plugin
                .path()
                .join("claude-code/plugins/distill/bin/distill-launch"),
        )
        .args(args)
        .env_clear()
        .env("HOME", &home)
        .env("PATH", "/usr/bin:/bin")
        .env("DISTILL_HOME", self.dir.path().join("distill-home"))
        .env(
            "DISTILL_RELEASES_URL",
            format!("file://{}", self.releases().display()),
        )
        .stdin(Stdio::null())
        .output()
        .unwrap()
    }

    fn downloaded(&self) -> PathBuf {
        self.data()
            .join("bin")
            .join(workspace_version())
            .join("distill")
    }
}

fn stdout(out: &std::process::Output) -> String {
    String::from_utf8(out.stdout.clone()).unwrap()
}

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8(out.stderr.clone()).unwrap()
}

#[test]
fn launcher_downloads_the_pinned_release_once() {
    let Some(launch) = Launch::new() else { return };

    let out = launch.run(&["mcp"]);
    assert!(out.status.success(), "{}", stderr(&out));
    // stdout is the MCP transport: only the binary may write to it.
    assert_eq!(stdout(&out), "fake distill: mcp\n");
    assert!(stderr(&out).contains("downloading distill"));
    assert!(launch.downloaded().is_file());

    std::fs::remove_dir_all(launch.releases()).unwrap();
    let again = launch.run(&["stats"]);
    assert!(again.status.success(), "{}", stderr(&again));
    assert_eq!(stdout(&again), "fake distill: stats\n");
    assert!(!stderr(&again).contains("downloading"));
}

#[test]
fn launcher_hook_never_downloads() {
    let Some(launch) = Launch::new() else { return };
    let out = launch.run(&["hook", "user-prompt-submit"]);
    assert!(out.status.success());
    assert_eq!(stdout(&out), "");
    assert!(!launch.data().exists());
}

#[test]
fn launcher_refuses_a_bad_checksum() {
    let Some(launch) = Launch::new() else { return };
    let dest = launch.releases().join(format!("v{}", workspace_version()));
    for target in TARGETS {
        std::fs::write(
            dest.join(format!("distill-{target}.tar.gz.sha256")),
            format!("{}  distill-{target}.tar.gz\n", "0".repeat(64)),
        )
        .unwrap();
    }
    let out = launch.run(&["mcp"]);
    assert_eq!(out.status.code(), Some(127));
    assert!(
        stderr(&out).contains("checksum mismatch"),
        "{}",
        stderr(&out)
    );
    assert!(!launch.downloaded().exists());
}

#[test]
fn launcher_reports_a_missing_release() {
    let Some(launch) = Launch::new() else { return };
    std::fs::remove_dir_all(launch.releases()).unwrap();
    let out = launch.run(&["mcp"]);
    assert_eq!(out.status.code(), Some(127));
    let err = stderr(&out);
    assert!(
        err.contains(&format!("release v{} exists", workspace_version())),
        "{err}"
    );
}

/// `distill init` run by a downloaded binary records its path; after the plugin pins a newer
/// release, that older download must not shadow it.
#[test]
fn launcher_prefers_the_pinned_release_over_a_recorded_download() {
    let Some(launch) = Launch::new() else { return };
    let old = launch.data().join("bin/0.0.0/distill");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, "#!/bin/sh\necho old\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&old, std::fs::Permissions::from_mode(0o755)).unwrap();
    let config = launch.dir.path().join("distill-home/config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("config.toml"),
        format!("bin_path = \"{}\"\n", old.display()),
    )
    .unwrap();

    let out = launch.run(&["stats"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(stdout(&out), "fake distill: stats\n");
}
