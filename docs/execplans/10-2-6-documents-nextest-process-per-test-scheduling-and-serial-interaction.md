# Document nextest process-per-test scheduling and `#[serial]` interaction (10.2.6)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE (delivered on 2026-06-27)

## Purpose / big picture

A reader of `docs/users-guide.md` who runs a stateful `rstest-bdd` suite under
both `cargo test` and `cargo nextest` should be able to answer three questions
without leaving the user guide:

1. Is `#[serial]` required? (Yes, under `cargo test`.)
2. Is `#[serial]` redundant under nextest? (Yes — redundant-but-harmless,
   because nextest runs each test in its own process.)
3. How do I serialise tests that must not run concurrently *across* processes
   or binaries? (Use `serial_test`'s `#[file_serial]` — which requires the
   `file_locks` feature — or a cargo-nextest *test-group* with
   `max-threads = 1`.)

The user guide already *asserts* these three facts in a single blockquote
(`docs/users-guide.md` lines ~1196–1208), and the design document records the
full matrix in `docs/rstest-bdd-design.md` §2.7.6.7 (lines ~2152–2194). That
prose was added speculatively in PR #519 ("Fold v0.6.0-beta2 GPUI adopter
feedback into design, roadmap, and ADRs") at the same time the roadmap item was
written. Roadmap item 10.2.6 is therefore *partially* satisfied in prose but
remains open for three concrete reasons that this plan closes:

- **Accuracy gap (the substantive defect).** The guidance names `#[file_serial]`
  but does not state that it is gated behind `serial_test`'s `file_locks`
  feature, nor that `#[serial]` and `#[file_serial]` do **not** mutually exclude
  one another (they use different lock mechanisms). An adopter who follows the
  current prose verbatim — the workspace pins `serial_test = "2"` with no
  `file_locks` feature — hits a compile error (`cannot find attribute
  file_serial`). This is the same class of latent-error defect that roadmap item
  10.2.4 corrected in the gpui mapping table. The matrix wording also overstates
  precision: it says `#[serial]` is "redundant-but-harmless under nextest for
  single-binary scenarios", but each test is its own process regardless of
  binary count, so `#[serial]` is redundant for *all* nextest scenarios; the
  "single-binary" qualifier is misleading and must go.
- **No worked example.** The finish line names two cross-process mechanisms but
  the playbook only links out to their documentation. An adopter has no
  copy-pasteable `.config/nextest.toml` test-group stanza or `#[file_serial]`
  snippet to start from, and no note that the file lock defaults to a path in
  the system temp directory.
- **Discoverability.** The guidance is buried inside the GPUI-specific "Reset
  protocol" subsection, although it applies to *any* stateful `rstest-bdd`
  suite. Nothing keeps the user-guide statement and the design matrix in step if
  a future edit weakens one of them.

After this change a reader will find a standalone, runner-agnostic subsection in
the user guide titled to match the design heading, stating the three (corrected)
facts, carrying the same two-column runner matrix as §2.7.6.7, and showing a
worked test-group stanza and a worked `#[file_serial]` snippet with the
`file_locks` caveat. A deterministic gate compares the two matrix tables row for
row — reusing the proven row-comparator from 10.2.4 — so the documents cannot
silently drift. The developer guide gains one short canonical note explaining
*why* the repository keeps `#[serial]` despite nextest isolation, which is the
genuine institutional-knowledge risk.

Success is observable as:

- `make markdownlint` and `make lint` pass, the latter now also running a new
  `scripts/check_serial_nextest_matrix.py` gate that compares the two matrix
  tables and fails on drift (demonstrated by a pytest companion).
- The user-guide subsection answers the three purpose questions, with corrected
  `file_locks` guidance and copy-pasteable examples.
- `docs/roadmap.md` item 10.2.6 is marked `[x]` with a delivery note.

## Scope decisions taken after expert review (for maintainer approval)

This plan was stress-tested by the Logisphere community-of-experts panel
(Pandalump, Wafflecat, Buzzy Bee, Telefono, Doggylump, Dinolump). The panel
verdict was **🔄 Revise: viable but over-scoped**. The first draft proposed an
executable demonstration test (rstest unit cases, a rstest-bdd scenario, a
`NEXTEST_EXECUTION_MODE` runtime guard, and a new `serial_test` dev-dependency)
plus a prose phrase-matching drift gate. The panel rejected both:

