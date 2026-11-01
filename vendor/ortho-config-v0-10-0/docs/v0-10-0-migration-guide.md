# Migration guide: v0.9.0 to v0.10.0

## Who should read this

Read this guide before upgrading an application or library from OrthoConfig
v0.9.0 to v0.10.0. Most applications upgrade without changing any code:
existing derives compile unchanged, loading still reads the process environment
by default, and every new capability is opt-in.

Four areas deserve a review before release:

- command lines that restate a default value now take effect, because an
  explicit command-line value always wins;
- Fluent catalogues keyed on generated message identifiers, which changed
  shape alongside documentation IR version 2.0;
- Rust code that builds `AgentContext` or `DocMetadata` with a struct literal;
  and
- scripts that spell a boolean flag value with a space, such as
  `--flag false`, which is still rejected.

Everything else is additive. The sections below separate the changes to review
from the improvements that can be adopted when useful. Every example in this
guide is compiled, run, or parsed by the project's test suite.

## Impact at a glance

| Priority                | Area                                      | What to do                                                         |
| ----------------------- | ----------------------------------------- | ------------------------------------------------------------------ |
| **Required**            | Dependency versions                       | Update every OrthoConfig crate requirement to v0.10.0 together.    |
| **Review**              | Explicit CLI values                       | Check command lines that restate defaults; they now take effect.   |
| **Review if localized** | Fluent identifiers and IR 2.0             | Re-key catalogues that used the dotted identifier forms.           |
| **Required if used**    | `AgentContext` and `DocMetadata` literals | Use `ProfilesDeclaration` constructors and name `profiles: None`.  |
| **Review if scripted**  | Boolean flag values                       | Write `--flag=false`; the space-separated form is still rejected.  |
| **Optional**            | Named profiles                            | Add `profiles` to offer `[profile.<name>]` overlays.               |
| **Optional**            | Stacked file scopes                       | Opt into `automatic_mode = "stack_scopes"`.                        |
| **Optional**            | Injected environment sources              | Replace process-environment mutation in tests with `MapEnv`.       |
| **Optional**            | Clap string defaults                      | Mark fields `cli_default_as_absent` and drop duplicated defaults.  |
| **Optional**            | Agent-native metadata and policy          | Declare `behaviour(...)` and run `--check-agent-native`.           |
| **Optional**            | Cargo external subcommands                | Wrap a hand-built parser with `external_subcommand`.               |
| **Optional**            | Standalone localization runtime           | Depend on `ortho_l10n` directly when only Fluent lookup is needed. |

_Table 1: Application-facing work when moving from v0.9.0 to v0.10.0._

## 1. Update dependency versions

Update the runtime crate, and any direct macro or tooling dependency, as one
unit.

Before:

<!-- tested-example: migration-dependencies-before -->
```toml
[dependencies]
ortho_config = { version = "0.9.0", features = ["yaml"] }
```

After:

<!-- tested-example: migration-dependencies-after -->
```toml
[dependencies]
ortho_config = { version = "0.10.0", features = ["yaml"] }
```

Most applications only need `ortho_config`; it re-exports the derive macros. If
application source imports `ortho_config_macros` directly, update that
requirement to the same version. Update an installed `cargo-orthohelp` at the
same time so that the documentation tooling reads the new IR version.

## 2. Expect explicit CLI values to win

An explicit command-line value now wins even when it equals the struct default.
This corrects a layering defect: the generated guard compared the whole parsed
CLI object against the whole defaults object and skipped the CLI layer when the
two matched. A configuration whose command line restated its own default
therefore discarded that layer, and a lower-precedence file or environment
value silently won instead.

The defect was invisible in most applications, because any other differing
field made the two objects unequal. It surfaced whenever the command line
covered every defaulted field with a value equal to that field's default. A
single-field configuration reaches that state as soon as the user restates the
one default:

