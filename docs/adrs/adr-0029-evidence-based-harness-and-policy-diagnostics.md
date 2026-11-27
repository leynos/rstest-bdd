# Architectural decision record (ADR) 0029: evidence-based harness and attribute-policy diagnostics

## Status

Accepted (2026-11-27): diagnose non-canonical first-party paths, missing
attribute policies, conflicting framework selectors, a missing executor, and the
runtime alias combined with a harness, each with a stable code, the consequence
that actually occurred, and a fix that removes the finding.

## Date

2026-11-27.

## Context and problem statement

`#[scenario]` and `scenarios!` let a test author choose a harness adapter
(`harness = ...`) and an attribute policy (`attributes = ...`). Procedural
macros cannot call trait methods, so the macros recognize first-party adapters
by the *spelling* of the supplied path. When the spelling is not the one they
expect, the generated test quietly changes shape: `#[gpui::test]` disappears, or
the build fails with an error that names neither the cause nor the fix.

ADR-0017 introduced one diagnostic for this family: a path that retained the
canonical adapter-crate identifier was reported, and the generated code fell
back to base harness API types. Three gaps remained:

- the message offered a remedy that never removed the warning (adding a
  `rstest-bdd-harness` dev-dependency makes code build, but the fallback check
  never consults that dependency);
- the shape the roadmap's fix text describes — a first-party harness whose
  attribute policy was simply omitted — was silent whenever the path was
  recognized at all;
- two explicit selectors that name different frameworks, a missing executor,
  and the deprecated alias combined with a harness all failed obscurely.

## Decision drivers

- Diagnose only what the token stream proves. The macros can act on syntax, not
  on trait implementations.
- Every message names a consequence that really occurred and a fix that removes
  the finding; a workaround belongs in the guide, not in the message.
- Keep the change additive: a program that compiles today must still compile,
  and every configuration that is valid stays silent.
- Keep ADR-0017's channel and ownership contract: pure resolution, one emission
  per expansion boundary, stable markers on stable, native diagnostics on
  nightly.
- Give each diagnostic a stable, searchable code.

## Decision outcome

Five diagnostics, each prefixed with a stable code:

| Code | Fires when |
| ---- | ---------- |
| W001 | a `harness` or `attributes` path has first-party evidence but is not recognized |
| W002 | an evidenced harness has no `attributes = ...` and the canonical spelling would emit an attribute the expansion does not |
| W003 | the harness selection and the explicit policy name different frameworks |
| E001 | an `async fn` scenario has no executor: no policy adds one and no harness provides one |
| E002 | `scenarios!` combines `runtime = "tokio-current-thread"` with a harness |

*Evidence* is three things together: the terminal segment is a first-party
adapter type name, the adapter crate's identifier appears anywhere below the
root segment, and the crate is a direct dependency of the caller. A custom type
such as `custom::TokioHarness` therefore carries no evidence and stays silent,
which ADR-0008 requires: harness-only third-party usage is valid.

`W002` is rendered in place of the harness slot's `W001`, while the underlying
`W001` remains a finding. That is what makes both of `W002`'s remedies
converge: adding the policy restores the attribute and leaves a warning that
was already reported, rather than revealing a new one.

`E001` and `E002` are errors because each fires only on a configuration that
already fails to compile, so a clearer error rejects nothing new. `E002`
replaces both the per-scenario "combining `harness` with `async fn`" errors and
the nightly "redundant" warning that described a fatal configuration as merely
redundant. When any error fires at a boundary, that boundary emits no warnings:
rustc's handling of lints beside a `compile_error!` varies, and both messages
already carry the fix where one applies.

Every fix is modelled as a transformation on the diagnostic state, and a lemma
(a fix never introduces a new finding) is checked exhaustively over the finite
state space. It falsified two designs during implementation: removing
`attributes` from a conflicting state whose harness is non-canonical, and
clearing the runtime alias from the same shape, each land in `W002`'s
antecedent. Both remedies are therefore composite — they also canonicalize the
harness — and the messages say so.

The messages never claim an attribute "was not applied" when the user wrote one.
The generator deduplicates against user attributes, so a user-written executor
attribute suppresses that claim and `W002` entirely.

