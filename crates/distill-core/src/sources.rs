//! What Distill knows about the agent calling it: which agent and session a hook fired in,
//! whether a note's source really exists, and how to reopen that session (spec D5, D6, D12).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{Agent, Source};

/// The fields Distill reads from a hook's stdin. Claude Code and Codex send the same shape
/// for `UserPromptSubmit`; Codex adds `turn_id`.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct HookInput {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
}

/// Which agent fired the hook. `claudecode_env` is whether `CLAUDECODE=1` is set, which
/// Claude Code exports to its hooks and tools.
pub fn detect_agent(input: &HookInput, claudecode_env: bool) -> Option<Agent> {
    if input.turn_id.is_some() {
        return Some(Agent::Codex);
    }
    if claudecode_env {
        return Some(Agent::ClaudeCode);
    }
    let transcript = input.transcript_path.as_deref()?.replace('\\', "/");
    if transcript.contains("/.codex/") {
        Some(Agent::Codex)
    } else if transcript.contains("/.claude/") {
        Some(Agent::ClaudeCode)
    } else {
        None
    }
}

const SKILL_HINT: &str = "Distill is installed. If you have not read the distill skill in \
this session, read it now and follow its rules for when to recall past notes and when to offer \
to distill.";

const NOT_SET_UP: &str = "Distill is installed but not set up on this device. If the user asks \
to distill or recall, tell them to run `distill init`.";

/// The text the `UserPromptSubmit` hook injects. `suggest` is `None` when Distill has no
/// config on this device yet.
pub fn prompt_context(input: &HookInput, agent: Option<Agent>, suggest: Option<bool>) -> String {
    let mut lines = Vec::new();
    if let (Some(agent), Some(session), Some(cwd)) =
        (agent, input.session_id.as_deref(), input.cwd.as_deref())
    {
        lines.push(format!(
            "distill-source: {} {session} {cwd}",
            agent.as_str()
        ));
    }
    match suggest {
        Some(on) => {
            lines.push(format!(
                "distill-suggest: {}",
                if on { "on" } else { "off" }
            ));
            lines.push(SKILL_HINT.to_string());
        }
        None => lines.push(NOT_SET_UP.to_string()),
    }
    lines.join("\n")
}

/// Hook stdout for `UserPromptSubmit`, understood by both Claude Code and Codex.
pub fn hook_output(context: &str) -> serde_json::Value {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": context,
        }
    })
}

/// Where the agents keep their state. Read from the process environment in production and
/// set explicitly in tests.
#[derive(Debug, Clone, Default)]
pub struct SourceEnv {
    /// `$CODEX_HOME`, or `~/.codex`.
    pub codex_home: Option<PathBuf>,
    /// `$CLAUDE_CONFIG_DIR`, or `~/.claude`.
    pub claude_home: Option<PathBuf>,
    /// `CLAUDE_CODE_SESSION_ID` of the process, set when Claude Code started it.
    pub claude_session_id: Option<String>,
}

impl SourceEnv {
    pub fn from_process() -> Self {
        let home = |var: &str, dir: &str| {
            std::env::var_os(var)
                .map(PathBuf::from)
                .or_else(|| directories::BaseDirs::new().map(|b| b.home_dir().join(dir)))
        };
        Self {
            codex_home: home("CODEX_HOME", ".codex"),
            claude_home: home("CLAUDE_CONFIG_DIR", ".claude"),
            claude_session_id: std::env::var("CLAUDE_CODE_SESSION_ID").ok(),
        }
    }
}

pub const PLUGIN_NAME: &str = "distill";

#[derive(Debug, Clone, Serialize)]
pub struct InstalledPlugin {
    pub agent: Agent,
    /// `distill@<marketplace>`.
    pub id: String,
    pub version: Option<String>,
}

