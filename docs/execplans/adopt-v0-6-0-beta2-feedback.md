# Fold v0.6.0-beta2 GPUI adopter feedback into the design, roadmap, and ADRs

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, and `Outcomes & retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

The first downstream adopter of `rstest-bdd` `0.6.0-beta2` — a GPUI desktop
drawing application — migrated one stateful GPUI behavioural test onto the
first-party `rstest-bdd-harness-gpui` harness and recorded a detailed
implementation report. Most of that report maps onto roadmap items that already
exist (the borrow-constraint redesign, the scenario-local state helper, the
`#[harness_context]` marker). However, the migration also surfaced concrete
gaps, factual corrections, and one genuine correctness foot-gun that are *not*
yet captured anywhere in this repository's design document, roadmap, ADRs
(architectural decision records), or adoption guides.

This plan does one thing: it turns that adopter feedback into a precise,
reviewed set of edits to `docs/roadmap.md`, `docs/rstest-bdd-design.md`, three
new ADRs, and the two adoption guides (`docs/users-guide.md` and
`docs/v0-6-0-migration-guide.md`), so a maintainer can land them with
confidence and without re-deriving the analysis.

After this plan is executed, a reader of the repository can observe:

1. The roadmap records every new or re-scoped work item the feedback warrants —
   the feature-file rebuild-invalidation fix, the gpui-version-accurate and
   lint-clean playbooks, the nextest-and-`serial_test` interaction note, the
   bulk-migration cookbook, the first-party GPUI scenario-state helper and
   cleanup-guard macro, the definitive resolution of the ambiguous roadmap item
   10.1.4, and the elevation of the v0.7.0 borrow redesign from "ambition" to
   committed direction.
2. The design document `§2.7.6.x` is *corrected* (its GPUI snippets target the
   real published `gpui 0.2.2` API, or clearly flag which gpui they target) and
   *extended* with a feature-file rebuild-invalidation subsection and a nextest
   parallelism subsection.
3. Three new ADRs (`adr-010`, `adr-011`, `adr-012`) are drafted, each in
   `Proposed` status, plus a tracked note resolving the lingering `Proposed`
   status of ADR-008.
4. The adoption guides carry the same corrections, so the next adopter does not
   repeat the four-shape gpui API mismatch, the lint-profile collisions, or the
   stale-feature-file confusion.

Success is observable as: a reviewer can `git diff` the listed files and see
each enumerated change; `make markdownlint`, `make nixie`, and `make vale` pass
on the modified Markdown; and a CodeRabbit `coderabbit review --agent` pass on
the branch returns no unresolved concerns.

This is a planning-and-documentation deliverable. It does **not** implement any
of the code work items it schedules (the `include_str!` emission, the
`GpuiScenarioState` helper, the guard-based `StepContext`); those remain
roadmap items delivered under their own ExecPlans. The single exception, if the
maintainer approves it, is the optional code change in Stage E (the
rebuild-invalidation fix), which is small, non-breaking, and closes an active
correctness foot-gun.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

- This plan edits documentation and ADRs only, except for the explicitly
  optional and separately gated Stage E. No public API, trait, or macro surface
  changes as part of Stages A–D.
- Do not change the meaning of any roadmap item already marked `[x]`
  (delivered). Delivered items may be *clarified* (for example, resolving the
  10.1.4 ambiguity by recording which branch shipped), but their delivered
  scope must not be retroactively rewritten.
- Preserve the existing design-document section-numbering scheme. New
  subsections slot in as `§2.7.6.6`, `§2.7.6.7`, … after the current `§2.7.6.5`,
  or as a new `§2.7.7`; they do not renumber existing sections.
- New ADRs follow the established house format observed in
  `docs/adr-007-harness-context-injection.md` and
  `docs/adr-009-consistent-implicit-fixture-name-normalization.md`: a level-one
  title `# Architectural decision record (ADR) NNN: <title>`, then `## Status`,
  `## Date`, `## Context and problem statement`, the options/decision, and
  consequences. New ADRs are created in `Proposed` status; this plan does not
  self-accept them.
- All prose uses en-GB-oxendict spelling ("-ize"/"-yse"/"-our") and obeys
  `docs/documentation-style-guide.md`, `.vale.ini`, and
  `.markdownlint-cli2.jsonc`. No Markdown file exceeds the repository line
  limits enforced by `make markdownlint`.
- Every factual claim about external tooling (`include_str!` dependency
  tracking, `cargo::rerun-if-changed` semantics, nextest process-per-test
  scheduling, `serial_test` scope, cucumber-rs runtime parsing) is cited to the
  authoritative source recorded in `Artifacts and notes`; do not assert these
  from memory.
- Dependency pins are not touched. This plan does not bump `gpui`, does not
  change the `vendor/gpui` path dependency, and does not add dependencies.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached.

- Scope: if landing Stages A–D requires editing more than the eight files named
  in `Interfaces and dependencies` (three new ADRs, `docs/roadmap.md`,
  `docs/rstest-bdd-design.md`, `docs/users-guide.md`,
  `docs/v0-6-0-migration-guide.md`, and this plan), stop and escalate.
- Roadmap scheduling: this plan *recommends* pulling the scenario-state helper
  (11.1.3/11.1.4) and the rebuild-invalidation fix forward to v0.6.0 final, but
  the actual release-train placement is a maintainer decision. If executing the
  plan would require committing to a release schedule not yet agreed, stop and
  present the trade-off rather than choosing unilaterally.
