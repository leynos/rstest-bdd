# Debugging Plan: Restore ctor Diagnostics

**Generated**: 2026-09-07
**Issue ID**: PR #708
**Severity**: High
**Falsification sub-agent**: alchemist
**Planning agent boundary**: This document was prepared by the planning agent.
Falsification must be executed by the named sub-agent, not by the planning
agent.

## Problem Statement

After changing the diagnostics constructor to `#[ctor(unsafe)]`, the complete
test suite's `cargo-bdd::cli::list_steps_runs` test receives no registry
output. The test fixture's constructor must run before libtest parses
`--dump-steps`.

## Context Summary

| Aspect              | Details                                                    |
| ------------------- | ---------------------------------------------------------- |
| First observed      | `make test` on 2026-09-07                                  |
| Reproduction rate   | One full-suite run; deterministic fixture test             |
| Affected components | `rstest-bdd`, cargo-bdd's minimal fixture and its lockfile |
| Recent changes      | Root `ctor` requirement moved from 0.4 to 1.0.13           |

### Error Artefacts

```plaintext
cargo-bdd::cli::list_steps_runs
Expected non-empty output from steps command
```

### Information Gaps

- The version of `ctor` resolved by the nested minimal fixture has not yet been
  measured after the root workspace requirement changed.

______________________________________________________________________

## Hypotheses

### H1: The minimal fixture resolves ctor 0.4.3

**Claim**: The minimal fixture's tracked `Cargo.lock` resolves `ctor` 0.4.3,
which cannot support `#[ctor(unsafe)]`. Its test binary consequently fails to
build and `cargo-bdd` has no registry binary to execute.

**Plausibility**: High — the root manifest previously permitted 0.4 while its
lockfile already contained 1.0.13; nested fixtures maintain independent locks.

**Prediction**: Locked metadata for the minimal fixture will show `ctor` 0.4.3.

#### H1 Falsification Plan

| Step | Action                                                                                                           | Expected Negative Result                                          |
| ---- | ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| 1    | Run `cargo metadata --locked --format-version 1` in the minimal fixture and inspect the resolved `ctor` package. | `ctor` resolves to 1.0.13, disproving the stale-lock explanation. |

**Tooling**: Cargo metadata and the tracked fixture manifest.

**Confidence on falsification**: High — metadata exactly describes the crate
compiled by the protocol test.

**Result**: Falsified. The alchemist's locked metadata experiment resolved
`ctor` 1.0.13.

______________________________________________________________________

### H2: The 1.x constructor does not emit the diagnostics dump

**Claim**: The fixture's `diagnostics_fixture` binary builds with `ctor` 1.0.13
but exits through libtest, rather than the `dump_steps` constructor, when
invoked with the diagnostics environment and `--dump-steps`.

**Plausibility**: High — H1 is falsified and `cargo-bdd` received no output.

**Prediction**: Directly running the newly built fixture binary with
`RSTEST_BDD_DUMP_STEPS=1 --dump-steps` produces empty stdout or an unrecognized
option error instead of JSON.

#### H2 Falsification Plan

| Step | Action                                                                                                                                              | Expected Negative Result                                                                           |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| 1    | Build the fixture's `diagnostics_fixture` test target, extract its executable from Cargo JSON, then run it with the diagnostics protocol arguments. | It exits successfully and writes a JSON registry dump, disproving a constructor-execution failure. |

**Tooling**: Cargo test JSON messages and the fixture executable.

**Confidence on falsification**: High — this is the precise process boundary
that `cargo-bdd` uses.

**Result**: Falsified. The alchemist built `diagnostics_fixture` and ran its
binary with the protocol arguments. It exited successfully and emitted JSON,
but every registry collection was empty.

______________________________________________________________________

### H3: The constructor precedes inventory registration

**Claim**: On Linux, `ctor` 1.x schedules `dump_steps` before the fixture's
`inventory::submit!` initializer. The constructor can therefore emit JSON but
cannot observe the fixture's steps or dump seed.

**Plausibility**: High — `inventory::submit!` registers through `.init_array`,
and H2 observed an empty registry before libtest.

**Prediction**: The fixture binary's `.init_array` places the `dump_steps`
constructor before the inventory submission initializer.

#### H3 Falsification Plan

| Step | Action                                                                                                                          | Expected Negative Result                                                                   |
| ---- | ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 1    | Inspect the direct-protocol fixture binary with `readelf` and `nm` to map `.init_array` entry order to its constructor symbols. | The inventory submission initializer precedes `dump_steps`, disproving the ordering claim. |

**Tooling**: `readelf`, `nm` and the already-built `diagnostics_fixture` binary.

**Confidence on falsification**: Medium — symbol names or linker
transformations may make entry identification inconclusive.

**Result**: Not falsified. `readelf` and `nm` show the `ctor` callback before
the fixture's `inventory::submit!` constructors in `.init_array`. On Linux,
the callback now uses `.init_array.99999`, which runs after the unprioritized
inventory registrations while still preceding libtest.

______________________________________________________________________

## Recommended Execution Order

1. **H1** — completed and falsified; the fixture resolves `ctor` 1.0.13.
2. **H2** — completed and falsified; the constructor emits an empty JSON dump.
3. **H3** — completed and not falsified; Linux constructor ordering is the
   root cause.

## Termination Criteria

- **Root cause identified**: H3 remains supported by the linked-binary
  inspection.
- **Resolved**: The exact diagnostics protocol produces registered data before
  libtest receives `--dump-steps`.

## Notes for Executing Agent

Run only the stated experiment. Do not update locks or source files.
