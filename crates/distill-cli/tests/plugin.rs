#![allow(clippy::unwrap_used)] // Test helpers panic on setup failure by design.

//! Guards the plugin files in `plugins/distill`: they are not Rust, so nothing else notices
//! when a manifest points at a file that moved.

use std::path::{Path, PathBuf};

use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(rel: &str) -> Value {
    let text = std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn manifests_agree_and_point_at_real_files() {
    let plugin = repo().join("plugins/distill");
    let claude = json("plugins/distill/.claude-plugin/plugin.json");
    let codex = json("plugins/distill/.codex-plugin/plugin.json");
    assert_eq!(claude["name"], "distill");
    assert_eq!(codex["name"], "distill");
    assert_eq!(
        claude["version"], codex["version"],
        "plugin versions differ"
    );

    let codex_mcp = codex["mcpServers"].as_str().unwrap();
    let mcp = json(&format!(
        "plugins/distill/{}",
        codex_mcp.trim_start_matches("./")
    ));
    assert_eq!(mcp["mcpServers"]["distill"]["args"][0], "mcp");
    assert!(
        plugin
            .join(codex["skills"].as_str().unwrap())
            .join("distill/SKILL.md")
            .is_file()
    );
    assert_eq!(claude["mcpServers"]["distill"]["args"][0], "mcp");

    let hooks = json("plugins/distill/hooks/hooks.json");
    let command = hooks["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(command.contains("${CLAUDE_PLUGIN_ROOT}/bin/distill-launch"));
    assert!(command.ends_with("hook user-prompt-submit"));
}

#[test]
fn launcher_is_executable() {
    let launcher = repo().join("plugins/distill/bin/distill-launch");
    assert!(launcher.is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&launcher).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "distill-launch lost its executable bit");
    }
}

#[test]
fn marketplaces_list_the_plugin() {
    let claude = json(".claude-plugin/marketplace.json");
    assert_eq!(claude["plugins"][0]["source"], "./plugins/distill");
    let codex = json(".agents/plugins/marketplace.json");
    assert_eq!(codex["plugins"][0]["source"]["path"], "./plugins/distill");
    assert_eq!(claude["name"], codex["name"]);
}