/// Distill plugins installed in Codex and Claude Code, plus problems reading their config.
pub fn installed_plugins(env: &SourceEnv) -> (Vec<InstalledPlugin>, Vec<String>) {
    let mut found = Vec::new();
    let mut problems = Vec::new();
    let prefix = format!("{PLUGIN_NAME}@");

    if let Some(path) = env
        .claude_home
        .as_ref()
        .map(|h| h.join("plugins/installed_plugins.json"))
        && path.is_file()
    {
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|e| e.to_string()))
        {
            Ok(json) => {
                let plugins = json["plugins"].as_object().into_iter().flatten();
                for (id, entries) in plugins.filter(|(id, _)| id.starts_with(&prefix)) {
                    found.push(InstalledPlugin {
                        agent: Agent::ClaudeCode,
                        id: id.clone(),
                        version: entries[0]["version"].as_str().map(str::to_string),
                    });
                }
            }
            Err(e) => problems.push(format!("could not read {}: {e}", path.display())),
        }
    }

    if let Some(home) = &env.codex_home
        && home.join("config.toml").is_file()
    {
        let path = home.join("config.toml");
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| t.parse::<toml::Table>().map_err(|e| e.to_string()))
        {
            Ok(table) => {
                let plugins = table.get("plugins").and_then(|p| p.as_table());
                for (id, entry) in plugins.into_iter().flatten() {
                    let enabled = entry
                        .get("enabled")
                        .and_then(|e| e.as_bool())
                        .unwrap_or(true);
                    if !id.starts_with(&prefix) || !enabled {
                        continue;
                    }
                    let marketplace = &id[prefix.len()..];
                    found.push(InstalledPlugin {
                        agent: Agent::Codex,
                        id: id.clone(),
                        version: newest_dir(
                            &home
                                .join("plugins/cache")
                                .join(marketplace)
                                .join(PLUGIN_NAME),
                        ),
                    });
                }
            }
            Err(e) => problems.push(format!("could not read {}: {e}", path.display())),
        }
    }
    (found, problems)
}

fn newest_dir(dir: &Path) -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.pop()
}

/// Checks that a note's source points at a real session, so a note never carries a
/// session id the agent made up. Returns warnings for checks that could not run.
pub fn verify(source: &Source, env: &SourceEnv) -> Result<Vec<String>> {
    source.validate()?;
    let mut warnings = Vec::new();
    match source.agent {
        Agent::Codex => {
            let dirs: Vec<PathBuf> = env
                .codex_home
                .iter()
                .flat_map(|h| [h.join("sessions"), h.join("archived_sessions")])
                .filter(|d| d.is_dir())
                .collect();
            if dirs.is_empty() {
                warnings.push(
                    "no Codex sessions directory on this device; session id not verified".into(),
                );
            } else if !dirs.iter().any(|d| has_rollout(d, &source.session_id)) {
                return Err(Error::InvalidSource {
                    reason: format!(
                        "no Codex session {} under {}",
                        source.session_id,
                        dirs[0].display()
                    ),
                });
            }
        }
        Agent::ClaudeCode => {
            if let Some(env_id) = &env.claude_session_id
                && env_id != &source.session_id
            {
                // After /clear the session changes but a running MCP server keeps its
                // environment, so the id from the hook wins.
                warnings.push(format!(
                    "session_id {} differs from CLAUDE_CODE_SESSION_ID {env_id}; kept the given id",
                    source.session_id
                ));
            }
        }
    }
    Ok(warnings)
}

fn has_rollout(dir: &Path, session_id: &str) -> bool {
    let suffix = format!("-{session_id}.jsonl");
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| e.file_name().to_string_lossy().ends_with(&suffix))
}

/// How to get back to the session a note came from: a desktop-app deep link and a
/// terminal command that works without the app. Neither URL scheme is documented by its
/// vendor; both were verified by hand on 2026-09-13 (spec D6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reopen {
    pub link: String,
    pub command: String,
}

pub fn reopen(source: &Source) -> Reopen {
    let id = &source.session_id;
    match source.agent {
        Agent::Codex => Reopen {
            link: format!("codex://threads/{id}"),
            command: format!("codex resume {id}"),
        },
        Agent::ClaudeCode => Reopen {
            link: format!("claude://resume?session={id}"),
            // Claude Code stores sessions per project directory.
            command: format!("cd {} && claude --resume {id}", shell_quote(&source.cwd)),
        },
    }
}

