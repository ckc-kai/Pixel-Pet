//! Transition table + `step()` for the pet state machine.
//!
//! Owner: agent A2. See `docs/architecture.md` §3 (especially §3.4 accumulator
//! semantics and §3.5 transition table) and `docs/agent-team-plan.md` §4.2.
//!
//! # Design — tier markers
//!
//! The FSM is **pure**: it owns no thresholds. Real thresholds (e.g.
//! `stretch_at_minutes = 60`, `tired_at_minutes = 75`, …) live in `Settings`
//! and are read by the orchestrator (agents A3/A5).
//!
//! The transition table must, however, distinguish 3 destinations from
//! `Working` on `Trigger::AccumulatedActiveTime` (→ `Stretch` / `Tired` /
//! `Sleep`). Since `Trigger` only carries a `Duration` payload and we cannot
//! change its enum shape, this module exposes opaque **tier markers** —
//! [`TIER_STRETCH`], [`TIER_TIRED`], [`TIER_SLEEP`], [`TIER_SPACED_OUT`] —
//! whose values are arbitrary identifiers (0/1/2/3 seconds) chosen to be
//! deliberately unrealistic as real durations. The orchestrator reads
//! `Settings` to decide *when* the accumulator has crossed a tier, then emits
//! `Trigger::AccumulatedActiveTime(TIER_STRETCH)` (or `TIER_TIRED`/`TIER_SLEEP`).
//!
//! `Trigger::IdleFor(_)` does NOT use a tier marker — its payload is the real
//! `spaced_out_idle_seconds` value emitted by [`Ctx::tick`]. There is only
//! one destination (`SpacedOut`) per from-state for `IdleFor`, so the table
//! matches by kind only and ignores the payload.

use std::time::Duration;

use crate::activity::Activity;
use crate::state::states::{Ctx, PetState, Trigger};

// ---------------------------------------------------------------------------
// Tier markers — opaque identifiers, NOT real thresholds.
// ---------------------------------------------------------------------------

/// Opaque marker for the Stretch tier crossing.
pub const TIER_STRETCH: Duration = Duration::from_secs(0);
/// Opaque marker for the Tired tier crossing.
pub const TIER_TIRED: Duration = Duration::from_secs(1);
/// Opaque marker for the Sleep tier crossing.
pub const TIER_SLEEP: Duration = Duration::from_secs(2);
/// Opaque marker for the SpacedOut idle tier (currently informational; the
/// `IdleFor` arm of the FSM matches by kind only — see module doc).
pub const TIER_SPACED_OUT: Duration = Duration::from_secs(3);

// ---------------------------------------------------------------------------
// Trigger kind — payload-less mirror of `Trigger` for static table matching.
// ---------------------------------------------------------------------------

/// Payload-less mirror of [`Trigger`] used as the table's matching key.
///
/// The static [`TRANSITIONS`] table cannot store full `Trigger` values
/// (they carry `Duration` / `PetState` / `MealKind` payloads), so each row
/// matches by *kind*; payload-aware disambiguation happens in the per-row
/// `guard` (see [`Transition::guard`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    AppStartup,
    ActivityActive,
    IdleFor,
    AccumulatedActiveTime,
    StretchAnimationFinished,
    EatingFinished,
    MealTime,
    /// Honored only in debug builds — see `step()` guard.
    ManualOverride,
}

impl TriggerKind {
    /// Project a `Trigger` to its `TriggerKind`.
    pub fn of(trigger: &Trigger) -> Self {
        match trigger {
            Trigger::AppStartup => Self::AppStartup,
            Trigger::ActivityActive => Self::ActivityActive,
            Trigger::IdleFor(_) => Self::IdleFor,
            Trigger::AccumulatedActiveTime(_) => Self::AccumulatedActiveTime,
            Trigger::StretchAnimationFinished => Self::StretchAnimationFinished,
            Trigger::EatingFinished => Self::EatingFinished,
            Trigger::MealTime(_) => Self::MealTime,
            Trigger::ManualOverride(_) => Self::ManualOverride,
        }
    }
}

// ---------------------------------------------------------------------------
// Transition row.
// ---------------------------------------------------------------------------

