# Scenario outline example column checking

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document follows the ExecPlans skill template.

## Purpose / Big Picture

After this change, the `rstest-bdd-server` language server will emit on-save
diagnostics that catch mismatches between Scenario Outline placeholders
(`<column>`) and Examples table column headers. Users will see warnings in
their editor when:

1. **Missing column**: A step in a Scenario Outline uses a placeholder
   (e.g., `<count>`) that has no matching column header in the Examples table.

2. **Surplus column**: An Examples table includes a column header that is not
   referenced by any `<placeholder>` in the Scenario Outline's steps.

Observable outcomes:

- Running `rstest-bdd-lsp` against a workspace with mismatched scenario outlines
  produces diagnostics at precise byte offsets in feature files.
- Users receive immediate feedback in their editor (VS Code, Neovim, etc.) via
  the Language Server Protocol (LSP) diagnostics without waiting for
  compilation.
- All existing tests continue to pass (`make test`), and the new diagnostics are
  validated by unit tests and behavioural tests in `rstest-bdd-server`.
- The feature is documented in `docs/users-guide.md` and
  `docs/rstest-bdd-language-server-design.md`.
- The corresponding roadmap entry is marked complete.

## Constraints

Hard invariants that must hold throughout implementation:

- **Diagnostics on feature files only**: The mismatch is a Gherkin structural
  issue. Reporting on the feature file provides the most actionable feedback.
- **Use existing indexing infrastructure**: Extend the current
  `FeatureFileIndex` and related structures rather than building parallel
  systems.
- **Single source of truth for placeholders**: Use a regex consistent with the
  macros crate's `PLACEHOLDER_RE` pattern (`<([^>\s][^>]*)>`).
- **Preserve existing diagnostic behaviour**: The current diagnostics
  (unimplemented steps, unused definitions, placeholder count, table/docstring)
  must continue to work unchanged.
- **Do not introduce new external dependencies**: The implementation must use
  crates already in the workspace.
- **File length limit**: No single file may exceed 400 lines. Extract helper
  modules if the implementation approaches this limit.
- **Quality gates**: `make check-fmt`, `make lint`, and `make test` must all
  pass before any commit.
- **Module-level doc comments**: Every new module must have a `//!` doc comment.
- **Public API documentation**: Every new public function/struct must have `///`
  rustdoc comments.

## Tolerances (Exception Triggers)

Thresholds that trigger escalation when breached:

- **Scope**: If implementation requires changes to more than 10 files or 800 net
  lines of code, stop and escalate.
- **Interface**: If a public API signature in existing crates must change, stop
  and escalate (new APIs are acceptable).
- **Dependencies**: If a new external crate dependency is required, stop and
  escalate.
- **Iterations**: If tests still fail after 3 debugging attempts on the same
  issue, stop and escalate.

## Risks

Known uncertainties that might affect the plan:

- Risk: The current `FeatureFileIndex.example_columns` is a flat list without
  association to specific scenario outlines. Extending the indexing model may
  require careful design. Severity: medium Likelihood: high Mitigation: Add
  `IndexedScenarioOutline` and `IndexedExamplesTable` structs to capture the
  hierarchical relationship.

- Risk: Steps within scenario outlines contain raw `<placeholder>` text that
  must be parsed at diagnostic time. Severity: low Likelihood: low Mitigation:
  Use a regex consistent with the macros crate to extract placeholders.

- Risk: Multiple Examples tables in the same scenario outline require careful
  handling. Severity: low Likelihood: medium Mitigation: Validate each Examples
  table independently against the same set of step placeholders.

## Progress

- [x] Stage A: Research and design
  - [x] Understand existing feature indexing in `indexing/feature.rs`
  - [x] Understand existing diagnostic patterns in `handlers/diagnostics/`
  - [x] Design new data structures for scenario outline tracking
  - [x] Document decision on placeholder extraction approach

- [x] Stage B: Extend feature file indexing
  - [x] Add `IndexedScenarioOutline` and `IndexedExamplesTable` structs
  - [x] Extend `FeatureFileIndex` with `scenario_outlines` field
  - [x] Update `index_feature_text()` to populate scenario outlines

- [x] Stage C: Implement diagnostic computation
  - [x] Create `handlers/diagnostics/scenario_outline.rs` module
  - [x] Implement placeholder extraction from step text
  - [x] Implement `compute_scenario_outline_column_diagnostics()`
  - [x] Add diagnostic codes to `mod.rs`

