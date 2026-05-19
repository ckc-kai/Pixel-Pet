//! Centralized configuration constants and the `Settings` schema.
//!
//! Per `docs/architecture.md` §4.3, every tunable lives in `Settings` or in
//! the constants below — no magic numbers anywhere else in the codebase.
//!
//! Agent A1 extends this module with `Settings::load_or_default` + clamp
//! validation; see `docs/agent-team-plan.md` §4.1.

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub activity: ActivitySettings,
    pub work: WorkSettings,
    pub meals: MealSettings,
    pub window: WindowSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActivitySettings {
    pub idle_threshold_seconds: u32,
    pub poll_interval_seconds: u32,
    pub spaced_out_idle_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MealSettings {
    pub breakfast: String,
    pub lunch: String,
    pub dinner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
