//! INV-11: the runner surface mentions no frontend type.
//!
//! The runner exists so that a non-Gherkin frontend can drive scenarios without
//! depending on Gherkin, on Markdown, or on the existing reporting pipeline. A
//! single frontend type leaking into a public signature — as a field, a bound,
//! an associated type, an alias, or a re-export — would silently re-couple the
//! two and would not be noticed in review, because the offending line is not the
//! `pub fn` a reviewer reads.
//!
//! This is a source-level check rather than a `compile_fail` test. A
//! `compile_fail` test can only show that *one* written example fails; it cannot
//! show that no *other* leak exists. The question INV-11 asks is universal, so
//! the check has to be too.
//!
//! The scan reaches the tree through `cap-std` rather than through `std::fs`.
//! Whitaker's `no_std_fs_operations` lint denies `std::fs` across the workspace
//! and offers no test-only exemption: in-source `expect` attributes cannot
//! suppress it, so a `std::fs` version of this file fails `make lint`. Reaching
//! for `cap-std` here is therefore deliberate rather than incidental, and is the
//! same remedy the neighbouring test-support modules use.
//!
//! # What the scan does and does not establish
//!
//! It is a token scan over source text, so it establishes that no *spelling* of
//! a known frontend or reporting type appears in the runner. It cannot
//! establish that no frontend type reaches a public signature, because a
//! frontend type re-exported under an unrelated name, or reached through a
//! `crate::` path that never spells `gherkin`, would be invisible to it.
//! Resolving names properly would mean a compiler pass over the crate's public
//! API, which is a different and much larger instrument than a unit test.
//!
//! The scan is therefore a cheap tripwire over the shapes a leak realistically
//! takes in this codebase, not a proof of INV-11. It is worth having because
//! the realistic failure is a future edit that reaches for `gherkin::Step` for
//! convenience, and because it fails loudly on that edit. The aliasing hole is
//! real and is recorded here rather than papered over: `use crate::X as y;`
//! hides every later `y::T`, and no token list can close that in general.
//!
//! One thing the scan *does* establish is that it read the tree it claims to
//! police. A path the walk cannot list, open, or read is carried out as an
//! `unreadable` message and fails the check, rather than being skipped — an
//! unread file is as invisible as a clean one, which is the same silence.

mod walk;

use camino::Utf8Path;
use walk::{Scanned, scan_root};

/// Tokens that must not appear in a non-comment line of the runner's sources.
///
/// `gherkin`, `markdown`, and `Trymark` name frontends. `reporting` and
/// `ScenarioRecord` name the reporting pipeline. `StepExecution` and
/// `BypassedScenario` name the existing runtime's control-flow types, which a
/// frontend-neutral runner must not adopt as its own vocabulary. The
/// `StepExecution` family is matched by [`control_flow_leak`] rather than here,
/// because it has legitimate completions; see that function.
///
/// Every token is matched with `contains`, not as a path prefix. A prefix like
/// `"reporting::"` matches only the fully qualified spelling, so it misses the
/// one import form that actually launders the dependency: `use crate::reporting
/// as rep;` binds the module under a name the scan has never heard of, and every
/// later use of it (`rep::Feature`) is then invisible. The same reasoning
/// applies to the frontend tokens, which is why none of them carries a trailing
/// `::` either — aliasing `gherkin` as `g` still writes the word `gherkin` at
/// the import, and that is the line the scan sees.
///
/// This list is a necessary condition, not a sufficient one, and widening it is
/// not free. A bare `snapshot` entry, for instance, would reject
/// `assert_snapshot!` — and INV-7 mandates an `insta` snapshot of the `Display`
/// projection in a file this scan reads. Each addition must therefore be
/// checked against the artefacts the rest of the plan requires. See the
/// module-level note on what the scan does and does not establish.
const FORBIDDEN: [&str; 6] = [
    "gherkin",
    "markdown",
    "trymark",
    "reporting",
    "ScenarioRecord",
    "BypassedScenario",
];

/// The existing runtime's control-flow type, matched with a right boundary.
///
/// Bare `StepExecution` is a strict prefix of `StepExecutionRequest` — the
/// argument type
/// [`execute_step`](crate::execution::execute_step) takes, and *the* type a
/// driver is required to build — and of `StepExecutionMode`, which a driver
/// names to ask how a step is registered. A bare `contains` would therefore
/// reject the driver's unavoidable interactions with the existing runtime,
/// which is the opposite of what this scan is for.
///
/// Carrying a delimiter instead does not work either, and the earlier revision
/// of this list tried exactly that: `"StepExecution::"` and `"StepExecution "`
/// miss `Vec<StepExecution>`, `Result<StepExecution>`, `Option<StepExecution>`,
/// and `(StepExecution,)` — every generic position, which is where a leaking
/// signature would actually put it. The leak INV-11 guards against is a
/// signature a caller must name, and a token list that only catches the
/// `StepExecution::Variant` spelling misses most of the ways it can be named.
const CONTROL_FLOW: &str = "StepExecution";

