# Architectural decision record (ADR) 0029: Run nested Cargo offline in tests

## Status

Accepted (2026-11-28): the test suite prefetches every manifest it spawns Cargo
against, every Cargo process it starts carries offline evidence, Cargo's
`[env]` table supplies a default for the processes it does not start, and the
Linux coverage lanes run the suite inside a private network namespace.

## Date

2026-11-28.

## Context and problem statement

A test that starts its own `cargo` — a nested fixture build, `cargo package`,
`cargo dylint`, a trybuild case — resolves dependencies, and may update the
crates.io index, while holding the package-cache lock that every repository on
the host shares. On a workstation running several agents, one repository's
nested Cargo can leave another repository's gate waiting for many minutes.
Issue 911 records the symptom: a test run that took minutes alone took far
longer beside a sibling build, with no test at fault.

Two things are wrong with "the cache is usually warm". The first is that warmth
is not a property the repository controls: a fresh CI cache, a new dependency,
or a lockfile bump all turn a nested build into a network request, and none of
them is visible in the test's own source. The second is that a suite meant to
demonstrate it needs no network has, until now, demonstrated nothing of the
sort — nothing stopped a child from reaching the registry, so a green run was
compatible with any amount of network use.

The question is how to make every Cargo process a test starts run offline, and
how to prove that none of them needs the network, without making ordinary
builds unusable and without depending on a single tool's behaviour.

## Decision drivers

- A test run must not need the network, and must fail loudly rather than
  quietly fetch when something is missing.
- The claim must be evidenced, not asserted: a green run should mean the
  network was available and unused, not merely that nobody noticed.
- A spawn this repository does not construct — a third-party crate's nested
  `cargo`, a build script — must be covered too, and cannot be edited.
- Ordinary builds, dependency updates and `cargo add` must keep working, so
  the default has to be overridable.
- The mechanism must not depend on lint suppression or on a tool that only
  one editor runs.

## Requirements

### Functional requirements

- Before any test binary starts, every manifest a test spawns Cargo against is
  fetched with `--locked --offline`, so a cold cache is a hard failure at the
  start of the run rather than a network request during it.
- Every Cargo spawn this repository constructs passes `--offline` (with
  `--locked` where it compiles) and sets `CARGO_NET_OFFLINE` on the child.
- A Cargo process started by a test but not constructed by this repository
  inherits `CARGO_NET_OFFLINE` from Cargo's `[env]` table.
- The Linux coverage lanes run the suite inside a private network namespace,
  so a green run proves the network was unnecessary.
- A negative control demonstrates the denial: a child that is not told to be
  offline still cannot reach the registry.

### Technical requirements

- The setup-script list is static, so a contract test keeps it equal to the
  manifests the repository actually spawns.
- The proof profile must inherit the default profile's timeouts, setup scripts
  and test groups, so the proof run is the same run plus a namespace.
- The compiler cache must stay reachable from inside the namespace.
- The `[env]` default must not be forced, so one command can opt out.
- Every new abstraction, and its permitted call sites, is recorded here rather
  than left implicit.

## Options considered

### Option A: pass `--offline` at each spawn and stop there

Rejected. It covers the spawns this repository writes, and nothing else. A
third-party crate's `cargo metadata`, a build script, or a helper that shells
out would still reach the network, and the suite would keep passing for the
wrong reason.

### Option B: rely on the `[env]` default alone

Rejected. It covers the third-party spawns, but it is a default rather than
evidence: a child can overwrite the variable, and no test would notice. It also
cannot demonstrate the network was unnecessary — a default only makes Cargo
refuse, it does not make the network absent.

### Option C: run the suite in a network namespace and stop there

Rejected. The namespace proves the endpoint, but without the prefetch the first
run on a cold cache would fail for want of packages rather than prove anything,
and the failure would be indistinguishable from a broken fixture.

### Option D: three cooperating layers

Accepted. A prefetch makes the packages present; explicit offline evidence at
every repository-owned spawn makes the intent local and reviewable; Cargo's
`[env]` default covers what the repository does not construct; and a namespace
proof profile turns the whole into a claim that can be tested. Each layer fails
closed on its own, and the negative control shows the last one is real.

## Decision outcome

### The prefetch

