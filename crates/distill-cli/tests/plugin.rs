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
        .filter(|f| f != "README.md" && f != "CHANGELOG.md")
        .collect();
    for agent in &AGENTS {
        let mut expected = shared.clone();
        expected.extend(files(&src.join(agent.name)));
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
