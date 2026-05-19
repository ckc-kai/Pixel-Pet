//! Settings IPC commands.
//!
//! All disk operations are wrapped in `tokio::task::spawn_blocking` so the
//! async executor is not stalled. `settings_patch` holds the `tokio::sync::Mutex`
//! guard for the full merge → save → store cycle to prevent TOCTOU races
//! between concurrent patch calls.

use std::sync::Arc;

use super::{merge_settings_patch, AppState, IpcError};
use crate::config::{self, Settings};
use crate::persistence;

/// Return the in-memory copy of `Settings`. Cheap (single mutex acquisition).
#[tauri::command]
#[specta::specta]
pub async fn settings_get(state: tauri::State<'_, AppState>) -> Result<Settings, IpcError> {
    let guard = state.settings.lock().await;
    Ok(guard.clone())
}

/// Deep-merge `patch` into the current settings and persist the result.
///
/// Returns the merged settings on success. Validation failure short-circuits
/// the disk write — the in-memory copy is also left untouched.
///
/// The mutex is held for the full merge → save → store sequence so concurrent
/// calls cannot interleave and produce an inconsistent disk / memory state.
///
/// `patch_json` is a JSON-encoded `string` on the wire (rather than
/// `serde_json::Value`) because specta v2.0.0-rc.25 has a stack-overflow bug
/// in its `Value::definition` impl. Callers stringify a partial `Settings`
/// shape with `JSON.stringify(patch)` before invoking; we parse it server-side.
#[tauri::command]
#[specta::specta]
pub async fn settings_patch(
    patch_json: String,
    state: tauri::State<'_, AppState>,
) -> Result<Settings, IpcError> {
    let patch: serde_json::Value = serde_json::from_str(&patch_json).map_err(|err| {
        tracing::warn!(error = %err, "settings_patch received malformed JSON");
        IpcError::BadRequest("patch must be a JSON object".into())
    })?;

    // Hold the lock for the entire operation to prevent TOCTOU between
    // concurrent patch calls. tokio::sync::Mutex is safe to hold across
    // the spawn_blocking await point.
    let mut guard = state.settings.lock().await;
    let merged = apply_patch(&guard, patch)?;

    let data_dir = Arc::clone(&state.data_dir);
    let merged_for_save = merged.clone();
    tokio::task::spawn_blocking(move || {
        persistence::settings::save_settings(&data_dir, &merged_for_save)
    })
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "settings_patch task panicked");
        IpcError::Internal
    })?
    .map_err(|err| {
        tracing::error!(?err, "settings_patch save failed");
        IpcError::from(err)
    })?;

    *guard = merged.clone();
    Ok(merged)
}

/// Pure-data merge + validate. Extracted so tests can call it directly without
/// constructing `tauri::State`.
pub(crate) fn apply_patch(
    current: &Settings,
    patch: serde_json::Value,
) -> Result<Settings, IpcError> {
    let base = serde_json::to_value(current).map_err(|err| {
        tracing::error!(error = %err, "failed to serialize current settings");
        IpcError::Internal
    })?;

    let merged_value = merge_settings_patch(base, patch);

    let merged: Settings = serde_json::from_value(merged_value).map_err(|err| {
        // Don't echo the serde message — it can include user-supplied field
        // names. The wire-format BadRequest is intentionally generic.
        tracing::warn!(error = %err, "settings_patch produced invalid Settings shape");
        IpcError::BadRequest("patch produced an invalid Settings shape".into())
    })?;

    config::validate(&merged).map_err(IpcError::from)?;
    Ok(merged)
}