/// The completions of [`CONTROL_FLOW`] that are legitimate in this tree.
///
/// Naming one of these is how a caller reaches the registry, not a way of
/// coupling to a frontend, so they are exempt. Nothing else is: a new public
/// alias like `StepExecutionOutcome` would be flagged, which is the intended
/// outcome.
const CONTROL_FLOW_ALLOWED: [&str; 2] = ["Mode", "Request"];

/// Whether a line names the control-flow type outside those two completions.
///
/// The rule is a right boundary: each occurrence of [`CONTROL_FLOW`] is read
/// together with whatever identifier characters follow it, and the resulting
/// word is a leak unless it is one of [`CONTROL_FLOW_ALLOWED`]. That catches
/// every generic and delimiter position, and it is deliberately *not* a
/// left boundary — a `StepExecution` written as part of a longer name is still
/// a mention of it.
fn control_flow_leak(lower: &str) -> bool {
    let token = CONTROL_FLOW.to_lowercase();
    let mut rest = lower;
    // `split_once` returns the text *just past* the first match, which is the
    // same slice offset arithmetic would produce — but without indexing a
    // string, which this crate denies (`clippy::string_slice`) and which needs
    // a panic argument to justify even where the offset is provably a boundary.
    while let Some((_, after)) = rest.split_once(token.as_str()) {
        // Underscore is an identifier character but is not alphanumeric, so a
        // `char::is_alphanumeric` boundary alone would treat `StepExecution_State`
        // as a bare mention and then fail to flag it — the same class of miss the
        // delimiter rule had.
        let tail: String = after
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
        let allowed = CONTROL_FLOW_ALLOWED
            .iter()
            .any(|completion| tail.eq_ignore_ascii_case(completion));
        if !allowed {
            return true;
        }
        rest = after;
    }
    false
}

/// The substring that marks a line as a comment, a doc comment, or a string
/// literal held across a line break, and so out of scope.
///
/// A doc-comment mention is permitted deliberately: the module doc *should* be
/// able to say "nothing here knows what Gherkin is", and the phrase is only
/// meaningful in prose.
fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
}

/// Find every forbidden token in one non-comment line.
///
/// Case-insensitive, because a frontend type named `MarkdownFrontend` leaks just
/// as thoroughly as one named `markdown`.
fn leaks_in(line: &str) -> Vec<&'static str> {
    if is_comment(line) {
        return Vec::new();
    }
    let lower = line.to_lowercase();
    let mut found: Vec<&'static str> = FORBIDDEN
        .iter()
        .copied()
        .filter(|token| lower.contains(&token.to_lowercase()))
        .collect();
    if control_flow_leak(&lower) {
        found.push(CONTROL_FLOW);
    }
    found
}