- The executable demonstration was judged non-falsifiable. Under nextest each
  test is its own process, so two `#[serial]` tests sharing a `thread_local!`
  counter never actually share it; under `cargo test` the reset helper alone
  guarantees the assertion. The runtime guard merely re-asserts cargo-nextest's
  own contract, and Doggylump found a concrete failure path: CI coverage legs
  run `cargo llvm-cov nextest` with `use-nextest: true`, setting `NEXTEST=1`,
  where the guard can false-assert and silently break coverage. **Cut entirely.**
- The prose phrase-matching gate is weaker than the 10.2.4 precedent it cited
  and breaks on benign `make fmt` rewraps. **Replaced** by a table-parity gate
  that compares the structured matrix rows, exactly as 10.2.4 compares the gpui
  mapping-table rows.

Two choices remain the maintainer's to confirm (asked, but defaulted here so the
plan is reviewable as a whole):

1. **Validation rigour: table-parity gate, no Rust tests.** Default taken. The
   alternatives are "docs-only, no gate" (lighter, relies on review) and "keep a
   hardened executable demonstration" (heavier). If you prefer either, say so at
   review and the plan adjusts; the table-parity gate is the panel's
   recommendation and matches how sibling item 10.2.4 shipped.
2. **Placement: promote to a runner-agnostic subsection.** Default taken. The
   alternative is to correct the guidance in place inside the GPUI stateful
   section (Wafflecat noted the guidance is GPUI-shaped today, since only GPUI
   scenarios share a process-wide `TestAppContext`). If you prefer in-place, the
   gate still applies but the subsection is not relocated.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

- **Do not change public trait contracts or macro surfaces.** Phase 10 is
  explicitly "small, non-breaking changes" (`docs/roadmap.md` §10). No edit to
  `StepContext`, the harness traits, or the
  `#[given]`/`#[when]`/`#[then]`/`#[scenario]` macros.
- **Do not add a live test-group to `.config/nextest.toml`.** The repository's
  own suite needs no cross-process exclusivity; the stanza is documented as an
  *example* only. Adding a real group would change scheduling for no benefit.
- **Do not enable `serial_test`'s `file_locks` feature on any committed
  workspace manifest, and do not add `serial_test` to any crate's
  dependencies.** The repository does not use `#[file_serial]`; the feature is
  adopter guidance only.
- **No new Rust test files, no new test crate, no new Rust dependency.** This is
  a documentation change plus one Python gate (per the post-review scope).
- **`docs/users-guide.md` cross-references use absolute GitHub URLs**, collected
  as reference-style link definitions at the bottom of the file, because the
  guide is vendored into consumer projects. `scripts/check_users_guide_links.py`
  (run by `make lint`) enforces this; any new link must follow the convention.
- **Markdown house style** (`docs/documentation-style-guide.md`, `AGENTS.md`
  lines ~307–314): en-GB Oxford spelling; prose and bullets wrap at 80 columns;
  code blocks at 120; tables and headings are not wrapped; dashes for bullets.
- **The user-guide matrix and the §2.7.6.7 matrix must be identical** after
  normalisation; the new gate enforces this row for row.

## Tolerances (exception triggers)

- **Scope.** If implementation requires changing more than 4 files (the two
  docs, the developer guide, the Makefile) plus the new script and its pytest
  companion, or more than ~250 added/changed lines, stop and escalate.
- **Interface.** If any public API signature, macro surface, or trait must
  change, stop and escalate.
- **Dependencies.** No new Rust dependency is in scope. If one seems necessary,
  stop and escalate.
- **Iterations.** If the gate or its companion still fails after 3 focused
  attempts, stop and escalate with the transcript.
- **Gate brittleness.** If the table-parity gate cannot be made robust to
  `make fmt` reflow without resorting to prose phrase-matching, stop and
  escalate rather than shipping a brittle matcher.
- **Ambiguity / placement.** If the maintainer wants the guidance kept GPUI-
  scoped rather than promoted, adjust placement before writing the gate around a
  single canonical location.

## Risks

- Risk: A table-parity gate, like 10.2.4's, fires when the two matrices are
  legitimately reworded together.
  Severity: low. Likelihood: medium.
  Mitigation: Compare normalised *data rows only* (collapse internal
  whitespace, ignore the header/separator), as `check_gpui_mapping_table.py`
  does; reword both tables together and the gate stays green. The pytest
  companion proves the gate fails on a single-row divergence.

