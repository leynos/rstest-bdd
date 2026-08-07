# ExecPlan 10.1.4: emit the scenario name when a GPUI scenario fails

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
and `Outcomes & retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item 10.1.4 closes the last quick-win gap exposed during the first
downstream beta migration of `rstest-bdd` 0.6.0-beta: when a step panics inside
a GPUI scenario, the failure currently surfaces as a raw panic with no scenario
name attached. A developer reading CI output cannot tell which `.feature`
scenario actually failed without cross-referencing test names by hand. This
defeats the purpose of writing Behaviour-Driven Development (BDD) scenarios
with human-readable titles.

After this work, any panic raised from a step running under `GpuiHarness` will
also carry the originating feature path, scenario name, and feature-file line
number into both standard error (stderr) and the resumed panic payload, so the
scenario name appears next to libtest's "test … FAILED" footer and in captured
nextest output. A feature-gated regression test in the
`rstest-bdd-harness-gpui` crate deliberately panics from inside a scenario and
asserts the augmented diagnostic, satisfying the roadmap's "failing-harness
regression" finish line.

Success is observable when:

1. Running the new feature-gated regression binary as
   `cargo test -p rstest-bdd-harness-gpui --features native-gpui-tests --test scenario_name_in_logs`
   passes. That binary captures the resumed panic payload of a deliberately
   failing scenario and asserts that the feature path, scenario name, and
   scenario line number all appear in the augmented diagnostic.
2. `make check-fmt`, `make lint`, `make test`, and `make markdownlint` all
   exit zero.
3. `docs/roadmap.md` item 10.1.4 is checked off only after items 1 and 2
   pass and CodeRabbit review concerns are cleared.

Implementation was authorized by the user on 2026-05-24; proceed
milestone-by-milestone within the tolerances below.

## Constraints

- Implement only roadmap item 10.1.4. Do not implement 10.2 documentation
  expansion or 11.x borrow-helper work beyond minor corrections that keep
  cross-references accurate.
- Preserve public trait contracts. Do not change `HarnessAdapter`,
  `HarnessError`, `HarnessErrorContext`, `ScenarioRunRequest`, `ScenarioRunner`,
  `ScenarioMetadata`, the `#[scenario]` or `scenarios!` attributes, or the
  `rstest_bdd_harness_context` reserved fixture key.
- Keep GPUI integration tests feature-gated behind
  `rstest-bdd-harness-gpui/native-gpui-tests`, mirroring 10.1.3.
- Augmentation must live inside `GpuiHarness`: no changes to
  `crates/rstest-bdd-harness/`, `crates/rstest-bdd-harness-tokio/`, or
  `crates/rstest-bdd-macros/` are warranted, because only GPUI's `run_test`
  envelope swallows step panics in a way that erases scenario context.
- Do not install a process-wide `panic::set_hook` for scenario context.
  Process-global hooks race with parallel `cargo test` threads on the same
  process and may collide with upstream GPUI's own hook in the future. Use a
  closure-local `panic::catch_unwind` + `panic::resume_unwind` instead.
- Do not store scenario name in thread-local statics other than where the
  existing 10.1.3 patterns already do so. Each call to `GpuiHarness::run` must
  carry its own scenario name through stack-local storage only.
- The augmented diagnostic must not silently transform the panic into `Ok`;
  failures must still propagate. `gpui::run_test` already calls
  `panic::resume_unwind` after retries; the harness must do the same after
  augmentation.
- Do not introduce new external dependencies. The change must compile against
  the existing `Cargo.toml` for `rstest-bdd-harness-gpui`.
- Avoid `unsafe`. The work needs only safe `std::panic` primitives.
- Use `rstest` for unit-style helper tests, `rstest-bdd` itself for any
  behavioural coverage, and `proptest` only if a property emerges that warrants
  it. The roadmap entry expects a regression test, not a proof.
- All step-panic diagnostics must use British English with Oxford spelling
  in user-facing messages (Oxford `-ize`, e.g. "behaviour", "initialize",
  "organize"), in line with `en-gb-oxendict` policy.
- Run validation commands sequentially. Capture each command's output to
  `/tmp/<action>-rstest-bdd-${BRANCH}.out` via `tee`.
- Commit each milestone only after its quality gates pass and any requested
  CodeRabbit findings are resolved.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation requires changes to more than
  six files or 350 net lines, excluding generated lockfile noise. The target
  shape is one focused edit to `gpui_harness.rs`, one new test binary, one new
  feature file, and at most one doc-comment touch-up each in `users-guide.md`
  and `developers-guide.md`.
- Interface: stop and escalate if satisfying the requirement appears to need
  a new public type, a new variant on `HarnessError`, a change to
  `ScenarioRunRequest`/`ScenarioRunner`, or any other public-API mutation.
- Vendored shim: stop and escalate if `vendor/gpui/src/lib.rs` must grow new
  surface to support the regression. The current `run_test`/`TestAppContext`
  shim is sufficient; if the augmentation appears to require a new shim API,
  redesign the augmentation instead.
- Dependencies: stop and escalate before adding any new crate or system
  package requirement.
- Process-wide hooks: stop and escalate if any solution proposes mutating
  `std::panic::set_hook` globally. Re-route through the closure-local
  `catch_unwind` approach instead.
- Iterations: stop and escalate if the same validation gate fails three
  consecutive fix attempts.
