# Documentation contents

- [Documentation contents](contents.md) is the index for the repository
  documentation set.

## Starting points

- [Users' guide](users-guide.md) explains how to use `rstest-bdd`, its macros,
  fixture model, harness adapters, and Gherkin integration.
- [Developer guide](developers-guide.md) explains contributor workflows,
  maintainer conventions, and implementation practices.
- [Repository layout](repository-layout.md) maps the workspace directories,
  crates, examples, support scripts, and vendored shims.
- [Documentation style guide](documentation-style-guide.md) defines the
  writing, formatting, naming, and document-type conventions used here.
- [Roadmap](roadmap.md) tracks accepted delivery work and longer-range
  development sequencing.

## Product and architecture

- [rstest-bdd design](rstest-bdd-design.md) describes the main framework
  design, user-facing model, macro architecture, and implementation rationale.
- [rstest-bdd language server design](rstest-bdd-language-server-design.md)
  describes planned language-server support for feature files and Rust step
  definitions.
- [Testing strategy](testing-strategy.md) explains the test layers and quality
  expectations for this repository.
- [Ergonomics and developer experience](ergonomics-and-developer-experience.md)
  records usability goals and developer-experience trade-offs.
- [Known issues](known-issues.md) collects documented defects, constraints, and
  limitations that maintainers should keep visible.
- [Changelog](CHANGELOG.md) records repository-level release history.

## User and migration references

- [Gherkin syntax](gherkin-syntax.md) summarizes the feature-file language
  accepted by the framework.
- [v0.5.0 migration guide](v0-5-0-migration-guide.md) helps users move through
  the v0.5.0 release changes.
- [v0.6.0 migration guide](v0-6-0-migration-guide.md) helps users move through
  the v0.6.0 release changes.
- [Releasing crates](releasing-crates.md) documents the release and publication
  process for workspace crates.

## Contributor references

- [Complexity antipatterns and refactoring strategies][complexity-guide]
  describes code-health risks and refactoring responses.
- [Cucumber-rs migration and async patterns][cucumber-async]
  records migration knowledge for async behaviour-driven testing.
- [Localizable Rust libraries with Fluent][fluent-guide]
  explains localization patterns relevant to diagnostics and messages.
- [Reliable testing in Rust via dependency injection][dependency-injection]
  captures testing design guidance for injectable dependencies.
- [Rust doctest DRY guide](rust-doctest-dry-guide.md) explains how to keep
  doctests readable without duplicating too much setup.
- [Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md)
  documents fixture-oriented testing patterns.
- [Scripting standards](scripting-standards.md) defines repository expectations
  for shell and Python support scripts.

## Decision records

- [ADR 001: async fixtures and test](adr-001-async-fixtures-and-test.md)
  records the async fixture and test decision.
- [ADR 002: stable step return classification][adr-002]
  records the return-value classification decision for step functions.
- [ADR 003: scenarios macro fixtures](adr-003-scenarios-macro-fixtures.md)
  records how scenario macros interact with fixtures.
- [ADR 004: policy crate](adr-004-policy-crate.md) records the decision to
  split policy concerns into a dedicated crate.
- [ADR 005: async step functions](adr-005-async-step-functions.md) records the
  async step-function design decision.
- [ADR 005a: harness adapter crates][adr-005a-harness]
  records the harness-adapter crate decision.
- [ADR 006: fallible scenario functions](adr-006-fallible-scenario-functions.md)
  records the scenario-function fallibility decision.
- [ADR 007: harness context injection](adr-007-harness-context-injection.md)
  records the harness-context injection decision.
- [ADR 008: harness-led attribute policy defaults][adr-008]
  records the harness-led attribute-policy default decision.
- [ADR 009: consistent implicit fixture name normalization][adr-009]
  records the fixture-name normalization decision.
- [ADR 010: feature-file change detection][adr-010] records the decision for
  making `.feature` files rebuild inputs.
- [ADR 011: first-party scenario-state helpers][adr-011] records the planned
  state-store and cleanup-helper direction for stateful harnesses.
- [ADR 012: guard-based `StepContext` borrowing][adr-012] records the
  committed v0.7.0 borrowing redesign direction.
- [ADR 013: adopt Whitaker `no_unwrap_or_else_panic`][adr-013] records the
  single-lint Whitaker gate decision.
- [ADR 014: parser-neutral scenario execution][adr-014] records the proposed
  structured scenario plan and runtime runner for non-Gherkin frontends.

## Execution plans

- [Execution plans](execplans/) contains implementation plans for roadmap
  tasks, issue work, and accepted follow-up changes.

[adr-002]: adr-002-stable-step-return-classification.md
[adr-005a-harness]: adr-005-harness-adapter-crates-for-framework-specific-test-integration.md
[adr-008]: adr-008-harness-led-attribute-policy-defaults.md
[adr-009]: adr-009-consistent-implicit-fixture-name-normalization.md
[adr-010]: adr-010-feature-file-change-detection.md
[adr-011]: adr-011-first-party-scenario-state-and-cleanup.md
[adr-012]: adr-012-guard-based-stepcontext-borrowing.md
[adr-013]: adr-013-adopt-whitaker-no-unwrap-or-else-panic.md
[adr-014]: adr-014-parser-neutral-scenario-execution.md
[complexity-guide]: complexity-antipatterns-and-refactoring-strategies.md
[cucumber-async]: cucumber-rs-migration-and-async-patterns.md
[dependency-injection]: reliable-testing-in-rust-via-dependency-injection.md
[fluent-guide]: localizable-rust-libraries-with-fluent.md
