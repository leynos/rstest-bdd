# Architectural decision record (ADR) 023: Adopt the shared CV-005 contract library

## Status

Accepted (2026-10-08): rstest-bdd runs the shared `cv005-contracts` check from
a pinned commit instead of keeping its own copy of the CV-005 contract.

## Date

2026-10-08.

## Context and problem statement

rstest-bdd keeps CodeScene coverage owned by `main`, the estate rule CV-005.
Its repository-local copy of the contract ran as test modules, and every time
the rule gained a clause the copy had to be edited again, with the same edit
made in each repository that held a copy.

## Decision drivers

- One definition of the rule that every repository shares and that its own
  suite proves against breaching fixtures.
- A fix to the rule should reach a repository as a reviewed change, not as a
  silent upgrade.
- Anything the library does not know should stay a small local test.

## Options considered

### Option A: Keep the local copy

Rejected. The copy drifts from the rule, and each clause costs an edit per
repository.

### Option B: Vendor the library

Rejected. A vendored copy is the local copy again, with the same drift.

### Option C: Run the library from a floating branch

Rejected. A floating source changes what the gate checks without a review.

### Option D: Run the library from a pinned commit

Accepted.

## Decision outcome

`make test-workflow-contracts` runs `cv005-contracts check --repository .`
through `uv tool run`, from the full commit named by `CV005_CONTRACTS_REF` in
the `Makefile`. `.github/cv005.toml` holds this repository's parameters. CI
runs the target in its own step and `make all` includes it.
`tests/workflow_contracts/cv005_wiring_test.py` holds the local wiring: it
fails if the pin is not a full commit, if the target stops running the pinned
checker with `check --repository .` under Python 3.14, if the repository
parameter is wrong, if `make all` drops the target, or if CI stops running it.
The pull-request lanes and the publisher's baseline jobs are paired in
`.github/cv005.toml` (`[[pairing]]` tables), and `RUST_TOOLCHAIN` is declared
as the one legitimate difference between a lane and its publisher job. The
coverage-lane pairing helpers (`coverage_lane_pairs.py`) and the strict
workflow loader stay local, because the publisher-setup and cache contracts
read through them.

## Known risks and limitations

- A fix to the rules reaches this repository only as a pin bump.
- The target needs `uv`, which fetches the Python 3.14 the library runs under.
- The library checks clauses and shape. It does not run the CodeScene upload,
  so the publisher run on the merge commit remains the proof of the upload.

## Architectural rationale

Ownership of the rule moves to the repository that owns the shared actions,
while this repository keeps the choice of when to adopt a change.
