//! Compile-fail fixture: `#[harness_context]` on a function with no step
//! attribute.
//!
//! The marker is inert: a step attribute consumes it and supplies the
//! parameter from the scenario's harness context. Nothing consumes it here, so
//! rustc reports an attribute it cannot resolve. Put the marker on a step
//! function instead.

fn records_count(#[harness_context] count: &mut u32) {
    *count += 1;
}

fn main() {}