# Debugging plan: Nextest global timeout

**Generated**: 2026-08-15 **Issue ID**: Commit gate failure **Severity**: High
**Falsification sub-agent**: alchemist **Planning agent boundary**: This
document was prepared by the planning agent. Falsification must be executed by
the named sub-agent, not by the planning agent.

## Problem Statement

`make test` must complete successfully, but the default Nextest profile
terminated `gpui_macro_fixtures_compile` when the whole run reached its
five-minute global timeout. The test had been running for 37 seconds and was
still compiling its trybuild fixture; 1,680 preceding tests had passed.

## Context Summary

| Aspect              | Details                                              |
| ------------------- | ---------------------------------------------------- |
| First observed      | 2026-08-15 during the required commit gate           |
| Reproduction rate   | One full cold-cache-adjacent run                     |
| Affected components | `.config/nextest.toml` and cargo-spawning tests      |
| Recent changes      | Registry refactor and lint-policy documentation only |

_Table 1: Debugging context summary._

### Error Artefacts

```plaintext
Cancelling due to global timeout: 1 test still running
SIGTERM [37.121s] rstest-bdd-harness-gpui::macro_compile::gpui_macro_fixtures_compile
Summary [300.011s] 1681/1685 tests run: 1680 passed, 1 failed, 7 skipped
```

### Information Gaps

- The duration of the full suite on a completely cold Cargo cache is unknown.
- The complete sequence of cargo-spawning test binaries before cancellation is
  not visible in the summary.

______________________________________________________________________

## Hypotheses

### H1: The default global timeout is shorter than the serial test schedule

**Claim**: The five-minute global timeout conflicts with cargo-spawning tests
that are deliberately serialized and can each run for up to 300 seconds.

**Plausibility**: High — the interrupted test was within its 300-second
per-test allowance, and serialization means its budget is additive rather than
parallel.

**Prediction**: The affected GPUI trybuild binary completes when it is run with
the other compile binary but without the full-suite scheduling load.

#### H1 Falsification Plan

| Step | Action                                                       | Expected Negative Result                        |
| ---- | ------------------------------------------------------------ | ----------------------------------------------- |
| 1    | Run the two compile-test binaries under the default profile. | Either binary times out or fails independently. |

_Table 2: H1 falsification step and expected negative result._

**Tooling**: `cargo nextest run` with an expression that selects only
`rstest-bdd::trybuild_macros` and `rstest-bdd-harness-gpui::macro_compile`.

**Confidence on falsification**: High for an intrinsic failure; a passing run
supports a scheduling-budget repair but does not measure the whole suite.

______________________________________________________________________

### H2: The GPUI trybuild fixture itself is broken

**Claim**: `gpui_macro_fixtures_compile` is failing or hanging independently of
Nextest's full-suite time budget.

**Plausibility**: Low — the captured output shows an active compilation and no
test assertion or compiler error before Nextest sent `SIGTERM`.

**Prediction**: Selecting only the GPUI compile-test binary still fails or
exceeds its per-test 300-second allowance.

#### H2 Falsification Plan

| Step | Action                                  | Expected Negative Result                              |
| ---- | --------------------------------------- | ----------------------------------------------------- |
| 1    | Run the selected compile-test binaries. | The GPUI binary passes within its per-test allowance. |

_Table 3: H2 falsification step and expected negative result._

**Tooling**: The same targeted `cargo nextest run` command as H1.

**Confidence on falsification**: Decisive for a fixture-level regression.

**Result**: Falsified. The selected test binaries passed in 49.786 seconds; the
GPUI fixture passed in 21.972 seconds. Evidence:
`/tmp/alchemist-h2-gpui-timeout-20260815.log`.

**Representative-cache validation (2026-08-16):** `make test` completed in
136.79 s, and
`cargo nextest run --profile long --workspace --all-targets --all-features`
completed in 23.54 s. Neither run emitted timeout warnings; the configured
`20m` default and `30m` long-profile global budgets retain cold-cache headroom.

______________________________________________________________________

## Recommended Execution Order

1. **H2** — the selected test is the smallest decisive experiment.
2. **H1** — apply a configuration repair only if H2 is falsified.

## Termination Criteria

- **Root cause identified**: The GPUI binary passes in isolation and the
  configured global timeout is shown to be incompatible with tests that run one
  at a time.
- **Escalation trigger**: The selected GPUI binary fails independently or
  exceeds 300 seconds.

## Notes for Executing Agent

Do not edit files or run full repository gates. Run only the supplied targeted
experiment, record elapsed time and the result, then return a verdict of
falsified, not-falsified, or inconclusive for H2.
