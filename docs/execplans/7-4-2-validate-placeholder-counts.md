# Validate placeholder counts, typed placeholders, and data table/docstring expectations

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IN PROGRESS

This document follows the ExecPlans skill template.

## Purpose / Big Picture

After this change, the `rstest-bdd-server` language server will emit on-save
diagnostics that catch mismatches between Gherkin step patterns and Rust
function signatures. Users will see warnings in their editor when:

1. **Placeholder count mismatch:** A step pattern declares a different number of
   placeholders than the function has step arguments (excluding fixtures,
   datatable, and docstring parameters).

2. **Data table expectation mismatch:** A feature step provides a data table,
   but the Rust implementation does not expect one (or vice versa).

3. **Docstring expectation mismatch:** A feature step provides a docstring,
   but the Rust implementation does not expect one (or vice versa).

Observable outcomes:

- Running `rstest-bdd-lsp` against a workspace with mismatched step definitions
  produces diagnostics at precise byte offsets in both feature and Rust files.
- Users receive immediate feedback in their editor (VS Code, Neovim, etc.) via
  the Language Server Protocol (LSP) diagnostics without waiting for
  compilation.
- All existing tests continue to pass (`make test`), and the new diagnostics are
  validated by unit tests and behavioural tests in `rstest-bdd-server`.
- The feature is documented in `docs/users-guide.md` under a "Language Server
  Diagnostics" section.
- The corresponding roadmap entry is marked complete.

## Constraints

Hard invariants that must hold throughout implementation:

- **Do not modify macro crates:** `rstest-bdd-macros` and `rstest-bdd` crates
  must not be changed unless strictly necessary for a shared utility.
- **Single source of truth for placeholders:** All placeholder extraction must
  use `rstest_bdd_patterns::pattern::lexer::lex_pattern()` as the single source
  of truth. The LSP server must not duplicate placeholder parsing logic. This
  ensures consistency between the language server and the runtime macros.
- **Preserve existing diagnostic behaviour:** The current "unimplemented step"
  and "unused step definition" diagnostics must continue to work unchanged.
- **Do not introduce new external dependencies:** The implementation must use
  crates already in the workspace (e.g., `lsp-types`, `gherkin`, `syn`,
  `rstest-bdd-patterns`).
- **File length limit:** No single file may exceed 400 lines. Extract helper
  modules if the implementation approaches this limit.
- **Quality gates:** `make check-fmt`, `make lint`, and `make test` must all
  pass before any commit.
- **Module-level doc comments:** Every new module must have a `//!` doc comment.
- **Public API documentation:** Every new public function/struct must have `///`
  rustdoc comments.

## Tolerances (Exception Triggers)

Thresholds that trigger escalation when breached:

- **Scope:** If implementation requires changes to more than 8 files or 600 net
  lines of code, stop and escalate.
- **Interface:** If a public API signature in `rstest-bdd-patterns` must change,
  stop and escalate (new APIs are acceptable).
- **Dependencies:** If a new external crate dependency is required, stop and
  escalate.
- **Iterations:** If tests still fail after 3 debugging attempts on the same
  issue, stop and escalate.
- **Ambiguity:** If placeholder counting semantics differ between the macros and
  the LSP, document the discrepancy and escalate for resolution.

## Risks

Known uncertainties that might affect the plan:

- Risk: Placeholder extraction from `IndexedStepDefinition` parameters may not
  align perfectly with how the macros count step arguments. Severity: medium
  Likelihood: low Mitigation: Use the same classification logic: step arguments
  are parameters whose names appear in the pattern's placeholder set; fixtures,
  datatable, and docstring parameters are excluded from the count.

- Risk: Determining which parameters are "fixtures" vs. "step arguments" in the
  LSP context is non-trivial since the LSP does not have access to rstest
  fixture definitions. Severity: medium Likelihood: medium Mitigation: Use
  placeholder name matching. A parameter is a "step argument" if its normalized
  name matches a placeholder name in the pattern. Parameters named `datatable`
  or `docstring` (or with `#[datatable]` attribute) are special. All other
  parameters are assumed to be fixtures.

- Risk: Feature steps with data tables or docstrings may not perfectly align
  with the "expects_table" / "expects_docstring" flags from Rust indexing.
  Severity: low Likelihood: low Mitigation: The `IndexedStepDefinition` already
  captures `expects_table` and `expects_docstring`. The `IndexedStep` already
  captures `.table` and `.docstring` presence. Cross-reference these directly.

