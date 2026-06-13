# GPUI version banner and vendored-to-published mapping table (10.2.4)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
and `Outcomes & retrospective` must be kept up to date as work proceeds.

Status: ACTIVE

## Purpose / big picture

Downstream teams that adopt the stateful GPUI playbook copy its snippets into
their own crates, which depend on the **published** `gpui 0.2.2` from crates.io.
The playbook and design snippets, however, are written against the **vendored**
`gpui` fork under `vendor/gpui` (a `path` dependency in the workspace
`Cargo.toml`). The two crates share the version string `0.2.2` but expose
different test-support APIs. Without a clear "which gpui am I reading?" banner
and an accurate vendored-to-published mapping table, an adopter who pastes a
snippet hits a compile error that looks like a regression in `rstest-bdd` rather
than an expected API divergence.

Roadmap item 10.2.4 closes that gap. A which-gpui banner and a mapping table of
the API shape differences must appear in **both** `docs/users-guide.md` and
`docs/rstest-bdd-design.md`, and `make markdownlint` must pass.

The banner and table already exist in both documents — they landed in PR #519
(commit `bb95cb0`) as a side effect of the broad "v0.6.0-beta2 adopter feedback"
planning ExecPlan, before 10.2.4 had its own owner. This item therefore is not
greenfield authoring. Its real work is to **verify, correct, reconcile, and
gate** that pre-seeded content so it actually delivers on its stated intent
(no silent compile-error mismatch), and then to mark the roadmap entry done.

Independent source verification (recorded in Decision log) established that the
table is **mostly accurate but contains one material inaccuracy in row 2** and
one omitted-but-real difference. The published `gpui 0.2.2` *does* expose
`VisualTestContext::window_handle()` (through the `VisualContext` trait), so the
current table's claim that the handle must be obtained via a verbose
`vcx.update(|window, _app| window.window_handle())` closure is misleading: it
overstates the divergence and would push adopters toward needlessly awkward
code. Shipping a table that misdescribes the published API is itself the "silent
mismatch" 10.2.4 exists to prevent, so correcting row 2 is the spine of this
plan.

A reader who finishes this work will be able to:

1. See, in both documents, a banner that states the snippets target the
   vendored `gpui` and that published-`gpui 0.2.2` adopters must adapt them.
2. Read a mapping table whose every row matches the **actual** published
   `gpui 0.2.2` test-support API, verified against the published crate source.
3. Trust that the two copies of the table cannot silently drift apart, because
   a deterministic check fails CI when they do.

Success is observable as: the corrected banner and table present in both
documents with identical table bodies; `make markdownlint`, `make lint`,
`make check-fmt`, and `make test` all green; a new consistency check that fails
when the two tables diverge and passes when they agree; and roadmap item 10.2.4
marked `[x]`.

## Constraints

- This is a documentation-and-tooling item. No crate under `crates/`, no
  harness API, no macro behaviour, and no file under `vendor/` may be modified.
  If delivery appears to require a Rust source change, stop and escalate.
- The files this plan may modify are: `docs/users-guide.md`,
  `docs/rstest-bdd-design.md`, `docs/roadmap.md`, `docs/developers-guide.md`
  (the table-sync maintenance note), and — only if the consistency-gate stage
  proceeds — a new `scripts/check_gpui_mapping_table.py` plus its pytest at
  `scripts/tests/test_check_gpui_mapping_table.py` and the `Makefile` line that
  wires the script into `make lint`. No other files.
- The finish line is defined by the roadmap: the banner and table appear in
  **both** `docs/users-guide.md` and `docs/rstest-bdd-design.md`, and
  `make markdownlint` passes. The consistency gate (Stage D) is value-add that
  implements the roadmap's own dual-track note; it must not compromise the
  finish line and is subject to a go/no-go.
- The table must continue to document exactly **four** API shape differences as
  named in the roadmap. The two additional real differences discovered (see
  Surprises) are recorded as `>`-quoted prose immediately after the table,
  explicitly framed as "beyond the four shapes", not as extra table rows. Do not
  widen to five-plus rows without maintainer approval.
- The vendored ("left") column of every row must match the real vendored fork
  under `vendor/gpui`; the published ("right") column must match the real
  published `gpui 0.2.2` on crates.io. Neither column may be changed on
  guesswork — each change cites the verifying source.
- Prose holds to en-GB Oxford spelling and `docs/documentation-style-guide.md`.
  Paragraphs and bullets wrap at 80 columns; fenced code blocks wrap at 120;
  tables and headings are not wrapped.
- `docs/users-guide.md` is vendored into consumer projects and uses absolute
  GitHub cross-reference links validated by `scripts/check_users_guide_links.py`
  under `make lint`; any new cross-reference in that file must use the canonical
  `BASE_URL` form, or `make lint` will fail.
- `make markdownlint`, `make lint`, `make check-fmt`, and `make test` must all
  pass before any `coderabbit review --agent` is requested.

## Tolerances (exception triggers)

- Scope: if delivery requires touching more than the files named in
  Constraints, or more than about 150 net lines across them, stop and escalate.
- Interface: if delivery seems to require changing any Rust public API, macro
  attribute, harness trait, feature flag, or any file under `vendor/`, stop and
  escalate.
- Dependencies: if the consistency gate seems to need a new Python or Rust
  dependency beyond the standard library and the repo's existing dev tooling
  (`uv`, `ruff`, `pytest`, `cuprum`), stop and escalate.
