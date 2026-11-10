//! Unit tests for the stamp's two-section encoding.

use std::path::PathBuf;

use super::{Fingerprint, Stamp, matching_stamp, parse, render, write_stamp};

/// The fingerprint text the round-trip cases compare against.
fn fingerprint_of(text: &str) -> Fingerprint { Fingerprint(text.to_owned()) }

/// A render/parse round trip preserves the fingerprint and every section.
#[test]
fn a_rendered_stamp_parses_back_to_what_was_written() {
    let fingerprint = fingerprint_of("version: 2\ntoolchain: release: 1.88.0\n");
    let recorded = vec![
        vec!["liba.rlib".to_owned(), "a.d".to_owned()],
        Vec::new(),
        vec!["probe".to_owned()],
    ];

    let text = render(&fingerprint, &recorded);
    let stamp = parse(text.as_str(), &fingerprint).expect("a stamp just rendered must parse");

    assert_eq!(
        stamp.recorded, recorded,
        "each section must keep its own names, including an empty one: the section index is how a \
         name is matched to the directory it belongs to"
    );
}

/// A different fingerprint does not match, even with the same names.
#[test]
fn a_changed_fingerprint_is_not_a_match() {
    let text = render(
        &fingerprint_of("inputs: old\n"),
        &[vec!["liba.rlib".to_owned()]],
    );
    assert!(
        parse(text.as_str(), &fingerprint_of("inputs: new\n")).is_none(),
        "a stamp must only describe the inputs it was written from"
    );
}

/// A version-1 stamp, which has no separator, is not a match.
#[test]
fn a_stamp_without_recorded_names_is_not_a_match() {
    let text = "version: 1\ntoolchain: release: 1.88.0\n";
    assert!(
        parse(text, &fingerprint_of(text)).is_none(),
        "a stamp that records no artefacts cannot answer whether they survive"
    );
}

/// The on-disk round trip, including the `None` for a missing file.
#[test]
fn a_written_stamp_is_found_and_a_missing_one_is_not() {
    let directory = tempfile::tempdir().expect("temp dir");
    let stamp = directory.path().join(".unit.rstest-bdd-prepared");
    assert!(
        matching_stamp(&stamp, &fingerprint_of("version: 2\n"))
            .expect("an absent stamp is not a read failure")
            .is_none(),
        "an absent stamp is not a match"
    );

    let fingerprint = fingerprint_of("version: 2\ntoolchain: release: 1.88.0\n");
    write_stamp(
        &stamp,
        &render(&fingerprint, &[vec!["liba.rlib".to_owned()]]),
    )
    .expect("write the stamp");

    let found: Stamp = matching_stamp(&stamp, &fingerprint)
        .expect("the stamp is readable")
        .expect("the stamp must match");
    assert_eq!(found.recorded, vec![vec!["liba.rlib".to_owned()]]);
    assert!(
        matching_stamp(
            &stamp,
            &fingerprint_of("version: 2\ntoolchain: release: 1.89.0\n")
        )
        .expect("the stamp is readable")
        .is_none(),
        "a stamp from another toolchain must not match"
    );
    assert!(
        !PathBuf::from(".unit.rstest-bdd-prepared").exists(),
        "the stamp must live where it was asked to"
    );
}

/// An unreadable stamp reports its cause rather than passing as absent.
#[cfg(unix)]
#[test]
fn an_unreadable_stamp_is_reported_rather_than_treated_as_absent() {
    // Folding a read failure into `None` makes a stamp that may well describe a
    // warm tree indistinguishable from no stamp at all, so the rebuild it
    // causes has no visible reason. A symlink loop reaches that state without
    // any permission setup, which would not hold when the suite runs as root.
    let directory = tempfile::tempdir().expect("temp dir");
    let stamp = directory.path().join(".unit.rstest-bdd-prepared");
    std::os::unix::fs::symlink(&stamp, &stamp).expect("symlink loop");

    let error = match matching_stamp(&stamp, &fingerprint_of("version: 2\n")) {
        Err(error) => error,
        Ok(matching) => panic!(
            "a loop must be reported as a read failure, not as {}",
            if matching.is_some() {
                "a match"
            } else {
                "an absence"
            }
        ),
    };
    assert_ne!(
        error.kind(),
        std::io::ErrorKind::NotFound,
        "a loop is not an absence: {error}"
    );
}
