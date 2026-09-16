//! The vault: a plain directory of Markdown files and the only source of truth. Every
//! write creates a new file or rewrites one owned by the caller, so file-sync tools rarely
//! see two devices touch the same file (spec D2).

use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result};
use crate::fsutil::{join_frontmatter, now_rfc3339, split_frontmatter, write_atomic};
use crate::model::{AnnotationMeta, NoteBody, NoteMeta, SCHEMA, TagAlias, TopicMeta};

pub const MANIFEST: &str = "distill.toml";
pub const NOTES_DIR: &str = "notes";
pub const TOPICS_DIR: &str = "topics";
pub const ALIASES_DIR: &str = "tag-aliases";
pub const ANNOTATIONS_DIR: &str = "annotations";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub vault_id: String,
    pub created: String,
}

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
    manifest: Manifest,
}

pub fn new_id() -> String {
    ulid::Ulid::generate().to_string()
}

impl Vault {
    pub fn open(root: &Path) -> Result<Self> {
        let path = root.join(MANIFEST);
        if !path.is_file() {
            return Err(Error::NotAVault {
                path: root.to_path_buf(),
                reason: format!("{MANIFEST} is missing"),
            });
        }
        let text = std::fs::read_to_string(&path).at(&path)?;
        let manifest: Manifest = toml::from_str(&text).map_err(|e| Error::NotAVault {
            path: root.to_path_buf(),
            reason: format!("{MANIFEST} is invalid: {e}"),
        })?;
        if manifest.format > SCHEMA {
            return Err(Error::NotAVault {
                path: root.to_path_buf(),
                reason: format!(
                    "vault format {} is newer than this distill (supports {SCHEMA}); upgrade distill",
                    manifest.format
                ),
            });
        }
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
        })
    }

    /// Opens an existing vault at `root`, or creates one. A non-empty directory without a
    /// manifest is refused unless `adopt` is set, so Distill never scatters files into an
    /// unrelated folder.
    pub fn open_or_create(root: &Path, adopt: bool) -> Result<(Self, bool)> {
        if root.join(MANIFEST).is_file() {
            return Ok((Self::open(root)?, false));
        }
        if root.exists() {
            let mut entries = std::fs::read_dir(root).at(root)?;
            if entries.next().is_some() && !adopt {
                return Err(Error::DirectoryNotEmpty {
                    path: root.to_path_buf(),
                });
            }
        }
        for dir in [NOTES_DIR, TOPICS_DIR, ALIASES_DIR, ANNOTATIONS_DIR] {
            std::fs::create_dir_all(root.join(dir)).at(root.join(dir))?;
        }
        let manifest = Manifest {
            format: SCHEMA,
            vault_id: new_id(),
            created: now_rfc3339(),
        };
        let text = toml::to_string_pretty(&manifest).map_err(|e| Error::InvalidFile {
            path: root.join(MANIFEST),
            reason: e.to_string(),
        })?;
        write_atomic(&root.join(MANIFEST), text.as_bytes())?;
        Ok((
            Self {
                root: root.to_path_buf(),
                manifest,
            },
            true,
        ))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn id(&self) -> &str {
        &self.manifest.vault_id
    }

    /// `notes/<yyyy>/<mm>/<id>-<slug>.md`, dated by the note's creation time.
    pub fn note_path(&self, id: &str, title: &str, created: &str) -> PathBuf {
        let (year, month) = (
            created.get(0..4).unwrap_or("0000"),
            created.get(5..7).unwrap_or("00"),
        );
        let slug = slugify(title);
        let name = if slug.is_empty() {
            format!("{id}.md")
        } else {
            format!("{id}-{slug}.md")
        };
        self.root.join(NOTES_DIR).join(year).join(month).join(name)
    }

    pub fn topic_path(&self, id: &str) -> PathBuf {
        self.root.join(TOPICS_DIR).join(format!("{id}.md"))
    }

    pub fn alias_path(&self, id: &str) -> PathBuf {
        self.root.join(ALIASES_DIR).join(format!("{id}.yml"))
    }

    pub fn annotation_path(&self, note_id: &str, id: &str) -> PathBuf {
        self.root
            .join(ANNOTATIONS_DIR)
            .join(note_id)
            .join(format!("{id}.md"))
    }

    pub fn write_note(&self, path: &Path, meta: &NoteMeta, body: &NoteBody) -> Result<()> {
        write_atomic(path, render_md(path, meta, &body.render())?.as_bytes())
    }

    pub fn write_topic(&self, meta: &TopicMeta, description: &str) -> Result<PathBuf> {
        let path = self.topic_path(&meta.id);
        write_atomic(&path, render_md(&path, meta, description)?.as_bytes())?;
        Ok(path)
    }

    pub fn write_alias(&self, alias: &TagAlias) -> Result<PathBuf> {
        let path = self.alias_path(&new_id());
        let yaml = to_yaml(&path, alias)?;
        write_atomic(&path, yaml.as_bytes())?;
        Ok(path)
    }

    pub fn write_annotation(&self, meta: &AnnotationMeta, text: &str) -> Result<PathBuf> {
        let path = self.annotation_path(&meta.note, &meta.id);
        write_atomic(&path, render_md(&path, meta, text)?.as_bytes())?;
        Ok(path)
    }
}

