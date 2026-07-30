# Architectural decision record (ADR) 015: name the step-return override outcome

## Status

Accepted (2026-07-30): `StepContext::insert_value` returns a `#[must_use]`
`InsertOutcome` enum naming all three results, rather than
`Option<Box<dyn Any>>`. `into_previous()` recovers the displaced override for
callers that only need the old shape.

## Date

2026-07-30.

## Context and problem statement

A step function may return a value that overrides the fixture of the same type
for the remainder of the scenario. The scenario runner records that override
through `StepContext::insert_value`, which returned `Option<Box<dyn Any>>`.

That signature conflated three outcomes behind one `None`:

- the value was recorded and displaced no earlier override;
- no fixture had the value's type, so the value was dropped;
- more than one fixture had the value's type, so the override would have been
  ambiguous and the value was dropped.

Only the first is success. The second and third silently discard a step's
return value, which is precisely the situation a test author needs to hear
about — a step that appears to publish state, but does not. Issue [#514][i514]
recorded that a caller could not distinguish them, and that the `Some(_)` case
carried the only unambiguous signal.

The runner itself does not need the distinction: it drops the previous value
either way. The problem is that the type offered no way for anyone else to ask.

## Decision drivers

- A silently dropped step return is a defect that should be diagnosable at the
  call site, not only through a log line.
- The runner's own call must stay a single expression; this is generated code
  and should not grow a match.
- Existing direct callers should have a mechanical migration, not a redesign.
- The public surface should not leak `Box<dyn Any>` handling into every caller
  that only wants to know whether the insert took effect.

## Decision outcome

`insert_value` returns `InsertOutcome`:

- `Inserted(Option<Box<dyn Any>>)` — a fixture uniquely matched the type and the
  override was recorded. The payload is the displaced override, or `None` when
  it displaced nothing.
- `NoMatch` — no fixture uses the value's type; the value was dropped.
- `AmbiguousIgnored` — several fixtures match; the value was dropped rather than
  bound to an arbitrary one.

The enum is `#[must_use]`, so discarding it implicitly warns. The generated
scenario runner keeps its single expression by binding explicitly with
`let _ = ctx.insert_value(…)`, which states that the outcome is knowingly
ignored at the one call site where dropping is correct.

Two accessors keep ordinary callers off the enum:

- `into_previous()` consumes the outcome and yields the displaced override,
  returning `None` for both dropped cases. This is exactly the old
  `Option<Box<dyn Any>>` result, so a caller that only wanted the previous value
  migrates by appending one call.
- `is_inserted()` reports whether the override was recorded, without consuming
  the outcome.

## Rationale

The three outcomes are facts about what happened, so the return type should name
them. Encoding them as an enum makes the dropped cases impossible to overlook by
accident while leaving the successful case's payload exactly where it was.

`#[must_use]` is the part that changes behaviour for existing code: a caller who
previously wrote `ctx.insert_value(v);` and ignored the result now gets a
warning. That is the intended pressure — the ignored result is the diagnostic.
The explicit `let _ =` in generated code is the escape hatch, used once, where
the runner has already decided that dropping is correct.

Returning the displaced value inside `Inserted` rather than as a separate
accessor keeps the success case honest: there is no way to observe a previous
override without first establishing that the insert happened.

## Options considered

### Keep `Option<Box<dyn Any>>` and log the dropped cases

Rejected. Logging is what the code already did, and issue [#514][i514] exists
because it was not enough: a log line is invisible to a caller and to a test
assertion. It also leaves the type lying about how many outcomes there are.

### Return `Result<Option<Box<dyn Any>>, InsertError>`

Rejected. Neither dropped case is an error the caller can handle or propagate —
they are diagnostic outcomes of a deliberate policy. Modelling them as `Err`
invites `?` at call sites that should not abort a scenario, and forces error
plumbing on a method that cannot fail.

### Return a boolean plus an out-parameter for the previous value

Rejected. It splits one outcome across two values, permits the invalid
combination of `false` with a displaced override, and reads worse than the
enum at every call site.

### Distinguish only "inserted" from "not inserted"

Rejected. It collapses `NoMatch` and `AmbiguousIgnored`, which have different
causes and different fixes: the first means the type is not a fixture at all,
the second means the suite needs to disambiguate which fixture is meant.

## Consequences

- Direct callers of `insert_value` must update. Those that only wanted the
  displaced override append `into_previous()`; those that care about the
  distinction match the enum. The v0.6.0 migration guide records both paths.
- Ignoring the result now warns, which will surface call sites that were
  discarding a step return without meaning to.
- Generated scenario code is unaffected in behaviour; the macros emit the
  explicit `let _ =` binding.
- The dropped cases remain dropped. This ADR changes what a caller can observe,
  not the runtime policy for ambiguous or unmatched types; changing that policy
  would be a separate decision.

## References

- Issue [#514][i514]: `StepContext::insert_value` overloaded `None` return.
- [ADR 012: guard-based `StepContext` borrowing][adr-012] covers the related
  borrowing redesign for the same type.
- [v0.6.0 migration guide][migration] records the caller-facing upgrade steps.

[adr-012]: adr-012-guard-based-stepcontext-borrowing.md
[i514]: https://github.com/leynos/rstest-bdd/issues/514
[migration]: v0-6-0-migration-guide.md