- Code change: if the optional Stage E rebuild-invalidation fix cannot be made
  non-breaking and confined to the macro crate plus one regression test, stop
  and escalate; do not expand it into a build-script redesign within this plan.
- Ambiguity: if resolving the gpui API divergence turns out to require changing
  the harness's `vendor/gpui` dependency (rather than documenting the
  divergence), stop and escalate — that is a separate architectural decision
  outwith this plan's remit.
- Iterations: if `make markdownlint`/`make nixie`/`make vale` or
  `coderabbit review --agent` cannot be brought clean after 4 focused attempts
  per milestone, stop and escalate with the captured diagnostics.

## Risks

- Risk: the GPUI snippets in the design document (`§2.7.6.2`) and the user's
  guide playbook are written against the *vendored* gpui under `vendor/gpui`,
  whose test API diverges from the published `gpui 0.2.2` that downstream crates
  depend on. "Correcting" the snippets to the published API could
  desynchronize them from the in-repo regression suite
  (`crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`), which compiles
  against the vendored gpui.
  Severity: high. Likelihood: high.
  Mitigation: do not rewrite the snippets to a single API. Add a prominent
  banner stating which gpui each snippet targets, and a mapping table from the
  vendored-gpui shapes (used by the regression suite) to the published
  `gpui 0.2.2` shapes (used by adopters). Record in the Decision Log why a
  one-API rewrite was rejected. Escalate per the Ambiguity tolerance if anyone
  proposes changing the vendored dependency instead.

- Risk: the nextest interaction note could mislead. The current playbook tells
  users to mark stateful GPUI scenarios `#[serial]`. Under cargo-nextest
  (process-per-test) `#[serial]` does not serialize tests against one another,
  because each test runs in its own process and `serial_test`'s mutex is
  in-process only. A naive "remove `#[serial]`" edit would then break the
  `cargo test` path, where `#[serial]` *is* required because all tests in one
  binary share a process and the thread-local.
  Severity: medium. Likelihood: medium.
  Mitigation: document the full matrix (cargo test vs nextest; same-binary vs
  cross-binary; `#[serial]` vs `#[file_serial]` vs nextest test-groups) rather
  than a single recommendation. Keep `#[serial]` recommended for the
  `cargo test` path and note it is redundant-but-harmless under nextest, because
  process-per-test already isolates per-process thread-local state.

- Risk: the rebuild-invalidation ADR (`adr-010`) recommends macro-emitted
  `include_str!`, but `include_str!` resolves its path relative to the *invoking*
  source file, while `#[scenario(path = ...)]` resolves `path` relative to
  `CARGO_MANIFEST_DIR`. A naive emission would change path semantics or break
  call sites.
  Severity: medium. Likelihood: medium.
  Mitigation: the ADR must specify emitting an absolute path (built from
  `CARGO_MANIFEST_DIR` or the call-site span), and the optional Stage E fix must
  add a regression test proving a `.feature`-only edit forces a rebuild without
  changing any existing call site. If absolute-path emission proves
  impracticable, the ADR's documented fallback is the build-script route
  (directory plus per-file `cargo::rerun-if-changed`), as proven by the
  `theoremc` prior art.

- Risk: ADR-008 remains in `Proposed` status while roadmap items 9.7.1–9.7.4
  shipped "under maintainer authorization". Touching the harness-led-defaults
  area in the design doc could surface this inconsistency and expand scope.
  Severity: low. Likelihood: medium.
  Mitigation: keep ADR-008 resolution as a clearly-labelled, separable
  follow-up note in the roadmap, not a blocking dependency of the GPUI feedback
  work. Do not change ADR-008's status as part of this plan unless the
  maintainer explicitly directs it.

## Progress

- [x] (2026-06-09) Stage A research complete: read `docs/roadmap.md` (phases
  9–12), `docs/rstest-bdd-design.md` `§2.7`–`§2.7.6.5`, the user's-guide GPUI
  playbook, the v0.6.0 migration guide, the `rstest-bdd-harness-gpui` public
  API, ADR-007/008/009 headers, and the canonical regression suite
  `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`. Resolved the
  external prior-art questions (`include_str!` tracking,
  `cargo::rerun-if-changed`, cucumber-rs runtime parsing, theoremc build.rs,
  nextest + `serial_test`) with cited sources (see `Artifacts and notes`).
- [x] (2026-06-09) Confirmed roadmap item 10.1.4 shipped the *affirmative*
  branch (scenario name embedded in panic messages and tracing events), not the
  documented-limitation fallback.
- [x] (2026-06-09) Confirmed the gpui API divergence: the playbook and design
  snippets mirror the vendored gpui used by `stateful_window.rs`, not the
  published `gpui 0.2.2` the adopter consumed.
- [ ] (drafting) Stage B: write the three ADR drafts (`adr-010`, `adr-011`,
  `adr-012`).
- [ ] Stage C: apply the roadmap edits.
- [ ] Stage D: apply the design-document and adoption-guide edits.
- [ ] Stage E (optional, maintainer-gated): land the macro-emitted
  `include_str!` rebuild-invalidation fix plus regression test.
- [ ] Community-of-experts review of the assembled plan and ADR drafts; revise.
- [ ] Gate (`make markdownlint`/`make nixie`/`make vale`) and
  `coderabbit review --agent` per milestone; clear all concerns.

## Surprises & discoveries

