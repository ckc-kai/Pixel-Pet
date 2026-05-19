//! IPC layer between Rust and the React frontend. Every cross-boundary call
//! goes through this module — see `docs/architecture.md` §2.
//!
//! **Privacy:** `IpcError` is flat by design. Internal `io::Error` /
//! `serde_json::Error` detail is logged server-side via `tracing` but never
//! serialized over the wire (CLAUDE.md §9).
//!
//! Phase 0 defines `IpcError` and the eight command stubs. Agent A5 wires
//! them to real backends and adds `tauri-specta` for generated TS bindings
//! (`docs/agent-team-plan.md` §4.5).

use serde::Serialize;

pub mod editor;
pub mod pet;
pub mod settings;

/// Wire-format error. Distinct from `StorageError` so the IPC layer owns the
/// public contract and storage details never leak.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum IpcError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    BadRequest(String),
    #[error("storage failure")]
    Storage,
    #[error("internal error")]
    Internal,
}

impl From<crate::persistence::StorageError> for IpcError {
    fn from(err: crate::persistence::StorageError) -> Self {
        use crate::persistence::StorageError as SE;
        match err {
            SE::NotFound => IpcError::NotFound(String::new()),
            SE::Validation(msg) => IpcError::BadRequest(msg),
            SE::Corrupt | SE::FutureVersion | SE::Io | SE::Encoding => IpcError::Storage,
        }
    }
}