- [x] Stage D: Wire diagnostics and add tests
  - [x] Wire into `publish_feature_diagnostics()`
  - [x] Add unit tests in `handlers/diagnostics/mod.rs`
  - [x] Create `tests/diagnostics_scenario_outline.rs` behavioural tests

- [x] Stage E: Documentation and cleanup
  - [x] Update `docs/users-guide.md`
  - [x] Update `docs/rstest-bdd-language-server-design.md`
  - [x] Mark `docs/roadmap.md` entry as done
  - [x] Run full quality gates
  - [x] Commit changes

## Surprises & Discoveries

1. **File length limit refactoring**: The `indexing/feature.rs` file exceeded
   the 400-line limit after adding scenario outline indexing. Solution:
   extracted scenario outline helper functions to a new
   `indexing/feature/outline.rs` submodule.

2. **Clippy `expect_used` lint**: The `expect()` call on regex compilation
   triggered a lint error. Solution: use `unwrap_or_else(|_| unreachable!())`
   since the regex is a compile-time constant.

3. **Test indexing assertions**: Direct array indexing in tests triggered
   `indexing_slicing` warnings. Solution: use `.first().expect()` and
   `.get(n).expect()` patterns instead.

## Decision Log

### 2026-01-21: Placeholder extraction approach

**Decision:** Use regex `<([^>\s][^>]*)>` to extract placeholders from scenario
outline step text, docstrings, and table cells.

**Rationale:** This regex is consistent with `PLACEHOLDER_RE` in the macros
crate (`crates/rstest-bdd-macros/src/parsing/placeholder.rs`). Using the same
pattern ensures the LSP and macros agree on what constitutes a valid
placeholder.

**Alternatives considered:**

1. Reuse the macros crate's regex directly — rejected due to circular dependency
   concerns and the fact that the regex is simple enough to duplicate.
2. Port the macros crate's placeholder module to a shared crate — rejected as
   over-engineering for a single regex.

### 2026-01-21: Diagnostic reporting location

**Decision:** Report all diagnostics on the feature file, not the Rust file.

**Rationale:** The mismatch is fundamentally a Gherkin structural issue:

- Missing columns mean the Examples table is incomplete
- Surplus columns mean the Examples table has unused data

Reporting on the feature file provides the most actionable feedback to users
editing their Gherkin specifications.

### 2026-01-21: Multiple Examples tables handling

**Decision:** Validate each Examples table independently against the scenario
outline's step placeholders.

**Rationale:** Each Examples table should be self-contained. A column is
"surplus" if that specific table has it but no step references it. A
placeholder is "missing" if that specific table lacks the column.

## Outcomes & Retrospective

**Completed:** 2026-01-21

**Summary:** Successfully implemented scenario outline example column
validation diagnostics for the rstest-bdd language server. The implementation:

- Added new data structures (`IndexedScenarioOutline`, `IndexedExamplesTable`)
- Extended feature file indexing to track scenario outlines and their Examples
  tables
- Created a new diagnostic module for column validation
- Added comprehensive unit tests (5 scenario outline tests in `mod.rs`) and
  behavioural tests (9 tests in `diagnostics_scenario_outline.rs`)
- Updated user documentation and design documentation
- Marked the roadmap entry as complete

**Files changed:** 14 files (10 modified, 4 new)

- New: `indexing/feature/outline.rs`,
  `handlers/diagnostics/scenario_outline.rs`,
  `tests/diagnostics_scenario_outline.rs`, `docs/execplans/7-4-3-*.md`

**Quality gates:** All passed (check-fmt, lint, test)

## Context and Orientation

This feature extends the `rstest-bdd-server` language server crate. The server
already provides:

- Navigation from Rust step definitions to feature steps (Go to Definition)
- Navigation from feature steps to Rust implementations (Go to Implementation)
- Diagnostics for unimplemented feature steps and unused step definitions
- Diagnostics for placeholder count mismatches
- Diagnostics for data table and docstring expectation mismatches

### Key Files

- `crates/rstest-bdd-server/src/indexing/mod.rs`: Data structures for indexed
  features and Rust files
- `crates/rstest-bdd-server/src/indexing/feature.rs`: Feature file indexing
  logic
- `crates/rstest-bdd-server/src/indexing/feature/table.rs`: Table/Examples
  header cell span extraction
