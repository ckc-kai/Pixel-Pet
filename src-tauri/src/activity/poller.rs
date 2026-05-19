//! Activity poller — periodic query of the `ActivitySource` with sleep-mode
//! backoff and the §5.4 fail-safe (errors default to `Active`).
//!
//! Owner: agent A3. See `docs/agent-team-plan.md` §4.3.