- Risk: `make fmt` reflows the new subsection and reintroduces MD013/MD039 lint
  errors (a known repository quirk), or wraps a table cell.
  Severity: low. Likelihood: medium.
  Mitigation: Markdown tables are not wrapped by house style; run
  `make markdownlint` after `make fmt`, per the recorded memory on `make fmt`
  Markdown non-idempotence, and fix before committing. The gate reads table
  rows, which `mdformat` keeps on one line each.

- Risk: The worked `#[file_serial]` snippet is `rust,no_run` and is not
  compiled by any gate (the user guide's Rust fences are not wired into
  doctests; `make test` uses nextest, which skips doctests).
  Severity: low. Likelihood: low.
  Mitigation: Keep the snippet minimal and idiomatic, matching the verified
  `serial_test` API; this is consistent with every other Rust fence in the user
  guide. The substantive correctness claim (the `file_locks` feature) is prose,
  not code, and is backed by the cited docs.rs reference.

- Risk: `serial_test` major-version skew — the workspace root pins `"2"` while
  `rstest-bdd-harness-tokio` pins `"3"`; an adopter who copies a `version = "3"`
  fragment into a tree resolving `"2"` hits a conflict.
  Severity: low. Likelihood: low.
  Mitigation: Make the adopter manifest fragment version-agnostic and state that
  `file_locks` is the load-bearing part, not the major version (the feature
  exists in both 2.x and 3.x).

## Progress

- [x] (Stage A) Confirm placement and validation rigour with the maintainer;
  capture the verbatim "before" text of the three required claims. (No code
  changes.)
- [x] (Stage B) Red: add the failing `scripts/check_serial_nextest_matrix.py`
  gate and its pytest companion. Observe the gate failing because the user guide
  has no matrix table yet (only §2.7.6.7 does).
- [x] (Stage C) Green: add the runner matrix table and the corrected,
  consolidated subsection to `docs/users-guide.md`; correct
  `docs/rstest-bdd-design.md` §2.7.6.7 (drop the "single-binary" qualifier; add
  the `file_locks` caveat, the serial-vs-file_serial non-exclusion note, the
  worked test-group stanza, the file-lock temp-dir default, and the minimum
  nextest version for groups); add the canonical convention note to
  `docs/developers-guide.md`. Make the gate pass.
- [x] (Stage D) Refactor; wire the gate into `make lint` and the companion into
  the `make test` pytest line; run all gates; run `coderabbit review --agent`;
  clear concerns.
- [x] (Stage E) Mark roadmap 10.2.6 `[x]`; finalise the living sections.

## Surprises & discoveries

- Observation: Implementation approval arrived in the user request for this
  session, so the ExecPlan approval gate is satisfied without a separate review
  pause.
  Evidence: The user asked to "proceed with implementation of the planned
  functionality" in this plan.
  Impact: The default scope decisions remain active: table-parity gate, no Rust
  tests, and a promoted runner-agnostic subsection.

- Observation: PR metadata was still plan-shaped at implementation start.
  Evidence: `gh pr view --json number,title,body,url` showed PR #547 titled
  `Plan: Document nextest process-per-test scheduling and #[serial]
  interaction (10.2.6)`.
  Impact: The PR title was updated to remove `Plan:`, the Lody session title
  was renamed to the new PR title, and the PR description's Lody session link
  was updated to
  `https://lody.ai/leynos/sessions/77300eb4-3c2c-489d-88f3-f28539dd420b`.

- Observation: The roadmap finish line is already met in prose by PR #519.
  Evidence: `docs/users-guide.md` ~1196–1208 and §2.7.6.7 already state all
  three claims and carry a matrix table (the table is currently only in the
  design doc).
  Impact: The work is correct-exemplify-consolidate-validate, not fresh prose —
  mirroring 10.2.4.

