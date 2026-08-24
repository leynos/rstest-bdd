# Architectural decision record (ADR) 016: pin the nightly rustfmt toolchain

## Status

Accepted (2026-08-16): use the dated nightly formatter selected by
`FMT_TOOLCHAIN`, and require every workspace member to inherit the root lint
policy through `lints.workspace = true`.

## Date

2026-08-16

## Context and problem statement

The workspace's `.rustfmt.toml` enables unstable formatting options. Stable
`rustfmt` therefore ignores part of the repository's formatting contract and
can produce a different workspace-wide reformat from the one checked by CI. The
workspace also has a root Clippy, Rust, and Rustdoc policy that only applies to
crates which opt into workspace lint inheritance. Examples are workspace
members and must not silently fall outside that policy.

## Decision

Pin formatting to `nightly-2026-08-07`:

- `Makefile` sets `FMT_TOOLCHAIN ?= nightly-2026-08-07` and uses it for
  `make fmt` and `make check-fmt`.
- CI installs that toolchain with
  `rustup toolchain install nightly-2026-08-07 --profile minimal --component rustfmt`
  before running `make check-fmt`.
- Contributors and editor integrations use the same dated nightly formatter;
  stable `cargo fmt` is not a substitute.

Every workspace crate, including examples, declares:

```toml
[lints]
workspace = true
```

This keeps the root lint policy authoritative and prevents new workspace
members from silently weakening the repository's quality gates.

## Consequences

- A formatter-date change is a deliberate repository-wide tooling change and
  must update the Makefile, CI installation, and contributor instructions
  together.
- A contributor must install one additional Rust toolchain for formatting, but
  ordinary builds, tests, and lint commands continue to use the stable
  toolchain from `rust-toolchain.toml`.
- New workspace members inherit the root lint and Rustdoc policy by default;
  local exceptions must remain narrow and justified.
