//! Centralized configuration constants and the `Settings` schema.
//!
//! Per `docs/architecture.md` §4.3, every tunable lives in `Settings` or in
//! the constants below — no magic numbers anywhere else in the codebase.
//!
//! Agent A1 extends this module with `Settings::load_or_default` + clamp
//! validation; see `docs/agent-team-plan.md` §4.1.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::persistence::StorageError;

/// Persistence schema version. Bump together with a migration in
/// `persistence::migrations`.
pub const SCHEMA_VERSION: u32 = 1;

/// Hard floor for the activity poll interval (architecture.md §5.2).
/// Cannot be lowered by user settings — protects the CPU budget (CLAUDE.md §4).
pub const POLL_INTERVAL_FLOOR_SECONDS: u32 = 5;

/// Hard floor for `idle_threshold_seconds`. Below 1s would mean every keypress
/// flips activity — pointless thrash.
pub const IDLE_THRESHOLD_FLOOR_SECONDS: u32 = 1;

/// Defensive ceiling on drawing dimensions when loading from disk. The editor
/// enforces its own size; this just prevents pathological loads.
pub const DRAWING_MAX_DIMENSION: u32 = 256;

/// Backoff multiplier on the poll interval while the pet is in `Sleep`.
/// Capped by [`SLEEP_BACKOFF_MAX_SECONDS`] to keep wake latency tolerable.
pub const SLEEP_BACKOFF_MULTIPLIER: u32 = 5;
pub const SLEEP_BACKOFF_MAX_SECONDS: u32 = 300;

/// Bundle identifier; used to derive the on-disk data directory.
pub const BUNDLE_ID: &str = "com.kaicheng.pixelpet";

/// Top-level settings, mirroring `settings.toml`.
///
/// `#[serde(default)]` on every section makes the loader tolerant of missing
/// fields — the v0 → v1 migration story until a real schema break happens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Settings {
    // Field-level `#[serde(default)]` overrides the container default for this
    // field only: a TOML file missing `schema_version` deserializes to `0`,
    // not to `SCHEMA_VERSION`. That's the signal A4's migration framework
    // uses to detect pre-versioned files (architecture.md §4.5).
    #[serde(default)]
    pub schema_version: u32,
    pub setup: SetupSettings,
    pub activity: ActivitySettings,
    pub work: WorkSettings,
    pub meals: MealSettings,
    pub window: WindowSettings,
}

/// One-time and system-level flags (architecture.md §1.3, §4.3).
///
/// `drawing_confirmed` gates the startup window routing: false → show editor,
/// true → show pet. It is set exactly once via `system_complete_drawing` and
/// is intentionally not exposed in any settings UI (the editor is one-time).
///
/// `auto_start` and `do_not_disturb` are persisted here so the schema is
/// stable; the UI wiring lands in Branch 2 (`feat/tray-and-settings`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct SetupSettings {
    pub drawing_confirmed: bool,
    pub auto_start: bool,
    pub do_not_disturb: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct ActivitySettings {
    pub idle_threshold_seconds: u32,
    pub poll_interval_seconds: u32,
    pub spaced_out_idle_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct WorkSettings {
    pub stretch_at_minutes: u32,
    pub tired_at_minutes: u32,
    pub sleep_at_minutes: u32,
    pub stretch_overlay_seconds: u32,
    pub eating_overlay_seconds: u32,
}

/// Meal trigger times. `HH:MM` 24-hour format. Parsing/validation lives in
/// the consumer (agent A2 / A4); this struct just carries the string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct MealSettings {
    pub breakfast: String,
    pub lunch: String,
    pub dinner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct WindowSettings {
    pub x: i32,
    pub y: i32,
    pub size: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            setup: SetupSettings::default(),
            activity: ActivitySettings::default(),
            work: WorkSettings::default(),
            meals: MealSettings::default(),
            window: WindowSettings::default(),
        }
    }
}

impl Default for ActivitySettings {
    fn default() -> Self {
        Self {
            idle_threshold_seconds: 60,
            poll_interval_seconds: 60,
            spaced_out_idle_minutes: 15,
        }
    }
}

impl Default for WorkSettings {
    fn default() -> Self {
        Self {
            stretch_at_minutes: 60,
            tired_at_minutes: 75,
            sleep_at_minutes: 90,
            stretch_overlay_seconds: 30,
            eating_overlay_seconds: 60,
        }
    }
}

