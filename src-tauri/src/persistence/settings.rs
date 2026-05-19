//! TOML settings loader + atomic writer.
//!
//! Owner: agent A4. See `docs/agent-team-plan.md` §4.4 and
//! `docs/architecture.md` §4.3.
//!
//! All path inputs are injected; this module never resolves the platform data
//! directory itself (that is A5's wiring at startup). Functions accept a
//! `&Path` so tests can pass a `tempfile::TempDir`.
//!
//! Privacy: every IO/parse failure is logged at the call site via
//! `tracing::error!` with full detail, then mapped to a scrubbed
//! [`StorageError`] before crossing the IPC boundary (CLAUDE.md §9,
//! architecture.md §2.3).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use super::StorageError;
use crate::config::Settings;

/// Filename of the canonical settings file inside the app data directory.
const SETTINGS_FILENAME: &str = "settings.toml";

/// Suffix appended to the previous file before any atomic overwrite.
const BACKUP_SUFFIX: &str = ".bak";

fn settings_path(dir: &Path) -> PathBuf {
    dir.join(SETTINGS_FILENAME)
}

fn backup_path(dir: &Path) -> PathBuf {
    dir.join(format!("{SETTINGS_FILENAME}{BACKUP_SUFFIX}"))
}

/// Load `dir/settings.toml`.
///
/// Behaviour:
/// - File absent → returns [`Settings::default`] (not an error; first-run case).
/// - File present but unreadable → [`StorageError::Io`].
/// - File present but unparseable → [`StorageError::Encoding`].
///
/// The on-disk schema is tolerant: `#[serde(default)]` on `Settings` means
/// unknown / missing fields are filled from defaults rather than failing.
pub fn load_settings(dir: &Path) -> Result<Settings, StorageError> {
    let path = settings_path(dir);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Settings::default());
        }
        Err(err) => {
            tracing::error!(error = %err, "failed to read settings.toml");
            return Err(StorageError::Io);
        }
    };

    toml::from_str(&content).map_err(|err| {
        tracing::error!(error = %err, "failed to parse settings.toml");
        StorageError::Encoding
    })
}

/// Persist `settings` to `dir/settings.toml` atomically.
///
/// Sequence:
/// 1. `mkdir -p dir`
/// 2. If `dir/settings.toml` already exists, copy it to `dir/settings.toml.bak`.
/// 3. Write the new content to a [`NamedTempFile`] in the same directory.
/// 4. Atomically `persist` the temp file over the canonical path.
///
/// Atomicity guarantee: a crash between any two steps leaves either the old
/// file or the new file fully on disk — never a partial write.
pub fn save_settings(dir: &Path, settings: &Settings) -> Result<(), StorageError> {
    fs::create_dir_all(dir).map_err(|err| {
        tracing::error!(error = %err, "failed to create settings directory");
        StorageError::Io
    })?;

    let target = settings_path(dir);
    if target.exists() {
        let backup = backup_path(dir);
        fs::copy(&target, &backup).map_err(|err| {
            tracing::error!(error = %err, "failed to write settings backup");
            StorageError::Io
        })?;
    }

    let serialized = toml::to_string_pretty(settings).map_err(|err| {
        tracing::error!(error = %err, "failed to serialize settings");
        StorageError::Encoding
    })?;

    let mut tmp = NamedTempFile::new_in(dir).map_err(|err| {
        tracing::error!(error = %err, "failed to create temporary settings file");
        StorageError::Io
    })?;

    tmp.write_all(serialized.as_bytes()).map_err(|err| {
        tracing::error!(error = %err, "failed to write temporary settings file");
        StorageError::Io
    })?;

    tmp.persist(&target).map_err(|err| {
        tracing::error!(error = %err.error, "failed to persist settings file");
        StorageError::Io
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_missing_file_returns_default() {
        let dir = TempDir::new().expect("tempdir");
        let settings = load_settings(dir.path()).expect("load");
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn save_then_load_round_trips_defaults() {
        let dir = TempDir::new().expect("tempdir");
        let original = Settings::default();
        save_settings(dir.path(), &original).expect("save");
        let loaded = load_settings(dir.path()).expect("load");
        assert_eq!(loaded, original);
    }

    #[test]
    fn save_then_load_round_trips_modified_settings() {
        let dir = TempDir::new().expect("tempdir");
        let mut original = Settings::default();
        original.activity.idle_threshold_seconds = 90;
        original.window.x = 42;
        original.meals.lunch = "13:00".to_string();
        save_settings(dir.path(), &original).expect("save");
        let loaded = load_settings(dir.path()).expect("load");
        assert_eq!(loaded, original);
    }

    #[test]
    fn save_writes_bak_of_preexisting_file() {
        let dir = TempDir::new().expect("tempdir");
        let mut first = Settings::default();
        first.window.size = 32;
        save_settings(dir.path(), &first).expect("first save");

        let mut second = Settings::default();
        second.window.size = 128;
        save_settings(dir.path(), &second).expect("second save");

        let bak = backup_path(dir.path());
        assert!(bak.exists(), "backup should exist after second save");
        let bak_content = fs::read_to_string(&bak).expect("read bak");
        let bak_parsed: Settings = toml::from_str(&bak_content).expect("parse bak");
        assert_eq!(bak_parsed, first, "backup should contain pre-save state");

        let loaded = load_settings(dir.path()).expect("load");
        assert_eq!(loaded, second);
    }

    #[test]
    fn save_leaves_no_leftover_temp_files() {
        // Atomic-write check: tempfile must be consumed by persist, leaving
        // only the canonical file (+ optional .bak on overwrite).
        let dir = TempDir::new().expect("tempdir");
        save_settings(dir.path(), &Settings::default()).expect("save");

        let entries: Vec<_> = fs::read_dir(dir.path())
            .expect("read_dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();

        assert_eq!(entries, vec![SETTINGS_FILENAME.to_string()]);
    }

    #[test]
    fn load_corrupt_toml_returns_encoding_error() {
        let dir = TempDir::new().expect("tempdir");
        fs::create_dir_all(dir.path()).expect("mkdir");
        fs::write(settings_path(dir.path()), "this is = not [valid toml")
            .expect("seed corrupt file");
        let err = load_settings(dir.path()).expect_err("should fail to parse");
        assert!(matches!(err, StorageError::Encoding));
    }
}