- Ambiguity: stop and present options if the assertion target ("scenario
  name appears in emitted diagnostics") can only be satisfied by either
  capturing stderr in a way `cargo test` cannot reproduce or by depending on a
  panic-message format that may legitimately change.

## Risks

- Risk: nextest and `cargo test` libtest both capture stderr per test, but
  the capture context can be torn down mid-unwind, hiding any `eprintln!`
  emitted only after `catch_unwind` returns. Severity: medium. Likelihood:
  medium. Mitigation: emit the scenario marker *before* invoking the inner
  runner (so the marker reaches the captured buffer regardless of where the
  panic surfaces) and again *after* `catch_unwind` recovers the payload but
  before `panic::resume_unwind`. The regression test asserts on both positions.

- Risk: a deliberately panicking step inside a feature-gated GPUI scenario
  could destabilize the test process if any teardown path (e.g. the
  `serial_test` fixture in `stateful_window.rs`) leaks state into a later
  scenario. Severity: medium. Likelihood: low. Mitigation: keep the new
  regression in its own test binary (`scenario_name_in_logs.rs`) so its
  `#[serial]` ordering does not interfere with `stateful_window.rs`, and ensure
  the regression's intentional panic only occurs inside a runner that the test
  asserts on via `catch_unwind`. Do not rely on the `#[scenario]` macro's
  generated runner for the failing path; build a hand-rolled
  `ScenarioRunRequest`/`ScenarioRunner` so the panic crosses exactly the
  `GpuiHarness::run` boundary the change targets.

- Risk: GPUI's `run_test` retries up to `max_retries` times by catching
  panics. Today the harness sets `max_retries = 0`, but if a future change
  raises it, the augmented marker could fire for each retry and confuse
  readers. Severity: low. Likelihood: low. Mitigation: include an attempt
  counter (or note that the harness currently runs at most once) in the marker,
  and document the behaviour in the harness module-level comment so future
  maintainers can update both call sites together.

- Risk: GPUI panic payloads in the wild can be any `Send + Any` value, not
  just `&str`/`String`. Severity: low. Likelihood: low. Mitigation: reuse the
  workspace `rstest_bdd::panic_message` helper to render the payload
  defensively, falling back to a type-id-only message for opaque payloads. Do
  not duplicate the downcast ladder.

- Risk: the augmented payload becomes a `Box<String>`, which downstream
  readers (other test runners, debuggers) may treat differently from the
  original `&'static str` payload. Severity: low. Likelihood: low. Mitigation:
  preserve the original payload by `resume_unwind`-ing the unaltered
  `Box<dyn Any + Send>` returned by `catch_unwind`. The augmentation surfaces
  through `eprintln!`/`tracing::error!` only; the panic payload is not
  rewritten. This avoids the rust-lang/rust #86027 panic-on-drop footgun and
  keeps the diagnostic plain.

- Risk: documentation drift from the new behaviour into wider GPUI playbook
  changes claimed by 10.2.1. Severity: low. Likelihood: medium. Mitigation:
  limit user-guide and developer-guide edits to the smallest cross-reference
  needed for 10.1.4 (one paragraph each, maximum), and leave the wider playbook
  to 10.2.1.

## Skills and documentation signposts

Use these skills while implementing this plan:

- `leta`: semantic Rust navigation. Already loaded.
- `rust-router`: route Rust design issues. Already loaded.
- `rust-errors`: when deciding what (if anything) belongs in `HarnessError`.
  Expect to confirm "nothing": step panics are not harness initialization
  failures.
- `rust-unused-code`: if a feature-gated test triggers `dead_code`/`unused`
  warnings, prefer `#[cfg(...)]` scoping over `#[allow(...)]`.
- `arch-crate-design`: confirm the augmentation belongs in
  `rstest-bdd-harness-gpui`, not the base harness.
- `nextest`: ensure the regression runs under both libtest and nextest, and
  that captured stderr is observable in nextest's failure summary.
- `execplans`: keep this plan current as a living document.
- `commit-message` and `pr-creation`: milestone commits and pull-request
  metadata.
- `en-gb-oxendict`: copy review of any user-facing strings introduced by
  the change.
- `code-review` and `simplify`: post-implementation self-review before the
  CodeRabbit pass.

Read these repository documents before implementation:

- `docs/roadmap.md`, item 10.1.4, for the finish line and prerequisite chain.
- `docs/rstest-bdd-design.md` section 2.7.5, for the first-party harness
  plugin targets including `GpuiHarness` and the vendored shim notes.
- `docs/rstest-bdd-design.md` section 2.7.6.2-2.7.6.3, for the interim GPUI
  state pattern and the v0.6.0-beta2 quick-wins scope this item closes.
- `docs/developers-guide.md`, "Test organization: harness-owned integration
  tests", for the existing GPUI test layout the new binary must fit into.
- `docs/users-guide.md`, "Using the GPUI harness", for the public surface
  the augmented diagnostic must remain compatible with.
- `docs/execplans/10-1-3-feature-gated-gpui-test-suite.md`, for prior-art on
  feature-gated GPUI regression authoring and CodeRabbit handling.
- `docs/rust-testing-with-rstest-fixtures.md`, for `rstest` fixture practice.
- `docs/gherkin-syntax.md`, for `.feature` syntax (if a behavioural test is
  added; see decision in `Plan of work`).
- `docs/complexity-antipatterns-and-refactoring-strategies.md`, for
  refactoring guardrails should the harness module approach the 400-line cap.

## Context and orientation

The reader is assumed to know nothing about this repository. Key paths:

- `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs` (180 lines) defines
  `GpuiHarness`. `GpuiHarness::run` extracts `scenario_name` from
  `ScenarioMetadata`, runs `gpui::run_test`, builds a `gpui::TestAppContext`,
  and passes it through `ScenarioRunRequest::run`. The closure passed to
  `gpui::run_test` is the single point at which a step panic would surface; see
  `run_request_once` and `run_scenario`.
- `vendor/gpui/src/lib.rs` (~300 lines) is a stable-compatible shim of
  upstream Zed GPUI. `run_test` internally calls `panic::catch_unwind` and, on
  failure after `max_retries` exhaustion, `panic::resume_unwind`s the payload.
  `on_fail_fn` is `Option<fn()>` with no arguments and no captures; it cannot
  carry scenario context.
- `crates/rstest-bdd-harness/src/runner.rs` defines `ScenarioMetadata`
  (feature path, scenario name, line, tags) and `ScenarioRunRequest`. The base
  harness deliberately uses `panic::resume_unwind`-friendly types: a `FnOnce`
  runner inside a `Box<dyn FnOnce(C) -> T>`.
- `crates/rstest-bdd-harness/src/error.rs` defines `HarnessError`, today
  carrying only `RuntimeBuildFailed`, plus the wrapping `HarnessErrorContext`
  used by macro-generated harness calls. Step panics are *not* harness errors
  and must continue to propagate via the panic channel.
- `crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs:169`
  shows the existing `tracing::error!` pattern for harness initialization
  failures. The augmentation uses the same field names (`harness_type`,
  `feature_path`, `scenario_name`) to keep log output consistent.
- `crates/rstest-bdd/src/panic_support.rs:24` exposes
  `rstest_bdd::panic_message`, which downcasts a `Box<dyn Any + Send>` panic
  payload to a printable `String` for `&str`, `String`, common scalars, and an
  opaque fallback. `rstest-bdd-harness-gpui` already depends on `rstest-bdd`
  (transitively); confirm the dependency line and add it if missing.

The existing GPUI test layout under `crates/rstest-bdd-harness-gpui/tests/` is:

| Binary                       | Role                                                    |
| ---------------------------- | ------------------------------------------------------- |
| `harness_behaviour`          | Unit-style adapter behaviour, including `catch_unwind`. |
| `scenario_macros`            | `#[scenario]` plus harness adapter integration.         |
| `stateful_window`            | Durable handles plus visual-context reconstruction.     |
| `macro_compile`              | trybuild compile-pass for GPUI fixtures.                |
| `attribute_policy_behaviour` | Attribute policy emission.                              |

The new regression binary, `scenario_name_in_logs`, fits this layout naturally
and is feature-gated identically.

## Plan of work

The plan proceeds through four stages with explicit go/no-go gates. Do not
proceed to the next stage if the previous stage's validation fails.

### Stage A: understand, baseline, and align (no production code changes)

A1. Verify the current branch is
`10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs` via
`git branch --show-current`. If not, rename via `git branch -m` first.

A2. Capture the focused GPUI baseline and the workspace lint/test baseline:

```bash
BRANCH=$(git branch --show-current)
cargo test -p rstest-bdd-harness-gpui --features native-gpui-tests \
  2>&1 | tee "/tmp/baseline-gpui-rstest-bdd-${BRANCH}.out"
make check-fmt 2>&1 | tee "/tmp/baseline-check-fmt-rstest-bdd-${BRANCH}.out"
make lint     2>&1 | tee "/tmp/baseline-lint-rstest-bdd-${BRANCH}.out"
```

The baseline must pass before implementation. Record the existing test count in
`Surprises & discoveries` for later parity.

A3. Confirm by inspection that `crates/rstest-bdd-harness-gpui/Cargo.toml`
declares a `dev-dependencies` entry for `rstest-bdd` (or otherwise has access to
`rstest_bdd::panic_message`). If not, add one in Stage C against the workspace
version (caret requirement, per `AGENTS.md`).

A4. Decide on regression test shape. The roadmap entry mentions both a
"failing-harness regression" and the alternative of "harness docs document the
upstream limitation". Choose the affirmative path: write a Rust unit test (no
Gherkin file required) in a dedicated `scenario_name_in_logs.rs` test binary
that uses `catch_unwind` to capture the augmented payload and a
`gag::BufferRedirect`-free stderr capture. Specifically, use the existing
pattern of `std::panic::catch_unwind(std::panic::AssertUnwindSafe(...))` around
a `GpuiHarness::run(request)` whose `ScenarioRunner` panics, and read scenario
context off the resumed payload + a sidecar log channel.

