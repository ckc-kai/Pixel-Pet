//! Pet-state enums and the per-tick context the FSM operates on.
//! See `docs/architecture.md` §3 for state semantics.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Visible pet state. Stays `Copy + Hash + Eq` so transition tables can use
/// it directly as a key (see `machine::TRANSITIONS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetState {
    Startup,
    Working,
    Stretch,
    Tired,
    Sleep,
    SpacedOut,
    Eating,
}

impl PetState {
    /// Stable on-disk slug; used for `drawings/<slug>.json` filenames.
    /// Changing a slug is a schema break — bump `SCHEMA_VERSION` and add a
    /// migration.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Working => "working",
            Self::Stretch => "stretch",
            Self::Tired => "tired",
            Self::Sleep => "sleep",
            Self::SpacedOut => "spaced_out",
            Self::Eating => "eating",
        }
    }

    /// Every state — useful for the persistence layer and exhaustive tests.
    pub fn all() -> &'static [PetState] {
        &[
            Self::Startup,
            Self::Working,
            Self::Stretch,
            Self::Tired,
            Self::Sleep,
            Self::SpacedOut,
            Self::Eating,
        ]
    }
}

/// Wall-clock meal kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MealKind {
    Breakfast,
    Lunch,
    Dinner,
}

/// Tier the active-time accumulator has reached. Lives in the trigger so the
/// FSM table can match by tier without learning real Settings thresholds.
///
/// `PartialOrd`/`Ord` are `Stretch < Tired < Sleep` so guards can compare
/// inequalities (e.g. "≥ Sleep tier").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkTier {
    Stretch,
    Tired,
    Sleep,
}

/// Inputs that drive a state transition. The transition table in
/// `machine::TRANSITIONS` matches against these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trigger {
    AppStartup,
    ActivityActive,
    /// Idle ≥ `spaced_out_idle_seconds`. The `Duration` payload is the actual
    /// configured threshold (informational); `step()` matches on the variant
    /// only because IdleFor has a single destination per from-state.
    IdleFor(Duration),
    /// Active-time accumulator has crossed into the named tier. Carries the
    /// tier directly so the FSM stays pure — real minute thresholds live in
    /// `Settings` and are consumed only by `Ctx::tick`.
    AccumulatedActiveTime(WorkTier),
    StretchAnimationFinished,
    EatingFinished,
    MealTime(MealKind),
    /// Honored only in debug builds — see machine.rs guard.
    ManualOverride(PetState),
}

/// FSM context — accumulators and last-meal latch. Kept out of `PetState`
/// so the enum stays `Copy + Hash`.
///
/// Mutated only via methods defined in `machine.rs`. Read-only accessors
/// are exposed here for transition guards.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ctx {
    /// Active-time accumulator (architecture.md §3.4).
    pub(crate) active_seconds: u64,
    /// Wall idle window since the last `Active` tick.
    pub(crate) idle_seconds: u64,
    /// Most recent meal trigger fired, to debounce same-meal repeats.
    pub(crate) last_meal: Option<MealKind>,
    /// Highest tier the accumulator has crossed this work session. Used by
    /// `Eating → {Sleep,Tired,Working}` re-evaluation. Reset on `SpacedOut`
    /// entry. Set by `Ctx::tick` when it emits an `AccumulatedActiveTime`
    /// trigger.
    pub(crate) tier: Option<WorkTier>,
}

impl Ctx {
    pub fn active_seconds(&self) -> u64 {
        self.active_seconds
    }

    pub fn idle_seconds(&self) -> u64 {
        self.idle_seconds
    }

    pub fn last_meal(&self) -> Option<MealKind> {
        self.last_meal
    }

    /// Highest tier crossed this work session (`None` = below Stretch).
    pub fn tier_reached(&self) -> Option<WorkTier> {
        self.tier
    }
}
