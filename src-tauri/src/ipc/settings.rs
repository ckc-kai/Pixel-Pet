//! Settings IPC commands. Owner: agent A5.

use super::{merge_settings_patch, AppState, IpcError};
use crate::config::{self, Settings};
use crate::persistence;

/// Return the in-memory copy of `Settings`. Cheap (single mutex acquisition).
#[tauri::command]
#[specta::specta]
pub fn settings_get(state: tauri::State<'_, AppState>) -> Result<Settings, IpcError> {
    let guard = state.settings.lock().map_err(|err| {
        tracing::error!(error = %err, "settings mutex poisoned");
        IpcError::Internal
    })?;
    Ok(guard.clone())
}

/// Deep-merge `patch` into the current settings and persist the result.
///
/// Returns the merged settings on success. Validation failure short-circuits
/// the disk write — the in-memory copy is also left untouched.
///
/// `patch_json` is a JSON-encoded `string` on the wire (rather than
/// `serde_json::Value`) because specta v2.0.0-rc.25 has a stack-overflow bug
/// in its `Value::definition` impl. Callers stringify a partial `Settings`
/// shape with `JSON.stringify(patch)` before invoking; we parse it server-side.
/// See `tests::apply_patch_*` for the merge semantics.
#[tauri::command]
#[specta::specta]
pub fn settings_patch(
    patch_json: String,
    state: tauri::State<'_, AppState>,
) -> Result<Settings, IpcError> {
    let patch: serde_json::Value = serde_json::from_str(&patch_json).map_err(|err| {
        tracing::warn!(error = %err, "settings_patch received malformed JSON");
        IpcError::BadRequest("patch must be a JSON object".into())
    })?;

    let merged = {
        let guard = state.settings.lock().map_err(|err| {
            tracing::error!(error = %err, "settings mutex poisoned");
            IpcError::Internal
        })?;
        apply_patch(&guard, patch)?
    };

    persistence::settings::save_settings(state.data_dir.as_ref(), &merged).map_err(|err| {
        tracing::error!(?err, "settings_patch save failed");
        IpcError::from(err)
    })?;

    let mut guard = state.settings.lock().map_err(|err| {
        tracing::error!(error = %err, "settings mutex poisoned");
        IpcError::Internal
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
