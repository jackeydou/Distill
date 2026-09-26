use std::path::PathBuf;

/// Errors a caller can act on. Variants that an agent may hit through `distill_save`
/// carry enough detail to retry without asking the user.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "no Distill config at {path}. Run `distill init --vault <dir>` to create a vault on this device."
    )]
    NotInitialized { path: PathBuf },

    #[error("{path} is not a Distill vault: {reason}")]
    NotAVault { path: PathBuf, reason: String },

    #[error(
        "{path} is not empty and has no distill.toml. Pick an empty directory, or pass --adopt to create a vault inside it."
    )]
    DirectoryNotEmpty { path: PathBuf },

    #[error("invalid vault location {path}: {reason}")]
    InvalidVaultLocation { path: PathBuf, reason: String },

    #[error("unknown tag \"{tag}\". Use an existing tag, or move it to new_tags.{}", suggest(.suggestions))]
    UnknownTag {
        tag: String,
        suggestions: Vec<String>,
    },

    #[error(
        "new tag \"{tag}\" is close to existing tag(s) {}. Use the existing tag, or resend with confirm_new: true.",
        quoted(.similar)
    )]
    SimilarTag { tag: String, similar: Vec<String> },

    #[error("tag \"{tag}\" is empty after normalization. Tags need at least one letter or digit.")]
    EmptyTag { tag: String },

    #[error("a note needs 1 to {max} tags, got {count}.")]
    TagCount { count: usize, max: usize },

    #[error("unknown topic {id}. Pass an id returned by recall, or \"new\".")]
    UnknownTopic { id: String },

    #[error(
        "invalid source: {reason}. The Distill hook injects a `distill-source:` line; copy it as-is. If it is missing, run `distill doctor`."
    )]
    InvalidSource { reason: String },

    #[error("invalid note: {reason}")]
    InvalidNote { reason: String },

    #[error("unknown note {id}.")]
    UnknownNote { id: String },

    #[error("unknown annotation {id}.")]
    UnknownAnnotation { id: String },

    #[error("invalid topic {id}: {reason}")]
    InvalidTopic { id: String, reason: String },

    #[error("cannot merge topic {from} into {into}: {reason}")]
    InvalidMerge {
        from: String,
        into: String,
        reason: String,
    },

    #[error(
        "no sync conflict for {kind} {id}. Reload the list; another device may have settled it."
    )]
    UnknownConflict { kind: String, id: String },

    #[error("{path} is not one of the conflicting copies of {kind} {id}: {}", .paths.join(", "))]
    NotAConflictCopy {
        kind: String,
        id: String,
        path: String,
        paths: Vec<String>,
    },

    #[error("unknown config key \"{key}\". Known keys: {known}.")]
    UnknownConfigKey { key: String, known: String },

    #[error("invalid value \"{value}\" for config key \"{key}\": {reason}")]
    InvalidConfigValue {
        key: String,
        value: String,
        reason: String,
    },

    #[error("could not read {path}: {reason}")]
    InvalidFile { path: PathBuf, reason: String },

    #[error(
        "embedding model failed while {action}. Run `distill model pull` to download it again."
    )]
    Embedding {
        action: String,
        #[source]
        source: fastembed::Error,
    },

    #[error("could not register the sqlite-vec extension: {reason}")]
    VectorExtension { reason: String },

    #[error("I/O error at {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("index database error")]
    Sqlite(#[from] rusqlite::Error),

    #[error("index migration failed")]
    Migration(#[from] rusqlite_migration::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) trait IoContext<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T> {
        self.map_err(|source| Error::Io {
            path: path.into(),
            source,
        })
    }
}

fn quoted(items: &[String]) -> String {
    items
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

fn suggest(items: &[String]) -> String {
    if items.is_empty() {
        String::new()
    } else {
        format!(" Similar existing tags: {}.", quoted(items))
    }
}