- `crates/rstest-bdd-server/src/handlers/diagnostics/mod.rs`: Diagnostic
  constants and exports
- `crates/rstest-bdd-server/src/handlers/diagnostics/publish.rs`: LSP
  diagnostic publishing
- `crates/rstest-bdd-server/src/test_support.rs`: Test utilities
- `crates/rstest-bdd-server/tests/diagnostics_*.rs`: Behavioural tests

### Key Data Structures

`FeatureFileIndex` (current):

```rust
pub struct FeatureFileIndex {
    pub path: PathBuf,
    pub source: String,
    pub steps: Vec<IndexedStep>,
    pub example_columns: Vec<IndexedExampleColumn>,  // Flat list
}
```

`IndexedExampleColumn`:

```rust
pub struct IndexedExampleColumn {
    pub name: String,
    pub span: Span,  // Byte span of header cell content
}
```

`IndexedStep`:

```rust
pub struct IndexedStep {
    pub keyword: String,
    pub step_type: StepType,
    pub text: String,
    pub span: Span,
    pub docstring: Option<IndexedDocstring>,
    pub table: Option<IndexedTable>,
}
```

### New Data Structures (to be added)

`IndexedScenarioOutline`:

```rust
pub struct IndexedScenarioOutline {
    pub name: String,
    pub span: Span,
    pub step_indices: Vec<usize>,  // Indices into FeatureFileIndex.steps
    pub examples: Vec<IndexedExamplesTable>,
}
```

`IndexedExamplesTable`:

```rust
pub struct IndexedExamplesTable {
    pub span: Span,
    pub columns: Vec<IndexedExampleColumn>,
}
```

### Term Definitions

- **Scenario Outline placeholder**: A `<name>` token in a scenario outline step
  that is substituted with values from the Examples table at runtime.
- **Examples table column**: A header cell in an Examples table (e.g.,
  `| name |` becomes column "name").
- **Missing column**: A placeholder referenced in steps but absent from the
  Examples table headers.
- **Surplus column**: A column in the Examples table that no step placeholder
  references.

## Plan of Work

### Stage A: Research and Design (complete)

Review existing infrastructure and design the approach. No code changes.

### Stage B: Extend Feature File Indexing

1. In `indexing/mod.rs`, add new structs:

   ```rust
   /// An Examples table from a scenario outline.
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct IndexedExamplesTable {
       /// Byte span covering the Examples block.
       pub span: Span,
       /// Column headers with their spans.
       pub columns: Vec<IndexedExampleColumn>,
   }

   /// A scenario outline with its steps and example tables.
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct IndexedScenarioOutline {
       /// The scenario outline's name.
       pub name: String,
       /// Byte span covering the scenario outline block.
       pub span: Span,
       /// Indices into FeatureFileIndex.steps for steps in this outline.
       pub step_indices: Vec<usize>,
       /// Example tables belonging to this outline.
       pub examples: Vec<IndexedExamplesTable>,
   }
   ```

2. Extend `FeatureFileIndex`:

   ```rust
   pub struct FeatureFileIndex {
       // ... existing fields ...
       pub scenario_outlines: Vec<IndexedScenarioOutline>,
   }
   ```

