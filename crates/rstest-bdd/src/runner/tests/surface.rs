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
