# ExecPlan 10.2.1: GPUI playbook for the user guide and migration guide

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, and `Outcomes & retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap item 10.2.1 closes a documentation gap exposed by the first downstream
beta migration of `rstest-bdd` 0.6.0-beta. The interim Graphical Processing
User Interface (GPUI) stateful pattern documented in
`docs/rstest-bdd-design.md` §2.7.6.2 is already exercised by the regression
suite at `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`, but the
user-facing material is incomplete. The "Stateful GPUI scenarios with durable
handles" subsection in `docs/users-guide.md` (around lines 1088–1105) starts a
code block, leaves it as an empty doctest stub, and then jumps to unrelated
localization prose. `docs/v0-6-0-migration-guide.md` covers harness selection
but omits durable handles, `VisualTestContext` reconstruction, the reserved
harness-context fixture key as a documented public contract, and the explicit
world-reset protocol entirely.

After this work is implemented, a Behaviour-Driven Development (BDD) author
adopting `rstest-bdd-harness-gpui` can migrate a stateful GPUI scenario from
beginning to end by reading two documents only: `docs/users-guide.md` for the
in-depth playbook and `docs/v0-6-0-migration-guide.md` for the migration-tuned
quick-start. Neither guide should require the reader to open
`crates/rstest-bdd-harness-gpui/src/gpui_harness.rs`, the macro expansion under
`crates/rstest-bdd-macros/src/`, or the regression suite under
`crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` to locate the
recommended pattern.

The user-visible behaviour after this change is observable through three
concrete acceptance points:

1. `docs/users-guide.md` contains a complete "Stateful GPUI scenarios" playbook
   that covers, in order, harness selection (`GpuiHarness`), the reserved
   harness-context fixture key (`rstest_bdd_harness_context`), durable handles
   (`Entity<T>` and `AnyWindowHandle`), `VisualTestContext` reconstruction from
   the durable window handle plus the harness-provided `TestAppContext`, and
   the explicit world-reset protocol (reset before assignment and reset after
   scenario teardown via a `Drop` guard). The example compiles as
   `rust,no_run` doctests and lines up name-for-name with the regression test
   under `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`.
2. `docs/v0-6-0-migration-guide.md` carries a short "Migrate a stateful GPUI
   test" playbook as a subsection inside the existing "Adopt GPUI harness
   configuration" entry (which is itself under "New features requiring new
   practices"). The subsection walks a reader from a non-stateful 0.5-style
   world to the recommended 0.6 shape and cross-references both the user
   guide playbook and the design document subsection without restating the
   rationale.
3. The repository quality gates pass: `make check-fmt`, `make lint`,
   `make test`, and `make markdownlint` all exit zero, including any new or
   refreshed doctests in `users-guide.md`, and a CodeRabbit pass on the final
   commit returns no unresolved concerns.

This task is documentation-led. The plan must not change public Rust APIs, the
existing GPUI harness implementation, or the regression coverage. If a doctest
revision requires a tiny, additive helper to make a snippet compile under
`rust,no_run`, that helper must mirror an existing public type, never extend
it.

Implementation requires explicit approval before proceeding past the DRAFT
status. The community-of-experts review attached to this plan (see
"Surprises & discoveries") should also be reflected back into any later
revision before any user-visible commits are produced.

## Constraints

- Implement only roadmap item 10.2.1. Do not implement 10.2.2 (mutable
  `StepContext` troubleshooting entry), 10.2.3 (feature-gated test guidance),
  11.x helper APIs, or any 12.x redesign work. Cross-references to those items
  are allowed where they help a reader plan ahead; substantive content is not.
- Treat the design document and ADR-007 as authoritative. Do not propose a
  thread-local-free pattern, a typed harness-context extractor, or any
  alternative the design document defers to v0.6.1 or v0.7.0. The user guide
  playbook should label the thread-local pattern as the interim v0.6
  workaround it is, name §2.7.6.5 as the redesign target, and avoid implying
  the workaround is the long-term recommendation.
- Preserve the public contracts referenced in the playbook. Do not rename the
  reserved fixture key (`rstest_bdd_harness_context`), the `GpuiHarness`,
  `GpuiAttributePolicy`, or the published example modules. If a contract
  needs renaming to make the documentation honest, stop and escalate.
- Code samples must compile as `rust,no_run` doctests in `users-guide.md` and
  must use the same names as
  `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`, including
  `ScenarioState`, `SCENARIO_STATE`, `reset_state_after_scenario`,
  `reset_state_before_assignment`, `ScenarioStateCleanup`,
  `scenario_state_cleanup`, and `serial_test::serial` (each `#[scenario]` in
  the worked example must carry `#[serial]` because the regression suite at
  `stateful_window.rs` lines 352 and 363 applies `#[serial]` and the reset
  protocol is unsound without it). Drift between the doctest and the
  regression suite is a constraint violation, not a tolerance: escalate
  rather than diverge.
