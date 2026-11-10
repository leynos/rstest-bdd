//! The stamp file's own round trip: write a fingerprint, read it back.
//!
//! `inputs` covers what makes a *fingerprint* differ — the environment, the
//! toolchain, the variant, the input hash. This module covers the other half
//! of the same question, which the fingerprints there cannot reach: a gate
//! never compares two freshly composed texts, it compares a composed text
//! against one read back out of a file. `render` writes that file and `parse`
//! reads it, and a parser that normalized the text on the way back would admit
//! a build whose inputs genuinely differ from the ones the stamp describes.

use super::{environment_in, preparation};
use crate::nested_cargo::preparation::stamp::{compose, parse, render};

#[test]
fn a_stamp_reads_back_as_the_fingerprint_it_recorded() {
    // The baseline every other case here depends on. If `render` and `parse`
    // disagreed about the separator or the trailing line break, the gate would
    // refuse a stamp it had just written and every preparation would be
    // repeated on every run.
    let target = std::env::temp_dir().join("target");
    let environment = environment_in(&target);
    let fingerprint = compose(
        &preparation(&target, &environment, "v", "hash"),
        "release: 1.88.0 host: x",
    );
    let stamp = render(&fingerprint, &[]);
    assert!(
        parse(stamp.as_str(), &fingerprint).is_some(),
        "a stamp must admit the fingerprint it was written for: {:?}",
        stamp.as_str()
    );
}

#[test]
fn a_trailing_space_in_the_inputs_does_not_reuse_another_build_s_stamp() {
    // The last field of a fingerprint is `inputs`, and both `render` and
    // `parse` once normalized with `trim_end`. That made the two texts below
    // compare equal in one direction: a stamp written for the hash without the
    // trailing space admitted the build whose hash has one, so a changed
    // fixture kept its prepared artefacts. The values differ, so the gate must
    // refuse.
    let target = std::env::temp_dir().join("target");
    let environment = environment_in(&target);
    let plain = compose(
        &preparation(&target, &environment, "v", "hash"),
        "release: 1.88.0 host: x",
    );
    let spaced = compose(
        &preparation(&target, &environment, "v", "hash "),
        "release: 1.88.0 host: x",
    );
    assert_ne!(
        plain, spaced,
        "a trailing space in the input hash must change the fingerprint"
    );

    let stamp = render(&plain, &[]);
    assert!(
        parse(stamp.as_str(), &spaced).is_none(),
        "a stamp written for `hash` must not admit `hash `: {:?}",
        stamp.as_str()
    );
}

#[test]
fn a_stamp_refuses_a_different_variant_or_toolchain() {
    // The other two fields that reach the composition as their own lines. A
    // parser keyed only on the environment section would pass the cases above
    // and still admit a stamp across a toolchain change.
    let target = std::env::temp_dir().join("target");
    let environment = environment_in(&target);
    let stamp = render(
        &compose(
            &preparation(&target, &environment, "all-features", "hash"),
            "release: 1.88.0 host: x",
        ),
        &[],
    );
    assert!(
        parse(
            stamp.as_str(),
            &compose(
                &preparation(&target, &environment, "no-default-features", "hash"),
                "release: 1.88.0 host: x",
            ),
        )
        .is_none(),
        "a stamp must not survive a variant change"
    );
    assert!(
        parse(
            stamp.as_str(),
            &compose(
                &preparation(&target, &environment, "all-features", "hash"),
                "release: 1.92.0 host: x",
            ),
        )
        .is_none(),
        "a stamp must not survive a toolchain change"
    );
}
