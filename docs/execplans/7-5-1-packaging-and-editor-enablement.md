# Packaging and editor enablement

This execution plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This document follows the ExecPlans skill template.

## Purpose / Big Picture

After this change, the `rstest-bdd-lsp` binary ships with three CLI options
(`--log-level`, `--debounce-ms`, `--workspace-root`) that allow users and
editors to configure the server without relying solely on environment
variables. Editor integration is documented for VS Code, Neovim, and Zed.
End-to-end smoke tests validate the full server stack by starting the binary,
exchanging JSON Remote Procedure Call (JSON-RPC) messages, and asserting
correct behaviour.

Observable outcomes:

- Running `rstest-bdd-lsp --help` shows all three CLI flags with descriptions.
- CLI flags override environment variable values (precedence: Default -> env
  var -> CLI flag).
- The `--workspace-root` flag overrides the Language Server Protocol (LSP)
  client's root Uniform Resource Identifier (URI) for workspace discovery.
- Three smoke tests pass in continuous integration (CI):
  initialize/shutdown, definition navigation, and diagnostic publication.
- The design document and user guide contain VS Code, Neovim, and Zed editor
  integration examples.
- The corresponding roadmap entries (7.5.1, 7.5.2) are marked complete.

## Constraints

Hard invariants that must hold throughout implementation:

- **Do not modify macro crates:** `rstest-bdd-macros` and `rstest-bdd` crates
  must not be changed unless strictly necessary.
- **Preserve existing behaviour:** All existing tests, navigation handlers, and
  diagnostics must continue to work unchanged.
- **Do not introduce new external dependencies:** The implementation must use
  crates already in the workspace.
- **File length limit:** No single file may exceed 400 lines.
- **Quality gates:** `make check-fmt`, `make lint`, and `make test` must all
  pass before any commit.
- **Module-level doc comments:** Every new module must have a `//!` doc comment.
- **Public API documentation:** Every new public function/struct must have `///`
  rustdoc comments.

## Tolerances (Exception Triggers)

Thresholds that trigger escalation when breached:

- **Scope:** If implementation requires changes to more than 10 files or 800
  net lines of code, stop and escalate.
- **Interface:** If a public API signature in other crates must change, stop and
  escalate.
- **Dependencies:** If a new external crate dependency is required, stop and
  escalate.
- **Iterations:** If tests still fail after 3 debugging attempts on the same
  issue, stop and escalate.

## Risks

Known uncertainties that might affect the plan:

- Risk: Smoke tests may be flaky due to timing or buffering issues. Severity:
  medium. Likelihood: medium. Mitigation: Use `--debounce-ms 0` to eliminate
  indexing delay. Use `read_response_for_id()` to skip interleaved
  notifications. Read with bounded message counts rather than wall-clock
  timeouts.

- Risk: Zed configuration may be incomplete because Zed's LSP integration
  model evolves. Severity: low. Likelihood: medium. Mitigation: Document the
  settings.json approach with a note that Zed may require an extension for full
  integration.

- Risk: Windows path handling in smoke tests. Severity: medium. Likelihood:
  low. Mitigation: Use `lsp_types::Url::from_file_path()` and
  `from_directory_path()` for platform-correct URIs throughout.

## Progress

- [x] Stage A: Wire `--debounce-ms` CLI flag
  - [x] Add field to `Args` struct in `main.rs`
  - [x] Pass through to `config.apply_overrides()`

- [x] Stage B: Add `workspace_root` to `ServerConfig`
  - [x] Add `workspace_root: Option<PathBuf>` field to `ServerConfig`
  - [x] Read `RSTEST_BDD_LSP_WORKSPACE_ROOT` environment variable
  - [x] Extend `apply_overrides()` with `workspace_root` parameter
  - [x] Add `with_workspace_root()` builder method
  - [x] Add `--workspace-root` CLI flag to `Args`
  - [x] Add unit tests for env var parsing, default, and override application
  - [x] Update `lib.rs` crate-level doc comment

- [x] Stage C: Use `workspace_root` in lifecycle handler
  - [x] Modify `handle_initialise()` to prefer config workspace root
  - [x] Add unit test for config workspace root override

