//! Pixel Pet library entry point.
//!
//! `main.rs` is a thin shim; all logic lives here and in the modules below.
//! See `docs/architecture.md` §1.1 for the module layout.

pub mod activity;
pub mod config;
pub mod ipc;
pub mod persistence;
pub mod state;

/// Tauri application entry. Wires plugins, IPC handlers, managed state.
/// Concrete handler list is filled in by agent A5 (`docs/agent-team-plan.md` §4.5).
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