fn shell_quote(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-~".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "01a0d530-42ae-7731-8a1d-b4e07d9b837d";

    fn input(turn: bool, transcript: Option<&str>) -> HookInput {
        HookInput {
            session_id: Some(ID.into()),
            cwd: Some("/Users/me/my project".into()),
            transcript_path: transcript.map(str::to_string),
            turn_id: turn.then(|| "t1".to_string()),
        }
    }

    #[test]
    fn detects_agent() {
        assert_eq!(detect_agent(&input(true, None), false), Some(Agent::Codex));
        assert_eq!(
            detect_agent(&input(false, None), true),
            Some(Agent::ClaudeCode)
        );
        assert_eq!(
            detect_agent(
                &input(false, Some("/Users/me/.claude/projects/x/a.jsonl")),
                false
            ),
            Some(Agent::ClaudeCode)
        );
        assert_eq!(
            detect_agent(
                &input(false, Some(r"C:\Users\me\.codex\sessions\r.jsonl")),
                false
            ),
            Some(Agent::Codex)
        );
        assert_eq!(detect_agent(&input(false, None), false), None);
    }

    #[test]
    fn context_lines() {
        let text = prompt_context(&input(true, None), Some(Agent::Codex), Some(true));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            format!("distill-source: codex {ID} /Users/me/my project")
        );
        assert_eq!(lines[1], "distill-suggest: on");
        assert!(lines[2].contains("distill skill"));

        let off = prompt_context(&input(true, None), Some(Agent::Codex), Some(false));
        assert!(off.contains("distill-suggest: off"));

        let unknown_agent = prompt_context(&input(false, None), None, Some(true));
        assert!(!unknown_agent.contains("distill-source"));

        let not_set_up = prompt_context(&input(true, None), Some(Agent::Codex), None);
        assert!(not_set_up.contains("distill init"));
    }

    #[test]
    fn verifies_codex_rollout() {
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join("sessions/2026/09/13");
        std::fs::create_dir_all(&day).unwrap();
        std::fs::write(
            day.join(format!("rollout-2026-09-13T22-48-00-{ID}.jsonl")),
            "",
        )
        .unwrap();
        let env = SourceEnv {
            codex_home: Some(home.path().to_path_buf()),
            ..Default::default()
        };
        let mut source = Source {
            agent: Agent::Codex,
            session_id: ID.into(),
            cwd: "/tmp".into(),
            git_repo: None,
        };
        assert!(verify(&source, &env).unwrap().is_empty());
        source.session_id = "11111111-2222-3333-4444-555555555555".into();
        assert!(matches!(
            verify(&source, &env),
            Err(Error::InvalidSource { .. })
        ));

        let no_codex = SourceEnv::default();
        assert_eq!(verify(&source, &no_codex).unwrap().len(), 1);
    }

    #[test]
    fn finds_installed_plugins() {
        let codex = tempfile::tempdir().unwrap();
        let claude = tempfile::tempdir().unwrap();
        std::fs::write(
            codex.path().join("config.toml"),
            "[plugins.\"distill@distill\"]\nenabled = true\n[plugins.\"other@x\"]\nenabled = true\n",
        )
        .unwrap();
        std::fs::create_dir_all(codex.path().join("plugins/cache/distill/distill/0.0.1")).unwrap();
        std::fs::create_dir_all(claude.path().join("plugins")).unwrap();
        std::fs::write(
            claude.path().join("plugins/installed_plugins.json"),
            r#"{"version":2,"plugins":{"distill@distill":[{"version":"0.0.1"}],"x@y":[{}]}}"#,
        )
        .unwrap();
        let env = SourceEnv {
            codex_home: Some(codex.path().to_path_buf()),
            claude_home: Some(claude.path().to_path_buf()),
            claude_session_id: None,
        };
        let (found, problems) = installed_plugins(&env);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|p| p.version.as_deref() == Some("0.0.1")));
    }

    #[test]
    fn claude_mismatch_is_a_warning() {
        let source = Source {
            agent: Agent::ClaudeCode,
            session_id: ID.into(),
            cwd: "/tmp".into(),
            git_repo: None,
        };
        let env = SourceEnv {
            claude_session_id: Some("99999999-2222-3333-4444-555555555555".into()),
            ..Default::default()
        };
        assert_eq!(verify(&source, &env).unwrap().len(), 1);
    }

    #[test]
    fn reopen_links() {
        let mut source = Source {
            agent: Agent::Codex,
            session_id: ID.into(),
            cwd: "/Users/me/my project".into(),
            git_repo: None,
        };
        assert_eq!(reopen(&source).link, format!("codex://threads/{ID}"));
        source.agent = Agent::ClaudeCode;
        let r = reopen(&source);
        assert_eq!(r.link, format!("claude://resume?session={ID}"));
        assert_eq!(
            r.command,
            format!("cd '/Users/me/my project' && claude --resume {ID}")
        );
    }
}
