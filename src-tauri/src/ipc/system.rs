//! System-level IPC commands.
//!
//! These commands cross the OS/window boundary in ways the other domain
//! modules (`pet`, `editor`, `settings`) intentionally do not. Keep this
//! module narrow — one command per system intent.

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::{AppState, IpcError};
use crate::persistence;

/// Labels used in `tauri.conf.json` (architecture.md §1.3). Centralized here
/// so the routing in `lib.rs::run()` and this command never drift.
pub const EDITOR_WINDOW_LABEL: &str = "editor";
pub const PET_WINDOW_LABEL: &str = "pet";

/// Complete the one-time drawing ritual.
///
/// Sequence (all-or-nothing as far as user-observable state goes):
/// 1. Set `setup.drawing_confirmed = true` in memory.
/// 2. Persist `settings.toml` atomically. On failure: revert in-memory flag and
///    return `Storage`.
/// 3. Hide the editor window (`editor`) and show the pet window (`pet`).
///
/// Window show/hide is best-effort: if the labels are missing (developer
/// removed them from `tauri.conf.json`) we log and return `Internal`. The
/// persistence step is the source of truth — the next launch will route to the
/// pet window regardless of whether the runtime hide/show succeeded.
///
/// **This command is irreversible by design.** No companion `redraw` IPC is
/// exposed (CLAUDE.md product anchor: drawing is a one-time ritual).
#[tauri::command]
#[specta::specta]
pub async fn system_complete_drawing(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), IpcError> {
    // Hold the settings lock for the read → mutate → save cycle so a concurrent
    // settings_patch cannot interleave and produce inconsistent disk/memory
    // state. Matches the pattern in `settings_patch`.
    let mut guard = state.settings.lock().await;

    if guard.setup.drawing_confirmed {
        // Idempotent: already confirmed. Still swap the windows in case the
        // caller is recovering from a bad UI state.
        drop(guard);
        return swap_to_pet_window(&app);
    }

    let mut next = guard.clone();
    next.setup.drawing_confirmed = true;

    let data_dir = Arc::clone(&state.data_dir);
    let next_for_save = next.clone();
    tokio::task::spawn_blocking(move || {
        persistence::settings::save_settings(&data_dir, &next_for_save)
    })
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "system_complete_drawing save task panicked");
        IpcError::Internal
    })?
    .map_err(|err| {
        tracing::error!(?err, "system_complete_drawing save_settings failed");
        IpcError::from(err)
    })?;

    *guard = next;
    drop(guard);

    swap_to_pet_window(&app)
}

/// Hide the editor window and show the pet window. Extracted for reuse in
/// the idempotent fast-path above and to keep window-management noise out of
/// the lock-holding section.
fn swap_to_pet_window(app: &AppHandle) -> Result<(), IpcError> {
    let editor = app.get_webview_window(EDITOR_WINDOW_LABEL).ok_or_else(|| {
        tracing::error!(label = EDITOR_WINDOW_LABEL, "editor window not registered");
        IpcError::Internal
    })?;
    let pet = app.get_webview_window(PET_WINDOW_LABEL).ok_or_else(|| {
        tracing::error!(label = PET_WINDOW_LABEL, "pet window not registered");
        IpcError::Internal
    })?;

    if let Err(err) = editor.hide() {
        tracing::warn!(error = %err, "failed to hide editor window");
    }
    pet.show().map_err(|err| {
        tracing::error!(error = %err, "failed to show pet window");
        IpcError::Internal
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    //! `system_complete_drawing` exercises window APIs that need a live Tauri
    //! runtime, which we don't bring up in unit tests (cf. existing
    //! `ipc/tests.rs` patterns). What we *can* test in isolation:
    //!
    //! - The pure persistence half: setting `drawing_confirmed = true` and
    //!   round-tripping through `settings.toml`.
    //!
    //! The window-swap path is covered by manual smoke ("delete settings,
    //! launch, draw, confirm, see pet window") and an integration test in a
    //! future PR if Tauri's `MockRuntime` matures.

    use crate::config::Settings;
    use crate::persistence;
    use tempfile::TempDir;

    #[test]
    fn confirm_flag_persists_through_save_load_round_trip() {
        let dir = TempDir::new().expect("tempdir");
        let mut settings = Settings::default();
        assert!(!settings.setup.drawing_confirmed);

        settings.setup.drawing_confirmed = true;
        persistence::settings::save_settings(dir.path(), &settings).expect("save");

        let loaded = persistence::settings::load_settings(dir.path()).expect("load");
        assert!(
            loaded.setup.drawing_confirmed,
            "drawing_confirmed must survive a save/load cycle"
        );
    }

    #[test]
    fn confirm_flag_is_independent_of_other_setup_fields() {
        // Sanity: flipping drawing_confirmed must not bleed into auto_start or
        // do_not_disturb (Branch 2 will rely on the partition).
        let dir = TempDir::new().expect("tempdir");
        let mut settings = Settings::default();
        settings.setup.drawing_confirmed = true;
        persistence::settings::save_settings(dir.path(), &settings).expect("save");

        let loaded = persistence::settings::load_settings(dir.path()).expect("load");
        assert!(loaded.setup.drawing_confirmed);
        assert!(!loaded.setup.auto_start);
        assert!(!loaded.setup.do_not_disturb);
    }
}
