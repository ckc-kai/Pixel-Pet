//! Activity detection — see `docs/architecture.md` §5.
//!
//! **Privacy contract** (CLAUDE.md §9): this module answers exactly one
//! question — "has the user been active recently?". No key codes, no mouse
//! coords, no screen capture, no window/process introspection. Ever.
//!
//! Phase 0 defines `Activity`, `ActivitySource`, and the pure `classify`
//! function. Agent A3 (`docs/agent-team-plan.md` §4.3) implements the macOS
//! source and the poller loop.

use std::time::Duration;

#[cfg(target_os = "macos")]
pub mod macos;
pub mod poller;

/// Binary activity classification — the only thing this module exposes about
/// the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Activity {
    Active,
    Idle,
}

/// Failures from the underlying OS query. Detail is intentionally coarse —
/// the fail-safe always defaults to `Active` (architecture.md §5.4), so
/// callers never branch on a specific reason.
#[derive(Debug, thiserror::Error)]
pub enum ActivityError {
    #[error("activity source unavailable")]
    Unavailable,
}

/// Abstraction over the OS query so the poller can be unit-tested with a
/// scripted mock (`mockall` is pre-installed for A3).
pub trait ActivitySource: Send + Sync {
    /// Seconds since the most recent system-wide input event.
    ///
    /// Implementations must not log, surface, or persist anything beyond this
    /// scalar — see privacy contract above.
    fn seconds_since_input(&self) -> Result<f64, ActivityError>;
}

/// Pure classifier — branched out so it is trivially unit-testable.
pub fn classify(secs_since_input: f64, threshold: Duration) -> Activity {
    if secs_since_input > threshold.as_secs_f64() {
        Activity::Idle
    } else {
        Activity::Active
    }
}
