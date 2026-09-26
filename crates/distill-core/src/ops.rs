//! Operations shared by the CLI, the MCP server and the web server. Each validates its
//! input here, at the package boundary, and trusts its own types below.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{Dirs, LocalConfig, UiConfig, ensure_dir, pick_ui_port};
use crate::embed::Embedder;
use crate::error::{Error, IoContext, Result};
use crate::fsutil::{content_hash, now_rfc3339};
use crate::index::{
    Conflict, Index, InvalidFile, RecallResult, SimilarTopic, SyncReport, TopicPair,
};
use crate::model::{AnnotationMeta, NoteBody, NoteMeta, SCHEMA, Source, TopicMeta};
use crate::redact::redact;
use crate::sources::{InstalledPlugin, SourceEnv, installed_plugins};
use crate::tags::{MAX_TAGS_PER_NOTE, normalize, resolve_alias, similar_to, suggestions};
use crate::vault::{Vault, new_id};

pub const NEW_TOPIC: &str = "new";

/// An opened Distill setup: config, vault and an index already synced with the vault.
pub struct Distill {
    pub dirs: Dirs,
    pub config: LocalConfig,
    pub vault: Vault,
    pub index: Index,
}

impl Distill {
    pub fn open() -> Result<Self> {
        Self::open_in(Dirs::discover()?)
    }

    pub fn open_in(dirs: Dirs) -> Result<Self> {
        let config = LocalConfig::load(&dirs)?;
        let vault = Vault::open(&config.vault_path())?;
        let mut index = Index::open(&dirs.index_file(vault.id()), vault.root())?;
        index.sync()?;
        Ok(Self {
            dirs,
            config,
            vault,
            index,
        })
    }
}

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub vault: PathBuf,
    pub adopt: bool,
    pub bin_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InitReport {
    pub vault: PathBuf,
    pub vault_id: String,
    /// False when the directory already held a vault, e.g. synced from another device.
    pub created_vault: bool,
    pub config_file: PathBuf,
    pub index_file: PathBuf,
    pub device_id: String,
    pub ui_port: u16,
    pub indexed: SyncReport,
}

/// Creates or joins a vault and writes this device's config. Safe to run again: the
/// device id and UI port are kept, so links already handed out stay valid.
pub fn init(dirs: &Dirs, opts: InitOptions) -> Result<InitReport> {
    let vault_path = std::path::absolute(&opts.vault).at(&opts.vault)?;
    for own in [&dirs.config_dir, &dirs.data_dir] {
        let own = std::path::absolute(own).at(own)?;
        if vault_path.starts_with(&own) || own.starts_with(&vault_path) {
            return Err(Error::InvalidVaultLocation {
                path: vault_path,
                reason: format!(
                    "it overlaps Distill's own directory {}; the index must stay out of the vault",
                    own.display()
                ),
            });
        }
    }
    let (vault, created_vault) = Vault::open_or_create(&vault_path, opts.adopt)?;
    let existing = match LocalConfig::load(dirs) {
        Ok(config) => Some(config),
        Err(Error::NotInitialized { .. }) => None,
        Err(other) => return Err(other),
    };
    let config = LocalConfig {
        vault: vault_path.clone(),
        device_id: existing
            .as_ref()
            .map_or_else(new_id, |c| c.device_id.clone()),
        bin_path: opts.bin_path,
        ui: existing.as_ref().map_or_else(
            || UiConfig {
                port: pick_ui_port(),
                ..UiConfig::default()
            },
            |c| c.ui.clone(),
        ),
        suggest: existing.map(|c| c.suggest).unwrap_or_default(),
    };
    ensure_dir(&dirs.config_dir)?;
    config.save(dirs)?;
    let index_file = dirs.index_file(vault.id());
    let mut index = Index::open(&index_file, vault.root())?;
    let indexed = index.sync()?;
    Ok(InitReport {
        vault: vault_path,
        vault_id: vault.id().to_string(),
        created_vault,
        config_file: dirs.config_file(),
        index_file,
        device_id: config.device_id,
        ui_port: config.ui.port,
        indexed,
    })
}

