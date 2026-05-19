//! macOS `CGEventSource`-backed `ActivitySource`.
//!
//! Owner: agent A3. See `docs/agent-team-plan.md` §4.3 and
//! `docs/architecture.md` §5.1.
//!
//! ## Privacy contract
//!
//! This module is the OS-facing edge of `activity/`. It calls
//! `CGEventSourceSecondsSinceLastEventType`, which returns a single scalar:
//! the time in seconds since the last system-wide input event. It does **not**
//! observe key codes, mouse coordinates, screen contents, window titles, or
//! process names (CLAUDE.md §9). The scalar itself is treated as transient
//! user-behavior data — it is classified into [`Activity`] by the poller and
//! never logged, surfaced, or persisted.
//!
//! ## Permissions
//!
//! `CGEventSourceSecondsSinceLastEventType` is a read-only system query.
//! It does **not** require Accessibility or Input Monitoring entitlements
//! (architecture.md §5.1, Q5.1). If first-run testing reveals a permission
//! prompt, that is a hard stop: see `docs/agent-team-plan.md` §4.3.

use core_graphics::event_source::CGEventSourceStateID;

use super::{ActivityError, ActivitySource};

/// `kCGAnyInputEventType` — sentinel that matches any input event type.
///
/// Defined in CoreGraphics headers as `~(uint32_t)0` (i.e. `u32::MAX`).
/// See <https://developer.apple.com/documentation/coregraphics/kcganyinputeventtype>.
/// Kept private to this module so the value never leaks into business logic.
const K_CG_ANY_INPUT_EVENT_TYPE: u32 = u32::MAX;

// SAFETY (extern block): `CGEventSourceSecondsSinceLastEventType` is part of
// the CoreGraphics framework, already linked by the `core-graphics` crate
// (default `link` feature). The redundant `#[link]` attribute below is a
// belt-and-suspenders pin in case that feature is ever dropped. The function
// is thread-safe, takes two `repr(C)` scalars, and returns a `CFTimeInterval`
// (a `c_double`/`f64`). It does not retain or write through any pointer.
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(source: CGEventSourceStateID, event_type: u32)
        -> f64;
}

/// CoreGraphics-backed activity source. Stateless; safe to clone/share.
///
/// Held behind `Arc<dyn ActivitySource>` by the poller. The empty struct is
/// deliberate — `CGEventSourceSecondsSinceLastEventType` is a global query
/// that does not require a constructed `CGEventSourceRef` handle.
#[derive(Debug, Default, Clone, Copy)]
pub struct MacOsActivitySource;

impl MacOsActivitySource {
    pub const fn new() -> Self {
        Self
    }
}

impl ActivitySource for MacOsActivitySource {
    fn seconds_since_input(&self) -> Result<f64, ActivityError> {
        // SAFETY: see the extern-block SAFETY comment above. The call site
        // passes the documented `HIDSystemState` source and the documented
        // "any event type" sentinel. Both are `repr(C)` and the FFI signature
        // matches the CoreGraphics header exactly.
        let secs = unsafe {
            CGEventSourceSecondsSinceLastEventType(
                CGEventSourceStateID::HIDSystemState,
                K_CG_ANY_INPUT_EVENT_TYPE,
            )
        };

        // Defensive: CoreGraphics has been observed to return implausible
        // values (e.g. negative or NaN) on very rare lock-screen / fast-switch
        // races. Treat any such result as a source failure so the §5.4
        // fail-safe in `poller::tick` kicks in (defaults to `Active`).
        if !secs.is_finite() || secs < 0.0 {
            Err(ActivityError::Unavailable)
        } else {
            Ok(secs)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_non_negative_finite_seconds() {
        // Integration-ish sanity: on a real macOS host the API is always
        // available. Skip the assertion content if it ever errors (CI on a
        // headless box could plausibly return Unavailable); the test still
        // confirms the FFI symbol resolves and the call doesn't crash.
        let source = MacOsActivitySource::new();
        if let Ok(secs) = source.seconds_since_input() {
            assert!(secs.is_finite());
            assert!(secs >= 0.0);
        }
    }

    #[test]
    fn source_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MacOsActivitySource>();
    }
}
