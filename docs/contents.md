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

## Requests for comments

- [RFC 0001: Explicit step-library scopes][rfc-0001]
  defines closed-world, lexical selection of a scenario's step vocabulary.
- [RFC 0002: Named step-argument binding][rfc-0002]
  defines name-based `StepArgs` capture binding and shared conversion policy.

## User and migration references

- [Gherkin syntax](gherkin-syntax.md) summarizes the feature-file language
  accepted by the framework.
- [v0.5.0 migration guide](v0-5-0-migration-guide.md) helps users move through
  the v0.5.0 release changes.
- [v0.6.0 migration guide](v0-6-0-migration-guide.md) helps users move through
  the v0.6.0 release changes.
- [v0.7.0 migration guide](v0-7-0-migration-guide.md) helps users adopt the
  v0.7.0 scoped-vocabulary and named-capture contracts.
- [Releasing crates](releasing-crates.md) documents the release and publication
  process for workspace crates.
- [Lading user guide](lading-users-guide.md) documents the `lading` release
  tool, which bumps workspace versions, synchronizes internal dependency
  requirements, updates version references in Markdown, and plans and executes
  crate publication.

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

- [ADR 0001: async fixtures and test](adrs/adr-0001-async-fixtures-and-test.md)
  records the async fixture and test decision.
- [ADR 0002: stable step return classification][adr-0002]
  records the return-value classification decision for step functions, as
  amended by ADR-0019.
- [ADR 0003: scenarios macro fixtures](adrs/adr-0003-scenarios-macro-fixtures.md)
  records how scenario macros interact with fixtures.
- [ADR 0004: policy crate](adrs/adr-0004-policy-crate.md) records the decision
  to split policy concerns into a dedicated crate.
- [ADR 0005: async step functions](adrs/adr-0005-async-step-functions.md)
  records the async step-function design decision.
- [ADR 0006: fallible scenario functions](adrs/adr-0006-fallible-scenario-functions.md)
  records the scenario-function fallibility decision.
- [ADR 0007: harness context injection](adrs/adr-0007-harness-context-injection.md)
  records the harness-context injection decision.
- [ADR 0008: harness-led attribute policy defaults][adr-0008]
  records the harness-led attribute-policy default decision.
- [ADR 0009: consistent implicit fixture name normalization][adr-0009]
  records the decision for consistent implicit fixture-name normalization.
- [ADR 0010: feature-file change detection][adr-0010] records the decision for
  making `.feature` files rebuild inputs.
- [ADR 0011: first-party scenario-state helpers][adr-0011] records the planned
  state-store and cleanup-helper direction for stateful harnesses.
- [ADR 0012: guard-based `StepContext` borrowing][adr-0012] records the
  committed v0.7.0 borrowing redesign direction.
- [ADR 0013: adopt Whitaker `no_unwrap_or_else_panic`][adr-0013] records the
  single-lint Whitaker gate decision.
- [ADR 0014: retain the users-guide link validator][adr-0014] records the
  decision to keep `check_users_guide_links.py`, scoped to repository
  references in the vendored users' guide.
- [ADR 0015: name the step-return override outcome][adr-0015] records the
  decision to return `InsertOutcome` from `StepContext::insert_value` instead of
  `Option<Box<dyn Any>>`.
- [ADR 0016: pin the nightly rustfmt toolchain][adr-0016] records the formatting
  toolchain and workspace lint-inheritance decision.
- [ADR 0017: toolchain-aware adapter fallback diagnostics][adr-0017] records the
  toolchain-specific warning and pure-resolution decision for first-party
  adapter fallbacks.
- [ADR 0018: parser-neutral scenario execution][adr-0018] records the accepted
  structured scenario plan and runtime runner for non-Gherkin frontends.
- [ADR 0019: type-directed step-return classification][adr-0019] records the
  inherent-method dispatch that makes local `Result` aliases fallible.
- [ADR 0020: consolidate on the `tracing` logging facade][adr-0020] records the
  decision to emit every workspace diagnostic through `tracing`, keeping `log`
  only to detect an installed `log` listener.
- [ADR 0021: record the users-guide base URL once][adr-0021] records the
  decision to generate the vendored guide's reference block from one recorded
  base URL.
- [ADR 0022: a separate compiler-cache family for the Windows publish dry
  run][adr-0022] records the decision to give the dry run's objects their own
  archive family, sharing the sccache directory with the instrumented one.
- [ADR 0023: adopt the shared CV-005 contract library][adr-0023] records the
  move from a local copy of the CV-005 contract to the pinned shared check.
- [ADR 0024: structural usage tracking in registry lookups][adr-0024]
  records the structural usage-marking decision and the scoped-query boundary.
- [ADR 0025: harness adapter crates][adr-0025-harness]
  records the harness-adapter crate decision.

## Execution plans

- [Execution plans](execplans/) contains implementation plans for roadmap
  tasks, issue work, and accepted follow-up changes.

[adr-0002]: adrs/adr-0002-stable-step-return-classification.md
[adr-0008]: adrs/adr-0008-harness-led-attribute-policy-defaults.md
[adr-0009]: adrs/adr-0009-consistent-implicit-fixture-name-normalization.md
[adr-0010]: adrs/adr-0010-feature-file-change-detection.md
[adr-0011]: adrs/adr-0011-first-party-scenario-state-and-cleanup.md
[adr-0012]: adrs/adr-0012-guard-based-stepcontext-borrowing.md
[adr-0013]: adrs/adr-0013-adopt-whitaker-no-unwrap-or-else-panic.md
[adr-0014]: adrs/adr-0014-retain-users-guide-link-validator.md
[adr-0015]: adrs/adr-0015-insert-outcome-for-step-return-overrides.md
[adr-0016]: adrs/adr-0016-pinned-nightly-rustfmt.md
[adr-0017]: adrs/adr-0017-toolchain-aware-adapter-fallback-diagnostics.md
[adr-0018]: adrs/adr-0018-parser-neutral-scenario-execution.md
[adr-0019]: adrs/adr-0019-type-directed-step-return-classification.md
[adr-0020]: adrs/adr-0020-consolidate-on-the-tracing-logging-facade.md
[adr-0021]: adrs/adr-0021-single-source-base-url-for-users-guide-links.md
[adr-0022]: adrs/adr-0022-separate-compiler-cache-family-for-the-publish-dry-run.md
[adr-0023]: adrs/adr-0023-adopt-the-shared-cv005-contract-library.md
[adr-0024]: adrs/adr-0024-structural-usage-tracking-in-registry-lookups.md
[adr-0025-harness]: adrs/adr-0025-harness-adapter-crates-for-framework-specific-test-integration.md
[complexity-guide]: complexity-antipatterns-and-refactoring-strategies.md
[cucumber-async]: cucumber-rs-migration-and-async-patterns.md
[dependency-injection]: reliable-testing-in-rust-via-dependency-injection.md
[fluent-guide]: localizable-rust-libraries-with-fluent.md
[rfc-0001]: rfcs/rfc-0001-explicit-step-library-scopes.md
[rfc-0002]: rfcs/rfc-0002-named-step-argument-binding.md
