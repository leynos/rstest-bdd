# ExecPlan 10.2.2: E0499/E0502 troubleshooting entry in the v0.6.0 migration guide

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`,
`Decision log`, and `Outcomes & retrospective` must be kept up to date as work
proceeds.

Status: DRAFT (awaiting user approval; revised 2026-06-04 to absorb the
Logisphere pre-implementation review — see "Surprises & discoveries")

## Purpose / big picture

Roadmap item 10.2.2 closes the last named documentation gap exposed by the
first downstream beta migration of `rstest-bdd` 0.6.0-beta. The
`StepContext::borrow_mut` contract has a known design limitation: a step that
requests two mutable fixtures from the same `StepContext` — typically a
harness-context fixture plus a world fixture, both bound as `&mut T` — cannot
compile, because the generated wrapper has to call `ctx.borrow_mut(...)` twice
on the same `&mut StepContext`. Rust rejects that as `E0499` ("cannot borrow
`*ctx` as mutable more than once at a time"). The mixed shape, where one
parameter is `&T` and the other is `&mut T`, fails as `E0502` ("cannot borrow
`*ctx` as mutable because it is also borrowed as immutable") for the same
structural reason. The constraint is recorded in
`docs/rstest-bdd-design.md` §2.7.6.1, the interim workaround is recorded in
§2.7.6.2, and the v0.7.0 redesign that replaces both is tracked in §2.7.6.5
and roadmap items 12.1.x. Roadmap item 10.2.1 already added the stateful
GPUI playbook to `docs/users-guide.md` and the "Migrate a stateful GPUI test"
subsection to `docs/v0-6-0-migration-guide.md`.

What is still missing is a *short, name-the-symptom-by-its-rustc-code*
troubleshooting entry that a downstream user lands on the first time they
hit `E0499` or `E0502` in step code. Today, a reader who follows the
borrow-checker error to its rustc diagnostic gets generic Rust-language
guidance and has to do "compiler-error archaeology" — read the macro
expansion, then track down §2.7.6.1 — before they realise the fix is in the
stateful GPUI playbook they have not yet been told about. The migration
guide must close that loop.

After this work is implemented, a Behaviour-Driven Development (BDD) author
adopting `rstest-bdd` 0.6.0 who sees `E0499` or `E0502` in a step wrapper
can resolve it by reading `docs/v0-6-0-migration-guide.md` only. The
entry names both error codes, shows the offending step shape, explains
why the wrapper cannot satisfy it under the v0.6 `StepContext` API, and
points the reader at the three concrete v0.6-compatible escapes: the
stateful GPUI playbook (when the second mutable is harness context), a
single mutable plus an immutable borrow (when one fixture is read-only),
or splitting the step into two smaller steps (when neither escape fits).
It cross-links design §2.7.6.1 for rationale and §2.7.6.5 plus roadmap
item 12.1.x for the redesign target, so a reader who wants to know
"why is this the v0.6 answer" or "is this getting better" can find both
in one click.

The user-visible behaviour after this change is observable through three
concrete acceptance points:

1. `docs/v0-6-0-migration-guide.md` contains a "Two mutable fixtures
   trigger `E0499` or `E0502`" troubleshooting subsection that names both
   error codes by symbol, shows a minimal failing step (matching the
   §2.7.6.1 schematic identifier-for-identifier), distinguishes the
   `E0499` case (two `&mut`) from the `E0502` case (one `&` and one
   `&mut`), and links the stateful GPUI playbook anchor
   `#migrate-a-stateful-gpui-test`, the user-guide playbook anchor
   `users-guide.md#stateful-gpui-scenarios-with-durable-handles`, and the
   design-document subsection §2.7.6.1.
2. The existing `## Common errors and fixes` bullet list gains a one-line
   entry pointing at the new troubleshooting subsection by anchor, so a
   reader scanning for error symptoms is funnelled to the long-form
   guidance rather than reading a one-liner that repeats the symptom
   without explaining the fix.
3. The repository quality gates pass: `make check-fmt`, `make lint`,
   `make test`, and `make markdownlint` all exit zero, and a CodeRabbit
   pass on the final commit returns no unresolved concerns.

This task is documentation-led. The plan must not change public Rust APIs,
the macro codegen at
`crates/rstest-bdd-macros/src/codegen/wrapper/arguments/fixtures.rs`, the
`StepContext::borrow_mut` signature at
`crates/rstest-bdd/src/context/mod.rs`, or any harness implementation.
The whole point of the entry is that the v0.6 contract is what it is and
the reader has to work around it; an entry that secretly demands an API
change would be re-scoping the work into 11.x or 12.x.

Implementation requires explicit approval before proceeding past the
DRAFT status. The community-of-experts review attached to this plan
(see "Surprises & discoveries") must also be reflected back into any
later revision before any user-visible commits are produced.

## Constraints

- Implement only roadmap item 10.2.2. Do not implement 10.2.3 (feature-
  gated test guidance), 11.x helper APIs (typed borrow-error enum,
  scenario-local state helper), or any 12.x redesign work. Cross-references
  to those items are allowed where they help a reader plan ahead;
  substantive content is not.
- Treat the design document and ADR-007 as authoritative. The entry must
  describe the v0.6 behaviour and the v0.6-compatible workarounds. It must
  not propose a thread-local-free pattern, a typed harness-context
  extractor, or any alternative the design document defers to v0.6.1 or
  v0.7.0. The v0.6/v0.7 framing must match the framing already used by
  10.2.1: this is an interim shape, §2.7.6.5 is the redesign target.
- Preserve the public contracts referenced in the entry. Do not rename the
  reserved fixture key (`rstest_bdd_harness_context`), the
  `StepContext::borrow_mut` or `borrow_ref` signatures, the `GpuiHarness`,
  the `TokioHarness`, or the published example modules. If reflecting the
  design document accurately would require a non-additive surface change,
  stop and escalate; the rationale section of the entry has to be honest
  about a v0.6 limitation, not paper it over.
- The failing-shape code sample must be tagged `rust,ignore` (not
  `rust,no_run`, and **not** `rust,compile_fail,ignore`), because the
  whole point is that it *does not compile* under the current
  generated wrapper. `rust,ignore` documents that the doctest harness
  must skip the snippet without asserting the failure mode — the
  intent is captured in a one-line caption immediately above the
  fence so a reader who pastes the snippet sees the warning without
  scrolling. The reason for avoiding `compile_fail` is operational:
  a future codegen change or rustc edition shift that legitimately
  resolves the conflict would flip a `compile_fail` doctest into a
  spurious "expected compile failure didn't fail" CI red, while a
  plain `ignore` decays gracefully into a stale-but-quiet snippet.
  Compile-time drift detection belongs in a dedicated trybuild
  fixture, deferred to 11.x — see the deferred follow-up captured
  under Outcomes & retrospective.
