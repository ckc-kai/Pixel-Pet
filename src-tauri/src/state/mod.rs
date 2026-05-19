//! State machine — see `docs/architecture.md` §3.
//!
//! Phase 0 defines the public types here (`states.rs`). Agent A2 owns the
//! transition table and `step` function in `machine.rs`
//! (`docs/agent-team-plan.md` §4.2).

pub mod machine;
pub mod states;

pub use states::{Ctx, MealKind, PetState, Trigger};