## Progress

- [x] Stage A: Design and prototyping (no code changes to main logic yet)
  - [x] Read and understand existing diagnostic computation in
        `handlers/diagnostics/compute.rs`
  - [x] Read and understand `SpecificityScore::calculate()` in
        `rstest-bdd-patterns/src/specificity.rs`
  - [x] Design the new diagnostic types and message formats
  - [x] Document decision on placeholder vs. fixture parameter classification

- [x] Stage B: Implement placeholder count validation
  - [x] Add new diagnostic code constant(s) to `handlers/diagnostics/mod.rs`
  - [x] Add placeholder extraction helper in `handlers/diagnostics/compute.rs`
  - [x] Implement `compute_signature_mismatch_diagnostics()` for Rust files
  - [x] Add unit tests for placeholder count validation

- [x] Stage C: Implement data table and docstring expectation validation
  - [x] Implement `compute_table_docstring_mismatch_diagnostics()` for feature
        files (combined function)
  - [x] Add unit tests for table/docstring validation

- [x] Stage D: Integration and behavioural tests
  - [x] Add behavioural tests in `tests/diagnostics_placeholder.rs` for
        placeholder mismatches
  - [x] Add behavioural tests in `tests/diagnostics_table_docstring.rs` for
        data table/docstring mismatches
  - [x] Verify end-to-end diagnostic publishing via publish functions

- [x] Stage E: Documentation and cleanup
  - [x] Update `docs/users-guide.md` with diagnostics section
  - [x] Update `docs/roadmap.md` to mark the feature as done
  - [ ] Run full quality gates (`make check-fmt`, `make lint`, `make test`)
  - [ ] Commit changes

## Surprises & Discoveries

(To be updated as work proceeds)

## Decision Log

### 2026-01-11: Parameter classification approach

**Decision:** Use placeholder name matching to classify parameters as step
arguments vs. fixtures.

**Rationale:** The LSP server does not have access to rstest fixture
definitions. A parameter is a "step argument" if its normalized name appears in
the set of placeholder names extracted from the pattern via `lex_pattern()`.
Parameters marked `is_datatable` or `is_docstring` are excluded. All remaining
parameters are assumed to be fixtures and not counted.

**Alternatives considered:**

1. Type-based heuristics (e.g., primitives are step args) — rejected as
   unreliable and not matching macro behaviour.
2. Require explicit fixture annotations — rejected as too invasive for users.

### 2026-01-11: Placeholder extraction source of truth

**Decision:** Use `rstest_bdd_patterns::pattern::lexer::lex_pattern()` as the
single source of truth for placeholder extraction.

**Rationale:** This ensures consistency between the language server and the
runtime macros. The `SpecificityScore::calculate()` internally uses
`lex_pattern()`, so the same tokens are used throughout.

## Outcomes & Retrospective

(To be completed at major milestones or upon completion)

## Context and Orientation

This feature extends the `rstest-bdd-server` language server crate. The server
already provides:

- Navigation from Rust step definitions to feature steps (Go to Definition)
- Navigation from feature steps to Rust implementations (Go to Implementation)
- Diagnostics for unimplemented feature steps and unused step definitions

### Key Files

- `crates/rstest-bdd-server/src/handlers/diagnostics/mod.rs`: Diagnostic
  constants, publishing logic
- `crates/rstest-bdd-server/src/handlers/diagnostics/compute.rs`: Core
  diagnostic computation (unimplemented steps, unused definitions)
- `crates/rstest-bdd-server/src/handlers/diagnostics/publish.rs`: LSP
  `publishDiagnostics` notification logic
- `crates/rstest-bdd-server/src/indexing/mod.rs`: Data structures
  (`IndexedStepDefinition`, `IndexedStepParameter`, `IndexedStep`)
- `crates/rstest-bdd-server/src/indexing/registry.rs`: `CompiledStepDefinition`
  and `StepDefinitionRegistry`
- `crates/rstest-bdd-server/src/test_support.rs`: Test utilities
  (`ScenarioBuilder`, `SingleFilePairScenario`)
- `crates/rstest-bdd-server/tests/diagnostics_basic.rs`: Behavioral tests for
  unimplemented step and unused definition diagnostics
