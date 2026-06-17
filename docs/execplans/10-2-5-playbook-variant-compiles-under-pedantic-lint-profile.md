# Lint-clean GPUI playbook variant that compiles under a pedantic lint profile (10.2.5)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

A downstream team adopting the stateful-GPUI playbook in `docs/users-guide.md`
runs a stricter Clippy profile than this repository does: on top of the
workspace lints they also enable `clippy::shadow_reuse` and an in-house
"no `unwrap_or_else(|| panic!(…))`" lint (call it `no_unwrap_or_else_panic`).
Under that profile the playbook's current accessor shape
(`Option::unwrap_or_else(|| panic!(…))`, and the schematic
`let mut world = world.borrow_mut()` re-bind) does not compile cleanly, so the
adopter must hand-translate the worked example before it builds.

After this change, the playbook offers a **lint-clean accessor variant** that a
pedantic adopter can paste verbatim: it uses `let … else { panic!(…) }` with a
fresh binding name (no value-reusing shadow) and contains no `unwrap_or_else`
panic. Crucially, the variant is not merely asserted to be lint-clean — it is
**backed by a compiled, feature-gated regression module** that the repository's
own gate builds under `#![deny(clippy::shadow_reuse, clippy::expect_used,
clippy::unwrap_used)]`, plus a deterministic textual check that the module and
the documented snippet contain no `unwrap_or_else` and stay in sync.

You can observe success three ways:

1. `make lint` compiles the new feature-gated test target under the denied
   pedantic lints with `-D warnings` and passes.
2. `make test` runs a new `rstest-bdd` behavioural scenario that exercises the
   lint-clean accessor path, plus `rstest` unit tests for the accessor's happy
   and unhappy (panic) paths, and the Python doc-sync check via `pytest`.
3. Reading `docs/users-guide.md`, the "Lint-clean variant" subsection now
   carries a full worked accessor (not a one-line teaser) that matches the
   compiled module identifier-for-identifier, and a reader on a pedantic profile
   can copy it without edits.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

- Public trait contracts must not change. This is a v0.6.0-beta3 documentation
  and test quick-win (roadmap §10). Do not alter `StepContext`, `HarnessAdapter`,
  `GpuiHarness`, the reserved `rstest_bdd_harness_context` fixture key, or any
  macro surface.
- Do not remove or weaken the existing default-form playbook (the
  `unwrap_or_else(|| panic!(…))` worked example). The design document
  (`docs/rstest-bdd-design.md` §2.7.6.2) deliberately documents when that shape
  is appropriate; the lint-clean variant is offered *alongside* it, not as a
  replacement that deletes it.
- Do not suppress workspace lints. `Cargo.toml` keeps `expect_used` and
  `unwrap_used` at `deny`; AGENTS.md says keep `expect_used` strict. The
  lint-clean module must satisfy them by construction, never via `#[allow]`.
  (The workspace also denies `allow_attributes_without_reason`, so any `allow`
  would additionally need a reason — avoid `allow` entirely here.)
- The vendored-versus-published `gpui 0.2.2` mapping table is dual-track
  (roadmap note after §10.2.7). Any snippet added here targets the **vendored**
  gpui used by the regression suite and must carry/inherit the existing
  "which gpui" banner; do not introduce a third API dialect.
- British English with Oxford spelling in all prose (`docs/*.md`), per the
  documentation style guide and the en-GB-oxendict convention.
- `make check-fmt`, `make lint`, and `make test` must all succeed before any
  CodeRabbit review and before each commit (commit gating).

## Tolerances (exception triggers)

- Scope: if the change requires touching more than ~12 files or more than ~450
  net lines (excluding the ExecPlan and ADR prose), stop and escalate.
- Interface: if any public API signature or the `[features]`/public test
  surface of `rstest-bdd-harness-gpui` must change beyond adding one
  `[[test]]` entry and reusing the existing `native-gpui-tests` feature, stop
  and escalate.
- Dependencies: if a new crate dependency (Rust or Python) is required, stop and
  escalate. The plan as designed adds none.
