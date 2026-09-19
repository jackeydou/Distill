mod render;

use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use distill_core::Distill;
use distill_core::config::{Dirs, SETTABLE_KEYS};
use distill_core::ops::{InitOptions, SaveRequest, default_vault_path, expand_vault_arg, init};
use serde::Serialize;

#[derive(Parser)]
#[command(
    name = "distill",
    version,
    about = "Keep what you learn from AI agents, and notice when you ask again."
)]
struct Cli {
    /// Print machine-readable JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a vault (or join one synced from another device) and set up this device.
    Init {
        /// Vault directory. Accepts ~/… and, on macOS, icloud:<path>.
        #[arg(long)]
        vault: Option<String>,
        /// Create the vault inside a non-empty directory.
        #[arg(long)]
        adopt: bool,
        /// Accept the proposed vault location without asking.
        #[arg(long, short)]
        yes: bool,
    },
    /// Save a note. Reads the save request as JSON from stdin or --input.
    Save {
        #[arg(long)]
        input: Option<PathBuf>,
        /// Override the request's topic: an existing topic id, or "new".
        #[arg(long)]
        topic: Option<String>,
    },
    /// Find topics you asked about before that resemble a question.
    Recall {
        #[arg(required = true, num_args = 1..)]
        question: Vec<String>,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// Search notes by keyword. With no query, lists recent notes.
    Search {
        query: Vec<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Counts by topic, tag, week and project.
    Stats,
    /// Tags in use and how many notes carry each.
    Tags,
    /// Add an annotation (your own understanding) to a note.
    Annotate {
        note_id: String,
        #[arg(required = true, num_args = 1..)]
        text: Vec<String>,
    },
    /// Rebuild this device's index from the vault.
    Reindex,
    /// Show, switch or move the vault.
    #[command(subcommand)]
    Vault(VaultCommand),
    /// Read or change this device's settings.
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Check the vault, index and setup, and list what needs attention.
    Doctor,
}

#[derive(Subcommand)]
enum VaultCommand {
    Show,
    /// Point this device at another existing vault.
    Use {
        path: String,
    },
    /// Copy the vault to an empty directory and switch to the copy. The original is kept.
    Move {
        path: String,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    List,
    Get { key: String },
    Set { key: String, value: String },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("distill: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let json = cli.json;
    match cli.command {
        Command::Init { vault, adopt, yes } => {
            let dirs = Dirs::discover()?;
            let vault = choose_vault(vault, yes)?;
            let report = init(
                &dirs,
                InitOptions {
                    vault,
                    adopt,
                    bin_path: std::env::current_exe().ok(),
                },
            )?;
            emit(json, &report, render::init)
        }
        Command::Save { input, topic } => {
            let text = match input {
                Some(path) => std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?,
                None => {
                    let mut buf = String::new();
                    std::io::stdin().read_to_string(&mut buf)?;
                    buf
                }
            };
            let mut req: SaveRequest = serde_json::from_str(&text)
                .context("the save request is not valid JSON for distill save")?;
            if let Some(topic) = topic {
                req.topic = topic;
            }
            let result = Distill::open()?.save(req)?;
            emit(json, &result, render::saved)
        }
        Command::Recall { question, limit } => {
            let result = Distill::open()?.index.recall(&question.join(" "), limit)?;
            emit(json, &result, render::recall)
        }
        Command::Search { query, tag, limit } => {
            let hits = Distill::open()?
                .index
                .search(&query.join(" "), tag.as_deref(), limit)?;
            emit(json, &hits, |h| render::hits(h))
        }
        Command::Stats => emit(json, &Distill::open()?.index.stats()?, render::stats),
        Command::Tags => emit(json, &Distill::open()?.index.tags()?, |t| render::tags(t)),
        Command::Annotate { note_id, text } => {
            let path = Distill::open()?.annotate(&note_id, &text.join(" "))?;
            emit(json, &serde_json::json!({ "path": path }), |_| {
                format!("Annotation saved to {}", path.display())
            })
        }
        Command::Reindex => {
            let mut d = Distill::open()?;
            let report = d.index.rebuild()?;
            emit(json, &report, render::sync)
        }
        Command::Vault(cmd) => vault(cmd, json),
        Command::Config(cmd) => config(cmd, json),
        Command::Doctor => {
            let report = Distill::open()?.doctor()?;
            emit(json, &report, render::doctor)?;
            if !report.ok {
                bail!("doctor found {} problem(s)", report.problems.len());
            }
            Ok(())
        }
    }
}

fn vault(cmd: VaultCommand, json: bool) -> Result<()> {
    let mut d = Distill::open()?;
    match cmd {
        VaultCommand::Show => {}
        VaultCommand::Use { path } => d.use_vault(&expand_vault_arg(&path)?)?,
        VaultCommand::Move { path } => {
            let copied = d.move_vault(&expand_vault_arg(&path)?)?;
            if !json {
                println!("Copied {copied} file(s). The original vault was left in place.");
            }
        }
    }
    let info = serde_json::json!({
        "vault": d.vault.root(),
        "vault_id": d.vault.id(),
        "index_file": d.dirs.index_file(d.vault.id()),
    });
    emit(json, &info, |_| {
        format!(
            "Vault: {}\nVault id: {}\nIndex: {}",
            d.vault.root().display(),
            d.vault.id(),
            d.dirs.index_file(d.vault.id()).display()
        )
    })
}

fn config(cmd: ConfigCommand, json: bool) -> Result<()> {
    let dirs = Dirs::discover()?;
    let mut config = distill_core::config::LocalConfig::load(&dirs)?;
    match cmd {
        ConfigCommand::List => {
            let mut values = serde_json::Map::new();
            for key in ["vault", "device_id"].iter().chain(SETTABLE_KEYS) {
                values.insert(key.to_string(), config.get(key)?.into());
            }
            emit(json, &values, |v| {
                v.iter()
                    .map(|(k, v)| format!("{k} = {}", v.as_str().unwrap_or_default()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
        ConfigCommand::Get { key } => {
            let value = config.get(&key)?;
            emit(json, &serde_json::json!({ key.clone(): value }), |_| {
                value.clone()
            })
        }
        ConfigCommand::Set { key, value } => {
            config.set(&key, &value)?;
            config.save(&dirs)?;
            let value = config.get(&key)?;
            emit(json, &serde_json::json!({ key.clone(): value }), |_| {
                format!("{key} = {value}")
            })
        }
    }
}

/// Resolves the vault for `init`: the flag, or the proposed default after confirmation.
fn choose_vault(flag: Option<String>, yes: bool) -> Result<PathBuf> {
    if let Some(arg) = flag {
        return Ok(expand_vault_arg(&arg)?);
    }
    let proposed = default_vault_path()
        .context("could not find a documents or home directory; pass --vault <dir>")?;
    if yes {
        return Ok(proposed);
    }
    if !std::io::stdin().is_terminal() {
        bail!(
            "no --vault given and stdin is not a terminal. Pass --vault <dir>, or --yes to use {}",
            proposed.display()
        );
    }
    print!(
        "Vault location [{}]\n(press Enter to accept, or type another path): ",
        proposed.display()
    );
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let line = line.trim();
    if line.is_empty() {
        Ok(proposed)
    } else {
        Ok(expand_vault_arg(line)?)
    }
}

fn emit<T: Serialize>(json: bool, value: &T, human: impl FnOnce(&T) -> String) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{}", human(value));
    }
    Ok(())
}
