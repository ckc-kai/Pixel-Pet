//! Activity poller — periodic query of the [`ActivitySource`] with sleep-mode
//! backoff and the §5.4 fail-safe (errors default to [`Activity::Active`]).
//!
//! Owner: agent A3. See `docs/agent-team-plan.md` §4.3.
//!
//! ## Design
//!
//! Two pure building blocks plus one async driver:
//!
//! - [`effective_interval`] — given the configured base interval and the
//!   pet's current [`PetState`], computes the next sleep duration. Backs off
//!   to `SLEEP_BACKOFF_MULTIPLIER × base` (capped at
//!   `SLEEP_BACKOFF_MAX_SECONDS`) when the pet is in [`PetState::Sleep`].
//! - [`tick`] — runs one `seconds_since_input` query and applies the §5.4
//!   fail-safe: any error → [`Activity::Active`], with a once-only
//!   `tracing::warn!`.
//! - [`spawn`] — owns the `tokio::sync::watch` channel and the async loop
//!   that strings these together.
//!
//! Splitting the loop body lets unit tests exercise the rules synchronously
//! and avoid timing flakes.
//!
//! ## Privacy
//!
//! The raw `seconds_since_input` value is **user-behavior data** (it implies
//! when the user last typed/clicked). It is converted to a binary
//! [`Activity`] immediately and never logged. Only the coarse failure event
//! "source unavailable" is logged, and only once.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::config::{
    ActivitySettings, POLL_INTERVAL_FLOOR_SECONDS, SLEEP_BACKOFF_MAX_SECONDS,
    SLEEP_BACKOFF_MULTIPLIER,
};
use crate::state::PetState;

use super::{classify, Activity, ActivitySource};

/// Once-only failure latch. Wraps the §5.4 fail-safe so subsequent failures
/// don't spam the log. Test-only `count` field gives tests a cheap way to
/// assert "warn fired exactly N times" without depending on `tracing-test`.
#[derive(Debug, Default)]
pub(crate) struct FailureLatch {
    fired: AtomicBool,
    count: AtomicUsize,
}

impl FailureLatch {
    pub(crate) fn note_failure(&self) {
        // `swap` returns the *previous* value. The first failure sees `false`
        // and is responsible for emitting the warning.
        if !self.fired.swap(true, Ordering::Relaxed) {
            self.count.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                "activity source unavailable; defaulting to Active. \
                 Subsequent failures suppressed."
            );
        }
    }

    /// Reset the latch so the *next* failure logs again. Called whenever the
    /// source successfully recovers, so transient failures stay loud.
    pub(crate) fn note_success(&self) {
        self.fired.store(false, Ordering::Relaxed);
    }

    #[cfg(test)]
    pub(crate) fn warn_count(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }
}

/// Clamp the user-configured poll interval against the system floor.
///
/// The floor protects the §4 CPU budget; it is not overridable by settings
/// (architecture.md §5.2). A1's `Settings::validate` rejects below-floor
/// values up-front; this is a defensive secondary clamp.
pub(crate) fn base_interval_from_setting(configured_seconds: u32) -> Duration {
    let secs = configured_seconds.max(POLL_INTERVAL_FLOOR_SECONDS) as u64;
    Duration::from_secs(secs)
}

/// Compute the next sleep duration given the pet's current state.
///
/// When the pet is in [`PetState::Sleep`], poll less often
/// (`SLEEP_BACKOFF_MULTIPLIER × base`, capped at `SLEEP_BACKOFF_MAX_SECONDS`)
/// per architecture.md §5.2. All other states use the base interval.
pub(crate) fn effective_interval(base: Duration, current_state: PetState) -> Duration {
    if matches!(current_state, PetState::Sleep) {
        let cap = Duration::from_secs(SLEEP_BACKOFF_MAX_SECONDS as u64);
        let scaled = base.saturating_mul(SLEEP_BACKOFF_MULTIPLIER);
        if scaled > cap {
            cap
        } else {
            scaled
        }
    } else {
        base
    }
}

