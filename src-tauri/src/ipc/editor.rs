//! Drawing / editor IPC commands. Owner: agent A5.

use super::IpcError;
use crate::persistence::Drawing;
use crate::state::PetState;

#[tauri::command]
pub fn editor_load_drawing(state: PetState) -> Result<Option<Drawing>, IpcError> {
    let _ = state;
    unimplemented!("agent A5")
}

#[tauri::command]
pub fn editor_save_drawing(state: PetState, drawing: Drawing) -> Result<(), IpcError> {
    let _ = (state, drawing);
    unimplemented!("agent A5")
}

#[tauri::command]
pub fn editor_list_states() -> Result<Vec<PetState>, IpcError> {
    Ok(PetState::all().to_vec())
}
