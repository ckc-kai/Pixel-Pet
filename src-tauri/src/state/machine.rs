//! Transition table + `step()` for the pet state machine.
//!
//! Owner: agent A2. See `docs/architecture.md` §3 (especially §3.4 accumulator
//! semantics and §3.5 transition table) and `docs/agent-team-plan.md` §4.2.
//!
//! # Phase 0 contract evolution (approved by team-lead)
//!
//! `Trigger::AccumulatedActiveTime` was evolved from a `Duration` payload to a
//! `WorkTier` payload (see `states.rs`). This keeps the FSM pure — real minute
//! thresholds live in `Settings` and only `Ctx::tick` reads them — while
//! letting the static transition table match the three Working-tier
//! destinations directly on the tier variant.
//!
//! `Trigger::IdleFor(Duration)` still carries the real `spaced_out_idle_seconds`
//! value, but `step()` matches by variant only (IdleFor has a single
//! destination per from-state).

use std::time::Duration;

use crate::activity::Activity;
use crate::state::states::{Ctx, PetState, Trigger, WorkTier};

// ---------------------------------------------------------------------------
// Trigger kind — payload-less mirror of `Trigger` for static table matching.
// ---------------------------------------------------------------------------

/// Payload-less mirror of [`Trigger`] used as the table's matching key.
///
/// The static [`TRANSITIONS`] table cannot store full `Trigger` values
/// (they carry `WorkTier` / `Duration` / `PetState` / `MealKind` payloads),
/// so each row matches by *kind*; payload-aware disambiguation happens in
/// the per-row `guard` (see [`Transition::guard`]).
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
/// taken from the trigger payload at match time (handled in `step()` ahead
/// of table iteration).
pub struct Transition {
    pub from: PetState,
    pub trigger: TriggerKind,
    /// Optional guard. Receives both `&Ctx` (accumulator state) and the
    /// original `&Trigger` (for payload-aware checks, e.g. tier variant).
    pub guard: Option<fn(&Ctx, &Trigger) -> bool>,
    /// Static destination, or `None` when the destination is derived from
    /// the trigger payload (e.g. `ManualOverride(target)`).
    pub to: Option<PetState>,
}

// ---------------------------------------------------------------------------
// Guard helpers — all expressed in terms of `Ctx` + `Trigger`.
//
// NB: these guards never reference real settings thresholds. They compare
// the trigger's `WorkTier` payload to the row's expected tier. The FSM
// remains pure; real minute thresholds live in `Settings`.
// ---------------------------------------------------------------------------

fn is_tier(trigger: &Trigger, tier: WorkTier) -> bool {
    matches!(trigger, Trigger::AccumulatedActiveTime(t) if *t == tier)
}

fn g_stretch_tier(_ctx: &Ctx, trigger: &Trigger) -> bool {
    is_tier(trigger, WorkTier::Stretch)
}

fn g_tired_tier(_ctx: &Ctx, trigger: &Trigger) -> bool {
    is_tier(trigger, WorkTier::Tired)
}

fn g_sleep_tier(_ctx: &Ctx, trigger: &Trigger) -> bool {
    is_tier(trigger, WorkTier::Sleep)
}

/// `Eating → Sleep` on finish when the accumulator reached the Sleep tier.
fn g_eating_to_sleep(ctx: &Ctx, _trigger: &Trigger) -> bool {
    ctx.tier_reached() == Some(WorkTier::Sleep)
}

/// `Eating → Tired` on finish when the accumulator reached exactly Tired.
fn g_eating_to_tired(ctx: &Ctx, _trigger: &Trigger) -> bool {
    ctx.tier_reached() == Some(WorkTier::Tired)
}

/// `Eating → Working` on finish when below the Tired tier (None or Stretch).
fn g_eating_to_working(ctx: &Ctx, _trigger: &Trigger) -> bool {
    matches!(ctx.tier_reached(), None | Some(WorkTier::Stretch))
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
        guard: Some(g_stretch_tier),
        to: Some(PetState::Stretch),
    },
    Transition {
        from: PetState::Working,
        trigger: TriggerKind::AccumulatedActiveTime,
        guard: Some(g_tired_tier),
        to: Some(PetState::Tired),
    },
    Transition {
        from: PetState::Working,
        trigger: TriggerKind::AccumulatedActiveTime,
        guard: Some(g_sleep_tier),
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
        guard: Some(g_sleep_tier),
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

    // ── ManualOverride placeholder ────────────────────────────────────
    // The actual ManualOverride logic lives at the top of `step()` because
    // it applies from any state and is gated by `#[cfg(debug_assertions)]`.
    // This row keeps the §3.5 row count (18) accurate; `to: None` plus the
    // step() short-circuit means the row is never selected by iteration.
    Transition {
        from: PetState::Startup,
        trigger: TriggerKind::ManualOverride,
        guard: None,
        to: None,
    },
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

/// Threshold bundle for `Ctx::tick`. All values in seconds. Sourced from
/// `Settings` by the caller (A3/A5) — A1 owns clamp/ordering validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    pub stretch_at_seconds: u32,
    pub tired_at_seconds: u32,
    pub sleep_at_seconds: u32,
    pub spaced_out_idle_seconds: u32,
}