`make prefetch-fixture-deps` fetches the root workspace and every discovered
fixture manifest with `--locked`. Nextest runs one setup script per manifest
before any test binary starts, so a cold cache is a startup failure with a
named command. `offline_test_contract_test.py` keeps the literal setup-script
list equal to the manifests tests spawn, so a new fixture cannot be added
without its prefetch.

### Offline evidence at every repository-owned spawn

Each spawn is built by a pure function — `ui_clippy_command`,
`metadata_command`, `build_command`, `expand_probe_command`,
`dylint_probe_command`, `whitaker_lint_command` — so its argument list and
environment are asserted without running Cargo. The nested-Cargo harness drops
any inherited `CARGO_NET_OFFLINE` and pushes its own last, because the last
write is the one Cargo reads.

### The `[env]` default

`.cargo/config.toml` declares `CARGO_NET_OFFLINE = "true"` under `[env]` without
`force`. Cargo applies that table to every process it starts — rustc, build
scripts and the test binaries — and ignores it when resolving its own build, so
ordinary builds and dependency updates still reach the network. Leaving `force`
off keeps a one-command escape for a developer who needs to fetch something new.

### The proof profile

`profile.offline-proof` declares one wrapper rule that runs every test on Linux
through `unshare --net --map-current-user`. The Linux coverage lanes select it
with `NEXTEST_PROFILE`, so a green run means the suite completed with no
network to reach. The runner configuration relaxes Ubuntu's AppArmor
restriction on unprivileged user namespaces and then proves the wrapper starts,
so a lane that cannot relax it fails instead of passing tests the wrapper never
wrapped. The compiler cache moves to `SCCACHE_SERVER_UDS` because a private
namespace has its own loopback and sccache's default TCP port does not exist
inside it.

### The negative control

`a_denied_child_cannot_reach_the_registry` runs only under the proof profile.
It removes `CARGO_NET_OFFLINE` from a child and asks Cargo to fetch a manifest
naming a package that cannot exist, so the child must consult the index.
Denied, it reports `unable to update registry`; with the wrapper removed it
reports `no matching package` and the test fails.

## Known risks and limitations

- Windows has the prefetch and both environment layers, but no kernel denial
  the runner can grant, so the proof profile is Linux-only.
- trybuild and macrotest resolve without `--locked` because each generates its
  own manifest. Offline mode makes them fail closed on a missing package, but
  they can still select another cached version. That is existing behaviour and
  is out of scope here.
- A `cargo test` invoked from outside the repository with `--manifest-path`
  does not read this `.cargo/config.toml`, so it gets no `[env]` default.
  `offline_contract` reports that clearly.
- Concordat's `nested-cargo-offline-tests` rule reads this contract, and its
  own reader records a command passed to another function as unseen; a helper
  that hands a `Command` to a callee is reported as indeterminate even when it
  is offline.

## Architectural rationale

Offline mode is a statement about intent; a namespace is a statement about
capability. Keeping both is what makes the claim honest. A test can be offline
and still be wrong — it can depend on a package that happens to be cached — and
the prefetch is what fixes that. A test can be denied and still be slow,
because every nested build recompiles; the setup scripts are what fix that. And
a spawn can be either of those and still be invisible, because nothing in the
source says so; the pure builders are what fix that, by making the argument
list and environment something a test reads rather than something a reviewer
hopes for.

The layers are ordered by ownership. The parts this repository writes say
`--offline` themselves, because the intent belongs with the call. The parts it
does not write inherit a default, because a third-party crate cannot be edited.
The proof is a property of the run, not of any one spawn, so it lives in the
profile that starts the run.

## References

- [Issue 911](https://github.com/leynos/rstest-bdd/issues/911).
- [Execution plan: run Cargo children offline in integration tests][plan].
- [Developers' guide: nested Cargo in tests][developers-guide].
- [Testing strategy][testing-strategy].
- [Nextest wrapper scripts](https://nexte.st/docs/configuration/wrapper-scripts/).
- [Cargo configuration:
  `[env]`](https://doc.rust-lang.org/cargo/reference/config.html#env).

[plan]: ../execplans/issue-911-test-run-cargo-children-offline-in-integration-tests.md
[developers-guide]: ../developers-guide.md
[testing-strategy]: ../testing-strategy.md