- Observation: the GPUI code snippets in `docs/rstest-bdd-design.md` `§2.7.6.2`
  and the "Stateful GPUI scenarios with durable handles" playbook in
  `docs/users-guide.md` are written against a gpui API that the published
  `gpui 0.2.2` on crates.io does *not* expose.
  Evidence: the playbook (`docs/users-guide.md` lines ~1264–1316) and the
  regression suite `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`
  lines 85, 88, 98, 110–111, 130–135, 147–149 use a one-argument
  `add_window_view(|_context| View::default())` closure, a
  `VisualTestContext::window_handle()` method, an `Option`-returning
  `VisualTestContext::from_window(...)` (`.unwrap_or_else(|| panic!(...))`), and
  `Option`/`Result`-wrapped `read_entity`/`update_entity` (`== Some(1)`,
  `== Ok(())`). The downstream adopter reported that published `gpui 0.2.2`
  instead has a two-argument `add_window_view(|_window, view_cx| ...)` closure,
  a `Window::window_handle()` accessor (no `VisualTestContext::window_handle()`),
  a by-value `VisualTestContext::from_window(...) -> Self`, and identity
  `type Result<T> = T` (so `read_entity`/`update_entity` return `R` directly).
  These shapes come from the vendored gpui at `vendor/gpui`
  (`Cargo.toml`: `gpui = { version = "0.2.2", path = "vendor/gpui", ... }`).
  Impact: the documentation cannot be corrected to a single API without
  desynchronizing it from the in-repo regression suite. The fix is a
  which-gpui banner plus a vendored→published mapping table. This is the single
  largest doc-correctness gap the feedback exposes.