- Verification environment: if `make test` cannot actually run the
  `native-gpui-tests` target in this sandbox (no display/native GPUI), stop and
  escalate to agree the fallback (compile + clippy via `make lint`, plus the
  textual and unit checks, with the BDD scenario verified in CI). See Risk R1.
- Iterations: if `make lint`/`make test` still fail after 3 focused attempts on
  a milestone, stop and escalate.
- Ambiguity: if the doc-sync check design (textual proxy for the in-house lint)
  proves unworkable, stop and present alternatives rather than improvising a
  heavier mechanism (e.g. a real dylint lint) unprompted.

## Risks

- Risk R1: the `native-gpui-tests` suite needs a native GPUI environment to
  *run*. `make lint` only compiles/clippy-checks it (which is enough to prove it
  builds under the pedantic lints), but `make test` must run the new BDD
  scenario to prove behaviour.
  Severity: medium. Likelihood: medium.
  Mitigation: roadmap §10.1.3 records that the automated GPUI suite already
  passes in CI, so the environment exists there. Mirror the existing
  `stateful_window.rs` setup exactly. If the local sandbox cannot run it,
  escalate per the Tolerances rather than marking behaviour verified.

- Risk R2: `no_unwrap_or_else_panic` has **no** built-in Clippy equivalent.
  Research (Trail of Bits dylint; Clippy lint index) confirms neither
  `unwrap_used`/`expect_used` nor any built-in lint flags
  `unwrap_or_else(|| panic!(…))`; Clippy's `expect_fun_call` actually *suggests*
  that form. A faithful in-house lint would be a bespoke dylint crate.
  Severity: medium. Likelihood: high (it is a certainty, not a maybe).
  Mitigation: do not build a dylint crate in this quick-win (blast radius,
  CI infra). Enforce the property deterministically with a textual gate (no
  `.unwrap_or_else(` in the lint-clean module or its documented snippet) and
  rely on `#![deny(clippy::shadow_reuse, clippy::expect_used,
  clippy::unwrap_used)]` for the lints Clippy *can* express. Record the
  rejected dylint option in the Decision Log and ADR-013, with citations, so the
  choice is auditable. (See `Interfaces and dependencies` for the exact
  reasoning chain.)

- Risk R3: doc-snippet drift. The worked example must match the compiled
  module identifier-for-identifier (the playbook already promises this for the
  default form). A hand-copied snippet can silently drift.
  Severity: medium. Likelihood: medium.
  Mitigation: add `scripts/check_lint_clean_playbook.py` (modelled on
  `scripts/check_gpui_mapping_table.py`) that extracts the documented accessor
  block and compares it to the compiled module's marked region, and wire it into
  `make lint` and the `pytest` set in `make test`.

- Risk R4: `make fmt` markdown step is not idempotent and can introduce
  MD013/MD039 violations (see memory `make-fmt-markdown-not-idempotent`).
  Severity: low. Likelihood: medium.
  Mitigation: run `make markdownlint` after any `make fmt`, before committing
  doc changes.

- Risk R5: dual-track maintenance multiplies. Adding a second mirrored suite
  (default + lint-clean) means a future vendored-gpui bump must update two
  suites *and* the mapping table (the roadmap note after §10.2.7 already flags
  the table). Severity: low–medium. Likelihood: medium.
  Mitigation: keep the lint-clean module minimal and bind it to the **existing**
  `stateful_window.feature` (no second feature file), so only the accessor lines
  diverge from the default suite; the doc-sync check (R3) makes any divergence a
  gate failure rather than silent drift; ADR-013 records the accepted cost.

- Risk R6: a shared `tests/common`-style module consumed by two integration-test
  binaries can raise `dead_code` warnings for items one binary does not use,
  which `-D warnings` turns into a hard failure.
  Severity: low. Likelihood: medium.
  Mitigation: follow the existing test-organisation convention
  (`docs/developers-guide.md` §7); if a shared module is awkward, keep one
  canonical `expect_stored` definition in the **ungated** unit-test file with
  sentinel markers and let the gated BDD module re-declare its own copy, with the
  doc-sync check (R3) guarding both against drift. Decide during Stage B and
  record in the Decision Log.

## Progress