This avoids depending on libtest stderr capture (which differs between
`cargo test` and `cargo-nextest`) and instead asserts on:

- the resumed panic payload, which the harness must augment via a
  re-raised `Box<String>` *only when augmentation is safe*, or
- a `tracing::error!`-emitted record captured through a `tracing-test`
  subscriber set up in the test, **if** the workspace already depends on
  `tracing-test`. If it does not, the test instead captures the augmented
  message by installing a thread-local `tracing` collector via the
  `tracing-subscriber` crate's `Layer` machinery that the workspace already
  uses (confirm in Stage A by `grep`-ing `tracing-subscriber` across the
  workspace before Stage C begins).

If neither approach is available without new dependencies, fall back to
**asserting on the resumed panic payload** by changing the harness to re-raise a
`Box<String>` whose contents start with the scenario name. The constraint that
the original payload type may change is acceptable for the GPUI harness
specifically, because step panics are not part of the public contract — they
are diagnostic artefacts. Record this decision in `Decision log` before Stage C.

A5. Sketch the augmentation site in a short note (added to `Decision log`)
referencing `gpui_harness.rs:48-66` and `gpui_harness.rs:69-95` so the next
agent knows precisely where the closure-local `catch_unwind` lives.

### Stage B: scaffolding the red regression test