/// A single row in the FSM transition table.
///
/// `to` may be `None` only for `ManualOverride` rows whose destination is
/// taken from the trigger payload at match time.
pub struct Transition {
    pub from: PetState,
    pub trigger: TriggerKind,
    /// Optional guard. Receives both `&Ctx` (accumulator state) and the
    /// original `&Trigger` (for payload-aware checks, e.g. tier marker).
    pub guard: Option<fn(&Ctx, &Trigger) -> bool>,
    /// Static destination, or `None` when the destination is derived from
    /// the trigger payload (e.g. `ManualOverride(target)`).
    pub to: Option<PetState>,
}

// ---------------------------------------------------------------------------
// Guard helpers — all expressed in terms of `Ctx` + `Trigger`.
//
// NB: these guards never reference real settings thresholds. They compare
// the trigger payload to the opaque tier markers above. The FSM remains
// pure; real thresholds live in `Settings`.
// ---------------------------------------------------------------------------

fn is_tier(trigger: &Trigger, tier: Duration) -> bool {
    matches!(trigger, Trigger::AccumulatedActiveTime(d) if *d == tier)
}

fn g_stretch_marker(_ctx: &Ctx, trigger: &Trigger) -> bool {
    is_tier(trigger, TIER_STRETCH)
}

fn g_tired_marker(_ctx: &Ctx, trigger: &Trigger) -> bool {
    is_tier(trigger, TIER_TIRED)
}

fn g_sleep_marker(_ctx: &Ctx, trigger: &Trigger) -> bool {
    is_tier(trigger, TIER_SLEEP)
}

/// `Eating → Sleep` when the accumulator at the time of `EatingFinished`
/// indicates the Sleep tier. We do NOT have a Settings threshold here —
/// instead, the orchestrator stamps `ctx.active_seconds` based on the most
/// recently observed tier marker via the `Ctx::tier_at_least_*` helpers,
/// which compare the *seconds* the orchestrator set when it last emitted a
/// tier-crossing `AccumulatedActiveTime`. See [`Ctx::tier_reached`] /
/// [`Ctx::mark_tier`] below.
fn g_eating_to_sleep(ctx: &Ctx, _trigger: &Trigger) -> bool {
    ctx.tier_reached() >= AccumulatorTier::Sleep
}

fn g_eating_to_tired(ctx: &Ctx, _trigger: &Trigger) -> bool {
    ctx.tier_reached() == AccumulatorTier::Tired
}

fn g_eating_to_working(ctx: &Ctx, _trigger: &Trigger) -> bool {
    ctx.tier_reached() < AccumulatorTier::Tired
}

/// Tier ladder used by `Ctx` to record the highest accumulator tier reached
/// in the current work session. Pure; no Settings access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum AccumulatorTier {
    /// Below the Stretch tier (fresh session or just reset).
    #[default]
    None,
    Stretch,
    Tired,
    Sleep,
}

// ---------------------------------------------------------------------------
// Transition table — all 18 rows of architecture.md §3.5.
// ---------------------------------------------------------------------------