- [ ] (Stage A) Orientation and red specification agreed (this plan approved).
- [ ] (Stage B) Red: new feature file + lint-clean module + unit tests added and
  observed to fail/not-yet-compile for the expected reason; doc-sync check added
  and observed to fail before the snippet is updated.
- [ ] (Stage C) Green: lint-clean module compiles under the denied pedantic
  lints; BDD scenario and unit tests pass; users-guide snippet updated to match;
  doc-sync check passes.
- [ ] (Stage D) Refactor + docs: design doc §2.7.6.2 note, ADR-013, developers
  guide section, roadmap tick; full gates green; CodeRabbit clean.

(Use timestamps when ticking these during execution.)

## Surprises & discoveries

- Observation: Clippy cannot express `no_unwrap_or_else_panic`; `expect_fun_call`
  recommends the very pattern the in-house lint forbids.
  Evidence: Clippy lint index entries for `unwrap_used`, `expect_used`,
  `expect_fun_call`, `panic`, `panic_in_result_fn`; Trail of Bits dylint README
  and "Write Rust lints without forking Clippy" (2021-11-09).
  Impact: shapes the verification strategy (textual proxy, not a Clippy lint);
  recorded in ADR-013.

(Append further discoveries during execution.)

## Decision log

- Decision: verify the variant with a compiled, feature-gated regression module
  as the source of truth, plus a textual doc-sync check — not a doctest and not
  a real dylint lint.
  Rationale: the repository has no mechanism that compiles `users-guide.md`
  snippets (confirmed: no skeptic/mdbook-test/doc-comment; the gpui lib sets
  `doctest = false`). The established repo pattern for keeping docs honest is a
  text-comparison Python check wired into `make lint` + `pytest`
  (`check_gpui_mapping_table.py`, `check_users_guide_links.py`). A compiled test
  module gives genuine "compiles under the pedantic profile" proof for the lints
  Clippy can express; the textual check covers the one it cannot.
  Date/Author: 2026-06-17, planning agent.

- Decision: reject building a `no_unwrap_or_else_panic` dylint crate in this
  phase.
  Rationale: it would be a brand-new bespoke lint (no existing dylint example
  covers panic-in-combinator), pulling dylint into the CI gate — disproportionate
  blast radius for a documentation quick-win in roadmap §10. Documented as an
  option in ADR-013 for a future phase.
  Date/Author: 2026-06-17, planning agent.

- Decision: offer the lint-clean accessor as an *additional* worked variant and
  keep the default `unwrap_or_else(|| panic!())` form.
  Rationale: the roadmap finish line says "offers a … accessor variant"; the
  design doc justifies the default form. Promoting lint-clean to the sole form
  (and converting `stateful_window.rs` in place) was the strongest alternative
  considered but is rejected: it rewrites a passing, feature-gated regression
  suite and orphans the design-doc rationale for when `unwrap_or_else(|| panic!())`
  is appropriate.
  Date/Author: 2026-06-17, planning agent.

- Decision (post-review): bind the lint-clean module to the existing
  `stateful_window.feature` rather than authoring a second feature file, and keep
  the canonical `expect_stored` accessor plus its unit tests **ungated** so they
  run in every environment.
  Rationale: fewer files, less GPUI runtime, and a smaller dual-track maintenance
  surface (Logisphere review — Wafflecat/Pandalump). Ungating the accessor makes
  the unit-test acceptance environment-independent.
  Date/Author: 2026-06-17, planning agent (incorporating Logisphere design
  review).

(Append further decisions during execution.)

## Outcomes & retrospective

(To be completed at milestones and at the end. Compare against Purpose.)

## Context and orientation

You are working in the `rstest-bdd` workspace. The relevant pieces:

- `docs/users-guide.md` — the consumer-facing guide. The stateful-GPUI playbook
  lives under "Using the GPUI harness" → "Stateful GPUI scenarios with durable
  handles" (around lines 1088–1460 at time of writing). Its worked example
  mirrors the regression suite "identifier for identifier". The existing
  "Lint-clean variant" subsection (around lines 1409–1433) is currently a short
  teaser that says roadmap 10.2.5 will flesh it out. **That subsection is the
  primary deliverable to expand.**