- `crates/rstest-bdd-server/tests/diagnostics_placeholder.rs`: Behavioral tests
  for placeholder count mismatch diagnostics
- `crates/rstest-bdd-server/tests/diagnostics_table_docstring.rs`: Behavioral
  tests for table/docstring expectation mismatch diagnostics
- `crates/rstest-bdd-patterns/src/specificity.rs`: `SpecificityScore` with
  `placeholder_count`
- `crates/rstest-bdd-patterns/src/pattern/lexer.rs`: `lex_pattern()` returns
  `Vec<Token>` including `Token::Placeholder { name, hint, .. }`

### Key Data Structures

`IndexedStepDefinition` (from Rust parsing):

```rust
struct IndexedStepDefinition {
    keyword: StepType,
    pattern: String,
    pattern_inferred: bool,
    function: RustFunctionId,
    parameters: Vec<IndexedStepParameter>,
    expects_table: bool,
    expects_docstring: bool,
    line: u32,
}
```

`IndexedStepParameter`:

```rust
struct IndexedStepParameter {
    name: Option<String>,
    ty: String,
    is_datatable: bool,
    is_docstring: bool,
}
```

`IndexedStep` (from feature file parsing):

```rust
struct IndexedStep {
    keyword: String,
    step_type: StepType,
    text: String,
    span: Span,
    docstring: Option<IndexedDocstring>,
    table: Option<IndexedTable>,
}
```

`SpecificityScore` (from rstest-bdd-patterns):

```rust
struct SpecificityScore {
    literal_chars: usize,
    placeholder_count: usize,
    typed_placeholder_count: usize,
}
```

### Term Definitions

- **Placeholder:** A `{name}` or `{name:type}` token in a step pattern that
  captures a value from the step text at runtime.
- **Step argument:** A function parameter whose normalized name matches a
  placeholder name in the pattern.
- **Fixture:** A function parameter that is injected by rstest and does not
  correspond to a placeholder.
- **Datatable parameter:** A parameter marked with `is_datatable: true` by the
  Rust indexer. This is determined by either the `#[datatable]` attribute or
  the parameter name `datatable`. The canonical type is `DataTable` from
  `rstest_bdd`.
- **Docstring parameter:** A parameter marked with `is_docstring: true` by the
  Rust indexer. This is determined by either the `#[docstring]` attribute or
  the parameter name `docstring`. The canonical type is `String`.
- **Placeholder count mismatch:** The number of placeholder occurrences in the
  pattern differs from the number of step arguments in the function signature.

  **Counting rules:**

  - **Pattern side:** `count_placeholder_occurrences` counts every `{name}`
    token in the pattern, including duplicates. The pattern `"I compare {x}
    with {x}"` yields **2 occurrences**. This matches the macro's
    `capture_count` semantics, where each occurrence corresponds to a captured
    value at runtime.

  - **Signature side:** `extract_placeholder_names` returns a `HashSet` of
    distinct placeholder names (here, just `x`). A function parameter is
    counted as a step argument if its normalized name appears in that set (or
    if it is a step struct). Therefore, a function with one parameter `x: u32`
    contributes **1 step argument**.

  - **Comparison:** The diagnostic fires when these two counts differ.

  **Concrete examples for pattern `"I compare {x} with {x}"` (2 occurrences):**

  | Function signature | Step args | Diagnostic? |
  | ------------------ | --------- | ----------- |

  | `fn compare() {}`             | 0 | Yes — 2 ≠ 0                      |
  | `fn compare(x: u32) {}`       | 1 | Yes — 2 ≠ 1                      |
  | `fn compare(x: u32, y: u32)`  | 1 | Yes — 2 ≠ 1 (`y` not in set)     |
  | `fn compare(x: u32, _x: u32)` | 2 | No — 2 = 2 (both normalize)      |

  In the last row both `x` and `_x` normalize to `x`, which is in the
  placeholder set, so each counts as a step argument.

## Plan of Work

### Stage A: Design (research only)

Review the existing diagnostic infrastructure and placeholder parsing. No code
changes; only reading and note-taking. Verify understanding by examining test
cases in `crates/rstest-bdd-server/tests/diagnostics_basic.rs`,
`crates/rstest-bdd-server/tests/diagnostics_placeholder.rs`,
`crates/rstest-bdd-server/tests/diagnostics_table_docstring.rs`, and
`crates/rstest-bdd-server/src/handlers/diagnostics/compute.rs`.