- The design document schematic at `docs/rstest-bdd-design.md` §2.7.6.2
  currently shows `cx.add_window_view(|_, cx| ...)` and a non-`Option`-
  returning `VisualTestContext::from_window` (around lines 1981–2008). The
  playbook commit must update the schematic to match the regression suite's
  `|_context|` closure and the `Option`-returning `from_window` shape, or
  the playbook will contradict the document it cites.
- Snippets that demonstrate the `Drop`-based reset guard must show both the
  `#[fixture]` declaration and the order in which the guard fires relative to
  durable-handle assignment. The reset-before-assignment ordering rationale
  from `docs/rstest-bdd-design.md` §2.7.6.2 must be quoted or paraphrased so a
  novice can justify the call to a maintainer at review time.
- The migration guide entry must use the same fixture key wording and the same
  `Drop`-guard sketch as the user guide. Where the migration guide otherwise
  needs to be terse, the difference between the two playbooks should be depth
  (the user guide carries the worked example; the migration guide carries the
  checklist plus a one-screen sketch), not contradictory advice.
- All prose must use British English with Oxford spelling
  (`en-GB-oxendict`), in line with `docs/documentation-style-guide.md`. Treat
  identifiers and external proper nouns (`color`, `LICENSE`, library names) as
  exceptions where applicable.
- Cross-link the playbooks from `docs/contents.md` only if the existing index
  entries undersell GPUI coverage. If `docs/users-guide.md` and
  `docs/v0-6-0-migration-guide.md` remain the canonical entries, prefer
  improving in-document anchors over expanding the index.
- Update `docs/CHANGELOG.md` only if the user-visible documentation surface
  changes substantively. The change description must call out the GPUI
  playbook addition without restating the design rationale.
- Do not add new dependencies, new feature flags, or new test crates. Any new
  validation must live in the existing `crates/rstest-bdd-harness-gpui/tests/`
  surface and remain behind the existing `native-gpui-tests` feature gate, in
  line with `docs/execplans/10-1-3-feature-gated-gpui-test-suite.md`.
- Run formatting, linting, and tests sequentially. Capture each command's
  output with `tee` to
  `/tmp/<action>-rstest-bdd-${BRANCH}.out` where `${BRANCH}` is
  `10-2-1-migration-guide-for-gpui-stateful-tests`, in line with the agent
  instructions in `/home/leynos/.claude/CLAUDE.md`.
- Use `coderabbit review --agent` after each substantive documentation
  milestone. CodeRabbit concerns must be cleared or explicitly recorded before
  moving on. Do not request a CodeRabbit pass while deterministic gates fail.
