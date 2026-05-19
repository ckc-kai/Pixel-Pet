//! On-disk persistence — TOML settings + per-state JSON drawings.
//! See `docs/architecture.md` §4.
//!
//! Phase 0 defines `Drawing`, `StorageError`, and `From` impls that scrub
//! privacy-sensitive detail (`io::Error` paths, parser positions) before they
//! cross the IPC boundary. Agent A4 implements the actual I/O in `settings.rs`,
//! `drawings.rs`, and `migrations.rs` (`docs/agent-team-plan.md` §4.4).

use serde::{Deserialize, Serialize};

use crate::state::PetState;

pub mod drawings;
pub mod migrations;
pub mod settings;

/// User pixel-art for one pet state.
///
/// `palette[0]` is the transparent slot by convention. `pixels[y][x]` indexes
/// into `palette`; out-of-range indices are rejected by the loader as
/// [`StorageError::Corrupt`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct Drawing {
    pub schema_version: u32,
    pub state: PetState,
    pub width: u32,
    pub height: u32,
    pub palette: Vec<String>,
    /// Row-major: `pixels[y][x]`.
    pub pixels: Vec<Vec<u32>>,
}

/// Persistence-layer error. Privacy-safe: never carries `io::Error` text or
/// filesystem paths in its `Display` output (architecture.md §2.3,
/// CLAUDE.md §9). Detail is logged server-side via `tracing` at the call site.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("not found")]
    NotFound,
    #[error("invalid input: {0}")]
    Validation(String),
    #[error("corrupt data")]
    Corrupt,
    #[error("future schema version")]
    FutureVersion,
    #[error("storage failure")]
    Io,
    #[error("storage failure")]
    Encoding,
}

impl From<std::io::Error> for StorageError {
    fn from(_: std::io::Error) -> Self {
        StorageError::Io
    }
}

impl From<toml::de::Error> for StorageError {
    fn from(_: toml::de::Error) -> Self {
        StorageError::Encoding
    }
}

impl From<toml::ser::Error> for StorageError {
    fn from(_: toml::ser::Error) -> Self {
        StorageError::Encoding
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(_: serde_json::Error) -> Self {
        StorageError::Encoding
    }
}