### Stage B: Placeholder Count Validation

1. In `handlers/diagnostics/mod.rs`, add:
   - `const CODE_PLACEHOLDER_COUNT_MISMATCH: &str = "placeholder-count-mismatch";`

2. In `handlers/diagnostics/compute.rs`, add a new function:

   ```rust
   pub fn compute_signature_mismatch_diagnostics(
       state: &ServerState,
       rust_path: &Path,
   ) -> Vec<Diagnostic>
   ```

   This function:
   - Retrieves `CompiledStepDefinition`s for the given Rust file from the
     registry.
   - For each definition, extracts placeholder names from the pattern using
     `rstest_bdd_patterns::SpecificityScore::calculate()` to get
     `placeholder_count`.
   - Counts step arguments: parameters whose normalized names appear in the
     placeholder set (derived from `lex_pattern()` and filtering for
     `Token::Placeholder`).
   - If `placeholder_count != step_argument_count`, emit a diagnostic on the
     step definition's line.

3. Add a helper function to extract placeholder names from a pattern:

   ```rust
   fn extract_placeholder_names(pattern: &str) -> HashSet<String>
   ```

   Uses `lex_pattern()` and collects `Token::Placeholder { name, .. }` names.

4. Add a helper to classify parameters:

   ```rust
   fn count_step_arguments(
       parameters: &[IndexedStepParameter],
       placeholder_names: &HashSet<String>,
   ) -> usize
   ```

   Counts parameters where:
   - `!param.is_datatable && !param.is_docstring`
   - `param.name` (normalized) appears in `placeholder_names`

5. Wire the new diagnostic computation into `publish_rust_diagnostics()` in
   `handlers/diagnostics/publish.rs`.

6. Add unit tests in `handlers/diagnostics.rs` (or a new submodule) covering:
   - Correct signature (no diagnostic)
   - Too few placeholders (diagnostic)
   - Too many placeholders (diagnostic)
   - Inferred pattern with no placeholders
   - Pattern with typed placeholders

### Stage C: Data Table and Docstring Expectation Validation

1. Add additional diagnostic codes:
   - `const CODE_TABLE_EXPECTED: &str = "table-expected";`
   - `const CODE_TABLE_NOT_EXPECTED: &str = "table-not-expected";`
   - `const CODE_DOCSTRING_EXPECTED: &str = "docstring-expected";`
   - `const CODE_DOCSTRING_NOT_EXPECTED: &str = "docstring-not-expected";`

2. Create a new function for feature-side validation:

   ```rust
   pub fn compute_table_docstring_mismatch_diagnostics(
       state: &ServerState,
       feature_index: &FeatureFileIndex,
   ) -> Vec<Diagnostic>
   ```

   For each step in the feature file:
   - Find matching Rust implementation(s) by keyword and regex.
   - If the feature step has a table, but the Rust step does not expect one,
     emit
     `table-not-expected` on the table span.
   - If the Rust step expects a table, but the feature step has none, emit
     `table-expected` on the step span.
   - Same logic for docstrings.

3. Wire into `publish_feature_diagnostics()`.

4. Add unit tests covering:
   - Feature step with table, Rust expects table (no diagnostic)
   - Feature step with table, Rust does not expect table (diagnostic)
   - Feature step without table, Rust expects table (diagnostic)
   - Same for docstrings

### Stage D: Behavioural Tests

Add end-to-end tests using `ScenarioBuilder` in:

- `crates/rstest-bdd-server/tests/diagnostics_placeholder.rs`
- `crates/rstest-bdd-server/tests/diagnostics_table_docstring.rs`

Test scenarios:

1. Placeholder count mismatch scenario:
   - Feature: `Given I have {count} apples`
   - Rust: `#[given("I have {count} apples")] fn step() {}` (missing parameter)
   - Assert diagnostic emitted on Rust file

2. Data table mismatch scenario:
   - Feature: step with data table
   - Rust: step without `datatable` parameter
   - Assert diagnostic emitted on feature file

3. Docstring mismatch scenario:
   - Similar to data table

### Stage E: Documentation and Finalization

1. Update `docs/users-guide.md`:
   - Add a "Language Server Diagnostics" section under the language server
     heading (or create one if needed).
   - Document the new diagnostic codes and what they mean.
   - Provide examples of patterns that trigger each diagnostic.