impl Default for MealSettings {
    fn default() -> Self {
        Self {
            breakfast: "08:00".to_string(),
            lunch: "12:30".to_string(),
            dinner: "19:00".to_string(),
        }
    }
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            size: 64,
        }
    }
}

/// Load `settings.toml` from `path`, falling back to defaults if the file does
/// not exist. Parses, then runs [`validate`] before returning.
///
/// Errors:
/// - [`StorageError::Io`] for unreadable but extant files.
/// - [`StorageError::Encoding`] for malformed TOML.
/// - [`StorageError::Validation`] when clamp checks reject the contents.
///
/// Missing `schema_version` is permitted and parses to `0`; A4's migration
/// framework decides what to do with it (architecture.md §4.5).
pub fn load_or_default(path: &Path) -> Result<Settings, StorageError> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Settings::default());
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to read settings.toml");
            return Err(StorageError::Io);
        }
    };

    let settings: Settings = toml::from_str(&contents).map_err(|e| {
        tracing::warn!(error = %e, "failed to parse settings.toml");
        StorageError::Encoding
    })?;

    validate(&settings)?;
    Ok(settings)
}

/// Reject settings whose tunables fall outside the contract:
/// - `activity.poll_interval_seconds` must be ≥ [`POLL_INTERVAL_FLOOR_SECONDS`]
/// - `activity.idle_threshold_seconds` must be ≥ [`IDLE_THRESHOLD_FLOOR_SECONDS`]
/// - `work.stretch_at_minutes` < `work.tired_at_minutes` < `work.sleep_at_minutes`
///
/// Returns [`StorageError::Validation`] with a human-readable message on
/// violation. Never panics; never logs settings contents (they may contain
/// user-tuned values we treat as semi-private).
pub fn validate(s: &Settings) -> Result<(), StorageError> {
    if s.activity.poll_interval_seconds < POLL_INTERVAL_FLOOR_SECONDS {
        return Err(StorageError::Validation(format!(
            "activity.poll_interval_seconds ({}) is below the {}s floor",
            s.activity.poll_interval_seconds, POLL_INTERVAL_FLOOR_SECONDS
        )));
    }
    if s.activity.idle_threshold_seconds < IDLE_THRESHOLD_FLOOR_SECONDS {
        return Err(StorageError::Validation(format!(
            "activity.idle_threshold_seconds ({}) is below the {}s floor",
            s.activity.idle_threshold_seconds, IDLE_THRESHOLD_FLOOR_SECONDS
        )));
    }
    if s.work.stretch_at_minutes >= s.work.tired_at_minutes {
        return Err(StorageError::Validation(format!(
            "work.stretch_at_minutes ({}) must be less than work.tired_at_minutes ({})",
            s.work.stretch_at_minutes, s.work.tired_at_minutes
        )));
    }
    if s.work.tired_at_minutes >= s.work.sleep_at_minutes {
        return Err(StorageError::Validation(format!(
            "work.tired_at_minutes ({}) must be less than work.sleep_at_minutes ({})",
            s.work.tired_at_minutes, s.work.sleep_at_minutes
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_settings(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("settings.toml");
        std::fs::write(&path, body).expect("write tmp settings.toml");
        path
    }

    #[test]
    fn defaults_validate_cleanly() {
        let s = Settings::default();
        assert_eq!(s.schema_version, SCHEMA_VERSION);
        validate(&s).expect("shipped defaults must pass validation");
    }

    #[test]
    fn defaults_round_trip_through_toml() {
        let original = Settings::default();
        let serialized = toml::to_string(&original).expect("serialize defaults");
        let parsed: Settings = toml::from_str(&serialized).expect("parse defaults");
        assert_eq!(original, parsed);
    }

    #[test]
    fn load_or_default_returns_defaults_when_file_missing() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("does-not-exist.toml");
        let s = load_or_default(&missing).expect("missing file is not an error");
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn load_or_default_parses_well_formed_file() {
        let dir = tempdir().expect("tempdir");
        let path = write_settings(
            dir.path(),
            r#"
schema_version = 1

[setup]
drawing_confirmed = true
auto_start = false
do_not_disturb = false

[activity]
idle_threshold_seconds = 90
poll_interval_seconds = 30
spaced_out_idle_minutes = 15

[work]
stretch_at_minutes = 60
tired_at_minutes = 75
sleep_at_minutes = 90
stretch_overlay_seconds = 30
eating_overlay_seconds = 60

[meals]
breakfast = "08:00"
lunch = "12:30"
dinner = "19:00"

[window]
x = 100
y = 100
size = 64
"#,
        );
        let s = load_or_default(&path).expect("parses");
        assert_eq!(s.activity.idle_threshold_seconds, 90);
        assert_eq!(s.activity.poll_interval_seconds, 30);
        assert!(s.setup.drawing_confirmed);
        assert!(!s.setup.auto_start);
    }

    #[test]
    fn setup_defaults_to_all_false() {
        let s = Settings::default();
        assert!(!s.setup.drawing_confirmed);
        assert!(!s.setup.auto_start);
        assert!(!s.setup.do_not_disturb);
    }

    #[test]
    fn setup_section_missing_uses_defaults() {
        // Forward-compat: a settings.toml predating the [setup] section must
        // load cleanly with drawing_confirmed=false (i.e. show editor on next launch).
        let dir = tempdir().expect("tempdir");
        let path = write_settings(
            dir.path(),
            r#"
schema_version = 1
[activity]
idle_threshold_seconds = 60
poll_interval_seconds = 60
spaced_out_idle_minutes = 15
"#,
        );
        let s = load_or_default(&path).expect("parses");
        assert_eq!(s.setup, SetupSettings::default());
    }

    #[test]
    fn poll_interval_below_floor_is_rejected() {
        let mut s = Settings::default();
        s.activity.poll_interval_seconds = POLL_INTERVAL_FLOOR_SECONDS - 1;
        match validate(&s) {
            Err(StorageError::Validation(msg)) => {
                assert!(msg.contains("poll_interval_seconds"), "got: {msg}");
            }
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn idle_threshold_below_floor_is_rejected() {
        let mut s = Settings::default();
        s.activity.idle_threshold_seconds = 0;
        match validate(&s) {
            Err(StorageError::Validation(msg)) => {
                assert!(msg.contains("idle_threshold_seconds"), "got: {msg}");
            }
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn stretch_must_precede_tired() {
        let mut s = Settings::default();
        s.work.stretch_at_minutes = s.work.tired_at_minutes;
        match validate(&s) {
            Err(StorageError::Validation(msg)) => {
                assert!(msg.contains("stretch_at_minutes"), "got: {msg}");
            }
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn tired_must_precede_sleep() {
        let mut s = Settings::default();
        s.work.tired_at_minutes = s.work.sleep_at_minutes;
        match validate(&s) {
            Err(StorageError::Validation(msg)) => {
                assert!(msg.contains("tired_at_minutes"), "got: {msg}");
            }
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn load_or_default_propagates_validation_error() {
        let dir = tempdir().expect("tempdir");
        // poll_interval=1 is below the 5s floor.
        let path = write_settings(
            dir.path(),
            r#"
schema_version = 1
[activity]
poll_interval_seconds = 1
"#,
        );
        match load_or_default(&path) {
            Err(StorageError::Validation(_)) => {}
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn unknown_toml_fields_are_tolerated() {
        // Per architecture.md §4.5 we want forward-compatible loads: a newer
        // build adding a knob should not brick an older binary.
        let dir = tempdir().expect("tempdir");
        let path = write_settings(
            dir.path(),
            r#"
schema_version = 1
mystery_future_field = "ignored"

[activity]
idle_threshold_seconds = 60
poll_interval_seconds = 60
spaced_out_idle_minutes = 15
unknown_activity_knob = 42
"#,
        );
        let s = load_or_default(&path).expect("unknown fields should be tolerated");
        assert_eq!(s.activity.idle_threshold_seconds, 60);
    }

    #[test]
    fn missing_schema_version_parses_to_zero() {
        // Pre-versioned files (architecture.md §4.5) parse to schema_version=0
        // so A4's migration framework can detect and upgrade them.
        let dir = tempdir().expect("tempdir");
        let path = write_settings(
            dir.path(),
            r#"
[activity]
idle_threshold_seconds = 60
poll_interval_seconds = 60
spaced_out_idle_minutes = 15
"#,
        );
        let s = load_or_default(&path).expect("missing schema_version must not panic");
        assert_eq!(s.schema_version, 0);
    }

    #[test]
    fn malformed_toml_is_encoding_error() {
        let dir = tempdir().expect("tempdir");
        let path = write_settings(dir.path(), "this = is = not = toml\n");
        match load_or_default(&path) {
            Err(StorageError::Encoding) => {}
            other => panic!("expected Encoding, got {other:?}"),
        }
    }
}