<!-- tested-example: migration-explicit-cli -->
```rust
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_")]
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

With v0.9.0, `ACME_PORT=9000 cargo run -- --port 8080` printed `port=9000`,
discarding the explicit argument. With v0.10.0 the command line wins, and
omitting the argument still defers to the environment:

<!-- tested-example: migration-explicit-cli-run -->
```console
$ ACME_PORT=9000 cargo run -- --port 8080
port=8080
$ ACME_PORT=9000 cargo run
port=9000
```

No migration is required. Applications that came to rely on the discarded value
can restore the previous outcome by omitting the argument, which is the
supported way to say "let a lower layer decide".

## 3. Re-key Fluent catalogues for IR 2.0

Derived configurations now implement `OrthoConfigLocalization`, and the
generated message identifiers changed shape. Documentation IR version rises from
`1.1` to `2.0` because identifier _values_ change meaning for consumers keyed
on the old shapes:

- the dotted `{app}.fields.{field}.help` form becomes the hyphenated
  `{mounted-path}-args-{field}-help`; and
- nested command defaults now extend the mounted command path rather than a
  standalone base.

Re-spell catalogues carefully, because the separators are load-bearing. Roots
and clap flag-style names are hyphen-joined, but an argument's own segment
keeps the raw Rust field name, so an underscored field yields `is_dry_run`, not
`is-dry-run`. The fixed suffixes `long_help` and `value_name` are
underscore-separated for the same reason. Normalization preserves both `-` and
`_`, so the two spellings produce _distinct_ keys: a respelled entry translates
cleanly and is simply never looked up.

Rather than predicting identifiers, read them from the generated constants, as
the user's guide shows in
[Use the generated message identifiers](users-guide.md#use-the-generated-message-identifiers),
or export the whole inventory as described in
[Export identifier artefacts for tooling](users-guide.md#export-identifier-artefacts-for-tooling).
Set `#[ortho_config(localization_base = "acme.cli")]` to choose the catalogue
base explicitly; when it is omitted, the application name supplies the default.
Normalized argument-identifier collisions within one derived struct are
compile-time errors. Hand-built command trees keep their existing runtime
validation, so retain any checks around them.

Consumers that pin the IR version should read
`ortho_config::docs::ORTHO_DOCS_IR_VERSION` rather than hard-code either value.
Older documents remain readable.

## 4. Update `AgentContext` and `DocMetadata` struct literals

Two Rust-level changes affect only code that builds these types with a struct
literal. Neither changes the serialized form.

`AgentContext.profiles` is retyped from `SupportDeclaration` to
`ProfilesDeclaration`. A literal that fed a `SupportDeclaration` into
`AgentContext.profiles` no longer type-checks. Start from `AgentContext::new`
and assign the field through a constructor instead:

<!-- tested-example: migration-agent-context-profiles -->
```rust
use ortho_config::{AgentContext, ProfilesDeclaration, serde_json};

fn main() -> Result<(), serde_json::Error> {
    let mut context = AgentContext::new("acme");
    context.profiles = ProfilesDeclaration::unsupported();
    println!("{}", serde_json::to_string(&context.profiles)?);
    Ok(())
}
```

The example prints `{"supported":false}`, the same wire shape as before. Use
`ProfilesDeclaration::supported(...)` for a struct that opts into profiles. The
constructors also protect callers from _future_ field additions.

`DocMetadata` gains a `profiles` field in both copies of the type:
`ortho_config::docs::DocMetadata`, the library-side type, and
`cargo_orthohelp::schema::DocMetadata`, the bridge-side mirror. The field is
`#[serde(default)]`, so JSON written against the previous release still
deserializes. A struct literal must still name it: add `profiles: None` to
preserve the previous behaviour. The derive only produces `Some` for a struct
declared with `#[ortho_config(profiles)]`. The field carries the new
`DocProfilesMeta` type, which holds the selector's `flag` and `env_var` as
plain strings.

## 5. Review boolean flag spellings

Generated boolean flags now accept an optional `=<BOOL>` value. Before, a
boolean flag was a presence-only switch that could only express `true`, so a
`true` from a configuration file or environment variable could not be cleared
from the command line, and `--excited=false` was rejected outright.

