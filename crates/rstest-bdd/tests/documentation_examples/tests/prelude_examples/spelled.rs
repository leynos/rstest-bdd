//! The users' guide's spelled-out prelude example, compiled as written.
//!
//! The import below is character-identical to the fenced example marked
//! `prelude-spelled-import` in `docs/users-guide.md`. Spelling the glob out is
//! what proves each advertised name resolves at the crate root, so this module
//! exists to make the compiler check the guide's list rather than the prelude's
//! own `pub use` lines.
#![expect(unused_imports, reason = "resolution is the property under test")]

use rstest_bdd::{
    ScenarioState,
    Slot,
    StepArgs,
    StepContext,
    StepError,
    StepResult,
    given,
    scenario,
    scenarios,
    then,
    when,
};
