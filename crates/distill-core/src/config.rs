//! Per-device configuration. Never synced: it lives outside the vault.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result};
use crate::fsutil::write_atomic;

pub const DEFAULT_UI_PORT: u16 = 4777;

/// Overrides both the config and data directories. Used by tests and by users who want
/// several independent Distill setups on one machine.
pub const HOME_ENV: &str = "DISTILL_HOME";
/// Overrides the vault recorded in the config file.
pub const VAULT_ENV: &str = "DISTILL_VAULT";

#[derive(Debug, Clone)]
pub struct Dirs {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl Dirs {
    pub fn discover() -> Result<Self> {
        if let Some(home) = std::env::var_os(HOME_ENV) {
            let home = PathBuf::from(home);
            return Ok(Self {
                config_dir: home.join("config"),
                data_dir: home.join("data"),
            });
        }
        let project = directories::ProjectDirs::from("", "", "Distill").ok_or_else(|| {
            Error::InvalidVaultLocation {
                path: PathBuf::new(),
                reason: format!("could not find a home directory; set {HOME_ENV}"),
            }
        })?;
        Ok(Self {
            config_dir: project.config_dir().to_path_buf(),
            data_dir: project.data_dir().to_path_buf(),
        })
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn index_file(&self, vault_id: &str) -> PathBuf {
        self.data_dir.join("index").join(format!("{vault_id}.db"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalConfig {
    pub vault: PathBuf,
    pub device_id: String,
    /// Where this binary lived when `distill init` ran. Plugin launchers read it to find
    /// `distill` when a desktop app's PATH does not include the install directory.
    pub bin_path: Option<PathBuf>,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub suggest: SuggestConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub port: u16,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_UI_PORT,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestConfig {
    pub enabled: bool,
}

impl Default for SuggestConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Keys `distill config get/set` accepts. `vault` is changed with `distill vault use/move`.
pub const SETTABLE_KEYS: &[&str] = &["ui.port", "suggest.enabled"];

impl LocalConfig {
    pub fn load(dirs: &Dirs) -> Result<Self> {
        let path = dirs.config_file();
        if !path.exists() {
            return Err(Error::NotInitialized { path });
        }
        let text = std::fs::read_to_string(&path).at(&path)?;
        toml::from_str(&text).map_err(|e| Error::InvalidFile {
            path,
            reason: e.to_string(),
        })
    }

    pub fn save(&self, dirs: &Dirs) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|e| Error::InvalidFile {
            path: dirs.config_file(),
            reason: e.to_string(),
        })?;
        write_atomic(&dirs.config_file(), text.as_bytes())
    }

    /// The vault to use: `DISTILL_VAULT` if set, otherwise the configured one.
    pub fn vault_path(&self) -> PathBuf {
        std::env::var_os(VAULT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| self.vault.clone())
    }

    pub fn get(&self, key: &str) -> Result<String> {
        match key {
            "vault" => Ok(self.vault_path().display().to_string()),
            "device_id" => Ok(self.device_id.clone()),
            "ui.port" => Ok(self.ui.port.to_string()),
            "suggest.enabled" => Ok(self.suggest.enabled.to_string()),
            _ => Err(unknown_key(key)),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let invalid = |reason: &str| Error::InvalidConfigValue {
            key: key.to_string(),
            value: value.to_string(),
            reason: reason.to_string(),
        };
        match key {
            "ui.port" => {
                let port: u16 = value
                    .parse()
                    .map_err(|_| invalid("expected a port number from 1024 to 65535"))?;
                if port < 1024 {
                    return Err(invalid("ports below 1024 need root; pick 1024 or higher"));
                }
                self.ui.port = port;
            }
            "suggest.enabled" => {
                self.suggest.enabled = value
                    .parse()
                    .map_err(|_| invalid("expected true or false"))?;
            }
            _ => return Err(unknown_key(key)),
        }
        Ok(())
    }

    pub fn note_url(&self, note_id: &str) -> String {
        format!("http://distill.localhost:{}/notes/{note_id}", self.ui.port)
    }
}

fn unknown_key(key: &str) -> Error {
    Error::UnknownConfigKey {
        key: key.to_string(),
        known: SETTABLE_KEYS.join(", "),
    }
}

/// Picks the UI port at init: the default when it is free, otherwise any free port.
/// The result is stored and never re-picked, so links stay valid.
pub fn pick_ui_port() -> u16 {
    let free = |port: u16| std::net::TcpListener::bind(("127.0.0.1", port)).is_ok();
    if free(DEFAULT_UI_PORT) {
        return DEFAULT_UI_PORT;
    }
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap_or(DEFAULT_UI_PORT)
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).at(path)
}
