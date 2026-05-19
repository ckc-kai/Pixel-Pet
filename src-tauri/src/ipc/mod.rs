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

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tokio::sync::watch;

use crate::config::Settings;
use crate::state::PetState;

pub mod editor;
pub mod pet;
pub mod settings;

/// Shared runtime state managed by Tauri (`.manage(AppState)`).
///
/// Cloning is cheap — every field is `Arc`-wrapped, including the inner
/// `Mutex`es. Wiring lives in `lib.rs::run()`.
///
/// The watch channel carries the latest [`PetState`] so `pet_subscribe_state`
/// can fan out updates to the frontend without polling.
#[derive(Clone)]
pub struct AppState {
    /// Current visible pet state — kept here so IPC `pet_get_state` can read
    /// it without consulting the FSM directly.
    pub pet_state: Arc<Mutex<PetState>>,
    /// Active settings; mirrored on disk by `persistence::save_settings`.
    pub settings: Arc<Mutex<Settings>>,
    /// Resolved per-platform application data directory. All persistence
    /// helpers take this as their first argument.
    pub data_dir: Arc<PathBuf>,
    /// Broadcast channel for `pet_subscribe_state`. The sender is held by
    /// `AppState`; each subscribe call creates a new `Receiver`.
    pub state_tx: watch::Sender<PetState>,
}

impl AppState {
    pub fn new(pet_state: PetState, settings: Settings, data_dir: PathBuf) -> Self {
        let (state_tx, _state_rx) = watch::channel(pet_state);
        Self {
            pet_state: Arc::new(Mutex::new(pet_state)),
            settings: Arc::new(Mutex::new(settings)),
            data_dir: Arc::new(data_dir),
            state_tx,
        }
    }
}

/// Wire-format error. Distinct from `StorageError` so the IPC layer owns the
/// public contract and storage details never leak.
#[derive(Debug, thiserror::Error, Serialize, Type)]
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

/// Deep-merge a JSON patch into a base object. Used by `settings_patch` so
/// callers can send partial updates without having to round-trip the entire
/// `Settings` struct.
///
/// Rules:
/// - Both arguments must be `Object`. If either is not, `patch` wins
///   (mirrors the [JSON Merge Patch RFC 7396] shallow case).
/// - Keys present in `patch` overwrite keys in `base`.
/// - When both sides hold an `Object` for the same key, recurse — this
///   preserves sibling keys the patch did not mention.
/// - `Null` in the patch deletes the corresponding key in `base` (also
///   RFC 7396 behaviour).
///
/// [JSON Merge Patch RFC 7396]: https://www.rfc-editor.org/rfc/rfc7396
pub fn merge_settings_patch(
    base: serde_json::Value,
    patch: serde_json::Value,
) -> serde_json::Value {
    use serde_json::Value;
    match (base, patch) {
        (Value::Object(mut base_map), Value::Object(patch_map)) => {
            for (key, patch_val) in patch_map {
                if patch_val.is_null() {
                    base_map.remove(&key);
                    continue;
                }
                let merged = match base_map.remove(&key) {
                    Some(existing) => merge_settings_patch(existing, patch_val),
                    None => patch_val,
                };
                base_map.insert(key, merged);
            }
            Value::Object(base_map)
        }
        (_, patch) => patch,
    }
}

#[cfg(test)]
mod tests;
