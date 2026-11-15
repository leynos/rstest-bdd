//! A curated set of imports for common BDD step definitions and scenarios.
//!
//! The prelude is a subset of the [`rstest_bdd`] crate root. It adds no names
//! that are unavailable at the root, so users can choose this module for a
//! glob import while keeping the full crate API available separately.
//!
//! `ScenarioState` and `StepArgs` each name a trait and a derive macro. Rust
//! keeps those items in separate namespaces, so one import can bring both
//! halves into scope, as with `serde::Serialize` and its derive macro.
//!
//! The inert parameter marker `#[harness_context]` needs no import. A step
//! macro reads it from the parameter's attribute syntax.
//!
//! # Example
//!
//! A prelude glob supplies the step attributes, scenario macros, state and
//! capture types, and the runtime context used by a step:
//!
//! ```
//! use rstest_bdd::prelude::*;
//!
//! #[derive(Default, ScenarioState)]
//! struct World {
//!     value: Slot<u32>,
//! }
//!
//! #[derive(StepArgs)]
//! struct CapturedValue {
//!     value: u32,
//! }
//!
//! #[given("the manual async step runs")]
//! fn manual_given() {}
//!
//! #[when("the manual async step continues")]
//! fn manual_when() {}
//!
//! #[then("the manual async step completes")]
//! fn manual_then() {}
//!
//! #[scenario(path = "tests/features/manual_async_scenario.feature")]
//! fn manual_scenario() {}
//!
//! #[given("a precondition")]
//! fn precondition() {}
//!
//! #[when("an action occurs")]
//! fn action() {}
//!
//! #[when("an action occurs with {num}")]
//! fn action_with_number(num: i32) { let _ = num; }
//!
//! #[then("events are recorded")]
//! fn record_events() {}
//!
//! scenarios!("tests/features/auto");
//!
//! let mut world = World::default();
//! world.value.set(1);
//! world.reset();
//! assert_eq!(world.value.get(), None);
//! assert_eq!(CapturedValue::FIELD_COUNT, 1);
//!
//! let cell = StepContext::owned_cell(1_u32);
//! let mut context = StepContext::default();
//! context.insert_owned_harness_context::<u32>(&cell);
//! *context
//!     .borrow_harness_context_mut::<u32>()
//!     .expect("harness context should be available") += 1;
//! assert_eq!(
//!     *context
//!         .borrow_harness_context::<u32>()
//!         .expect("harness context should remain available"),
//!     2,
//! );
//!
//! let result: StepResult<(), StepError> = Ok(());
//! assert!(result.is_ok());
//! ```

pub use rstest_bdd_macros::{ScenarioState, StepArgs, given, scenario, scenarios, then, when};

pub use crate::{
    StepError,
    StepResult,
    context::StepContext,
    state::{ScenarioState, Slot},
    step_args::StepArgs,
};
