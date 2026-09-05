# Architectural decision record (ADR) 021: validate derived fixture lockfiles in every pull request

## Status

Accepted (2026-09-06): standalone fixture workspaces must keep their committed
`Cargo.lock` valid against every dependency input, and normal pull requests
must fail early when one is stale. The write-enabled repair workflow stays
restricted to Dependabot pull requests.

## Date

2026-09-06.

## Context and problem statement

Six standalone Cargo workspaces live inside this repository and commit their
own `Cargo.lock` outside the root workspace's resolution: the cargo-bdd minimal
fixture, the two feature-rebuild fixtures, the trybuild `ui_lints` harness, and
the two published-GPUI fixtures. Each declares its own `[workspace]` stanza, so
Cargo resolves it independently of the root workspace.

That independence is what makes the fixtures useful — the published-GPUI
fixtures must resolve `gpui` from crates.io rather than inherit the workspace's
`vendor/gpui` path dependency — but it also means the root lockfile does not
cover them. When ADR 020 added a dependency edge to `rstest-bdd`, both
published-GPUI fixture lockfiles went stale, and the failure surfaced on CI as
`cannot update the lock file ... because --locked was passed` inside
`make check-published-gpui`, far from the commit that caused it.

The repository already had repair automation:
`.github/workflows/refresh-derived-fixture-lockfiles.yml` regenerates the two
published-GPUI fixture lockfiles and pushes them back to the pull-request
branch. It runs only on `pull_request_target`, and its job is gated to
`dependabot[bot]`, because a workflow that pushes to contributor branches from
a context holding write credentials would let arbitrary pull-request code run
with write access. That restriction is correct and stays. Its consequence is
that a stale fixture lockfile is repaired automatically only for Dependabot;
every other contributor discovers the staleness from a failing `--locked`
build.

The failure mode is systemic. It recurs whenever any commit changes a shared
dependency input — a workspace member manifest, a fixture's registry
requirements, or a path dependency — and nothing detects the resulting
staleness before CI's `--locked` steps run.

## Decision drivers

- **Detection must be cheap and total.** Compiling every fixture on every
  change is wasteful; the check must cover every tracked standalone lockfile
  without a build.
- **The registry must not drift.** Separate, hand-maintained lists in the
  `Makefile` and the workflows have already diverged in spirit: the refresh
  workflow knew only two lockfiles while the repository tracks six.
- **The write boundary must not move.** `pull_request_target` must never push
  to a normal contributor branch; the Dependabot-only gate is a security
  property, not a convenience.
- **Failures must be actionable.** A stale-lockfile error must name the
  refresh target so a contributor can fix the pull request without
  archaeology.

## Decision outcome

1. The `Makefile` owns a single registry, `DERIVED_LOCKFILE_MANIFESTS`, of
   every standalone workspace manifest whose `Cargo.lock` is committed.
2. `make check-derived-lockfiles` validates each registered manifest with
   `cargo metadata --locked --manifest-path <manifest> --format-version 1`,
   which proves lockfile freshness without compiling the fixture.
3. `make update-derived-lockfiles` regenerates every registered lockfile,
   seeding the feature-rebuild fixtures from the minimal fixture's lockfile so
   the three keep sharing one dependency resolution and its compiled units.
4. `make derived-lockfile-paths` prints the registered lockfiles; the refresh
   workflow derives its commit surface from it instead of restating the list.
5. Staging is a prerequisite only for manifests that resolve generated local
   package paths (today, the published-GPUI end-to-end fixture), so plain
   in-repository path dependencies need no packaging step first.
6. Normal CI runs `make check-derived-lockfiles` as an early step on the Linux
   tools lane, with read-only permissions, so a contributor pull request fails
   with the stale-lockfile message instead of a downstream `--locked` error.
7. The refresh workflow's trigger, permissions, and `dependabot[bot]` gate are
   unchanged.

## Rationale

`cargo metadata --locked` is the same staleness oracle Cargo itself applies
before any `--locked` build: it refuses to run when the lockfile no longer
matches the manifest inputs, and it does so without resolving or compiling
anything. Checking every registered fixture with it makes the gate cheap and
complete.

Centralizing the list in the `Makefile` removes the drift surface. The check,
the refresh target, the workflow's commit surface, and the contract tests all
read the same registry, so adding a fixture means adding one line. The
contract test `test_registry_covers_every_tracked_standalone_lockfile` closes
the loop from the other side: it enumerates every tracked `Cargo.lock` whose
manifest declares a `[workspace]` stanza and fails when one is missing from
the registry, so a new fixture cannot be committed unregistered.

Early CI detection and Dependabot-only repair are complementary. The check
gives every contributor the signal and the fix command; the workflow silently
repairs Dependabot's pull requests, whose manifest bumps are the lockfile
driving changes most of the time. Neither needs to hold write credentials for
the other's branch.

## Options considered

- **Compile every fixture on each pull request.** Rejected: it multiplies CI
  cost for information `cargo metadata --locked` already provides, and the
  existing fixture-specific steps already compile the fixtures that need
  compiling.
- **Broaden the refresh workflow to all pull requests.** Rejected:
  `pull_request_target` checks out the base repository by default, and running
  a pull request's own Makefile from a write-enabled context would execute
  untrusted code with write credentials. The `dependabot[bot]` gate stays.
- **Restate the lockfile list in the workflow.** Rejected: the workflow's list
  and the Makefile's would drift, which is the failure this decision exists to
  prevent; deriving the commit surface from `make derived-lockfile-paths`
  keeps one owner.
- **Filter the CI check by changed paths.** Considered and declined: a
  lockfile can be staled by a change to a path dependency's transitive inputs,
  which no `paths:` filter tracks precisely, and the check costs seconds.

## Consequences

- A pull request that changes a dependency input of any registered fixture
  fails fast unless its committed lockfile is refreshed and committed with the
  change.
- `make check-published-gpui`, `make e2e-published-gpui`, the feature-rebuild
  suite, and the trybuild UI suites keep their existing `--locked`
  invocations; this decision adds detection, never unlocks a build.
- Adding a standalone fixture workspace now carries a registration duty,
  enforced by the workflow-contract tests.
- Dependabot pull requests continue to receive automatic lockfile refreshes
  through the existing write-enabled workflow, whose permissions and actor
  gate are unchanged.

## References

- `Makefile`: `DERIVED_LOCKFILE_MANIFESTS` registry and the
  `check-derived-lockfiles`, `update-derived-lockfiles`, and
  `derived-lockfile-paths` targets.
- `.github/workflows/refresh-derived-fixture-lockfiles.yml`: Dependabot-only
  repair automation.
- `.github/workflows/ci.yml`: early "Check derived fixture lockfiles" step.
- `tests/workflow_contracts/derived_fixture_lockfiles_test.py`: registry
  anti-drift and CI-coverage contracts.
- ADR 020, whose `tracing` edge to `rstest-bdd` surfaced the failure mode.
