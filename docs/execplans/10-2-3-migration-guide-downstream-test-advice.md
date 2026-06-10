# Migration guide downstream-test advice (10.2.3)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
and `Outcomes & retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Downstream adopters who migrate to `rstest-bdd` v0.6.0 currently have no
documented way to tell a real API regression from a fixture or feature-flag
mistake. The first beta migration showed adopters reaching for compiler-error
archaeology before they had run the project's own gate on the migrated tree.
This roadmap item closes that loop in the migration guide so that, before a
user files a "v0.6.0 broke my tests" report or rewires a step macro, they have
already run the feature-gated test suite and the repository's CI-equivalent
gate locally and confirmed the failure shape.

A reader who finishes the new section will be able to:

1. Identify the two command shapes that exercize feature-gated tests — the
   project's own `make test` (or equivalent gate) and a generic
   `cargo test --workspace --all-features` invocation for projects without a
   Make-based gate.
2. Name the repository's CI-equivalent gate as the canonical proof that the
   migration is sound, and run it on the migrated tree before opening an issue
   or rewriting a step.
3. Distinguish "feature gate not enabled" failures from genuine API
   regressions, so harness-feature interactions surface before the user reaches
   for `cargo expand` or the borrow-checker docs.

Success is observable as: a new bullet in the migration checklist that names
both command shapes verbatim, `make markdownlint` passing on the modified file,
and downstream adopters being able to follow the same advice without reading
the design document or this ExecPlan.

## Constraints

- The change is documentation-only. No code under `crates/`, no harness API,
  and no macro behaviour may be modified by this plan. If implementation
  reveals a docs change that requires a code adjustment, stop and escalate.
- `docs/v0-6-0-migration-guide.md`, `docs/roadmap.md`, and (only if
  required) `docs/contents.md` are the only files this plan is permitted to
  modify. The roadmap tick at `docs/roadmap.md:769` is treated as part of the
  same logical delivery, mirroring the squash-merge pattern used by the 10.2.1
  and 10.2.2 PRs.
- Existing migration-guide content must be preserved verbatim except where
  the new bullet is inserted and where the new label-style link definition is
  added. The "Migration checklist" heading at line 422 and the "Common errors
  and fixes" heading at line 446 are anchors that must not move except by
  inserting between them.
- Command shapes named in the new prose must match the repository's own
  Makefile (`Makefile:11,26-32`): `make test` resolves to
  `cargo nextest run --workspace --all-targets --all-features` or
  `cargo test --workspace --all-targets --all-features` (fallback when
  `cargo-nextest` is absent). The generic shape we recommend to downstream
  projects without `make` is `cargo test --workspace --all-features`. Do not
  introduce a third shape that does not appear in either.
- Prose must hold to en-GB Oxford spelling and the documentation style guide
  at `docs/documentation-style-guide.md`. Paragraphs and bullet items wrap at
  80 columns; fenced code blocks wrap at 120.
- `make markdownlint` must pass on the modified file before any review is
  requested.
- The migration guide is the canonical home for migration-specific runbooks
  (`docs/v0-6-0-migration-guide.md`); the users' guide is not the target. The
  research established that `docs/users-guide.md` has no equivalent "run
  downstream tests" guidance, and 10.2.3 must not introduce one there.

## Tolerances (exception triggers)

- Scope: if the change requires touching more than three files
  (`docs/v0-6-0-migration-guide.md`, `docs/roadmap.md`, optionally
  `docs/contents.md`), or more than about 120 net lines across them, stop and
  escalate.
- Interface: if delivery seems to require changing a Rust public API, a
  macro attribute, a harness trait, or a feature flag, stop and escalate —
  10.2.3 is a documentation item.
- Dependencies: if delivery seems to require a new tool (markdown plugin,
  linter, dictionary entry beyond the existing acronym allowlist, etc.), stop
  and escalate.
- Iterations: if `make markdownlint` still fails after three focused
  attempts, stop and escalate with the specific rule code and offending line.
- Ambiguity: if reviewer feedback proposes splitting the advice across the
  users' guide and migration guide (against the research), surface the conflict
  in Decision log and ask before acting.
- Time: if drafting the prose plus running gates takes more than two hours
  total, stop and escalate.

## Risks

- Risk: the new bullet drifts from the project's actual gate. The Makefile
  may change (`cargo-nextest` may be made mandatory, feature plumbing may move
  into a wrapper target) and a stale bullet would mislead adopters. Severity:
  low. Likelihood: low. Mitigation: quote the Makefile target name
  (`make test`) and the generic cargo invocation, but do not pin a specific
  binary like `cargo-nextest` in the migration guide. Reference
  `docs/rstest-bdd-design.md` §2.7.6.3 as the durable anchor.
- Risk: adopters whose projects do not use `make` ignore the advice as
  internal-only. Severity: medium. Likelihood: medium. Mitigation: the bullet
  must lead with the portable shape (`cargo test --workspace --all-features` or
  the project's CI-equivalent gate) and give `make test` as the
  rstest-bdd-specific example, not as the primary recommendation.
- Risk: the new advice is read as a general testing guide and bloats the
  migration checklist with unrelated diagnostics. Severity: low. Likelihood:
  medium. Mitigation: keep the new content to a single checklist bullet,
  matching the imperative style of the eight existing bullets. Do not add a
  rationale subsection, a fault tree, or a flowchart; defer the "why" to the
  design document via a label-style cross-link, mirroring the
  `[design-borrow-constraint]` pattern used by 10.2.2.
- Risk: markdownlint rejects the new bullet because of relative-link or
  line-length issues. Severity: low. Likelihood: medium. Mitigation: run
  `make markdownlint` after each substantive edit; keep the bullet under 80
  columns; use the existing label-style links rather than inline URLs.

## Progress

- [x] (2026-06-07) Capture user approval for this plan (gate before
  implementation begins).
- [x] (2026-06-07) Stage A — locate insertion point and capture absence.
  Confirm the new bullet will land between `docs/v0-6-0-migration-guide.md:444`
  and `:446`, and run the red-stage grep that proves the advice is not yet
  present.
- [x] (2026-06-07) Stage B — add the checklist bullet and the new
  label-style link.
- [x] (2026-06-07) Stage C — refactor for wrap width and link style; run
  `make markdownlint` as the substantive gate, then `make check-fmt`,
  `make lint`, and `make test` as sanity checks, capturing logs under
  `/tmp/<action>-rstest-bdd-${BRANCH}.out`.
- [x] (2026-06-07) Stage D — run `coderabbit review --agent` and clear
  concerns before marking the roadmap entry done.
- [x] (2026-06-07) Update `docs/roadmap.md:769` to mark item 10.2.3 as
  delivered with the date and a one-line summary, following the 10.2.1/10.2.2
  pattern.
- [x] (2026-06-07) Update `docs/contents.md` only if the migration guide
  gains a new top-level heading visible from the index (this plan does not
  currently expect a new index entry).

## Surprises & discoveries

- Stage A found no overlapping existing prose in
  `docs/v0-6-0-migration-guide.md` for `cargo test`, `make test`,
  `--all-features`, `make markdownlint`, or `CI`; the planned bullet can be
  inserted without reconciling duplicate guidance.
- The first `make markdownlint` run failed only on a line-wrap issue in this
  ExecPlan's updated Progress entry. Reflowing that entry fixed the failure;
  the second run reported `Summary: 0 error(s)`.

## Decision log

- Decision: treat the user's 2026-06-07 instruction to proceed with
  implementation as approval of this ExecPlan and move the plan from `DRAFT` to
  `IN PROGRESS`. Rationale: the execplans skill requires an approval gate
  before implementation, and the latest user request explicitly asks to
  implement this named plan while keeping it current. Date/Author: 2026-06-07,
  implementation agent.
- Decision: confine the change to `docs/v0-6-0-migration-guide.md` (plus
  the roadmap tick) and skip `docs/users-guide.md`. Rationale: research (the
  "Survey CI gates and downstream commands" agent and the "Research v0.6.0
  migration guide and related docs" agent) found that the users' guide does not
  currently carry migration-specific runbooks; it describes scenario authoring.
  The migration guide is the established home for cross-release runbook advice
  in v0.6.0, including 10.2.1's GPUI playbook and 10.2.2's borrow-checker
  workaround. Date/Author: 2026-06-07, planning agent.
- Decision: recommend the portable `cargo test --workspace --all-features`
  shape first and present `make test` as the project-internal example.
  Rationale: the roadmap text gives both shapes as alternatives. Downstream
  adopters mostly do not run `make`. The Makefile target plumbing
  (`Makefile:11`) sets
  `CARGO_FLAGS := --workspace --all-targets --all-features`, so the portable
  shape is a faithful reflection of the project's own gate. Date/Author:
  2026-06-07, planning agent.
- Decision: do not pin a specific test runner (e.g. `cargo-nextest`) in the
  migration guide. Rationale: the Makefile falls back to `cargo test` when
  `cargo-nextest` is absent (`Makefile:28-32`). Pinning the runner would create
  a documentation maintenance burden whenever the project's runner choice
  changes. Date/Author: 2026-06-07, planning agent.
- Decision: apply a documentation-shaped Red-Green-Refactor substitute
  rather than a code RGR cycle. Rationale: the execplans skill permits a
  "nearest observable substitute" for changes where the code RGR does not
  apply. A grep-based red assertion proves the advice is absent; the green
  assertion proves it is present with the exact command shapes named by the
  roadmap. Date/Author: 2026-06-07, planning agent.
- Decision: drop the rationale subsection above "Further reading"; ship a
  single checklist bullet only. Rationale: a Logisphere design review on the v0
  draft observed that the eight existing checklist bullets are terse
  imperatives without inline rationale, and that adding a rationale subsection
  would be the only asymmetric prose backing in the checklist. Deferring the
  "why" to the design document via a label-style link keeps the checklist
  consistent and reduces the diff. The design anchor at §2.7.6.3 already
  carries the motivating prose ("migration-guide coverage for feature-gated
  downstream tests such as `cargo test --all-features`"). Date/Author:
  2026-06-07, planning agent.
- Decision: name the new label `[design-beta2-quick-wins]` rather than
  `[design-feature-gates]`. Rationale: the same Logisphere review noted that
  the existing labels at `docs/v0-6-0-migration-guide.md:403-420` follow a
  "topic-of-heading" pattern (`design-borrow-constraint` → §2.7.6.1 "Borrow
  constraint…", `design-interim-gpui` → §2.7.6.2 "Interim GPUI…"). The §2.7.6.3
  heading is "v0.6.0-beta2 quick wins", so `design-beta2-quick-wins` matches
  the convention; `design-feature-gates` would describe the bullet's content,
  not the linked heading, and would invite drift if the heading changes.
  Date/Author: 2026-06-07, planning agent.

## Outcomes & retrospective

The new migration-checklist bullet landed immediately before
`## Common errors and fixes`, preserving the existing checklist shape while
naming both required command forms: `cargo test --workspace --all-features` and
`make test`. The roadmap entry 10.2.3 is marked delivered with a dated note
pointing back to this ExecPlan. `docs/contents.md` stayed unchanged because the
migration guide did not gain a new top-level heading.

