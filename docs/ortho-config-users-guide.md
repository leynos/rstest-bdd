# OrthoConfig user's guide

Configuration should not be the hardest part of writing a command-line
application. OrthoConfig describes settings as a Rust struct, then loads that
struct from defaults, a configuration file, environment variables, and
command-line arguments.

This guide is for application authors using OrthoConfig v0.10.0. It starts with
a small working CLI and grows it one practical task at a time. Each section
builds on the ones before it, so reading in order works well, but every section
also stands on its own. Stop as soon as the application has what it needs.

Every example in this guide is compiled, run, or parsed by the project's test
suite, so the code shown here is known to work with the release it describes.

## Find the right section

The sections run from first steps to specialist tooling. Use this table to jump
straight to a task.

| Stage   | Task                                                   | Section                                                                                                                                                           |
| ------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Start   | Add the crate and build a layered CLI                  | [Install OrthoConfig](#install-orthoconfig), [first CLI](#build-the-first-layered-cli)                                                                            |
| Start   | Read settings from files and environment variables     | [See the configuration surface](#see-the-configuration-surface)                                                                                                   |
| Start   | Turn a configured boolean off for one run              | [Switch a boolean off](#switch-a-boolean-off-from-the-command-line)                                                                                               |
| Shape   | Combine lists and maps across sources                  | [Merge lists and maps](#merge-lists-and-maps)                                                                                                                     |
| Shape   | Choose the flag and filenames used for discovery       | [Make discovery match the application](#make-discovery-match-the-application)                                                                                     |
| Shape   | Offer `--profile staging` style overlays               | [Switch between named profiles](#switch-between-named-profiles)                                                                                                   |
| Shape   | Layer system, user, and project files                  | [Control file layers](#control-which-configuration-files-become-layers)                                                                                           |
| Shape   | Read YAML or JSON5                                     | [Enable another file format](#enable-another-file-format)                                                                                                         |
| Grow    | Give subcommands their own settings                    | [Subcommands](#give-each-subcommand-its-own-settings)                                                                                                             |
| Grow    | Ship a `cargo <name>` tool                             | [Cargo external subcommands](#cargo-external-subcommand-entry-points)                                                                                             |
| Grow    | Report errors and translate help                       | [Errors](#handle-errors-at-the-application-boundary), [localization](#localize-help-and-parse-failures-together)                                                  |
| Test    | Load configuration without touching the process        | [Test discovery](#test-discovery-without-changing-the-process-environment), [resolve without process state](#resolve-discovery-and-merging-without-process-state) |
| Operate | Observe discovery and merging in production            | [Add production diagnostics](#add-production-diagnostics)                                                                                                         |
| Operate | Depend on OrthoConfig under another name               | [Use an aliased dependency](#use-an-aliased-dependency)                                                                                                           |
| Publish | Generate man pages, PowerShell help, and agent context | [Generate help](#generate-help-from-the-same-metadata)                                                                                                            |
| Publish | Describe commands safely to automation                 | [Agent contract](#offer-a-compact-contract-to-automation), [behaviour](#declare-behaviour-metadata-for-agents)                                                    |
| Upgrade | Move from an earlier release                           | [Upgrade from an earlier release](#upgrade-from-an-earlier-release)                                                                                               |

_Table 1: Tasks covered by this guide and where to find them._

## Install OrthoConfig

OrthoConfig needs Serde to turn merged values into the application's
configuration type. Add `clap` when the application defines its own command or
subcommand parser:

<!-- tested-example: guide-install -->
```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
ortho_config = "0.10.0"
serde = { version = "1.0", features = ["derive"] }
```

The default features support TOML and the JSON-backed merge machinery used by
the derive. Optional `json5`, `yaml`, and `metrics` features are covered later.
OrthoConfig re-exports its derive macros, so a separate `ortho_config_macros`
dependency is not needed.

## Build the first layered CLI

Start with one struct. The `prefix` is used for environment variables and for
the default file-discovery names. A trailing underscore is conventional and
keeps names such as `ACME_PORT` easy to read; OrthoConfig adds one when it is
missing.

<!-- tested-example: guide-first-cli -->
```rust
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_")]
struct Config {
    #[ortho_config(default = String::from("127.0.0.1"))]
    host: String,

    #[ortho_config(default = 8080, cli_short = 'p')]
    port: u16,

    #[ortho_config(default = String::from("info"))]
    log_level: String,
}

fn main() -> OrthoResult<()> {
    let config = Config::load()?;
    println!(
        "host={} port={} log_level={}",
        config.host, config.port, config.log_level
    );
    Ok(())
}
```

That one definition provides three spellings for each field:

| Rust field  | Command line     | Environment      | TOML        |
| ----------- | ---------------- | ---------------- | ----------- |
| `host`      | `--host`         | `ACME_HOST`      | `host`      |
| `port`      | `--port` or `-p` | `ACME_PORT`      | `port`      |
| `log_level` | `--log-level`    | `ACME_LOG_LEVEL` | `log_level` |

_Table 2: Rust fields and their command-line, environment, and TOML names._

Values are merged from lowest to highest precedence:

1. `#[ortho_config(default = ...)]` values;
2. configuration files;
3. environment variables; and
4. command-line arguments.

This means a checked-in file can provide team defaults, an environment variable
can adapt them for a deployment, and a one-off CLI option can override both.
[Named profiles](#switch-between-named-profiles) add one optional tier between
files and environment variables; nothing else changes this order.

## See the configuration surface

Different sources suit different moments. Start with durable team settings in
`.acme.toml`, in the directory the application runs from:

<!-- tested-example: guide-file -->
```toml
host = "0.0.0.0"
port = 9000
log_level = "debug"
```

At deployment time, an environment variable can change the host without
rewriting the file. For a one-off run, a CLI option can change the port again:

<!-- tested-example: guide-file-run -->
```console
$ ACME_HOST=api.internal cargo run -- --port 3000
host=api.internal port=3000 log_level=debug
```

The result shows all three surfaces working together: `log_level` comes from
TOML, `ACME_HOST` supplies `host`, and `--port` wins for `port`. The command
uses POSIX shell syntax; in PowerShell, set `$env:ACME_HOST = "api.internal"`
before running the same Cargo command.

### Where files are found

Discovery tries candidates in this order and uses the first one that loads:

1. a path given with the generated `--config-path` option;
2. a path named by the `ACME_CONFIG_PATH` environment variable;
3. the XDG configuration directories (`$XDG_CONFIG_HOME`, then each entry of
   `$XDG_CONFIG_DIRS`, falling back to `/etc/xdg`), looking for
   `acme/config.toml` and `.acme.toml`;
4. `%APPDATA%` on Windows;
5. the home directory, including `~/.config/acme/config.toml` and
   `~/.acme.toml`; and
6. `.acme.toml` in the project directory, which defaults to the current working
   directory.

Explicitly requested files are required: a missing `--config-path` is an error
rather than a silent fallback. The `--config-path` option is hidden from
`--help` by default;
[make discovery match the application](#make-discovery-match-the-application)
shows how to rename it and make it visible.

### Share settings between files

Configuration files can contain an `extends` entry naming a parent file.
Relative paths are resolved from the file that declares them, and parent layers
are merged before the child, so the child overrides only what it restates.
OrthoConfig reports a missing parent with its absolute path and the referencing
file, so the failure is actionable.

## Merge lists and maps

TOML naturally handles lists and nested values. An application can add
collections to its configuration struct and let each source contribute to them:

<!-- tested-example: guide-collections -->
```rust
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Worker {
    name: String,
    concurrency: u8,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_")]
struct Config {
    #[ortho_config(skip_cli, default = Vec::new())]
    workers: Vec<Worker>,

    #[ortho_config(skip_cli, default = BTreeMap::new())]
    labels: BTreeMap<String, String>,
}

fn main() -> OrthoResult<()> {
    let config = Config::load()?;
    for worker in &config.workers {
        println!("worker={} concurrency={}", worker.name, worker.concurrency);
    }
    for (key, value) in &config.labels {
        println!("label {key}={value}");
    }
    Ok(())
}
```

The matching `.acme.toml` uses an array of tables for the workers and an
ordinary table for the labels:

<!-- tested-example: guide-collection-file -->
```toml
[[workers]]
name = "queue-a"
concurrency = 4

[[workers]]
name = "queue-b"
concurrency = 2

[labels]
region = "eu-west"
tier = "worker"
```

Running the program prints both workers and both labels. `skip_cli` keeps these
structured fields out of the command line, because `Worker` has no natural flag
spelling; files and environment variables still supply them.

When more than one source supplies a collection, a field-level `merge_strategy`
decides how they combine:

| Field type              | Default  | Alternative | Effect of the default                                      |
| ----------------------- | -------- | ----------- | ---------------------------------------------------------- |
| `Vec<T>`                | `append` | `replace`   | Higher-precedence items are added after lower-layer items  |
| Maps such as `BTreeMap` | `keyed`  | `replace`   | Entries merge by key; untouched lower-layer entries remain |

_Table 3: Collection merge strategies._

Write `#[ortho_config(merge_strategy = "replace")]` when a higher-precedence
source should swap the whole collection rather than add to it. Choose the
policy deliberately when operators may combine file, environment, and CLI
values; an unsupported pairing, such as `keyed` on a `Vec`, is a compile-time
error.

## Switch a boolean off from the command line

A boolean field has three meaningful states: absent, explicitly `true`, and
explicitly `false`. The last one matters when a file or environment variable
supplies `true` and the operator wants to override it for a single run.

Generated boolean flags accept an optional value, so the flag does the work
without a separate negation option:

<!-- tested-example: guide-boolean-override -->
```rust
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_")]
struct Config {
    /// Adds an exclamation mark to the greeting.
    #[ortho_config(default = false)]
    excited: bool,
}

fn main() -> OrthoResult<()> {
    let config = Config::load()?;
    println!("excited={}", config.excited);
    Ok(())
}
```

The spellings behave as follows:

| Command line      | File or environment | Result  | Why                                     |
| ----------------- | ------------------- | ------- | --------------------------------------- |
| _(omitted)_       | `excited = true`    | `true`  | The lower-precedence source still wins  |
| `--excited`       | _(none)_            | `true`  | A bare flag means `true`                |
| `--excited=true`  | _(none)_            | `true`  | The explicit spelling of the same value |
| `--excited=false` | `excited = true`    | `false` | The command line clears the lower layer |

_Table 4: Boolean flag spellings and the resulting value._

Use `=` when supplying a value. A bare `--excited` never consumes the argument
that follows it, so another option may follow it without being swallowed. The
space-separated form, `--excited false`, is rejected instead: without that rule
the flag would swallow the token after it, so `--excited notes.txt` would try
to read `notes.txt` as the boolean value rather than as a positional operand.

Omitting the flag is not the same as passing `--excited=false`. An omitted flag
leaves the value absent, which lets a configuration file or environment
variable supply it. Passing `--excited=false` always produces `false`, even
when a lower-precedence file or environment value supplies `true`. Use the
explicit form when the intent is to override, and omission when the intent is
to defer.

An explicit value always wins over the lower layers, including when it happens
to repeat the field's own default. `--port 8080` therefore overrides
`ACME_PORT=9000`, just as `--excited=false` overrides `ACME_EXCITED=true`. Use
omission, not a repeated default, when the intent is to let the lower layer
supply the value.

Every boolean field behaves this way, including `Option<bool>` fields. A field
declared as `Option<bool>` distinguishes "no value was supplied anywhere" from
an explicit `false`, which matters when a
[subcommand merge](#give-each-subcommand-its-own-settings) must tell a missing
value from a deliberate one.

The generated documentation metadata describes the same surface. Boolean fields
report `takes_value: true` with a `BOOL` value name, `true` and `false` as
possible values, and `value_optional: true`. Man pages render the flag as
`--excited[=BOOL]`, with the bracketed placeholder italicized.

## Make discovery match the application

Application names do not need to bend around OrthoConfig's defaults. Put the
discovery contract beside the struct when the public flag or filenames are part
of the CLI design:

<!-- tested-example: guide-discovery -->
```rust
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "ACME_",
    discovery(
        app_name = "acme-server",
        config_file_name = "server.toml",
        dotfile_name = ".acme-server.toml",
        project_file_name = ".acme-server.toml",
        config_cli_long = "config",
        config_cli_short = 'c',
        config_cli_visible = true
    )
)]
struct Config {
    #[ortho_config(default = 8080)]
    port: u16,
}

fn main() -> OrthoResult<()> {
    let config = Config::load()?;
    println!("port={}", config.port);
    Ok(())
}
```

This application accepts `--config` and `-c`, shows them in `--help`, reads
`ACME_CONFIG_PATH`, looks for `.acme-server.toml` in project locations, and uses
`server.toml` in platform configuration directories. Keeping these choices in
the derive also exposes them through `OrthoConfigDocs`, so generated help
describes the same contract.

### Handle every `load_first` outcome

Applications that drive discovery by hand use `ConfigDiscovery` directly.
`ConfigDiscovery::load_first` distinguishes three outcomes. `Ok(Some(...))`
contains the first successfully parsed candidate. `Ok(None)` means discovery
had no candidates to try. `Err(...)` means candidates existed but none loaded
successfully; surface or map that error rather than treating it as absence:

<!-- tested-example: guide-load-first-outcomes -->
```rust
use ortho_config::{ConfigDiscovery, OrthoResult};

fn load_discovered_config(discovery: &ConfigDiscovery) -> OrthoResult<()> {
    match discovery.load_first() {
        Ok(Some(_config)) => {
            // Merge or deserialize the discovered Figment value.
            println!("discovery=loaded");
            Ok(())
        }
        Ok(None) => {
            // Continue with application defaults.
            println!("discovery=absent");
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn main() -> OrthoResult<()> {
    let discovery = ConfigDiscovery::builder("acme").build();
    load_discovered_config(&discovery)
}
```

## Switch between named profiles

Profiles are named bundles of configuration values kept inside the
configuration file, such as a `staging` profile that raises the log level and
moves the port. Profile support is opt-in: add `profiles` to the struct-level
attribute.

<!-- tested-example: guide-profiles -->
```rust
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_", profiles)]
struct Config {
    #[ortho_config(default = String::from("info"))]
    log_level: String,

    #[ortho_config(default = 8080)]
    port: u16,
}

fn main() -> OrthoResult<()> {
    let outcome = Config::load_with_profile()?;
    if let Some(selected) = outcome.selection().first() {
        println!("profile={} from {}", selected.name, selected.source);
    }
    let config = outcome.into_config();
    println!("log_level={} port={}", config.log_level, config.port);
    Ok(())
}
```

Declare each profile as a `[profile.<name>]` table in `.acme.toml`. JSON5 and
YAML files use the equivalent `profile.<name>` key path:

<!-- tested-example: guide-profiles-file -->
```toml
log_level = "info"

[profile.staging]
log_level = "debug"
port = 9000
```

Select a profile with the generated `--profile` flag or the `ACME_PROFILE`
environment variable. The selector variable is always the struct's `prefix`
followed by `PROFILE`:

<!-- tested-example: guide-profiles-run -->
```console
$ cargo run
log_level=info port=8080
$ cargo run -- --profile staging
profile=staging from the --profile flag
log_level=debug port=9000
$ ACME_PROFILE=staging cargo run -- --port 3000
profile=staging from the selector environment variable
log_level=debug port=3000
```

The last run shows the precedence rule: a profile is still configuration-file
data, so environment variables and command-line arguments outrank it. With
profiles enabled, precedence from lowest to highest is:

1. application-defined defaults;
2. configuration files;
3. the selected profile, with one overlay per file in the resolved chain that
   defines it, base file first;
4. environment variables; and
5. command-line arguments.

`load_with_profile()` returns a `ProfileLoadOutcome`. Borrow the configuration
with `config()`, take it with `into_config()`, and read the selection with
`selection()`, which holds zero or one `SelectedProfile` today. Each entry
carries the profile `name` and its `source`, either `ProfileSource::Flag` or
`ProfileSource::Environment`. `load_with_profile_from_iter` accepts explicit
arguments, and the `_with_sources` variants accept
[injected environment sources](#resolve-discovery-and-merging-without-process-state).
The ordinary `load()` and `load_from_iter()` keep working on an opted-in
struct when the application does not need to report the selection.

### Selection rules

- The `--profile` flag beats `ACME_PROFILE` whenever both are present. An empty
  flag value also suppresses the environment fallback.
- An empty selector, such as `ACME_PROFILE=""` left behind by a stray `export`,
  means "no profile" rather than an invalid name.
- The name `default` is reserved and also means "no profile"; defining
  `[profile.default]` fails with `OrthoError::ReservedProfileName`.
- Profile names are case-sensitive and must match `[A-Za-z0-9_-]+`.
- A selector value that is not valid Unicode fails the load rather than
  silently falling back to the unprofiled configuration.

### Profile errors

Selecting a profile that no file defines fails with
`OrthoError::UnknownProfile`. The error names the profile, says whether it came
from the flag or the environment variable, and lists up to 16 available names
in sorted order. When no configuration file was found at all, or files exist
but define no profiles, the error says which, so a leaked `ACME_PROFILE` is not
mistaken for a missing file.

Inside a profile body, the keys `cmds` and `inherits` are forbidden and fail
with `OrthoError::ProfileForbiddenKey`: subcommand configuration ignores
profiles, and `inherits` is reserved for future single-parent inheritance.

Opting in reserves the `profile` name across all three sources. The `profile`
root key is taken out of file tables, `ACME_PROFILE` is not merged as a
setting, and the generated flag is not part of the merged command-line values.
A field that claims the `profile` key, the `--profile` flag, or the
`ACME_PROFILE` variable on an opted-in struct is a compile-time error. Structs
that do not opt in keep the four-tier precedence, gain no `--profile` flag, and
treat a `[profile.*]` table like any other unknown key.

## Control which configuration files become layers

File discovery normally stops at the first candidate that loads, and that
remains the default: `AutomaticMode::FirstWins` scans candidates most-preferred
first and stops at the first file that parses. The opt-in
`AutomaticMode::StackScopes` composes every applicable file that loads instead,
so a system-wide file can provide a base that user and project files refine.

Three ideas order that stack:

- a **scope** is `DiscoveryScope::System`, `DiscoveryScope::User`, or
  `DiscoveryScope::Project`, covering automatic locations such as `/etc/xdg`,
  the home and application-data folders, and the project directory;
- **scope order is precedence order**: scopes are applied in the order given,
  and the default `system`, `user`, `project` order lets project files override
  user files, which in turn override system-wide files; and
- within a scope the least-preferred location is applied first and the
  most-preferred last, so the location that first-wins would have selected
  still wins, while every lower-preferred location contributes a base layer for
  the keys it alone sets.

That reversal is deliberate, because the two orderings run in opposite
directions: the candidate list is a _preference_ order, while a composed layer
list is a _precedence_ order. Walking candidates in reverse is what lets one
rule, "later applied wins", serve both. Emitting them in preference order
instead would let a fallback such as `~/.demo.toml` silently override
`$XDG_CONFIG_HOME/demo/config.toml` as soon as a second location started
loading.

Canonical paths are de-duplicated across scopes, keeping the earliest position
in application order, so a file reachable from two scopes contributes one layer
rather than two.

### Configure the policy from the derive

The policy keys live in the same `discovery(...)` attribute as the other
discovery settings. Supplying any of them switches the generated loader onto
the policy path:

<!-- tested-example: guide-scoped-discovery -->
```rust
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "ACME_",
    discovery(
        app_name = "acme",
        env_vars = ["ACME_CONFIG_PATH", "ACME_LEGACY_CONFIG"],
        explicit_mode = "optional",
        automatic_mode = "stack_scopes",
        scope_order = ["system", "user", "project"]
    )
)]
struct Config {
    #[ortho_config(default = 8080)]
    port: u16,
}

fn main() -> OrthoResult<()> {
    let config = Config::load()?;
    println!("port={}", config.port);
    Ok(())
}
```

With neither selector variable set and no candidate file present, the example
prints `port=8080`: the stacked scopes contribute nothing and the default
survives. The policy keys are:

- `env_vars`: an array of environment variable names, tried in order, so that
  an older name still selects a file. It is mutually exclusive with `env_var`.
- `explicit_mode`: `"required_exclusive"` (the default) or `"optional"`.
- `automatic_mode`: `"first_wins"` (the default) or `"stack_scopes"`.
- `scope_order`: an array drawn from `"system"`, `"user"`, and `"project"`,
  applied in the order written.
- `project_root_from`: the name of a `PathBuf` or `Option<PathBuf>` CLI field
  whose value supplies the project root described below.

Every part of this section is opt-in. Discovery stays first-wins and every
selected path stays required unless one of the keys above says otherwise, so
applications that do not use them need no change.

### Select a file explicitly

Behind the derive keys sits `ConfigFilePolicy`, which places an ordered chain
of explicit selectors above automatic discovery. Applications that build the
policy by hand start from `ConfigFilePolicy::from_builder(...)`. Each rung is a
`ConfigPathSelector`:

- `ConfigPathSelector::cli(path)` takes an optional path already parsed by a
  CLI adapter, such as the generated `--config-path` field;
- `ConfigPathSelector::env("VAR")` names an environment variable to read; an
  empty value counts as unset;
- `.label("primary")`, applied to a rung, replaces the diagnostics label it
  would otherwise carry, such as the variable name; and
- `.legacy_alias()`, applied to a rung, marks it as an older, compatibility
  variable name.

Rungs resolve in order and the first that yields a path wins. A winning rung
suppresses the rungs after it and automatic probing entirely, so a file named
on the command line is never joined by automatically discovered files. When no
rung yields a path, the policy falls back to automatic discovery over
`scope_order`.

`ExplicitMode` decides what a selected path means:

- `ExplicitMode::RequiredExclusive`, the default, reports a selected file that
  is missing or unreadable as a failure and loads no fallback; and
- `ExplicitMode::Optional` accepts an absent selected path while still
  suppressing automatic discovery.

Resolution returns a `FileLayerOutcome`, a replayable value reporting the
winning selection through `selection()`, non-fatal defects through
`reportable_errors()`, and the scopes that actually contributed layers through
`origins()`. Under `AutomaticMode::FirstWins`, `origins()` is always empty,
because that mode walks a flat candidate list and scope has no bearing on what
loads.

### Root the project scope

The project scope is rooted at the working directory by default. Two spellings
replace it with a caller-selected directory:

- `ConfigFilePolicy::project_root(root)` replaces every automatic project root
  on a hand-built policy; and
- the derive key `project_root_from = "field_name"` names the CLI field whose
  value becomes that root. The field must be of type `PathBuf` or
  `Option<PathBuf>`, and must not be skipped for the CLI; the derive rejects
  any other field at compile time.

## Enable another file format

TOML is enabled by default. Enable the `json5` or `yaml` feature when users
already work in that format:

<!-- tested-example: guide-yaml-install -->
```toml
[dependencies]
ortho_config = { version = "0.10.0", features = ["yaml"] }
```

YAML uses YAML 1.2 semantics: legacy words such as `yes` and `on` remain
strings, and duplicate mapping keys are rejected. In this file, `enabled` and
`mode` are therefore strings and only `port` is a number:

<!-- tested-example: guide-yaml -->
```yaml
enabled: yes
mode: on
port: 8080
```

The `yaml` feature also requires the `serde_json` feature, which is part of the
default feature set. If default features are disabled, enable both explicitly.

## Give each subcommand its own settings

Many CLIs have global options plus commands with different configuration. Derive
`OrthoConfig` for each subcommand's argument struct and merge only the
selected command:

<!-- tested-example: guide-subcommand -->
```rust
use clap::{Parser, Subcommand};
use ortho_config::{OrthoConfig, OrthoResult, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "acme")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve(ServeConfig),
}

#[derive(Debug, Default, Parser, Deserialize, Serialize, OrthoConfig)]
#[command(name = "serve")]
#[ortho_config(prefix = "ACME_SERVE_")]
struct ServeConfig {
    #[arg(long)]
    port: Option<u16>,
}

fn main() -> OrthoResult<()> {
    match Cli::parse().command {
        Command::Serve(cli) => {
            let config = cli.load_and_merge()?;
            println!("port={:?}", config.port);
        }
    }
    Ok(())
}
```

Subcommand settings live under a `cmds` namespace so they cannot collide with
global settings. For the `serve` command above, a file supplies them in a
`[cmds.serve]` table and the environment supplies them as
`ACME_SERVE_CMDS_SERVE_PORT`: the struct prefix, then `CMDS_`, the command
name, and the field.

For an enum with many variants, derive `SelectedSubcommandMerge` on the enum
and call `load_globals_and_merge_selected_subcommand`. The generated match
keeps the entry point small; the
[Hello World example](../examples/hello_world/) uses this pattern.

### Let files override clap defaults

A `clap` default normally counts as a command-line value, so it would beat a
file or environment value. Add `#[ortho_config(cli_default_as_absent)]` to a
field when the clap default should sit below files and environment variables
instead. The derive infers `default_value`, `default_value_t`, and
`default_values_t` defaults, so the value need only be declared in clap.
`default_value` is replayed through clap's parser, including `ValueEnum` and
`#[arg(value_parser = ...)]` fields. Scalars, `Option<T>`, and `Vec<T>` are
supported; nested wrappers and map fields require an explicit
`#[ortho_config(default = ...)]` instead.

For a subcommand, pair the attribute with a match-aware loader such as
`load_and_merge_with_matches`. Clap's match metadata distinguishes a parser
default from an explicit command-line value; without it, values in the parsed
struct are treated as command-line input. The subcommand merge starts from the
struct's `Default` implementation, so implement `Default` to agree with the
clap defaults. The
[v0.10.0 migration guide](v0-10-0-migration-guide.md#infer-parser-faithful-string-defaults)
has a complete, tested example using `load()`.

### Keep parse-only commands separate

Some commands only parse arguments and do not participate in configuration
loading. Keep those arguments in a separate clap-only type:

<!-- tested-example: guide-clap-only-parse -->
```rust
use clap::Parser;

#[derive(Debug, Parser)]
struct ParseOnlyArgs {
    #[arg(long)]
    format: Option<String>,
}

fn main() {
    let args = ParseOnlyArgs::parse();
    println!("format={:?}", args.format);
}
```

This parse-only workflow calls clap directly and does not load files or merge
environment values. Because `ParseOnlyArgs` does not derive `OrthoConfig`, it
has no `load_and_merge` or source-aware merge API. Use an `OrthoConfig`-derived
struct, such as `ServeConfig` above, when a command needs configuration
layering.

For a hand-built Cargo external-subcommand parser, see the next section.

## Cargo external-subcommand entry points

Cargo runs `cargo <name> [OPTIONS]` by locating a binary named `cargo-<name>` on
`PATH` and executing it with the subcommand name injected as the second
argument: argv becomes `["<path>/cargo-<name>", "<name>", OPTIONS...]`. A
parser that models only `cargo-<name> [OPTIONS]` rejects that injected `<name>`
token before any application logic can run.

`ortho_config::cargo::external_subcommand` wraps a hand-built `clap::Command`
in the standard wrapper shape: a synthetic `cargo` parent that requires a
single subcommand carrying the tool's real options. The worked example below
parses the Cargo-injected argv all the way to the option value:

<!-- tested-example: guide-cargo-external-subcommand -->
```rust
use ortho_config::cargo::external_subcommand;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args_command = clap::Command::new("demo")
        .version("1.2.3")
        .arg(
            clap::Arg::new("verbose")
                .long("verbose")
                .action(clap::ArgAction::SetTrue),
        );
    let cli = external_subcommand("cargo-demo", "demo", args_command);
    let matches = match cli.try_get_matches_from(std::env::args()) {
        Ok(matches) => matches,
        Err(error) => error.exit(),
    };
    let demo = matches
        .subcommand_matches("demo")
        .ok_or_else(|| std::io::Error::other("subcommand_required guarantees a subcommand"))?;
    assert!(demo.get_flag("verbose"));
    Ok(())
}
```

The returned command accepts both invocation forms with the same inner options
and no duplicated parser setup:

- `cargo demo --verbose` (Cargo dispatch: argv
  `['cargo-demo', 'demo', '--verbose']`), and
- `cargo-demo demo --verbose` (direct invocation with the same injected
  token).

The adoption cost is that options move one level down: callers read them through
`subcommand_matches("demo")` on the parsed matches rather than from the
top-level matches.

The wrapper is CLI entry-point structure, not configuration loading. It does
not change OrthoConfig's merge precedence and it does not add a second
configuration-loading pathway.

Derive-based callers do not need the helper. Wrap the `Args` struct in a
single-variant `#[command(subcommand)]` enum, as `cargo-orthohelp` does in
`cargo-orthohelp/src/cli/mod.rs`.

### Binary-level obligations

The helper leaves the following binary-level obligations with the caller:

- **Version.** The version belongs on the inner command, where the caller
  controls it. The synthetic parent is version-less, so a top-level
  `cargo-demo --version` intentionally errors with `UnknownArgument` rather
  than printing a version. This diverges from the derive path, where
  `cargo-orthohelp` sets `#[command(version)]` on its parent and
  `cargo-orthohelp --version` works.
- **Invocation hints.** Cargo-facing binaries should augment the
  `MissingSubcommand` and `UnknownArgument` errors with a hint naming the
  `cargo <name>` invocation form, as `cargo-orthohelp/src/main.rs` does. The
  following program adds that hint to the wrapped command:

<!-- tested-example: guide-cargo-error-hints -->
```rust
use ortho_config::cargo::external_subcommand;
use std::io::Write;

fn main() {
    let cli = external_subcommand("cargo-demo", "demo", clap::Command::new("demo"));
    if let Err(error) = cli.try_get_matches_from(std::env::args_os()) {
        exit_for_clap_error(&error);
    }
}

fn exit_for_clap_error(error: &clap::Error) -> ! {
    let kind = error.kind();
    let exit_code = error.exit_code();
    if matches!(
        kind,
        clap::error::ErrorKind::UnknownArgument | clap::error::ErrorKind::MissingSubcommand
    ) {
        drop(write_augmented_clap_error(error));
        std::process::exit(exit_code);
    }
    error.exit();
}

fn write_augmented_clap_error(error: &clap::Error) -> std::io::Result<()> {
    let mut stderr = std::io::stderr().lock();
    write!(stderr, "{error}")?;
    writeln!(
        stderr,
        "note: invoke this tool via `cargo <name>` or as `cargo-<name> <name> [OPTIONS]`"
    )
}
```

- **Tracing.** Cargo-facing binaries should initialize a tracing subscriber
  before parsing and emit a debug event at the dispatch boundary once the
  injected token is accepted. `cargo-orthohelp/src/main.rs` is the reference
  implementation. The library helper does not install a subscriber, because
  libraries must not install global recorders;
  [Add production diagnostics](#add-production-diagnostics) shows the
  application-side set-up.

The design rationale is recorded in
[ADR-0004](adrs/adr-0004-cargo-external-subcommand-entry-point.md).

## Handle errors at the application boundary

Library APIs return `OrthoResult<T>`, whose error is an `Arc<OrthoError>`.
Propagate it while loading, then render or map it where the application owns
the user experience:

<!-- tested-example: guide-errors -->
```rust
use ortho_config::{OrthoConfig, OrthoError};
use serde::Deserialize;

#[derive(Debug, Deserialize, OrthoConfig)]
struct Config {
    port: u16,
}

fn main() {
    match Config::load_from_iter(["acme", "--port", "not-a-number"]) {
        Ok(config) => println!("port={}", config.port),
        Err(error) => match error.as_ref() {
            OrthoError::CliParsing(clap_error) => eprintln!("{clap_error}"),
            other => eprintln!("configuration error: {other}"),
        },
    }
}
```

Preserve `clap`'s display-only exits for `--help` and `--version`; use
`is_display_request` when a wider application error layer needs to distinguish
them. `OrthoError::try_aggregate` combines independent validation failures
without inventing an error for an empty collection. The result extension traits
`OrthoResultExt`, `OrthoMergeExt`, and `ResultIntoFigment` keep conversions
explicit at integration boundaries.

## Localize help and parse failures together

Localization is most reliable when the command metadata is translated before
parsing and any resulting error goes through the same localizer.
`LocalizedParse` provides that path for any `clap::Parser`:

<!-- tested-example: guide-localization -->
```rust
use clap::Parser;
use ortho_config::{LocalizedParse, NoOpLocalizer};

#[derive(Debug, Parser)]
#[command(name = "acme", bin_name = "acme")]
struct Cli {
    #[arg(long)]
    verbose: bool,
}

fn main() -> Result<(), clap::Error> {
    let localizer = NoOpLocalizer::new();
    let cli = Cli::try_parse_localized_from(
        ["acme", "--verbose"],
        &localizer,
    )?;
    assert!(cli.verbose);
    println!("verbose={}", cli.verbose);
    Ok(())
}
```

`NoOpLocalizer` keeps clap's own text, which makes it a safe starting point.
Swap in `FluentLocalizer` for translated catalogues. Use
`LocalizeCmd::with_base` with `parse_localized_command` when catalogue
identifiers must use an explicit root rather than the binary name. Missing
translations fall back to the original `clap` text and emit a warning event, so
users still receive a useful error.

### Use the generated message identifiers

Derived configurations implement `OrthoConfigLocalization`. Refer to Fluent
messages through the generated constants rather than repeating identifier
strings; this prevents catalogue and command declarations from drifting. Set
the catalogue root explicitly with `localization_base`; when it is omitted, the
derived application name supplies the default base:

<!-- tested-example: guide-localization-ids -->
```rust
use ortho_config::{OrthoConfig, OrthoConfigLocalization};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_", localization_base = "acme.cli")]
struct Config {
    /// Report what would change without changing it.
    #[ortho_config(default = false)]
    is_dry_run: bool,
}

fn main() {
    println!("base={}", Config::LOCALIZATION_BASE);
    println!("about={}", Config::ABOUT_ID);
    for ids in Config::ARG_IDS {
        println!("{} help={}", ids.name, ids.help_id);
    }
}
```

The program prints:

<!-- tested-example: guide-localization-ids-output -->
```plaintext
base=acme.cli
about=acme-cli-about
is_dry_run help=acme-cli-args-is_dry_run-help
```

Pass `Config::LOCALIZATION_BASE` to `LocalizeCmd::with_base` so the command
tree and the catalogue share one root. The generated command constants are
`ABOUT_ID`, `LONG_ABOUT_ID`, `USAGE_ID`, `VERSION_ID`, `LONG_VERSION_ID`,
`AFTER_HELP_ID`, and `AFTER_LONG_HELP_ID`. `ARG_IDS` contains one
`ArgLocalizationIds` record per eligible argument, with its clap `name` and
`help_id`, `long_help_id`, and `value_name_id` constants.

Identifiers normalize each dotted base segment to a Fluent-safe form and join
segments with hyphens, while an argument's own segment keeps the Rust field
name, underscores included. Duplicate normalized argument identifiers are
rejected at compile time. Flattened, subcommand, and `skip_cli` fields do not
receive argument constants.

Per-field localized defaults are not yet available. The
`#[ortho_config(localized_default = "...")]` attribute is recognized so that
its absence is reported rather than silently ignored: using it fails the build
with a diagnostic explaining that the feature is deferred.

### Export identifier artefacts for tooling

Translation tooling can read every derived identifier from a generated
artefact. Emission is opt-in and provisional. Force a fresh expansion, because
Cargo does not track the variable for proc-macro crates:

<!-- tested-example: guide-identifier-artefact -->
```console
cargo clean -p <package> && ORTHO_CONFIG_EMIT_IDENTIFIERS=1 cargo build -p <package>
```

The resulting `${OUT_DIR}/ortho-config/cli-identifiers.json` is a standalone
declaration inventory. Do not set the variable globally, and use the
documentation IR for identifiers of mounted command trees. When the output
exceeds 1 MiB, the index lists `cli-identifiers.<n>.json` parts; read those
listed parts rather than expecting all entries in the single index file.

Concurrent compiler processes for one package serialize their writes behind an
exclusive lock, so a build that looks stuck may be waiting for a peer. To see
that, add `ORTHO_CONFIG_ARTEFACT_REPORT=1` alongside the emission variable and
rebuild. Three best-effort diagnostic files may then appear in the artefact
directory:

- `cli-identifiers.lock-wait.<pid>.json` appears when a publication waits for
  the lock longer than five seconds, is refreshed while the wait continues, and
  is removed once the lock is granted. Check its `written_at_ms` before acting
  on it: process identifiers are reused, so a marker that is not recent
  describes a wait that has ended.
- `cli-identifiers.build-report.<pid>.json` names the process, the expansion,
  the stage it reached, how long it waited, and when. A crate with several
  derives keeps the report of its last publication.
- `cli-identifiers.observation-errors.<pid>.jsonl` records any observation that
  could not be written, so a build with no report is distinguishable from one
  that could not write it.

These files explain a build and never fail one, and none of them is part of the
identifier inventory.

## Test discovery without changing the process environment

Tests that mutate environment variables interfere with one another. Build a
`ConfigDiscovery` with `MapEnv` instead. Each test owns its values and can run
in parallel:

<!-- tested-example: guide-hermetic-discovery -->
```rust
use ortho_config::{ConfigDiscovery, MapEnv};
use std::sync::Arc;

fn main() {
    let environment = Arc::new(
        MapEnv::new()
            .with_var("ACME_CONFIG", "/srv/acme/server.toml")
            .with_var("HOME", "/home/tester"),
    );

    let discovery = ConfigDiscovery::builder("acme")
        .env_var("ACME_CONFIG")
        .env_source(environment)
        .clear_project_roots()
        .build();

    assert_eq!(
        discovery.candidates().first().map(|path| path.as_path()),
        Some(std::path::Path::new("/srv/acme/server.toml"))
    );
    println!("candidate=/srv/acme/server.toml");
}
```

`ProcessEnv` remains the default, so production applications do not need to
change. `EnvSource` deliberately supports lookup by name but not enumeration;
discovery cannot accidentally scan or log unrelated environment values.

## Resolve discovery and merging without process state

The previous section isolates file discovery. Tests and embedding applications
can go further and resolve the complete configuration without reading the
process environment at all. Two source traits divide the work:

- `EnvSource`, shared as `SharedEnvSource`, answers named lookups such as
  `ACME_CONFIG_PATH` during discovery; and
- `ScanEnvSource`, shared as `SharedScanEnvSource`, grants the explicit
  ability to enumerate variables for the environment merge layer.

`MapEnv` implements both traits. Create one `Arc<MapEnv>`, then coerce cloned
`Arc` values to each shared type; the same map then drives discovery lookups
and merge enumeration. The derive-generated `load_from_iter_with_sources`
accepts both:

<!-- tested-example: guide-source-aware-load -->
```rust
use ortho_config::{
    MapEnv, OrthoConfig, OrthoResult, SharedEnvSource, SharedScanEnvSource,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_")]
struct Config {
    #[ortho_config(default = 8080)]
    port: u16,
}

fn main() -> OrthoResult<()> {
    let environment = Arc::new(MapEnv::new().with_var("ACME_PORT", "9000"));
    let discovery: SharedEnvSource = environment.clone();
    let merge: SharedScanEnvSource = environment;

    let config = Config::load_from_iter_with_sources(["acme"], discovery, merge)?;
    println!("port={}", config.port);
    Ok(())
}
```

The example prints `port=9000`. No process variable is read: the discovery
source handles named lookups, while the scan source supplies the merge layer.
The ordinary `load()` and `load_from_iter()` methods continue to use the
process environment.

### Compose layers without deserializing

When layer composition is needed without deserializing a configuration,
`OrthoConfig` also generates `compose_layers_from_iter_with_sources`. It takes
the same three arguments and returns the composed layers:

<!-- tested-example: guide-compose-layers-with-sources -->
```rust
use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_")]
pub struct Config {
    #[serde(default)]
    pub port: u16,
}

fn main() {
    let environment = Arc::new(
        MapEnv::new()
            .with_var("ACME_CONFIG_PATH", "/srv/acme.toml")
            .with_var("ACME_PORT", "9000"),
    );
    let discovery: SharedEnvSource = environment.clone();
    let merge: SharedScanEnvSource = environment;

    let _composition = Config::compose_layers_from_iter_with_sources(
        ["acme"], discovery, merge,
    );
    println!("composition=ready");
}
```

The ordinary `compose_layers_from_iter` and `compose_layers` methods remain
process-backed.

### Build an environment provider from a map

Applications that assemble their own `figment` stack can attach the scan source
to `CsvEnv`, OrthoConfig's environment provider, with `with_source`:

<!-- tested-example: guide-csv-env-with-source -->
```rust
use ortho_config::figment::Figment;
use ortho_config::{CsvEnv, MapEnv};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct Database {
    host: String,
}

#[derive(Debug, Deserialize)]
struct Settings {
    database: Database,
    tags: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = Arc::new(
        MapEnv::new()
            .with_var("APP_DATABASE__HOST", "localhost")
            .with_var("APP_TAGS", "one,two"),
    );
    let provider = CsvEnv::prefixed("APP_").split("__").with_source(source);
    let settings: Settings = Figment::new().merge(provider).extract()?;
    println!("host={} tags={:?}", settings.database.host, settings.tags);
    Ok(())
}
```

The example prints `host=localhost tags=["one", "two"]`. The injected provider
follows the same declarative rules as the process-backed provider:

- prefix matching is case-insensitive and strips the matching prefix;
- `lowercase(true)` and `uppercase(true)` transform names at their place in the
  builder chain;
- each `split` replaces its separator with a dotted key component, so
  `DATABASE__HOST` becomes `database.host`; and
- CSV parsing is enabled by default: comma-containing scalar values become
  lists, while values beginning with `[`, `{`, or a quote remain structured or
  quoted values. Use `csv(false)` when a comma must remain part of a scalar.

Providers that use `map()` or `filter_map()` cannot be injected, because
arbitrary closures cannot be replayed against a `ScanEnvSource`; the
source-aware path rejects them rather than silently changing the key mapping.

### Inject sources into a subcommand

For an `OrthoConfig`-derived subcommand, pass a `SharedScanEnvSource` to
`load_and_merge_with_sources` instead of calling the process-backed
`load_and_merge`:

<!-- tested-example: guide-subcommand-merge-sources -->
```rust
use clap::Parser;
use ortho_config::{MapEnv, OrthoConfig, OrthoResult, SubcmdConfigMerge};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Default, Parser, Deserialize, Serialize, OrthoConfig)]
#[command(name = "serve")]
#[ortho_config(prefix = "ACME_SERVE_")]
struct ServeConfig {
    #[arg(long)]
    port: Option<u16>,
}

fn main() -> OrthoResult<()> {
    let cli = ServeConfig::parse_from(["serve"]);
    let environment = Arc::new(
        MapEnv::new().with_var("ACME_SERVE_CMDS_SERVE_PORT", "9000"),
    );
    let config = cli.load_and_merge_with_sources(environment)?;
    println!("port={:?}", config.port);
    Ok(())
}
```

The example prints `port=Some(9000)`. This injects the environment layer only;
files are still discovered from the process, and explicit command-line values
keep their precedence.

When both file discovery and environment scanning must use explicit sources for
an enum of subcommands, call
`load_globals_and_merge_selected_subcommand_with_sources`. Its file context
performs named discovery lookups, while the shared scanning source supplies the
environment layer. The selected command's CLI values retain their normal
precedence. For example, this injects `APP_CMDS_RUN_LEVEL` without reading the
process environment:

<!-- tested-example: guide-selected-subcommand-sources -->
```rust
use std::{path::Path, sync::Arc};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use ortho_config::{
    MapEnv, OrthoConfig, SelectedSubcommandSources, SharedScanEnvSource,
    SubcommandFileContext, load_globals_and_merge_selected_subcommand_with_sources,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, ortho_config::SelectedSubcommandMerge)]
enum Commands {
    Run(RunArgs),
}

#[derive(Debug, Default, Deserialize, Serialize, Parser, OrthoConfig)]
#[command(name = "run")]
#[ortho_config(prefix = "APP_")]
struct RunArgs {
    #[arg(long)]
    level: Option<u8>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Cli::command().try_get_matches_from(["app", "run"])?;
    let cli = Cli::from_arg_matches(&matches)?;
    let environment = Arc::new(MapEnv::new().with_var("APP_CMDS_RUN_LEVEL", "3"));
    let files = SubcommandFileContext::new(Path::new("."), environment.as_ref());
    let merge_source: SharedScanEnvSource = environment.clone();
    let sources = SelectedSubcommandSources::new(files, merge_source);
    let (_globals, command) = load_globals_and_merge_selected_subcommand_with_sources(
        &matches,
        cli.command,
        sources,
        || Ok::<_, std::io::Error>(()),
    )?;
    assert_eq!(match command { Commands::Run(config) => config.level }, Some(3));
    Ok(())
}
```

`SelectedSubcommandSources` applies only to the selected command. The caller
still owns globals loading; if globals must also avoid process access, inject
their file and environment sources separately in the globals loader.

A manual `SelectedSubcommandMerge` implementation supports injected sources by
overriding the trait's `load_and_merge_selected_with_sources` method.
Otherwise, the default returns
`SelectedSubcommandMergeError::SourceInjectionUnsupported` without invoking the
process-backed `load_and_merge_selected` method.

### Choose the file-discovery source

The process-backed loaders are the default. Use them when the running process's
own environment should decide which configuration files are found and which
variables are merged: they take their named lookups from the live process
environment and use the current directory as the local base.

Use `SubcommandFileContext::new(base, discovery)` with a source-aware `_at`
loader when the search inputs must be supplied rather than inherited. The
context pairs an explicit local file base with a lookup-only `EnvSource`; it
answers named lookups for `HOME`, `USERPROFILE` (non-Unix), `XDG_CONFIG_HOME`,
and `XDG_CONFIG_DIRS`, and on non-Unix targets may also supply the native
configuration directory. The explicit-prefix and configured-prefix loaders are
`load_and_merge_subcommand_with_sources_at` and
`load_and_merge_subcommand_for_with_sources_at`; their match-aware variants are
`load_and_merge_subcommand_with_matches_with_sources_at` and
`load_and_merge_subcommand_for_with_matches_with_sources_at`. Pass
`SubcommandCliMatches::new(&cli, &matches)` to a match-aware variant, which is
required for fields marked `cli_default_as_absent`.

On Unix and Redox, candidates are tried in the order `HOME` dotfiles, the XDG
bases (the configured `XDG_CONFIG_HOME`, else `$HOME/.config` or the platform
home fallback, then each absolute `XDG_CONFIG_DIRS` entry), and finally the
explicit base. Only the first existing `config.<ext>` per extension is kept
from the XDG bases. On other platforms, discovery uses `HOME`/`USERPROFILE` or
the platform home fallback, then the native configuration directory from
`EnvSource::config_dir_fallback` when available, and finally the explicit base.

On Unix and Redox, missing XDG candidates, including candidates whose metadata
check returns `NotFound`, are skipped. Any other metadata-check error stops
discovery and is returned as `OrthoError::File`, with the candidate path and
underlying I/O error. Inspect the reported path and correct the discovery
source or filesystem permissions before retrying.

<!-- tested-example: guide-subcommand-sources -->
```rust
use clap::Parser;
use ortho_config::{
    MapEnv, OrthoConfig, OrthoResult, SharedScanEnvSource, SubcommandFileContext,
    load_and_merge_subcommand_for_with_sources_at,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Default, Parser, Deserialize, Serialize, OrthoConfig)]
#[command(name = "serve")]
#[ortho_config(prefix = "ACME_SERVE_")]
struct ServeCli {
    #[arg(long)]
    port: Option<u16>,
}

fn main() -> OrthoResult<()> {
    let cli = ServeCli::default();
    // Naming both XDG locations keeps the search inside supplied paths:
    // nothing outside this map and the explicit base is consulted.
    let discovery = MapEnv::new()
        .with_var("XDG_CONFIG_HOME", "/srv/acme/config")
        .with_var("XDG_CONFIG_DIRS", "/srv/acme/fallback");
    let base = std::path::Path::new("/srv/acme/local");
    let files = SubcommandFileContext::new(base, &discovery);
    let merge: SharedScanEnvSource =
        Arc::new(MapEnv::new().with_var("ACME_SERVE_CMDS_SERVE_PORT", "9000"));

    let config = load_and_merge_subcommand_for_with_sources_at(&cli, files, merge)?;
    println!("port={:?}", config.port);
    Ok(())
}
```

The example prints `port=Some(9000)`. No process variable is read: the merge
source supplies the `ACME_SERVE_CMDS_SERVE_PORT` layer, and file discovery
reads only the variables the context's `EnvSource` answers. Because the map
names both XDG variables, discovery's default `XDG_CONFIG_DIRS` of `/etc/xdg`
is never reached. The merge layer outranks the file layer, so the printed value
does not depend on what those directories happen to contain.

The localization runtime is also published as its own crate. An application
that only needs Fluent lookup and formatting can depend on `ortho_l10n`
directly and avoid configuration discovery, the derive macros, Figment, and
Clap entirely. The types are the same ones `ortho_config` re-exports, so code
that mixes imports from both names the same types and needs no adapter. See the
[ortho_l10n README](../ortho_l10n/README.md)
for its API.

## Add production diagnostics

OrthoConfig emits structured `tracing` events for discovery attempts, selected
files, skips, and failures. The library does not install a subscriber; the
binary should do that once during start-up. Add the subscriber with its
environment-filter support:

<!-- tested-example: guide-tracing-install -->
```toml
[dependencies]
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

<!-- tested-example: guide-tracing -->
```rust
use ortho_config::{OrthoConfig, OrthoResult};
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
struct Config {
    #[ortho_config(default = 8080)]
    port: u16,
}

fn main() -> OrthoResult<()> {
    tracing_subscriber::fmt()
        .with_env_filter("ortho_config=debug")
        .with_writer(std::io::stderr)
        .try_init()
        .ok();

    let config = Config::load()?;
    println!("port={}", config.port);
    Ok(())
}
```

Diagnostics go to standard error, so standard output stays clean for the
application's own results. Do not log configuration values or candidate paths
around these events. The crate's own diagnostics avoid path and value fields
because configuration locations and values may be sensitive.

Metrics are a low-cost opt-in when the application already has a `metrics`
recorder:

<!-- tested-example: guide-metrics-install -->
```toml
[dependencies]
ortho_config = { version = "0.10.0", features = ["metrics"] }
```

The feature emits bounded counters for discovery, merging, and subcommand path
resolution, such as `ortho_config.discovery.attempts` and
`ortho_config.merge.outcomes`. OrthoConfig never installs a recorder, and
enabling the feature does nothing visible until the application installs one.

## Use an aliased dependency

Cargo permits dependency aliases. The derive macros can generate paths through
that alias, which is useful in workspaces that reserve the canonical crate name:

<!-- tested-example: guide-alias-install -->
```toml
[dependencies]
config_layer = { package = "ortho_config", version = "0.10.0" }
serde = { version = "1.0", features = ["derive"] }
```

Name the alias on every type that derives an OrthoConfig macro:

<!-- tested-example: guide-alias-derive -->
```rust
use config_layer::{OrthoConfig, OrthoResult};
use serde::Deserialize;

#[derive(Deserialize, OrthoConfig)]
#[ortho_config(crate = "config_layer", prefix = "ACME_")]
struct Config {
    #[ortho_config(default = 8080)]
    port: u16,
}

fn main() -> OrthoResult<()> {
    let config = Config::load_from_iter(["acme"])?;
    assert_eq!(config.port, 8080);
    println!("port={}", config.port);
    Ok(())
}
```

The same attribute is supported by `SelectedSubcommandMerge`. OrthoConfig
re-exports the dependencies used by generated code, so a derive-only consumer
does not need direct `figment`, `uncased`, `xdg`, or format-parser dependencies.

## Generate help from the same metadata

`#[derive(OrthoConfig)]` also implements `OrthoConfigDocs`. The metadata
records fields, source names, precedence, discovery, defaults, and nested
subcommands. Derive `OrthoConfigSubcommandDocs` on a `clap::Subcommand` enum so
the generated tree includes every variant.

Inspect the metadata in code:

<!-- tested-example: guide-orthohelp-metadata -->
```rust
use ortho_config::{OrthoConfig, OrthoConfigDocs};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_")]
struct Config {
    /// Address on which the service listens.
    #[ortho_config(default = String::from("127.0.0.1"))]
    host: String,
}

fn main() {
    let metadata = Config::get_doc_metadata();
    assert_eq!(metadata.fields.len(), 1);
    assert_eq!(metadata.fields[0].name, "host");
    println!("field={}", metadata.fields[0].name);
}
```

Or use `cargo-orthohelp` to emit intermediate representation (IR), Unix man
pages, PowerShell help, compact agent context, or all formats. The
[README](../README.md#install-the-documentation-generator) explains how to
install it. Point it at a package:

<!-- tested-example: guide-orthohelp-command -->
```console
cargo orthohelp --package hello_world --format agent-context
```

The tool builds a small bridge against the selected package. Name the root
configuration type in the package's `Cargo.toml` with
`[package.metadata.ortho_config] root_type = "crate::path::Type"` (or pass
`--root-type`), keep that type public, and make its documentation metadata
available from the selected library or binary target. The default format is
`ir`. `--format agent-context` writes `agent-context.json` and also emits the
same compact JSON document to standard output, so a pipeline can consume it
directly.

## Generate every format at once

Passing `--format all` runs the IR, man-page, PowerShell, and agent-context
generators in a single invocation. Use it when one release build has to supply
human help and agent context together:

<!-- tested-example: guide-orthohelp-format-all -->
```console
cargo orthohelp --format all --package hello_world --locale en-US
```

Every artefact is written below the output root, which defaults to
`<target_dir>/orthohelp/out` and can be redirected with `--out-dir`. For a
single locale the tree is:

<!-- tested-example: guide-orthohelp-format-all-tree -->
```plaintext
<out>/
├── agent-context.json
├── ir/
│   └── en-US.json
├── man/
│   └── man1/
│       └── hello_world.1
└── powershell/
    └── HelloWorld/
        ├── HelloWorld.psd1
        ├── HelloWorld.psm1
        └── en-US/
            ├── HelloWorld-help.xml
            └── about_HelloWorld.help.txt
```

The IR generator writes one `ir/<locale>.json` per resolved locale. Man pages
live under `man/man<N>/<bin>.<N>`, where `N` is the section number: the
`--man-section` option defaults to `1`, accepts `1` to `8`, and names both the
directory and the file extension. The PowerShell generator writes its module
under `powershell/<ModuleName>/`, with a culture subdirectory per resolved
locale holding `<ModuleName>-help.xml` and `about_<ModuleName>.help.txt`.

`ModuleName` is resolved from these sources in order:

1. `--ps-module-name`;
2. the `module_name` key under `[package.metadata.ortho_config.windows]`;
3. the hint declared on the type itself as
   `#[ortho_config(windows(module_name = "Custom"))]`; and
4. the binary name.

Each source applies only when every earlier one is absent, so a type declaring
`Custom` generates `Custom` even when package metadata declares a
`[package.metadata.ortho_config.windows]` table without its own `module_name`.

Agent context is not locale-scoped: `agent-context.json` sits at the output
root rather than under a locale directory, so a run with several locales still
writes exactly one such document, which is also written to standard output.
Command summaries inside it are resolved from the package's `en-US` resources
rather than from a requested `--locale`, and are omitted when no such resource
exists.

### Choose locales

`--locale` is repeatable, and repeated values are deduplicated while keeping
the order in which they were first given. With more than one locale the IR
gains one JSON file per locale, the man tree nests each locale as
`<locale>/man/man<N>/<bin>.<N>`, and the PowerShell module gains one culture
subdirectory per locale.

`--locale` takes precedence over `--all-locales`. When neither is given, the
command uses the single locale `en-US`, even when the package metadata or the
package's `locales/` directory declares others. With `--all-locales` and no
`--locale`, the command renders the locales declared in package metadata, or
those discovered under the package's `locales/` directory when metadata
declares none, and falls back to `en-US` only when neither source is available.

The PowerShell `en-US` fallback is separate from that locale default. When the
requested locales omit `en-US`, the generator also writes an `en-US` help set
built from the first locale, because PowerShell culture probing strongly
prefers an `en-US` culture to be present. `--ensure-en-us` controls this and is
on by default; pass `--ensure-en-us false` to suppress the extra culture. The
fallback copies text rather than translating it: with `--locale ja` the `en-US`
directory holds the Japanese help, so do not read its name as a language
guarantee.

`--format all` is additive. It writes the same artefacts, at the same paths, as
running `--format ir`, `--format man`, `--format ps`, and
`--format agent-context` one after another, and each format keeps its own
output paths as a compatibility contract. The
[cargo-orthohelp design](cargo-orthohelp-design.md) records those per-format
paths and the versioning rule that protects them.

## Offer a compact contract to automation

Agent context complements human help with a small, stable JSON description of
commands, inputs, output modes, interaction, and mutation boundaries. A common
application convention is a `context --json` command; `cargo-orthohelp` uses
`--format agent-context` for generation.

The smallest valid context created by `AgentContext::new("acme")` serializes to
this shape:

<!-- tested-example: guide-agent-context -->
```json
{
  "schema_version": "1",
  "kind": "acme.agent_context",
  "package": "acme",
  "commands": [],
  "profiles": { "supported": false },
  "feedback": { "supported": false },
  "policy": { "agent_native": "warn", "exceptions": [] },
  "skill_manifests": []
}
```

Fill `AgentCommand` entries only with claims the executable honours.
`SkillManifest` and `SkillCommandRef` link skills to real commands. They do not
replace command validation or grant an agent capabilities that the CLI does not
have. Consumers should ignore unknown fields, so that later additive releases
do not break them. For constructing and serializing these types from Rust, see
the [API guide](cargo-orthohelp-api-guide.md).

### Advertise profile support

A struct that opts into [profiles](#switch-between-named-profiles) advertises
the selection contract in agent context. For a struct with `prefix = "APP_"`,
`cargo-orthohelp` emits:

<!-- tested-example: guide-profile-metadata -->
```json
{
  "profiles": {
    "supported": true,
    "selection": {
      "flag": "profile",
      "env_var": "APP_PROFILE"
    }
  }
}
```

The `flag` follows the agent-context input convention and omits the leading
`--`. Each command in the context also lists a `profile` input, so an agent can
discover the flag from any command. The unsupported case still serializes as
`{ "supported": false }`, and the optional `selection` and `list_command`
fields are omitted when absent; `list_command` is reserved for a future
profile-listing command.

Which profile is active is a runtime fact rather than generated metadata. An
application that offers `context --json` can report it from
`ProfileLoadOutcome::selection()`, for example as
`{"name": "staging", "source": "flag"}` or
`{"name": "staging", "source": "environment"}`; the application owns that
rendering.

## Declare behaviour metadata for agents

Commands that prompt or mutate state should declare that behaviour so agents
can decide whether an invocation is safe before running it. Add a struct-level
`behaviour(...)` attribute to the command's arguments struct:

<!-- tested-example: guide-behaviour-metadata -->
```rust
use ortho_config::{OrthoConfig, OrthoConfigDocs};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "APP_",
    behaviour(
        interaction = "interactive",
        mutation = "delete",
        bypass = "--force",
        dry_run = "--dry-run"
    )
)]
struct PurgeArgs {
    /// Skip the confirmation prompt.
    #[ortho_config(default = false)]
    force: bool,

    /// Report what would be deleted without deleting it.
    #[ortho_config(default = false)]
    dry_run: bool,
}

fn main() {
    if let Some(behaviour) = PurgeArgs::get_doc_metadata().behaviour {
        println!(
            "mutation={:?} bypass={:?}",
            behaviour.mutation, behaviour.bypass
        );
    }
}
```

The example prints `mutation=Some(Delete) bypass=Some("--force")`. Supported
keys and values are:

- `interaction = "non_interactive"` or `"interactive"` — whether the command
  can prompt;
- `mutation = "read_only"`, `"write"`, `"delete"`, or `"submit"` — the
  mutation boundary;
- `bypass = "--force"` — the confirmation or prompt bypass flag, which must
  match `--[a-z0-9]+(-[a-z0-9]+)*`; and
- `dry_run = "--dry-run"` — the dry-run flag, with the same grammar.

The declaration flows into the generated documentation IR and, through the
`cargo-orthohelp` bridge, into agent context as `interaction_mode`,
`mutation_effect`, `bypass_flag`, and `dry_run_flag`. Commands left unannotated
report `"unknown"` for the two enums and `null` for the two flags; the tool
never infers these facts from command names or flags.

`interaction = "non_interactive"` combined with a `bypass` declaration is a
compile error: a command that never prompts has nothing to bypass.

## Agent-native policy checking

`cargo orthohelp --check-agent-native` runs an opt-in policy check against a
target package. It does not build the bridge crate or require `root_type`, a
library target, or an `ortho_config` dependency, so it works for packages still
adopting the toolchain. Configure the policy under
`[package.metadata.ortho_config.policy]` in the target package's `Cargo.toml`:

<!-- tested-example: guide-policy-config -->
```toml
[package.metadata.ortho_config.policy]
mode = "warn"

[[package.metadata.ortho_config.policy.exceptions]]
kind = "verb"
name = "get"
reason = "redundant but part of the migration surface"

[[package.metadata.ortho_config.policy.exceptions]]
kind = "flag"
name = "--format"
reason = "scoped legacy flag"
command_path = "fixture"
```

The keys are:

| Key                         | Type                      | Default  | Meaning                                        |
| --------------------------- | ------------------------- | -------- | ---------------------------------------------- |
| `mode`                      | `"off"`/`"warn"`/`"deny"` | `"off"`  | Enforcement mode for the check.                |
| `exceptions[].kind`         | `"verb"`/`"flag"`         | required | Whether the exception names a verb or a flag.  |
| `exceptions[].name`         | string                    | required | The verb or flag as typed on the command line. |
| `exceptions[].reason`       | string                    | required | Why the project exempts this vocabulary item.  |
| `exceptions[].command_path` | string                    | absent   | Optional space-separated command path scope.   |

_Table 5: Agent-native policy configuration keys._

Then run the check, naming the package:

<!-- tested-example: guide-policy-check -->
```console
cargo orthohelp --check-agent-native --package my-cli --out-dir out
```

The command atomically writes a machine-stable `out/policy-report.json`, emits
the same compact JSON document to standard output, prints a one-line human
summary to standard error, and skips generation. The report includes the
effective mode, findings, configured exceptions with their reasons, and the
canonical vocabulary in `vocabulary.verbs` and `vocabulary.flags`.

The enforcement default is `off`: a package with no policy table checks
nothing. In `warn` mode, findings are reported without failing the command; in
`deny` mode, deny-level findings cause a non-zero exit after the report has
been written. Today the check validates the policy configuration itself, such
as malformed, redundant, or duplicated exceptions, and reports the canonical
vocabulary; linting command verbs and flags against that vocabulary is planned
for a later release. Exception reasons appear in `policy-report.json` but not
in agent context, which carries only the exception kind, name, and optional
command scope.

`--policy-mode <off|warn|deny>` overrides the configured mode for the report
and requires `--check-agent-native`. It never changes the generated agent
context, which records what the project has committed to.

Adding `--format` runs the policy check before generation. The policy report is
then the only JSON document on standard output; successful checks continue
generation, including writing `agent-context.json` for `agent-context` and
`all`, while a deny-mode violation emits the report and fails before any
generator artefact is created.

Because a misspelt table _name_ still resolves to `off`, gate on the mode
rather than assuming the check ran. The off-mode summary ("nothing was
checked") is one signal; a `jq`-based assertion is a stronger continuous
integration (CI) recipe:

<!-- tested-example: guide-policy-ci -->
```console
cargo orthohelp --check-agent-native --package my-cli --out-dir out
jq -e '.mode != "off"' out/policy-report.json
```

## Lint the declared behaviour

The behaviour rules are exported by `cargo_orthohelp::policy::rules` as
`check_behaviour(context, mode) -> PolicyReport`. They consume a compiled agent
context rather than a package manifest, which is why they are a library API
instead of part of `--check-agent-native`: the policy-only run deliberately
skips the bridge build, and the behaviour rules need its output. Generate
`agent-context.json` with `cargo orthohelp --format agent-context`, then check
it:

<!-- tested-example: guide-check-command -->
```rust
use cargo_orthohelp::policy::PolicyMode;
use cargo_orthohelp::policy::rules::behaviour::check_behaviour;
use ortho_config::{AgentContext, serde_json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string("agent-context.json")?;
    let agent_context: AgentContext = serde_json::from_str(&json)?;

    let report = check_behaviour(&agent_context, PolicyMode::Warn);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
```

Each behaviour finding carries a stable `rule_id` and `code`. `location` is
always `null` for now: agent context carries no source spans, so the `message`
names the command path plus the exact annotation to add. For a context whose
only command, `purge`, declares no behaviour, the program prints:

<!-- markdownlint-disable MD013 -->
<!-- tested-example: guide-check-report -->
```json
{
  "version": "1",
  "tool": "cargo-orthohelp",
  "mode": "warn",
  "results": [
    {
      "rule_id": "agent-native.behaviour.undeclared",
      "code": "interaction_unknown",
      "severity": "warn",
      "message": "command `purge` has undeclared interaction behaviour; add `behaviour(interaction = \"non_interactive\")` or `behaviour(interaction = \"interactive\")` to its arguments struct",
      "location": null
    },
    {
      "rule_id": "agent-native.behaviour.undeclared",
      "code": "mutation_unknown",
      "severity": "warn",
      "message": "command `purge` has undeclared mutation boundary; add `behaviour(mutation = \"read_only\")`, `behaviour(mutation = \"write\")`, `behaviour(mutation = \"delete\")`, or `behaviour(mutation = \"submit\")` to its arguments struct",
      "location": null
    }
  ],
  "summary": { "off": 0, "warn": 2, "deny": 0, "total": 2 },
  "exceptions": [],
  "vocabulary": {
    "verbs": ["get", "list", "create", "update", "delete", "jobs", "profile", "feedback"],
    "flags": ["--json", "--no-input", "--force", "--dry-run", "--limit", "--cursor", "--wait", "--profile", "--deliver"]
  }
}
```
<!-- markdownlint-enable MD013 -->

The rule set contains four rules:

- `agent-native.behaviour.destructive-bypass` / `destructive_bypass_missing` —
  a `mutation = "delete"` command that declares no bypass;
- `agent-native.behaviour.prompt-bypass` / `prompt_bypass_missing` — an
  `interaction = "interactive"` command that declares no bypass;
- `agent-native.behaviour.bypass-unknown` / `bypass_flag_unknown` — a declared
  bypass that matches no declared input, which is contradiction detection
  between two declarations rather than name inference; and
- `agent-native.behaviour.undeclared` / `interaction_unknown` and
  `mutation_unknown` — omitted metadata, which stays undeclared.

Findings follow command-path order. A command declared `non_interactive` is
exempt from `destructive-bypass`: it cannot prompt, so the declaration itself
is the approved non-interactive path. `PolicyMode::Off` evaluates no rule and
returns a report with no findings, still carrying the canonical vocabulary.
Annotate incrementally, starting with destructive commands:

1. Declare `behaviour(mutation = "delete", bypass = "--force", ...)` on each
   destructive command's arguments struct.
2. Add `interaction` and other `mutation` declarations as the command surface
   stabilizes.
3. Run the check in warning mode before enforcing it in continuous
   integration.

## Upgrade from an earlier release

Each release with compatibility notes has a migration guide that separates
required changes from improvements that can be adopted later:

- [v0.10.0 migration guide](v0-10-0-migration-guide.md) — from v0.9.0. Most
  applications need no code changes; review explicit command-line values that
  restate defaults, Fluent catalogue identifiers, and struct literals of
  `AgentContext` or `DocMetadata`.
- [v0.9.0 migration guide](v0-9-0-migration-guide.md) — from v0.8.0, including
  the YAML 1.2 parser change and `load_first` error reporting.

The [changelog](../CHANGELOG.md) lists every release's additions and fixes.

## A practical path from here

For a new CLI, begin with the first layered struct and add only the required
sections. A typical progression is:

1. choose stable CLI and environment names;
2. add a project file for durable settings;
3. customize discovery if the defaults are not part of the public interface;
4. split independent commands into subcommand configurations;
5. replace environment mutation in tests with injected sources;
6. localize help and initialize tracing at the application boundary; and
7. generate human and agent documentation once the command surface stabilizes.

The [Hello World application](../examples/hello_world/) demonstrates these
pieces in a larger layout. Its public greeting command is `greet`: file
settings use `[cmds.greet]`, and environment settings use
`HELLO_WORLD_CMDS_GREET_<FIELD>`. The
[API documentation](https://docs.rs/ortho_config) is the source for complete
type and method signatures.