B1. Create `crates/rstest-bdd-harness-gpui/tests/scenario_name_in_logs.rs` as a
new feature-gated test binary. Top-of-file shape:

```rust
//! Regression coverage for scenario-name diagnostics in `GpuiHarness`.
//!
//! These tests prove that when a step running under `GpuiHarness` panics,
//! the resumed payload and/or emitted log carry the originating feature
//! path, scenario name, and scenario line number so developers can
//! orientate failures quickly.
#![cfg(feature = "native-gpui-tests")]

use rstest::rstest;
use rstest_bdd_harness::{
    HarnessAdapter, ScenarioMetadata, ScenarioRunRequest, ScenarioRunner,
};
use rstest_bdd_harness_gpui::GpuiHarness;
use std::panic::{AssertUnwindSafe, catch_unwind};
```

B2. Add a happy-path baseline test that proves the augmentation does not
regress the success path:

```rust
#[rstest]
fn successful_scenario_does_not_inject_failure_marker() {
    // ScenarioMetadata with a recognisable name and line.
    // ScenarioRunner that returns Ok and does not panic.
    // Assert: harness.run(...) returns Ok, no captured stderr contains
    // the failure marker prefix.
}
```

B3. Add the primary failing-scenario test:

```rust
#[rstest]
fn failing_scenario_diagnostic_includes_scenario_name() {
    let metadata = ScenarioMetadata::new(
        "tests/features/scenario_name_in_logs.feature",
        "Step panics with augmented diagnostic",
        7,
        vec!["@regression".to_string()],
    );
    let runner = ScenarioRunner::new(|_context: gpui::TestAppContext| {
        panic!("step panic without scenario context");
    });
    let request = ScenarioRunRequest::new(metadata, runner);
    let harness = GpuiHarness::new();

    let result = catch_unwind(AssertUnwindSafe(|| harness.run(request)));
    let Err(payload) = result else {
        panic!("expected GpuiHarness to propagate scenario panic");
    };
    let message = rstest_bdd::panic_message(payload.as_ref());

    assert!(
        message.contains("Step panics with augmented diagnostic"),
        "expected scenario name in augmented diagnostic, got: {message}",
    );
    assert!(
        message.contains("tests/features/scenario_name_in_logs.feature"),
        "expected feature path in augmented diagnostic, got: {message}",
    );
    assert!(
        message.contains(":7"),
        "expected scenario line in augmented diagnostic, got: {message}",
    );
    assert!(
        message.contains("step panic without scenario context"),
        "expected original panic message preserved, got: {message}",
    );
}
```

B4. Add a guard against unintentional context leakage between scenarios:

```rust
#[rstest]
fn second_scenario_after_failure_runs_with_fresh_diagnostic_context() {
    // Run the failing scenario via catch_unwind, then run a successful
    // scenario with a different name; assert that the successful run
    // does not carry the previous scenario's name in any way (e.g. the
    // captured Ok value path emits no marker referencing the prior
    // scenario). This guards against thread-local context bleeding.
}
```

B5. Run the new binary at this point:

```bash
BRANCH=$(git branch --show-current)
cargo test -p rstest-bdd-harness-gpui --features native-gpui-tests \
  --test scenario_name_in_logs \
  2>&1 | tee "/tmp/red-scenario-name-rstest-bdd-${BRANCH}.out"
```

The expected outcome is: the happy-path test passes, the failing-scenario test
fails because the resumed panic message contains only "step panic without
scenario context" and not the scenario name. Confirm the *failure mode* matches
the missing-augmentation hypothesis; if the test fails for another reason, stop
and escalate.

Commit the red regression so the change history records the intended
falsification before any production code changes.

### Stage C: implementation in `GpuiHarness`

C1. In `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs`, add a private
helper that augments a panic payload:

```rust
fn augment_panic(
    payload: Box<dyn std::any::Any + Send>,
    metadata: &rstest_bdd_harness::ScenarioMetadata,
) -> Box<dyn std::any::Any + Send> {
    let message = rstest_bdd::panic_message(payload.as_ref());
    let augmented = format!(
        "rstest-bdd-harness-gpui scenario panicked: \
         feature={feature_path}, scenario={scenario_name:?}, \
         line={scenario_line}: {message}",
        feature_path = metadata.feature_path(),
        scenario_name = metadata.scenario_name(),
        scenario_line = metadata.scenario_line(),
    );
    Box::new(augmented)
}
```

Use `rstest_bdd::panic_message` to render the original payload. Reasoning:

- It already handles the common payload types (`&str`, `String`, scalars,
  `Box<str>`, `fmt::Arguments`) and falls back to an opaque description for
  unknown payloads, so the augmentation does not lose information.
