//! Distill core: the vault (Markdown files, the source of truth), the per-device SQLite
//! index derived from it, and the operations every front end shares.

pub mod config;
pub mod error;
mod fsutil;
pub mod model;

pub use error::{Error, Result};
