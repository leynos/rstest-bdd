# Architectural decision record (ADR) 016: consolidate on the `tracing` logging facade

## Status

Accepted (2026-08-16): every workspace crate emits diagnostics through
`tracing`. `rstest-bdd` retains a `log` dependency solely to detect whether a
`log` listener is installed, and enables tracing's `log` compatibility feature
so consumers of either facade keep receiving warnings.

## Date

2026-08-16.

## Context and problem statement

The workspace used two logging facades at once. `rstest-bdd` emitted through
`log`, while `rstest-bdd-server` used `tracing` with `tracing-subscriber`.
Issue [#386][i386] recorded this as a deliberate split along the conventional
line: libraries prefer `log` because it is lighter, and async servers prefer
`tracing` because it carries span context.

That rationale no longer described the code. `tracing` had already reached the
downstream-facing library layer:

- `rstest-bdd-harness` depends on `tracing` and re-exports it publicly, so the
  facade is part of that crate's API surface.
- `rstest-bdd-macros` generates `tracing::error!` calls into user test
  binaries, so any suite using a harness links `tracing` whether or not it
  asked for it.
- `rstest-bdd-harness-gpui` emits `tracing` events and captures them in tests.

The split was therefore not "one facade per tier" but two facades in the same
downstream dependency graph, with `rstest-bdd` the only crate on the older one.

Dependency weight did not favour either choice. `log` reaches `rstest-bdd`'s
graph transitively through `i18n-embed`, and `tracing` reaches
`rstest-bdd-server`'s graph transitively through `async-lsp` and `tower`, so
neither facade disappears from a build whichever way the decision goes.

## Decision drivers

- A test framework's diagnostics must remain visible to consumers who have
  configured no logging at all, which is the common case.
- Consumers already running a `log` logger must not silently lose warnings.
- The choice should not force a breaking change on a published API.
- Span context is wanted for the async scenario runtime introduced in
  [#384][pr384], and `log` cannot express it.

## Decision outcome

`rstest-bdd` emits warnings through `tracing::warn!`. The two former `log`
call sites — the ambiguous step-return override in `StepContext::insert_value`
and the specificity-calculation failure in the step registry — now use
`tracing`.

`rstest-bdd` enables tracing's `log` feature. When no `tracing` subscriber has
ever been installed, tracing's macros emit a `log` record instead, so a
consumer running `env_logger` or a similar logger keeps receiving warnings
unchanged.

Warning delivery for step-context diagnostics is owned by the private
`context::warnings` module. It emits the event and decides whether to mirror
the message to stderr, probing both delivery routes:

Table: Warning delivery routes and the stderr fallback

| `tracing` subscriber | `log` logger | Warning delivered by     | Stderr mirror |
| -------------------- | ------------ | ------------------------ | ------------- |
| Records `WARN`       | Any          | The subscriber           | No            |
| None ever installed  | Installed    | Tracing's `log` bridge   | No            |
| Filters `WARN` out   | None         | Nothing                  | Yes           |
| None ever installed  | None         | Nothing                  | Yes           |

The probe and the emitting macro are colocated so both resolve the same
`module_path!()` target and are subject to identical filtering.

## Rationale

Consolidating on `tracing` rather than on `log` is the only direction that
breaks nothing. `rstest-bdd-harness` re-exports `tracing` publicly and the
macros generate calls against that re-export, so moving the workspace to `log`
would be a breaking change to a published API and would discard the structured
logging the language server already relies on.

Enabling the `log` compatibility feature is what makes the migration safe for
existing consumers rather than merely convenient for the workspace. Without it,
a suite running `env_logger` would stop seeing these warnings through its
logger. The feature costs nothing in dependency terms because `log` is already
in the graph.

Retaining a direct `log` dependency in `rstest-bdd` is deliberate and narrow.
It is not used to emit anything; it answers one question — whether a `log`
listener exists — which the stderr fallback must know to avoid either printing
a duplicate or swallowing the warning. Tracing's bridge fires only while no
subscriber has ever been set, and that condition is not observable through
`tracing` alone.

The stderr mirror fires when a subscriber filters `WARN` out and no `log`
logger is present. Preferring a redundant line to a silently dropped
correctness warning matches the previous behaviour, where a `log` logger set to
a level above `warn` also produced the mirror.

## Options considered

### Keep both facades and document the split

Rejected. It leaves two facades in the same downstream graph and leaves the
stated rationale contradicting what the harness crates already do. It also
leaves `rstest-bdd` unable to attach span context to scenario diagnostics.

### Consolidate on `log`

Rejected. `rstest-bdd-harness` re-exports `tracing` as public API, so this
would be a breaking change for third-party harness adapters. It would also
remove span support from the language server, whose `async-lsp` and `tower`
stack emits `tracing` events regardless of what the workspace chooses.

### Drop `log` entirely and rely only on the stderr fallback

Rejected. Consumers running a `log` logger would stop receiving these warnings
through it. They would still see the message on stderr, but routing a
diagnostic away from a logger the consumer deliberately configured is a
regression this consolidation should not impose.

### Enable tracing's `log-always` feature instead

Rejected. `log-always` emits a `log` record even when a subscriber is active,
so a consumer running both a subscriber and a `log` logger would see every
warning twice.

## Consequences

- `rstest-bdd` gains a direct `tracing` dependency and keeps `log` as a
  listener probe only. New emission sites must use `tracing`.
- Consumers with a `tracing` subscriber now receive these warnings natively,
  with span context available.
- Consumers with only a `log` logger are unaffected.
- The `attributes` default feature of `tracing` is left enabled. It pulls
  `syn` and `quote`, both already present in any consumer's graph through
  `rstest-bdd-macros`, so disabling it would not reduce the build.
- Warning delivery is testable without process-global state: the tests install
  scoped subscribers, so they behave identically under `cargo test` and
  cargo-nextest.

## References

- Issue [#386][i386]: consider consolidating logging facades.
- Pull request [#384][pr384]: raised the split while adding the Tokio
  current-thread scenario runtime.
- [ADR 015: name the step-return override outcome][adr-015] covers the
  ambiguous-override outcome whose warning this decision reroutes.
- [Developers' guide observability guidance](developers-guide.md#observability-guidance)
  records the emission conventions for harness authors.

[adr-015]: adr-015-insert-outcome-for-step-return-overrides.md
[i386]: https://github.com/leynos/rstest-bdd/issues/386
[pr384]: https://github.com/leynos/rstest-bdd/pull/384