- Observation: under cargo-nextest, `#[serial]` does not serialize stateful
  GPUI scenarios against each other.
  Evidence: nextest runs each test in its own process and parallelises across
  them (<https://nexte.st/docs/design/how-it-works/>); `serial_test`'s
  `#[serial]` is an in-process mutex, and cross-process serialization requires
  `#[file_serial]` (<https://docs.rs/serial_test/>). Process-per-test therefore
  isolates per-process thread-local scenario state automatically, while
  `#[serial]` buys nothing across processes.
  Impact: the playbook's `#[serial]` guidance is correct for `cargo test`
  (one process per integration binary, thread-parallel within) but needs an
  explicit nextest caveat. The repository's own `make test` uses nextest, so
  this matters for adopters who copy the gate.

- Observation: `#[scenario(path = "...")]` reads the `.feature` file with
  ordinary filesystem I/O at macro-expansion time, so Cargo's fingerprinting
  cannot see the dependency and a `.feature`-only edit does not trigger a
  rebuild.
  Evidence: confirmed mechanism — rustc registers files referenced by
  `include_str!`/`include_bytes!`/`include!` into dep-info, but plain
  `std::fs` reads inside a proc-macro emit no dep-info entry
  (<https://github.com/rust-lang/cargo/issues/1510>,
  <https://doc.rust-lang.org/std/macro.include_str.html>). The adopter observed
  a corrupted expectation appearing to pass from stale cache until an unrelated
  `.rs` file was touched.
  Impact: this is a real correctness foot-gun for a *testing* framework and is
  not recorded anywhere in the repo. It warrants a dedicated ADR and a roadmap
  item, and optionally an immediate non-breaking fix (Stage E).

- Observation: roadmap item 10.1.4 ("Failing GPUI scenarios include the
  scenario name in logs … or the harness docs document the upstream
  limitation") shipped the affirmative branch, but the roadmap text leaves the
  outcome ambiguous.
  Evidence: `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs` augments the
  panic payload with the feature path, scenario name, and feature-file line
  (`augmented_panic_message`, lines ~135, 180, 245, 266, 275–280), and
  regression tests `scenario_name_in_logs.rs` plus
  `augmented_panic_message_includes_scenario_name_for_payload_type` assert it.
  Impact: the roadmap should state definitively that the name is embedded, with
  the test reference, removing the "or documented limitation" ambiguity.

## Decision log

- Decision: assign the three new ADRs the next free numbers — `adr-010`
  (feature-file change detection), `adr-011` (first-party scenario-state helpers
  and cleanup), `adr-012` (guard-based `StepContext` borrowing committed for
  v0.7.0). Rationale: the highest existing ADR is 009; sequential numbering
  matches the established convention. Date/Author: 2026-06-09 / Claude (plan
  author).

- Decision: document the gpui API divergence with a banner plus a mapping table
  rather than rewriting the snippets to a single API. Rationale: the regression
  suite compiles against the vendored gpui; a one-API rewrite would either break
  the suite's mirroring contract or misrepresent what adopters compile against.
  Date/Author: 2026-06-09 / Claude.

- Decision: recommend macro-emitted `include_str!` as the preferred
  rebuild-invalidation mechanism, with the build-script (`cargo::rerun-if-changed`)
  route as a documented fallback for the directory-glob `scenarios!` case.
  Rationale: `include_str!` closes the loop invisibly to consumers and cannot be
  forgotten per call site; the build-script route is proven by `theoremc` but
  reintroduces the "emit one rerun-if line per file or regress" trap. Date/Author:
  2026-06-09 / Claude.

- Decision: keep ADR-008's `Proposed`→`Accepted` resolution as a separable,
  clearly-labelled roadmap follow-up, not a dependency of this feedback work.
  Rationale: it is pre-existing and orthogonal to the GPUI adopter feedback;
  bundling it would expand scope and blast radius. Date/Author: 2026-06-09 /
  Claude.

- Decision: treat this document as a DRAFT requiring explicit maintainer
  approval before Stages B–E execute, per the execplans approval gate.
  Rationale: the plan reschedules roadmap items and proposes an optional code
  change; both are maintainer calls. Date/Author: 2026-06-09 / Claude.

## Outcomes & retrospective

To be completed at delivery. Will compare the landed edits against the four
observable outcomes in `Purpose / big picture`, and record any feedback item
that could not be scheduled and why.

## Context and orientation

`rstest-bdd` is a Rust behaviour-driven-development framework: a procedural
macro (`#[scenario(path = "...")]`, `scenarios!`) binds a Gherkin `.feature`
file to step functions (`#[given]`/`#[when]`/`#[then]`) collected with
`inventory`. A **harness adapter** (`rstest_bdd_harness::HarnessAdapter`) wraps
how a generated scenario body executes; the **GPUI harness**
(`rstest-bdd-harness-gpui::GpuiHarness`) runs scenarios inside GPUI's headless
test app and injects a `gpui::TestAppContext` into steps via the reserved
fixture key `rstest_bdd_harness_context`. GPUI is Zed's UI framework; the
harness crate depends on a **vendored** gpui at `vendor/gpui`
(`Cargo.toml` line 73), pinned as `version = "0.2.2"` but exposing a test API
that differs from the published `gpui 0.2.2` on crates.io.

Key files this plan touches or references:

- `docs/roadmap.md` — phases 9 (harness adapters, delivered), 10 (v0.6.0-beta2
  quick wins, delivered), 11 (v0.6.1 additive hardening, open), 12 (v0.7.0
  pre-1.0 ambitions, open).
- `docs/rstest-bdd-design.md` — `§2.7` harness adapters; `§2.7.6.1` borrow
  constraint (E0499/E0502); `§2.7.6.2` interim GPUI state pattern (lines
  1947–2021, contains the divergent snippet); `§2.7.6.3` v0.6.0-beta2 quick
  wins; `§2.7.6.4` v0.6.1 helpers; `§2.7.6.5` v0.7.0 redesign (lines 2058–2067);
  `§3.2.2` OUT_DIR AST-caching aspiration (lines ~1277–1282).
- `docs/users-guide.md` — "Stateful GPUI scenarios with durable handles"
  playbook (lines ~1088–1360), including "Reset protocol" (~1131) which
  mentions `#[serial]` (~1159) and the durable-handle snippets (~1264–1316).
- `docs/v0-6-0-migration-guide.md` — "Adopt GPUI harness configuration" (~325),
  "Migrate a stateful GPUI test" (~356), the migration checklist (~424), and
  "Two mutable fixtures trigger `E0499` or `E0502`" (~474).
- `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` — the canonical
  thread-local durable-handle regression suite the playbook mirrors.
- `crates/rstest-bdd-harness-gpui/src/gpui_harness.rs` — `augmented_panic_message`
  (the 10.1.4 affirmative implementation).
- Existing ADRs `docs/adr-007-…`, `docs/adr-008-…` (`Proposed`),
  `docs/adr-009-…` for format and status conventions.

Terms used below:

- **Vendored gpui / published gpui**: the `vendor/gpui` path dependency vs the
  crates.io `gpui 0.2.2` release; their test APIs differ in four shapes.
- **Rebuild invalidation**: making Cargo re-run the build when a `.feature`
  file changes, via emitted `include_str!` or a build-script
  `cargo::rerun-if-changed` directive.
- **Thread-local durable-handle pattern**: the v0.6-compatible interim pattern
  storing `Entity<T>` and `AnyWindowHandle` in a `thread_local! RefCell` with a
  two-sided reset protocol and a `Drop` cleanup fixture.

## Plan of work

The work is organized by deliverable. Stages B–D are documentation/ADR edits;
Stage E is an optional, separately-gated code fix. Each stage ends with the
relevant gates green before proceeding.

### Stage A — Research and orientation (complete, no edits)

Done. Findings are recorded in `Surprises & discoveries`, with sources in
`Artifacts and notes`. No further research is required to begin drafting.

### Stage B — Draft three ADRs (new files)

Create each ADR in `Proposed` status, following the house format. The
prescriptive content (context, options, decision, consequences) is given in
`Interfaces and dependencies`.

1. `docs/adr-010-feature-file-change-detection.md` — how compile-time scenario
   binding makes `.feature` edits visible to Cargo.
2. `docs/adr-011-first-party-scenario-state-and-cleanup.md` — where the
   scenario-local state helper and per-scenario cleanup live (generic core in
   `rstest-bdd` vs GPUI-specialized re-export), and the cleanup-ordering
   contract.
3. `docs/adr-012-guard-based-stepcontext-borrowing.md` — record the guard-based
   borrow redesign as a committed v0.7.0 direction, with the v0.6→v0.7 migration
   mapping.

Validation for Stage B: `make markdownlint` passes on the three new files; each
ADR cross-references the roadmap item(s) and design subsection(s) it governs.

### Stage C — Roadmap edits (`docs/roadmap.md`)

Apply, in document order, the additions and clarifications specified in
`Interfaces and dependencies` under "Roadmap edits". In summary:

1. Add a new Phase 10 quick-win subsection (proposed `10.3`, "Close the
   feature-file rebuild gap") with one non-breaking item, referencing `adr-010`.
2. Clarify delivered item 10.1.4 to record the affirmative outcome and its test.
3. Add new Phase 10.2 documentation items for: the gpui-version banner and
   mapping table; the lint-clean playbook variant; the nextest/`serial_test`
   interaction note; and the bulk-migration cookbook.
4. Re-scope Phase 11 items 11.1.3 and 11.1.4 to name the GPUI-specialized
   `GpuiScenarioState` helper and the cleanup-guard fixture macro, referencing
   `adr-011`; add a priority note recommending they (and the rebuild fix) be
   pulled forward to v0.6.0 final, flagged as a maintainer scheduling decision.
5. Amend the Phase 12 intro and item 12.1.1 to reference `adr-012` and state the
   borrow redesign is a committed direction.
6. Add a separable follow-up note recommending ADR-008 be moved from `Proposed`
   to `Accepted` (clearly marked as orthogonal to the GPUI feedback).

Validation for Stage C: `make markdownlint` passes; every new roadmap item has a
finish line and a Design Doc / ADR reference, matching the style of existing
items.

### Stage D — Design-document and adoption-guide edits

In `docs/rstest-bdd-design.md`:

1. Add a which-gpui banner and a vendored→published mapping table immediately
   before the `§2.7.6.2` snippet, and annotate the snippet as targeting the
   vendored gpui.
2. Add `§2.7.6.6 "Feature-file rebuild invalidation"` documenting the foot-gun
   and the `adr-010` decision, and tighten the `§3.2.2` OUT_DIR-caching
   paragraph to distinguish *invalidation* (correctness) from *caching*
   (performance).
3. Add `§2.7.6.7 "Test-runner parallelism and scenario state"` documenting the
   cargo test vs nextest matrix and the `#[serial]`/`#[file_serial]`/test-group
   guidance.
4. Refine `§2.7.6.4` to reference `adr-011` (the first-party helper) and
   `§2.7.6.5` to reference `adr-012` (the committed borrow redesign).

Then, in `docs/users-guide.md` (playbook) and
`docs/v0-6-0-migration-guide.md`:

1. Add the which-gpui banner and mapping table to the playbook; add the
   lint-clean variant (rename trimmed/`borrow` bindings instead of shadowing;
   use a `let … else { panic!(…) }` accessor instead of
   `unwrap_or_else(|| panic!(…))`/`expect`); and add the nextest/`serial_test`
   caveat beside the existing `#[serial]` guidance.
2. Add a "feature-file edits and rebuilds" caveat to the migration guide's
   "Common errors and fixes", cross-linking `adr-010` and the new design
   subsection. Mark it as removable once the Stage E fix lands.
3. Add a short "bulk migration: sharing a step library" cookbook subsection to
   the playbook, factoring the durable-handle helpers into one shared steps
   module per consuming crate.

Validation for Stage D: `make markdownlint`, `make nixie`, and `make vale` pass;
the playbook and design snippets agree with each other and with the mapping
table.

### Stage E — Optional rebuild-invalidation fix (maintainer-gated code change)

Only if the maintainer approves landing the fix in this branch rather than as a
separate roadmap ExecPlan. Following Red-Green-Refactor:

1. Red: add a regression test that proves a `.feature`-only edit forces a
   rebuild/refailure (modelled on `theoremc`'s `tests/build_discovery_bdd.rs`,
   including a one-second `mtime` tick before the edit). Run it; expect failure
   against the current `std::fs`-read macro.
2. Green: have the `#[scenario]`/`scenarios!` expansion emit
   `include_str!("<absolute path>")` (to a discarded `const`) for each bound
   `.feature` file, with the path built from `CARGO_MANIFEST_DIR` so call-site
   semantics are unchanged. Re-run the regression test; expect pass.
3. Refactor: ensure no existing call site changes; run `make check-fmt`,
   `make lint`, and `make test`.

Validation for Stage E: the new regression test fails before and passes after;
`make test` is green; the migration-guide caveat from Stage D step 6 is updated
to name the release that carries the fix.

## Concrete steps

Run from the worktree root.

Per-milestone documentation gates (run sequentially, never in parallel):

```bash
make markdownlint 2>&1 | tee "/tmp/markdownlint-rstest-bdd-$(git branch --show-current).out"
make nixie        2>&1 | tee "/tmp/nixie-rstest-bdd-$(git branch --show-current).out"
make vale         2>&1 | tee "/tmp/vale-rstest-bdd-$(git branch --show-current).out"
```

Expected: each exits zero. `make nixie` is a no-op for files without Mermaid but
is run because the design doc is edited.

CodeRabbit validation after each milestone (only once the gates above are
green):

```bash
coderabbit review --agent 2>&1 | tee "/tmp/coderabbit-rstest-bdd-$(git branch --show-current).out"
```

If CodeRabbit reports its rate limit is exceeded, wait and retry:

```bash
vsleep "$(shuf -i 45-90 -n 1)m"
```

Stage E gates (only if Stage E is approved):

```bash
make check-fmt 2>&1 | tee "/tmp/check-fmt-rstest-bdd-$(git branch --show-current).out"
make lint      2>&1 | tee "/tmp/lint-rstest-bdd-$(git branch --show-current).out"
make test      2>&1 | tee "/tmp/test-rstest-bdd-$(git branch --show-current).out"
```

## Validation and acceptance

Acceptance is observable in the repository:

- `docs/adr-010-feature-file-change-detection.md`,
  `docs/adr-011-first-party-scenario-state-and-cleanup.md`, and
  `docs/adr-012-guard-based-stepcontext-borrowing.md` exist, are in `Proposed`
  status, follow the house format, and each cross-reference their roadmap item
  and design subsection.
- `docs/roadmap.md` contains: the new feature-file rebuild item referencing
  `adr-010`; a clarified 10.1.4 naming the `scenario_name_in_logs.rs` evidence;
  the four new documentation items; re-scoped 11.1.3/11.1.4 referencing
  `adr-011`; an amended 12.1.1 referencing `adr-012`; and the labelled ADR-008
  follow-up note.
- `docs/rstest-bdd-design.md` contains the which-gpui banner and mapping table
  by `§2.7.6.2`, the new `§2.7.6.6` and `§2.7.6.7` subsections, and the
  tightened `§3.2.2` wording.
- `docs/users-guide.md` and `docs/v0-6-0-migration-guide.md` carry the banner,
  mapping table, lint-clean variant, nextest caveat, feature-file caveat, and
  bulk-migration cookbook.
- Gates: `make markdownlint`, `make nixie`, `make vale` all pass.
- Review: `coderabbit review --agent` returns no unresolved concerns on the
  branch.

Quality criteria ("done"): every numbered feedback point from the adopter report
is either (a) reflected in a roadmap item, design subsection, or ADR, or (b)
explicitly recorded in `Outcomes & retrospective` as out-of-scope with a reason.

Because Stages A–D are documentation-only, the Red-Green-Refactor substitute is
the gate suite plus CodeRabbit: the "red" state is the current docs (missing the
corrections / containing the divergent snippet), and the "green" state is the
edited docs passing all gates and review. Stage E uses true Red-Green-Refactor
as described above.

## Idempotence and recovery

- All Stage A–D edits are additive or clarifying Markdown changes; re-running
  the gates is safe and repeatable. New ADR files are created once; re-running
  `git checkout` restores prior states.
- The only mildly destructive edit is rewording delivered roadmap item 10.1.4;
  it is recoverable via `git checkout -- docs/roadmap.md` before commit and via
  history afterwards. The reword only *clarifies* a delivered outcome and must
  not change its delivered scope (see Constraints).
- Stage E is fully reversible until committed (`git checkout -- crates/...`) and
  is gated behind explicit maintainer approval.

## Artifacts and notes

Cited sources backing the external-tooling claims (gathered Stage A):

- `include_str!`/`include_bytes!`/`include!` register dep-info entries so Cargo
  rebuilds on edits; plain `std::fs` reads in a proc-macro do not:
  <https://github.com/rust-lang/cargo/issues/1510>,
  <https://doc.rust-lang.org/std/macro.include_str.html>,
  <https://github.com/rust-lang/rust/issues/58069#issuecomment-1197286157>.
  Path caveat: `include_str!` resolves relative to the invoking source file.
- `cargo::rerun-if-changed` (double-colon form, Rust 1.77+; single-colon
  `cargo:` for older): directory targets scan recursively by mtime; emitting no
  `rerun-if` directive makes Cargo re-run the script on any package-file change,
  but emitting any directive switches to a narrow allow-list:
  <https://doc.rust-lang.org/cargo/reference/build-scripts.html>.
- cucumber-rs parses `.feature` files at runtime (`World::run(path)`), avoiding
  the stale-cache foot-gun at the cost of compile-time validation:
  <https://cucumber-rs.github.io/cucumber/main/quickstart.html>.
- theoremc prior art (build-script route): always emits
  `cargo::rerun-if-changed=theorems` even when absent, recurses into nested
  directories with per-file lines, generates an OUT_DIR suite via `include!()`,
  and asserts the literal directive strings in `tests/build_discovery_bdd.rs`
  (with a one-second mtime tick before edits): <https://github.com/leynos/theoremc>.
- nextest runs each test in its own process and parallelises across them
  (<https://nexte.st/docs/design/how-it-works/>); `serial_test`'s `#[serial]`
  is in-process only, so cross-process serialization needs `#[file_serial]`
  (<https://docs.rs/serial_test/>, <https://github.com/palfrey/serial_test>).

The vendored→published gpui mapping table (to be embedded in the design doc and
playbook):

| Operation | Vendored gpui (`vendor/gpui`, regression suite + current docs) | Published `gpui 0.2.2` (downstream adopter) |
| --- | --- | --- |
| `add_window_view` closure | `\|_context\| View::default()` (one argument) | `\|_window, view_cx\| View::new(view_cx)` (two arguments) |
| obtain window handle | `visual_cx.window_handle()` on `VisualTestContext` | `vcx.update(\|window, _app\| window.window_handle())` via `Window::window_handle()` |
| `VisualTestContext::from_window` | returns `Option<VisualTestContext>` (`.unwrap_or_else`/`.ok_or`) | returns `VisualTestContext` by value |
| `read_entity` / `update_entity` | `Option`/`Result` wrappers (`Some(1)`, `Ok(())`) | identity `type Result<T> = T`; returns `R` directly |

## Interfaces and dependencies

This section is prescriptive. It names the exact files and the content each must
contain at the end of the milestone.

### Files touched (tolerance bound: these eight only)

1. `docs/adr-010-feature-file-change-detection.md` (new)
2. `docs/adr-011-first-party-scenario-state-and-cleanup.md` (new)
3. `docs/adr-012-guard-based-stepcontext-borrowing.md` (new)
4. `docs/roadmap.md` (edit)
5. `docs/rstest-bdd-design.md` (edit)
6. `docs/users-guide.md` (edit)
7. `docs/v0-6-0-migration-guide.md` (edit)
8. `docs/execplans/adopt-v0-6-0-beta2-feedback.md` (this plan)

Stage E, if approved, additionally touches the macros crate
(`crates/rstest-bdd-macros/`) and adds one regression test target; that expands
the tolerance bound and must be re-approved.

### ADR-010 — Feature-file change detection for compile-time scenario binding

- Status: `Proposed`.
- Context: `#[scenario(path = ...)]` and `scenarios!` read `.feature` files via
  `std::fs` at macro-expansion time. Cargo does not track those reads, so a
  `.feature`-only edit does not trigger a rebuild; a corrupted expectation can
  appear to pass from stale cache until an unrelated `.rs` file changes. This is
  a correctness foot-gun for a testing framework.
- Options:
  1. Macro-emitted `include_str!("<absolute path>")` (to a discarded `const`)
     for each bound feature file. rustc registers the dep automatically; the fix
     is invisible to consumers and cannot be forgotten per call site. Caveat:
     `include_str!` resolves relative to the invoking source file, so the macro
     must emit an absolute path derived from `CARGO_MANIFEST_DIR`.
  2. A shipped build-script helper emitting `cargo::rerun-if-changed` for the
     features directory plus one line per discovered `.feature` file (the
     `theoremc` pattern). Robust for the directory-glob `scenarios!` case, but
     reintroduces the "emit one line per file or regress" trap and adds a
     build-script obligation.
  3. OUT_DIR AST caching (the existing `§3.2.2` aspiration). Orthogonal — it is
     a *performance* optimization, not an *invalidation* mechanism — and does
     not by itself solve the foot-gun.
- Decision: prefer option 1 for `#[scenario]` (single known path) and offer
  option 2 as a complement for `scenarios!` (directory glob, file set unknown
  until build). Add a regression test asserting invalidation, treating it as a
  tested contract. Distinguish invalidation from caching in `§3.2.2`.
- Consequences: closes the foot-gun without consumer action; the absolute-path
  emission must be covered by a path-resolution test; the build-script
  complement is opt-in for `scenarios!`.
- Governs roadmap item: new Phase 10.3 rebuild item. Design Doc: new `§2.7.6.6`.

### ADR-011 — First-party scenario-state helpers and per-scenario cleanup

- Status: `Proposed`.
- Context: every stateful GPUI scenario hand-rolls a `thread_local! RefCell`
  plus a `Drop` cleanup guard and a two-sided reset protocol (see
  `stateful_window.rs`). This boilerplate is the dominant adoption cost and is
  identical across scenarios. Roadmap 11.1.3/11.1.4 propose a generic helper and
  cleanup registration; the adopter asks specifically for a GPUI-shaped
  `GpuiScenarioState<T>` and a cleanup-guard fixture macro.
- Options: (a) ship only a generic `ScenarioState<T>` in `rstest-bdd`; (b) ship
  only a GPUI-specific helper in `rstest-bdd-harness-gpui`; (c) ship a generic
  core in `rstest-bdd` (with `set`/`with`/`with_mut`/`take`/`reset` plus cleanup
  registration) and re-export a GPUI-specialized `GpuiScenarioState` and
  cleanup-guard fixture macro from `rstest-bdd-harness-gpui`.
- Decision: option (c). It keeps the helper reusable for future harnesses (e.g.
  a Bevy `World`) while giving GPUI adopters a zero-boilerplate path. The ADR
  fixes the cleanup-ordering contract (reset before assignment in the opening
  step; cleanup via `Drop` after success, failure, and skip) and the
  registration order users must follow.
- Consequences: additive and semver-compatible (v0.6.1); the GPUI re-export
  depends on the generic core landing first.
- Governs roadmap items: re-scoped 11.1.3/11.1.4 and a new cleanup-guard-macro
  item. Design Doc: `§2.7.6.4`.

### ADR-012 — Guard-based `StepContext` borrowing committed for v0.7.0

- Status: `Proposed`.
- Context: `StepContext::borrow_mut(&mut self, ...)` returns a guard tied to the
  `&mut self` borrow, so a generated wrapper cannot borrow two distinct mutable
  fixtures at once — a step requesting both `&mut TestAppContext` and
  `&mut World` fails with `E0499`/`E0502` (design `§2.7.6.1`). This is the root
  cause of the thread-local workaround tax every GPUI adopter pays. Roadmap
  12.1.1 lists the redesign as a v0.7.0 *ambition*; the adopter recommends an
  explicit ADR confirming it is a *commitment*.
- Decision: record the guard-based redesign as a committed v0.7.0 direction:
  `Result`-returning borrow APIs carrying `FixtureBorrowError` (11.1.1),
  concurrent distinct-key mutable borrows, an opaque `FixtureRefMut` (12.1.2),
  and a stable world lifecycle (12.1.3). Include the v0.6→v0.7 migration mapping
  from the thread-local durable-handle pattern to the lifecycle hooks, so
  adopters can plan.
- Consequences: a breaking change reserved for v0.7.0 with a migration guide;
  it supersedes the interim pattern of `§2.7.6.2`. Pairs with the v0.6.1
  additive helper (`adr-011`) as the stepping stone.
- Governs roadmap items: amended Phase 12 intro and 12.1.1. Design Doc:
  `§2.7.6.5`.

### Roadmap edits (`docs/roadmap.md`)

Prescriptive list (apply in document order, matching existing item style — each
new item carries a finish line and a `Design Doc:` / ADR reference):

- New subsection `### 10.3. Close the feature-file rebuild gap` with item
  `10.3.1`: "Editing only a `.feature` file triggers a scenario rebuild." Finish
  line: the `#[scenario]`/`scenarios!` expansion registers the feature file as a
  Cargo dependency (macro-emitted `include_str!` of an absolute path), and a
  regression test proves a `.feature`-only edit forces recompilation and a fresh
  failure. Non-breaking; candidate for v0.6.0 final. Design Doc: `§2.7.6.6`;
  ADR-010.
- Clarify delivered `10.1.4`: append that the affirmative branch shipped — the
  scenario name is embedded in the augmented panic message and tracing events
  (`crates/rstest-bdd-harness-gpui/src/gpui_harness.rs`), with regression tests
  `scenario_name_in_logs.rs` and
  `augmented_panic_message_includes_scenario_name_for_payload_type`. Do not
  alter its `[x]` status or delivered scope.
- New Phase 10.2 documentation items:
  - `10.2.4`: the GPUI playbook and design snippets state which gpui version
    they target and carry a vendored→published `gpui 0.2.2` mapping table.
    Finish line: banner + table present in `docs/users-guide.md` and
    `docs/rstest-bdd-design.md`; `make markdownlint` passes. Design Doc:
    `§2.7.6.2`.
  - `10.2.5`: a lint-clean playbook variant compiles under a pedantic profile
    (`clippy::shadow_reuse`, `clippy::expect_used`, the in-house
    `no_unwrap_or_else_panic`). Finish line: the playbook offers a no-shadowing,
    no-`unwrap_or_else`-panic variant using a `let … else { panic!(…) }`
    accessor. Design Doc: `§2.7.6.2`.
  - `10.2.6`: the playbook documents how nextest's process-per-test scheduling
    interacts with `#[serial]` and per-process thread-local scenario state.
    Finish line: the playbook states `#[serial]` is required for `cargo test`,
    redundant-but-harmless under nextest, and that cross-process exclusivity
    needs `#[file_serial]` or a nextest test-group. Design Doc: `§2.7.6.7`.
  - `10.2.7`: a bulk-migration cookbook shows sharing one durable-handle step
    library across many GPUI scenarios in a single consuming crate. Finish line:
    a cookbook subsection in the user guide. Design Doc: `§2.7.6.2`.
- Re-scope `11.1.3` to name the generic `ScenarioState<T>` core in `rstest-bdd`
  *and* the GPUI-specialized `GpuiScenarioState` re-export in
  `rstest-bdd-harness-gpui`; re-scope `11.1.4` to add a cleanup-guard
  fixture-generating macro. Reference ADR-011. Add a priority note recommending
  11.1.3/11.1.4 and 10.3.1 be pulled forward to v0.6.0 final, flagged as a
  maintainer scheduling decision (do not change phase placement unilaterally).
- Amend the Phase 12 introduction and item `12.1.1` to reference ADR-012 and
  state the guard-based borrow redesign is a committed v0.7.0 direction.
- Add a labelled follow-up note (separate from the GPUI feedback) recommending
  ADR-008 be advanced from `Proposed` to `Accepted`, since roadmap 9.7.1–9.7.4
  shipped under maintainer authorization while it remains `Proposed`.

### Design-document edits (`docs/rstest-bdd-design.md`)

- Before the `§2.7.6.2` code block: a which-gpui banner plus the mapping table
  from `Artifacts and notes`; annotate the snippet as vendored-gpui shaped.
- New `§2.7.6.6 Feature-file rebuild invalidation`: the foot-gun, the ADR-010
  decision (emitted `include_str!` preferred; build-script complement for
  `scenarios!`), and the path-resolution caveat.
- New `§2.7.6.7 Test-runner parallelism and scenario state`: the cargo test vs
  nextest matrix; `#[serial]` vs `#[file_serial]` vs nextest test-groups; why
  process-per-test isolates thread-local state.
- Tighten `§3.2.2`: distinguish OUT_DIR AST *caching* (performance) from feature
  file *invalidation* (correctness), cross-linking `§2.7.6.6`.
- `§2.7.6.4`: reference ADR-011 for the first-party helper placement.
- `§2.7.6.5`: reference ADR-012 for the committed borrow redesign.

### Adoption-guide edits

- `docs/users-guide.md` playbook: add the banner + mapping table; the lint-clean
  variant; the nextest/`serial_test` caveat beside the existing `#[serial]`
  guidance; and the bulk-migration cookbook subsection.
- `docs/v0-6-0-migration-guide.md`: add a "feature-file edits and rebuilds"
  entry to "Common errors and fixes", cross-linking ADR-010 and `§2.7.6.6`, and
  marked removable once the Stage E fix lands.

## Revision note

Initial draft (2026-06-09). Establishes the planning scope: three new ADRs,
roadmap additions/clarifications, design-doc corrections and two new
subsections, and adoption-guide corrections, all driven by the
`0.6.0-beta2` GPUI adopter feedback. Awaiting maintainer approval before Stages
B–E execute. Remaining work: community-of-experts review of this plan and the
ADR drafts, then per-milestone gating and CodeRabbit validation.