- The augmented payload is a `String`, which is `'static + Send + Any` and
  is reliably renderable by libtest's panic-message printer and by
  `panic::resume_unwind`. Downstream callers that want the original payload can
  already see it inside the augmented string.

C2. Wrap the existing `Self::run_scenario(...)` call in `run_request_once` with
`panic::catch_unwind(AssertUnwindSafe(...))`. Before invoking the runner, emit a
`tracing::error!`-style "scenario starting" record via `tracing::debug!` (one
line, `feature_path` and `scenario_name` fields). On `Err(payload)`:

- Emit a `tracing::error!` record with the existing field names from the
  macro layer (`harness_type = "rstest_bdd_harness_gpui::GpuiHarness"`,
  `feature_path`, `scenario_name`, `scenario_line`, plus the rendered
  `error = %message`).
- Emit a single `eprintln!` with the same content so the diagnostic is
  visible without a tracing subscriber. This is the safety net the Wyvern Buzzy
  Bee persona flagged for CI log scanning.
- `panic::resume_unwind(augment_panic(payload, &metadata))`.

Do not catch a panic from `finish_context` (the cleanup teardown). If the
teardown itself panics, propagate it unaugmented so the failure points at the
teardown bug, not at a phantom scenario step. Document this in a two-line
comment by `finish_context`.

C3. Update the module-level doc comment on `GpuiHarness` to mention the
augmentation:

```rust
//! GPUI harness adapter for scenario execution.
//!
//! When a step running under `GpuiHarness` panics, the harness captures
//! the panic payload, prepends the feature path, scenario name, and
//! feature-file line, and re-raises the augmented message via
//! `panic::resume_unwind`. The harness emits the same context to
//! `tracing::error!` and to stderr so test runners that do not collect
//! `tracing` events still surface the scenario name on failure.
```

C4. Verify the file remains under 400 lines (per `AGENTS.md`). If the addition
pushes it over, extract `augment_panic` and the new tests into a sibling module
(e.g. `crates/rstest-bdd-harness-gpui/src/diagnostics.rs`) and re-export through
`lib.rs`. Prefer in-place edits first.

C5. If `rstest-bdd-harness-gpui` does not already depend on `rstest-bdd`, add a
`dev-dependencies = { path = "../rstest-bdd", version = "..." }` line matching
the existing workspace version. Production dependency is **not** needed because
`panic_message` is only used in the harness module via a `#[cfg(test)]`-guarded
helper *only if* the implementation chooses to expose the helper to the test.
Otherwise, depend on `rstest-bdd` in the runtime `dependencies` table for
`augment_panic` to use it directly; record the choice in `Decision log`.

C6. Re-run the focused gate:

```bash
BRANCH=$(git branch --show-current)
cargo test -p rstest-bdd-harness-gpui --features native-gpui-tests \
  2>&1 | tee "/tmp/green-scenario-name-rstest-bdd-${BRANCH}.out"
```

The previously red test must now pass; all previously passing tests must
continue to pass. If any pass-to-fail regression appears, inspect the diff
against `gpui_harness.rs` and the new test binary, then either fix in place or
revert and re-plan.

C7. Run the repository quality gates sequentially:

```bash
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-rstest-bdd-${BRANCH}.out"
make lint      2>&1 | tee "/tmp/lint-rstest-bdd-${BRANCH}.out"
make test      2>&1 | tee "/tmp/test-rstest-bdd-${BRANCH}.out"
```

Fix any lint or formatting issues in the code itself, not by suppressing
warnings.

C8. Commit the implementation milestone with a clear `feat:` or `fix:` subject
line referencing 10.1.4.

### Stage D: documentation and hardening

D1. Update `docs/users-guide.md` "Using the GPUI harness" with a short
paragraph (max five sentences) explaining that `GpuiHarness` augments
step-panic diagnostics with the scenario name, feature path, and line number,
and that the augmented payload is available via the panic channel as well as
through `tracing::error!` and stderr. Cross-reference the new regression test
for readers who want a concrete example.

D2. Update `docs/developers-guide.md` "Test organization: harness-owned
integration tests" table by adding a row for the new `scenario_name_in_logs`
binary. Keep the row format identical to the existing rows.

D3. Update `docs/rstest-bdd-design.md` section 2.7.5 only if the implementation
changes the documented behaviour of `GpuiHarness`. The existing description
currently states that the harness "passes [the context] through
`request.run(...)`". Add one or two sentences at the end of the
`rstest-bdd-harness-gpui` bullet noting the new augmentation behaviour and that
the public trait surface is unchanged. If a wider rewrite seems necessary,
escalate before editing.

D4. If documentation changed, run the Markdown gates:

```bash
BRANCH=$(git branch --show-current)
make fmt          2>&1 | tee "/tmp/fmt-rstest-bdd-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-rstest-bdd-${BRANCH}.out"
```

D5. Run CodeRabbit after the behavioural milestone and again after the
documentation milestone:

```bash
BRANCH=$(git branch --show-current)
coderabbit review --agent \
  2>&1 | tee "/tmp/coderabbit-rstest-bdd-${BRANCH}.out"
```

Clear every actionable concern before moving to the next milestone. If
CodeRabbit is unavailable or unauthenticated, record the exact command output
in this plan and decide whether to continue or escalate.

D6. Mark `docs/roadmap.md` item 10.1.4 done (`- [x]`) and update this plan's
`Status` to `COMPLETE`. Commit the roadmap update alongside the implementation
and docs.

