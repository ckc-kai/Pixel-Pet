//! Pixel Pet library entry point.
//!
//! `main.rs` is a thin shim; all logic lives here and in the modules below.
//! See `docs/architecture.md` §1.1 for the module layout.

pub mod activity;
pub mod config;
pub mod ipc;
pub mod persistence;
pub mod state;

use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

use crate::ipc::AppState;
use crate::state::PetState;

/// Build the tauri-specta [`Builder`] with every IPC command this app exposes.
///
/// `pet_force_transition` is wired in only when `debug_assertions` is on, so
/// release builds cannot accidentally ship the FSM-override path.
///
/// Extracted from `run()` so unit tests can construct + export the builder
/// without spinning up a real Tauri runtime.
pub fn build_specta_builder() -> Builder<tauri::Wry> {
    #[cfg(debug_assertions)]
    {
        Builder::<tauri::Wry>::new().commands(collect_commands![
            ipc::pet::pet_get_state,
            ipc::pet::pet_subscribe_state,
            ipc::pet::pet_force_transition,
            ipc::editor::editor_load_drawing,
            ipc::editor::editor_save_drawing,
            ipc::editor::editor_list_states,
            ipc::settings::settings_get,
            ipc::settings::settings_patch,
        ])
    }
    #[cfg(not(debug_assertions))]
    {
        Builder::<tauri::Wry>::new().commands(collect_commands![
            ipc::pet::pet_get_state,
            ipc::pet::pet_subscribe_state,
            ipc::editor::editor_load_drawing,
            ipc::editor::editor_save_drawing,
            ipc::editor::editor_list_states,
            ipc::settings::settings_get,
            ipc::settings::settings_patch,
        ])
    }
}

/// Tauri application entry. Wires plugins, IPC handlers, managed state.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = build_specta_builder();

    #[cfg(debug_assertions)]
    {
        use specta_typescript::Typescript;
        if let Err(err) =
            specta_builder.export(Typescript::default(), "../src/lib/types/bindings.ts")
        {
            tracing::warn!(error = %err, "failed to export TypeScript bindings");
        }
    }

    // ---- Tauri application ---------------------------------------------- //
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);

            // Resolve the per-user app-data directory. Done once at setup so
            // the rest of the app can treat it as immutable.
            let data_dir = app.path().app_data_dir().map_err(|err| {
                tracing::error!(error = %err, "failed to resolve app_data_dir");
                err
            })?;
            if let Err(err) = std::fs::create_dir_all(&data_dir) {
                tracing::error!(error = %err, "failed to create app_data_dir");
                return Err(Box::new(err) as Box<dyn std::error::Error>);
            }

            // Load settings (defaults if missing or invalid). We validate after
            // loading so a hand-edited file with out-of-range values falls back
            // to defaults rather than running with unclamped poll intervals.
            let settings = persistence::settings::load_settings(&data_dir)
                .and_then(|s| {
                    config::validate(&s).map_err(|e| {
                        tracing::warn!(error = ?e, "loaded settings failed validation; using defaults");
                        crate::persistence::StorageError::Validation(e.to_string())
                    })?;
                    Ok(s)
                })
                .unwrap_or_else(|err| {
                    tracing::warn!(?err, "failed to load settings; using defaults");
                    config::Settings::default()
                });

            // No persisted pet state yet — boot at Working. The FSM may emit
            // a Startup→Working transition on its first tick; until A6 wires
            // the runtime loop, this is the safe default.
            let initial_state = PetState::Working;

            let app_state = AppState::new(initial_state, settings, data_dir);
            app.manage(app_state);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke-test: the specta builder builds and exports a non-empty TS file.
    /// This guards against a future command being added without `#[specta]`
    /// or a Type-derive being dropped from a shared struct.
    ///
    /// Side-effect: also writes the real `../src/lib/types/bindings.ts` so
    /// `cargo test` doubles as the binding-generation step until the app
    /// runtime owns it (CLAUDE.md §3 — keep the IPC surface small + typed).
    #[test]
    fn specta_builder_exports_typescript_bindings() {
        use specta_typescript::Typescript;
        use std::fs;

        let dir = tempfile::TempDir::new().expect("tempdir");
        let out = dir.path().join("bindings.ts");
        let builder = build_specta_builder();
        builder
            .export(Typescript::default(), &out)
            .expect("specta export must succeed");

        let content = fs::read_to_string(&out).expect("bindings.ts written");
        // Every command should land in the generated bindings.
        for cmd in [
            "petGetState",
            "petSubscribeState",
            "editorLoadDrawing",
            "editorSaveDrawing",
            "editorListStates",
            "settingsGet",
            "settingsPatch",
        ] {
            assert!(
                content.contains(cmd),
                "binding for `{cmd}` missing from generated TS"
            );
        }

        // Best-effort: also drop the bindings into the real frontend path so
        // `cargo test` keeps it fresh. Failures here are non-fatal — they
        // mean we're running outside the repo (e.g. CI sandbox).
        let real_path = std::path::Path::new("../src/lib/types/bindings.ts");
        if let Some(parent) = real_path.parent() {
            if parent.exists() {
                let _ = build_specta_builder().export(Typescript::default(), real_path);
            }
        }
    }
}
