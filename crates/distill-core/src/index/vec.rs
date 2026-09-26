//! Registers the statically linked sqlite-vec extension with SQLite, so every connection
//! opened afterwards has the `vec0` virtual table and the `vec_*` functions (spec D3).
//!
//! This is the only `unsafe` code in the workspace; the workspace lint denies it everywhere
//! else.
#![allow(unsafe_code)]

use std::sync::OnceLock;

use rusqlite::auto_extension::{RawAutoExtension, register_auto_extension};

use crate::error::{Error, Result};

/// Registers once per process, before the first `Connection::open`.
pub(super) fn register() -> Result<()> {
    static REGISTERED: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| {
            // SAFETY: `sqlite3_vec_init` is sqlite-vec's C entry point, compiled with
            // SQLITE_CORE against the SQLite that rusqlite bundles. Its real signature is
            // SQLite's extension-init signature, which is what `RawAutoExtension` describes;
            // the crate only declares it as `fn()`. The init routine opens no databases and
            // does not touch the auto-extension list, which is what
            // `register_auto_extension` requires.
            unsafe {
                let init = std::mem::transmute::<unsafe extern "C" fn(), RawAutoExtension>(
                    sqlite_vec::sqlite3_vec_init,
                );
                register_auto_extension(init).map_err(|e| e.to_string())
            }
        })
        .clone()
        .map_err(|reason| Error::VectorExtension { reason })
}