/// Where `distill init` proposes the vault: iCloud Drive on macOS when it is enabled,
/// otherwise the platform's documents folder.
pub fn default_vault_path() -> Option<PathBuf> {
    if let Some(icloud) = icloud_drive().filter(|p| p.is_dir()) {
        return Some(icloud.join("Distill"));
    }
    let user = directories::UserDirs::new()?;
    let docs = user
        .document_dir()
        .map_or_else(|| user.home_dir().join("Documents"), Path::to_path_buf);
    Some(docs.join("Distill"))
}

fn icloud_drive() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let home = directories::BaseDirs::new()?.home_dir().to_path_buf();
    Some(home.join("Library/Mobile Documents/com~apple~CloudDocs"))
}

/// Expands `~/…` and, on macOS, `icloud:<path>` into a real path.
pub fn expand_vault_arg(arg: &str) -> Result<PathBuf> {
    if let Some(rest) = arg.strip_prefix("icloud:") {
        let drive = icloud_drive().ok_or_else(|| Error::InvalidVaultLocation {
            path: PathBuf::from(arg),
            reason: "the icloud: shorthand only works on macOS; pass a real path".into(),
        })?;
        return Ok(drive.join(rest.trim_start_matches('/')));
    }
    if let Some(rest) = arg.strip_prefix("~/")
        && let Some(base) = directories::BaseDirs::new()
    {
        return Ok(base.home_dir().join(rest));
    }
    Ok(PathBuf::from(arg))
}

/// One note to save. Field descriptions double as the MCP tool schema.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SaveRequest {
    /// One line naming the question, in the user's language.
    pub title: String,
    /// The underlying question the user wanted answered.
    pub question: String,
    /// The answer the conversation arrived at.
    pub conclusion: String,
    /// Short supporting points worth remembering.
    #[serde(default)]
    pub key_points: Vec<String>,
    /// What is still unclear or was left unanswered.
    #[serde(default)]
    pub open_questions: Vec<String>,
    /// The topic id from `distill_recall` when this is the same question asked before;
    /// otherwise `"new"`.
    pub topic: String,
    /// Existing tags only (from the tag list `distill_recall` returns).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Tags to create, only when no existing tag fits.
    #[serde(default)]
    pub new_tags: Vec<String>,
    /// Set after a "close to existing tag" error, when the new tag really is different.
    #[serde(default)]
    pub confirm_new: bool,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize)]
pub struct SaveResult {
    pub note_id: String,
    pub topic_id: String,
    pub topic_created: bool,
    /// Notes in the topic including this one: "the Nth time you asked this".
    pub ask_count: usize,
    pub tags: Vec<String>,
    pub tags_created: Vec<String>,
    pub path: PathBuf,
    pub url: String,
}

impl SaveRequest {
    fn body(&self) -> NoteBody {
        NoteBody {
            title: self.title.clone(),
            question: self.question.clone(),
            conclusion: self.conclusion.clone(),
            key_points: self.key_points.clone(),
            open_questions: self.open_questions.clone(),
        }
    }
}

impl Distill {
    pub fn save(&mut self, req: SaveRequest) -> Result<SaveResult> {
        req.source.validate()?;
        let body = req.body();
        body.validate()?;
        let (tags, tags_created) = self.check_tags(&req)?;
        let body = redact_body(body);

        let (topic_id, topic_created) = if req.topic == NEW_TOPIC {
            (new_id(), true)
        } else if self.index.topic_exists(&req.topic)? {
            (self.index.resolve_topic(&req.topic)?, false)
        } else {
            return Err(Error::UnknownTopic { id: req.topic });
        };

        let created = now_rfc3339();
        if topic_created {
            let topic = TopicMeta {
                schema: SCHEMA,
                id: topic_id.clone(),
                label: body.title.clone(),
                merged_into: None,
                created: created.clone(),
            };
            self.vault.write_topic(&topic, "")?;
        }
        let note_id = new_id();
        let meta = NoteMeta {
            schema: SCHEMA,
            id: note_id.clone(),
            topic: topic_id.clone(),
            tags: tags.clone(),
            source: req.source,
            created: created.clone(),
            updated: None,
        };
        let path = self.vault.note_path(&note_id, &body.title, &created);
        self.vault.write_note(&path, &meta, &body)?;
        self.index.sync()?;

        Ok(SaveResult {
            ask_count: self.index.ask_count(&topic_id)?,
            url: self.config.note_url(&note_id),
            note_id,
            topic_id,
            topic_created,
            tags,
            tags_created,
            path,
        })
    }

