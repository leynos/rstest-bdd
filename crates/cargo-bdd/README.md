# cargo-bdd

*Where Rustaceans send their step library for a health inspection.*

> **TL;DR**: `cargo bdd` is the nosy cousin of `rstest-bdd`. It builds every
> test target with `cargo test --no-run`, reruns the binaries with
> `RSTEST_BDD_DUMP_STEPS=1 --dump-steps`, and reports what your steps get up to.

## Why this subcommand?

- **Keep scenarios honest**: compare what the feature files promise with what
  the registry actually exposes.
- **Sniff out dead vines**: `unused` highlights steps that never executed during
  that run so you can prune them before they rot.
- **Spot copy-pasta**: `duplicates` groups identical keyword+pattern pairs so
  you can refactor instead of playing whack-a-mole.
- **Stay inside `cargo`**: no bespoke runner, no sidecar daemon—just another
  subcommand that speaks the same toolchain dialect as the rest of your tests.

Think of it as the kitchen inspector for your courgette-driven development
setup: it will politely tap every saucepan and make sure no scenario is serving
mouldy leftovers.

## Commands on the menu

- `cargo bdd steps` — prints every registered step with its keyword, pattern,
  and source location.
- `cargo bdd unused` — lists steps flagged `used = false` in that same process
  execution.
- `cargo bdd duplicates` — groups definitions that share both keyword and
  pattern, separating each duplicate set with `---`.

The user’s guide showcases invocations such as `cargo bdd unused --quiet` and
`cargo bdd duplicates --json`. Follow your local appetite for formatting when
consuming the raw output.

## How it raids your step stash

1. Ask Cargo for metadata, then build each workspace test target with
   `cargo test --no-run --message-format=json --all-features`.
1. Collect every compiled test binary path from the JSON stream.
1. For each binary, run it with `RSTEST_BDD_DUMP_STEPS=1 --dump-steps` to make
   it spill a JSON inventory of registered steps.
1. Stitch the responses together and apply whichever subcommand filter you
   requested.

Any binary that crashes without recognising `--dump-steps` is politely skipped
(the tool assumes it simply is not an `rstest-bdd` test). Binaries that fail
for other reasons cause the subcommand to error, so you still notice genuinely
broken builds.

## Practical rituals

- Run behavioural tests first if you want accurate `unused` results—the flag is
  per-process, so only steps exercised during that diagnostic run count as
  “used”.
- Keep `#[scenario]` tests compiling even when you are mid-refactor; the
  subcommand leans on `cargo test --no-run`, so red builds mean no diagnostics.
- Pair the tool with CI to catch stale steps: dumping the registry into
  artefacts makes it easier to review churn alongside feature changes.
- When the JSON output looks crowded, pipe it into your formatter of choice or
  reach for the options highlighted in the user’s guide.

## Troubleshooting

- **“Unknown option '--dump-steps'”** — the binary was not built with
  `rstest-bdd` instrumentation; the tool shrugs and moves on.
- **“cargo test failed … skipping”** — the target failed to compile or link.
  Fix the test crate before expecting step diagnostics.
- **Empty output** — either the workspace exposes no test targets, or every
  target ignored the dump flag. Double-check you imported `rstest-bdd` in the
  crate you are probing.

## Further reading

For deeper lore—including how fixtures bind to steps, why the registry cares
about `used`, and the philosophy behind keeping BDD inside the Rust testing
stack—curl up with `docs/users-guide.md`.

Now go forth, interrogate your steps, and may every duplicate be composted into
fresh behaviour.