impl Ctx {
    /// Advance accumulators for a single poll tick and emit any triggers
    /// that fire as a result. Pure given inputs; no clock access.
    ///
    /// The `thresholds` arg is the only place real settings values enter
    /// the FSM layer.
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
    /// * `Stretch` does NOT pause the active accumulator.
    /// * When the active accumulator first crosses a tier threshold this
    ///   session, emits `Trigger::AccumulatedActiveTime(WorkTier::…)` and
    ///   advances `self.tier`. At most one tier-crossing trigger per tick;
    ///   if the tick jumps multiple tiers, only the highest is emitted
    ///   (lower tiers are still recorded on `self.tier` history via the
    ///   monotonic `>` comparison — but in practice a single 60 s poll
    ///   cannot jump 15-minute-spaced tiers).
    pub fn tick(
        &mut self,
        activity: Activity,
        poll_interval_seconds: u32,
        thresholds: Thresholds,
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

                if let Some(tier) = self.advance_tier(thresholds) {
                    triggers.push(Trigger::AccumulatedActiveTime(tier));
                }
            }
            Activity::Idle => {
                self.idle_seconds = self
                    .idle_seconds
                    .saturating_add(u64::from(poll_interval_seconds));
                if self.idle_seconds >= u64::from(thresholds.spaced_out_idle_seconds) {
                    triggers.push(Trigger::IdleFor(Duration::from_secs(u64::from(
                        thresholds.spaced_out_idle_seconds,
                    ))));
                }
            }
        }

        triggers
    }

    /// Compute the highest tier reachable at `self.active_seconds` and, if it
    /// is strictly above `self.tier`, advance the latch and return the new
    /// tier. Otherwise return `None`. Pure with respect to mutation of
    /// other fields.
    fn advance_tier(&mut self, thresholds: Thresholds) -> Option<WorkTier> {
        let reachable = if self.active_seconds >= u64::from(thresholds.sleep_at_seconds) {
            Some(WorkTier::Sleep)
        } else if self.active_seconds >= u64::from(thresholds.tired_at_seconds) {
            Some(WorkTier::Tired)
        } else if self.active_seconds >= u64::from(thresholds.stretch_at_seconds) {
            Some(WorkTier::Stretch)
        } else {
            None
        };

        match (reachable, self.tier) {
            (Some(new), None) => {
                self.tier = Some(new);
                Some(new)
            }
            (Some(new), Some(curr)) if new > curr => {
                self.tier = Some(new);
                Some(new)
            }
            _ => None,
        }
    }

    /// Reset accumulators on entering `SpacedOut` (architecture.md §3.4).
    /// Idempotent.
    pub fn reset_on_spaced_out(&mut self) {
        self.active_seconds = 0;
        self.idle_seconds = 0;
        self.tier = None;
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

    // Canonical defaults from architecture.md §4.3 — used ONLY in tests, never
    // in production code (the FSM reads thresholds from caller-supplied args).
    const STRETCH_AT_S: u32 = 60 * 60;
    const TIRED_AT_S: u32 = 75 * 60;
    const SLEEP_AT_S: u32 = 90 * 60;
    const SPACED_OUT_S: u32 = 15 * 60;

    fn thresholds() -> Thresholds {
        Thresholds {
            stretch_at_seconds: STRETCH_AT_S,
            tired_at_seconds: TIRED_AT_S,
            sleep_at_seconds: SLEEP_AT_S,
            spaced_out_idle_seconds: SPACED_OUT_S,
        }
    }

    fn tick_active(c: &mut Ctx, poll: u32, state: PetState) -> Vec<Trigger> {
        c.tick(Activity::Active, poll, thresholds(), state)
    }

    fn tick_idle(c: &mut Ctx, poll: u32, state: PetState) -> Vec<Trigger> {
        c.tick(Activity::Idle, poll, thresholds(), state)
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
    fn working_to_stretch_on_stretch_tier() {
        assert_eq!(
            step(
                PetState::Working,
                &Trigger::AccumulatedActiveTime(WorkTier::Stretch),
                &ctx()
            ),
            Some(PetState::Stretch)
        );
    }

    #[test]
    fn working_to_tired_on_tired_tier() {
        assert_eq!(
            step(
                PetState::Working,
                &Trigger::AccumulatedActiveTime(WorkTier::Tired),
                &ctx()
            ),
            Some(PetState::Tired)
        );
    }

    #[test]
    fn working_to_sleep_on_sleep_tier() {
        assert_eq!(
            step(
                PetState::Working,
                &Trigger::AccumulatedActiveTime(WorkTier::Sleep),
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
    fn tired_to_sleep_on_sleep_tier() {
        assert_eq!(
            step(
                PetState::Tired,
                &Trigger::AccumulatedActiveTime(WorkTier::Sleep),
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
        c.tier = Some(WorkTier::Sleep);
        assert_eq!(
            step(PetState::Eating, &Trigger::EatingFinished, &c),
            Some(PetState::Sleep)
        );
    }

    #[test]
    fn eating_to_tired_when_tier_is_tired() {
        let mut c = ctx();
        c.tier = Some(WorkTier::Tired);
        assert_eq!(
            step(PetState::Eating, &Trigger::EatingFinished, &c),
            Some(PetState::Tired)
        );
    }

    #[test]
    fn eating_to_working_when_tier_is_none() {
        let c = ctx();
        assert_eq!(
            step(PetState::Eating, &Trigger::EatingFinished, &c),
            Some(PetState::Working)
        );
    }

    #[test]
    fn eating_to_working_when_tier_is_stretch() {
        let mut c = ctx();
        c.tier = Some(WorkTier::Stretch);
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
                &Trigger::AccumulatedActiveTime(WorkTier::Sleep),
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
        let triggers = tick_active(&mut c, 60, PetState::Working);
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
        let triggers = tick_idle(&mut c, 60, PetState::Working);
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
        c.idle_seconds = u64::from(SPACED_OUT_S - 60); // 14 min
        let triggers = tick_idle(&mut c, 60, PetState::Working);
        assert_eq!(c.idle_seconds(), u64::from(SPACED_OUT_S));
        assert_eq!(
            triggers,
            vec![Trigger::IdleFor(Duration::from_secs(u64::from(
                SPACED_OUT_S
            )))],
            "must emit IdleFor exactly with the configured threshold"
        );
    }

    #[test]
    fn accumulator_resets_on_spaced_out_entry() {
        let mut c = ctx();
        c.active_seconds = 5400; // 90 min
        c.idle_seconds = 900;
        c.tier = Some(WorkTier::Sleep);
        c.on_entered(PetState::SpacedOut);
        assert_eq!(c.active_seconds(), 0);
        assert_eq!(c.idle_seconds(), 0);
        assert_eq!(c.tier_reached(), None);
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
        tick_active(&mut c, 30, PetState::Stretch);
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
        tick_active(&mut c, 60, PetState::Eating);
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
            tick_active(&mut c, 60, PetState::Working);
        }
        assert_eq!(c.active_seconds(), 300);
    }

    #[test]
    fn idle_then_active_clears_idle_but_preserves_active() {
        let mut c = ctx();
        tick_active(&mut c, 60, PetState::Working);
        tick_idle(&mut c, 60, PetState::Working);
        tick_idle(&mut c, 60, PetState::Working);
        assert_eq!(c.active_seconds(), 60);
        assert_eq!(c.idle_seconds(), 120);
        tick_active(&mut c, 60, PetState::Working);
        assert_eq!(c.active_seconds(), 120);
        assert_eq!(c.idle_seconds(), 0);
    }

    // --- Tier emission semantics -------------------------------------------

    #[test]
    fn crossing_stretch_threshold_emits_stretch_tier_once() {
        let mut c = ctx();
        c.active_seconds = u64::from(STRETCH_AT_S - 60);
        let triggers = tick_active(&mut c, 60, PetState::Working);
        assert!(triggers.contains(&Trigger::AccumulatedActiveTime(WorkTier::Stretch)));
        assert_eq!(c.tier_reached(), Some(WorkTier::Stretch));

        // Next tick — still in Stretch tier, should not re-emit.
        let triggers = tick_active(&mut c, 60, PetState::Working);
        assert!(!triggers
            .iter()
            .any(|t| matches!(t, Trigger::AccumulatedActiveTime(_))));
    }

    #[test]
    fn crossing_tired_threshold_emits_tired_tier() {
        let mut c = ctx();
        c.active_seconds = u64::from(TIRED_AT_S - 60);
        c.tier = Some(WorkTier::Stretch);
        let triggers = tick_active(&mut c, 60, PetState::Working);
        assert!(triggers.contains(&Trigger::AccumulatedActiveTime(WorkTier::Tired)));
        assert_eq!(c.tier_reached(), Some(WorkTier::Tired));
    }

    #[test]
    fn crossing_sleep_threshold_emits_sleep_tier() {
        let mut c = ctx();
        c.active_seconds = u64::from(SLEEP_AT_S - 60);
        c.tier = Some(WorkTier::Tired);
        let triggers = tick_active(&mut c, 60, PetState::Working);
        assert!(triggers.contains(&Trigger::AccumulatedActiveTime(WorkTier::Sleep)));
        assert_eq!(c.tier_reached(), Some(WorkTier::Sleep));
    }

    #[test]
    fn tier_does_not_downgrade_after_idle() {
        // Even if idle pauses active accumulator, tier latch stays put until
        // SpacedOut reset.
        let mut c = ctx();
        c.active_seconds = u64::from(TIRED_AT_S);
        c.tier = Some(WorkTier::Tired);
        tick_idle(&mut c, 60, PetState::Tired);
        assert_eq!(c.tier_reached(), Some(WorkTier::Tired));
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
