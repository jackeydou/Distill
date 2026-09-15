//! Vault file formats. These types are the on-disk contract: changing a field is a
//! format change and must bump `SCHEMA` while still reading older files.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    Codex,
    ClaudeCode,
}

impl Agent {
    pub fn as_str(self) -> &'static str {
        match self {
            Agent::Codex => "codex",
            Agent::ClaudeCode => "claude-code",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "codex" => Ok(Agent::Codex),
            "claude-code" => Ok(Agent::ClaudeCode),
            other => Err(Error::InvalidSource {
                reason: format!("agent must be \"codex\" or \"claude-code\", got \"{other}\""),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub agent: Agent,
    pub session_id: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_repo: Option<String>,
}

impl Source {
    pub fn validate(&self) -> Result<()> {
        if !is_uuid(&self.session_id) {
            return Err(Error::InvalidSource {
                reason: format!("session_id \"{}\" is not a UUID", self.session_id),
            });
        }
        if self.cwd.trim().is_empty() {
            return Err(Error::InvalidSource {
                reason: "cwd is empty".into(),
            });
        }
        Ok(())
    }
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteMeta {
    pub schema: u32,
    pub id: String,
    pub topic: String,
    pub tags: Vec<String>,
    pub source: Source,
    pub created: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
}

/// The prose of a note. Rendered into fixed Markdown sections so the file stays readable
/// and editable in any editor.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteBody {
    pub title: String,
    pub question: String,
    pub conclusion: String,
    #[serde(default)]
    pub key_points: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<String>,
}

const H_QUESTION: &str = "问题";
const H_CONCLUSION: &str = "结论";
const H_KEY_POINTS: &str = "要点";
const H_OPEN_QUESTIONS: &str = "仍不清楚";

impl NoteBody {
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("title", &self.title),
            ("question", &self.question),
            ("conclusion", &self.conclusion),
        ] {
            if value.trim().is_empty() {
                return Err(Error::InvalidNote {
                    reason: format!("{name} is empty"),
                });
            }
        }
        if self.title.contains('\n') {
            return Err(Error::InvalidNote {
                reason: "title must be a single line".into(),
            });
        }
        Ok(())
    }

    pub fn render(&self) -> String {
        let mut out = format!("# {}\n", self.title.trim());
        section(&mut out, H_QUESTION, self.question.trim());
        section(&mut out, H_CONCLUSION, self.conclusion.trim());
        if !self.key_points.is_empty() {
            section(&mut out, H_KEY_POINTS, &bullets(&self.key_points));
        }
        if !self.open_questions.is_empty() {
            section(&mut out, H_OPEN_QUESTIONS, &bullets(&self.open_questions));
        }
        out
    }

    /// Reads the sections back. Unknown sections are ignored here; the full text is
    /// still indexed for search.
    pub fn parse(markdown: &str) -> Self {
        let mut body = NoteBody::default();
        let mut current: Option<&str> = None;
        let mut buf = String::new();
        let flush = |heading: Option<&str>, buf: &mut String, body: &mut NoteBody| {
            let text = buf.trim().to_string();
            match heading {
                Some(H_QUESTION) => body.question = text,
                Some(H_CONCLUSION) => body.conclusion = text,
                Some(H_KEY_POINTS) => body.key_points = parse_bullets(&text),
                Some(H_OPEN_QUESTIONS) => body.open_questions = parse_bullets(&text),
                _ => {}
            }
            buf.clear();
        };
        for line in markdown.lines() {
            if let Some(title) = line.strip_prefix("# ") {
                if body.title.is_empty() {
                    body.title = title.trim().to_string();
                }
                continue;
            }
            if let Some(heading) = line.strip_prefix("## ") {
                flush(current, &mut buf, &mut body);
                current = Some(match heading.trim() {
                    H_QUESTION => H_QUESTION,
                    H_CONCLUSION => H_CONCLUSION,
                    H_KEY_POINTS => H_KEY_POINTS,
                    H_OPEN_QUESTIONS => H_OPEN_QUESTIONS,
                    _ => "",
                });
                continue;
            }
            buf.push_str(line);
            buf.push('\n');
        }
        flush(current, &mut buf, &mut body);
        body
    }
}

fn section(out: &mut String, heading: &str, text: &str) {
    out.push_str(&format!("\n## {heading}\n\n{text}\n"));
}

fn bullets(items: &[String]) -> String {
    items
        .iter()
        .map(|i| format!("- {}", i.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_bullets(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.trim_start().strip_prefix("- "))
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMeta {
    pub schema: u32,
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_into: Option<String>,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagAlias {
    pub schema: u32,
    pub from: String,
    pub to: String,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationMeta {
    pub schema: u32,
    pub id: String,
    pub note: String,
    /// Reserved for annotations on a passage. Not written in v1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<serde_json::Value>,
    pub created: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_body_round_trip() {
        let body = NoteBody {
            title: "bun:sqlite 在 macOS 上加载扩展".into(),
            question: "为什么加载不了 sqlite-vec？".into(),
            conclusion: "系统 SQLite 禁止加载扩展。".into(),
            key_points: vec!["用自带的 libsqlite3".into(), "或换 rusqlite".into()],
            open_questions: vec!["Linux 上是否一样".into()],
        };
        assert_eq!(NoteBody::parse(&body.render()), body);
    }

    #[test]
    fn note_body_without_optional_sections() {
        let body = NoteBody {
            title: "t".into(),
            question: "q".into(),
            conclusion: "c".into(),
            ..Default::default()
        };
        assert_eq!(NoteBody::parse(&body.render()), body);
    }

    #[test]
    fn source_requires_uuid_session() {
        let mut source = Source {
            agent: Agent::Codex,
            session_id: "01a0d530-42ae-7731-8a1d-b4e07d9b837d".into(),
            cwd: "/tmp".into(),
            git_repo: None,
        };
        assert!(source.validate().is_ok());
        source.session_id = "not-a-uuid".into();
        assert!(matches!(
            source.validate(),
            Err(Error::InvalidSource { .. })
        ));
    }
}