2. Update `docs/roadmap.md`:
   - Mark the "Validate placeholder counts…" item as `[x]` under Phase 7.

3. Run quality gates:
   - `make check-fmt`
   - `make lint`
   - `make test`

4. Commit with message:

   ```text
   Add placeholder and table/docstring mismatch diagnostics to LSP

   Implement on-save diagnostics for:
   - Placeholder count mismatches between patterns and function signatures
   - Data table expectation mismatches
   - Docstring expectation mismatches

   closes #<issue-number-if-any>

   Generated with Claude Code
   ```

## Concrete Steps

All commands are run from the repository root.

### Stage B Commands

```bash
# After implementing the changes, run unit tests for the server crate:
cargo test -p rstest-bdd-server

# Expected output: all tests pass including new placeholder validation tests
```

### Stage C Commands

```bash
# Same as Stage B:
cargo test -p rstest-bdd-server
```

### Stage D Commands

```bash
# Run behavioural tests (split across multiple test binaries):
cargo test -p rstest-bdd-server --test diagnostics_basic
cargo test -p rstest-bdd-server --test diagnostics_placeholder
cargo test -p rstest-bdd-server --test diagnostics_table_docstring

# Or run all diagnostics tests at once:
cargo test -p rstest-bdd-server diagnostics

# Expected output: all tests pass including new end-to-end scenarios
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

- Tests: All existing tests pass. New unit tests cover placeholder count
  validation and table/docstring mismatch detection. Behavioural tests verify
  end-to-end diagnostic publishing.
- Lint/typecheck: `make lint` passes with no warnings.
- Performance: No measurable regression in diagnostic computation time (not
  formally benchmarked, but should remain sub-second for typical workspaces).

Quality method:

- Run `make test` and verify all tests pass.
- Verify `make lint` produces clean output.
- Confirm `make check-fmt` exits without errors.
- Manually test with VS Code (optional) by opening a project with mismatched
  step definitions and observing diagnostics.

Acceptance behaviour:

1. Open a feature file with a step `Given I have {count} apples`.
2. Create a Rust file with `#[given("I have {count} apples")] fn step() {}`
   (macro and empty body).
3. Save both files.
4. Observe diagnostic on the Rust step definition:

   ```plaintext
   Placeholder count mismatch: pattern has 1 placeholder occurrence(s) but function has 0 step argument(s)
   ```

5. Add parameter `count: u32` to the function and save.
6. Observe diagnostic clears.

## Idempotence and Recovery

All stages are re-runnable. If a stage fails partway:

- Discard local changes with `git checkout .` and retry from the beginning of
  that stage.
- Unit tests are isolated and do not leave persistent state.
- The language server state is in-memory only; restarting it resets all indices.

## Artifacts and Notes

(Transcripts and key snippets will be added as work proceeds)

## Interfaces and Dependencies

**New Functions (rstest-bdd-server):**

In `crates/rstest-bdd-server/src/handlers/diagnostics/compute.rs`:

```rust
/// Compute diagnostics for signature mismatches in step definitions.
///
/// Checks that each step definition's placeholder count matches the number
/// of step arguments in the function signature.
pub fn compute_signature_mismatch_diagnostics(
    state: &ServerState,
    rust_path: &Path,
) -> Vec<Diagnostic>

/// Compute diagnostics for table/docstring expectation mismatches.
///
/// Checks that feature steps with tables/docstrings have matching Rust
/// implementations that expect them, and vice versa.
pub fn compute_table_docstring_mismatch_diagnostics(
    state: &ServerState,
    feature_index: &FeatureFileIndex,
) -> Vec<Diagnostic>
```

**Reused APIs (rstest-bdd-patterns):**

```rust
rstest_bdd_patterns::SpecificityScore::calculate(pattern: &str)
    -> Result<SpecificityScore, PatternError>

rstest_bdd_patterns::pattern::lexer::lex_pattern(pattern: &str)
    -> Result<Vec<Token>, PatternError>
```

**Diagnostic Codes:**

```rust
CODE_PLACEHOLDER_COUNT_MISMATCH = "placeholder-count-mismatch"
CODE_TABLE_EXPECTED = "table-expected"
CODE_TABLE_NOT_EXPECTED = "table-not-expected"
CODE_DOCSTRING_EXPECTED = "docstring-expected"
CODE_DOCSTRING_NOT_EXPECTED = "docstring-not-expected"
```