Validation succeeded through `make markdownlint`, `make check-fmt`, `make lint`,
`make test`, green-stage grep assertions, and two `coderabbit review --agent`
passes with zero findings. The only correction needed during implementation was
reflowing overlong lines in this ExecPlan's Progress entries after updates; the
migration-guide and roadmap prose passed once wrapped.

## Context and orientation

`rstest-bdd` is a Rust BDD framework that uses the [Gherkin syntax][gherkin] to
drive `rstest`-style fixtures. Stable references for new readers:

- The roadmap entry under change is at
  `docs/roadmap.md:769`. It belongs to "10. First-cut beta feedback:
  v0.6.0-beta2 quick wins" → "10.2. Update adoption documentation before v0.6.0
  final".
- The v0.6.0 migration guide at `docs/v0-6-0-migration-guide.md` is the
  authoritative migration runbook. The "Migration checklist" section starts at
  line 422. The "Common errors and fixes" section starts at line 446. The
  "Further reading" section starts at line 569.
- The design document at `docs/rstest-bdd-design.md` §2.7.6.3 (lines
  2023-2048) records the "v0.6.0-beta2 quick wins" set, including the explicit
  line "migration-guide coverage for feature-gated downstream tests such as
  `cargo test --all-features`" at line 2032. This is the durable design anchor
  the migration guide must link.
