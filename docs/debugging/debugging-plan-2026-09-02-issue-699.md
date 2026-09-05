# Debugging plan: Synchronize the LSP definition smoke test

**Generated**: 2026-09-02
**Issue ID**: #699
**Severity**: CI-blocking test failure
**Falsification sub-agent**: alchemist
**Planning agent boundary**: This document was prepared by the planning agent.
Falsification must be executed by the named sub-agent, not by the planning
agent.

## Problem statement

The Windows smoke test sends a definition request after `index_and_wait`, but
the server sometimes returns `null` rather than a location array. The helper
sends saves for the feature and Rust files together, then waits only for a Rust
`publishDiagnostics` notification. That notification must be replaced or
strengthened with a deterministic protocol that establishes that both indexes
are available before the definition request is sent.

## Context summary

| Aspect | Details |
| --- | --- |
| First observed | Windows CI run 33584006657 on PR #706 |
| Reproduction rate | Intermittent; Linux passed, Windows failed |
| Affected components | `smoke_lsp` synchronization and deferred save replay |
| Recent changes | The helper was restored, but it sent both saves before waiting |

### Error artefacts

```plaintext
smoke_definition_request_returns_locations ... FAILED
expected array of locations, got: null
```

### Information gaps

- The CI runner's internal scheduling is not observable from the failed log.
- The exact transport-to-router interleaving requires a deterministic local
  experiment rather than inference from one CI run.

______________________________________________________________________

## Hypotheses

### H1: The two save notifications are handled out of dependency order

**Claim**: The server can process the Rust save and publish its diagnostics
before it applies the feature save, so waiting for that Rust notification does
not establish that a matching feature index exists.

**Plausibility**: High — the helper sends both notifications without a
per-document acknowledgement, while the server starts asynchronous workspace
preparation and deferred replay.

**Prediction**: If the feature save is acknowledged before the Rust save is
sent, and the Rust save is then acknowledged, the definition response is
always a location array.

#### H1 falsification plan

| Step | Action | Expected Negative Result |
| --- | --- | --- |
| 1 | Inspect the save and deferred-replay paths for a serialization guarantee from the first notification to the second. | A proved ordering guarantee falsifies H1. |
| 2 | Run the smallest smoke test variant that waits for the feature diagnostic before sending the Rust save. | A `null` response despite both phase acknowledgements falsifies H1. |

**Tooling**: Focused source inspection and the single `smoke_lsp` test target.

**Confidence on falsification**: High. A proven single-router ordering or a
failed phase-acknowledged test rules out the proposed race.

______________________________________________________________________

### H2: The helper accepts a Rust diagnostic from a failed index

**Claim**: A Rust parse or read failure publishes an empty diagnostic vector,
and `index_and_wait` treats it as successful indexing even though the step
registry was not updated.

**Plausibility**: Medium — the fatal Rust-index error path deliberately emits
an empty `publishDiagnostics` notification.

**Prediction**: A failed Rust index can produce the exact notification that the
helper currently accepts while `handle_definition` has no step definition.

#### H2 falsification plan

| Step | Action | Expected Negative Result |
| --- | --- | --- |
| 1 | Inspect the smoke fixture's Rust source and the Rust indexing result path. | A successful index is guaranteed before the accepted notification, falsifying H2. |

**Tooling**: Focused source inspection only; no production edits.

**Confidence on falsification**: High. The fixture and error-path contracts are
directly inspectable.

______________________________________________________________________

### H3: The notification receiver loses a required protocol message

**Claim**: `recv_notification_matching` consumes non-matching messages, so a
notification needed to establish readiness is discarded before the request.

**Plausibility**: Low — the definition response uses id-matched reception, but
the helper intentionally ignores all other notifications.

**Prediction**: A required readiness signal occurs before the accepted Rust
notification and cannot be recovered by the test harness.

#### H3 falsification plan

| Step | Action | Expected Negative Result |
| --- | --- | --- |
| 1 | Inspect receiver semantics and emitted diagnostic order for the two saves. | No required signal is discarded, falsifying H3. |

**Tooling**: Focused source inspection only; no production edits.

**Confidence on falsification**: Medium. This rules out loss within the test
harness, but not server scheduling.

______________________________________________________________________

## Recommended execution order

1. **H1** — directly tests the most likely cross-platform race.
2. **H2** — cheaply rules out a false-positive readiness signal.
3. **H3** — confirms whether the receiver constrains the replacement protocol.

## Termination criteria

- **Root cause identified**: One hypothesis survives its falsification attempt
  and yields a deterministic replacement protocol.
- **Escalation trigger**: All hypotheses are falsified; revise this plan using
  the experiment outputs and the failed Windows log.

## Falsification results

- **H1 falsified**: Direct and deferred save handling preserve the feature then
  Rust order. The original helper nevertheless left a single, ambiguous
  readiness boundary. The replacement uses phase-specific acknowledgements.
- **H2 falsified for this fixture**: Its Rust source indexes successfully,
  although an unrelated fatal index failure can publish empty diagnostics.
- **H3 not falsified**: `recv_notification_matching` discards non-matching
  messages. The replacement must therefore wait for each URI before sending
  the next dependent save.

## Notes for executing agent

Run only the supplied minimal experiment. Do not run repository-wide gates,
modify tracked files, or use sleeps. Report a verdict for each hypothesis with
the source or test evidence that supports it.