D7. Push the branch (tracking
`origin/10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs`), update
the draft pull request with the final state, validation transcripts, and a link
to the Lody session.

## Validation expectations

Quality criteria (what "done" means):

- Tests: the focused feature-gated GPUI gate
  (`cargo test -p rstest-bdd-harness-gpui --features native-gpui-tests`)
  passes; the new binary `scenario_name_in_logs` contributes at least two
  passing tests, including the failing-scenario diagnostic assertion. The full
  workspace `make test` passes.
- Lint/typecheck: `make check-fmt` exits 0, `make lint` exits 0,
  `make markdownlint` exits 0 when Markdown changed.
- Behaviour: a deliberately panicking GPUI scenario produces a captured
  panic payload whose rendered message contains the scenario name, feature
  path, and scenario line; the original panic message is preserved inside the
  augmented payload.
- Documentation: the harness module-level doc comment, the user guide,
  and the developer guide each include a single, accurate paragraph describing
  the augmentation. The design document's GPUI bullet has a one-sentence update
  noting the behaviour.

Quality method (how we check):

- The new test binary is the primary regression. Run it in isolation
  with `--test scenario_name_in_logs` to confirm the diagnostic shape.
- The Tokio harness must remain unaffected: run
  `cargo test -p rstest-bdd-harness-tokio` and confirm it still passes without
  any code change in that crate.
- CodeRabbit `--agent` review must report zero actionable findings, or
  every finding must be addressed before the milestone is complete.

Expected successful output snippets (paraphrased):

```plaintext
running 3 tests
test successful_scenario_does_not_inject_failure_marker ... ok
test failing_scenario_diagnostic_includes_scenario_name ... ok
test second_scenario_after_failure_runs_with_fresh_diagnostic_context ... ok

test result: ok. 3 passed; 0 failed; ...
```

```plaintext
make check-fmt: exits 0
make lint: exits 0
make test: exits 0
make markdownlint: exits 0 when Markdown changed
```

## Idempotence and recovery

- The plan is re-runnable: every step is additive and uses absolute
  workspace paths. If Stage B's red regression is committed and Stage C later
  fails, revert Stage C with `git restore -p`, fix the augmentation, and
  re-run. Do not amend committed milestones; create a new commit instead.
- The vendored shim `vendor/gpui/src/lib.rs` is not modified; if a future
  shim refresh changes `run_test`'s signature, only `gpui_harness.rs` needs
  revisiting. Document any shim-driven follow-up in `Surprises & discoveries`.
- If CodeRabbit or `make lint` flags a new finding, fix the underlying
  issue in code rather than suppressing the lint, per `AGENTS.md`.

## Interfaces and dependencies

This change does not introduce any new public types, traits, or functions. As
implemented in `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs`, the
augmentation is split across several private helpers on `impl GpuiHarness` and
one module-private RAII guard, none of which are reachable outside the crate:

```rust
// Builds the augmented panic string from the original payload and metadata.
fn augmented_panic_message(
    payload: &(dyn std::any::Any + Send),
    metadata: &rstest_bdd_harness::ScenarioMetadata,
) -> String;

// Observability: emits a structured `tracing::error!` event only.
fn record_panic_event(message: &str, metadata: &rstest_bdd_harness::ScenarioMetadata);

// I/O primitive: injectable writer; callers select stderr at the call site.
fn write_stderr_diagnostic_to(
    writer: &mut impl std::io::Write,
    message: &str,
) -> std::io::Result<()>;

// RAII guard that ensures `finish_context` runs on both the success and
// the panic paths.
struct ContextCleanup<'a> { /* dispatcher and context borrows */ }
```

The earlier draft of this section described a single `augment_panic` helper
that returned a re-boxed payload. The implementation diverged into the helpers
above so observability (tracing) and I/O (stderr) are separable, the writer is
an explicit dependency, and cleanup runs via `Drop` rather than by guarding a
single point of return. All helpers remain private; integration tests exercise
them end-to-end through `HarnessAdapter::run`, and the lone unit test that
needs the I/O primitive directly
(`write_stderr_diagnostic_to_returns_err_on_io_failure`) lives in the
crate-internal `#[cfg(test)] mod tests` block alongside the implementation.

If a future refactor moves any helper into
`crates/rstest-bdd-harness-gpui/src/diagnostics.rs`, the visibility remains
`pub(crate)` at most; do not export it.

The change relies on the following existing public surfaces, which are not
modified:

- `gpui::run_test(...)` — vendored shim signature unchanged.
- `rstest_bdd_harness::ScenarioMetadata` getter accessors remain unchanged:
  `feature_path`, `scenario_name`, and `scenario_line`.
- `rstest_bdd_harness::HarnessAdapter::run` — `GpuiHarness`'s
  implementation continues to return `HarnessResult<T>` for the success path
  and propagate via `panic::resume_unwind` for the failure path.
- `rstest_bdd::panic_message` — used to render the original payload.
- `tracing::error!` / `tracing::debug!` — already a dev-dependency in
  the workspace; confirm at Stage A whether `rstest-bdd-harness-gpui` already
  lists `tracing` (it should, per
  `crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs:169`).

Adopt no new external crate dependencies. Confirm during Stage A that `tracing`
and (if used) `rstest-bdd` are already on the dependency graph for
`rstest-bdd-harness-gpui`; if either is missing, add it as a regular dependency
in Stage C and record the addition in `Decision log`.