/// Authoritative transition table. The order of rows for a given
/// `(from, trigger_kind)` pair matters: `step()` returns the first match.
#[rustfmt::skip]
pub const TRANSITIONS: &[Transition] = &[
    // ── Startup ────────────────────────────────────────────────────────
    Transition {
        from: PetState::Startup,
        trigger: TriggerKind::AppStartup,
        guard: None,
        to: Some(PetState::Working),
    },

    // ── Working ───────────────────────────────────────────────────────
    Transition {
        from: PetState::Working,
        trigger: TriggerKind::AccumulatedActiveTime,
        guard: Some(g_stretch_marker),
        to: Some(PetState::Stretch),
    },
    Transition {
        from: PetState::Working,
        trigger: TriggerKind::AccumulatedActiveTime,
        guard: Some(g_tired_marker),
        to: Some(PetState::Tired),
    },
    Transition {
        from: PetState::Working,
        trigger: TriggerKind::AccumulatedActiveTime,
        guard: Some(g_sleep_marker),
        to: Some(PetState::Sleep),
    },
    Transition {
        from: PetState::Working,
        trigger: TriggerKind::IdleFor,
        guard: None,
        to: Some(PetState::SpacedOut),
    },
    Transition {
        from: PetState::Working,
        trigger: TriggerKind::MealTime,
        guard: None,
        to: Some(PetState::Eating),
    },

    // ── Stretch (brief overlay) ───────────────────────────────────────
    Transition {
        from: PetState::Stretch,
        trigger: TriggerKind::StretchAnimationFinished,
        guard: None,
        to: Some(PetState::Working),
    },
    Transition {
        from: PetState::Stretch,
        trigger: TriggerKind::IdleFor,
        guard: None,
        to: Some(PetState::SpacedOut),
    },

    // ── Tired (sticky) ────────────────────────────────────────────────
    Transition {
        from: PetState::Tired,
        trigger: TriggerKind::AccumulatedActiveTime,
        guard: Some(g_sleep_marker),
        to: Some(PetState::Sleep),
    },
    Transition {
        from: PetState::Tired,
        trigger: TriggerKind::IdleFor,
        guard: None,
        to: Some(PetState::SpacedOut),
    },
    Transition {
        from: PetState::Tired,
        trigger: TriggerKind::MealTime,
        guard: None,
        to: Some(PetState::Eating),
    },

    // ── Sleep (sticky) ────────────────────────────────────────────────
    Transition {
        from: PetState::Sleep,
        trigger: TriggerKind::IdleFor,
        guard: None,
        to: Some(PetState::SpacedOut),
    },
    Transition {
        from: PetState::Sleep,
        trigger: TriggerKind::MealTime,
        guard: None,
        to: Some(PetState::Eating),
    },

    // ── SpacedOut ─────────────────────────────────────────────────────
    Transition {
        from: PetState::SpacedOut,
        trigger: TriggerKind::ActivityActive,
        guard: None,
        to: Some(PetState::Working),
    },

    // ── Eating (re-evaluate accumulator on finish) ────────────────────
    Transition {
        from: PetState::Eating,
        trigger: TriggerKind::EatingFinished,
        guard: Some(g_eating_to_sleep),
        to: Some(PetState::Sleep),
    },
    Transition {
        from: PetState::Eating,
        trigger: TriggerKind::EatingFinished,
        guard: Some(g_eating_to_tired),
        to: Some(PetState::Tired),
    },
    Transition {
        from: PetState::Eating,
        trigger: TriggerKind::EatingFinished,
        guard: Some(g_eating_to_working),
        to: Some(PetState::Working),
    },

    // ── ManualOverride (debug builds only — see step()) ───────────────
    // The row is present unconditionally; `step()` gates its honoring on
    // `#[cfg(debug_assertions)]`. `to: None` signals "use trigger payload".
    Transition {
        from: PetState::Startup,
        trigger: TriggerKind::ManualOverride,
        guard: None,
        to: None,
    },
    // NB: ManualOverride from any from-state is handled in step() before
    // table iteration, so a single row above is enough as a placeholder
    // for table-row-count accounting (architecture.md §3.5 "any | … row).
];

// ---------------------------------------------------------------------------
// step()
// ---------------------------------------------------------------------------

/// Compute the next state given the current state, an input trigger, and
/// the FSM context. Returns `None` when no transition matches (no-op tick).
pub fn step(current: PetState, trigger: &Trigger, ctx: &Ctx) -> Option<PetState> {
    // ManualOverride is global ("any → target") and gated to debug builds.
    if let Trigger::ManualOverride(target) = trigger {
        #[cfg(debug_assertions)]
        {
            return Some(*target);
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = target;
            return None;
        }
    }

    let kind = TriggerKind::of(trigger);
    TRANSITIONS
        .iter()
        .filter(|t| t.from == current && t.trigger == kind)
        .find(|t| t.guard.is_none_or(|g| g(ctx, trigger)))
        .and_then(|t| t.to)
}

// ---------------------------------------------------------------------------
// Ctx — accumulator semantics (architecture.md §3.4)
// ---------------------------------------------------------------------------

impl Ctx {
    /// Advance accumulators for a single poll tick and emit any triggers
    /// that fire as a result. Pure given inputs; no clock access.
    ///
    /// Rules (architecture.md §3.4):
    /// * `Activity::Active` → `active_seconds += poll_interval_seconds`,
    ///   `idle_seconds = 0`. Emits `Trigger::ActivityActive` (consumed by
    ///   `SpacedOut → Working`; harmless to other states because no row
    ///   matches).
    ///   * Exception: `Eating` pauses the active accumulator.
    /// * `Activity::Idle` → `idle_seconds += poll_interval_seconds`,
    ///   `active_seconds` UNCHANGED. When `idle_seconds >=
    ///   spaced_out_idle_seconds`, emits
    ///   `Trigger::IdleFor(spaced_out_idle_seconds)`.
    /// * `Stretch` does NOT pause the active accumulator — see test
    ///   `stretch_does_not_pause_accumulator`.
    pub fn tick(
        &mut self,
        activity: Activity,
        poll_interval_seconds: u32,
        spaced_out_idle_seconds: u32,
        current_state: PetState,
    ) -> Vec<Trigger> {
        let mut triggers = Vec::new();

        match activity {
            Activity::Active => {
                self.idle_seconds = 0;
                let paused = matches!(current_state, PetState::Eating);
                if !paused {
                    self.active_seconds = self
                        .active_seconds
                        .saturating_add(u64::from(poll_interval_seconds));
                }
                triggers.push(Trigger::ActivityActive);
            }
            Activity::Idle => {
                self.idle_seconds = self
                    .idle_seconds
                    .saturating_add(u64::from(poll_interval_seconds));
                if self.idle_seconds >= u64::from(spaced_out_idle_seconds) {
                    triggers.push(Trigger::IdleFor(Duration::from_secs(u64::from(
                        spaced_out_idle_seconds,
                    ))));
                }
            }
        }

        triggers
    }