- Use the relevant skills: `execplans` for this living plan, `leta` for code
  and symbol navigation, `rust-router` for routing into Rust-specific concerns
  if a snippet starts looking like a redesign, `en-gb-oxendict` for prose,
  `df12-copy` for tone where the playbook needs to flow as prose rather than
  reference material, `commit-message` for each commit, and `pr-creation` when
  the draft PR is updated. Load `firecrawl` only if an external reference
  (such as upstream GPUI's `add_window_view` documentation) needs to be cited
  and the in-tree shim under `vendor/gpui` does not already capture the
  contract precisely enough.
- Do not mark roadmap item 10.2.1 done in `docs/roadmap.md` until the
  documentation has shipped, the quality gates have passed, and the
  community-of-experts revision has been reflected into the merged plan.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation requires touching more than four
  documentation files (the user guide, the migration guide, the contents
  index, and the changelog) plus, at most, one harness regression-test
  comment edit to keep the regression suite and the user guide playbook
  pointing at each other. If a doctest in `users-guide.md` cannot compile
  without changing implementation files, stop and escalate; the design must
  already cover the shape.
- Lines: stop and escalate if the user guide playbook grows beyond roughly
  220 net Markdown lines (excluding fenced code), or if the migration guide
  playbook grows beyond roughly 80 net Markdown lines. The constraint is
  readability, not line-counting; treat the threshold as an early warning, not
  a hard budget.
- Code shape: stop and escalate if a `rust,no_run` doctest requires more than
  one `# use ...` hidden import per snippet beyond what
  `rstest-bdd-harness-gpui/tests/stateful_window.rs` already imports, or if a
  doctest needs `#[allow(...)]` to silence a lint. Either signals that the
  user-facing pattern has hidden complexity the design document has not
  acknowledged.
- Iterations: stop and escalate if `make markdownlint` still fails after three
  focused fix attempts, or if `make test` fails after the documentation
  edits alone for three focused fix attempts. Repeated failures point to a
  documentation-implementation mismatch, not a typo.
- Time: stop and escalate if a single documentation milestone (drafting the
  user guide playbook, drafting the migration guide playbook, validating the
  doctests, or reflecting CodeRabbit feedback) takes more than four working
  hours of agent time.
- Interface: stop and escalate if reflecting the design document accurately
  would require a non-additive change to the public surface, including
  `GpuiHarness`, `GpuiAttributePolicy`, the reserved harness-context fixture
  key, or the macro contract.
- Ambiguity: stop and escalate if the user-supplied roadmap text and the
  design document disagree on the playbook scope, or if the community-of-
  experts review surfaces a contradiction the agent cannot resolve from the
  design document alone. Present the alternatives and request direction.

## Risks

- Risk: the user guide playbook and the regression suite drift over time as
  upstream GPUI changes the `add_window_view`, `VisualTestContext::from_window`,
  or `Entity<T>` shape. Severity: medium. Likelihood: medium. Mitigation:
  keep the playbook anchored to the regression suite by name and cross-link
  `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` from the user
  guide; in the same pull request, open a tracked follow-up issue to lift
  the regression-suite step bodies into a `#[doc = include_str!(...)]`-able
  snippet file, since name-anchoring is a convention check and not an
  automated drift signal. Record the follow-up reference in this plan's
  Outcomes section so a later maintainer can decide whether to land the
  scaffolding inside the v0.7.0 lifecycle redesign or sooner.
- Risk: a reader who only sees the migration playbook concludes that
  thread-local state is the recommended long-term shape and writes new
  scenarios accordingly. Severity: medium. Likelihood: medium. Mitigation:
  open both playbooks with a clearly set-off callout (a Markdown admonition
  or block-quoted note) that labels the pattern as the v0.6 interim
  workaround, names `docs/rstest-bdd-design.md` §2.7.6.5 and roadmap items
  12.1.x as the redesign target, and remains visible to a skim reader rather
  than relying on a single opening sentence.
- Risk: the playbook adds language that conflicts with the harness panic-
  diagnostics description from roadmap item 10.1.4. Severity: low.
  Likelihood: low. Mitigation: cross-reference the existing "GPUI panic
  diagnostics carry scenario context" subsection rather than restating it,
  and keep the playbook's failure-mode advice limited to the reset protocol.
- Risk: rust doctests in `users-guide.md` rely on items that are not part of
  the public surface of `rstest-bdd-harness-gpui`, breaking `cargo test
  --doc` when downstream readers copy the snippet verbatim. Severity:
  medium. Likelihood: low if the snippet mirrors the regression suite,
  medium if the snippet drifts. Mitigation: keep the doctest minimal, use
  `rust,no_run`, mark internal helpers as scenario-local code rather than
  re-exported library helpers, and add a one-line caption noting that the
  helpers belong in the test crate.
- Risk: markdownlint trips on the playbook's nested fenced blocks or the
  bulleted reset-protocol checklist. Severity: low. Likelihood: medium.
  Mitigation: rehearse formatting against `make markdownlint` after the first
  pass, before structural revisions.
- Risk: the playbook leaks language from the design document that exceeds the
  user guide's reading-level expectations. Severity: low. Likelihood: medium.
  Mitigation: have the `df12-copy` skill voice the prose and have
  `compressed-authority` review any passage that runs more than two sentences
  of justification.

## Progress

- [ ] Stage A complete: investigation, gap inventory, and community-of-experts
  review captured here.
- [ ] Stage B complete: user guide playbook drafted in `docs/users-guide.md`
  with doctests that mirror `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`.
- [ ] Stage C complete: migration guide playbook drafted in
  `docs/v0-6-0-migration-guide.md` with cross-references to the user guide
  and the design document.
- [ ] Stage D complete: validation gates (`make check-fmt`, `make lint`,
  `make test`, `make markdownlint`) pass sequentially, with logs in `/tmp/`.
- [ ] Stage E complete: `coderabbit review --agent` returns no unresolved
  concerns and the resulting commit ships.
- [ ] Stage F complete: roadmap entry 10.2.1 marked done in `docs/roadmap.md`
  and `docs/CHANGELOG.md` updated to mention the GPUI playbook.

Use timestamps when the plan transitions to APPROVED status; the DRAFT status
intentionally carries no per-step timestamps until implementation begins.

## Surprises & discoveries

- Observation: `docs/users-guide.md` already contains a
  "Stateful GPUI scenarios with durable handles" subsection at lines 1088–
  1105, but the code block is a doctest skeleton followed by an unrelated
  localization paragraph, which means the section is published in a half-
  finished state today.
  Evidence: `docs/users-guide.md` lines 1088–1112 (line 1099 opens a
  `rust,no_run` block whose body is just a hidden struct definition; line
  1107 begins "The selection function preserves the caller-supplied order",
  which is from the localization chapter).
  Impact: the playbook work is partly a fix-up of an already-shipped header.
  The draft user-facing change should replace the truncated block in place,
  not append a new subsection elsewhere, so the document stops claiming a
  section it does not deliver.
- Observation: `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`
  carries everything the design document promises, including the reset-after
  guard (`ScenarioStateCleanup`) and the reset-before-assignment call inside
  `#[given("a fresh GPUI window is opened")]`.
  Evidence: lines 26–60 define `ScenarioState`, `SCENARIO_STATE`, the reset
  helpers, and the `Drop`-based cleanup; lines 103–123 demonstrate reset-
  before-assignment; lines 125–164 demonstrate `VisualTestContext::from_window`
  reconstruction in `#[when]` and `#[then]` steps.
  Impact: the playbook should reuse the regression-suite names verbatim and
  cite the file path so the prose and the binary cannot drift silently.
- Observation: `docs/v0-6-0-migration-guide.md` documents the GPUI harness
  configuration step (lines 324–348) but does not mention durable handles,
  the reserved fixture key in any depth, or the world-reset protocol.
  Evidence: searching the migration guide for "VisualTestContext", "reset",
  "Entity<", or "AnyWindowHandle" returns no matches.
  Impact: the migration playbook is genuinely additive; there is no existing
  text to refactor.
- Observation: the Logisphere pre-implementation review (2026-06-02)
  returned a "Proceed with conditions" verdict and ten concrete revisions.
  The most load-bearing of those, now reflected in this plan's constraints,
  Stage A outline, and risk register, are: the migration playbook lives
  inside "Adopt GPUI harness configuration" (Pandalump's contradiction
  finding); each `#[scenario]` in the worked example must carry `#[serial]`
  (Doggylump's parallel-reader scenario); the playbook must show
  `VisualTestContext::from_window` returning `Option<_>` and call out the
  `unwrap_or_else(|| panic!(...))` shape (Telefono); the reset protocol
  paragraph must include a worked failure example and a justification that
  the constructor reset and the `Drop` reset are not redundant (Doggylump's
  duplicate-reset deletion scenario); the playbook commit must update the
  design document schematic at §2.7.6.2 so the prose and the document it
  cites no longer disagree (Buzzy Bee, Telefono); the v0.6/v0.7 framing
  must be a set-off callout, not a single sentence (Dinolump); the user
  guide outline must distinguish the fixed fixture key from the
  adapter-specific parameter name (Telefono); and the drift-risk mitigation
  must promise a tracked follow-up issue for `#[doc = include_str!(...)]`
  scaffolding rather than only a retrospective note (Buzzy Bee).
  Evidence: review transcript captured in the conversation that produced
  this plan revision.
  Impact: Stages A, B, and C all carry new acceptance criteria that did
  not appear in the v1 draft. Stage A is now a sign-off gate rather than
  an edit pass.
- Observation: Wafflecat's strongest alternative — single-source the
  playbook through `#[doc = include_str!(...)]` so identifier drift becomes
  a compile error — was rejected for this iteration on cost grounds but
  captured as a follow-up. The current plan's constraint that doctests
  mirror the regression suite by name is therefore the convention-level
  fallback; the follow-up issue tracked under the drift-risk mitigation is
  the path toward the stronger guarantee.
  Evidence: same review transcript.
  Impact: the Outcomes section must record the follow-up issue identifier
  when the playbook ships.
- Observation: the existing GPUI example crate at `examples/gpui-counter/`
  only demonstrates a non-stateful counter and a single context-recording
  step; it is not a vehicle for the stateful playbook.
  Evidence: `examples/gpui-counter/tests/counter.rs` lines 31–44 record a
  `gpui::TestAppContext` but never call `add_window_view`, store an
  `Entity<T>`, or reconstruct a `VisualTestContext`.
  Impact: the playbook should reference the regression suite for the worked
  example rather than expand the counter example, which keeps the example
  crate simple and avoids implying that every GPUI scenario needs the
  stateful pattern.

## Decision log

- Decision: scope the playbook to documentation only; do not refactor the
  GPUI harness, the macro expansion, or the example crate as part of 10.2.1.
  Rationale: the design document already pins the interim pattern and the
  regression suite already proves it. Refactoring would either widen scope
  into 11.x helper-API territory or duplicate work the v0.7.0 lifecycle
  redesign will absorb.
  Date/author: 2026-06-02 (drafting agent).
- Decision: anchor the user guide doctest to the regression-suite names
  rather than invent new names.
  Rationale: divergence would require maintaining two parallel patterns and
  would let the playbook claim a shape the harness does not exercise.
  Date/author: 2026-06-02 (drafting agent).
- Decision: place the migration playbook inside the existing
  "Adopt GPUI harness configuration" section in
  `docs/v0-6-0-migration-guide.md`, with a new subsection rather than a new
  top-level section.
  Rationale: the migration guide already orients the reader from non-harness
  scenarios to harness scenarios; appending a stateful-pattern subsection
  fits the existing flow and avoids a second table of contents entry.
  Date/author: 2026-06-02 (drafting agent).
- Decision: skip Firecrawl prior-art research.
  Rationale: the source material (design §2.7.6.2, ADR-007, and the
  regression suite) is in-tree and complete; external research would not
  resolve any open question. Reopen the decision if the community-of-experts
  review identifies a missing reference.
  Date/author: 2026-06-02 (drafting agent).

## Outcomes & retrospective

To be filled in at completion. Document, at minimum, the diff between the
DRAFT playbook outline and what shipped, whether the doctests caught any
regression-suite drift, and whether the playbook surfaces the v0.7.0
lifecycle redesign promise prominently enough that a downstream reader would
not assume thread-local state is the long-term recommendation.

## Context and orientation

A reader picking this plan up cold should know the following:

- `rstest-bdd-harness-gpui` is one of the first-party harness adapter crates
  introduced in `rstest-bdd` 0.6.0. It re-exports the base harness API from
  `rstest-bdd-harness` and adds `GpuiHarness`, which delegates each scenario
  to `gpui::run_test`, injects a `gpui::TestAppContext` through the reserved
  fixture key `rstest_bdd_harness_context`, and re-raises step panics with
  feature, scenario, and line context attached. Roadmap items 9.4.5,
  10.1.1–10.1.4 already shipped the harness, the dependency-matrix
  documentation, the feature-gated suite, and the panic diagnostics.
- A "stateful" GPUI scenario is one whose steps share durable resources, in
  practice an `Entity<T>` for the view under test and an `AnyWindowHandle`
  for the window that owns it. The harness currently injects only the
  `TestAppContext`; durable resources have to live in scenario-local state
  because the v0.6 `StepContext::borrow_mut` API cannot simultaneously
  borrow `&mut TestAppContext` and `&mut World` from the same context
  (design §2.7.6.1). The accepted v0.6 workaround is a thread-local world
  with an explicit reset protocol (design §2.7.6.2). The v0.7.0 redesign
  (design §2.7.6.5) is the intended long-term fix.
- The reserved harness-context fixture key is `rstest_bdd_harness_context`.
  Step functions request the injected context with
  `#[from(rstest_bdd_harness_context)]`. The parameter name on the receiving
  side is adapter-specific; the fixture key itself is fixed and is the same
  for Tokio, GPUI, and any future first-party adapter.
- Worked references live in three files: the design document at
  `docs/rstest-bdd-design.md` §§2.7.6.1–2.7.6.2 (rationale), the regression
  suite at `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`
  (executable evidence), and the feature file at
  `crates/rstest-bdd-harness-gpui/tests/features/stateful_window.feature`
  (the Gherkin shape the regression suite binds to). The playbook should
  cite the file paths so a reader can read, compile, and run the same
  pattern locally.
- Style: `docs/documentation-style-guide.md` mandates British English with
  Oxford spelling, sentence-case headings, 80-column prose, 120-column
  code, and the Oxford comma where it aids comprehension. The
  `en-gb-oxendict` skill enforces these.

## Plan of work

The plan is staged so each stage ends with a verifiable artefact. Move to
the next stage only when the previous stage's validation passes.

### Stage A: outline sign-off (gate, not edits)

Stage A is a gate, not an edit pass: it confirms the outline below is the one
implementation will follow. No documentation files change during Stage A.

The agreed playbook structure for `docs/users-guide.md` is:

1. A clearly set-off callout (Markdown admonition or block-quoted note) up
   front classifying the pattern as the v0.6 interim workaround for
   ADR-007, naming `docs/rstest-bdd-design.md` §2.7.6.5 and roadmap items
   12.1.x as the redesign target, and visible to a skim reader. A single
   opening sentence is not sufficient.
2. A "When to reach for the stateful playbook" paragraph that draws the line
   between scenarios that need shared mutable harness context plus durable
   handles and scenarios that can keep using ordinary `rstest` fixtures.
3. A "Durable handles versus visual context" paragraph that explains why
   `VisualTestContext` is not stored across steps and why `Entity<T>` plus
   `AnyWindowHandle` are.
4. A "Reset protocol" paragraph that explains both halves of the protocol:
   reset before assigning fresh scenario state in the first `#[given]`, and
   reset after teardown through a `Drop`-based fixture guard. The paragraph
   must include a worked failure example mirroring the regression suite's
   `stale_window_count` assertion at `stateful_window.rs` lines 107 and
   119–122, and a one-line caption explaining why the constructor reset and
   the `Drop` reset are not redundant: they cover different reuse paths
   (skipped scenarios, panicking scenarios, and the case where a fresh
   fixture constructor runs before the previous scenario's `Drop` has been
   observed on the same serial thread). A reader who reads only this
   paragraph must come away knowing that deleting either reset call is a
   correctness regression.
5. A worked example, in three doctested snippets, that mirrors the
   regression suite line for line and uses the same identifiers. The
   snippets cover (i) the scenario-state and reset helpers (including the
   `#[serial]` attribute on each `#[scenario]`), (ii) the `#[given]` that
   opens a window and stores the handles, and (iii) one of the `#[when]`
   and `#[then]` steps that rebuild `VisualTestContext` from the stored
   window handle plus the harness-provided `TestAppContext`. The snippets
   must show that `VisualTestContext::from_window` returns
   `Option<VisualTestContext>` and use the `unwrap_or_else(|| panic!(...))`
   shape used at `stateful_window.rs` lines 98–100, 130–131, and 143–144.
   Pick one error shape for the step bodies — `StepResult<()>` returns or
   `unwrap_or_else(|| panic!(...))` — and apply it consistently across all
   three snippets, with a one-sentence trade-off note explaining the
   choice.
6. A "Where to read more" cross-reference list pointing at
   `docs/rstest-bdd-design.md` §2.7.6.2,
   `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`,
   `crates/rstest-bdd-harness-gpui/tests/features/stateful_window.feature`,
   and the roadmap entries that close out the v0.7.0 redesign.
7. A one-paragraph note distinguishing the fixed fixture *key*
   (`rstest_bdd_harness_context`) from the *parameter* name (which is
   adapter-specific and chosen by the step author). Cite the regression
   suite's `context` parameter name as one valid choice and note that the
   `#[from(rstest_bdd_harness_context)]` attribute is the contract, not the
   binding name. This protects the playbook from teaching readers to copy
   parameter names that look like part of the contract.

Confirm that the agreed playbook structure for `docs/v0-6-0-migration-guide.md`
is:

1. A short one-paragraph entry within "Adopt GPUI harness configuration" that
   names the stateful-pattern subsection and labels it as the v0.6 interim
   shape.
2. A "Migrate a stateful GPUI test" subsection that summarizes the playbook
   in five steps, mirrors the user guide outline, and links the user guide
   playbook by anchor.
3. A migration-checklist line item that asks the reader to apply the reset
   protocol before promoting any scenario from non-stateful to stateful.

Acceptance: the outline above is recorded under "Plan of work", and the
community-of-experts review has signed off. No documentation edits yet. The
review summary lives under "Surprises & discoveries"; any later edits to the
outline must update both locations together.

### Stage B: user guide playbook (edits)

In `docs/users-guide.md`, replace the existing
"Stateful GPUI scenarios with durable handles" subsection (lines 1088–1105 at
draft time) with the playbook agreed in Stage A. Keep the heading text the
same to avoid breaking inbound anchors. The replacement content must:

- Cite the regression suite path explicitly so a reader can read the
  executable evidence.
- Use the identifiers `ScenarioState`, `SCENARIO_STATE`,
  `reset_state_after_scenario`, `reset_state_before_assignment`,
  `ScenarioStateCleanup`, and `scenario_state_cleanup` exactly as the
  regression suite uses them.
- Mark each example as `rust,no_run` and add the hidden imports needed for
  the snippet to compile (`use rstest::fixture;`, the macro imports, the
  `gpui` imports). Keep hidden lines to a minimum.
- End with a short captioned "Where to read more" list, anchored
  consistently with the rest of the user guide.

Acceptance: `make markdownlint` passes. `make test` passes, including any
doctests touched in `users-guide.md`.

### Stage C: migration guide playbook (edits)

In `docs/v0-6-0-migration-guide.md`, add a subsection to "Adopt GPUI harness
configuration" titled "Migrate a stateful GPUI test". The subsection must:

- Open with a single sentence labelling the pattern as the v0.6 interim
  shape and naming the design document subsection and v0.7.0 redesign
  target.
- List five migration steps mapped to the user guide playbook headings.
  Steps must be observable as either a `Cargo.toml` change, a step-function
  change, a scenario-state addition, a reset-protocol addition, or a fixture
  cleanup wiring change.
- Link the user guide playbook by anchor and the design subsection by anchor.
- Add a checklist item to the "Migration checklist" section that captures
  the reset protocol.

Acceptance: `make markdownlint` passes. `make test` passes. Cross-references
resolve when previewed with the same Markdown renderer that powers
`docs/contents.md`.

### Stage D: index and changelog (edits)

Add or refresh entries in `docs/contents.md` only if the existing entries do
not surface the GPUI playbook with enough specificity. Otherwise leave the
index alone.

Add a single `## Unreleased` bullet in `docs/CHANGELOG.md` that says, in one
sentence, that the user guide and v0.6.0 migration guide now carry a stateful
GPUI playbook.

Acceptance: `make markdownlint` passes. The changelog entry follows the
existing tone in `docs/CHANGELOG.md`.

### Stage E: validation and CodeRabbit review

Run the quality gates sequentially, capturing each command with `tee`:

```bash
make check-fmt   2>&1 | tee /tmp/check-fmt-rstest-bdd-${BRANCH}.out
make lint        2>&1 | tee /tmp/lint-rstest-bdd-${BRANCH}.out
make test        2>&1 | tee /tmp/test-rstest-bdd-${BRANCH}.out
make markdownlint 2>&1 | tee /tmp/markdownlint-rstest-bdd-${BRANCH}.out
```

Replace `${BRANCH}` with the branch name
`10-2-1-migration-guide-for-gpui-stateful-tests`. Each command must exit zero
before the next runs.

Run `coderabbit review --agent` on the resulting commit. Address every
non-cosmetic concern in place, or record the deferral in the decision log
with rationale. Do not run CodeRabbit until the deterministic gates above
pass.

### Stage F: roadmap close-out

Mark `docs/roadmap.md` item 10.2.1 as done, including a delivery date and a
one-sentence summary, only after Stage E passes and the documentation edits
are committed. Update `Outcomes & retrospective` in this plan in the same
commit.

## Concrete steps

The exact command sequence inside the working tree at
`/home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/cd04aa20-f3a6-4a88-af9b-ff649e0d31a1`:

1. `git branch --show-current` to confirm the branch is
   `10-2-1-migration-guide-for-gpui-stateful-tests` before any commit.
2. Edit `docs/users-guide.md` per Stage B.
3. Edit `docs/v0-6-0-migration-guide.md` per Stage C.
4. Edit `docs/contents.md` and `docs/CHANGELOG.md` per Stage D, only as
   needed.
5. Run the four quality gates per Stage E.
6. `coderabbit review --agent` and resolve concerns.
7. Mark roadmap item 10.2.1 done per Stage F and amend
   `Outcomes & retrospective` in this plan.

Each commit message is produced with the `commit-message` skill and applied
via `git commit -F`. Each commit ends with the standard `Co-Authored-By`
trailer.

## Validation and acceptance

Acceptance is observable when:

- `docs/users-guide.md` contains a complete "Stateful GPUI scenarios with
  durable handles" subsection whose doctest compiles and whose identifiers
  line up with `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`.
- `docs/v0-6-0-migration-guide.md` contains a "Migrate a stateful GPUI
  test" subsection inside "Adopt GPUI harness configuration", with the
  required cross-references and a matching migration-checklist line item.
- `make check-fmt`, `make lint`, `make test`, and `make markdownlint` exit
  zero in sequence.
- `coderabbit review --agent` returns no unresolved concerns on the final
  commit.
- `docs/roadmap.md` item 10.2.1 is checked off, and `docs/CHANGELOG.md`
  contains a one-line entry for the playbook in `## Unreleased`.

Quality criteria for "done":

- Tests: `make test` passes; any doctest changes in `users-guide.md`
  succeed under `cargo test --doc` when the `native-gpui-tests` feature is
  not active (because the snippets are `rust,no_run`).
- Lint: `make lint` passes; `make markdownlint` passes; `make check-fmt`
  passes.
- Performance: not applicable; the change is documentation-only.
- Security: not applicable; no new dependencies, no new code paths.

Quality method:

- Sequential local runs of the four gate commands above, with logs in
  `/tmp/`.
- `coderabbit review --agent` on the final commit.
- The community-of-experts review captured in this plan is reflected back
  into the final draft before the PR moves from DRAFT to READY.

## Idempotence and recovery

All steps are re-runnable. The documentation edits are idempotent because
each edit replaces a known fragment with a known fragment. If a gate fails:

- `make markdownlint` failure: re-run after fixing the warnings shown in
  `/tmp/markdownlint-rstest-bdd-${BRANCH}.out`. Do not edit `.markdownlint`
  rules to silence findings; fix the prose instead.
- `make test` failure on a doctest: read the failing doctest line from the
  log, narrow the snippet to the smallest shape that compiles under
  `rust,no_run`, and re-run.
- `coderabbit review --agent` failure: address each concern in place; if a
  concern cannot be addressed without violating a constraint, stop and
  escalate.

If the regression suite changes underfoot while the playbook is being
written, re-run the investigation in Stage A before continuing.

## Artifacts and notes

The minimum artefacts to keep after delivery are:

- The diff for `docs/users-guide.md`, `docs/v0-6-0-migration-guide.md`,
  `docs/contents.md` (if touched), and `docs/CHANGELOG.md`.
- The four log files in `/tmp/` from the validation stage.
- The CodeRabbit summary for the final commit.

## Interfaces and dependencies

This plan introduces no new public Rust interfaces. The dependencies referenced
in the playbook are unchanged:

- `rstest-bdd-harness-gpui::GpuiHarness` (re-exported as
  `rstest_bdd_harness_gpui::GpuiHarness`) for scenario harness selection.
- `rstest-bdd-harness-gpui::GpuiAttributePolicy` for attribute-policy
  resolution, inferred from the canonical harness path when
  `attributes = ...` is omitted.
- The reserved fixture key `rstest_bdd_harness_context`, requested by step
  functions as `#[from(rstest_bdd_harness_context)] cx: &mut gpui::TestAppContext`.
- The upstream GPUI types `gpui::TestAppContext`, `gpui::VisualTestContext`,
  `gpui::Entity<T>`, and `gpui::AnyWindowHandle`. The playbook treats the
  shape of `TestAppContext::add_window_view` and
  `VisualTestContext::from_window` as authoritative; if upstream renames or
  resigns either function, stop and escalate.

The playbook must reference the following file paths verbatim so a reader can
read the executable evidence locally:

- `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs`
- `crates/rstest-bdd-harness-gpui/tests/features/stateful_window.feature`
- `docs/rstest-bdd-design.md` §§2.7.6.1–2.7.6.2 and §2.7.6.5
- `docs/v0-6-0-migration-guide.md` "Adopt GPUI harness configuration"
- `docs/users-guide.md` "Stateful GPUI scenarios with durable handles"

## Revision note

Initial DRAFT authored 2026-06-02 by the drafting agent. Revised the same
day to reflect a Logisphere pre-implementation design review that returned a
"Proceed with conditions" verdict. The revision resolved a placement
contradiction in the Purpose section, added `#[serial]` and the
`Option`-returning `VisualTestContext::from_window` shape to the constraint
list, required the playbook commit to update `docs/rstest-bdd-design.md`
§2.7.6.2 so the schematic matches the regression suite, promoted the v0.6
interim labelling from a single sentence to a set-off callout, added a
parameter-name-vs-fixture-key clarification to the user-guide outline,
replaced the drift-risk retrospective note with a tracked follow-up issue
commitment, renamed Stage A to "outline sign-off" (a gate, not an edit
pass), and captured Wafflecat's `#[doc = include_str!(...)]` alternative
under Surprises & discoveries as a deferred follow-up. Any later edit must
update the Status field at the top of the plan, append a brief note to this
section, and keep the living sections current.