3. Update `feature.rs` `index_feature_text()` to detect scenario outlines
   (by checking `scenario.keyword` for "Scenario Outline" or "Scenario
   Template") and populate the new structures.

### Stage C: Implement Diagnostic Computation

1. Create new file `handlers/diagnostics/scenario_outline.rs`:

   ```rust
   //! Scenario outline example column validation diagnostics.

   use std::collections::HashSet;
   use std::sync::LazyLock;
   use lsp_types::Diagnostic;
   use regex::Regex;
   use crate::indexing::FeatureFileIndex;

   static OUTLINE_PLACEHOLDER_RE: LazyLock<Regex> =
       LazyLock::new(|| Regex::new(r"<([^>\s][^>]*)>").expect("valid regex"));

   pub fn compute_scenario_outline_column_diagnostics(
       feature_index: &FeatureFileIndex,
   ) -> Vec<Diagnostic>
   ```

2. Implement helper functions:
   - `extract_outline_placeholders(text: &str) -> HashSet<String>`
   - `collect_placeholders_from_step(step: &IndexedStep) -> HashSet<String>`

3. Implement main algorithm:
   - For each `IndexedScenarioOutline`:
     - Collect all placeholders from steps (text, docstring, table cells)
     - For each `IndexedExamplesTable`:
       - Compute missing columns (placeholders - column names)
       - Compute surplus columns (column names - placeholders)
       - Emit diagnostics with appropriate spans

4. Add diagnostic codes in `mod.rs`:

   ```rust
   const CODE_EXAMPLE_COLUMN_MISSING: &str = "example-column-missing";
   const CODE_EXAMPLE_COLUMN_SURPLUS: &str = "example-column-surplus";
   ```

### Stage D: Wire Diagnostics and Add Tests

1. In `publish.rs`, update feature diagnostics to include the new computation:

   ```rust
   diagnostics.extend(compute_scenario_outline_column_diagnostics(feature_index));
   ```

2. Add unit tests in `handlers/diagnostics/mod.rs` tests module.

3. Create `tests/diagnostics_scenario_outline.rs` with parameterized test cases.

### Stage E: Documentation and Cleanup

1. Update `docs/users-guide.md` with new diagnostic descriptions.
2. Update `docs/rstest-bdd-language-server-design.md`.
3. Mark roadmap entry as done.
4. Run quality gates and commit.

## Concrete Steps

All commands are run from the repository root.

### Stage B Commands

```bash
# After implementing indexing changes:
cargo test -p rstest-bdd-server indexing
# Expected: all indexing tests pass
```

### Stage C Commands

```bash
# After implementing diagnostic computation:
cargo test -p rstest-bdd-server scenario_outline
# Expected: new tests pass
```

### Stage D Commands

```bash
# After adding behavioural tests:
cargo test -p rstest-bdd-server --test diagnostics_scenario_outline
# Expected: all behavioural tests pass
```

### Stage E Commands

```bash
# Full quality gate:
set -o pipefail && make check-fmt 2>&1 | tee /tmp/check-fmt.log
# Expected: exit 0

set -o pipefail && make lint 2>&1 | tee /tmp/lint.log
# Expected: exit 0

set -o pipefail && make test 2>&1 | tee /tmp/test.log
# Expected: exit 0, all tests pass
```

## Validation and Acceptance

Quality criteria:

- Tests: All existing tests pass. New unit tests cover scenario outline column
  validation. Behavioural tests verify end-to-end diagnostic publishing.
- Lint/typecheck: `make lint` passes with no warnings.
- Documentation: Users guide and design document updated.

Quality method:

- Run `make test` and verify all tests pass.
- Verify `make lint` produces clean output.
- Confirm `make check-fmt` exits without errors.

Acceptance behaviour:

1. Open a feature file with a Scenario Outline:

   ```gherkin
   Scenario Outline: test
     Given the system has <count> items
     Examples:
       | other |
       | 5     |
   ```

2. Save the file.
3. Observe diagnostic on the step:

   ```plaintext
   Placeholder '<count>' has no matching column in Examples table
   ```

4. Add the missing column and save:

   ```gherkin
     Examples:
       | count | other |
       | 5     | x     |
   ```

5. Observe diagnostic clears for the step.
6. Observe new diagnostic on the `other` column:

   ```plaintext
   Examples column 'other' is not referenced by any step placeholder
   ```

## Idempotence and Recovery

All stages are re-runnable. If a stage fails partway:

- Discard local changes with `git checkout .` and retry from the beginning of
  that stage.
- Unit tests are isolated and do not leave persistent state.
- The language server state is in-memory only; restarting it resets all indices.

## Artifacts and Notes

(Transcripts and key snippets will be added as work proceeds)

## Interfaces and Dependencies

**New Structs (rstest-bdd-server/indexing):**

```rust
pub struct IndexedExamplesTable {
    pub span: Span,
    pub columns: Vec<IndexedExampleColumn>,
}

pub struct IndexedScenarioOutline {
    pub name: String,
    pub span: Span,
    pub step_indices: Vec<usize>,
    pub examples: Vec<IndexedExamplesTable>,
}
```

**New Functions (rstest-bdd-server/handlers/diagnostics):**

```rust
pub fn compute_scenario_outline_column_diagnostics(
    feature_index: &FeatureFileIndex,
) -> Vec<Diagnostic>
```

**New Diagnostic Codes:**

```rust
CODE_EXAMPLE_COLUMN_MISSING = "example-column-missing"
CODE_EXAMPLE_COLUMN_SURPLUS = "example-column-surplus"
```