Now `--flag=false` supplies an explicit `false` that overrides any lower layer,
while omitting the flag still defers to it. With the
[boolean example from the user's guide](users-guide.md#switch-a-boolean-off-from-the-command-line)
and this `.acme.toml`:

<!-- tested-example: migration-boolean-file -->
```toml
excited = true
```

the explicit form clears the configured value for one run:

<!-- tested-example: migration-boolean-run -->
```console
$ cargo run -- --excited=false
excited=false
$ cargo run
excited=true
```

The bare flag and the omitted flag keep their previous meanings, so the change
is additive for every caller. The space-separated spelling is the exception:
`--excited false` is still rejected with clap's
`unexpected argument 'false' found` error, because the value must follow an
`=`. That spelling was never a way to express `false`. The same rules apply to
`Option<bool>` fields, which continue to distinguish "supplied nowhere" from an
explicit `false`.

Generated man pages now print the `--flag[=BOOL]` form, with the bracketed
placeholder italicized and joined to the flag so that it cannot be misread as
the invalid `--flag =BOOL` spelling. PowerShell help explains the optional
value in prose instead. The documentation IR reports
`CliMetadata.value_optional = true` alongside the `BOOL` value name and the
`true` and `false` possible values; older documents remain readable because the
field carries a serde default. Review any test that asserted the bare `--flag`
spelling in generated help.

## 6. Adopt new capabilities when useful

The capabilities below are opt-in. Applications that do not use them need no
changes.

### Named profiles

Add `profiles` to a struct's `#[ortho_config(...)]` attribute to offer
`[profile.<name>]` overlays selected with `--profile <name>` or
`<PREFIX>PROFILE`. Profiles form a new tier between configuration files and
environment variables:

<!-- tested-example: migration-profile-precedence -->
```plaintext
built-in defaults < config files < selected profile < environment < flags
```

The generated `load_with_profile()` and `load_with_profile_from_iter(...)`
entry points return a `ProfileLoadOutcome` that reports which profile was
selected; `load()` and `load_from_iter()` keep their signatures. The user's
guide section
[Switch between named profiles](users-guide.md#switch-between-named-profiles)
has a complete, tested walkthrough, including selection rules and errors.

Weigh two things before adopting the attribute:

- opting in reserves the `profile` key, the `--profile` flag, and the
  `<PREFIX>PROFILE` variable; an existing field that claims any of them is a
  compile-time error, so add the attribute as part of a normal build and test
  cycle; and
- the change is reversible, but removing the attribute later removes the
  `--profile` flag, which breaks scripts that use it, and leaves any
  `[profile.*]` tables as inert unknown keys.

Profile bodies must not contain `cmds` or `inherits`, and `[profile.default]`
is reserved; both fail with structured errors described in the user's guide.

### Stacked configuration scopes

Discovery still stops at the first file that loads, `AutomaticMode::FirstWins`,
by default. Opting into `AutomaticMode::StackScopes` makes every applicable
file that loads in each requested scope contribute a layer, with later scopes
overriding earlier ones. Opt in with the `automatic_mode = "stack_scopes"`
derive key, or with `ConfigFilePolicy::from_builder(...).automatic_mode(...)`
on a hand-built policy. See
[Control which configuration files become layers][users-guide-scoped-discovery].

### Injected environment sources

`load()`, `load_from_iter()`, and the existing subcommand merge methods
continue to read the process environment. When tests or an embedding
application must supply every environment value explicitly, use `MapEnv` with
the new source-aware entry points:

- `load_from_iter_with_sources` and `compose_layers_from_iter_with_sources` on
  any derived configuration;
- `load_and_merge_with_sources` on an `OrthoConfig`-derived subcommand;
- `load_globals_and_merge_selected_subcommand_with_sources` for an enum of
  subcommands; and
- `SubcommandFileContext` with the `_with_sources_at` loaders when file
  discovery inputs must be supplied too.

`CsvEnv::with_source` replays the declarative environment options against an
injected source. Providers that use `map()` or `filter_map()` cannot be
injected, because their closures cannot be replayed, and are rejected rather
than silently remapped. The user's guide sections
[Resolve discovery and merging without process state][users-guide-sources] and
[Choose the file-discovery source][users-guide-discovery] have tested examples
of each entry point.

### Infer parser-faithful string defaults

Enable `cli_default_as_absent` on a field whose clap default should remain
below configuration-file and environment values. Before, the default had to be
written twice, once for clap and once for OrthoConfig:

<!-- tested-example: migration-cli-default-before -->
```rust
use clap::Parser;
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_")]
struct ServeArgs {
    #[arg(long, default_value = "8080")]
    #[ortho_config(default = 8080, cli_default_as_absent)]
    port: u16,
}

fn main() -> OrthoResult<()> {
    let config = ServeArgs::load()?;
    println!("port={}", config.port);
    Ok(())
}
```

Now the derive infers the default from clap's `default_value` string, so the
duplicate can go:

<!-- tested-example: migration-cli-default-after -->
```rust
use clap::Parser;
use ortho_config::{OrthoConfig, OrthoResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ACME_")]
struct ServeArgs {
    #[arg(long, default_value = "8080")]
    #[ortho_config(cli_default_as_absent)]
    port: u16,
}

fn main() -> OrthoResult<()> {
    let config = ServeArgs::load()?;
    println!("port={}", config.port);
    Ok(())
}
```

<!-- tested-example: migration-cli-default-run -->
```console
$ cargo run
port=8080
$ ACME_PORT=9000 cargo run
port=9000
$ ACME_PORT=9000 cargo run -- --port 7000
port=7000
```

The inferred default fills the generated defaults layer, so it applies when
nothing else supplies the port, an environment value outranks it, and an
explicit argument still wins. When the same struct is merged as a subcommand
with a match-aware loader such as `load_and_merge_with_matches`, the merge
starts from the struct's `Default` implementation instead, so implement
`Default` to agree with the clap default there. `default_value_t` and
`default_values_t` keep their existing inference paths.

Earlier releases rejected `cli_default_as_absent` in any crate that did not
declare its own `serde_json` feature, because the derive tested the calling
crate's features. The derive no longer tests features, so an application that
added a `serde_json` feature only to satisfy it can remove that feature.

String `default_value` is parsed by a synthetic clap argument using the same
parser metadata as the field. This preserves behaviour for scalar primitive and
standard-library types, `Option<T>` and `Vec<T>` fields, `ValueEnum` fields,
and fields with a custom `#[arg(value_parser = ...)]` parser. Parser settings
that affect the result, such as a value delimiter or case-insensitive enum
parsing, are replayed with the generated argument. An explicit
`#[ortho_config(default = ...)]` always takes precedence over an inferred clap
default.

Nested `Option` or `Vec` wrappers and map fields are rejected at compile time
when combined with an inferred `default_value`, because their shape cannot be
reconstructed faithfully. Use an explicit `#[ortho_config(default = ...)]` for
those fields. If clap cannot parse a supported field's default, loading returns
`OrthoError::DefaultValueConversion` through the normal accumulated error path
rather than panicking; handle it alongside other load failures when
field-specific diagnostics matter.

### Agent-native behaviour metadata and policy

Add a struct-level `behaviour(...)` declaration when an agent needs explicit
interaction, mutation, bypass, or dry-run metadata, as shown in
[Declare behaviour metadata for agents][users-guide-behaviour]. The declaration
is carried through the documentation IR into agent-context schema version 1 as
`interaction_mode`, `mutation_effect`, `bypass_flag`, and `dry_run_flag`. The
agent-context change is additive: when interaction or mutation is not declared,
the values remain `unknown`, and nothing is inferred from command names or
flags.

Run `cargo orthohelp --check-agent-native --package <package>` to write a
policy report. The `--package` argument is required when the workspace has no
root package, such as a virtual workspace. Configure the policy in the
`[package.metadata.ortho_config.policy]` table, whose `off`, `warn`, and `deny`
modes select disabled, advisory, and failing behaviour; the default is `off`.
See [Agent-native policy checking][users-guide-policy] for the report shape,
exceptions, the `--policy-mode` override, and a CI recipe.

`cargo-orthohelp` also changed how it uses standard output. Without the policy
check, `--format agent-context` and `--format all` emit the agent-context JSON
on standard output as well as writing `agent-context.json`. With the policy
check, the policy report is the only JSON document on standard output;
successful checks continue generation and still write every selected artefact,
while a deny-mode violation fails before generator artefacts are created.
Pipelines that read standard output should expect the matching document.

### Cargo external-subcommand helper

Cargo invokes `cargo <name>` by executing `cargo-<name>` with `<name>` injected
as the first argument after the executable name. A hand-built parser that only
models the tool's options rejects that token before application logic runs.
Wrap the existing command with `ortho_config::cargo::external_subcommand` at
the entry-point boundary, then read the wrapped options through
`subcommand_matches("<name>")`. The helper does not alter OrthoConfig's merge
precedence. Derive-based callers can keep a single-variant
`#[command(subcommand)]` wrapper instead. See
[Cargo external-subcommand entry points][users-guide-cargo] for a tested
example and the binary-level obligations the helper leaves to the caller.

### Standalone localization runtime

`ortho_l10n` is a new publishable crate carrying the runtime half of
localization: `Localizer`, `FluentLocalizer` and its builder, the formatting
reports, the embedded English and Japanese catalogues, and the fallible
message-identifier convention. **No migration is required.** `ortho_config`
re-exports the same types rather than defining duplicates, so existing imports
keep working and code that mixes both paths names the same types.

Depend on `ortho_l10n` directly only when an application needs catalogue lookup
and Fluent formatting without loading configuration. Such a consumer resolves
36 packages rather than the facade's 139, because it avoids configuration
discovery, the derive macros, Figment, and Clap. A program that uses
`#[derive(OrthoConfig)]` or parses a Clap command still needs `ortho_config`.

<!-- tested-example: migration-l10n-dependency -->
```toml
[dependencies]
ortho_l10n = "0.10.0"
```

<!-- tested-example: migration-l10n-runtime -->
```rust
use ortho_l10n::{FluentLocalizer, Localizer, langid};

fn main() -> Result<(), ortho_l10n::FluentLocalizerError> {
    let localizer = FluentLocalizer::builder(langid!("en-US")).try_build()?;
    let about = localizer.message("cli.about", None, "fallback");
    println!("{about}");
    Ok(())
}
```

Lookup tries the exact identifier and then its dotted-to-hyphen spelling, so
`cli.about` resolves the embedded `cli-about` message and prints
`OrthoConfig command-line interface.`. See the
[ortho_l10n README](../ortho_l10n/README.md) for consumer catalogues,
precedence, and formatting reports.

## Upgrade checklist

- [ ] Update every OrthoConfig crate requirement, and `cargo-orthohelp`, to
  v0.10.0.
- [ ] Run tests with the feature combinations shipped by the application.
- [ ] Audit command lines that restate defaults across every defaulted field;
  they now take effect instead of being discarded.
- [ ] Re-key Fluent catalogues that used the dotted identifier forms, and
  accept documentation IR version `2.0` in any consumer that pins it.
- [ ] Replace `SupportDeclaration` literals for `AgentContext.profiles` with
  `ProfilesDeclaration` constructors, and add `profiles: None` to `DocMetadata`
  literals.
- [ ] Review generated-help assertions that expected a bare `--flag` spelling.
- [ ] Use `--flag=false` in any workflow that needs to clear a configured
  `true`; leave other invocations unchanged.
- [ ] Adopt profiles, stacked scopes, injected sources, behaviour metadata,
  and the Cargo helper only where the application needs them.

[users-guide-behaviour]: users-guide.md#declare-behaviour-metadata-for-agents
[users-guide-cargo]: users-guide.md#cargo-external-subcommand-entry-points
[users-guide-discovery]: users-guide.md#choose-the-file-discovery-source
[users-guide-policy]: users-guide.md#agent-native-policy-checking
[users-guide-scoped-discovery]: users-guide.md#control-which-configuration-files-become-layers
[users-guide-sources]: users-guide.md#resolve-discovery-and-merging-without-process-state
