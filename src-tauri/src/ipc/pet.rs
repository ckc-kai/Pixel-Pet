//! Pet-state IPC commands.
//!
//! All commands are `async` so `tokio::sync::Mutex` guards can be held
//! across await points without blocking the executor.

use std::sync::{atomic::Ordering, Arc};

use tauri::ipc::Channel;

use super::{AppState, IpcError};
use crate::state::PetState;

/// Maximum concurrent `pet_subscribe_state` subscriber tasks.
/// Prevents unbounded task leaks on hot-reload or repeated subscribe calls.
pub(super) const MAX_SUBSCRIBERS: usize = 4;

/// Return the pet's current visible state.
#[tauri::command]
#[specta::specta]
pub async fn pet_get_state(state: tauri::State<'_, AppState>) -> Result<PetState, IpcError> {
    let guard = state.pet_state.lock().await;
    Ok(*guard)
}

/// Subscribe the frontend to pet-state changes. The supplied Tauri `Channel`
/// receives every change emitted via [`AppState::state_tx`] until either side
/// is dropped.
///
/// Returns immediately after spawning the forwarder task; state changes arrive
/// via the channel asynchronously. The current state is sent once on
/// subscription so the frontend does not have to poll for the initial value.
///
/// Returns [`IpcError::BadRequest`] if [`MAX_SUBSCRIBERS`] concurrent
/// subscriber tasks are already active.
#[tauri::command]
#[specta::specta]
pub async fn pet_subscribe_state(
    state: tauri::State<'_, AppState>,
    channel: Channel<PetState>,
) -> Result<(), IpcError> {
    let prev = state.subscriber_count.fetch_add(1, Ordering::Relaxed);
    if prev >= MAX_SUBSCRIBERS {
        state.subscriber_count.fetch_sub(1, Ordering::Relaxed);
        return Err(IpcError::BadRequest(
            "subscriber limit reached; close the existing subscription first".into(),
        ));
    }

    let mut rx = state.state_tx.subscribe();
    let current = *rx.borrow();
    if let Err(err) = channel.send(current) {
        tracing::error!(error = %err, "failed to seed pet_subscribe_state channel");
    }

    let count = Arc::clone(&state.subscriber_count);
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
        count.fetch_sub(1, Ordering::Relaxed);
    });

    Ok(())
}

/// Force a transition for debugging. Compiled out in release builds — the
/// `#[cfg(debug_assertions)]` guard keeps the command surface honest.
#[cfg(debug_assertions)]
#[tauri::command]
#[specta::specta]
pub async fn pet_force_transition(
    target: PetState,
    state: tauri::State<'_, AppState>,
) -> Result<PetState, IpcError> {
    {
        let mut guard = state.pet_state.lock().await;
        *guard = target;
    }
    if let Err(err) = state.state_tx.send(target) {
        tracing::warn!(error = %err, "pet_force_transition broadcast failed");
    }
    Ok(target)
}