- `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` — the executable
  reference suite for the default-form playbook. It is self-gated with
  `#![cfg(feature = "native-gpui-tests")]`. The accessors that the lint-clean
  variant must re-express in `let … else` form are: `current_handles` (lines
  ~66–76, two `Option::unwrap_or_else(|| panic!())`), `read_counter_from_window`
  (lines ~93–101), and the two step bodies at lines ~125–150.
- `crates/rstest-bdd-harness-gpui/tests/features/stateful_window.feature` — the
  Gherkin the default suite binds to (two scenarios).
- `crates/rstest-bdd-harness-gpui/Cargo.toml` — declares `[features]
  native-gpui-tests = []`, sets `[lib] doctest = false, test = false`, and
  registers feature-gated integration tests via `[[test]]` with
  `required-features = ["native-gpui-tests"]` (e.g. `scenario_name_in_logs`).
- `Cargo.toml` (workspace root) — `[workspace.lints.clippy]` sets
  `pedantic = warn`, `unwrap_used = deny`, `expect_used = deny`,
  `allow_attributes_without_reason = deny`, `blanket_clippy_restriction_lints =
  deny`. It does **not** enable `shadow_reuse` (a Clippy *restriction* lint, allow
  by default). Naming individual restriction lints in a `deny`/`warn` attribute
  is permitted (only the blanket `clippy::restriction` group is denied).
- `Makefile` — `RUST_FLAGS ?= -D warnings`,
  `CARGO_FLAGS ?= --workspace --all-targets --all-features`,
  `CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(RUST_FLAGS)`. `make lint` runs
  `cargo clippy $(CLIPPY_FLAGS)` then `lint-python` and the `check_*.py` scripts.
  `make test` runs `cargo nextest run $(CARGO_FLAGS)` then a `pytest` set
  covering the doc-check scripts. Because `--all-features` is used, the
  `native-gpui-tests` target **is** compiled by `make lint` and run by
  `make test`.
- `scripts/check_gpui_mapping_table.py` + `scripts/tests/test_check_gpui_mapping_table.py`
  — the canonical "doc-sync" pattern to copy: a Python script that parses a
  Markdown region and compares it, wired into `make lint` and a `pytest` test
  wired into `make test`.
- `docs/documentation-style-guide.md` — ADR naming `adr-NNN-short-description.md`
  in `docs/`; ADR vs design-doc-note guidance; required ADR sections. Next free
  number is **ADR-013**.
- `docs/developers-guide.md` — home for internally facing conventions (it already
  documents the GPUI mapping-table check in a dedicated section). The new
  compiled-module-plus-doc-sync convention belongs here.
- `docs/rstest-bdd-design.md` §2.7.6.2 — "Interim GPUI state pattern"; already
  references the lint-clean variant and roadmap 10.2.5. Update its note to point
  at the now-delivered variant and ADR-013.

Terms defined:

- **Restriction lint**: a Clippy lint that is allow-by-default and only fires
  when explicitly enabled (e.g. `shadow_reuse`, `expect_used`, `unwrap_used`).
- **let-else**: `let PATTERN = EXPR else { DIVERGING_BLOCK };` — stable since
  Rust 1.65. With a fresh binding name it introduces no shadow.
- **Doc-sync check**: a deterministic script that fails the build when a
  documented code block diverges from its compiled source of truth.

## Plan of work

Stage A — understand and propose (no code changes). This plan. Go/no-go: user
approval.

Stage B — red. Establish failing/observable specifications first:

1. Reuse the **existing** `crates/rstest-bdd-harness-gpui/tests/features/stateful_window.feature`
   for the lint-clean binary; do **not** add a second feature file (Risk R5). The
   lint-clean module binds the same two scenarios from its own test binary.

