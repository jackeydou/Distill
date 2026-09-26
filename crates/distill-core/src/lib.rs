//! Distill core: the vault (Markdown files, the source of truth), the per-device SQLite
//! index derived from it, and the operations every front end shares.

pub mod config;
mod edit;
pub mod error;
mod fsutil;
pub mod index;
pub mod model;
pub mod ops;
pub mod redact;
pub mod sources;
pub mod tags;
pub mod vault;

pub use error::{Error, Result};
pub use ops::Distill;