- Observation: `#[file_serial]` is gated behind `serial_test`'s `file_locks`
  feature, and `#[serial]` and `#[file_serial]` do not lock against each other.
  Evidence:
  `https://docs.rs/serial_test/latest/serial_test/attr.file_serial.html`
  ("Available on crate feature `file_locks` only"; "no guarantees about one test
  with serial and another with file_serial as they lock using different
  methods"; the lock path "defaults to a file under a reasonable temp directory
  for the OS").
  Impact: The current guidance is wrong-by-omission; correcting it is the
  primary substantive change.

- Observation: cargo-nextest explicitly does not support in-process mutexes;
  test-groups with `max-threads = 1` are its native cross-process mutex, applied
  across the whole run (cross-binary), available since nextest 0.9.48.
  Evidence: `https://nexte.st/docs/configuration/test-groups/`.
  Impact: Confirms the worked stanza, the matrix wording, and the dropped
  "single-binary" qualifier.

- Observation: The red gate failed for the intended reason before the user-guide
  subsection existed.
  Evidence: `python3 scripts/check_serial_nextest_matrix.py; echo "exit=$?"`
  printed `heading not found: Test-runner parallelism and scenario state` and
  `exit=1`; `uv run --group python-tools pytest
  scripts/tests/test_check_serial_nextest_matrix.py` reported `10 passed`.
  Impact: The Stage B matcher is executable and falsifiable before any
  production documentation change.

- Observation: The matrix gate passed after adding the user-guide subsection
  and correcting the design matrix prose.
  Evidence: `python3 scripts/check_serial_nextest_matrix.py; echo "exit=$?"`
  printed `exit=0`, and the focused pytest command reported `10 passed`.
  Impact: The duplicated runner matrix is now machine-checked before Makefile
  wiring.

- Observation: `make fmt` is not idempotent for all existing Markdown files in
  this repository.
  Evidence: `make fmt` completed Rust and Python formatting, then
  `markdownlint-cli2 --fix` reported unrelated MD013/MD039 failures in existing
  documents. The formatter also wrapped the existing blockquoted GPUI table and
  aligned the new runner table.
  Impact: Reverted the unrelated formatter changes, restored the GPUI table
  rows to the established checker-compatible shape, and made
  `scripts/check_serial_nextest_matrix.py` accept normal Markdown table
  alignment so `make fmt` table alignment cannot trip the new gate.

- Observation: The deterministic quality gates passed after the table-parser
  robustness fix.
  Evidence: `make check-fmt` passed; `make lint` passed including Clippy, Ruff,
  Pylint, users-guide links, GPUI table parity, and serial/nextest matrix
  parity; `make test` passed with nextest `1489 passed, 7 skipped` and Python
  helper tests `44 passed`; `make markdownlint` reported `0 error(s)`.
  Impact: The branch is ready for the requested CodeRabbit milestone review
  after committing and pushing the Stage B-D implementation.

- Observation: CodeRabbit found no concerns after the Stage B-D implementation
  commit was pushed.
  Evidence: `coderabbit review --agent` completed with
  `{"type":"complete","status":"review_completed","findings":0}`.
  Impact: No follow-up implementation changes were required before closing the
  roadmap item.

- Observation: The first-draft executable demonstration was non-falsifiable and
  fragile under coverage runners.
  Evidence: Community-of-experts panel review (Wafflecat, Doggylump, Dinolump);
  CI coverage legs use `use-nextest: true` (`.github/workflows/ci.yml`).
  Impact: Cut from scope; see Decision Log.

## Decision log

- Decision: Treat 10.2.6 as correct-exemplify-consolidate-validate, not fresh
  prose.
  Rationale: The finish-line claims already exist; honest delivery fixes the
  `file_locks` omission and the "single-binary" overstatement, adds worked
  examples, improves discoverability, and adds a table-parity regression gate,
  paralleling 10.2.4.
  Date/Author: 2026-06-27, planning agent.

- Decision: Proceed with the plan's default scope decisions.
  Rationale: The maintainer's implementation request did not redirect the two
  open decisions, so the accepted implementation uses the table-parity gate
  with no Rust tests and promotes the guidance to a runner-agnostic user-guide
  subsection.
  Date/Author: 2026-06-27, implementation agent.

- Decision: Cut the executable demonstration test, the rstest-bdd scenario, the
  `NEXTEST_EXECUTION_MODE` runtime guard, and the `serial_test` dev-dependency.
  Rationale: Panel verdict — the demonstration is non-falsifiable (process-per-
  test means the "shared" thread-local is never shared; the reset helper alone
  satisfies the cargo-test assertion) and the runtime guard re-asserts nextest's
  own contract while risking a silent coverage-leg failure under
  `cargo llvm-cov nextest`. Sibling item 10.2.4 shipped doc+gate with no Rust
  tests for the same class of item; matching that shape keeps reviewer
  recognition and avoids permanent CI weight for a one-time prose fix.
  Date/Author: 2026-06-27, planning agent (post-panel).

- Decision: Replace the prose phrase-matching gate with a table-parity gate
  (`scripts/check_serial_nextest_matrix.py`).
  Rationale: Panel verdict — phrase-matching free prose is brittle (false
  positives on `make fmt` reflow, e.g. `max-threads = 1` wrapping; invites
  "fix the script not the doc"). Comparing normalised matrix rows between the
  two documents is the objective, newline-stable contract 10.2.4 already proved.
  Naming follows the sibling (`check_gpui_mapping_table.py`) and names the
  structured artefact (the matrix), not the prose.
  Date/Author: 2026-06-27, planning agent (post-panel).

- Decision: Anchor both matrices with the exact heading
  "Test-runner parallelism and scenario state".
  Rationale: The design document already uses that phrase, and the red gate's
  missing-heading failure proves the user guide must grow the same
  runner-agnostic section rather than hiding the matrix inside the GPUI reset
  blockquote.
  Date/Author: 2026-06-27, implementation agent.

- Decision: Document the new checker in `docs/developers-guide.md` alongside
  the existing users-guide link and GPUI mapping-table validators.
  Rationale: The script is an internal maintenance interface enforced by
  `make lint`; the developer guide is the canonical home for such repository
  workflow conventions.
  Date/Author: 2026-06-27, implementation agent.

- Decision: Let the new runner-matrix checker accept aligned Markdown tables.
  Rationale: `mdtablefix` aligns table columns, so requiring a literal
  `| Runner |` prefix would make the gate brittle against the repository's own
  formatter. The checker still compares normalised data rows and the pytest
  companion now covers formatter-aligned input.
  Date/Author: 2026-06-27, implementation agent.

- Decision: Make the developer-guide convention note the canonical home for the
  "why keep `#[serial]`" rationale and the `file_locks` caveat.
  Rationale: Panel verdict (Dinolump) — the convention note is the one durable,
  low-toil artefact and captures the genuine institutional-knowledge risk; one
  canonical explanation beats restating a niche fact in four places. The user
  guide and design doc cross-reference it rather than duplicating the rationale.
  Date/Author: 2026-06-27, planning agent (post-panel).

- Decision: No new ADR.
  Rationale: The decision (keep `#[serial]`; document test-groups and
  `#[file_serial]` as the cross-process mechanisms; note the `file_locks` gate)
  is a low-blast-radius, reversible documentation recommendation, not a
  hard-to-reverse architectural choice. Recorded in §2.7.6.7 prose, the
  developer-guide note, and this log, per `docs/documentation-style-guide.md`.
  Date/Author: 2026-06-27, planning agent.

- Decision: No property tests, bounded model checking, deductive proof, or
  snapshot tests.
  Rationale: The change introduces no invariant over a range of inputs, states,
  or orderings, and produces no multivariant output format. Proportionate
  rigour per the task's "best judgement" clause; the only machine-checkable
  invariant (the two matrices agree) is covered by the table-parity gate.
  Date/Author: 2026-06-27, planning agent.

- Decision: Adopter manifest fragment is version-agnostic.
  Rationale: Panel verdict (Telefono, Pandalump) — pinning `serial_test = "3"`
  in an example contradicts the workspace `"2"` pin and can cause an adopter
  resolver conflict; `file_locks` is the load-bearing part and exists in both
  2.x and 3.x.
  Date/Author: 2026-06-27, planning agent (post-panel).

## Outcomes & retrospective

Delivered. The user guide now answers the three purpose questions in a
standalone "Test-runner parallelism and scenario state" subsection:

- `#[serial]` is required for `cargo test` stateful scenarios.
- `#[serial]` is redundant-but-harmless under nextest because nextest runs each
  test in a separate process.
- Cross-process exclusivity requires `#[file_serial]` with `serial_test`'s
  `file_locks` feature, or a cargo-nextest test-group with `max-threads = 1`.

The design document carries the same matrix and corrected caveats, and the
developer guide now records the canonical maintainer rationale for keeping
`#[serial]` while not adding a live repository test-group. The new
`scripts/check_serial_nextest_matrix.py` gate is wired into `make lint`, and
its pytest companion is wired into `make test`. The checker compares normalised
matrix rows and accepts formatter-aligned Markdown tables, avoiding the brittle
literal-prefix failure observed during the `make fmt` cycle.

Validation completed: `make check-fmt`, `make lint`, `make test`, and
`make markdownlint` passed. `make fmt` was attempted; it exposed an existing
repository-wide Markdown formatter non-idempotence where `mdtablefix` rewrites
unrelated documents and `markdownlint-cli2 --fix` then reports unrelated
MD013/MD039 errors. Those unrelated formatter edits were reverted, the active
docs were kept focused, and `make markdownlint` passed on the final tree.
CodeRabbit reviewed the pushed milestone with zero findings.

## Context and orientation

`rstest-bdd` is a Rust behaviour-driven-development layer over the `rstest`
fixture framework. Stateful GPUI scenarios share mutable state across steps
through a thread-local (`thread_local!`) `ScenarioState` and a two-sided reset
protocol, and each such scenario carries `#[serial]` from the
[`serial_test`](https://docs.rs/serial_test/) crate so that, under `cargo
test`, only one runs at a time on a shared test thread.

Key files (full repository-relative paths):

- `docs/users-guide.md` — the consumer-facing playbook. The relevant region is
  "Stateful GPUI scenarios with durable handles" (a `####` heading, lines
  ~1088–1486) with `#####` subsections including "Reset protocol" (the current
  `#[serial]`/nextest blockquote lives here, ~1196–1208). The next sibling `###`
  is "Skipping scenarios" at line ~1488. The user guide does **not** currently
  carry the runner matrix table; only the design doc does.
- `docs/rstest-bdd-design.md` §2.7.6.7 "Test-runner parallelism and scenario
  state" (lines ~2152–2194) — the authoritative matrix, including the two-column
  table at ~2181–2186 ("Runner | `#[serial]` effect | Cross-process
  exclusivity"). Surrounding subsections run §2.7.6.1 through §2.7.6.6.
- `docs/developers-guide.md` — has "nextest configuration (`.config/nextest.toml`)"
  (line ~108), "nextest on Windows: trybuild deadlock" (line ~136), and a
  "Thread-local state and test isolation" heading (line ~547). It has no
  `#[serial]`-vs-nextest convention note yet.
- `.config/nextest.toml` — the live nextest config (timeouts only; no
  test-groups). Not modified by this plan.
- `Makefile` — `make lint` runs Clippy then the Python doc gates
  (`check_rs_file_lengths.py`, `check_users_guide_links.py`,
  `check_gpui_mapping_table.py`). `make test` builds, runs the Rust suite under
  nextest (skipping doctests and trybuild compile-tests; see the memory
  "trybuild skipped under nextest"), then runs the pytest companions under
  `scripts/tests/`.
- `scripts/check_gpui_mapping_table.py` and its companion
  `scripts/tests/test_check_gpui_mapping_table.py` — the structural model for
  the new gate (heading lookup, whitespace-normalised row comparison, exit
  codes, custom error type).

Terms of art (defined on first use):

- **process-per-test**: cargo-nextest spawns a fresh OS process for each test;
  `NEXTEST_EXECUTION_MODE` is `"process-per-test"` at test runtime.
- **`#[serial]`**: a `serial_test` attribute that serialises annotated tests via
  an *in-process* mutex. It has no effect across process boundaries.
- **`#[file_serial]`**: a `serial_test` attribute (feature `file_locks`) that
  serialises annotated tests via a *file lock* (default path under the OS temp
  directory), so it works across processes and binaries. It does not lock
  against `#[serial]`.
- **nextest test-group**: a named concurrency limit in `.config/nextest.toml`;
  `max-threads = 1` is a logical mutex applied across the whole run (cross-
  binary), available since nextest 0.9.48.

## Plan of work

Stage A — understand and confirm (no code changes). Re-read the three artefacts.
Record, in the Decision Log, the verbatim "before" text of the three required
claims so the "after" can be shown not to weaken them. Confirm placement and
validation rigour with the maintainer (the two scope decisions above).

Stage B — red gate. Add `scripts/check_serial_nextest_matrix.py`, modelled
structurally on `scripts/check_gpui_mapping_table.py`:

- It locates the runner matrix table under the relevant heading in *both*
  `docs/users-guide.md` and `docs/rstest-bdd-design.md` §2.7.6.7, extracts the
  data rows, normalises internal whitespace per row, and exits non-zero with a
  human-readable per-row diff if the two tables differ or either is missing.
- Before Stage C it fails for the intended reason: the user guide has no matrix
  table yet (heading/table not found), so the gate reports "table not found
  under heading" — observe this exit code 1.
- A pytest companion `scripts/tests/test_check_serial_nextest_matrix.py` covers
  a passing fixture and at least two failing fixtures (missing table; a single
  divergent row), and asserts the gate *fails* on the deletion fixture so that
  weakening the matcher also breaks the companion.

  ```bash
  python3 scripts/check_serial_nextest_matrix.py; echo "exit=$?"   # expect exit=1
  ```

Stage C — implementation (documentation edits to turn the gate green).

- `docs/users-guide.md`: introduce a runner-agnostic `#####` subsection titled to
  match the design heading — "Test-runner parallelism and scenario state"
  (placed as the final child of the stateful section, before "Skipping
  scenarios"; or, if the maintainer keeps it GPUI-scoped, corrected in place).
  Carry the same two-column matrix table as §2.7.6.7. State the three corrected
  claims; correct the guidance so it (a) drops the "single-binary" qualifier,
  (b) states `#[file_serial]` requires `serial_test`'s `file_locks` feature,
  (c) notes `#[serial]` and `#[file_serial]` do not mutually exclude, and
  (d) notes the file lock defaults to a path in the OS temp directory with
  optional `path`/`key` overrides. Add a worked `.config/nextest.toml`
  test-group stanza and a worked `#[file_serial]` snippet plus a version-
  agnostic adopter manifest fragment
  (`serial_test = { version = "…", features = ["file_locks"] }`). Cross-
  reference the developer-guide convention note and §2.7.6.7. Replace the old
  blockquote with a one-line pointer to the new subsection. Add any new links as
  absolute-URL reference definitions.
- `docs/rstest-bdd-design.md` §2.7.6.7: drop the "single-binary" qualifier from
  the prose and keep the table wording in step with the user guide; add the
  `file_locks` caveat, the serial-vs-file_serial non-exclusion note, the file-
  lock temp-dir default, the worked test-group stanza, and the minimum nextest
  version (0.9.48) for test-groups.
- `docs/developers-guide.md`: add a short "`#[serial]`, `#[file_serial]`, and
  nextest test-groups" subsection near the existing nextest material. This is
  the canonical home for *why* the repository keeps `#[serial]` (cargo-test
  compatibility), relies on nextest process isolation, does not wire a live
  test-group, and treats `file_locks`/`#[file_serial]` as adopter-only. The user
  guide and design doc point here for the rationale.

Stage D — refactor, wire, and review. Append
`python3 scripts/check_serial_nextest_matrix.py` to the `make lint` recipe
(after the existing gates) and add the pytest companion to the `make test`
pytest invocation. Run `make check-fmt`, `make lint`, `make test`, then
`make fmt` and `make markdownlint` (the last after `make fmt`). Only once all
are green, run `coderabbit review --agent` and clear every concern.

Stage E — finalise. Mark `docs/roadmap.md` 10.2.6 `[x]` with a delivery note
referencing this execplan. Update all living sections.

## Concrete steps

Run from the repository root.

Use `tee` into a per-action log, per the workspace command guidance:

```bash
ACTION=lint
tee_log=/tmp/$ACTION-rstest-bdd-$(git branch --show-current).out
make $ACTION 2>&1 | tee "$tee_log"
```

Red evidence (Stage B), expected to fail before Stage C:

```bash
python3 scripts/check_serial_nextest_matrix.py; echo "exit=$?"     # expect exit=1
$UV_ENV uv run pytest scripts/tests/test_check_serial_nextest_matrix.py \
  2>&1 | tee /tmp/red-rstest-bdd-$(git branch --show-current).out   # companion red
```

Green evidence (after Stage C):

```bash
python3 scripts/check_serial_nextest_matrix.py; echo "exit=$?"     # expect exit=0
make check-fmt && make lint && make test
make fmt && make markdownlint
```

## Validation and acceptance

Acceptance is behavioural:

- Reading `docs/users-guide.md`, a consumer finds one subsection that answers
  the three purpose questions, carries the runner matrix, shows a copy-pasteable
  test-group stanza and a `#[file_serial]` snippet, and notes the `file_locks`
  feature requirement, the serial-vs-file_serial non-exclusion, and the temp-dir
  default lock path.
- `python3 scripts/check_serial_nextest_matrix.py` exits `0` on the finished
  docs and `1` if either matrix is removed or the two diverge (demonstrated by
  the pytest companion).
- `make check-fmt`, `make lint` (including the new gate), `make test` (including
  the new companion), and `make markdownlint` all pass.

Red-Green-Refactor evidence to record here as work proceeds:

- Red: `check_serial_nextest_matrix.py` exits `1` because the user guide has no
  matrix; the companion's deletion fixture proves the gate fails on a removed
  row.
- Green: both pass after the Stage C edits add the matching matrix.
- Refactor: gate wired into `make lint`, companion into `make test`; full gate
  sweep green; CodeRabbit concerns cleared.

Quality criteria ("done"):

- Tests: the new gate and its pytest companion pass.
- Lint/typecheck: `make lint` (including the new gate) and `make check-fmt`
  pass; `make markdownlint` passes after `make fmt`.
- Docs: user guide and design §2.7.6.7 agree (matrix row-for-row, corrected
  prose); developer guide carries the canonical rationale; roadmap 10.2.6 marked
  `[x]`.

Quality method: run the gates locally via the `tee` pattern, then a single
`coderabbit review --agent` pass per milestone once the deterministic gates are
green.

## Idempotence and recovery

All edits are additive: documentation edits, one new script, one new pytest
companion, one `make lint` line, one `make test` pytest path. Steps are
re-runnable. If the gate is over-strict (false positive on valid prose), confirm
it compares only normalised data rows and reword both matrices together; do not
weaken the documentation to satisfy a brittle matcher, and do not fall back to
prose phrase-matching (escalate instead). To roll back, revert the feature-
branch commits; nothing mutates shared state, `.config/nextest.toml`, or other
crates.

## Interfaces and dependencies

- New script: `scripts/check_serial_nextest_matrix.py` (Python 3.12+, standard
  library only; if it ever shells out, use `cuprum` rather than `subprocess` per
  repository convention), with `main() -> int` returning non-zero on violations,
  modelled on `scripts/check_gpui_mapping_table.py` (heading lookup,
  `normalise_table_row`, custom error type, exit codes).
- New pytest companion:
  `scripts/tests/test_check_serial_nextest_matrix.py`, added to the `make test`
  pytest invocation alongside the existing companions.
- Makefile: append `python3 scripts/check_serial_nextest_matrix.py` to the
  `lint` recipe and the new pytest file to the `test` recipe's pytest line.
- No Rust source, test, dependency, or `.config/nextest.toml` change.

## Signposted documentation and skills

- ExecPlan authoring: the `execplans` skill (this document's envelope and living
  sections).
- Test/runner knowledge: the `nextest` skill (process-per-test model,
  test-groups, `NEXTEST_*` environment variables) and `rust-unit-testing`
  (`serial_test`, `#[serial]`/`#[file_serial]`). `rust-router` routes further
  Rust questions.
- Code navigation: the `leta` skill.
- Sibling precedent: `docs/execplans/10-2-4-gpui-version-banner-and-mapping-table.md`
  (the drift-gate and pytest-companion pattern this plan reuses) and
  `docs/execplans/adopt-v0-6-0-beta2-feedback.md` (origin of §2.7.6.7 and the
  10.2.x items).
- Authoritative external references used while drafting:
  `https://nexte.st/docs/configuration/test-groups/` and
  `https://docs.rs/serial_test/latest/serial_test/attr.file_serial.html`.
- House documents to honour: `docs/documentation-style-guide.md`, `AGENTS.md`,
  `docs/developers-guide.md`, `docs/rstest-bdd-design.md` §2.7.6.7,
  `docs/rust-testing-with-rstest-fixtures.md`, and `rust-doctest-dry-guide.md`.

## Revision note

Initial draft (2026-06-27) proposed docs correction plus an executable
demonstration suite and a prose drift gate. Revision 1 (2026-06-27, post-panel)
incorporated the Logisphere community-of-experts verdict (🔄 Revise: over-
scoped): cut the executable demonstration, the rstest-bdd scenario, the
`NEXTEST_EXECUTION_MODE` guard, and the `serial_test` dev-dependency; replaced
the prose gate with a table-parity gate reusing the 10.2.4 row comparator;
made the developer-guide note the canonical rationale; tightened the accuracy
corrections (drop the misleading "single-binary" qualifier, add the
serial-vs-file_serial non-exclusion and file-lock temp-dir notes, make the
adopter manifest fragment version-agnostic). Scope and tolerances narrowed
accordingly. Awaiting maintainer approval before implementation.