- The recommended-shape code samples must compile as `rust,no_run`
  doctests and must use the same identifiers as
  `crates/rstest-bdd-harness-gpui/tests/stateful_window.rs` and the
  user-guide playbook (`ScenarioState`, `SCENARIO_STATE`,
  `reset_state_after_scenario`, `reset_state_before_assignment`,
  `ScenarioStateCleanup`, `scenario_state_cleanup`, `serial_test::serial`).
  Drift between the migration-guide snippet and the user-guide playbook
  is a constraint violation, not a tolerance: escalate rather than
  diverge.
- The entry must name both error codes in the subsection heading and in
  the prose. Mentioning only `E0499` (the common case) is insufficient,
  because a reader who hits `E0502` (mixed mutability) needs to find the
  same advice from the same anchor. Cross-link the rustc canonical
  pages `https://doc.rust-lang.org/error_codes/E0499.html` and
  `https://doc.rust-lang.org/error_codes/E0502.html` once in the rationale
  paragraph so a reader who lands from a rustc diagnostic recognises the
  page.
- The rationale paragraph must be honest about *why* the wrapper triggers
  these errors at all: it is not the step body, it is the generated
  wrapper's two `ctx.borrow_mut(...)` calls on `&mut self`. Quoting or
  paraphrasing the §2.7.6.1 sentence is acceptable; inventing a new
  explanation is not.
- The entry must name three escape hatches in priority order: (1) when
  the second mutable fixture is harness context (the common GPUI
  shape), redirect to the stateful playbook; (2) when **both**
  parameters can be borrowed immutably (read-only inspection of both
  fixtures suffices), reshape both to `&T`. Reshaping only one
  parameter does **not** resolve the conflict — one `&T` plus one
  `&mut T` still produces `E0502` because `borrow_ref` holds `&ctx`
  while `borrow_mut` needs `&mut ctx`; (3) when neither of the above
  fits, split the step into two consecutive Gherkin steps. A reader
  must be able to pick an escape after one read, and must come away
  understanding that mixed mutability is **not** a fix.
- The Common errors and fixes section is the legacy entry-point for
  rustc-error symptoms in this guide. Add a short bullet pair there
  ("Error: ..." / "Fix: see the [Two mutable fixtures…] subsection")
  rather than duplicating the rationale. The bullet must use the exact
  rustc message strings from the canonical error index so a reader
  pattern-matches by string search.
- Update `docs/v0-6-0-migration-guide.md` only. Do not edit
  `docs/users-guide.md`, the design document, or `docs/known-issues.md`
  unless reflecting the design document accurately requires a small
  cross-reference patch (for example, adding a stub anchor target in
  the design document if the existing §2.7.6.1 heading does not provide
  one). Any such patch must be a strict cross-link addition; rationale
  edits in the design document are out of scope and would trip the
  scope tolerance.
- All prose must use British English with Oxford spelling
  (`en-GB-oxendict`), in line with `docs/documentation-style-guide.md`.
  Treat identifiers and external proper nouns (`color`, `LICENSE`,
  library names, rustc error codes) as exceptions where applicable.
- Cross-link the troubleshooting subsection from `docs/contents.md` only
  if the existing `v0-6-0-migration-guide.md` index entry undersells
  troubleshooting coverage. If the existing entry remains the canonical
  index, prefer an in-document anchor over expanding the index.
- Update `docs/CHANGELOG.md` only if the user-visible documentation
  surface changes substantively. A one-line `## Unreleased` bullet noting
  the new troubleshooting entry is sufficient.
- Do not add new dependencies, new feature flags, or new test crates.
  Any validation must reuse the existing doctest pipeline (`make test`
  via `cargo test --doc`) and `make markdownlint`.
- Run formatting, linting, and tests sequentially. Capture each command's
  output with `tee` to
  `/tmp/<action>-rstest-bdd-${BRANCH}.out` where `${BRANCH}` is
  `10-2-2-e0499-e0502-troubleshooting-guide`, in line with the agent
  instructions in `/home/leynos/.claude/CLAUDE.md`.
- Use `coderabbit review --agent` after each substantive documentation
  milestone. CodeRabbit concerns must be cleared or explicitly recorded
  before moving on. Do not request a CodeRabbit pass while deterministic
  gates fail.
- Use the relevant skills: `execplans` for this living plan, `leta` for
  code and symbol navigation, `rust-router` for routing into
  Rust-specific concerns if a snippet starts to look like a redesign,
  `en-gb-oxendict` for prose, `df12-copy` for tone where the entry
  needs to flow as prose rather than reference material,
  `compressed-authority` for the rationale paragraph, `commit-message`
  for each commit, and `pr-creation` when the draft PR is updated. Load
  `firecrawl` only if a citation gap appears beyond the citations
  captured in Surprises & discoveries.
- Do not mark roadmap item 10.2.2 done in `docs/roadmap.md` until the
  documentation has shipped, the quality gates have passed, and the
  community-of-experts revision has been reflected into the merged
  plan.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation requires touching more than
  three documentation files (`docs/v0-6-0-migration-guide.md`,
  `docs/CHANGELOG.md`, and at most one of `docs/contents.md`,
  `docs/known-issues.md`, or `docs/rstest-bdd-design.md` for a strict
  cross-link addition). A fourth file in the diff is a scope signal that
  the entry is doing more than troubleshooting.
- Lines: stop and escalate if the troubleshooting subsection grows beyond
  roughly 140 net Markdown lines (excluding fenced code), or if the
  bullet pair in `## Common errors and fixes` grows beyond two lines.
  The constraint is readability, not line-counting; treat the threshold
  as an early warning, not a hard budget.
- Code shape: stop and escalate if a `rust,no_run` doctest requires more
  than one `# use ...` hidden import per snippet beyond what the
  user-guide playbook already establishes. Repeated hidden imports are
  a signal that the entry is teaching a new pattern rather than
  redirecting to the existing one.
- Iterations: stop and escalate if `make markdownlint` still fails after
  three focused fix attempts, or if `make test` fails after the
  documentation edits alone for three focused fix attempts. Repeated
  failures point to a documentation-implementation mismatch, not a typo.
- Time: stop and escalate if a single documentation milestone (drafting
  the troubleshooting subsection, wiring the cross-references,
  validating the doctests, or reflecting CodeRabbit feedback) takes
  more than three working hours of agent time.
- Interface: stop and escalate if reflecting the design document
  accurately would require a non-additive change to the public surface,
  including `StepContext`, the generated-wrapper contract, the reserved
  harness-context fixture key, or any first-party adapter.
- Ambiguity: stop and escalate if the user-supplied roadmap text and the
  design document disagree on the entry scope, or if the
  community-of-experts review surfaces a contradiction the agent cannot
  resolve from the design document alone. Present the alternatives and
  request direction.