2. Add the **ungated** canonical accessor and its `rstest` unit tests so the
   happy/unhappy paths always run, independent of the native-GPUI environment.
   Place a single small, well-named, gpui-free accessor — the canonical
   source-of-truth the playbook quotes — bounded by sentinel comments:

   ```rust
   // LINT-CLEAN-ACCESSOR-BEGIN
   /// Unwrap stored scenario state, panicking with `what` if the invariant
   /// (a handle was stored) has been violated. Lint-clean: no `unwrap_or_else`,
   /// no value-reusing shadow.
   fn expect_stored<T>(value: Option<T>, what: &str) -> T {
       let Some(value_present) = value else {
           panic!("{what}");
       };
       value_present
   }
   // LINT-CLEAN-ACCESSOR-END
   ```

   Add the `rstest` unit tests beside it: a happy path (`Some(value)` returns the
   value — assert with `googletest`/`pretty_assertions`) and an unhappy path
   (`None` panics with the supplied message — `#[should_panic(expected = "…")]`).
   Because `expect_stored` is gpui-free and the file is ungated, these run under
   `make test` even where native GPUI is unavailable. Decide during this step
   whether the gated BDD module (step 3) shares this definition via a
   `tests/common`-style module or re-declares its own copy guarded by the
   doc-sync check (Risk R6); record the choice in the Decision Log.

3. Add the gated BDD module
   `crates/rstest-bdd-harness-gpui/tests/stateful_window_lint_clean.rs`,
   self-gated with `#![cfg(feature = "native-gpui-tests")]` and headed by
   `#![deny(clippy::shadow_reuse, clippy::expect_used, clippy::unwrap_used)]`.
   Note: the `#![deny(...)]` is a **regression guard** — the let-else form is
   shadow-free and unwrap-free by construction, so the attribute proves the form
   *stays* clean under the pedantic profile rather than proving the default suite
   violated those lints. The module mirrors `stateful_window.rs` but re-expresses
   every accessor in lint-clean form, e.g. the reconstruction accessor:

   ```rust
   let Some(mut visual_context) =
       gpui::VisualTestContext::from_window(window, context)
   else {
       panic!("stored window handle should reconstruct visual context");
   };
   ```

   Bind the two scenarios from the existing `stateful_window.feature` with
   `#[serial]` and the `scenario_state_cleanup` fixture, exactly as
   `stateful_window.rs` does.

4. Register both new test binaries in
   `crates/rstest-bdd-harness-gpui/Cargo.toml` (the gated BDD binary requires the
   feature; the ungated accessor/unit-test binary does not):

   ```toml
   [[test]]
   name = "stateful_window_lint_clean"
   path = "tests/stateful_window_lint_clean.rs"
   required-features = ["native-gpui-tests"]
   ```

5. Add `scripts/check_lint_clean_playbook.py` (model:
   `scripts/check_gpui_mapping_table.py`). It must:
   - read the sentinel-bounded (`LINT-CLEAN-ACCESSOR-BEGIN/END`) canonical region
     from the ungated accessor file;
   - read the fenced Rust block(s) under the "Lint-clean variant" subsection of
     `docs/users-guide.md`;
   - normalise both sides **explicitly** before comparing: strip a single leading
     doctest hidden-line marker (a `#` optionally followed by one space) from each
     doc line, strip common leading indentation, strip trailing whitespace, and
     drop blank lines; then compare the resulting line sequences;
   - assert neither side contains `.unwrap_or_else(` (the deterministic proxy for
     `no_unwrap_or_else_panic`) nor a value-reusing self-shadow of the form
     `let <ident> = <ident>`;
   - exit non-zero with a precise unified diff on mismatch.
   Add `scripts/tests/test_check_lint_clean_playbook.py` (pytest) covering a
   matching case (pass), a drift case (fail), an `unwrap_or_else`-present case
   (fail), and a hidden-line-marker normalisation case (pass). Use `cuprum`, not
   `subprocess`, if a subprocess is needed (per project convention).

6. Wire the new script into `make lint` (alongside the other `check_*.py`
   invocations) and into the `pytest` invocation inside `make test`.

   Observe red: before the users-guide snippet is updated to match, running
   `python3 scripts/check_lint_clean_playbook.py` must fail; the new BDD scenario
   must fail to bind until its steps exist; the unit test for the unhappy path
   must fail until `expect_stored` panics correctly.

Stage C — green. Make the smallest changes to pass:

1. Implement the step bodies so the BDD scenario passes (`make test` runs the
   `native-gpui-tests` target — see Risk R1).