- The repository's own gate lives in `Makefile`:
  - line 11: `CARGO_FLAGS ?= --workspace --all-targets --all-features`.
  - lines 26-32: `make test` runs `cargo nextest run` (preferred) or
    `cargo test` (fallback) with `CARGO_FLAGS`.
  - line 41: `make lint` runs `cargo clippy` with the same flags.
  - lines 55-57: `make check-fmt` runs `cargo fmt --all --check` and Ruff
    format check.
  - lines 59-60: `make markdownlint` runs `markdownlint-cli2` over every
    `.md` outside `target/` and `node_modules/`.
- CI mirrors these targets in `.github/workflows/ci.yml`. Coverage runs
  use `leynos/shared-actions/generate-coverage`, which delegates to
  `cargo nextest` or `cargo llvm-cov test` per matrix, with the same
  `--all-features` plumbing.
- Prior `10.2.x` execplans give the validation pattern this plan follows:
  - `docs/execplans/10-2-1-migration-guide-for-gpui-stateful-tests.md:54-55,
    129-135, 644-662` (full gate sequence, log-tee, CodeRabbit).
  - `docs/execplans/10-2-2-e0499-e0502-troubleshooting-guide.md:179-180,
    850-866` (same pattern, applied to the borrow-checker entry).

Definitions used in this plan (no prior knowledge assumed):

