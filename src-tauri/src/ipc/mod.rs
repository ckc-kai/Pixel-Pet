//! IPC layer between Rust and the React frontend. Every cross-boundary call
//! goes through this module — see `docs/architecture.md` §2.
//!
//! **Privacy:** `IpcError` is flat by design. Internal `io::Error` /
//! `serde_json::Error` detail is logged server-side via `tracing` but never
//! serialized over the wire (CLAUDE.md §8).

use std::path::PathBuf;
use std::sync::{atomic::AtomicUsize, Arc};

use serde::Serialize;
use specta::Type;
use tokio::sync::{watch, Mutex};

use crate::config::Settings;
use crate::state::PetState;

pub mod editor;
pub mod pet;
pub mod settings;

/// Shared runtime state managed by Tauri (`.manage(AppState)`).
///
/// All mutex fields use `tokio::sync::Mutex` so guards can be held safely
/// across `.await` points inside async Tauri commands. `Arc` wrapping makes
/// `AppState` cheaply cloneable without copying the underlying data.
#[derive(Clone)]
pub struct AppState {
    /// Current visible pet state.
    pub pet_state: Arc<Mutex<PetState>>,
    /// Active settings; mirrored on disk by `persistence::save_settings`.
    pub settings: Arc<Mutex<Settings>>,
    /// Resolved per-platform application data directory.
    pub data_dir: Arc<PathBuf>,
    /// Broadcast channel for `pet_subscribe_state`.
    pub state_tx: watch::Sender<PetState>,
    /// Active `pet_subscribe_state` subscriber task count.
    /// Capped at [`pet::MAX_SUBSCRIBERS`] to prevent unbounded task leaks.
    pub subscriber_count: Arc<AtomicUsize>,
}

impl AppState {
    pub fn new(pet_state: PetState, settings: Settings, data_dir: PathBuf) -> Self {
        let (state_tx, _state_rx) = watch::channel(pet_state);
        Self {
            pet_state: Arc::new(Mutex::new(pet_state)),
            settings: Arc::new(Mutex::new(settings)),
            data_dir: Arc::new(data_dir),
            state_tx,
            subscriber_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

/// Wire-format error. Distinct from `StorageError` so the IPC layer owns the
/// public contract and storage details never leak.
///
/// **Privacy (CLAUDE.md §8):**
/// - `NotFound` carries **no message payload** — a caller-supplied string would
///   be serialized to the frontend verbatim, risking filesystem-path leaks.
///   Use `BadRequest` when a safe, user-readable diagnostic is needed.
/// - `Storage` and `Internal` are opaque by design; full detail is logged
///   server-side via `tracing::error!`.
#[derive(Debug, thiserror::Error, Serialize, Type)]
#[serde(tag = "kind", content = "message")]
pub enum IpcError {
    #[error("not found")]
    NotFound,
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
            SE::NotFound => IpcError::NotFound,
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