## Progress

- [x] (2026-05-24 17:48 CEST) Load `leta`, `rust-router`, and supporting
  Rust skills. Create the `leta` workspace for this worktree. The workspace was
  already registered, so no new Leta state was needed.
- [x] (2026-05-24 17:48 CEST) Confirm and rename branch to
  `10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs`, tracking
  `origin/10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs`. The
  branch name was already correct; no rename was needed.
- [x] (2026-05-24 17:50 CEST) Capture baseline
      `cargo test -p rstest-bdd-harness-gpui --features native-gpui-tests`,
      `make check-fmt`,
      and `make lint`; verify all pass; record test count. The focused GPUI
      baseline passed with 24 tests: 2 attribute-policy tests, 6
      harness-behaviour tests, 1 trybuild macro test, 8 scenario-macro tests,
      and 7 stateful-window tests.
- [x] (2026-05-24 17:50 CEST) Decide regression test channel (resumed panic
  payload vs. tracing subscriber capture) and record decision.
- [x] (2026-05-24 17:52 CEST) Add the red regression test binary
  `scenario_name_in_logs.rs` and confirm it fails for the expected reason.
- [x] (2026-05-24 17:55 CEST) Implement `augment_panic` and the closure-local
  `catch_unwind`/`resume_unwind` in `gpui_harness.rs`. Run focused gate. The
  focused binary passed with 3 tests, and the full GPUI feature-gated suite
  passed with 27 tests.
- [x] (2026-05-24 18:01 CEST) Run `make check-fmt`, `make lint`, `make test`
  sequentially; fix issues in the code, not by suppression. `make test` ran
  1453 nextest tests with 1453 passed and 7 skipped, then ran 62 Python release
  automation tests with 62 passed.
- [x] (2026-05-24 18:12 CEST) Update `users-guide.md`,
  `developers-guide.md`, and (if needed) `rstest-bdd-design.md` §2.7.5.
- [x] (2026-05-24 18:13 CEST) Run `make fmt` and `make markdownlint` if
  Markdown changed. `make fmt` again failed in the Markdown auto-fix phase on
  unrelated documents, so unrelated formatter churn was reverted. The required
  `make markdownlint` gate then passed with zero errors.
- [x] (2026-05-24 18:21 CEST) Run CodeRabbit `--agent`; clear all actionable
  findings; re-run gates if changes are made. CodeRabbit completed with
  `findings: 0` for both the behavioural and documentation milestones.
- [x] (2026-05-24 18:12 CEST) Mark `docs/roadmap.md` item 10.1.4 done.
- [x] (2026-05-24 18:26 CEST) Push branch and update draft PR with
  validation transcripts and the Lody session link. PR #496 now describes the
  implemented 10.1.4 scope and remains a draft.

## Surprises & discoveries

- 2026-05-24 17:48 CEST: `leta workspace add` reported that this worktree was
  already registered. Treat this as satisfying the workspace creation
  requirement.

- 2026-05-24 17:48 CEST: `rstest-bdd-harness-gpui` already lists
  `rstest-bdd` as a dev-dependency, so the regression test can call
  `rstest_bdd::panic_message` without dependency changes. Production code does
  not yet have access to `panic_message`; Stage C must either add `rstest-bdd`
  as a regular dependency or use a local rendering helper.

- 2026-05-24 17:50 CEST: Stage A baselines passed. Logs:
  `/tmp/baseline-gpui-rstest-bdd-10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs.out`,
  `/tmp/baseline-check-fmt-rstest-bdd-10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs.out`,
  and
  `/tmp/baseline-lint-rstest-bdd-10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs.out`.

- 2026-05-24 17:51 CEST: The first red-regression attempt failed at compile
  time because `HarnessError` intentionally does not implement `PartialEq`. The
  success-path assertions now unwrap the harness result with a diagnostic
  message before comparing the returned value.

- 2026-05-24 17:52 CEST: The corrected red regression failed for the expected
  reason: the captured panic payload contained only
  `step panic without scenario context`. Because repository instructions forbid
  committing failing quality gates, the red state was not committed; the
  transcript remains in
  `/tmp/red-scenario-name-rstest-bdd-10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs.out`.

- 2026-05-24 17:54 CEST: `make fmt` applied Rust formatting but then failed
  on pre-existing Markdown lint issues across unrelated documents. The
  formatter churn outside this task was reverted, preserving only task files.
  Future documentation edits should be validated with focused inspection plus
  `make markdownlint`, recording any unrelated baseline failures explicitly.

- 2026-05-24 18:01 CEST: `make markdownlint` passed after the unrelated
  formatter churn was reverted. The earlier `make fmt` failure came from the
  formatter's auto-fix path, not from the lint target.

- 2026-05-24 18:06 CEST: CodeRabbit review for commit `bc6efa6` completed
  successfully with zero findings. Transcript:
  `/tmp/coderabbit-rstest-bdd-10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs.out`.

- 2026-05-24 18:13 CEST: The documentation milestone touched
  `docs/users-guide.md`, `docs/developers-guide.md`,
  `docs/rstest-bdd-design.md`, and `docs/roadmap.md`. `make fmt` still fails in
  its auto-fix path on unrelated Markdown files, but `make markdownlint` passes
  after reverting the unrelated formatter churn.