- **CI-equivalent gate**: the locally runnable command sequence that the
  project's continuous-integration system would run on a pull request. For
  `rstest-bdd` this is `make check-fmt`, `make lint`, `make test`,
  `make markdownlint`, run sequentially; for downstream projects it is whatever
  the project's CI does locally — frequently a single `make` target or `cargo`
  invocation with feature flags applied.
- **Feature-gated test**: a test annotated to run only when one or more
  Cargo features are enabled. With the workspace's `--all-features` flag, every
  feature-gated test runs; with default features only, several harness suites
  (notably `rstest-bdd-harness-gpui`) are skipped.
- **Migration checklist**: the bulleted list of preparatory tasks under
  `## Migration checklist` in the migration guide. Each bullet is an imperative
  step a migrating user should tick off before claiming v0.6.0 compatibility.

## Plan of work

### Stage A — locate the insertion point and capture absence

Re-read the migration guide's "Migration checklist" and "Common errors and
fixes" sections in the worktree. Confirm that the eight existing bullets at
`docs/v0-6-0-migration-guide.md:424-444` are single-imperative migration steps
with no inline rationale prose, and that the new bullet therefore belongs
alongside them in the same imperative voice. Run two assertions:

```bash
grep -n "cargo test\|make test\|--all-features\|make markdownlint\|CI" \
    docs/v0-6-0-migration-guide.md

grep -F "cargo test --workspace --all-features" \
    docs/v0-6-0-migration-guide.md \
    && echo "advice already present" \
    || echo "advice not yet present"
```

Expected output before any edit: the first grep is empty or matches only
incidental strings unrelated to the new advice; the second prints
`advice not yet present`. This second line is the documentation-shaped
red-stage assertion. Record both outputs verbatim in `Artifacts and notes`
before editing. If the first grep returns overlapping hits, surface them in
`Decision log` before proceeding.

### Stage B — apply the documentation change

In `docs/v0-6-0-migration-guide.md`:

1. Insert a new checklist bullet between the existing bullet at lines
   440-444 (the stateful GPUI reset bullet) and the
   `## Common errors and fixes` heading at line 446. The bullet must:
   - Begin with an imperative verb (matching the surrounding bullets).
   - Name the portable command shape verbatim:
     `cargo test --workspace --all-features`.
   - Name the project-shape example verbatim: `make test` for projects
     using a Make-based gate, presented as one example of the project's
     CI-equivalent gate rather than as a universal recommendation.
   - Frame the recommendation as running the gate "before assuming v0.6.0
     broke your API", echoing the Purpose framing rather than the
     internal jargon "before API diagnosis".
   - Link the design anchor as a label-style reference. Reuse the existing
     `[design-…]` label convention rooted in the heading of the linked
     section; the new label is `[design-beta2-quick-wins]` pointing at
     `rstest-bdd-design.md#2763-v060-beta2-quick-wins`, matching the
     `[design-borrow-constraint]` and `[design-interim-gpui]` precedent at
     lines 405-410.
2. Add the new label-style link definition near the other definitions at
   lines 403-420, alphabetised within the existing cluster.

No new sub-heading is introduced. The "why" remains in the design document; the
checklist keeps its single-imperative voice.

In `docs/contents.md`: leave unchanged. The migration guide's existing entry at
`docs/contents.md:40-41` already subsumes the topic.

In `docs/roadmap.md:769`: only update when delivery is complete, following the
10.2.1/10.2.2 pattern: change `- [ ]` to `- [x]`, append the delivery date and
a one-line summary that references this ExecPlan path.

### Stage C — validate