## Risks

- Risk: the troubleshooting entry teaches the thread-local workaround as
  a first-choice pattern rather than an interim shape, even though
  §2.7.6.5 calls it a v0.6-only workaround.
  Severity: medium. Likelihood: medium.
  Mitigation: open the subsection with the same v0.6-interim callout
  shape used by 10.2.1's playbook (a set-off block-quoted note that names
  §2.7.6.5 and roadmap items 12.1.x as the redesign target). Lead with
  the redirect to the GPUI playbook and the `&T` reshape, and put the
  thread-local pattern under the playbook link rather than restating it
  inline.
- Risk: the entry shows a *minimal failing example* that compiles
  successfully on a future toolchain (for example, because the generated
  wrapper learns to split borrows), and downstream readers conclude the
  workaround is unnecessary.
  Severity: low. Likelihood: low.
  Mitigation: tag the failing snippet `rust,compile_fail,ignore` and pin
  the trigger to "the generated wrapper code" rather than "Rust itself".
  A toolchain or codegen change that resolves the conflict will then
  legitimately turn the doctest into a no-op rather than a false claim.
- Risk: a reader hits `E0502` (mixed mutability), searches the migration
  guide for `E0502`, finds nothing, and concludes the framework's
  documentation does not cover their case.
  Severity: medium. Likelihood: medium.
  Mitigation: put both codes in the subsection heading and in the
  rationale paragraph, and add the exact rustc message strings ("cannot
  borrow `*ctx` as mutable more than once at a time", "cannot borrow
  `*ctx` as mutable because it is also borrowed as immutable") to the
  `Common errors and fixes` bullet pair so string search hits.
- Risk: the rationale paragraph implies that `StepContext::borrow_mut`
  is buggy or about to be removed before the v0.7.0 redesign, breaking
  downstream confidence in the v0.6 contract.
  Severity: low. Likelihood: low.
  Mitigation: state plainly that the v0.6 contract is the supported
  shape, name the constraint as a design limitation rather than a bug,
  and cite ADR-007 plus §2.7.6.1 for the rationale.
- Risk: markdownlint trips on the nested fenced blocks, the long error
  strings, or the rustc-error-code links.
  Severity: low. Likelihood: medium.
  Mitigation: rehearse formatting against `make markdownlint` after the
  first pass, before structural revisions. Keep error strings to a
  single Markdown line each and use inline code, not block quotes.
- Risk: the entry duplicates content with the user-guide playbook
  (already shipped under 10.2.1) and the two drift over time.
  Severity: medium. Likelihood: medium.
  Mitigation: keep the migration-guide entry strictly about *recognising
  the symptom and choosing an escape*. The worked-out playbook lives in
  the user guide; the migration-guide entry links to it by anchor and
  does not reproduce the snippet.
- Risk: the failing-shape snippet's parameter names drift from the
  §2.7.6.1 schematic, so readers cross-referencing the two see different
  variable names.
  Severity: low. Likelihood: medium.
  Mitigation: lift `cx: &mut gpui::TestAppContext` and `world: &mut UiWorld`
  from §2.7.6.1 (lines 1929–1939) verbatim. If §2.7.6.1's schematic
  changes underfoot, re-run Stage A before editing.

## Progress

- [ ] Stage A: outline sign-off (gate, not edits). Acceptance: the
  outline below is approved by the user; the community-of-experts review
  is reflected in Surprises & discoveries; no documentation edits yet.
- [ ] Stage B: draft the troubleshooting subsection in
  `docs/v0-6-0-migration-guide.md`. Acceptance: the subsection compiles
  as a doctest (failing snippet tagged `rust,compile_fail,ignore`,
  recommended snippets tagged `rust,no_run`), and `make markdownlint`
  plus `make test` exit zero.
- [ ] Stage C: wire cross-references. Acceptance: the
  `## Common errors and fixes` bullet pair is added, the
  `users-guide-playbook` link reference is reused where appropriate, and
  any strict cross-link addition to the design document is recorded in
  the Decision log.
- [ ] Stage D: changelog and index. Acceptance: `docs/CHANGELOG.md` has
  a one-line `## Unreleased` bullet noting the troubleshooting entry,
  and `docs/contents.md` is updated only if the existing entry does not
  surface troubleshooting coverage.
- [ ] Stage E: validation gates (`make check-fmt`, `make lint`,
  `make test`, `make markdownlint`) and CodeRabbit review on the
  resulting commit. Acceptance: all gates exit zero sequentially;
  CodeRabbit returns no unresolved concerns.
- [ ] Stage F: roadmap close-out. Mark `docs/roadmap.md` item 10.2.2 as
  done with a delivery date and one-sentence summary; finalise
  Outcomes & retrospective in this plan.

Use timestamps when the plan transitions to APPROVED status; the DRAFT
status intentionally carries no per-step timestamps until implementation
begins.

## Surprises & discoveries

- Observation: the existing `## Common errors and fixes` section in
  `docs/v0-6-0-migration-guide.md` (lines 433–446) uses a terse
  one-bullet-per-symptom shape with no anchored subsections. The
  E0499/E0502 case is substantially longer than the surrounding
  entries because it has to name two codes, a rationale, and three
  escape hatches.
  Evidence: lines 435–446 of the migration guide; each bullet pair is
  two short lines.
  Impact: the troubleshooting entry should not be a sibling bullet in
  the existing terse list. It should be its own subsection (preserving
  the terse list as the symptom-search index) and the existing list
  should gain one bullet pair that points at the subsection by anchor.
- Observation: the failing-shape example in design §2.7.6.1
  (`docs/rstest-bdd-design.md` lines 1929–1939) was aligned with the
  10.2.1 playbook to use the `|_context|` closure form and the
  `Option`-returning `VisualTestContext::from_window`. The migration
  guide's failing snippet must therefore match the new shape, not the
  pre-10.2.1 `|_, cx|` closure.
  Evidence: ExecPlan `10-2-1-migration-guide-for-gpui-stateful-tests.md`
  lines 451–456 record the change.
  Impact: copy the failing-shape snippet from §2.7.6.1 verbatim rather
  than transcribing from memory.
- Observation: the generated step wrapper emits the borrow calls at
  `crates/rstest-bdd-macros/src/codegen/wrapper/arguments/fixtures.rs`
  lines 84–101 inside `gen_fixture_decl_inner`. The
  `BorrowKind::Mutable` arm produces `ctx.borrow_mut::<T>(...)`, and
  the wrapper invokes the function once per fixture parameter. Two
  `&mut T` parameters therefore become two sequential `ctx.borrow_mut`
  calls before either guard is dropped.
  Evidence: file lines 69–102; `StepContext::borrow_mut` at
  `crates/rstest-bdd/src/context/mod.rs` lines 180–188 takes
  `&'b mut self`.
  Impact: the rationale paragraph can be precise about the two calls
  without leaking macro-implementation detail. The reader does not need
  to read the macro source; the entry just needs to name "two
  `ctx.borrow_mut(...)` calls in the generated wrapper" so the rustc
  error annotation matches.
- Observation: the official rustc error pages
  (`https://doc.rust-lang.org/error_codes/E0499.html` and
  `https://doc.rust-lang.org/error_codes/E0502.html`) do not recommend
  `RefCell` or interior mutability; their fix advice is structural
  ("ensure that you don't have any other references to the variable
  before trying to access it with a different mutability"). This
  matches the migration guide's recommended escapes: redirect to the
  playbook (which uses a thread-local `RefCell` *outside* `StepContext`,
  not inside it), reshape one parameter to `&T`, or split the step.
  Evidence: Firecrawl research pass dated 2026-06-04.
  Impact: cite the rustc pages once in the rationale paragraph so
  readers who arrive via a rustc diagnostic recognise the page, but do
  not claim the rustc pages recommend the rstest-bdd workaround
  shape — they don't, and they shouldn't.
- Observation: cucumber-rs (the closest Rust BDD peer) sidesteps the
  problem by passing `&mut World` to every step. There is therefore no
  borrow-conflict pattern equivalent to two mutable fixtures, and the
  cucumber-rs book has no comparable troubleshooting entry.
  Evidence: Firecrawl research pass dated 2026-06-04 against
  `https://cucumber-rs.github.io/cucumber/main/`.
  Impact: the troubleshooting entry should not claim parity with
  cucumber-rs's docs on this topic; it is filling a gap the field
  doesn't otherwise address. The Decision log records this so a later
  reviewer does not ask for a cucumber-rs cross-reference.
- Observation: the Rustonomicon's "Splitting Borrows" chapter
  (`https://doc.rust-lang.org/nomicon/borrow-splitting.html`) is the
  canonical Rust-language reference for "borrow checker doesn't
  understand disjoint indices into a container". This is the
  language-level reason `StepContext`'s `HashMap`-backed lookup cannot
  produce two `&mut` guards even when the requests target different
  keys.
  Evidence: Firecrawl research pass dated 2026-06-04.
  Impact: a one-sentence cross-reference in the rationale paragraph
  ("the borrow checker does not understand `HashMap` indices as
  disjoint, even when the keys are different") earns the reader a
  language-level mental model without restating the Rustonomicon.
- Observation: tokio's *Shared state* tutorial and Bevy's `ParamSet`
  documentation both use the same shape we are proposing here: name the
  symptom in rustc terms, explain why the framework's API produces it,
  point at the recommended workaround, link to the deeper rationale.
  Evidence: Firecrawl research pass dated 2026-06-04 against
  `https://tokio.rs/tokio/tutorial/shared-state` and
  `https://docs.rs/bevy_ecs/0.13.0/bevy_ecs/system/struct.ParamSet.html`.
  Impact: the entry shape (symptom → why → escape hatches → link to
  rationale) has industry precedent, so the entry can be terse without
  inventing new structure. Cite as prior art in the Decision log, not
  in the user-facing entry.
- Observation: the Logisphere pre-implementation review (2026-06-04)
  returned a "Proceed with conditions" verdict and six concrete
  revisions. The most load-bearing of those, now reflected in this
  plan's constraints, Stage A outline, and Decision log, are:
  (Telefono — correctness, load-bearing) reshape-to-`&T` resolves
  `E0499` **only when both** parameters become `&T`; the prior
  draft implied that one `&T` plus one `&mut T` was a fix, which is
  false because `borrow_ref` holds `&ctx` while `borrow_mut` needs
  `&mut ctx`, so mixed mutability is the symptom (`E0502`) rather
  than the cure. (Doggylump — pre-mortem) the "split the step"
  escape must explicitly warn that an ad-hoc shared mutable on the
  side reproduces the same conflict inside one of the new steps,
  and route the reader back to the playbook redirect in that case.
  (Pandalump — structural placement) the proposed `## Troubleshooting`
  umbrella is demoted: the new subsection lives **inside**
  `## Common errors and fixes` as a level-3 entry, with the bullet
  pair added at the head of the existing list, so there is one
  troubleshooting anchor namespace rather than two. (Buzzy Bee —
  drift) the failing snippet is tagged `rust,ignore` rather than
  `rust,compile_fail,ignore` to avoid spurious CI red on a future
  codegen split; compile-time drift detection becomes a deferred
  11.x trybuild fixture under Outcomes. (Dinolump — anti-cargo-cult)
  the redirect escape carries an explicit "do not adopt for
  single-mutable scenarios" warning, in addition to the v0.6-interim
  callout. (Wafflecat — alternative) `#[diagnostic::on_unimplemented]`
  on a sealed marker trait emitted by the wrapper is recorded as
  the strongest alternative and deferred to 11.x. The Buzzy Bee
  improvement to show a non-GPUI failing snippet alongside the
  GPUI one is also absorbed; the Pandalump anchor-stub-upfront note
  is moved from Stage C into Stage B.
  Evidence: Logisphere review transcript dated 2026-06-04 in the
  conversation that produced this revision.
  Impact: Stage A now records the corrected escape ordering, the
  collapsed section structure, the `rust,ignore` choice, the
  Wafflecat deferral, and the inline anti-cargo-cult warning; Stage
  B and C steps were adjusted to match.

## Decision log

- Decision: scope the entry to documentation only; do not refactor the
  macro codegen, `StepContext`, or any harness as part of 10.2.2.
  Rationale: the design document already pins the v0.6 contract and the
  v0.7.0 redesign at §2.7.6.5; any codegen or context-API change would
  re-scope the work into 11.x or 12.x and would create a v0.6.0 final
  surface that does not match the design document.
  Date/author: 2026-06-04 (drafting agent).
- Decision: place the troubleshooting subsection as a level-3 entry
  **inside** the existing `## Common errors and fixes` section,
  immediately after the four current bullet pairs, rather than under
  a new `## Troubleshooting` umbrella. The bullet pair is added at
  the head of the existing bullet list, pointing at the new
  subsection below by anchor.
  Rationale: a sibling top-level umbrella would overlap the existing
  troubleshooting section, split reader search ranking, and offer
  two near-equivalent landing points to skim readers. Keeping a
  single section preserves one anchor namespace and lets the terse
  bullets act as the symptom-search index for their own long-form
  subsection.
  Date/author: 2026-06-04 (drafting agent, revised after Logisphere
  review).
- Decision: tag the failing-shape snippet `rust,ignore` rather than
  `rust,compile_fail,ignore`, and carry the intent in a one-line
  caption immediately above the fence.
  Rationale: `compile_fail` would assert a failure mode that depends
  on the v0.6 generated wrapper. A future codegen change or rustc
  edition shift that legitimately resolves the conflict would flip
  the doctest into a spurious "expected compile failure didn't fail"
  CI red. `rust,ignore` decays gracefully into a stale-but-quiet
  snippet, and compile-time drift detection lives in a deferred
  trybuild follow-up (see Outcomes & retrospective).
  Date/author: 2026-06-04 (drafting agent, revised after Logisphere
  review).
- Decision: name the escape hatches in the order (1) redirect to the
  GPUI playbook when the second mutable is harness context, (2)
  reshape one parameter to `&T` when read-only access suffices, (3)
  split the step. Rationale: the most common shape that triggers the
  error in the field is the GPUI stateful one; the cheapest fix
  (reshape to `&T`) is second because it is a one-character source
  change; splitting the step is last because it requires reordering
  Gherkin.
  Date/author: 2026-06-04 (drafting agent).
- Decision: cite the rustc canonical error pages once each in the
  rationale paragraph; do not cite the cucumber-rs book.
  Rationale: rustc citations help readers who arrive from a compiler
  diagnostic recognise the page they are on; cucumber-rs's contract
  differs enough that a cross-reference invites a "why don't you do
  it this way" discussion in user issues that the v0.7.0 redesign
  will pre-empt.
  Date/author: 2026-06-04 (drafting agent).
- Decision: do not edit `docs/known-issues.md` as part of 10.2.2.
  Rationale: the file is reserved for *open* compiler bugs and
  unresolved upstream issues. The E0499/E0502 case is a deliberate
  design limitation with a documented v0.6 workaround and a planned
  v0.7.0 fix; the migration guide is the right home.
  Date/author: 2026-06-04 (drafting agent). Logisphere review did
  not overturn this decision.
- Decision: do **not** add a `## Migration checklist` item for the
  troubleshooting entry. Rationale: the checklist is for upgraders
  preparing a migration, not for first-time step authors. A reader
  lands on the troubleshooting subsection because a build already
  failed, so a checklist entry that says "do not write two mutable
  fixtures" duplicates the build signal without preventing the
  failure. Recorded so a later reviewer does not ask for the
  symmetry.
  Date/author: 2026-06-04 (drafting agent, Logisphere open question).
- Decision: an explicit "do not adopt the thread-local pattern for
  single-mutable scenarios" warning lives inline under the playbook
  redirect escape, in addition to the v0.6-interim callout.
  Rationale: the callout works for the playbook reader who follows
  the link, but a troubleshooting-entry skim reader needs a second
  guard rail at the point of copy-paste. Two anchors against
  cargo-culting cost one sentence; ergonomic regressions cost more.
  Date/author: 2026-06-04 (drafting agent, Dinolump revision).
- Decision: ship two failing-shape snippets, not one. The first is
  the §2.7.6.1 GPUI shape; the second is a non-GPUI shape
  (`&mut SqlPool` + `&mut World` or equivalent).
  Rationale: design §2.7.6.1 is explicit that the limitation is
  `StepContext`-wide, not GPUI-specific. A single GPUI-flavoured
  snippet reads as "GPUI-only" and risks under-serving non-GPUI
  adopters.
  Date/author: 2026-06-04 (drafting agent, Buzzy Bee improvement).
- Decision: the "Reshape both parameters to `&T`" escape is named
  with **both** in the heading and the rationale, and the rationale
  explicitly states that mixed mutability (`&T` plus `&mut T`) is
  not a fix — it remains the `E0502` case.
  Rationale: the prior draft implied that "one immutable plus one
  mutable" produced "a successful immutable + mutable split", which
  is false: `borrow_ref` holds `&ctx` while `borrow_mut` needs
  `&mut ctx`, so mixed mutability is the symptom rather than the
  fix. Naming this correctly is the load-bearing correctness change
  for the entire entry.
  Date/author: 2026-06-04 (drafting agent, Telefono revision).
- Decision: the "Split the step" escape carries an explicit
  guard-rail sentence — if both halves still need both fixtures
  mutably, the conflict resurfaces inside one of the new steps and
  the reader must use the playbook redirect.
  Rationale: a downstream reader who picks the split without this
  guard rail will write step A (`&mut cx`) and step B (`&mut world`)
  with an ad-hoc shared mutable on the side, then file a "world
  reset doesn't work" bug. The guard rail names the failure mode in
  advance.
  Date/author: 2026-06-04 (drafting agent, Doggylump revision).
- Decision: custom rustc diagnostics via
  `#[diagnostic::on_unimplemented]` on a sealed marker trait emitted
  by the wrapper, surfaced by Wafflecat as the strongest alternative
  during the Logisphere review, are deferred to the 11.x scope.
  Rationale: documentation here is the v0.6.0-final ceiling; the
  diagnostic is the v0.6.1 ceiling and requires codegen surgery
  inconsistent with the no-public-API-change constraint of this
  plan. The 10.2.2 entry will cross-link to the eventual
  diagnostic when 11.x lands.
  Date/author: 2026-06-04 (drafting agent, Wafflecat alternative).

## Outcomes & retrospective

To be completed at delivery. Record:

- the delivery date and the commit set that landed the change;
- the exact subsection heading and anchor that shipped;
- the doctest tag chosen for the failing snippets (planned: `rust,
  ignore`) and the reason for any deviation from the plan;
- the `make test` count delta versus the pre-change baseline, so a
  later maintainer can confirm no doctest fell off the run accidentally;
- a one-paragraph note on how the entry has held up after the first
  downstream consumer reads it (to be added in a later revision once
  beta feedback returns).

### Deferred follow-ups

These follow-ups are tracked here as the current source of record
until GitHub issues are filed; add the issue numbers alongside each
entry when they are opened, and close the loop in a later plan
revision.

1. **Compile-time drift detection via trybuild.** The Buzzy Bee
   revision noted that `rust,ignore` documents intent but does not
   assert the failure mode. A dedicated trybuild fixture under
   `crates/rstest-bdd-macros/tests/` that pins the two-mutable
   wrapper case and the mixed-mutability case to their rustc
   diagnostics would convert the documentation claim into a
   compile-time check. Deferred to 11.x because adding trybuild
   coverage is a development-time investment outside the
   v0.6.0-final documentation cut.
2. **Custom rustc diagnostic via `#[diagnostic::on_unimplemented]`.**
   The Wafflecat alternative, deferred to 11.x. A sealed marker
   trait emitted by the wrapper when it would produce two
   `borrow_mut` calls could surface a tailored rustc message that
   names the failure shape in user terms. When this lands, the
   10.2.2 troubleshooting subsection should gain a one-line
   cross-reference and the bullet pair in `## Common errors and
   fixes` should be updated with the new diagnostic message.
3. **First downstream review of the entry.** Capture the first
   downstream beta consumer's experience reading the new
   troubleshooting subsection — did they recognise the symptom from
   the rustc message, did they pick the right escape on the first
   read, and did they avoid cargo-culting the thread-local pattern
   into a single-mutable scenario? If any of those broke, revise the
   entry before the v0.6.0 final cut.

## Context and orientation

A reader picking this plan up cold should know the following:

- `rstest-bdd` is a Behaviour-Driven Development (BDD) test framework
  built on top of `rstest`. Each scenario is generated by a procedural
  macro and runs through a sequence of step functions. Step functions
  receive fixtures by name through a per-scenario `StepContext`.
- A "fixture" is the rstest concept of a parameter that the framework
  constructs for the test function. `rstest-bdd` extends this so that
  step functions can borrow fixtures from `StepContext` by name and
  type. The borrow is mediated by `StepContext::borrow_ref::<T>(name)`
  for `&T` and `StepContext::borrow_mut::<T>(name)` for `&mut T`.
- The reserved fixture key `rstest_bdd_harness_context` is how step
  functions request the harness-injected context (for example, the
  `gpui::TestAppContext` from `GpuiHarness` or the application context
  from a custom harness). The key is fixed; the parameter name on the
  receiving side is adapter-specific.
- The generated step-wrapper code lives at
  `crates/rstest-bdd-macros/src/codegen/wrapper/arguments/fixtures.rs`.
  Each fixture parameter becomes a `let mut guard = ctx.borrow_mut::<T>("name")?;`
  line (mutable case) or `let guard = ctx.borrow_ref::<T>("name")?;`
  (immutable case). The wrapper does not split or interleave the
  borrows; it emits them sequentially.
- `StepContext::borrow_mut` at
  `crates/rstest-bdd/src/context/mod.rs` lines 180–188 takes
  `&'b mut self` and returns a guard tied to `'b`. Two sequential
  calls hold the receiver borrow alive across both calls, which is
  what triggers `E0499` for two mutable fixtures and `E0502` for one
  mutable plus one immutable.
- The design document captures the constraint at
  `docs/rstest-bdd-design.md` §2.7.6.1, the interim workaround at
  §2.7.6.2, and the v0.7.0 redesign target at §2.7.6.5. ADR-007
  (`docs/adr-007-harness-context-injection.md`) records the harness
  context contract that produced the constraint.
- 10.2.1 (delivered 2026-06-04) added the stateful GPUI playbook to
  `docs/users-guide.md` ("Stateful GPUI scenarios with durable
  handles") and to `docs/v0-6-0-migration-guide.md` ("Migrate a
  stateful GPUI test"). The troubleshooting entry under 10.2.2 should
  link these by anchor rather than restate them.
- Style: `docs/documentation-style-guide.md` mandates British English
  with Oxford spelling, sentence-case headings, 80-column prose,
  120-column code, and the Oxford comma where it aids comprehension.
  The `en-gb-oxendict` skill enforces these.

## Plan of work

The plan is staged so each stage ends with a verifiable artefact. Move
to the next stage only when the previous stage's validation passes.

### Stage A: outline sign-off (gate, not edits)

Stage A is a gate, not an edit pass: it confirms the outline below is
the one implementation will follow. No documentation files change
during Stage A.

The agreed entry structure for `docs/v0-6-0-migration-guide.md` is:

1. **Keep `## Common errors and fixes` as the sole troubleshooting
   home.** Do not add a sibling `## Troubleshooting` umbrella; a
   second top-level section overlapping the existing one would split
   reader search and rank arbitrarily. The new long-form entry lives
   as a level-3 subsection inside `## Common errors and fixes`,
   positioned immediately after the existing terse bullet list. The
   bullet list keeps the symptom-search role; the new subsection
   carries the rationale and the escapes.
2. A `### Two mutable fixtures trigger E0499 or E0502` subsection
   inside `## Common errors and fixes`, immediately after the four
   existing terse bullet pairs. Subsection content, in order:

   a. A short v0.6-interim callout (block-quoted note matching the
      shape used by 10.2.1's playbook), naming
      `docs/rstest-bdd-design.md` §2.7.6.5 and roadmap items 12.1.x as
      the redesign target so a skim reader sees the temporariness.
   b. A "Symptoms" paragraph that quotes the rustc message strings for
      both codes inline, names the wrapper as the offending site
      (not the step body), and shows the failing-shape doctest tagged
      `rust,compile_fail,ignore`. The snippet uses the §2.7.6.1
      identifiers verbatim: `cx: &mut gpui::TestAppContext` and
      `world: &mut UiWorld`. A one-line caption explains the `E0502`
      case ("if either parameter is `&T`, the wrapper still cannot
      hold a shared and an exclusive borrow of `StepContext` at once,
      so the error code is `E0502` instead of `E0499`").
   c. A "Why this happens" paragraph that ties the two errors to the
      sequential `ctx.borrow_mut(...)` calls in the generated wrapper
      and the `&mut self` receiver on `StepContext::borrow_mut`. It
      cites design §2.7.6.1 for rationale, names ADR-007 for the
      contract, and includes the one-sentence Rustonomicon
      cross-reference about disjoint container indices. Inline link the
      rustc pages for both error codes once.
   d. A "Workarounds" paragraph that lists the three escape hatches in
      the agreed order. Each escape is one short paragraph:
        - **Redirect to the stateful GPUI playbook** when the second
          mutable fixture is the harness context. Link
          `users-guide.md#stateful-gpui-scenarios-with-durable-handles`
          and `#migrate-a-stateful-gpui-test`. State that the
          thread-local pattern keeps durable handles only and that the
          step then borrows only one fixture (`&mut TestAppContext`)
          from `StepContext`. Add one explicit warning sentence: **do
          not adopt this shape for scenarios that need only one
          mutable fixture** — the thread-local pattern is the v0.6
          workaround for the two-mutable case alone, and applying it
          unconditionally costs ergonomics for no borrow-checker
          benefit.
        - **Reshape both parameters to `&T`** when read-only access to
          both fixtures is sufficient. State that this turns the
          wrapper into two `borrow_ref` calls, both holding `&ctx`,
          which the borrow checker accepts. Add a one-line caution:
          reshaping only one parameter does **not** resolve the
          conflict — one `&T` plus one `&mut T` is the mixed case,
          producing `E0502` because `borrow_ref` holds a `&ctx`
          guard while `borrow_mut` needs `&mut ctx`. If only one
          parameter can be made immutable, fall through to the next
          escape.
        - **Split the step** when neither escape fits, by writing two
          consecutive Gherkin steps that **each touch one fixture
          only** and pass state between them through ordinary
          `rstest` fixtures (a shared mutable world fixture, for
          example). Add an explicit guard rail: if both halves still
          need both fixtures mutably — for example, if the second
          step still has to mutate the harness context **and** the
          world — splitting does not resolve the conflict; the same
          constraint resurfaces inside one of the new steps. In that
          case, use the playbook redirect. Show a one-line
          before/after Gherkin sketch so the structural shape is
          unambiguous.
   e. A "Where to read more" cross-reference list pointing at the
      design subsections (§2.7.6.1 for rationale, §2.7.6.2 for the
      interim pattern, §2.7.6.5 for the redesign), the user-guide
      playbook, the existing migration-guide "Migrate a stateful
      GPUI test" subsection, and ADR-007.
3. A new bullet pair at the **head** of the existing
   `## Common errors and fixes` bullet list (above the four current
   pairs), pointing skim-readers at the new subsection that follows:
   - **Error:** `cannot borrow *ctx as mutable more than once at a
     time (E0499)` or `cannot borrow *ctx as mutable because it is
     also borrowed as immutable (E0502)` in a generated step wrapper.
   - **Fix:** See the *Two mutable fixtures trigger E0499 or E0502*
     subsection below (anchor
     `#two-mutable-fixtures-trigger-e0499-or-e0502`).
4. No edit to `## Migration checklist`. The 10.2.1 checklist item
   already covers the reset protocol for stateful GPUI; the
   troubleshooting entry does not add a new checklist obligation
   because the user is reading the entry *because* a check failed.

Acceptance: the outline above is recorded under "Plan of work", and the
community-of-experts review has signed off. No documentation edits
yet. The review summary lives under "Surprises & discoveries"; any
later edits to the outline must update both locations together.

### Stage B: troubleshooting subsection (edits)

In `docs/v0-6-0-migration-guide.md`, insert the `### Two mutable
fixtures trigger E0499 or E0502` subsection inside the existing
`## Common errors and fixes` section, immediately after the four
existing bullet pairs. The replacement content must:

- Cite the design subsection paths (`docs/rstest-bdd-design.md`
  §2.7.6.1, §2.7.6.2, §2.7.6.5), ADR-007, and the rustc canonical
  error pages exactly once each.
- Use the identifiers `cx: &mut gpui::TestAppContext` and
  `world: &mut UiWorld` from the §2.7.6.1 schematic in the failing
  snippet. Hidden imports must be limited to one `# use` per snippet
  beyond what the user-guide playbook already establishes.
- Tag the failing snippet `rust,ignore`, with a one-line caption
  directly above the fence: "This snippet is intentionally rejected
  by the v0.6 generated wrapper; see *Why this happens* below." Tag
  any recommended-shape snippets `rust,no_run`.
- Show **two** failing-shape snippets, not one. The first uses the
  `gpui::TestAppContext` + `UiWorld` shape lifted from design
  §2.7.6.1; the second uses a non-GPUI shape such as
  `&mut SqlPool` + `&mut World` (or an equivalent that does not pull
  in a GPUI-specific identifier). The aim is to keep readers from
  reading the entry as "GPUI-only"; design §2.7.6.1 is explicit that
  the limitation is `StepContext`-wide, not GPUI-specific.
- In the same edit pass, insert the `<a id>` anchor stubs the
  Stage A outline calls for, immediately above the new subsection
  heading and above any cross-link target whose dotted/numeric form
  may be stripped by the renderer. Do not wait for Stage C to
  discover the anchor problem.
- End the subsection with the "Where to read more" cross-reference
  list anchored consistently with the rest of the migration guide.
- Reuse the existing link reference definition
  `[users-guide-playbook]: users-guide.md#stateful-gpui-scenarios-with-durable-handles`
  (already at lines 406–407) for the playbook link, so the document
  has one source of truth.

Acceptance: `make markdownlint` exits zero. `make test` exits zero,
including any doctests the entry adds. The failing snippet does not
appear in the doctest counts (because `compile_fail,ignore`).

### Stage C: cross-references (edits)

In the same file, add the bullet pair at the **head** of the
`## Common errors and fixes` bullet list per the Stage A outline. The
bullet pair uses the exact rustc message strings as inline code.

If, while drafting Stage B, the design document's §2.7.6.1 heading
does not produce a clickable anchor (Markdown renderers sometimes
strip dots and numbers from anchor IDs), add a `<a
id="borrow-constraint-exposed-by-gpui-adoption"></a>` stub immediately
above the §2.7.6.1 heading in `docs/rstest-bdd-design.md` and link to
that stub instead. Record any such addition in the Decision log. Do
not edit the §2.7.6.1 rationale itself; this is strictly a
cross-link addition.

Acceptance: `make markdownlint` exits zero. The cross-links resolve
when previewed with the same Markdown renderer that powers
`docs/contents.md`.

### Stage D: changelog and index (edits)

Add a single `## Unreleased` bullet in `docs/CHANGELOG.md`:
"Documentation: v0.6.0 migration guide gained a troubleshooting entry
for `E0499`/`E0502` from two mutable `StepContext` fixtures, with
workarounds and a cross-link to the stateful GPUI playbook." (Subject
to Stage A community review.)

Refresh `docs/contents.md` only if the existing
`v0-6-0-migration-guide.md` entry does not surface troubleshooting
coverage. Otherwise leave the index alone.

Acceptance: `make markdownlint` exits zero. The changelog entry
follows the existing tone in `docs/CHANGELOG.md`.

### Stage E: validation and CodeRabbit review

Run the quality gates sequentially, capturing each command with `tee`:

```bash
make check-fmt    2>&1 | tee /tmp/check-fmt-rstest-bdd-${BRANCH}.out
make lint         2>&1 | tee /tmp/lint-rstest-bdd-${BRANCH}.out
make test         2>&1 | tee /tmp/test-rstest-bdd-${BRANCH}.out
make markdownlint 2>&1 | tee /tmp/markdownlint-rstest-bdd-${BRANCH}.out
```

Replace `${BRANCH}` with the branch name
`10-2-2-e0499-e0502-troubleshooting-guide`. Each command must exit
zero before the next runs.

Run `coderabbit review --agent` on the resulting commit. Address every
non-cosmetic concern in place, or record the deferral in the Decision
log with rationale. Do not run CodeRabbit until the deterministic
gates above pass.

### Stage F: roadmap close-out

Mark `docs/roadmap.md` item 10.2.2 as done, including a delivery date
and a one-sentence summary, only after Stage E passes and the
documentation edits are committed. Update `Outcomes & retrospective`
in this plan in the same commit.

## Concrete steps

The exact command sequence inside the working tree at
`/home/leynos/.lody/repos/github---leynos---rstest-bdd/worktrees/26ed0e2a-6f68-4d83-8fb8-187c63ad40a4`:

1. `git branch --show-current` to confirm the branch is
   `10-2-2-e0499-e0502-troubleshooting-guide` before any commit. If
   the branch is still `feat/plan-e0499-e0502-troubleshooting` (the
   pre-approval planning branch), rename it locally with
   `git branch -m 10-2-2-e0499-e0502-troubleshooting-guide` before
   the first edit commit.
2. Edit `docs/v0-6-0-migration-guide.md` per Stage B.
3. Edit `docs/v0-6-0-migration-guide.md` per Stage C, and optionally
   `docs/rstest-bdd-design.md` for a strict cross-link anchor stub.
4. Edit `docs/CHANGELOG.md` and (only as needed) `docs/contents.md`
   per Stage D.
5. Run the four quality gates per Stage E.
6. `coderabbit review --agent` and resolve concerns.
7. Mark roadmap item 10.2.2 done per Stage F and amend
   `Outcomes & retrospective` in this plan.

Each commit message is produced with the `commit-message` skill and
applied via `git commit -F`. Each commit ends with the standard
`Co-Authored-By` trailer.

## Validation and acceptance

Acceptance is observable when:

- `docs/v0-6-0-migration-guide.md` contains the `### Two mutable
  fixtures trigger E0499 or E0502` subsection inside the existing
  `## Common errors and fixes` section. The failing snippets are
  tagged `rust,ignore`, carry the warning caption above the fence,
  and use the identifiers from design §2.7.6.1 plus a second
  non-GPUI shape.
- `## Common errors and fixes` gains a bullet pair at the head of its
  bullet list whose **Error:** line contains the exact rustc message
  strings for `E0499` and `E0502`, and whose **Fix:** line names the
  new subsection by anchor.
- `make check-fmt`, `make lint`, `make test`, and `make markdownlint`
  exit zero in sequence.
- `coderabbit review --agent` returns no unresolved concerns on the
  final commit.
- `docs/roadmap.md` item 10.2.2 is checked off, and
  `docs/CHANGELOG.md` contains a one-line entry for the troubleshooting
  entry in `## Unreleased`.

Quality criteria for "done":

- Tests: `make test` passes; any doctest changes in the migration
  guide succeed under `cargo test --doc`. The failing snippets do
  not contribute to the doctest count (because tagged `rust,ignore`).
- Lint: `make lint` passes; `make markdownlint` passes;
  `make check-fmt` passes.
- Performance: not applicable; the change is documentation-only.
- Security: not applicable; no new dependencies, no new code paths.

Quality method:

- Sequential local runs of the four gate commands above, with logs
  in `/tmp/`.
- `coderabbit review --agent` on the final commit.
- The community-of-experts review captured in this plan is reflected
  back into the final draft before the PR moves from DRAFT to READY.

## Idempotence and recovery

All steps are re-runnable. The documentation edits are idempotent
because each edit replaces a known fragment with a known fragment. If
a gate fails:

- `make markdownlint` failure: re-run after fixing the warnings shown
  in `/tmp/markdownlint-rstest-bdd-${BRANCH}.out`. Do not edit
  `.markdownlint` rules to silence findings; fix the prose instead.
- `make test` failure on a doctest: read the failing doctest line from
  the log, narrow the snippet to the smallest shape that compiles
  under `rust,no_run`, and re-run. If the failing-snippet's
  `compile_fail,ignore` tag becomes brittle (for example, under
  a future `cargo test --doc` change), reverse the Decision log entry
  to a plain `rust,ignore` and document why.
- `coderabbit review --agent` failure: address each concern in place;
  if a concern cannot be addressed without violating a constraint,
  stop and escalate.

If the design document or the 10.2.1 user-guide playbook changes
underfoot while the entry is being written, re-run the investigation
in Stage A before continuing.

## Artifacts and notes

The minimum artefacts to keep after delivery are:

- The diff for `docs/v0-6-0-migration-guide.md`,
  `docs/CHANGELOG.md`, and any auxiliary file touched (the design
  document if a cross-link anchor stub was needed; `docs/contents.md`
  if the index was refreshed).
- The four log files in `/tmp/` from the validation stage.
- The CodeRabbit summary for the final commit.

## Interfaces and dependencies

This plan introduces no new public Rust interfaces. The dependencies
referenced in the entry are unchanged:

- `rstest_bdd::StepContext::borrow_mut` and
  `rstest_bdd::StepContext::borrow_ref` for the borrow shapes.
- `rstest_bdd_harness::HarnessAdapter` for the harness contract that
  produces the `rstest_bdd_harness_context` fixture key.
- `rstest_bdd_harness_gpui::GpuiHarness` and the upstream
  `gpui::TestAppContext` type for the failing-snippet example.
- The reserved fixture key `rstest_bdd_harness_context`, requested by
  step functions as
  `#[from(rstest_bdd_harness_context)] cx: &mut gpui::TestAppContext`.
- The `serial_test` crate, named in the user-guide playbook the entry
  links to (no direct citation in the troubleshooting entry).

The entry must reference the following file paths verbatim so a reader
can read the supporting evidence locally:

- `docs/rstest-bdd-design.md` §§2.7.6.1, §2.7.6.2, §2.7.6.5.
- `docs/v0-6-0-migration-guide.md` "Migrate a stateful GPUI test".
- `docs/users-guide.md` "Stateful GPUI scenarios with durable handles".
- `docs/adr-007-harness-context-injection.md`.

## Revision note

Initial DRAFT authored 2026-06-04 by the drafting agent following an
Explore-agent inventory of the existing references and a Firecrawl
research pass for external prior art. Revised the same day to absorb
a Logisphere pre-implementation design review that returned a
"Proceed with conditions" verdict. The revision: corrected the
reshape-to-`&T` escape so the mixed-mutability `E0502` case is named
honestly (Telefono); added a guard-rail sentence to the split-the-
step escape so an ad-hoc shared mutable does not reproduce the
conflict inside one of the new steps (Doggylump); collapsed the
proposed `## Troubleshooting` umbrella into a level-3 subsection of
the existing `## Common errors and fixes` (Pandalump); switched the
failing-snippet tag from `compile_fail,ignore` to `rust,ignore` with
an above-fence caption, and promised a trybuild follow-up for
compile-time drift detection (Buzzy Bee); added an inline anti-
cargo-cult warning under the playbook-redirect escape (Dinolump);
recorded `#[diagnostic::on_unimplemented]` as the strongest
alternative, deferred to 11.x (Wafflecat); and absorbed the
non-GPUI failing-snippet improvement plus the anchor-stubs-upfront
note (Buzzy Bee improvement, Pandalump green). Pending: user
approval before the plan moves from DRAFT to APPROVED. Any later
edit must update the Status field at the top of the plan, append a
brief note to this section, and keep the living sections current.
