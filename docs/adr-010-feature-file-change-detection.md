# Architectural decision record (ADR) 010: feature-file change detection for compile-time scenario binding

## Status

Proposed

## Date

2026-06-10

## Context and problem statement

`#[scenario(path = "...")]` and `scenarios!` read `.feature` files via
`std::fs` at macro-expansion time. Cargo tracks Rust sources and the outputs
of build scripts, but it cannot see file reads made by a procedural macro
through ordinary filesystem I/O. As a result:

- Editing only a `.feature` file does not trigger a rebuild of the scenario
  binary.
- The binary, and all its compiled expectations, remain stale until an
  unrelated `.rs` file in the crate changes.
- A test expectation that is corrupted in a `.feature` file can appear to
  *pass* from the Cargo build cache until something forces a fresh compilation.

The downstream adopter reported this foot-gun in practice: a deliberately
corrupted expectation appeared to pass until an unrelated source file was
touched. This is a severe correctness issue for a *testing* framework.

The decision is which mechanism should register the `.feature` file as a
rebuild dependency of the crate that binds it.

## Decision drivers

- Close the rebuild-invalidation gap so `.feature`-only edits trigger a
  recompilation.
- Keep the fix invisible to consumers wherever possible; adopters should not
  need to add a `build.rs` or change call sites.
- Do not embed absolute paths into compiled artefacts; they break reproducible
  builds (Nix sandboxes, cross-compilation, `sccache`/`buildcache` cache-key
  divergence, Windows versus POSIX path separators).
- Avoid increasing binary size more than necessary.
- Provide a documented mechanism for `scenarios!` (directory-glob binding)
  where the `.feature` file set is not known until build time.
- Treat invalidation as a tested contract, not advisory prose.

## Options considered

### Option A: macro-emitted `include_str!`

Have the `#[scenario]` macro emit an `include_str!("…/foo.feature")`
expression, discarded into a hidden item (for example under `#[doc(hidden)]`
or as an ignored binding), so rustc registers the path in dep-info.

Pros:

- Fully invisible to consumers: no `build.rs`, no extra files.
- rustc registers `include_str!` paths in dep-info automatically.
- Works for single-file `#[scenario]` binding without any consumer action.

Cons:

- `include_str!` resolves its path *relative to the invoking source file*,
  not relative to `CARGO_MANIFEST_DIR`. The macro must therefore emit a
  path that rustc resolves correctly from the call site. Using an absolute
  `CARGO_MANIFEST_DIR`-rooted path is **rejected** (see below); a
  call-site-relative path requires computing the offset from the call site's
  `Span` to the feature file, which is portable but requires care.
- Embeds the full feature-file text into the binary at the call site. For
  large suites with many scenarios, this adds binary-size overhead.
- The discarded item must not trip `dead_code` under a pedantic lint profile.
  An `#[allow(dead_code)]` annotation on the generated item or a `let _ = …`
  binding avoids this.
- `scenarios!` (directory-glob binding) must track every file discovered in
  the directory; emitting one `include_str!` per file handles this but makes
  the total embedded size proportional to suite size.

**Absolute-path `include_str!` variant — rejected.** The macro could
construct an absolute path from `CARGO_MANIFEST_DIR` and pass it to
`include_str!`. This is rejected because:

1. Absolute paths baked into artefacts break reproducible builds: two
   identical builds from different directories produce binaries with different
   embedded byte sequences, defeating `sccache`/`buildcache` and breaking
   hermetic build systems (Nix sandboxes, GitHub Actions cache).
2. Paths containing Windows drive letters or UNC prefixes are not portable
   across build environments.
3. The Cargo ecosystem treats absolute build-directory paths as a known
   portability hazard; the `$CARGO_MANIFEST_DIR` macro is intentionally
   restricted to use inside `include_str!` only when the relative-path form
   cannot be computed.

This option variant is therefore not a valid implementation of Option A.

### Option B: build-script helper emitting `cargo::rerun-if-changed`

Provide an `rstest-bdd-build` helper crate (or a function in `rstest-bdd`)
that consuming crates call from their `build.rs`. The helper scans the
features directory and emits:

```text
cargo::rerun-if-changed=tests/features
cargo::rerun-if-changed=tests/features/foo.feature
cargo::rerun-if-changed=tests/features/bar.feature
```