/// Run one query against the source and apply the §5.4 fail-safe.
///
/// Pure given its inputs (no I/O beyond the source call and a possible
/// `tracing::warn!`). Tests drive this directly with `MockSource`.
pub(crate) fn tick(
    source: &dyn ActivitySource,
    threshold: Duration,
    latch: &FailureLatch,
) -> Activity {
    match source.seconds_since_input() {
        Ok(secs) => {
            latch.note_success();
            classify(secs, threshold)
        }
        Err(_) => {
            latch.note_failure();
            Activity::Active
        }
    }
}

/// Spawn the activity polling task.
///
/// Returns the task handle (so the caller can `abort()` on shutdown) plus a
/// `watch` receiver that always carries the latest [`Activity`]. The channel
/// starts at [`Activity::Active`] — same fail-safe stance as §5.4.
///
/// `state_rx` is read each loop iteration to decide whether to back off.
/// `settings` is read once at spawn; live-reloading on settings change is
/// the caller's responsibility (it can `abort()` and respawn).
pub fn spawn(
    source: Arc<dyn ActivitySource>,
    settings: ActivitySettings,
    state_rx: watch::Receiver<PetState>,
) -> (JoinHandle<()>, watch::Receiver<Activity>) {
    let (activity_tx, activity_rx) = watch::channel(Activity::Active);
    let handle = tokio::spawn(run(source, settings, state_rx, activity_tx));
    (handle, activity_rx)
}

