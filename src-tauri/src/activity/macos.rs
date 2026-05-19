//! macOS `CGEventSource`-backed `ActivitySource`.
//!
//! Owner: agent A3. See `docs/agent-team-plan.md` §4.3 and
//! `docs/architecture.md` §5.1. Use `core-graphics::event_source::CGEventSource`
//! with `CGAnyInputEventType`. Wrap any FFI in a single safe function with a
//! `// SAFETY:` comment.