2. Expand the "Lint-clean variant" subsection of `docs/users-guide.md` into a
   full worked accessor that matches the compiled module's sentinel region
   verbatim. Keep the existing two-pattern explanation (shadow_reuse and
   `unwrap_or_else`) and the existing teaser's `let … else` example, but replace
   "Roadmap item 10.2.5 tracks updating this playbook …" with the delivered
   variant and a pointer to the regression module and ADR-013. Preserve the
   "which gpui" banner applicability.
3. Run `python3 scripts/check_lint_clean_playbook.py` — expect pass.

Stage D — refactor, documentation, cleanup:

1. `docs/rstest-bdd-design.md` §2.7.6.2: update the existing lint-clean note to
   state the variant is delivered, name the regression module, and cite ADR-013.
2. Create `docs/adr-013-<short-description>.md` (e.g.
   `adr-013-lint-clean-playbook-verification.md`) following the style guide:
   Status `Accepted` with date; Context (downstream pedantic profiles;
   `no_unwrap_or_else_panic` has no Clippy equivalent — cite the research);
   Decision (compiled feature-gated module as source of truth +
   `#![deny(shadow_reuse, expect_used, unwrap_used)]` + textual doc-sync proxy);
   Options Considered (doctest; real dylint lint — rejected with rationale);
   Consequences; Known Limitations (the textual proxy is not a true AST lint).
   Reference it from the design doc front matter / §2.7.6.2.
3. `docs/developers-guide.md`: add a section documenting the
   compiled-module-plus-doc-sync convention for lint-clean snippets and how to
   extend it when the gpui API or accessors change (mirrors the existing
   mapping-table-check section). Note the sentinel-comment contract.
4. Mark roadmap item 10.2.5 as done (`- [x] 10.2.5. …`) with a "Delivered
   (date):" note and a pointer to this ExecPlan, matching the house style of
   the 10.2.x entries.
5. Run full gates; request CodeRabbit; clear all concerns.

Each stage ends with validation; do not proceed past a failing stage.

## Concrete steps

Run all commands from the worktree root
(`/home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/<id>`). Log to
`/tmp` per the command-output convention, e.g.:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-rstest-bdd-$(git branch --show-current).out
make lint      2>&1 | tee /tmp/lint-rstest-bdd-$(git branch --show-current).out
make test      2>&1 | tee /tmp/test-rstest-bdd-$(git branch --show-current).out
make markdownlint 2>&1 | tee /tmp/mdlint-rstest-bdd-$(git branch --show-current).out
```

Focused red/green checks during Stage B/C:

```bash
# Doc-sync check (red before step 8, green after)
python3 scripts/check_lint_clean_playbook.py

# Its pytest (red→green)
uv run pytest scripts/tests/test_check_lint_clean_playbook.py

# Clippy on just the new target under the denied pedantic lints
cargo clippy -p rstest-bdd-harness-gpui --all-features --tests -- -D warnings

# The lint-clean accessor unit tests (no native env needed if expect_stored is gpui-free)
cargo nextest run -p rstest-bdd-harness-gpui --all-features expect_stored

# The lint-clean BDD scenario (needs native GPUI env — see Risk R1)
cargo nextest run -p rstest-bdd-harness-gpui --all-features stateful_window_lint_clean
```

Expected: the doc-sync check prints nothing and exits 0 once the snippet
matches; the focused clippy run reports no warnings; the unit tests show the
happy case passing and the `#[should_panic]` case passing; the BDD scenario
passes in an environment with native GPUI.

Do not run lint/format/test suites in parallel (shared build cache); run them
sequentially.

## Validation and acceptance

Red-Green-Refactor evidence to record during execution:

- Red: `python3 scripts/check_lint_clean_playbook.py` fails because the
  users-guide block does not yet match the compiled sentinel region; the new
  unit test's unhappy path fails before `expect_stored` is implemented to panic;
  the BDD scenario fails to resolve steps before they are written.
- Green: after Stage C, the doc-sync check exits 0, the unit tests pass
  (happy + `#[should_panic]`), and the BDD scenario passes under
  `native-gpui-tests`.
- Refactor: after Stage D, `make check-fmt`, `make lint`, `make test`, and
  `make markdownlint` all pass.

Two-tier acceptance (separate *compile-verified* from *behaviour-verified* so a
sandbox without native GPUI cannot let unrun behaviour masquerade as verified —
see Risk R1 and the pre-mortem):