- 2026-05-24 18:21 CEST: CodeRabbit review after the documentation milestone
  completed successfully with zero findings. Transcript:
  `/tmp/coderabbit-docs-rstest-bdd-10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs.out`.

- 2026-05-24 18:26 CEST: The branch was pushed to
  `origin/10-1-4-failing-gpui-scenarios-include-scenario-name-in-logs`, and
  draft PR #496 was updated with the implemented scope, validation results,
  CodeRabbit findings count, and Lody session link.

## Decision log

- Decision: choose the closure-local `catch_unwind` / `resume_unwind`
  approach (Option A) over installing a `panic::set_hook` (Option B) or
  documenting an upstream limitation only (Option D). Rationale:
  `panic::set_hook` is process-global and races with `cargo test`'s parallel
  test threads; closure-local `catch_unwind` is hook-free, nextest-safe, and
  survives a future swap of the vendored shim for upstream GPUI (whose
  `run_test` shape matches the shim). Documenting an upstream limitation
  without code change would leave the diagnostic gap open; the gap is fixable
  inside our crate without touching public APIs. Date/Author: 2026-05-24,
  planning phase.

- Decision: emit augmented diagnostic via both `eprintln!` and
  `tracing::error!`, in addition to mutating the resumed panic payload to a
  `Box<String>`. Rationale: nextest captures stderr per test; libtest captures
  stdout by default. `eprintln!` is the lowest-common-denominator channel that
  reaches both. `tracing::error!` mirrors the existing pattern at
  `crates/rstest-bdd-macros/src/codegen/scenario/runtime/harness.rs:169` and
  keeps the project's observability story consistent. Date/Author: 2026-05-24,
  planning phase.

- Decision: keep `HarnessError` unchanged; do not add a `StepPanicked`
  variant. Rationale: `gpui::run_test` already propagates step panics via the
  panic channel rather than returning an `Err`. Adding a variant would
  advertise a capability the trait cannot deliver without a breaking signature
  change. The Wyvern Telefono persona explicitly flagged this as contract
  pollution. Documented step panics remain diagnostic artefacts; harness errors
  remain reserved for harness initialization failures. Date/Author: 2026-05-24,
  planning phase.

- Decision: place the regression in a new
  `scenario_name_in_logs.rs` test binary instead of extending
  `harness_behaviour.rs` or `stateful_window.rs`. Rationale: keeps the
  deliberate panic isolated, matches the existing one-concern-per-binary layout
  in `crates/rstest-bdd-harness-gpui/tests/`, and stops the regression from
  interleaving with `stateful_window.rs`'s `#[serial]` ordering. Date/Author:
  2026-05-24, planning phase.

- Decision: do not author a `.feature` file or `#[scenario]`-driven
  behavioural test for the regression. Rationale: the roadmap finish line asks
  for a "failing-harness regression". The most direct expression of that is a
  hand-built `ScenarioRunRequest` whose `ScenarioRunner` panics, exercised
  through `GpuiHarness::run`. Going through `#[scenario]` would add macro
  overhead and obscure the assertion target. Behavioural coverage for the wider
  GPUI workflow already exists in `stateful_window.rs`. Date/Author:
  2026-05-24, planning phase.

- Decision: treat the user's 2026-05-24 implementation request as approval to
  move the ExecPlan from draft to execution. Rationale: the plan was already
  authored, the user explicitly asked to proceed with implementation, and the
  request also requires the plan to stay current during the work. Date/Author:
  2026-05-24, implementation phase.

- Decision: assert the regression through the resumed panic payload, not a
  tracing subscriber. Rationale: the plan requires deterministic pre-CodeRabbit
  validation, and the panic payload is observable through `catch_unwind`
  without relying on libtest or nextest stderr capture details. The harness can
  still emit `tracing::error!` and stderr diagnostics in Stage C for human CI
  logs. Date/Author: 2026-05-24, implementation phase.

- Decision: add `rstest-bdd` and `tracing` as regular dependencies of
  `rstest-bdd-harness-gpui`. Rationale: production augmentation uses
  `rstest_bdd::panic_message` to render arbitrary panic payloads, and
  `tracing::error!` is part of the required diagnostic channel. Both are
  existing workspace dependencies, so this introduces no new external crate.
  Date/Author: 2026-05-24, implementation phase.

- Decision: re-raise an augmented `String` payload and explicitly drop the
  original payload after rendering it. Rationale: libtest and nextest reliably
  display string panic payloads, while the original payload's human-readable
  content remains embedded in the augmented diagnostic. Clippy requires the
  ownership handoff to be explicit because the original box is not resumed
  directly. Date/Author: 2026-05-24, implementation phase.

## Outcomes & retrospective

Roadmap item 10.1.4 is complete. Failing GPUI scenarios now surface the
scenario name, feature path, feature-file line number, and original panic
message through the resumed panic payload, `tracing::error!`, and stderr.

The regression test `scenario_name_in_logs.rs` covers the success path, the
augmented failing path, and fresh harness state after a caught panic. The full
GPUI feature-gated suite, workspace format check, Clippy lint, workspace test
suite, Markdown lint, and two CodeRabbit reviews passed. The only caveat is that
`make fmt` currently fails in its Markdown auto-fix phase on unrelated
documents even though `make markdownlint` passes after reverting that churn.

## Revision note

- 2026-05-24: initial draft prepared with Wyvern team consultation and
  Firecrawl-supplemented prior-art research. Awaiting user approval before
  implementation.
