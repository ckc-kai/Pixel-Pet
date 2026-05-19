//! Pet-state IPC commands. Owner: agent A5.
//!
//! Each command is annotated with `#[specta::specta]` so `tauri-specta` can
//! emit a typed TS binding. State lives in [`AppState`] (see `ipc/mod.rs`).

use tauri::ipc::Channel;

use super::{AppState, IpcError};
use crate::state::PetState;

/// Return the pet's current visible state.
#[tauri::command]
#[specta::specta]
pub fn pet_get_state(state: tauri::State<'_, AppState>) -> Result<PetState, IpcError> {
    let guard = state.pet_state.lock().map_err(|err| {
        tracing::error!(error = %err, "pet_state mutex poisoned");
        IpcError::Internal
    })?;
    Ok(*guard)
}

/// Subscribe the frontend to pet-state changes. The supplied Tauri `Channel`
/// receives every change emitted via [`AppState::state_tx`] until either side
/// is dropped.
///
/// Returns immediately after spawning the forwarder task; the channel itself
/// carries the stream.
#[tauri::command]
#[specta::specta]
pub fn pet_subscribe_state(
    state: tauri::State<'_, AppState>,
    channel: Channel<PetState>,
) -> Result<(), IpcError> {
    let mut rx = state.state_tx.subscribe();
    // Emit the current value immediately so subscribers don't wait for the
    // first transition to learn the state.
    let current = *rx.borrow();
    if let Err(err) = channel.send(current) {
        tracing::error!(error = %err, "failed to seed pet_subscribe_state channel");
    }

    tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() {
                // Sender dropped — app is shutting down.
                break;
            }
            let next = *rx.borrow();
            if let Err(err) = channel.send(next) {
                tracing::warn!(error = %err, "pet_subscribe_state channel send failed");
                break;
            }
        }
    });

    Ok(())
}

/// Force a transition for debugging. Compiled out in release builds — the
/// `#[cfg(debug_assertions)]` guard at registration time keeps the surface
/// area honest.
#[cfg(debug_assertions)]
#[tauri::command]
#[specta::specta]
pub fn pet_force_transition(
    target: PetState,
    state: tauri::State<'_, AppState>,
) -> Result<PetState, IpcError> {
    {
        let mut guard = state.pet_state.lock().map_err(|err| {
            tracing::error!(error = %err, "pet_state mutex poisoned");
            IpcError::Internal
        })?;
        *guard = target;
    }
    if let Err(err) = state.state_tx.send(target) {
        tracing::warn!(error = %err, "pet_force_transition broadcast failed");
    }
    Ok(target)
}
