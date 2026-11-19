# Architectural decision record (ADR) 025: the nested-Cargo preparation gate keeps a boolean query

## Status

Accepted (2026-11-19): the gate keeps its current shape — a private
`matching_stamp` predicate behind a public `ensure_prepared` that may prepare.
The alternative, a public typed query returning the stamp-and-artefact decision
and a separately named mutating entry point, is declined for this surface.

## Date

2026-11-19.

## Context and problem statement

`crates/rstest-bdd-harness/src/nested_cargo/preparation/lifecycle.rs` exposes
one public gate per runner position:

- `ensure_prepared` decides by runner and either admits a prepared tree or
  prepares one.
- `ensure_prepared_with` takes the runner decision as a parameter, and
  `ensure_prepared_reported` takes the injected `Instruments` as well, so the
  protocol can be exercised without mutating the environment or spawning
  `cargo -vV`.
- `prepare_now` is the unconditional preparation the `prepare-nested-fixtures`
  target reaches.
- `invalidate` withdraws a stamp a witness has refuted.

The stamp decision itself is `matching_stamp`, which is private and answers a
boolean: does a current stamp describe a tree whose recorded artefacts are all
present? A reviewer asked for a public typed query that returns *why* a tree
was or was not admitted, and for the mutating entry point to be renamed so that
a caller cannot reach a build through something named like a query.

The observation behind the request is real: `ensure_prepared` can run a build,
and its name does not say so. What is disputed is the remedy.

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

Option 1 was declined because it buys a caller-visible decision that no caller
in this workspace wants, at the cost of exposing the artefact and profile
derivation that `profiles.rs` and `artefacts/` exist to keep in one place. A
public decision type would either become a second source of truth for "what
counts as prepared" or force those modules' internals into the public API.

Option 3 was declined because it makes every call site responsible for the same
two-step sequence, and a site that asked only the first question would silently
skip preparation — the failure mode the gate exists to prevent.

What the declined request did change, and what this PR does carry:

- `ensure_prepared`'s `# Errors` section distinguishes the two runners rather
  than claiming the read error is always returned. Under plain `cargo test` an
  unreadable stamp is logged and prepared over; under Nextest it is refused.
- The module prose describes the lazy recovery, the process-wide
  `PREPARE_LOCK`, and the `cargo-spawning` test group's role in ordering
  accurately, rather than implying the mutex serializes independent processes.
- The stderr reporter is injected through `ensure_prepared_reported`,
  `prepare_now_reported` and `invalidate_reported`, so the diagnostics are
  observable from a test instead of only from a terminal.

The saved request for a public typed query is therefore **recorded as
declined** rather than as an implemented change. A future change that finds a
caller which genuinely needs the decision without acting on it should reopen
this ADR rather than growing the query beside the gate.

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

Stderr reporting left the stamp module with them. `lifecycle::report_stderr`
is the one binding production uses, reached through `lifecycle::reporter_of`,
so the diagnostics belong to the gate that decides to emit them rather than to
the protocol that was refused.

Moving that reporter and the artefact inspection into `lifecycle.rs` took it
past the repository's per-file line budget, so the gate is a module root too:

- `lifecycle/instruments.rs` holds the injected boundaries —
  `Instruments`, `reporter_of`, `report_stderr`, `probe_of` and
  `instruments_of` — with the production probe and reporter paired in one
  place, so the compiler the gate identifies and the stream it writes through
  cannot drift apart.
- `lifecycle.rs` keeps the decision and the build: the four entry points, the
  `Target` view passed to `prepare_locked`, and the locking and stamping that
  follow a build.

The parent re-exports both, so `lifecycle::Instruments`,
`lifecycle::reporter_of` and `lifecycle::instruments_of` resolve exactly as
before for `deadline`, `invalidate` and the workspace tests.

No call site outside `preparation/` moved: the module root re-exports the same
names under the same visibility. The split is recorded here because it is the
deferral this decision named, not because it changes the decision.

## Consequences

- The gate stays one function per runner position, and every caller's name
  states whether it may build.
- A caller cannot obtain the stamp decision without either acting on it or
  taking the preparable entry point.
- The reason a tree was refused is carried by the gate's error rather than by a
  decision value, so there is one explanation path rather than two.
- If a future caller needs to distinguish refusal reasons programmatically, the
  error chain's source carries them today; that is the extension point to use
  before widening the public surface.

## Links

- `crates/rstest-bdd-harness/src/nested_cargo/preparation/lifecycle.rs`
- `crates/rstest-bdd-harness/src/nested_cargo/preparation/lifecycle/`
- `crates/rstest-bdd-harness/src/nested_cargo/preparation/profiles.rs`
- `crates/rstest-bdd-harness/src/nested_cargo/preparation/artefacts/`
- `crates/rstest-bdd-harness/src/nested_cargo/preparation/stamp/`
- ADR-010 (feature-file change detection) is the sibling decision about what a
  stamp must cover; this ADR is about how the stamp is queried.
