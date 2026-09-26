//! Setup shared by the integration tests: a fresh config, data directory and vault.
#![allow(dead_code, reason = "each test binary uses a different subset")]

use distill_core::Distill;
use distill_core::config::Dirs;
use distill_core::model::{Agent, Source};
use distill_core::ops::{InitOptions, SaveRequest, init};

pub struct Setup {
    pub dir: tempfile::TempDir,
    pub dirs: Dirs,
}

pub fn setup() -> Setup {
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

pub fn open(s: &Setup) -> Distill {
    Distill::open_in(s.dirs.clone()).unwrap()
}

pub fn request(topic: &str, tags: &[&str], new_tags: &[&str]) -> SaveRequest {
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