    /// Returns the note's final tags and the ones it creates. `tags` must already exist;
    /// `new_tags` must not be near-duplicates of existing ones unless confirmed.
    fn check_tags(&self, req: &SaveRequest) -> Result<(Vec<String>, Vec<String>)> {
        let known: BTreeSet<String> = self.index.known_tags()?.into_iter().collect();
        let aliases = self.index.aliases()?;
        let mut tags: Vec<String> = Vec::new();
        let mut created: Vec<String> = Vec::new();
        for raw in &req.tags {
            let tag = resolve_alias(&normalize(raw)?, &aliases);
            if !known.contains(&tag) {
                return Err(Error::UnknownTag {
                    tag: raw.clone(),
                    suggestions: suggestions(&tag, known.iter().map(String::as_str)),
                });
            }
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        for raw in &req.new_tags {
            let tag = resolve_alias(&normalize(raw)?, &aliases);
            if !known.contains(&tag) {
                let similar = similar_to(&tag, known.iter().map(String::as_str));
                if !similar.is_empty() && !req.confirm_new {
                    return Err(Error::SimilarTag { tag, similar });
                }
                if !created.contains(&tag) {
                    created.push(tag.clone());
                }
            }
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        if tags.is_empty() || tags.len() > MAX_TAGS_PER_NOTE {
            return Err(Error::TagCount {
                count: tags.len(),
                max: MAX_TAGS_PER_NOTE,
            });
        }
        Ok((tags, created))
    }

    pub fn annotate(&mut self, note_id: &str, text: &str) -> Result<PathBuf> {
        if self.index.note(note_id)?.is_none() {
            return Err(Error::UnknownNote {
                id: note_id.to_string(),
            });
        }
        if text.trim().is_empty() {
            return Err(Error::InvalidNote {
                reason: "annotation text is empty".into(),
            });
        }
        let meta = AnnotationMeta {
            schema: SCHEMA,
            id: new_id(),
            note: note_id.to_string(),
            anchor: None,
            created: now_rfc3339(),
            updated: None,
        };
        let path = self
            .vault
            .write_annotation(&meta, &format!("{}\n", redact(text.trim())))?;
        self.index.sync()?;
        Ok(path)
    }

    /// Copies the vault to `dest`, verifies every file, then points this device at the
    /// copy. The original stays untouched as a recovery copy.
    pub fn move_vault(&mut self, dest: &Path) -> Result<usize> {
        let dest = std::path::absolute(dest).at(dest)?;
        if dest.exists() && std::fs::read_dir(&dest).at(&dest)?.next().is_some() {
            return Err(Error::DirectoryNotEmpty { path: dest });
        }
        let src = self.vault.root().to_path_buf();
        let mut copied = 0;
        for entry in walkdir::WalkDir::new(&src) {
            let entry = entry.map_err(|e| Error::Io {
                path: e.path().unwrap_or(&src).to_path_buf(),
                source: e.into(),
            })?;
            let Ok(rel) = entry.path().strip_prefix(&src) else {
                continue;
            };
            let target = dest.join(rel);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&target).at(&target)?;
            } else if entry.file_type().is_file() {
                let bytes = std::fs::read(entry.path()).at(entry.path())?;
                std::fs::write(&target, &bytes).at(&target)?;
                let written = std::fs::read(&target).at(&target)?;
                if content_hash(&written) != content_hash(&bytes) {
                    return Err(Error::InvalidFile {
                        path: target,
                        reason: "copy does not match the original; the vault was not moved".into(),
                    });
                }
                copied += 1;
            }
        }
        self.use_vault(&dest)?;
        Ok(copied)
    }