fn render_md<T: Serialize>(path: &Path, meta: &T, body: &str) -> Result<String> {
    Ok(join_frontmatter(&to_yaml(path, meta)?, body))
}

fn to_yaml<T: Serialize>(path: &Path, value: &T) -> Result<String> {
    yaml_serde::to_string(value).map_err(|e| Error::InvalidFile {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Parses a Markdown file with YAML frontmatter. This is the boundary where files edited
/// by hand or by other devices enter Distill, so every error names the file.
pub fn parse_md<T: DeserializeOwned>(path: &Path, text: &str) -> Result<(T, String)> {
    let invalid = |reason: String| Error::InvalidFile {
        path: path.to_path_buf(),
        reason,
    };
    let (yaml, body) =
        split_frontmatter(text).ok_or_else(|| invalid("missing `---` frontmatter".into()))?;
    let meta: T = yaml_serde::from_str(yaml).map_err(|e| invalid(format!("frontmatter: {e}")))?;
    Ok((meta, body.to_string()))
}

pub fn parse_yaml<T: DeserializeOwned>(path: &Path, text: &str) -> Result<T> {
    yaml_serde::from_str(text).map_err(|e| Error::InvalidFile {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })
}

/// Checks a parsed file's `schema` against what this build understands.
pub fn check_schema(path: &Path, schema: u32) -> Result<()> {
    if schema == 0 || schema > SCHEMA {
        return Err(Error::InvalidFile {
            path: path.to_path_buf(),
            reason: format!("schema {schema} is not supported (this distill reads 1..={SCHEMA})"),
        });
    }
    Ok(())
}

/// Lowercase ASCII words and CJK characters joined by `-`, at most 48 characters.
fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars() {
        if c.is_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.extend(c.to_lowercase());
        } else {
            dash = true;
        }
        if out.chars().count() >= 48 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(
            slugify("bun:sqlite 在 macOS 上加载扩展"),
            "bun-sqlite-在-macos-上加载扩展"
        );
        assert_eq!(slugify("  --  "), "");
    }

    #[test]
    fn create_refuses_non_empty_dir_without_adopt() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("other.txt"), "x").unwrap();
        assert!(matches!(
            Vault::open_or_create(dir.path(), false),
            Err(Error::DirectoryNotEmpty { .. })
        ));
        let (vault, created) = Vault::open_or_create(dir.path(), true).unwrap();
        assert!(created);
        let (again, created) = Vault::open_or_create(dir.path(), false).unwrap();
        assert!(!created);
        assert_eq!(vault.id(), again.id());
    }
}
