//! Drawing / editor IPC commands.
//!
//! All disk operations are wrapped in `tokio::task::spawn_blocking` so the
//! async Tauri executor is never stalled by synchronous filesystem I/O.

use std::sync::Arc;

use super::{AppState, IpcError};
use crate::config::DRAWING_MAX_DIMENSION;
use crate::persistence::{self, Drawing, StorageError};
use crate::state::PetState;

/// Load the drawing for `state`. Returns `Ok(None)` when no drawing has been
/// saved yet (caller falls back to the built-in default).
#[tauri::command]
#[specta::specta]
pub async fn editor_load_drawing(
    state: PetState,
    app_state: tauri::State<'_, AppState>,
) -> Result<Option<Drawing>, IpcError> {
    let data_dir = Arc::clone(&app_state.data_dir);
    tokio::task::spawn_blocking(move || {
        match persistence::drawings::load_drawing(&data_dir, state) {
            Ok(opt) => Ok(opt),
            Err(StorageError::NotFound) => Ok(None),
            Err(err) => {
                tracing::error!(?err, "editor_load_drawing failed");
                Err(IpcError::from(err))
            }
        }
    })
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "editor_load_drawing task panicked");
        IpcError::Internal
    })?
}

/// Persist `drawing` for `state`. Enforces:
/// - canvas dimensions ≤ [`DRAWING_MAX_DIMENSION`] (256 × 256)
/// - `drawing.state == state` (no cross-state writes via this command)
///
/// The persistence layer re-validates structural invariants (row/column
/// counts, palette bounds) before writing.
#[tauri::command]
#[specta::specta]
pub async fn editor_save_drawing(
    state: PetState,
    drawing: Drawing,
    app_state: tauri::State<'_, AppState>,
) -> Result<(), IpcError> {
    validate_save_request(state, &drawing)?;
    let data_dir = Arc::clone(&app_state.data_dir);
    tokio::task::spawn_blocking(move || {
        persistence::drawings::save_drawing(&data_dir, &drawing).map_err(|err| {
            tracing::error!(?err, "editor_save_drawing failed");
            IpcError::from(err)
        })
    })
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "editor_save_drawing task panicked");
        IpcError::Internal
    })?
}

/// List every pet state for which the editor can hold a drawing.
#[tauri::command]
#[specta::specta]
pub fn editor_list_states() -> Result<Vec<PetState>, IpcError> {
    Ok(PetState::all().to_vec())
}

/// IPC-layer validation. Kept as a free function so unit tests can exercise
/// it without constructing a real `tauri::State`.
pub(crate) fn validate_save_request(state: PetState, drawing: &Drawing) -> Result<(), IpcError> {
    if drawing.state != state {
        return Err(IpcError::BadRequest(format!(
            "drawing.state ({:?}) does not match command argument ({:?})",
            drawing.state, state
        )));
    }
    if drawing.width == 0 || drawing.height == 0 {
        return Err(IpcError::BadRequest(format!(
            "drawing dimensions must be non-zero (got {}x{})",
            drawing.width, drawing.height
        )));
    }
    if drawing.width > DRAWING_MAX_DIMENSION || drawing.height > DRAWING_MAX_DIMENSION {
        return Err(IpcError::BadRequest(format!(
            "drawing dimensions ({}x{}) exceed the {DRAWING_MAX_DIMENSION}px cap",
            drawing.width, drawing.height
        )));
    }
    Ok(())
}