- [x] Stage D: Smoke tests
  - [x] Create `tests/smoke_lsp/` with JSON-RPC wire helpers
  - [x] Implement `smoke_initialise_and_shutdown` test
  - [x] Implement `smoke_definition_request_returns_locations` test
  - [x] Implement `smoke_diagnostics_published_for_unimplemented_step` test
  - [x] Handle interleaved notifications with `read_response_for_id()`

- [x] Stage E: Documentation
  - [x] Add Phase 7.5 implementation status to design document
  - [x] Add CLI options table and Zed example to design document
  - [x] Add CLI options subsection to user guide
  - [x] Add `RSTEST_BDD_LSP_WORKSPACE_ROOT` to env var table in user guide
  - [x] Add Zed editor integration subsection to user guide
  - [x] Mark roadmap items 7.5.1 and 7.5.2 as done

- [x] Stage F: Write this ExecPlan document

- [x] Stage G: Final validation
  - [x] Run `make check-fmt`
  - [x] Run `make lint`
  - [x] Run `make test`

## Surprises & Discoveries

- Two pre-existing doctest failures in `placeholder.rs` and
  `table_docstring.rs` used `ServerState::default()` but `ServerState` does not
  implement `Default`. Fixed by replacing with
  `ServerState::new(ServerConfig::default())`.

- The definition smoke test initially failed because `read_message()` read a
  `publishDiagnostics` notification instead of the definition response. Fixed
  by introducing `read_response_for_id()` which skips notifications until the
  response with the expected `id` arrives.

## Decision Log

### 2026-02-07: workspace_root as hard override

**Decision:** The `--workspace-root` CLI flag acts as a hard override for
workspace discovery, replacing the LSP client's `root_uri` and
`workspace_folders`.

**Rationale:** The primary use case is when the editor sends incorrect paths or
during headless/scripted testing. Having the CLI override the client value
provides maximum control. The implementation uses `Option::or_else()` so the
client's root is only consulted when no CLI/env override is set.

**Alternatives considered:**

1. Use workspace_root only as a fallback when the client sends nothing —
   rejected as less useful since editors typically do send a root.

### 2026-02-07: Smoke test architecture

**Decision:** Smoke tests start the real binary as a child process and
communicate via JSON-RPC over stdin/stdout.

**Rationale:** Existing integration tests call handler functions directly
without going through the binary. Smoke tests validate the full stack: CLI
argument parsing, server startup, JSON-RPC framing, routing, indexing pipeline,
handler responses, and graceful shutdown. Using `env!("CARGO_BIN_EXE_...")`
ensures the binary is automatically built by Cargo.

### 2026-02-07: Zed editor documentation approach

**Decision:** Document Zed configuration via `settings.json` with a note about
the extension system.

**Rationale:** Zed's LSP integration is evolving and may require a dedicated
extension for full integration. The settings.json approach works for
development and testing purposes and provides a starting point for users.

## Outcomes & Retrospective

All objectives achieved:

- Three CLI options (`--log-level`, `--debounce-ms`, `--workspace-root`) are
  implemented and tested.
- Editor integration documented for VS Code, Neovim, and Zed.
- Three end-to-end smoke tests validate the server binary in CI.
- Roadmap items 7.5.1 and 7.5.2 marked complete.
- Pre-existing doctest failures fixed as a side effect.

## Context and Orientation

This feature completes Phase 7.5 of the roadmap — the final step in the
"Language server foundations" phase. The server already provides:

- Navigation from Rust step definitions to feature steps (Go to Definition)
- Navigation from feature steps to Rust implementations (Go to Implementation)
- Nine categories of on-save diagnostics

### Key Files

- `crates/rstest-bdd-server/src/main.rs`: Binary entry point with CLI argument
  parsing
- `crates/rstest-bdd-server/src/config.rs`: Server configuration (env vars,
  overrides)
- `crates/rstest-bdd-server/src/handlers/lifecycle.rs`: Initialize handler
  using workspace root
- `crates/rstest-bdd-server/tests/smoke_lsp/main.rs`: End-to-end smoke tests
- `crates/rstest-bdd-server/tests/smoke_lsp/wire.rs`: JSON-RPC wire helpers
- `docs/rstest-bdd-language-server-design.md`: Design document
- `docs/users-guide.md`: User guide
- `docs/roadmap.md`: Project roadmap

