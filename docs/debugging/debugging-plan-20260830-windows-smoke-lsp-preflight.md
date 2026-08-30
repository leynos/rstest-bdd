# Debugging Plan: Windows `smoke_lsp` pre-flight failure

**Generated**: 2026-08-30
**Issue ID**: PR #648 Windows CI
**Severity**: Blocking CI failure
**Falsification sub-agent**: alchemist
**Planning agent boundary**: This document was prepared by the planning agent.
Falsification must be executed by the named sub-agent, not by the planning
agent.

## Problem Statement

The Windows publish pre-flight fails before the actual `smoke_lsp` assertion
can be identified. Lading currently drops Cargo stdout when stderr is also
present, and `smoke_lsp` discards the language server's stderr. Repair these
diagnostic paths first, then use the next native Windows CI execution to
identify and correct the actual platform-specific failure.

## Context Summary

| Aspect | Details |
| --- | --- |
| First observed | PR #648 Windows CI run 33332323095 |
| Reproduction rate | Reproducible on Windows CI; native Windows unavailable locally |
| Affected components | Lading Cargo pre-flight and `rstest-bdd-server` smoke LSP test |
| Recent changes | Dev-fast configuration and workspace lint remediation |

### Error Artefacts

```plaintext
Windows pre-flight reports only Cargo warnings on stderr while omitting
stdout, which contains test outcomes. `smoke_lsp` starts the server with
stderr redirected to null, so timeout and early-exit failures omit server
diagnostics.
```

### Information Gaps

- The actual failing Windows `smoke_lsp` assertion is not present in the
  current diagnostic output.
- The local host is Linux and cannot execute the native Windows test binary.

______________________________________________________________________

## Hypotheses

### H1: Cargo pre-flight hides the failing test result

**Claim**: Cargo writes test results to stdout and warnings to stderr, and
Lading's stderr-first selection drops the test result when both are non-empty.

**Plausibility**: High — the reporter selects `stderr or stdout`.

**Prediction**: A controlled failed Cargo result with both streams will produce
an error that names and includes each stream exactly once.

#### H1 Falsification Plan

| Step | Action | Expected Negative Result |
| --- | --- | --- |
| 1 | Add a unit test with failed test output and warnings. | The error lacks either labelled stream. |
| 2 | Run the focused Lading tests. | The dual-stream assertion fails. |

**Tooling**: Focused `pytest` unit tests for Cargo pre-flight reporting.

**Confidence on falsification**: Decisive for the pre-flight reporting path.

______________________________________________________________________

### H2: The LSP server emits a useful early-exit diagnostic

**Claim**: The server's stderr explains an early exit or receive timeout, but
the smoke harness currently discards it.

**Plausibility**: High — the child is spawned with `Stdio::null()` for stderr.

**Prediction**: A controlled child that writes a marker to stderr and exits
will surface that marker in the smoke harness failure diagnostic.

#### H2 Falsification Plan

| Step | Action | Expected Negative Result |
| --- | --- | --- |
| 1 | Capture child stderr without changing JSON-RPC stdout. | The marker is absent from the failure message. |
| 2 | Run the smoke LSP test target. | Existing cleanup or protocol assertions regress. |

**Tooling**: Focused Rust integration test with a controlled child process.

**Confidence on falsification**: Decisive for child-stderr diagnostic capture.

______________________________________________________________________

### H3: A platform-specific smoke assertion fails after diagnostics are fixed

**Claim**: Once both streams are retained, the Windows run identifies one
concrete `smoke_lsp` assertion requiring a Windows-specific correction.

**Plausibility**: Unknown — current logs cannot distinguish assertion,
environment, or startup failures.

**Prediction**: The next native Windows CI run names one test and its failed
assertion, without an ambiguous pre-flight wrapper.

#### H3 Falsification Plan

| Step | Action | Expected Negative Result |
| --- | --- | --- |
| 1 | Pin rstest-bdd to the repaired Lading revision and run Windows CI. | No precise smoke test or assertion is reported. |
| 2 | Run the reported test on Windows after a minimal fix. | The same assertion still fails. |

**Tooling**: GitHub Actions Windows runner and separate captured command
streams.

**Confidence on falsification**: Decisive once a native Windows runner returns
the preserved diagnostics.

______________________________________________________________________

## Recommended Execution Order

1. **H1** — restore the pre-flight's missing Cargo output first.
2. **H2** — restore the child process's missing LSP output independently.
3. **H3** — use the newly observable native Windows failure as the only basis
   for a platform correction.

## Termination Criteria

- **Root cause identified**: A native Windows execution reports a named failing
  smoke test and assertion, and the smallest targeted fix makes it pass.
- **Escalation trigger**: Diagnostics remain incomplete after H1 and H2 pass,
  or the Windows runner fails before `smoke_lsp` begins.

## Progress

- [x] Correct the four example manifests and add a TOML contract test for the
  package and lint tables.
- [x] Confirm H1 with a controlled Cargo result: the original reporter omitted
  stdout when stderr was present. Lading commit
  `e0a8d43fa3d6d7598cad0d4c25883e7ea625feb9` now retains separately labelled,
  bounded streams.
- [x] Confirm H2 with a controlled early-exiting child: its stderr marker is
  included in the smoke-test diagnostic without entering JSON-RPC stdout.
- [x] Pin the local and CI Lading references to the diagnostic commit.
- [ ] Run the native Windows pre-flight and inspect its separately preserved
  output before making a platform-specific behavioural change.

## Notes for Executing Agent

Do not infer Windows behaviour from Linux. Keep Cargo stdout and stderr
separate, bound all captured diagnostics, and preserve JSON-RPC stdout as a
protocol-only stream.