async fn run(
    source: Arc<dyn ActivitySource>,
    settings: ActivitySettings,
    state_rx: watch::Receiver<PetState>,
    activity_tx: watch::Sender<Activity>,
) {
    let base = base_interval_from_setting(settings.poll_interval_seconds);
    let threshold = Duration::from_secs(settings.idle_threshold_seconds as u64);
    let latch = FailureLatch::default();

    loop {
        let current_state = *state_rx.borrow();
        let activity = tick(source.as_ref(), threshold, &latch);

        // Channel `send` only errors when every receiver has been dropped —
        // i.e. the app is shutting down. No subscribers means nothing to do.
        if activity_tx.send(activity).is_err() {
            break;
        }

        tokio::time::sleep(effective_interval(base, current_state)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::{ActivityError, ActivitySource};
    use mockall::mock;

    mock! {
        pub Source {}
        impl ActivitySource for Source {
            fn seconds_since_input(&self) -> Result<f64, ActivityError>;
        }
    }

    fn threshold_60s() -> Duration {
        Duration::from_secs(60)
    }

    #[test]
    fn classify_at_threshold_is_active() {
        // Boundary: secs == threshold → Active (architecture.md §5.3 uses
        // strict `>` so the boundary belongs to Active).
        assert_eq!(classify(60.0, threshold_60s()), Activity::Active);
    }

    #[test]
    fn classify_just_above_threshold_is_idle() {
        assert_eq!(classify(60.001, threshold_60s()), Activity::Idle);
    }

    #[test]
    fn tick_classifies_scripted_sequence_over_10_iterations() {
        let scripted = [0.5_f64, 1.0, 30.0, 70.0, 0.0, 5.0, 65.0, 100.0, 10.0, 0.1];
        let mut iter = scripted.into_iter();

        let mut mock = MockSource::new();
        mock.expect_seconds_since_input()
            .times(10)
            .returning(move || Ok(iter.next().unwrap_or(0.0)));

        let latch = FailureLatch::default();
        let got: Vec<Activity> = (0..10)
            .map(|_| tick(&mock, threshold_60s(), &latch))
            .collect();

        // Threshold is 60s; secs strictly greater than 60 → Idle.
        let expected = vec![
            Activity::Active, // 0.5
            Activity::Active, // 1.0
            Activity::Active, // 30.0
            Activity::Idle,   // 70.0
            Activity::Active, // 0.0
            Activity::Active, // 5.0
            Activity::Idle,   // 65.0
            Activity::Idle,   // 100.0
            Activity::Active, // 10.0
            Activity::Active, // 0.1
        ];
        assert_eq!(got, expected);
        assert_eq!(latch.warn_count(), 0, "no failures expected");
    }

    #[test]
    fn source_error_emits_active_and_warns_once_not_ten_times() {
        let mut mock = MockSource::new();
        mock.expect_seconds_since_input()
            .times(10)
            .returning(|| Err(ActivityError::Unavailable));

        let latch = FailureLatch::default();
        for _ in 0..10 {
            // §5.4 fail-safe: on error, default to Active.
            assert_eq!(tick(&mock, threshold_60s(), &latch), Activity::Active);
        }

        assert_eq!(
            latch.warn_count(),
            1,
            "warn must fire exactly once across 10 consecutive failures"
        );
    }

    #[test]
    fn failure_latch_rearms_after_success() {
        // Real-world recovery: source fails (warn), recovers (silent), then
        // fails again — the second failure should re-warn so transient blips
        // stay observable.
        let latch = FailureLatch::default();
        latch.note_failure();
        latch.note_success();
        latch.note_failure();
        assert_eq!(latch.warn_count(), 2);
    }

    #[test]
    fn sleep_backoff_scales_by_multiplier_when_below_cap() {
        let base = Duration::from_secs(10);
        let backed = effective_interval(base, PetState::Sleep);
        assert_eq!(
            backed,
            Duration::from_secs(10 * SLEEP_BACKOFF_MULTIPLIER as u64),
            "Sleep mode uses 5x base when under the cap"
        );
        assert!(
            backed >= base.saturating_mul(SLEEP_BACKOFF_MULTIPLIER),
            "Sleep backoff must be at least 5x base"
        );
    }

    #[test]
    fn sleep_backoff_caps_at_max() {
        // 120s base × 5 = 600s, capped at SLEEP_BACKOFF_MAX_SECONDS (300s).
        let base = Duration::from_secs(120);
        let backed = effective_interval(base, PetState::Sleep);
        assert_eq!(
            backed,
            Duration::from_secs(SLEEP_BACKOFF_MAX_SECONDS as u64)
        );
    }

    #[test]
    fn non_sleep_states_use_base_interval() {
        let base = Duration::from_secs(60);
        for &state in &[
            PetState::Startup,
            PetState::Working,
            PetState::Stretch,
            PetState::Tired,
            PetState::SpacedOut,
            PetState::Eating,
        ] {
            assert_eq!(
                effective_interval(base, state),
                base,
                "{state:?} must not back off"
            );
        }
    }

    #[test]
    fn wake_from_sleep_classifies_active_on_next_tick() {
        // The "wake" path: pet is in Sleep, source reports activity within
        // threshold — classify returns Active. The poller emits Active and
        // the FSM (A2) handles the actual Sleep → Working transition.
        let mut mock = MockSource::new();
        mock.expect_seconds_since_input()
            .times(1)
            .returning(|| Ok(0.5));

        let latch = FailureLatch::default();
        assert_eq!(tick(&mock, threshold_60s(), &latch), Activity::Active);
    }

    #[test]
    fn floor_enforcement_clamps_low_intervals() {
        // poll_interval_seconds below the floor is bumped *up* to the floor.
        // (A1's Settings::validate also rejects this; the clamp here is a
        // defensive second line.)
        let clamped = base_interval_from_setting(1);
        assert_eq!(
            clamped,
            Duration::from_secs(POLL_INTERVAL_FLOOR_SECONDS as u64)
        );

        // Above-floor settings pass through unchanged.
        let passthrough = base_interval_from_setting(60);
        assert_eq!(passthrough, Duration::from_secs(60));
    }

    #[test]
    fn mock_source_and_arc_dyn_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockSource>();
        assert_send_sync::<Arc<dyn ActivitySource>>();
    }
}
