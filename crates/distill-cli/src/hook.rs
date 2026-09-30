//! `distill hook user-prompt-submit`: runs on every prompt in Codex and Claude Code.
//!
//! It must never get in the way of the user's prompt, so it always exits 0 and treats a
//! missing or unreadable config as "not set up". It reads only stdin and the config file;
//! it never opens the vault or the index.

use std::io::Read;

use distill_core::config::{Dirs, LocalConfig};
use distill_core::sources::{HookInput, detect_agent, hook_output, prompt_context};

pub fn user_prompt_submit() {
    let mut stdin = String::new();
    let input: HookInput = std::io::stdin()
        .read_to_string(&mut stdin)
        .ok()
        .and_then(|_| serde_json::from_str(&stdin).ok())
        .unwrap_or_default();
    let claudecode = std::env::var("CLAUDECODE").is_ok_and(|v| v == "1");
    let agent = detect_agent(&input, claudecode);
    let suggest = Dirs::discover()
        .ok()
        .and_then(|dirs| LocalConfig::load(&dirs).ok())
        .map(|config| config.suggest.enabled);
    let context = prompt_context(&input, agent, suggest, &shell_command());
    println!("{}", hook_output(&context));
}

/// `distill` when that name on PATH is this binary, otherwise this binary's path quoted for a
/// POSIX shell. Falls back to `distill` when the path is unknown; the hook must not fail.
fn shell_command() -> String {
    let Some(exe) = std::env::current_exe().and_then(std::fs::canonicalize).ok() else {
        return "distill".into();
    };
    let on_path = std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path)
            .any(|dir| std::fs::canonicalize(dir.join("distill")).is_ok_and(|p| p == exe))
    });
    if on_path {
        return "distill".into();
    }
    let exe = exe.to_string_lossy();
    if exe
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c))
    {
        exe.into_owned()
    } else {
        format!("'{}'", exe.replace('\'', r"'\''"))
    }
}