- Compile-verified (must hold in every environment, including this sandbox):
  `make lint` compiles the lint-clean module under `#![deny(clippy::shadow_reuse,
  clippy::expect_used, clippy::unwrap_used)]` with `-D warnings`; the ungated
  `rstest` unit tests (happy + `#[should_panic]`) pass under `make test`; the
  doc-sync `pytest` passes. These alone substantiate "compiles under a pedantic
  lint profile".
- Behaviour-verified (must hold where native GPUI is available — CI per roadmap
  §10.1.3, else escalate per Tolerances, do **not** mark done from compilation
  alone): the `native-gpui-tests` BDD scenario binding runs and passes.

Quality criteria ("done"):

- Tests: ungated `rstest` unit tests (happy + unhappy) pass everywhere; the
  `rstest-bdd` scenario passes under `native-gpui-tests` (CI or native env); the
  new `pytest` for the doc-sync check passes.
- Lint/typecheck: `make lint` passes, including `cargo clippy
  --workspace --all-targets --all-features -- -D warnings` (which compiles the
  lint-clean module under `#![deny(clippy::shadow_reuse, clippy::expect_used,
  clippy::unwrap_used)]`) and the new `check_lint_clean_playbook.py`.
- Docs: `make markdownlint` passes; users-guide variant matches the compiled
  module; design doc §2.7.6.2, ADR-013, and developers guide updated; roadmap
  10.2.5 ticked.
- No new dependencies; no public API change.

Quality method: run the four `make` targets sequentially with `tee`, then a
`coderabbit review --agent` pass per milestone, clearing all concerns before the
next milestone. CodeRabbit is requested only after the deterministic gates are
green.