Modelled on the `theoremc` prior art
(<https://github.com/leynos/theoremc>), which always emits the directory
even when absent, adds nested sub-directories, and adds one line per
discovered file.

Pros:

- Does not embed any file content into the binary; no binary-size cost.
- Does not embed any absolute path into the artefact (the directive is a
  build-script output consumed by Cargo, not compiled into the binary).
- The natural fit for `scenarios!` (directory-glob binding): the build
  script scans the same directory.
- Emitting the directory line ensures that adding a new `.feature` file also
  triggers a rebuild (Cargo watches directory mtime for additions/deletions).

Cons:

- Requires consumer action: every crate using `#[scenario]` or `scenarios!`
  must add a `build.rs` (or a `build` key in `Cargo.toml`). This is not
  invisible.
- Emitting *any* `cargo::rerun-if-changed` directive switches Cargo to a
  narrow allow-list mode: if the build script emits a directive but omits a
  file that later changes, Cargo will not rebuild. The helper must emit one
  line per file discovered, not just the directory, to avoid this trap.
- An omission (forgetting to add a new `.feature` sub-directory) silently
  regresses invalidation. Mitigation: always emit the top-level directory
  line even when absent (Cargo ignores directives for non-existent paths),
  and scan recursively.

### Option C: `proc_macro::tracked_path` (unstable)

Use the unstable `proc_macro::tracked_path::path()` API, which is the
primitive designed for proc-macro file tracking and registers paths in
dep-info without embedding content.

Pros:

- The right long-term answer: no content embedding, correct scope (proc-macro
  layer), no build-script obligation.
- Does not require an absolute path; the proc-macro span provides the context
  to resolve a relative path.

Cons:

- Blocked on stabilisation (`proc_macro::tracked_path` is nightly-only as of
  the decision date).
- Usable behind a `nightly` feature gate during the window, but not as a
  default mechanism.

### Option D: OUT_DIR AST caching

Cache the parsed Gherkin ASTs in `OUT_DIR` so the macro only re-parses a
`.feature` file when its modification time has changed (noted in
`§3.2.2` of the design document).

This option is **orthogonal** — it is a *performance* optimisation (reducing
compile-time overhead for large suites) and does not by itself make Cargo
aware of `.feature` file changes. It does not close the rebuild-invalidation
foot-gun. Recorded here to keep the analysis complete; addressed separately
in `§3.2.2`.

| Axis | A (relative `include_str!`) | B (build script) | C (`tracked_path`) | D (OUT_DIR cache) |
| --- | --- | --- | --- | --- |
| Consumer-invisible | High | Low | High | Low |
| Binary-size cost | Medium | None | None | None |
| Absolute-path risk | None (relative only) | None | None | None |
| `scenarios!` fit | Medium (one per file) | High (directory scan) | High | None |
| Reproducible builds | High | High | High | N/A |
| Stable today | Yes | Yes | No | Yes |

*Table 1: Trade-offs for feature-file rebuild-invalidation mechanisms.*

## Decision outcome

Neither option is unambiguously superior across all axes. This ADR records
the trade-offs and establishes the binding constraints; the choice of mechanism
is deferred to the implementing ExecPlan (roadmap item 11.3.1), which has
access to the actual call-site span data and the `scenarios!` implementation.

Binding constraints for the implementing ExecPlan:

1. **Absolute-path `include_str!` is rejected.** The implementation must not
   embed an absolute `CARGO_MANIFEST_DIR`-rooted path into the compiled
   artefact for the reasons stated above.
2. **The fix must be treated as a tested contract.** A portability-aware
   regression test must prove that a `.feature`-only edit forces recompilation
   and a fresh test failure, modelled on `theoremc`'s
   `tests/build_discovery_bdd.rs`. The test must tolerate coarse filesystem
   `mtime` granularity and must be serialised so nextest's process-per-test
   parallelism cannot race on the workspace `target` directory.
3. **Option B (build script) is the preferred default** for
   `scenarios!` directory-glob binding. It avoids binary-size overhead and
   cleanly handles the case where the set of feature files is not known at
   macro-expansion time.
4. **Option A (relative-path `include_str!`) is preferred for
   `#[scenario]`** single-file binding, if the call-site span can yield a
   reliable relative path that rustc resolves correctly. This is the path of
   zero consumer friction.
5. **Option C (`tracked_path`)** is recorded as the right long-term answer.
   Usable behind a `nightly` feature gate; stabilisation should be monitored
   and adopted when available.
6. **Option D (OUT_DIR cache)** is out of scope for this ADR. It addresses
   compile performance, not invalidation correctness; `§3.2.2` tracks it
   separately.

## Testing strategy

The implementing ExecPlan (roadmap item 11.3.1) must cover three layers, in
addition to the binding constraint that invalidation is a tested contract:

1. **Rebuild-invalidation regression test (required).** A portability-aware
   integration test proves a `.feature`-only edit forces recompilation and a
   fresh failure, modelled on `theoremc`'s `tests/build_discovery_bdd.rs`. It
   must tolerate coarse filesystem `mtime` granularity (touch to a
   guaranteed-later timestamp, or tick a second), run serialised in its own
   process with an isolated `target`/temp directory so nextest's
   process-per-test parallelism cannot race on a shared workspace `target`,
   and — for the `include_str!` path — assert no absolute `CARGO_MANIFEST_DIR`
   path is embedded in the artefact (inspect expanded output or the compiled
   `.d` dep-info).
2. **Trybuild compile-time test (required).** Because the mechanism is emitted
   by the `#[scenario]`/`scenarios!` proc-macros, compile-time behaviour is
   part of the contract and must be pinned by `trybuild` fixtures, not left to
   the runtime regression test alone:
   - a **compile-pass** fixture proving the emitted `include_str!` (or
     build-script wiring) compiles cleanly for a representative `.feature`
     binding; and
   - a **compile-fail** fixture proving a `#[scenario(path = …)]` pointing at a
     missing `.feature` file still fails at compile time with a clear
     diagnostic, so the invalidation change does not regress the existing
     missing-file error path.

   These join the existing `rstest-bdd::trybuild_macros` / `*::macro_compile`
   suites and inherit their nextest slow-timeout override. Both fixtures are
   required acceptance criteria for roadmap item 11.3.1.
3. **Diagnostic snapshots (required for any touched diagnostic).** For any
   user-facing diagnostic the change touches (for example the missing-`.feature`
   error), pin the rendered message with a focused `insta` snapshot using stable
   redaction (`insta::with_settings!` filters over absolute paths, line/column
   numbers, and rustc version strings) so the snapshot is portable across
   machines and toolchains and does not drift like a raw `.stderr`. Prefer a
   focused snapshot over a whole-`.stderr` capture, and back it with explicit
   semantic or substring assertions on the load-bearing fragments — for example
   that the message names the offending `.feature` path and the `#[scenario]`
   call-site — so the test fails loudly on a meaning change even if an
   unrelated reflow would otherwise let a full-text snapshot drift unnoticed.
   The `trybuild` compile-fail `.stderr` and the `insta` snapshot are
   complementary: the former gates the diagnostic at the macro boundary, the
   latter pins its wording with redaction the raw `.stderr` cannot express.

## Consequences

- The rebuild-invalidation foot-gun is closed once 11.3.1 lands.
- Consumers of `#[scenario]` gain invisible rebuild tracking with no
  `build.rs` obligation (Option A path).
- Consumers of `scenarios!` gain rebuild tracking with an opt-in `build.rs`
  helper (Option B path).
- `§2.7.6.6` of the design document documents the foot-gun and this decision.
- `§3.2.2` of the design document is tightened to distinguish
  *invalidation* (this ADR) from *caching* (performance, a separate concern).
- A portability-aware regression test is added alongside the implementation,
  asserting invalidation as a contract.
- Compile-time behaviour is pinned as a required contract: `trybuild`
  compile-pass and compile-fail fixtures, plus redacted `insta` snapshots with
  semantic assertions for any touched diagnostic (see *Testing strategy*).

## Governs

- Roadmap item: Phase 11.3.1 ("Editing only a `.feature` file triggers a
  scenario rebuild"), targeted at v0.6.0 final.
- Design document: new `§2.7.6.6` (feature-file rebuild invalidation) and
  updated `§3.2.2`.