## Plan of Work

### Stage A: Wire `--debounce-ms` (2 lines)

Add `debounce_ms: Option<u64>` to the `Args` struct and pass it through to
`config.apply_overrides()` instead of `None`.

### Stage B: Add `workspace_root` to config (~30 lines)

Add the `workspace_root: Option<PathBuf>` field to `ServerConfig`, read the
`RSTEST_BDD_LSP_WORKSPACE_ROOT` env var, extend `apply_overrides()`, and add
the `--workspace-root` CLI flag.

### Stage C: Use in lifecycle handler (~5 lines)

Replace the workspace path extraction with a chain that prefers
`config.workspace_root` over the client-provided root.

### Stage D: Smoke tests (~300 lines)

Create `tests/smoke_lsp.rs` with JSON-RPC wire helpers and three tests:
initialize/shutdown, definition request, diagnostic publication.

### Stage E: Documentation (~110 lines across 3 files)

Add CLI options table and Zed integration to design doc and user guide. Mark
roadmap items done.

### Stage F: ExecPlan

Write this document.

### Stage G: Final validation

Run `make check-fmt`, `make lint`, `make test`.

## Concrete Steps

All commands are run from the repository root.

```bash
# After implementing code changes:
cargo test -p rstest-bdd-server --features test-support

# After documentation changes:
make check-fmt
make markdownlint

# Final quality gate:
set -o pipefail && make check-fmt 2>&1 | tee /tmp/check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/lint.log
set -o pipefail && make test 2>&1 | tee /tmp/test.log
```

## Validation and Acceptance

Quality criteria:

- All existing tests pass unchanged.
- Three new smoke tests pass.
- New unit tests cover workspace root config, env var parsing, and override
  application.
- `make check-fmt`, `make lint`, and `make test` all pass.

Acceptance behaviour:

1. Run `rstest-bdd-lsp --help` and see all three flags.
2. Run `rstest-bdd-lsp --debounce-ms 0 --log-level trace` and verify
   trace-level output.
3. Run `cargo nextest run -p rstest-bdd-server -E 'test(smoke_)'` and see
   3/3 pass.

## Idempotence and Recovery

All stages are re-runnable. If a stage fails partway:

- Discard local changes with `git checkout .` and retry from the beginning of
  that stage.
- Unit tests are isolated and do not leave persistent state.
- Smoke tests use `tempfile::TempDir` which is cleaned up on drop.

## Artifacts and Notes

Files modified:

- `crates/rstest-bdd-server/src/config.rs`
- `crates/rstest-bdd-server/src/main.rs`
- `crates/rstest-bdd-server/src/lib.rs`
- `crates/rstest-bdd-server/src/handlers/lifecycle.rs`
- `crates/rstest-bdd-server/src/handlers/diagnostics/placeholder.rs` (doctest
  fix)
- `crates/rstest-bdd-server/src/handlers/diagnostics/table_docstring.rs`
  (doctest fix)

Files created:

- `crates/rstest-bdd-server/tests/smoke_lsp/main.rs`
- `crates/rstest-bdd-server/tests/smoke_lsp/wire.rs`
- `docs/execplans/7-5-1-packaging-and-editor-enablement.md`

Files updated (docs):

- `docs/rstest-bdd-language-server-design.md`
- `docs/users-guide.md`
- `docs/roadmap.md`

## Interfaces and Dependencies

**New CLI Flags (rstest-bdd-lsp):**

```text
--log-level <LEVEL>        Log level (trace, debug, info, warn, error)
--debounce-ms <MS>         Debounce interval in milliseconds
--workspace-root <PATH>    Override workspace root path for discovery
```

**New Environment Variable:**

```text
RSTEST_BDD_LSP_WORKSPACE_ROOT   Override workspace root path for discovery
```

**New Config Field:**

```rust
pub struct ServerConfig {
    pub workspace_root: Option<PathBuf>,  // NEW
    // ... existing fields unchanged
}
```

**Modified Signature:**

```rust
pub fn apply_overrides(
    mut self,
    log_level: Option<LogLevel>,
    debounce_ms: Option<u64>,
    workspace_root: Option<PathBuf>,  // NEW parameter
) -> Self
```