/// Read the runner's sources, skipping this module's own files.
///
/// Paths are relative to the runner root rather than absolute, so a finding
/// reads `outcome/step.rs:12: gherkin` instead of a path that varies with the
/// checkout location, and so the sort in `walk` is a stable, readable ordering.
fn runner_sources() -> Scanned {
    scan_root(&Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runner"))
}

#[test]
fn no_frontend_types_in_public_api() {
    let Scanned {
        sources,
        unreadable,
    } = runner_sources();
    assert!(
        !sources.is_empty(),
        "the runner source scan found no files, so this check proves nothing",
    );
    // A file the walk could not read is exactly as invisible to the scan as a
    // clean one, so it is reported as a failure rather than skipped quietly.
    assert!(
        unreadable.is_empty(),
        "the runner source scan could not read every path it reached:\n{}",
        unreadable.join("\n"),
    );

    let mut findings = Vec::new();
    for (path, contents) in &sources {
        for (offset, line) in contents.lines().enumerate() {
            for token in leaks_in(line) {
                findings.push(format!(
                    "{path}:{}: {token} in `{}`",
                    offset + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        findings.is_empty(),
        "the runner surface must mention no frontend type:\n{}",
        findings.join("\n"),
    );
}

/// The scan actually reaches the tree it claims to police.
///
/// Without this, a `collect` that silently returned nothing (a moved directory,
/// a changed extension) would make `no_frontend_types_in_public_api` pass while
/// checking nothing at all.
///
/// The expectations are whole relative paths, not bare file names. A name-only
/// expectation cannot tell a descent into `outcome/` from an unrelated
/// `outcome.rs` sitting at the top level: an earlier draft of this guard matched
/// `"outcome"` against each file name, and the literal was satisfied by
/// `tests/outcome.rs` — a file the guard was not asking about — so it proved
/// nothing about the directory it named. Whole paths leave no such slack. A
/// `child_path` that dropped its prefix, collapsing every key to a bare file
/// name, is caught here and slips past the name-only form.
///
/// The list went stale once already, and that is worth stating because the
/// staleness was invisible. It named the files of EP-M1 and was not extended
/// when EP-M2 added `scope.rs` and the whole `engine/` subtree, so the guard
/// kept passing while never claiming the newest files — the one direction in
/// which a completeness guard fails quietly. `engine/policy_tests/` is named by
/// its `mod.rs` for the reason above: one entry that proves the *descent*, not
/// one entry per test file.
#[test]
fn the_scan_finds_the_runner_tree() {
    let Scanned {
        sources,
        unreadable,
    } = runner_sources();
    assert!(
        unreadable.is_empty(),
        "the completeness guard must read the whole tree; unreadable:\n{}",
        unreadable.join("\n"),
    );
    let paths = sources
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<Vec<_>>();

    for expected in [
        "mod.rs",
        "engine/drive_sync.rs",
        "engine/mod.rs",
        "engine/policy.rs",
        "engine/policy_tests/mod.rs",
        "outcome/failure.rs",
        "outcome/mod.rs",
        "outcome/step.rs",
        "plan.rs",
        "plan/builder.rs",
        "scope.rs",
        "source.rs",
        // The walk is a separate file and holds no forbidden token, so it is
        // scanned like any other. Only this module's own two files are exempt,
        // because they carry the token list itself.
        "tests/surface/walk.rs",
    ] {
        assert!(
            paths.contains(&expected),
            "expected the scan to reach {expected}; found {paths:?}",
        );
    }
}

/// One negative control per leak shape.
///
/// The five shapes are the ones the first draft's line-anchored matcher missed:
/// a variant field, a trait bound, an associated type, a type alias, and a
/// re-export through a private module. Each fragment below is a real snippet
/// that would compile, and each must be flagged — a matcher that looked only at
/// lines beginning with `pub` would pass every one of them.
#[test]
fn every_leak_shape_is_flagged() {
    let shapes = [
        // A field on a `pub enum` variant. The variant line does not begin with
        // `pub`, so a `pub`-anchored matcher cannot see it.
        (
            "variant field",
            "    Step {\n        step: gherkin::Step,\n    },",
        ),
        // A trait bound on a public function.
        (
            "trait bound",
            "pub fn drive<S: gherkin::StepSource>(source: S) {}",
        ),
        // An associated type on a public trait.
        ("associated type", "    type Item = gherkin::Step;"),
        // A type alias, which a matcher looking for `struct`/`enum`/`fn` would
        // never visit.
        ("type alias", "pub type Steps = Vec<gherkin::Step>;"),
        // A re-export of a frontend type through a private module.
        (
            "re-export",
            "pub use crate::runner::reporting::ScenarioRecord;",
        ),
    ];

    for (shape, fragment) in shapes {
        assert!(
            fragment.lines().any(|line| !leaks_in(line).is_empty()),
            "the matcher must flag a leak in the form of a {shape}:\n{fragment}",
        );
    }

    // The bare-import shape, which a path-prefixed token list misses. These
    // carry no `::` at all, so a `"reporting::"` entry would let both through.
    for alias in ["use crate::reporting as rep;", "use crate::reporting;"] {
        assert!(
            !leaks_in(alias).is_empty(),
            "a bare import of a forbidden module must be flagged:\n{alias}",
        );
    }

    // The non-vacuity counterpart: prose about a frontend is not a leak, and a
    // matcher that flagged every mention would be useless in a crate whose
    // module docs exist to say what the runner is *not* coupled to.
    assert!(
        leaks_in("//! Nothing here knows what Gherkin is.").is_empty(),
        "a doc-comment mention must be permitted",
    );
}

/// The control-flow token is flagged by boundary, not by delimiter.
///
/// This is the guard for a hole the earlier revision had and did not see:
/// `"StepExecution::"` and `"StepExecution "` were the whole rule, and the
/// driver's most likely leaking signature — `fn drive(&mut self, step:
/// StepExecution)` or a `Vec<StepExecution>` field — matches neither. Both
/// directions are asserted, because a boundary rule that flagged the two
/// legitimate completions would break the driver, and one that exempted
/// everything ending in an alphanumeric would flag nothing at all.
#[test]
fn the_control_flow_token_is_flagged_by_boundary() {
    // Every position a leaking mention can occupy. The last three are the ones
    // the delimiter rule missed, and they are the ones a real signature uses.
    for leaking in [
        "let outcome = StepExecution::from(step);",
        "fn drive(&mut self, step: StepExecution) -> Outcome {",
        "struct Driver { steps: Vec<StepExecution> }",
        "type Planned = Option<StepExecution>;",
        "fn take(pair: (StepExecution, usize)) {}",
        "impl StepExecution { fn run(&self) {} }",
        "use crate::execution::StepExecution;",
        // Underscore is an identifier character but not alphanumeric, so a
        // boundary that only rejected alphanumerics would call this a bare
        // mention and then decline to flag it.
        "struct StepExecution_State;",
    ] {
        assert!(
            !leaks_in(leaking).is_empty(),
            "a mention of the control-flow type must be flagged:\n{leaking}",
        );
    }

    // The two completions a driver is *required* to name to reach the registry.
    // Flagging these would make the scan reject the driver it polices.
    for allowed in [
        "let mode = StepExecutionMode::Sync;",
        "fn build() -> StepExecutionRequest { Default::default() }",
        "StepExecutionRequest::new(mode, text)",
        "match step.mode() { StepExecutionMode::Async => {}, _ => {} }",
    ] {
        assert!(
            leaks_in(allowed).is_empty(),
            "a legitimate completion must not be flagged:\n{allowed}",
        );
    }
}