Test rigour judgement (per the task's testing menu):

- `rstest` unit tests: yes — `expect_stored` happy/unhappy paths.
- `rstest-bdd` behavioural test: yes — the lint-clean scenario.
- `googletest` + `pretty_assertions`: yes — in the unit tests.
- `insta` snapshots: not warranted — there is no multivariant rendered output;
  the doc-sync check is an exact textual match, better expressed as a diff than
  a snapshot. (Record this judgement in the Decision Log if challenged.)
- `proptest`/`kani`: not warranted — the change introduces no new invariant over
  a range of inputs/states; the accessor is a total function on `Option<T>` whose
  two cases the unit tests already cover exhaustively.
- `verus`: not warranted — no new lemma or contractual business logic; the
  let-else accessor is a direct restatement, not a property needing proof.

## Idempotence and recovery

All steps are additive and re-runnable. New files can be deleted to revert; the
single Makefile and Cargo.toml edits are small and localised. If `make fmt`
reorders Markdown and trips MD013/MD039, re-run `make markdownlint` and fix
before committing (Risk R4). Commit after each green stage so any stage can be
rolled back with `git revert`.

## Artifacts and notes

Key reference shapes (vendored gpui dialect; default form → lint-clean form):

```text
state.entity.unwrap_or_else(|| panic!("…"))
  → let Some(entity) = state.entity else { panic!("…"); };   // fresh binding

VisualTestContext::from_window(window, cx).unwrap_or_else(|| panic!("…"))
  → let Some(mut visual_context) = VisualTestContext::from_window(window, cx)
        else { panic!("…"); };
```

Research citations to fold into ADR-013:

- Clippy lint index: `shadow_reuse`, `shadow_same`, `shadow_unrelated`,
  `unwrap_used`, `expect_used`, `expect_fun_call` (recommends
  `unwrap_or_else(|| panic!())`), `panic`, `panic_in_result_fn`, `manual_let_else`
  — <https://rust-lang.github.io/rust-clippy/master/index.html>.
- Trail of Bits dylint: <https://github.com/trailofbits/dylint> and
  "Write Rust lints without forking Clippy"
  — <https://blog.trailofbits.com/2021/11/09/write-rust-lints-without-forking-clippy/>.
- let-else stabilised in Rust 1.65 —
  <https://blog.rust-lang.org/2022/11/03/Rust-1.65.0/>.

## Interfaces and dependencies

New files:

- An **ungated** accessor + unit-test file (e.g.
  `crates/rstest-bdd-harness-gpui/tests/lint_clean_accessor.rs`) — the canonical,
  sentinel-marked (`LINT-CLEAN-ACCESSOR-BEGIN/END`) `fn expect_stored<T>(value:
  Option<T>, what: &str) -> T` plus its `rstest` happy/unhappy unit tests; gpui-free
  so it runs in every environment.
- `crates/rstest-bdd-harness-gpui/tests/stateful_window_lint_clean.rs` — gated BDD
  module; `#![cfg(feature = "native-gpui-tests")]` +
  `#![deny(clippy::shadow_reuse, clippy::expect_used, clippy::unwrap_used)]`;
  binds the existing `stateful_window.feature` scenarios using let-else accessors.
- `scripts/check_lint_clean_playbook.py` — doc-sync + `no_unwrap_or_else_panic`
  textual proxy.
- `scripts/tests/test_check_lint_clean_playbook.py` — pytest for the above.
- `docs/adr-013-lint-clean-playbook-verification.md` — the decision record.

Edited files: `crates/rstest-bdd-harness-gpui/Cargo.toml` (one `[[test]]`),
`Makefile` (two lines: `make lint` + the `pytest` set), `docs/users-guide.md`
(expanded "Lint-clean variant"), `docs/rstest-bdd-design.md` (§2.7.6.2 note),
`docs/developers-guide.md` (new convention section), `docs/roadmap.md` (tick
10.2.5).

No new Rust crate or Python package dependencies. The in-house
`no_unwrap_or_else_panic` lint is **not** implemented as a dylint crate here; it
is approximated by the textual gate, with the dylint route recorded as a
deferred option in ADR-013.

## Signposted documentation and skills

- Skills: `leta` and `rust-router` (loaded; for navigation and Rust routing);
  `execplans` (this document); `rust-unit-testing` (rstest fixtures,
  googletest/pretty_assertions/should_panic shapes); `arch-decision-records`
  (ADR-013 Y-Statement framing); `proptest`/`kani`/`verus` (consulted and
  judged not warranted, see above); `commit-message` and `pr-creation` for
  delivery; `logisphere-design-review` (used to review this plan).
- Docs: `docs/users-guide.md` (deliverable), `docs/rstest-bdd-design.md`
  §2.7.6.1/§2.7.6.2/§2.7.6.5/§2.7.6.7, `docs/rstest-bdd-language-server-design.md`
  (unaffected; cross-referenced for completeness), `docs/developers-guide.md`,
  `docs/documentation-style-guide.md`, `docs/rust-doctest-dry-guide.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md` (extract-method,
  guard-clause, small-function rationale for `expect_stored`),
  `docs/gherkin-syntax.md` (one behaviour per scenario, declarative style),
  `docs/rust-testing-with-rstest-fixtures.md` (fixture naming/injection).

## Revision note

Initial draft (2026-06-17). Establishes the compiled-source-of-truth +
textual-doc-sync verification strategy after research confirmed `make lint`
already compiles the `native-gpui-tests` target under `-D warnings`, that
`shadow_reuse`/`expect_used`/`unwrap_used` are restriction lints opt-in per
attribute, and that no Clippy lint can express `no_unwrap_or_else_panic`.

Revision 2 (2026-06-17, post Logisphere design review — verdict ⚠️ proceed with
conditions). Changes: (1) the canonical `expect_stored` accessor and its unit
tests are now **ungated** so they run in every environment (Pandalump); (2) the
lint-clean module reuses the existing `stateful_window.feature` instead of a new
feature file (Wafflecat) — Risk R5 reframed as dual-track maintenance, new Risk
R6 for shared-module `dead_code` under `-D warnings`; (3) doc-sync normalisation
rules are now explicit, including stripping doctest hidden-line markers
(Telefono); (4) acceptance is split into compile-verified vs behaviour-verified
tiers (Doggylump pre-mortem); (5) the `#![deny(...)]` is documented as a
regression guard, not proof the default suite violated the lints (Buzzy Bee).
This affects the file inventory and Stage B/C steps but not the overall scope or
tolerances. Awaiting user approval before implementation.
