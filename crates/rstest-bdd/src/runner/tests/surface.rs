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

use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};

/// Tokens that must not appear in a non-comment line of the runner's sources.
///
/// `gherkin`, `markdown`, and `Trymark` name frontends. `reporting` and
/// `ScenarioRecord` name the reporting pipeline. `StepExecution` and
/// `BypassedScenario` name the existing runtime's control-flow types, which a
/// frontend-neutral runner must not adopt as its own vocabulary.
///
/// Every token is bare rather than a path prefix. A prefix like `"reporting::"`
/// matches only the fully qualified spelling, so it misses the one import form
/// that actually launders the dependency: `use crate::reporting as rep;` binds
/// the module under a name the scan has never heard of, and every later use of
/// it (`rep::Feature`) is then invisible. The same reasoning applies to the
/// frontend tokens, which is why none of them carries a trailing `::` either —
/// aliasing `gherkin` as `g` still writes the word `gherkin` at the import, and
/// that is the line the scan sees.
///
/// This list is a necessary condition, not a sufficient one. See the
/// module-level note on what the scan does and does not establish.
const FORBIDDEN: [&str; 7] = [
    "gherkin",
    "markdown",
    "trymark",
    "reporting",
    "ScenarioRecord",
    "StepExecution",
    "BypassedScenario",
];

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
    FORBIDDEN
        .iter()
        .copied()
        .filter(|token| lower.contains(&token.to_lowercase()))
        .collect()
}

/// Read the runner's sources, skipping this check's own file.
///
/// Paths are relative to the runner root rather than absolute, so a finding
/// reads `outcome/step.rs:12: gherkin` instead of a path that varies with the
/// checkout location, and so the sort below is a stable, readable ordering.
fn runner_sources() -> Vec<(String, String)> {
    let root = Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runner");
    let Ok(directory) = Dir::open_ambient_dir(&root, ambient_authority()) else {
        return Vec::new();
    };
    let mut sources = Vec::new();
    collect(&directory, "", &mut sources);
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

/// Recursively collect `(relative path, contents)` for every `.rs` file under
/// `directory`, except this file.
///
/// `prefix` is `directory`'s own path relative to the runner root, and is empty
/// at the root. Symlinked directories are not followed: `cap-std` opens
/// directories without traversing symlinks, so the scan cannot be redirected
/// outside the tree it was pointed at.
fn collect(directory: &Dir, prefix: &str, sources: &mut Vec<(String, String)>) {
    let Ok(entries) = directory.entries() else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(name) = entry.file_name() else {
            continue;
        };
        let relative = child_path(prefix, &name);
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir()
            && let Ok(child) = directory.open_dir(&name)
        {
            collect(&child, &relative, sources);
        } else if is_rust_source(&name) && name != "surface.rs" {
            // Reading this check's own source would find the literal tokens in
            // `FORBIDDEN` above and report them as leaks.
            if let Ok(contents) = directory.read_to_string(&name) {
                sources.push((relative, contents));
            }
        }
    }
}

/// Whether a directory entry is a Rust source file.
///
/// The comparison is case-insensitive, so a `.RS` file counts as source rather
/// than slipping past the scan. `Utf8Path::extension` only splits the name; it
/// does not fold case, so a bare `ext == "rs"` would let an upper-case spelling
/// of the same file through. A leak in such a file would then be reported as no
/// leak at all — the failure mode this whole module exists to catch, since the
/// scan's silence is indistinguishable from a clean tree.
fn is_rust_source(name: &str) -> bool {
    Utf8Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
}

/// Join a relative directory prefix to one of its children's names.
///
/// The root has an empty prefix, so its children are named bare; deeper entries
/// keep the whole path, which is what makes a nested finding identifiable.
fn child_path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

#[test]
fn no_frontend_types_in_public_api() {
    let sources = runner_sources();
    assert!(
        !sources.is_empty(),
        "the runner source scan found no files, so this check proves nothing",
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
#[test]
fn the_scan_finds_the_runner_tree() {
    let sources = runner_sources();
    let paths = sources
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<Vec<_>>();

    for expected in [
        "mod.rs",
        "outcome/failure.rs",
        "outcome/mod.rs",
        "outcome/step.rs",
        "plan.rs",
        "plan/builder.rs",
        "source.rs",
    ] {
        assert!(
            paths.contains(&expected),
            "expected the scan to reach {expected}; found {paths:?}",
        );
    }
}

/// An upper-case extension still counts as a Rust source file.
///
/// `Utf8Path::extension` splits the name but does not fold case, so an
/// `ext == "rs"` test lets `thing.RS` past the scan entirely: the file is never
/// read, its leaks are never reported, and the scan's silence is
/// indistinguishable from a clean tree. This is the guard the earlier
/// case-sensitive comparison lacked.
#[test]
fn an_upper_case_extension_is_still_source() {
    assert!(is_rust_source("mod.rs"), "a plain .rs file must be source");
    assert!(
        is_rust_source("odd.RS"),
        "an upper-case .RS file must not slip past the scan",
    );
    assert!(
        is_rust_source("odd.Rs"),
        "a mixed-case .Rs file must not slip past the scan",
    );

    // The counterpart: folding case must not widen the scan to everything.
    assert!(!is_rust_source("notes.md"), "a Markdown file is not source");
    assert!(
        !is_rust_source("README"),
        "a file with no extension is not source",
    );
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
    for alias in [
        "use crate::reporting as rep;",
        "use crate::reporting;",
    ] {
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
