//! Pet-state IPC commands. Owner: agent A5.
//!
//! Signatures are frozen in Phase 0; bodies are `unimplemented!()` until A5
//! wires them to `state::machine` via `tauri::State<AppState>`.

use super::IpcError;
use crate::state::PetState;

#[tauri::command]
pub fn pet_get_state() -> Result<PetState, IpcError> {
    unimplemented!("agent A5 — see docs/agent-team-plan.md §4.5")
}

#[tauri::command]
pub fn pet_subscribe_state() -> Result<(), IpcError> {
    // Real impl emits a Tauri event channel; signature may evolve when A5
    // chooses between `Channel<T>` and `app.emit_to`.
    unimplemented!("agent A5")
}

#[cfg(debug_assertions)]
#[tauri::command]
pub fn pet_force_transition(target: PetState) -> Result<PetState, IpcError> {
    let _ = target;
    unimplemented!("agent A5 — debug builds only")
}
