# Architectural decision record (ADR) 025: Adopt OrthoConfig for the `cargo-bdd` command-line interface

## Status

Accepted (2026-10-25): `crates/cargo-bdd` layers its per-subcommand settings
through `ortho_config` v0.10.0, while every library crate in the workspace
remains free of the dependency.

## Date

2026-10-25.

## Context and problem statement

`cargo-bdd` is the diagnostic front end: `steps`, `unused`, `duplicates` and
`skipped`. Every setting it has is a command-line flag. An operator who wants a
repository to always render skip reasons, or a CI lane that always wants the
JSON payload, has to repeat the flag at each call site, and there is nowhere to
record the choice once. Cargo's dependency form, `cargo bdd steps`, is also
rejected outright, because the parser has no slot for the `bdd` token Cargo
passes to the binary.

Two constraints shape any answer. First, the workspace advertises Rust 1.88 for
its libraries and a configuration layer is a new dependency in the build graph,
so the floor must not move for library consumers. Second, existing behaviour
with no configuration present is part of the tool's contract: the same flags,
defaults, output, exit statuses and streams.

OrthoConfig v0.10.0 supplies layered configuration over a clap parser, a
subcommand namespace convention, and a Cargo external-subcommand helper. It
also requires Rust 1.89.

## Decision drivers

- A project must be able to pin a diagnostic preference once, in a file it can
  review, instead of at every call site.
- A CI lane must be able to set the same preference in one place without
  editing the file.
- Anything the user actually types on the command line must still win.
- Both Cargo's `cargo bdd …` form and the direct `cargo-bdd …` form must work,
  and neither may change today's output when nothing is configured.
- Library crates must not inherit the application's dependency or its higher
  MSRV.

## Options considered

### Option A: Handwritten environment and file reading in `cargo-bdd`

Rejected. It reimplements precedence, discovery, subcommand namespacing, and
error reporting for four booleans, and every later setting would repeat the
work. It also has no answer for the Cargo entry shape.

### Option B: `clap`'s own `env` feature

Rejected. It covers environment variables and nothing else: no file tier, no
precedence between the two, and no Cargo entry shape.

### Option C: OrthoConfig v0.10.0 in `cargo-bdd` only

Accepted.

### Option D: OrthoConfig in a shared configuration crate used by all members

Rejected. It would put the dependency and the 1.89 floor into library crates
that have no configuration surface, which is precisely what the workspace's
MSRV promise rules out.

## Decision outcome

`crates/cargo-bdd` depends on `ortho_config = "0.10.0"` with default features
enabled, from the workspace dependency table. No other crate depends on it,
directly or through a feature. `crates/cargo-bdd` alone raises its
`rust-version` to `1.89`; the workspace stays at `1.88`.

The configurable settings are the per-subcommand booleans that already exist:
`--skipped` and `--json` for `steps`, and `--reasons` and `--json` for
`skipped`. Nothing else gains a configuration surface.

Configuration files are TOML. With default features the crate discovers only
`.toml`, so no other format is offered. Per-subcommand values live in a
`[cmds.<subcommand>]` table, which the subcommand loader selects from `clap`'s
command name. On Unix and Redox, it merges `~/.cargo_bdd.toml`, the first
existing XDG `cargo_bdd/config.toml`, and `./.cargo_bdd.toml` in that order. On
other platforms it checks the home dotfile, the native `cargo_bdd/config.toml`
directory (under `%APPDATA%` on Windows), and the working-directory dotfile.
Missing optional files contribute nothing; found files that are unreadable or
malformed return an error. The subcommand loader does not expose a
`--config-path` option or `CARGO_BDD_CONFIG_PATH` selector.

The environment prefix is `CARGO_BDD_`, and a subcommand setting is spelled
`CARGO_BDD_CMDS_<SUBCOMMAND>_<FIELD>`, for example
`CARGO_BDD_CMDS_STEPS_SKIPPED`. There are no legacy variables or files to
honour, because the tool has never been configurable; the prefix is introduced
whole.

Precedence is defaults, then configuration files, then environment, then the
command line. Every configurable field is marked
`#[ortho_config(cli_default_as_absent)]` so that a flag the user did not type
does not outrank a value from a file or the environment. The matching `Default`
implementation agrees with the clap default, because the subcommand merge
starts from `Default`.

Help text and parse failures stay English. No localizer is installed, so
OrthoConfig's `NoOpLocalizer` remains in force and the wording of help and
errors does not move.

There are no configuration profiles and no `--profile` flag. OrthoConfig
v0.10.0's subcommand path does not load profiles, and inventing a
repository-side profile tier would mean hand-writing a layer upstream does not
provide.

For the injected Cargo form, the entry point uses
`ortho_config::cargo::external_subcommand` when `bdd` is the first argument.
The existing flat `cargo-bdd <subcommand>` form continues through the original
parser. Both paths produce the same command arguments and use one dispatch
function. This split preserves the flat parser's help, usage and error output;
the helper's synthetic `cargo` parent would otherwise change that output.

## Known risks and limitations

- A configuration file discovered in the user's home directory or an XDG
  directory affects runs the operator may have forgotten about. The users'
  guide names the discovery locations so the effect is discoverable.
- The generated `--config-path` flag belongs to OrthoConfig's own hidden CLI
  struct and is not consulted by the subcommand loaders. `cargo-bdd` therefore
  does not offer `--config-path`; advertising a flag that silently does nothing
  would be worse than omitting it.
- OrthoConfig does not add `deny_unknown_fields`, so an unknown key in a
  configuration file is ignored rather than reported. A typo is silently
  ineffective.
- The Rust 1.89 floor applies to building `cargo-bdd`. A workspace build now
  needs 1.89 even though the libraries still need only 1.88.
- OrthoConfig v0.10.0 does not build with `default-features = false`
  (leynos/ortho-config#627), so the dependency cannot be trimmed in future
  without an upstream change.

## Architectural rationale

`cargo-bdd` is a driving adapter. It is the only member of the workspace that
has a command line, so it is the only member that needs a command-line
configuration layer. Keeping the dependency at that edge preserves the
dependency rule: the libraries describe behaviour and never learn where a
setting came from, while the adapter resolves settings and hands validated
values to the domain. The MSRV split follows the same boundary — the higher
floor is charged to the crate that takes the dependency, not to the consumers
who do not.
