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

use std::{fs, path::Path};

/// Tokens that must not appear in a non-comment line of the runner's sources.
///
/// `gherkin`, `markdown`, and `Trymark` name frontends. `reporting::` and
/// `ScenarioRecord` name the reporting pipeline. `StepExecution` and
/// `BypassedScenario` name the existing runtime's control-flow types, which a
/// frontend-neutral runner must not adopt as its own vocabulary.
const FORBIDDEN: [&str; 7] = [
    "gherkin",
    "markdown",
    "trymark",
    "reporting::",
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
fn runner_sources() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runner");
    let mut sources = Vec::new();
    collect(&root, &mut sources);
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

/// Recursively collect `(relative path, contents)` for every `.rs` file under
/// `directory`, except this file.
fn collect(directory: &Path, sources: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            // Reading this check's own source would find the literal tokens in
            // `FORBIDDEN` above and report them as leaks.
            if path.file_name().is_some_and(|name| name == "surface.rs") {
                continue;
            }
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            sources.push((path.display().to_string(), contents));
        }
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
#[test]
fn the_scan_finds_the_runner_tree() {
    let sources = runner_sources();
    let names = sources
        .iter()
        .map(|(path, _)| path.rsplit('/').next().unwrap_or_default())
        .collect::<Vec<_>>();

    for expected in ["mod.rs", "plan.rs", "source.rs", "outcome", "step.rs"] {
        assert!(
            names.iter().any(|name| name.contains(expected)),
            "expected the scan to reach {expected}; found {names:?}",
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

    // The non-vacuity counterpart: prose about a frontend is not a leak, and a
    // matcher that flagged every mention would be useless in a crate whose
    // module docs exist to say what the runner is *not* coupled to.
    assert!(
        leaks_in("//! Nothing here knows what Gherkin is.").is_empty(),
        "a doc-comment mention must be permitted",
    );
}