Re-flow the new bullet to the 80-column wrap if needed. Confirm the new bullet
sits visually beside the surrounding bullets. Run the substantive gate first,
then the sanity gates:

```bash
BRANCH=$(git branch --show-current)
make markdownlint 2>&1 | tee /tmp/markdownlint-rstest-bdd-${BRANCH}.out
make check-fmt    2>&1 | tee /tmp/check-fmt-rstest-bdd-${BRANCH}.out
make lint         2>&1 | tee /tmp/lint-rstest-bdd-${BRANCH}.out
make test         2>&1 | tee /tmp/test-rstest-bdd-${BRANCH}.out
```

`make markdownlint` is the test that can fail from prose changes; the other
three are sanity gates required by the project's standing contributor
instructions. Each gate must exit zero before the next runs. Record any
unexpected failure in `Surprises & discoveries`.

Run the green-stage grep assertion to confirm the new content is present:

```bash
grep -F "cargo test --workspace --all-features" \
    docs/v0-6-0-migration-guide.md
grep -F "make test" docs/v0-6-0-migration-guide.md
grep -F "[design-beta2-quick-wins]" docs/v0-6-0-migration-guide.md
```

Each grep must return at least one match.

Then run `coderabbit review --agent` and clear all concerns before requesting
human review or promoting the PR out of draft.

### Stage D — roadmap tick and delivery note

Update `docs/roadmap.md:769`: change `- [ ]` to `- [x]` and append a one-line
delivery summary that references this ExecPlan path, mirroring the 10.2.1 and
10.2.2 entries above it. Re-run `make markdownlint` to catch any new wrap-width
violation introduced by the roadmap edit. The roadmap tick is the final
evidence that the work shipped.

## Concrete steps

Working directory: `/path/to/rstest-bdd-worktree`. All commands assume this is
the current directory.

1. Confirm clean state and current branch.

   ```bash
   git status --short
   git branch --show-current
   ```

   Expected: empty `git status --short` and branch
   `10-2-3-migration-guide-downstream-test-advice`.

2. Run the Stage A absence assertions (see "Plan of work"). Record output
   in `Artifacts and notes`.

3. Edit `docs/v0-6-0-migration-guide.md` per Stage B: add the new
   checklist bullet and the new `[design-beta2-quick-wins]` reference-style
   link beside the existing design references.

4. Run validation gates in the order listed in Stage C
   (`make markdownlint` first, then the sanity trio). Stop on the first failure
   and fix the root cause; do not continue past a failing gate.

5. Run the Stage C green-stage greps and confirm matches for both command
   shapes and the new label.

6. Update `docs/roadmap.md:769` to mark 10.2.3 delivered. Re-run
   `make markdownlint` to catch any new wrap-width violation introduced by the
   roadmap edit.