    /// Record that the orchestrator has emitted a tier-crossing trigger.
    /// Used by `Eating → {Sleep,Tired,Working}` to re-evaluate at finish.
    pub fn mark_tier(&mut self, tier: AccumulatorTier) {
        if tier > self.tier {
            self.tier = tier;
        }
    }

    /// Highest tier marker reached this work session.
    pub fn tier_reached(&self) -> AccumulatorTier {
        self.tier
    }

    /// Reset accumulators on entering `SpacedOut` (architecture.md §3.4).
    /// Idempotent.
    pub fn reset_on_spaced_out(&mut self) {
        self.active_seconds = 0;
        self.idle_seconds = 0;
        self.tier = AccumulatorTier::None;
    }

    /// Apply post-transition side effects in one call. Mirrors the spec's
    /// "when handled by step → SpacedOut, reset active_seconds = 0".
    pub fn on_entered(&mut self, new_state: PetState) {
        if matches!(new_state, PetState::SpacedOut) {
            self.reset_on_spaced_out();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::Activity;
    use crate::state::states::{MealKind, Trigger};
    use rstest::rstest;

    fn ctx() -> Ctx {
        Ctx::default()
    }

    // --- Row coverage: one positive test per row in §3.5 -------------------

    #[test]
    fn startup_to_working_on_app_startup() {
        assert_eq!(
            step(PetState::Startup, &Trigger::AppStartup, &ctx()),
            Some(PetState::Working)
        );
    }

    #[test]
    fn working_to_stretch_on_stretch_tier_marker() {
        assert_eq!(
            step(
                PetState::Working,
                &Trigger::AccumulatedActiveTime(TIER_STRETCH),
                &ctx()
            ),
            Some(PetState::Stretch)
        );
    }

    #[test]
    fn working_to_tired_on_tired_tier_marker() {
        assert_eq!(
            step(
                PetState::Working,
                &Trigger::AccumulatedActiveTime(TIER_TIRED),
                &ctx()
            ),
            Some(PetState::Tired)
        );
    }

    #[test]
    fn working_to_sleep_on_sleep_tier_marker() {
        assert_eq!(
            step(
                PetState::Working,
                &Trigger::AccumulatedActiveTime(TIER_SLEEP),
                &ctx()
            ),
            Some(PetState::Sleep)
        );
    }

    #[test]
    fn working_to_spaced_out_on_idle_for() {
        assert_eq!(
            step(
                PetState::Working,
                &Trigger::IdleFor(Duration::from_secs(900)),
                &ctx()
            ),
            Some(PetState::SpacedOut)
        );
    }

    #[rstest]
    #[case(MealKind::Breakfast)]
    #[case(MealKind::Lunch)]
    #[case(MealKind::Dinner)]
    fn working_to_eating_on_meal_time(#[case] meal: MealKind) {
        assert_eq!(
            step(PetState::Working, &Trigger::MealTime(meal), &ctx()),
            Some(PetState::Eating)
        );
    }

    #[test]
    fn stretch_to_working_on_animation_finished() {
        assert_eq!(
            step(
                PetState::Stretch,
                &Trigger::StretchAnimationFinished,
                &ctx()
            ),
            Some(PetState::Working)
        );
    }

    #[test]
    fn stretch_to_spaced_out_on_idle_for() {
        assert_eq!(
            step(
                PetState::Stretch,
                &Trigger::IdleFor(Duration::from_secs(900)),
                &ctx()
            ),
            Some(PetState::SpacedOut)
        );
    }

    #[test]
    fn tired_to_sleep_on_sleep_tier_marker() {
        assert_eq!(
            step(
                PetState::Tired,
                &Trigger::AccumulatedActiveTime(TIER_SLEEP),
                &ctx()
            ),
            Some(PetState::Sleep)
        );
    }

    #[test]
    fn tired_to_spaced_out_on_idle_for() {
        assert_eq!(
            step(
                PetState::Tired,
                &Trigger::IdleFor(Duration::from_secs(900)),
                &ctx()
            ),
            Some(PetState::SpacedOut)
        );
    }

    #[test]
    fn tired_to_eating_on_meal_time() {
        assert_eq!(
            step(PetState::Tired, &Trigger::MealTime(MealKind::Lunch), &ctx()),
            Some(PetState::Eating)
        );
    }

    #[test]
    fn sleep_to_spaced_out_on_idle_for() {
        assert_eq!(
            step(
                PetState::Sleep,
                &Trigger::IdleFor(Duration::from_secs(900)),
                &ctx()
            ),
            Some(PetState::SpacedOut)
        );
    }

    #[test]
    fn sleep_to_eating_on_meal_time() {
        assert_eq!(
            step(
                PetState::Sleep,
                &Trigger::MealTime(MealKind::Dinner),
                &ctx()
            ),
            Some(PetState::Eating)
        );
    }

    #[test]
    fn spaced_out_to_working_on_activity_active() {
        assert_eq!(
            step(PetState::SpacedOut, &Trigger::ActivityActive, &ctx()),
            Some(PetState::Working)
        );
    }

    #[test]
    fn eating_to_sleep_when_tier_is_sleep() {
        let mut c = ctx();
        c.mark_tier(AccumulatorTier::Sleep);
        assert_eq!(
            step(PetState::Eating, &Trigger::EatingFinished, &c),
            Some(PetState::Sleep)
        );
    }

    #[test]
    fn eating_to_tired_when_tier_is_tired() {
        let mut c = ctx();
        c.mark_tier(AccumulatorTier::Tired);
        assert_eq!(
            step(PetState::Eating, &Trigger::EatingFinished, &c),
            Some(PetState::Tired)
        );
    }

    #[test]
    fn eating_to_working_when_tier_below_tired() {
        let c = ctx(); // tier defaults to None
        assert_eq!(
            step(PetState::Eating, &Trigger::EatingFinished, &c),
            Some(PetState::Working)
        );
    }

    #[test]
    fn eating_to_working_when_tier_is_stretch() {
        let mut c = ctx();
        c.mark_tier(AccumulatorTier::Stretch);
        assert_eq!(
            step(PetState::Eating, &Trigger::EatingFinished, &c),
            Some(PetState::Working)
        );
    }

    // --- Stickiness / forbidden transitions --------------------------------

    #[test]
    fn tired_does_not_auto_return_to_working_on_activity_active() {
        assert_eq!(
            step(PetState::Tired, &Trigger::ActivityActive, &ctx()),
            None
        );
    }

    #[test]
    fn tired_does_not_react_to_stretch_animation_finished() {
        assert_eq!(
            step(PetState::Tired, &Trigger::StretchAnimationFinished, &ctx()),
            None
        );
    }

    #[test]
    fn sleep_is_sticky_no_activity_active_exit() {
        assert_eq!(
            step(PetState::Sleep, &Trigger::ActivityActive, &ctx()),
            None
        );
    }

    #[test]
    fn sleep_does_not_react_to_accumulated_active_time() {
        assert_eq!(
            step(
                PetState::Sleep,
                &Trigger::AccumulatedActiveTime(TIER_SLEEP),
                &ctx()
            ),
            None
        );
    }

    #[test]
    fn working_unknown_trigger_no_transition() {
        assert_eq!(
            step(PetState::Working, &Trigger::EatingFinished, &ctx()),
            None
        );
    }

    // --- Accumulator semantics (§3.4) --------------------------------------

    #[test]
    fn active_tick_advances_accumulator_and_clears_idle() {
        let mut c = ctx();
        c.idle_seconds = 42;
        let triggers = c.tick(Activity::Active, 60, 900, PetState::Working);
        assert_eq!(c.active_seconds(), 60);
        assert_eq!(c.idle_seconds(), 0);
        assert!(triggers
            .iter()
            .any(|t| matches!(t, Trigger::ActivityActive)));
    }

    #[test]
    fn idle_tick_advances_idle_pauses_active() {
        // Q3.7 — short idle does NOT reset or advance the active accumulator.
        let mut c = ctx();
        c.active_seconds = 1800;
        let triggers = c.tick(Activity::Idle, 60, 900, PetState::Working);
        assert_eq!(
            c.active_seconds(),
            1800,
            "short idle must leave active untouched"
        );
        assert_eq!(c.idle_seconds(), 60);
        assert!(triggers.is_empty(), "no IdleFor below spaced_out window");
    }

    #[test]
    fn idle_tick_emits_idle_for_when_window_crossed() {
        let mut c = ctx();
        c.idle_seconds = 840; // 14 min
        let triggers = c.tick(Activity::Idle, 60, 900, PetState::Working);
        assert_eq!(c.idle_seconds(), 900);
        assert_eq!(
            triggers,
            vec![Trigger::IdleFor(Duration::from_secs(900))],
            "must emit IdleFor exactly with the configured threshold"
        );
    }

    #[test]
    fn accumulator_resets_on_spaced_out_entry() {
        let mut c = ctx();
        c.active_seconds = 5400; // 90 min
        c.idle_seconds = 900;
        c.mark_tier(AccumulatorTier::Sleep);
        c.on_entered(PetState::SpacedOut);
        assert_eq!(c.active_seconds(), 0);
        assert_eq!(c.idle_seconds(), 0);
        assert_eq!(c.tier_reached(), AccumulatorTier::None);
    }

    #[test]
    fn on_entered_non_spaced_out_does_not_reset() {
        let mut c = ctx();
        c.active_seconds = 3600;
        c.on_entered(PetState::Working);
        assert_eq!(c.active_seconds(), 3600);
    }

    #[test]
    fn stretch_does_not_pause_accumulator() {
        // Architecture §3.4: Stretch is a brief visual overlay; the
        // accumulator MUST keep counting through it so the 75-min Tired
        // transition fires on schedule.
        let mut c = ctx();
        c.active_seconds = 3540; // 59 min
                                 // 30 seconds of activity while in Stretch state.
        c.tick(Activity::Active, 30, 900, PetState::Stretch);
        assert_eq!(
            c.active_seconds(),
            3570,
            "Stretch overlay must NOT pause the active accumulator"
        );
    }

    #[test]
    fn eating_pauses_active_accumulator() {
        let mut c = ctx();
        c.active_seconds = 4500;
        c.tick(Activity::Active, 60, 900, PetState::Eating);
        assert_eq!(
            c.active_seconds(),
            4500,
            "Eating must pause the active accumulator"
        );
    }

    #[test]
    fn multiple_active_ticks_accumulate() {
        let mut c = ctx();
        for _ in 0..5 {
            c.tick(Activity::Active, 60, 900, PetState::Working);
        }
        assert_eq!(c.active_seconds(), 300);
    }

    #[test]
    fn idle_then_active_clears_idle_but_preserves_active() {
        let mut c = ctx();
        c.tick(Activity::Active, 60, 900, PetState::Working);
        c.tick(Activity::Idle, 60, 900, PetState::Working);
        c.tick(Activity::Idle, 60, 900, PetState::Working);
        assert_eq!(c.active_seconds(), 60);
        assert_eq!(c.idle_seconds(), 120);
        c.tick(Activity::Active, 60, 900, PetState::Working);
        assert_eq!(c.active_seconds(), 120);
        assert_eq!(c.idle_seconds(), 0);
    }

    #[test]
    fn tier_marker_is_monotonic() {
        let mut c = ctx();
        c.mark_tier(AccumulatorTier::Sleep);
        c.mark_tier(AccumulatorTier::Stretch); // attempt to downgrade
        assert_eq!(c.tier_reached(), AccumulatorTier::Sleep);
    }

    // --- ManualOverride: debug-only -----------------------------------------

    #[cfg(debug_assertions)]
    #[test]
    fn manual_override_works_in_debug_builds() {
        assert_eq!(
            step(
                PetState::Working,
                &Trigger::ManualOverride(PetState::Sleep),
                &ctx()
            ),
            Some(PetState::Sleep)
        );
        assert_eq!(
            step(
                PetState::Sleep,
                &Trigger::ManualOverride(PetState::Startup),
                &ctx()
            ),
            Some(PetState::Startup)
        );
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn manual_override_is_no_op_in_release_builds() {
        assert_eq!(
            step(
                PetState::Working,
                &Trigger::ManualOverride(PetState::Sleep),
                &ctx()
            ),
            None
        );
    }
}