The stable channel is unchanged from ADR-0017: a generated deprecated unit
struct whose note carries the message, one headline name per code. The nightly
channel is `proc_macro::Diagnostic`. The two are mutually exclusive. The
documented opt-out for `-D warnings` users is a module-level
`#[allow(deprecated)]`, because the marker is a sibling item of the generated
test that a function-level allow cannot reach; nightly native warnings cannot
be suppressed.

### The ADR-0008 prose departure

ADR-0008 says the `runtime` alias does not change attribute selection. In code,
the alias selects the Tokio hint when no first-party harness is recognized.
Every configuration in which that departure is observable is now `E002`, so
preserving the code changes nothing visible. This record notes the departure
rather than restating ADR-0008.

## Rationale

The macros can act only on what the tokens prove, so evidence is the honest
basis for a diagnosis. Rendering a diagnostic only when a consequence occurred
keeps the warnings trustworthy: a warning that names a consequence it did not
cause teaches users to ignore warnings.

The precedence order is implemented once, in `selected_hint`, and both code
generation and diagnostics call it. Two encodings could drift, and a diagnostic
that described a different precedence from the one the generated test used
would be worse than none.

Keeping fatal errors on `proc-macro-error3` and diagnostics in one module
preserves the existing fatal-error boundary while giving the diagnostic family
a single owner.

## Options considered

### Warn on every third-party harness without an attribute policy

Rejected. ADR-0008 declares harness-only usage valid; the repository's own
tests use synchronous harnesses that need no attribute. Under `-D warnings` that
warning would be a build failure for a patch upgrade. The only fix such a
warning could offer is a custom policy, which the macros cannot evaluate.
Roadmap 12.2.3 owns the policy-extension question and may supersede this rule.

### Keep the ADR-0017 message and add no new codes

Rejected. The message's remedy does not remove the warning, and it cannot
describe a missing policy or a conflict, because it fires on a different
condition.

### Report per-scenario errors for the alias-plus-harness case

Rejected. The alias is a property of the `scenarios!` invocation, not of each
generated test, and the existing per-scenario messages describe the symptom
rather than the cause.

## Consequences

- `codegen::policy_diagnostics` is the only emitter of harness and
  attribute-policy diagnostics, and its `selected_hint` is the only
  implementation of the precedence.
- Only the two expansion boundaries call `emit_diagnostics`; a generated
  scenario must never re-emit, because nightly emission is a side effect.
- Users who build with `-D warnings` or `#![deny(deprecated)]` see a build
  failure for the newly warned configurations; the users' guide documents the
  opt-out and the codes.
- Adding a finding means adding a model case, its fixes, its message, its
  stable marker, an `insta` wording case, and one stable and one nightly
  representative.

## Verification coverage

The model's domain is finite (294 states), so the silence, disjointness,
cardinality and fix-convergence lemmas are checked exhaustively in the unit
suite. Path classification is property-tested over an unbounded path space,
with named cases for custom, renamed, nested and unresolvable paths. A witness
table compares the real generated attributes with the model across both runtime
modes and both signature shapes. `trybuild` pins one representative per code and
channel, including the spans; an `insta` snapshot pins every wording variant.

## References

- [ADR-0004](adr-0004-policy-crate.md): the policy crate's shared vocabulary.
- [ADR-0008](adr-0008-harness-led-attribute-policy-defaults.md): harness-led
  attribute defaults.
- [ADR-0017](adr-0017-toolchain-aware-adapter-fallback-diagnostics.md): the
  channel and ownership contract this record extends.
- [Design: harness adapters and attribute policies][design-harness].
- [Users' guide: harness and attribute-policy diagnostics][users-diagnostics].
- [Developers' guide: policy diagnostics][developers-diagnostics].

[design-harness]: ../rstest-bdd-design.md#27-harness-adapters-and-attribute-policy-plugins-adr-0025-adr-0007
[users-diagnostics]: ../users-guide.md#harness-and-attribute-policy-diagnostics
[developers-diagnostics]: ../developers-guide.md#policy-diagnostics