- Table framing: if accurate documentation appears to require more than four
  table rows (i.e. the maintainer must decide whether to widen "four API shape
  differences"), stop and present the options before editing the row count.
- Iterations: if `make markdownlint` or `make lint` still fails after three
  focused attempts, stop and escalate with the rule code and offending line.
- Ambiguity: if the published-API verification cannot be reproduced against an
  authoritative source (the published crate tarball or the Zed `v0.2.2` tag),
  stop and escalate rather than ship an unverifiable row.
- Time: if drafting plus gates exceeds three hours, stop and escalate.

## Risks

- Risk: correcting row 2 changes the recommended idiom and could contradict the
  surrounding prose, which currently describes reconstructing
  `VisualTestContext` and reading a window handle. Severity: medium. Likelihood:
  medium. Mitigation: read the full §2.7.6.2 (design) and the "Stateful GPUI
  scenarios" section (users' guide) around each table; adjust any prose that
  asserts the verbose `.update(...)` form is required, and keep the vendored
  snippet unchanged (it is correct against the vendored fork).
- Risk: the two tables drift again after this fix, reintroducing the silent
  mismatch. Severity: medium. Likelihood: medium (the roadmap explicitly flags
  this as a recurring maintenance tax for every future gpui bump). Mitigation:
  Stage D adds a deterministic consistency check, following the established
  `check_users_guide_links.py` precedent, so drift fails `make lint`.
- Risk: the published column cannot be compile-checked inside this workspace,
  because the workspace pins `gpui` to the vendored path. Severity: low.
  Likelihood: high (structural). Mitigation: the vendored column is already
  locked to reality by the regression suite
  `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`; the published
  column is verified out-of-band against the published crate source and the
  verification is recorded in the Decision log. Optionally cross-reference the
  existing publish-time validator (`scripts/publish_check_gpui*.py`), which does
  build against the real published `gpui`, as the durable compile-time anchor.
- Risk: a markdownlint table-formatting rule (escaped pipes inside table cells,
  line length) rejects the edited row 2 or the new `>`-quoted prose. Severity:
  low. Likelihood: medium. Mitigation: keep the existing escaped-pipe (`\|`)
  cell convention, run `make markdownlint` immediately after the table edits
  (Stage C) and again after the roadmap edit, and do not let a table row wrap.
- Risk: the Stage D parser is brittle — escaped pipes, `make fmt` whitespace
  reflow (known non-idempotent for markdown in this repo), or a second
  `| Operation |` table could cause false CI failures on unrelated doc PRs,
  turning a drift guard into a nuisance. Severity: medium. Likelihood: medium.
  Mitigation: anchor the table by its preceding heading, compare normalised
  whole rows (never split cells on `\|`), and add a whitespace-only-mutation
  pytest case that must still pass. If the gate still proves flaky in review,
  defer it (go/no-go) rather than ship a nuisance.
- Risk: the gate gives false assurance. It only proves the two copies agree with
  each other, not that either agrees with the real published `gpui`. A future
  `gpui 0.2.3+` that changes the API mid-line would leave the table stale while
  the gate stays green. Severity: medium. Likelihood: medium. Mitigation: state
  this limitation in the script docstring and the developers-guide note; point
  at `scripts/publish_check_gpui*.py` as the release-time reality check; record
  the verification recipe so the next gpui bump re-checks the published column.
- Risk: scope creep into roadmap items 10.2.5–10.2.7 (lint-clean variant,
  nextest/serial note, bulk-migration cookbook), which share §2.7.6.2 and the
  same users'-guide region. Severity: low. Likelihood: medium. Mitigation: edit
  only the banner, the table, and prose strictly bound to the table's accuracy;
  leave the neighbouring subsections untouched.

## Progress

- [x] Capture user approval for this plan (gate before implementation begins).
  User asked to proceed with implementation on 2026-06-13.
- [x] Stage A — verification and reconciliation audit (no edits). Re-read both
  tables and the surrounding prose; confirm the row-2 inaccuracy and the
  header/parenthetical divergences against the recorded research; run the
  vendored-arity and prose-audit greps; quantify the prose lines Stage C touches.
- [x] Stage B — red checks. Run the consistency check (Stage D artifact) or the
  fallback grep so it fails against the current diverging/inaccurate tables,
  proving the gap before the fix.
- [x] Stage C — correct row 2 in both documents, add the two `>`-quoted
  beyond-the-four prose differences, reconcile headers/row-3/caption so the data
  rows match, adjust any prose asserting the verbose `.update(...)` form, and run
  `make markdownlint` as a mid-stage check.
- [x] Stage D — (go/no-go) add `scripts/check_gpui_mapping_table.py` and its
  pytest (incl. the whitespace-only pass case), wire it into `make lint`, add the
  `docs/developers-guide.md` table-sync note, and confirm the gate passes.
- [ ] Stage E — run `make markdownlint`, `make check-fmt`, `make lint`,
  `make test` (teed to `/tmp`); then `coderabbit review --agent`; clear all
  concerns.
- [ ] Stage F — mark roadmap item 10.2.4 `[x]` with a dated one-line summary
  referencing this ExecPlan; re-run `make markdownlint`.

## Surprises & discoveries

- Observation: the banner and mapping table for 10.2.4 already exist in both
  target documents. Evidence: `git show bb95cb0 -- docs/users-guide.md
  docs/rstest-bdd-design.md` shows the "Stage D" commit of the broad
  v0.6.0-beta2 feedback ExecPlan added them; the table is at
  `docs/rstest-bdd-design.md:1978-1985` and `docs/users-guide.md:1114-1119`.
  Impact: 10.2.4 is a verify/correct/reconcile/gate item, not greenfield
  authoring.
- Observation: row 2 of the table is inaccurate. Evidence: the published
  `gpui 0.2.2` source (crate tarball, commit `69e2130`) defines
  `impl VisualContext for VisualTestContext { fn window_handle(&self) ->
  AnyWindowHandle { self.window } }` at `src/app/test_context.rs:985-989`, and
  `Window::window_handle()` at `src/window.rs:1362`. So
  `VisualTestContext::window_handle()` *does* exist in published gpui; the only
  real difference from the vendored inherent method is that the published one is
  a `VisualContext` trait method and needs `use gpui::VisualContext` in scope.
  Impact: row 2 must be corrected in both documents.
- Observation: there are two real differences beyond the four table rows.
  Evidence: (1) published `add_window_view` returns
  `(Entity<V>, &mut VisualTestContext)` (`src/app/test_context.rs:256-263`)
  whereas the vendored fork returns `(Entity<T>, VisualTestContext)` by value
  (`vendor/gpui/src/lib.rs:145-148`); (2) the vendored `update_entity` returns
  `Result<(), EntityError>` and `read_entity` returns `Option<R>` — a typed
  missing-entity path the regression suite asserts on
  (`vendor/gpui/src/test_window.rs:207-237`) — while published `gpui` returns
  `R` directly with no such error channel. Impact: record both as `>`-quoted
  prose immediately after the table, framed as "beyond the four shapes", not as
  extra rows, to honour the roadmap's "four API shape differences" framing.
- Observation: rows 1, 3, and 4 are accurate. Evidence: published
  `add_window_view` closure is `FnOnce(&mut Window, &mut Context<V>) -> V`
  (two-arg); `from_window` returns `Self` by value
  (`src/app/test_context.rs:688`); `TestAppContext`/`VisualTestContext` define
  `type Result<T> = T`, so `read_entity`/`update_entity` return `R` directly.
  The vendored fork's one-arg closure, `Option<VisualTestContext>` from
  `from_window`, and `Option`/`Result`-wrapped entity reads are all genuine
  divergences. Impact: leave rows 1, 3, 4 as they are.
- Observation: the initial Stage D checker matched exact heading text and failed
  on the design document before reaching the intended row-drift comparison,
  because the real heading is numbered as "2.7.6.2 Interim GPUI state pattern".
  Evidence: `python3 scripts/check_gpui_mapping_table.py` first reported
  `heading not found: Interim GPUI state pattern`; after accepting numbered
  heading prefixes, it reported the intended row-3 divergence. Impact: the
  checker now accepts heading prefixes while still anchoring on the configured
  text.
- Observation: the ExecPlan file contained a duplicate copy of itself before
  implementation edits. Evidence: `make markdownlint` reported duplicate
  headings starting at line 817. Impact: the duplicate copy was removed as plan
  maintenance before continuing; no implementation files were affected.

## Decision log

- Decision: treat 10.2.4 as a verify/correct/reconcile/gate item rather than
  re-authoring the banner and table from scratch. Rationale: the content already
  exists in both documents (Surprises), so re-authoring would be churn; the
  unmet part of the intent is accuracy, cross-document consistency, and a
  drift-prevention gate. Date/Author: 2026-06-13, planning agent.
- Decision: correct row 2 to state that published `gpui 0.2.2` exposes
  `vcx.window_handle()` via the `VisualContext` trait (requires
  `use gpui::VisualContext`), returning `AnyWindowHandle` by value, and stop
  asserting the verbose `.update(...)` form is required. Rationale: independent
  verification against the published crate source (commit `69e2130`,
  `src/app/test_context.rs:985-989`; `src/window.rs:1362`) refutes the current
  claim; an inaccurate published column is the exact failure mode 10.2.4 must
  prevent. Date/Author: 2026-06-13, planning agent (research recorded below).
- Decision: keep the table at four rows and record the two beyond-the-four
  differences (the `add_window_view` return type, and the vendored
  `update_entity`/`read_entity` typed error channel) as `>`-quoted prose right
  after the table, not as extra rows and not folded into row 1's cells.
  Rationale: the roadmap commits to "four API shape differences"; adding rows
  changes the contract, and the panel (Pandalump 🐼, Telefono ☎️) found that
  cramming the return-type detail into row 1 overloaded the cell, made the row
  asymmetric, risked a markdownlint width failure, and blurred two separable
  concerns. Prose after the table documents both honestly while leaving each row
  single-concern. Escalate only if the maintainer prefers widening the table.
  Date/Author: 2026-06-13, planning agent (revised after community-of-experts
  panel).
- Decision: implement drift prevention as a Python consistency script wired into
  `make lint`, backed by a pytest, mirroring `scripts/check_users_guide_links.py`
  and its test `scripts/tests/test_check_users_guide_links.py`. Rationale: this
  is the repository's established idiom for doc-invariant linting; the roadmap's
  dual-track note explicitly asks to "make staleness a CI failure rather than a
  silent drift". Date/Author: 2026-06-13, planning agent.
- Decision: make the Stage D check robust by design — anchor the table by its
  preceding heading, compare normalised whole data rows (never split on `\|`),
  and prove with a whitespace-only-mutation pytest case that `make fmt` reflow
  cannot make it fire. Rationale: the panel pre-mortem (Doggylump 🐶) showed a
  naive byte-compare parser would false-positive on escaped pipes and the repo's
  known non-idempotent markdown formatting, turning the guard into a CI nuisance
  on unrelated doc PRs. Date/Author: 2026-06-13, planning agent (post-panel).
- Decision: state the gate's scope honestly — it catches doc-vs-doc drift, not
  drift from the real published `gpui`, which is checked at release time by
  `scripts/publish_check_gpui*.py`. Capture the published-column verification
  recipe in `docs/developers-guide.md` for the next gpui bump. Rationale: the
  panel (Doggylump 🐶, Wafflecat 🐈🧇, Dinolump 🦕) warned the gate could give
  false assurance and that the hand verification is labour-intensive to repeat.
  Date/Author: 2026-06-13, planning agent (post-panel).
- Decision: keep Stage D a genuine go/no-go and rely on Stages A–C plus
  `make markdownlint` for the finish line. Rationale: the roadmap finish line is
  narrow ("banner+table in both docs; markdownlint passes"); siblings 10.2.1–
  10.2.3 shipped doc-only with no new gate. The panel (Wafflecat 🐈🧇, Dinolump
  🦕) judged the gate justified by the recurring dual-track tax but optional
  against the finish line; the single-source-of-truth alternative was rejected
  (no markdown-include tooling in the repo). Date/Author: 2026-06-13, planning
  agent (post-panel).
- Decision: do not attempt to compile-check the published column inside the
  workspace. Rationale: the workspace pins `gpui` to `vendor/gpui` via a `path`
  dependency, so the published API cannot be exercised here. The vendored column
  is already covered by `stateful_window.rs`; the published column is verified
  out-of-band and may be cross-referenced to `scripts/publish_check_gpui*.py`,
  which builds against the real published crate at release time. Date/Author:
  2026-06-13, planning agent.
- Decision (testing rigour): apply pytest unit tests to the new consistency
  script and rely on the existing GPUI regression suite for the vendored column;
  do not add rstest unit tests, rust-rspec/`rstest-bdd` behavioural tests,
  `insta` snapshots, `proptest`, Kani, or Verus. Rationale: the change is
  documentation plus a pure file-parsing lint script. There is no Rust behaviour
  to unit-test, no externally observable workflow to drive with BDD, no
  multivariant runtime output (the cross-document consistency check is the
  apt "output consistency" guard, superseding an `insta` snapshot of static
  prose), and no input-domain invariant warranting property/model-checking/
  proof. The script's own logic is the only new executable surface and pytest is
  the repo-idiomatic adversary for it. Date/Author: 2026-06-13, planning agent.
- Decision: proceed with Stage D rather than defer the drift gate. Rationale:
  Stages A-C stayed within the named file scope and line-count tolerances, the
  script uses only the Python standard library, and its focused pytest proves
  the parser handles numbered headings and whitespace-only table alignment
  changes. Date/Author: 2026-06-13, implementing agent.

### Research provenance (published gpui 0.2.2)

Verified by downloading the published crate tarball
`https://static.crates.io/crates/gpui/gpui-0.2.2.crate` and reading
`crates/gpui/src/app/test_context.rs` (embedded VCS commit
`69e2130295c2649963eb639fc70b4f2ee8ea1624`, Zed tag `v0.2.2`). 0.2.2 is the
latest published gpui. Findings:

- `add_window_view`: `F: FnOnce(&mut Window, &mut Context<V>) -> V`, returns
  `(Entity<V>, &mut VisualTestContext)` (`test_context.rs:256-263`).
- `VisualTestContext::window_handle()`: exists via
  `impl VisualContext for VisualTestContext`, returns `AnyWindowHandle`
  (`test_context.rs:985-989`); `Window::window_handle()` at `window.rs:1362`.
- `VisualTestContext::from_window(window, cx) -> Self` by value
  (`test_context.rs:688`).
- `type Result<T> = T` on `TestAppContext` (`test_context.rs:33-34`) and
  inherited by `VisualTestContext` (`:902`); `read_entity`/`update_entity`
  return `R` directly.

Vendored fork (path dependency, version `0.2.2`) confirmed divergent:

- `add_window_view(impl FnOnce(&mut VisualTestContext) -> T) ->
  (Entity<T>, VisualTestContext)` (`vendor/gpui/src/lib.rs:145-148`).
- inherent `VisualTestContext::window_handle() -> AnyWindowHandle`
  (`vendor/gpui/src/test_window.rs:200-204`).
- `from_window(...) -> Option<Self>` (`vendor/gpui/src/test_window.rs:191-198`).
- `read_entity -> Option<R>`, `update_entity -> Result<(), EntityError>`; no
  identity `Result` alias (`vendor/gpui/src/test_window.rs:207-237`).

All four operations are exercised by
`crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` (gated on the
`native-gpui-tests` feature), which locks the vendored column to reality.

## Outcomes & retrospective

To be completed at delivery. Compare against the three reader outcomes and the
success criteria in Purpose: corrected banner/table in both documents, identical
table bodies, all gates green, the consistency check failing on drift and
passing on agreement, and roadmap 10.2.4 marked done.

## Context and orientation

`rstest-bdd` is a Rust behaviour-driven-development (BDD) framework that drives
`rstest`-style fixtures from [Gherkin][gherkin] feature files. It ships a
feature-gated GPUI harness crate, `rstest-bdd-harness-gpui`, for testing
[GPUI](https://www.gpui.rs/) user interfaces.

Key terms (no prior knowledge assumed):

- **Vendored gpui**: the in-repo fork at `vendor/gpui`, pulled in as a `path`
  dependency by the workspace `Cargo.toml` (`gpui = { version = "0.2.2", path =
  "vendor/gpui", ... }`). The regression suite and all design/playbook snippets
  compile against this fork.
- **Published gpui 0.2.2**: the crate of the same name and version on
  crates.io, which downstream adopters depend on. It exposes a different
  test-support API despite the shared version string.
- **VisualTestContext**: a gpui test-support type that drives a test window;
  borrows the app context, so it must be reconstructed per step rather than
  stored across steps.
- **Mapping table**: the four-row table contrasting the vendored and published
  test APIs for the operations the playbook uses.

Stable anchors for a new reader:

- The roadmap entry under change is `docs/roadmap.md:784-790` (item 10.2.4),
  under "10.2. Update adoption documentation before v0.6.0 final". The
  dual-track maintenance note is at `docs/roadmap.md:810-819`.
- The design table and banner are at `docs/rstest-bdd-design.md:1969-1989`
  (§2.7.6.2 "Interim GPUI state pattern"); the vendored snippet follows at
  `:1994-2032`.
- The users'-guide banner and table are at `docs/users-guide.md:1106-1122`
  (under "Stateful GPUI scenarios with durable handles"); the
  v0.6-interim-workaround note precedes them at `:1090-1102`.
- The vendored regression suite is
  `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` (feature
  `native-gpui-tests`).
- Doc-lint precedent: `scripts/check_users_guide_links.py`, invoked by
  `make lint` (`Makefile:45`), with its pytest at
  `scripts/tests/test_check_users_guide_links.py` (run by `make test`,
  `Makefile:36`).
- Publish-time validator that builds against the real published gpui:
  `scripts/publish_check_gpui.py`, `scripts/publish_check_gpui_manifest.py`,
  run via `scripts/run_publish_check.py` (`Makefile:70`).

### The corrected, shared table

Both documents carry an identical table — same header, separator, four data
rows, and caption (cells keep the existing `\|` escaped-pipe convention;
vendored = left, published = right). Only row 2 changes from what is on disk
today; rows 1, 3, and 4 are already accurate and are reproduced here so the
implementer has the whole target in one place (rendered as a real table so it
mirrors the on-disk form; in both documents it lives inside the `>`-quoted
banner blockquote):

| Operation | Vendored gpui (regression suite + these snippets) | Published `gpui 0.2.2` (downstream adopters) |
| --- | --- | --- |
| `add_window_view` closure | `\|_context\| View::default()` (one argument) | `\|_window, view_cx\| View::new(view_cx)` (two arguments) |
| obtain window handle | `visual_cx.window_handle()` (inherent method on `VisualTestContext`) | `vcx.window_handle()` (same call, but `window_handle` is a `VisualContext` trait method, so add `use gpui::VisualContext;`) |
| `VisualTestContext::from_window` | returns `Option<VisualTestContext>` (`.unwrap_or_else`/`.ok_or`) | returns `VisualTestContext` by value (no `Option`) |
| `read_entity` / `update_entity` | `Option`/`Result` wrappers (`Some(1)`, `Ok(())`) | identity `type Result<T> = T`; returns `R` directly |

The single substantive correction is row 2: the published column previously
claimed the handle had to be obtained through a verbose
`vcx.update(\|window, _app\| window.window_handle())` closure "via
`Window::window_handle()`", implying `VisualTestContext` had no `window_handle`
method. It does — through the `VisualContext` trait — so the correct published
idiom is the same `vcx.window_handle()` call as the vendored fork, with
`use gpui::VisualContext;` added. The verbose form is no longer presented as
required.

Two further real differences are recorded in prose immediately after the table
(as `>`-quoted lines continuing the banner blockquote), rather than as extra
rows, to honour the roadmap's "four API shape differences" framing and to keep
each row about a single concern:

1. `add_window_view` return type. Published `gpui 0.2.2` returns
   `(Entity<V>, &mut VisualTestContext)` — the visual context comes back as a
   mutable borrow — whereas the vendored fork returns
   `(Entity<T>, VisualTestContext)` by value. Adopters bind the visual context
   by mutable reference rather than owning it.
2. `update_entity` error channel. The vendored fork's `update_entity` returns
   `Result<(), EntityError>` and `read_entity` returns `Option<R>`, giving a
   typed missing-entity path the regression suite asserts on. Published `gpui`
   has no such wrapper (it returns `R` directly), so that typed error path does
   not exist for adopters and code must not depend on it.

Stage C unifies the header wording, the row-3 parenthetical, and the caption
across both documents so the rendered tables are identical. The Stage D check
(if it proceeds) compares the four **normalised data rows** — it does not split
cells on `\|`, so escaped pipes and `make fmt` whitespace reflow do not trip it;
headers and caption are unified by Stage C but the data rows are the
load-bearing content the gate enforces.

## Plan of work

### Stage A — verification and reconciliation audit (no edits)

Re-read `docs/rstest-bdd-design.md:1969-1989` and
`docs/users-guide.md:1090-1122` in the worktree. Confirm against the recorded
research that:

1. Row 2 is inaccurate in both documents (published `VisualTestContext` has
   `window_handle()`).
2. The two tables diverge in header wording, the row-3 parenthetical, and the
   presence of a caption.
3. Rows 1 (closure arity), 3, and 4 are otherwise accurate.

Diff the two current table bodies to capture every byte-level divergence:

```bash
sed -n '1978,1985p' docs/rstest-bdd-design.md
sed -n '1114,1119p' docs/users-guide.md
```

Confirm the vendored column against the live code so the left column cannot be
silently wrong:

```bash
# Vendored closure arity (expect one-arg `|_context|` call sites).
grep -n 'add_window_view(|_context|' \
    crates/rstest-bdd-harness-gpui/tests/stateful_window.rs
```

Pre-audit the surrounding prose so the Stage C edit scope is known before any
edit. List every place that mentions the window-handle access path or the
verbose `.update(...)` form near each table, and confirm `windows()` (used by
the regression suite as an accessor, outside the four documented operations) is
not silently presented as a fifth divergence:

```bash
grep -n 'window_handle\|\.update(\|windows()' \
    docs/rstest-bdd-design.md docs/users-guide.md
```

Record the divergences and the prose hits in `Surprises & discoveries` if any
differ from those already listed, and quantify how many prose lines Stage C must
touch. No edits in this stage.

### Stage B — red checks (prove the gap before fixing)

Establish a failing check that the current state does not satisfy. The primary
red is the Stage D consistency check; the grep is a fallback only if Stage D is
deferred.

1. Primary: draft `scripts/check_gpui_mapping_table.py` (Stage D) first and run
   it. It must FAIL now, reporting that the two tables' data rows differ (today
   the headers and the row-3 parenthetical diverge between the documents).
2. Fallback (only if Stage D is deferred): a documentation-shaped red assertion
   that targets the verbose closure form, accounting for the escaped pipes in
   the live cell:

   ```bash
   # The inaccurate row 2 routes through .update(|window, _app| ...).
   grep -F 'update(\|window, _app\| window.window_handle())' \
       docs/rstest-bdd-design.md docs/users-guide.md \
       && echo "RED: inaccurate row 2 present" \
       || echo "row 2 already corrected"
   ```

   Expected before the fix: `RED: inaccurate row 2 present` for both files. If
   this prints `row 2 already corrected` while the on-disk cell still shows the
   verbose form, the escaping has drifted — inspect the raw cell with
   `sed -n` before trusting the result.

Record the red output in `Artifacts and notes`.

### Stage C — correct and reconcile both tables

Replace the table body in both documents with the corrected, shared body from
"The corrected, shared table body" above. Specifically:

In `docs/rstest-bdd-design.md` (§2.7.6.2, the table at `:1978-1985`):

1. Replace row 2's published cell with the trait-method form:
   `vcx.window_handle()` (same call, but `window_handle` is a `VisualContext`
   trait method, so add `use gpui::VisualContext;`). Leave row 2's vendored cell
   as the inherent-method wording.
2. Leave rows 1, 3, and 4, the banner prose, and the caption intact.
3. Add the two prose differences (return type; `update_entity` error channel)
   as `>`-quoted lines continuing the banner blockquote, immediately after the
   caption, exactly as drafted under "The corrected, shared table".
4. Adjust any surrounding prose that implies the verbose `.update(...)` form is
   required for the published crate. The Stage A pre-audit grep over
   `window_handle`/`.update(` lists the exact lines; none may remain asserting
   that the published handle must come through `.update(...)`. The vendored
   snippet (which legitimately uses `visual_cx.window_handle()`) stays as is.

In `docs/users-guide.md` (the table at `:1114-1119`):

1. Apply the identical row 2 edit.
2. Reconcile the headers to the design-doc wording, add the row-3
   `(`.unwrap_or_else`/`.ok_or`)` parenthetical, and add the same caption line,
   so the four data rows (and the header and caption) are identical to the
   design doc. Add the same two `>`-quoted prose differences after the caption.
3. The banner lead-in "The four shapes that differ are:" stays as four rows; the
   two prose differences are explicitly framed as "beyond the four shapes" so
   the count is not contradicted.

Keep the vendored snippet at `docs/rstest-bdd-design.md:1994-2032` and the
users'-guide code blocks unchanged — they are correct against the vendored fork.

Run `make markdownlint` immediately after the table edits (before Stage D) to
confirm the new `>`-quoted prose and the edited row 2 do not trip a table or
line-length rule, then capture the row 2 cells with `sed -n` into
`Artifacts and notes` as proof the escaped pipes render correctly.

### Stage D — consistency gate (go/no-go)

Go/no-go: proceed only if Stages A–C stay within tolerances and the maintainer
has not asked to defer tooling. If skipped, record the decision and rely on the
narrow finish line plus the corrected tables.

Add `scripts/check_gpui_mapping_table.py`. The brittleness of markdown-table
parsing is the main design risk (the panel pre-mortem flagged escaped pipes,
`make fmt` whitespace reflow — which is known to be non-idempotent in this repo
— and a possible second `| Operation |` table). The spec therefore is:

- **Anchor the table, do not grab the first match.** Locate the table by the
  heading that precedes it: "Interim GPUI state pattern" in
  `docs/rstest-bdd-design.md` and "Stateful GPUI scenarios with durable handles"
  (or the which-gpui banner lead-in) in `docs/users-guide.md`. Within that
  section, take the first `| Operation |`-prefixed table.
- **Compare normalised whole rows, never split cells on `\|`.** Extract the four
  data rows (the lines after the `| --- |` separator, up to the first
  non-table line). Normalise each row by collapsing internal whitespace runs to
  a single space and trimming ends. Compare the two ordered lists of normalised
  rows. Not splitting on `\|` makes escaped pipes and `make fmt` alignment
  reflow harmless, because both documents reflow identically.
- **Exit non-zero** with a diff-style message naming the first offending row
  (its index and both normalised forms) when the lists differ; exit zero when
  they agree. Pure standard-library file parsing; no subprocess. (If a shell-out
  is ever needed, use `cuprum`, not `subprocess`, per repo convention.)
- **Document the gate's scope in the module docstring.** It catches
  *doc-vs-doc* drift (the two copies disagreeing). It does **not** verify either
  copy against the real published `gpui` — that cannot be done in-workspace
  because `gpui` is pinned to `vendor/gpui`. The published column's compile-time
  anchor is the release-time validator `scripts/publish_check_gpui*.py`; name it
  in the docstring so a future maintainer knows where reality is checked.

Add `scripts/tests/test_check_gpui_mapping_table.py` mirroring
`scripts/tests/test_check_users_guide_links.py`: cover (a) the pass case
(identical tables), (b) a content-mutation fail case (a data row changed in a
tmp copy), (c) a whitespace-only mutation that must STILL PASS (extra alignment
spaces in one copy — proves normalisation works and the gate will not fire after
`make fmt`), and (d) the table-not-found case (anchor heading missing).

Wire the script into `make lint` beside `check_users_guide_links.py`
(`Makefile:44-45`), and ensure the pytest is collected by `make test` (it lives
under `scripts/tests/`, already globbed at `Makefile:36`).

Also add a short note to `docs/developers-guide.md` (the doc-maintenance area):
the GPUI mapping table is duplicated in `docs/users-guide.md` and
`docs/rstest-bdd-design.md`; any edit must update both copies; `make lint` runs
`check_gpui_mapping_table.py` to enforce this; and the published column is
verified by extracting the published `gpui` crate tarball (record the
`git show <commit>:crates/gpui/src/app/test_context.rs` / tarball-extraction
recipe) when gpui is next bumped. This converts the one-off research recipe into
durable guidance and primes future maintainers and the neighbouring 10.2.5–
10.2.7 items to keep the two copies in step.

### Stage E — validate

Run the substantive gate first, then the sanity gates, teeing logs:

```bash
BRANCH=$(git branch --show-current)
make markdownlint 2>&1 | tee /tmp/markdownlint-rstest-bdd-${BRANCH}.out
make check-fmt    2>&1 | tee /tmp/check-fmt-rstest-bdd-${BRANCH}.out
make lint         2>&1 | tee /tmp/lint-rstest-bdd-${BRANCH}.out
make test         2>&1 | tee /tmp/test-rstest-bdd-${BRANCH}.out
```

Each must exit zero before the next. Then run the green-stage consistency
check (or grep), confirm the row-2 correction is present in both files, and run
`coderabbit review --agent`; clear all concerns before promoting the PR out of
draft. CodeRabbit must not be used to catch what these gates catch
deterministically, so do not request it until all four gates are green.

### Stage F — roadmap tick

Update `docs/roadmap.md:784-790`: change `- [ ]` to `- [x]` and append a dated
delivery note after the existing "Design Doc:" line, matching the 10.2.1–10.2.3
entries (which keep the original finish-line text and add a `Delivered <date>:`
block ending in `See \`docs/execplans/…\`.`). Wrap the note at 80 columns. Use
this shape, adjusted to the actual delivery date:

```plaintext
  Delivered 2026-06-13: corrected the published-column window-handle row in the
  vendored-to-published `gpui 0.2.2` mapping table in both
  `docs/users-guide.md` and `docs/rstest-bdd-design.md` (published
  `VisualTestContext` exposes `window_handle()` via the `VisualContext` trait),
  reconciled the two tables, and added a drift gate. See
  `docs/execplans/10-2-4-gpui-version-banner-and-mapping-table.md`.
```

If Stage D was deferred, drop "and added a drift gate" from the note. Re-run
`make markdownlint` to catch any wrap-width regression from the roadmap edit.
The roadmap tick is the final evidence the work shipped.

## Concrete steps

Working directory: the repository worktree root. All commands assume it is the
current directory.

1. Confirm clean state and branch.

   ```bash
   git status --short
   git branch --show-current   # expect 10-2-4-gpui-version-banner-and-mapping-table
   ```

2. Run the Stage A audit reads and the Stage B red check; record outputs in
   `Artifacts and notes`.

3. Apply the Stage C edits to `docs/rstest-bdd-design.md` and
   `docs/users-guide.md`. Re-run the Stage B check; it must now report the row 2
   correction is present (and, if Stage D is in place, that the tables match).

4. (Stage D, if go) add the script, its pytest, and the `Makefile` wiring; run
   the script directly and confirm it exits zero.

5. Run the Stage E gates in order (`make markdownlint`, `make check-fmt`,
   `make lint`, `make test`), stopping on the first failure and fixing the root
   cause. Commit the documentation correction and the gate as small, focused
   commits with imperative subjects naming the roadmap item, e.g.
   "Correct published gpui window-handle row in mapping table (10.2.4)" and
   "Gate vendored-to-published gpui mapping-table drift (10.2.4)".

6. Run `coderabbit review --agent`; clear all concerns; re-run the local gates
   from `make check-fmt` onwards if prose changes result.

7. (Stage F) mark roadmap 10.2.4 `[x]`; re-run `make markdownlint`; commit.

8. Push and ensure the draft PR (for this ExecPlan) reflects the delivery.

## Validation and acceptance

Acceptance is observable as:

1. Both `docs/rstest-bdd-design.md` and `docs/users-guide.md` carry the
   which-gpui banner and a four-row mapping table whose four data rows are
   identical (after whitespace normalisation) and whose row 2 states that
   published `gpui 0.2.2` exposes `vcx.window_handle()` via the `VisualContext`
   trait (requiring `use gpui::VisualContext;`); and both carry the same two
   `>`-quoted "beyond the four shapes" prose differences after the table.
2. The Stage B red check fails before the fix and the Stage E green check passes
   after it.
3. If Stage D proceeds: `scripts/check_gpui_mapping_table.py` exits zero on the
   corrected tables and non-zero when either table's body is mutated; its pytest
   passes under `make test`.
4. `make markdownlint`, `make check-fmt`, `make lint`, and `make test` all exit
   zero.
5. `coderabbit review --agent` reports no remaining concerns.
6. `docs/roadmap.md` marks item 10.2.4 `[x]` with a dated one-line summary
   referencing this ExecPlan.

Quality criteria:

- Tests: existing Rust and Python suites continue to pass via `make test`; if
  Stage D proceeds, the new pytest passes and exercises pass/fail/not-found.
- Lint/typecheck: `make lint` (including the Python doc-lint scripts) and
  `make check-fmt` pass; the documents satisfy `make markdownlint`.
- Performance: not applicable.
- Security: not applicable; documentation plus a read-only lint script.

Quality method:

- Local: the sequential gate above, logs teed to `/tmp`.
- CI: GitHub Actions mirrors these gates; the PR stays draft until CI is green.
- Human/CodeRabbit: `coderabbit review --agent` on the worktree before
  promotion.

Red-Green-Refactor note: the production change here is documentation plus a
pure-parsing lint script, so the code RGR cycle applies directly to the Stage D
script (write the failing pytest fail-case first, then the script, then refactor)
and a documentation-shaped substitute (Stage B red grep / failing consistency
check) covers the table correction itself, per the execplans skill's "nearest
observable substitute" allowance.

## Idempotence and recovery

Each step is re-runnable without damage:

- Stage A/B reads and greps are read-only.
- The table edits are contiguous replacements; reverting the commit restores the
  prior table bodies exactly.
- The Stage D script and pytest are additive; removing them and the one
  `Makefile` line fully reverts the gate.
- The roadmap edit is one checkbox plus trailing prose; reverting that commit
  restores the `- [ ]` state.
- `make` targets are idempotent in this repository.

If `make markdownlint` fails after an edit, fix the line-wrap or escaped-pipe
issue and re-run; do not undo earlier edits unless the rule code points at them.

## Artifacts and notes

- Stage A audit: the vendored regression suite contains one-argument
  `add_window_view(|_context| ...)` call sites at lines 85, 110, 170, 174, 205,
  210, 226, and 228; prose hits for the inaccurate published handle path were
  limited to the two mapping-table row-2 cells.
- Stage B red output: after fixing numbered-heading anchoring in the checker,
  `python3 scripts/check_gpui_mapping_table.py` failed with
  `GPUI mapping table data rows differ` and identified row 3, where the users'
  guide lacked the design table's `.unwrap_or_else`/`.ok_or` parenthetical.
- Stage C green output: after table reconciliation,
  `python3 scripts/check_gpui_mapping_table.py` exited zero, and
  `make markdownlint` exited zero after removing the duplicate ExecPlan copy.
- Stage D green output: `uv run --with pytest python -m pytest
  scripts/tests/test_check_gpui_mapping_table.py -q` reported `9 passed`, and
  `make lint` later ran `python3 scripts/check_gpui_mapping_table.py` as part
  of the full lint target.
- Stage E gate summaries so far: `make markdownlint` reported
  `Summary: 0 error(s)`; `make check-fmt` reported `34 files already
  formatted`; `make lint` reported `All checks passed!` from Ruff and completed
  the file-length, users-guide-link, and GPUI-table checks; `make test` reported
  `1487 tests run: 1487 passed, 7 skipped` from nextest and `95 passed` from
  pytest.
- CodeRabbit review (to be captured): the terminal
  `{"type":"complete","status":"review_completed","findings":0}` line.

## Interfaces and dependencies

This plan introduces no Rust interface. If Stage D proceeds it adds one Python
module and its test, and one `make lint` invocation line:

- `scripts/check_gpui_mapping_table.py` — a standard-library-only validator
  exposing a `main()` that returns an exit code; reads `docs/users-guide.md` and
  `docs/rstest-bdd-design.md`, extracts the `| Operation | … |` table from each,
  and asserts the four data rows match. No third-party imports; if a shell-out
  is ever required, use `cuprum` rather than `subprocess`.
- `scripts/tests/test_check_gpui_mapping_table.py` — pytest covering pass, fail,
  and table-not-found cases, following
  `scripts/tests/test_check_users_guide_links.py`.
- `Makefile` — one new `python3 scripts/check_gpui_mapping_table.py` line in the
  `lint` target beside the existing doc-lint scripts (`Makefile:44-45`).
- `docs/developers-guide.md` — a short maintenance note: the GPUI mapping table
  is duplicated in `docs/users-guide.md` and `docs/rstest-bdd-design.md`, both
  copies must be updated together, `make lint` enforces this via
  `check_gpui_mapping_table.py`, and the published column is re-verified by
  extracting the published `gpui` crate tarball at the next gpui bump (with the
  `git show <commit>:crates/gpui/src/app/test_context.rs` recipe recorded).

It depends on, and must stay consistent with, these existing surfaces (not
modified): the vendored fork under `vendor/gpui`; the regression suite
`crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`; the publish-time
validator `scripts/publish_check_gpui*.py`; and the Make gate targets
`markdownlint`, `lint`, `check-fmt`, `test`.

## Reference and skill signposts

- Execution-plan authoring: the `execplans` skill (loaded this session).
- Code navigation/refactoring: the `leta` skill and workspace (loaded this
  session); `rust-router` for any Rust follow-on.
- Rust testing strategy/terminology: `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/testing-strategy.md` (no Rust test work is added by this plan; the
  vendored column is covered by the existing GPUI regression suite).
- Doc-test DRY guidance: `docs/rust-doctest-dry-guide.md`.
- Complexity/refactoring heuristics: `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- Documentation style: `docs/documentation-style-guide.md`; en-GB Oxford
  spelling per the `en-gb-oxendict` skill.
- Gherkin terminology: `docs/gherkin-syntax.md`.
- Design anchor: `docs/rstest-bdd-design.md` §2.7.6.2; roadmap item and
  dual-track note: `docs/roadmap.md:784-790, 810-819`.
- Sibling execplans for layout/validation precedent:
  `docs/execplans/10-2-1-migration-guide-for-gpui-stateful-tests.md`,
  `docs/execplans/10-2-2-e0499-e0502-troubleshooting-guide.md`,
  `docs/execplans/10-2-3-migration-guide-downstream-test-advice.md`.
- Prior planning ExecPlan that seeded the banner/table:
  `docs/execplans/adopt-v0-6-0-beta2-feedback.md`.

[gherkin]: gherkin-syntax.md

## Revision note

- 2026-06-13 (post community-of-experts panel — Pandalump, Wafflecat, Buzzy Bee,
  Telefono, Doggylump, Dinolump): revised the draft before delivery.
  - What changed: (1) the two beyond-the-four differences (the `add_window_view`
    return type and the vendored typed `EntityError`/`Option` accessor channel)
    are now `>`-quoted prose after the table rather than folded into row 1's
    cells, keeping each row single-concern and avoiding cell overload and
    markdownlint width risk; (2) Stage D's comparison model is now unambiguous —
    anchor by heading, compare normalised whole data rows (no `\|` splitting),
    with a whitespace-only pytest case proving `make fmt` reflow cannot trip it;
    (3) the gate's scope is stated honestly (doc-vs-doc only, not external gpui
    drift) and cross-references `scripts/publish_check_gpui*.py`; (4) Stage A
    gains explicit vendored-arity and prose-audit greps; (5) Stage B's fallback
    grep now accounts for escaped pipes and is secondary to the Stage D check;
    (6) Stage F carries the exact roadmap delivery-note wording; (7) a
    `docs/developers-guide.md` table-sync + verification-recipe note is added.
  - Why: the panel surfaced a self-contradiction in the Stage D spec, a row-1
    cell-overload flaw, and three parser-brittleness failure modes that would
    have made the gate a CI nuisance, plus a false-assurance gap.
  - Effect on remaining work: no change to the spine (correct row 2, reconcile,
    optionally gate, mark roadmap done); the plan is now precise enough for a
    novice to execute and the gate is robust by construction. Stage D remains a
    genuine go/no-go against the narrow finish line.
