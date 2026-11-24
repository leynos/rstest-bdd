# Architectural decision record (ADR) 025: the nested-Cargo preparation gate keeps a boolean query

## Status

Accepted (2026-11-19), **superseded in part** (2026-11-19): the original
decision kept the gate in one shape — a private `matching_stamp` predicate
behind a public entry point that may prepare — and declined a public typed
query. That decline is now withdrawn. The gate is split into a read-only
admission query with a typed answer and a separately named build-capable entry
point. The reasoning that declined the *original* proposal is kept below,
because the replacement was designed to answer it rather than to ignore it. See
[Superseding decision](#superseding-decision-2026-11-19).

## Date

2026-11-19; superseding section added 2026-11-19.

## Context and problem statement

`crates/rstest-bdd-harness/src/nested_cargo/preparation/lifecycle.rs` exposed,
at the time of the original decision, one public gate per runner position:

- `ensure_prepared` decided by runner and either admitted a prepared tree or
  prepared one, reading `NEXTEST_RUN_ID` itself.
- `ensure_prepared_with` took the runner decision as a parameter, and
  `ensure_prepared_reported` took the injected `Instruments` as well, so the
  protocol could be exercised without mutating the environment or spawning
  `cargo -vV`.
- `prepare_now` is the unconditional preparation the `prepare-nested-fixtures`
  target reaches.
- `invalidate` withdraws a stamp a witness has refuted.

The stamp decision itself is `matching_stamp`, which is private and answers a
boolean: does a current stamp describe a tree whose recorded artefacts are all
present? A reviewer asked for a public typed query that returns *why* a tree
was or was not admitted, and for the mutating entry point to be renamed so that
a caller cannot reach a build through something named like a query.

The observation behind the request is real: `ensure_prepared` could run a
build, and its name did not say so. What was disputed was the remedy, and
[Superseding decision](#superseding-decision-2026-11-19) records the remedy
that was eventually taken.

## Decision drivers

- A timed assertion's whole purpose is to *not* build. A caller that could
  reach a build through a function it believed to be a query would defeat the
  budget the preparation exists to protect.
- The decision's inputs are not all available to a caller. `matching_stamp`
  compares the stamp against the artefact list the `Preparation` derived, which
  includes the profile directories `profiles.rs` computed from the child's
  target selection. A public query would have to either expose that derivation
  or accept a caller's copy of it.
- Every caller in this repository reaches the gate through a family helper
  (`ensure_cli_prepared`, `ensure_gated_with`, `TrybuildTree::preparation`)
  that already names its position. No caller needs the decision without acting
  on it.
- The diagnostics that explain a refusal already travel with the result: the
  gate's error names the family, the stamp path, and the missing artefacts.

## Considered options

1. **Public typed query plus a renamed mutating entry point.** Return a decision
   value carrying the reason, and require callers that may build to call a
   function whose name says so.
2. **Keep the boolean gate; document the build.** Keep `matching_stamp` private,
   keep `ensure_prepared` able to prepare, and make the error documentation and
   the module prose state exactly when a build happens and under which runner.
3. **Split the gate into a query and a repair step at the call sites.** Have
   each family ask "is it admitted?", then separately ask for preparation.

## Decision outcome

Option 2, for this surface and this PR.

Option 1 was declined because, as it was then specified, it bought a
caller-visible decision that no caller in this workspace wanted, at the cost of
exposing the artefact and profile derivation that `profiles.rs` and
`artefacts/` exist to keep in one place. A public decision type would either
become a second source of truth for "what counts as prepared" or force those
modules' internals into the public API.
[Superseding decision](#superseding-decision-2026-11-19) records how a query
was later added without either cost.

Option 3 was declined because it makes every call site responsible for the same
two-step sequence, and a site that asked only the first question would silently
skip preparation — the failure mode the gate exists to prevent.

What the declined request did change, and what this PR does carry:

- The gate's `# Errors` section distinguishes the two runners rather than
  claiming the read error is always returned. Under plain `cargo test` an
  unreadable stamp is logged and prepared over; under Nextest it is refused.
- The module prose describes the lazy recovery, the process-wide
  `PREPARE_LOCK`, and the `cargo-spawning` test group's role in ordering
  accurately, rather than implying the mutex serializes independent processes.
- The stderr reporter is injected through the reported entry points —
  `ensure_prepared_building_reported`, `prepare_now_reported` and
  `invalidate_reported` — so the diagnostics are observable from a test instead
  of only from a terminal.

The saved request for a public typed query was therefore recorded here as
**declined** at the time, and is withdrawn by the superseding section below.

The separation of `stamp.rs`'s responsibilities was deferred by this decision
and has since been carried out. `preparation/stamp.rs` is now a module root
that re-exports its parts, and each part lives in its own file:

- `stamp/format.rs` reads and writes the file's grammar — the version and
  section constants, `StampText`, `Stamp`, `parse` and `strip_line_break`.
- `stamp/fingerprint.rs` composes the text a stamp is compared against —
  `Fingerprint`, `compose`, `fingerprint_with`, `escape` and
  `is_fingerprint_variable`. `fingerprint_with` remains the only function that
  calls the injected probe.
- `stamp/file.rs` names and reads the stamp and does nothing else —
  `stamp_path` and `matching_stamp`.
- `stamp/diagnostics.rs` builds the messages a refusal produces. Both builders
  are pure: `missing_stamp_message` takes the descriptions the lifecycle caller
  gathered from `artefacts::evicted`, rather than inspecting the directories
  itself.

Stderr reporting left the stamp module with them. `lifecycle::report_stderr` is
the one binding production uses, reached through `lifecycle::reporter_of`, so
the diagnostics belong to the gate that decides to emit them rather than to the
protocol that was refused.

Moving that reporter and the artefact inspection into `lifecycle.rs` took it
past the repository's per-file line budget, so the gate is a module root too:

- `lifecycle/instruments.rs` holds the injected boundaries —
  `Instruments`, `reporter_of`, `report_stderr`, `probe_of` and
  `instruments_of` — with the production probe and reporter paired in one
  place, so the compiler the gate identifies and the stream it writes through
  cannot drift apart.
- `lifecycle.rs` keeps the decision and the build: the build-capable entry
  points, the `Target` view passed to `prepare_locked`, and the locking and
  stamping that follow a build.
- `lifecycle/admission.rs` holds the read-only query — `Admission`,
  `UnreadableStamp`, `admit` and `admit_with_probe` — so a name that reads like
  a question is implemented in a module that can only answer it.

The parent re-exports all three, so `lifecycle::Instruments`,
`lifecycle::reporter_of`, `lifecycle::instruments_of`, `lifecycle::admit` and
`lifecycle::ensure_prepared_building` resolve for `deadline`, `invalidate` and
the workspace tests.

No call site outside `preparation/` moved: the module root re-exports the same
names under the same visibility. The split is recorded here because it is the
deferral this decision named, not because it changes the decision.

## Consequences

- A caller cannot obtain the stamp decision without either acting on it or
  taking the preparable entry point; the superseding decision below keeps that
  property while making the preparable entry point's name admit what it does.
- The reason a tree was refused is carried by the gate's error rather than by a
  decision value, so there is one explanation path rather than two. The
  superseding decision adds an *answer* path beside it without replacing the
  error the gate raises.
- If a future caller needs to distinguish refusal reasons programmatically, the
  error chain's source carries them today; that is the extension point to use
  before widening the public surface. `Admission` is the narrow exception,
  added only for the inaction guarantee the section below describes.

## Superseding decision (2026-11-19)

The typed query is **adopted**, in a form that pays neither cost Option 1 was
declined for. The public surface is now:

- `admit(preparation, under_nextest) -> io::Result<Admission>` is the read-only
  admission query. It performs no build, writes no stamp and takes no lock. Its
  answer is a typed [`Admission`] with three variants: `Reusable`,
  `NotPrepared` and `Unreadable(UnreadableStamp)`.
- `admit_with_probe(preparation, under_nextest, probe)` is the same query with
  the toolchain probe supplied, so the answer can be exercised against a chosen
  compiler identity without spawning `cargo -vV`.
- `ensure_prepared_building(preparation, under_nextest, prepare)` is the
  build-capable entry point. Its name states that it may prepare, and its
  `PreparationOutcome` return value — `Reused` or `Prepared` — states whether a
  build ran.
- `ensure_prepared_building_reported` takes the injected `Instruments` as well.

What changed from the declined proposal, and why it is now acceptable:

- **No derivation is exposed.** The query is not a second implementation of
  "what counts as prepared". It consumes the same private `stamp::read_stamp`
  answer and calls the same `artefacts::all_present` predicate the gate does,
  so the two cannot disagree. The split that makes it possible is `read_stamp`
  returning a `StampReading { fingerprint, decision }` — one composition of the
  fingerprint serving both the comparison and the later stamp write — rather
  than the caller recomposing either.
- **The runner decision is no longer read inside the gate.** Every public
  entry point takes `under_nextest` as a parameter. The environment read moved
  to the call sites that resolve it — `nested_cargo::running_under_nextest` is
  now called by the test-support boundary (`TrybuildTree::ensure_prepared`), by
  the workspace suites that already took the decision explicitly, and nowhere
  inside a gate whose name does not mention it.
- **The two refusal variants earn their place by different callers.** `admit`
  exists for callers that need to *report* whether a tree is reusable without
  acting on the answer — the same role a diagnostics-first workflow needs — and
  the no-build/no-stamp/no-lock guarantee is exactly what makes it safe to
  reach from a context that must not prepare. `ensure_prepared_building`
  remains the only entry point that can build.

The tests added with the split live in `preparation/tests/admission.rs` and pin
each refusal separately: a cold tree answers `NotPrepared` and leaves no stamp,
a warm tree answers `Reusable` and its stamp comes back byte-identical, an
evicted profile answers `NotPrepared`, an unreadable stamp answers `Unreadable`
under Nextest and `NotPrepared` under plain `cargo test`, and the query answers
while another thread holds `PREPARE_LOCK`.

## Links

- `crates/rstest-bdd-harness/src/nested_cargo/preparation/lifecycle.rs`
- `crates/rstest-bdd-harness/src/nested_cargo/preparation/lifecycle/`
- `crates/rstest-bdd-harness/src/nested_cargo/preparation/profiles.rs`
- `crates/rstest-bdd-harness/src/nested_cargo/preparation/artefacts/`
- `crates/rstest-bdd-harness/src/nested_cargo/preparation/stamp/`
- ADR-010 (feature-file change detection) is the sibling decision about what a
  stamp must cover; this ADR is about how the stamp is queried.