    /// Points this device at another existing vault.
    pub fn use_vault(&mut self, path: &Path) -> Result<()> {
        let path = std::path::absolute(path).at(path)?;
        let vault = Vault::open(&path)?;
        self.config.vault = path;
        self.config.save(&self.dirs)?;
        let mut index = Index::open(&self.dirs.index_file(vault.id()), vault.root())?;
        index.sync()?;
        self.vault = vault;
        self.index = index;
        Ok(())
    }

    pub fn doctor(&self) -> Result<DoctorReport> {
        let conflicts = self.index.conflicts()?;
        let invalid_files = self.index.invalid_files()?;
        let mut problems = Vec::new();
        if let Some(bin) = &self.config.bin_path
            && !bin.exists()
        {
            problems.push(format!(
                "recorded binary {} no longer exists; run `distill init` again to update it",
                bin.display()
            ));
        }
        let (plugins, plugin_problems) = installed_plugins(&SourceEnv::from_process());
        problems.extend(plugin_problems);
        if plugins.is_empty() {
            problems.push(
                "the Distill plugin is not installed in Codex or Claude Code; see INSTALL.md"
                    .into(),
            );
        }
        if !conflicts.is_empty() {
            problems.push(format!(
                "{} sync conflict(s): the same id appears in several files",
                conflicts.len()
            ));
        }
        if !invalid_files.is_empty() {
            problems.push(format!(
                "{} file(s) could not be read and are left out of the index",
                invalid_files.len()
            ));
        }
        Ok(DoctorReport {
            ok: problems.is_empty(),
            problems,
            config_file: self.dirs.config_file(),
            vault: self.vault.root().to_path_buf(),
            vault_id: self.vault.id().to_string(),
            index_file: self.dirs.index_file(self.vault.id()),
            bin_path: self.config.bin_path.clone(),
            ui_port: self.config.ui.port,
            embedding_model: Embedder::installed(&self.dirs),
            plugins,
            conflicts,
            invalid_files,
        })
    }
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[ts(export)]
pub struct Duplicates {
    /// False until `distill model pull` has run; `pairs` is then empty.
    pub model_installed: bool,
    pub pairs: Vec<TopicPair>,
}

/// Recall and topic similarity, with embeddings when the model is installed.
impl Distill {
    pub fn recall(&self, question: &str, limit: usize) -> Result<RecallResult> {
        match Embedder::load(&self.dirs)? {
            Some(embedder) => self.index.recall_semantic(question, limit, &embedder),
            None => self.index.recall(question, limit),
        }
    }

    pub fn duplicate_topics(&self) -> Result<Duplicates> {
        Ok(match Embedder::load(&self.dirs)? {
            Some(embedder) => Duplicates {
                model_installed: true,
                pairs: self.index.duplicate_topics(&embedder)?,
            },
            None => Duplicates {
                model_installed: false,
                pairs: Vec::new(),
            },
        })
    }

    /// Topics close to `topic_id`; empty without the model.
    pub fn similar_topics(&self, topic_id: &str, limit: usize) -> Result<Vec<SimilarTopic>> {
        match Embedder::load(&self.dirs)? {
            Some(embedder) => self.index.similar_topics(topic_id, limit, &embedder),
            None => Ok(Vec::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub problems: Vec<String>,
    pub config_file: PathBuf,
    pub vault: PathBuf,
    pub vault_id: String,
    pub index_file: PathBuf,
    pub bin_path: Option<PathBuf>,
    pub ui_port: u16,
    /// Whether the embedding model is installed. Without it recall uses keywords only;
    /// that is not counted as a problem.
    pub embedding_model: bool,
    pub plugins: Vec<InstalledPlugin>,
    pub conflicts: Vec<Conflict>,
    pub invalid_files: Vec<InvalidFile>,
}

fn redact_body(body: NoteBody) -> NoteBody {
    NoteBody {
        title: redact(&body.title),
        question: redact(&body.question),
        conclusion: redact(&body.conclusion),
        key_points: body.key_points.iter().map(|s| redact(s)).collect(),
        open_questions: body.open_questions.iter().map(|s| redact(s)).collect(),
    }
}
