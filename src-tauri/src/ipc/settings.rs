//! Settings IPC commands. Owner: agent A5.

use super::IpcError;
use crate::config::Settings;

#[tauri::command]
pub fn settings_get() -> Result<Settings, IpcError> {
    unimplemented!("agent A5")
}

#[tauri::command]
pub fn settings_patch(patch: serde_json::Value) -> Result<Settings, IpcError> {
    let _ = patch;
    unimplemented!("agent A5")
}