7. Commit. Follow the squash-merge precedent of 10.2.1 and 10.2.2: the
   docs change and the roadmap tick may share a single commit, with an
   imperative subject naming the roadmap item number (for example, "Warn on
   feature-gated downstream tests (10.2.3)"), a wrapped body explaining what
   and why, and en-GB Oxford spelling.

8. Push and request review. Run `coderabbit review --agent` from the
   worktree before promoting the PR out of draft.

## Validation and acceptance

Acceptance is observable as:

1. The migration checklist in `docs/v0-6-0-migration-guide.md` contains a
   new bullet that names both `cargo test --workspace --all-features` and
   `make test` (or "the project's CI-equivalent gate") verbatim, and the bullet
   sits topically with the surrounding migration steps.
2. The grep assertions in Stage A (absence) and Stage C (presence)
   succeed as described.
3. `make markdownlint` exits zero, and the sanity gates `make check-fmt`,
   `make lint`, and `make test` continue to exit zero on the worktree.
4. `coderabbit review --agent` reports no remaining concerns.
5. `docs/roadmap.md:769` marks 10.2.3 as `[x]` with a one-line delivery
   summary that references this ExecPlan path.

Quality criteria:

- Tests: no Rust tests are added or modified; existing tests must continue
  to pass via `make test`.
- Lint and typecheck: `make lint` and `make check-fmt` pass; the documents
  satisfy `make markdownlint`.
- Performance: not applicable.
- Security: not applicable; documentation-only.

Quality method:

- Local: the sequential gate listed above, with logs teed to `/tmp/`.
- CI: GitHub Actions `ci.yml` runs the same gates; the PR remains draft
  until CI is green.
- Human / CodeRabbit: `coderabbit review --agent` on the local worktree
  before promotion.

## Idempotence and recovery

Each step is re-runnable without damage:

- The Stage B and Stage E grep assertions are read-only.
- The migration-guide edit is a single contiguous insertion; reverting the
  commit fully restores the prior state.
- The roadmap edit is one bullet's checkbox and trailing prose; reverting
  the second commit restores the `- [ ]` state.
- `make` targets are idempotent in this repository.

If `make markdownlint` fails after a partial edit, fix the line-wrap or link
issue and re-run; do not undo earlier edits unless the markdownlint rule code
points at them directly.

If `coderabbit review --agent` flags substantive prose changes, apply the edits
and re-run the local gates from `make check-fmt` onwards before re-running the
review tool.

## Artifacts and notes

- Stage A absence assertion, first grep:

  ```plaintext
  <no output>
  ```

- Stage A absence assertion, exact portable command:

  ```plaintext
  advice not yet present
  ```

- Stage C green-stage assertions:

  ```plaintext
  448:  use `cargo test --workspace --all-features`, or the project's Continuous
  449:  Integration (CI)-equivalent gate such as `make test` when a Make-based gate
  405:[design-beta2-quick-wins]: rstest-bdd-design.md#2763-v060-beta2-quick-wins
  451:  [v0.6.0-beta2 quick win][design-beta2-quick-wins].
  ```

- Stage C gate summaries:

  ```plaintext
  make markdownlint: Summary: 0 error(s)
  make check-fmt: 30 files already formatted
  make lint: All checks passed!
  make test: 1487 passed, 7 skipped; 62 Python tests passed
  ```

- Stage D CodeRabbit review:

  ```plaintext
  {"type":"complete","status":"review_completed","findings":0}
  ```

- Final CodeRabbit review after the roadmap tick:

  ```plaintext
  {"type":"complete","status":"review_completed","findings":0}
  ```

- Expected Stage A absence-assertion output:

  ```plaintext
  advice not yet present
  ```

- Expected Stage C green-stage output (after the change):

  ```plaintext
  docs/v0-6-0-migration-guide.md:NNN:  …`cargo test --workspace --all-features`…
  ```

  where `NNN` is the new bullet line number.

- Final transcript fragments from each `make` gate (just the trailing
  "passed" or "0 errors" line) are pasted here at completion.

## Interfaces and dependencies

This plan does not modify or introduce any Rust interface. It depends on, and
must remain consistent with, the following existing surfaces:

- The Makefile targets `make test`, `make check-fmt`, `make lint`, and
  `make markdownlint` (`Makefile:26-32, 41, 55-57, 59-60`). The plan references
  these by name; it does not change them.
- The design-document anchor `rstest-bdd-design.md#2763-v060-beta2-quick-wins`
  (or whichever stable anchor markdown-it / GitHub renders for §2.7.6.3). The
  plan adds a reference-style link to this anchor under the label
  `[design-beta2-quick-wins]`, matching the `[design-borrow-constraint]` /
  `[design-interim-gpui]` topic-of-heading naming convention at
  `docs/v0-6-0-migration-guide.md:403-420`.
- The migration guide's existing label-style reference cluster at
  `docs/v0-6-0-migration-guide.md:403-420`. The plan adds one new label,
  alphabetised.

No new crates, no new feature flags, no new Cargo configuration.

## Reference and skill signposts

- Execution-plan authoring: the `execplans` skill (loaded at the start of
  this session).
- Rust documentation conventions: `docs/documentation-style-guide.md`.
- Migration-guide siblings for layout precedent:
  - `docs/execplans/10-2-1-migration-guide-for-gpui-stateful-tests.md`.
  - `docs/execplans/10-2-2-e0499-e0502-troubleshooting-guide.md`.
- Project convention for Rust testing strategy:
  `docs/testing-strategy.md`, `docs/rust-testing-with-rstest-fixtures.md`
  (referenced for terminology; no Rust test work is added by this plan).
- Design anchor: `docs/rstest-bdd-design.md` §2.7.6.3
  (`docs/rstest-bdd-design.md:2023-2048`).
- Gherkin reference (for terminology only): `docs/gherkin-syntax.md`.
- En-GB Oxford spelling, with `outwith` and `caveat` permitted, follows the
  `en-gb-oxendict` skill.

[gherkin]: gherkin-syntax.md
