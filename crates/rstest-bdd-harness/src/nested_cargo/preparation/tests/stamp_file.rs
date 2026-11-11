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
fn a_stamp_admits_a_fingerprint_that_spells_the_separator() {
    // `inputs` is caller-supplied text and `render` writes the fingerprint
    // verbatim ahead of the separator line, so the separator's own spelling is
    // reachable from a value. Splitting on its first occurrence then finds the
    // one inside the fingerprint rather than the delimiter, and the stamp is
    // refused in the same breath as it was written: the Nextest gate reports a
    // missing or stale stamp, and `cargo test` prepares the family again.
    let target = std::env::temp_dir().join("target");
    let environment = environment_in(&target);
    let fingerprint = compose(
        &preparation(&target, &environment, "v", "@@ recorded-artefacts"),
        "release: 1.88.0 host: x",
    );
    let stamp = render(&fingerprint, &[]);
    assert!(
        parse(stamp.as_str(), &fingerprint).is_some(),
        "a stamp must admit the fingerprint it was written for, whatever the inputs spell: {:?}",
        stamp.as_str()
    );
}

#[test]
fn a_stamp_reads_back_whether_or_not_the_fingerprint_ends_its_own_break() {
    // `compose` ends every field with a newline, so the terminating-break form
    // is the ordinary one and `render` relies on it. The other form is
    // defensive: `strip_line_break` falls back to the text unchanged when
    // there is no break to remove, and `render` supplies the separating newline
    // either way, so a fingerprint that does not end its own line must still
    // read back. Both sides of this pair must hold, because requiring the break
    // unconditionally and making it optional are each wrong in one direction.
    let target = std::env::temp_dir().join("target");
    let environment = environment_in(&target);
    let preparation = preparation(&target, &environment, "v", "hash");
    let toolchain = "release: 1.88.0 host: x";
    let terminated = compose(&preparation, toolchain);
    assert!(
        terminated.as_str().ends_with('\n'),
        "compose is expected to end the fingerprint with a break: {:?}",
        terminated.as_str()
    );

    let written = render(&terminated, &[]);
    assert!(
        parse(written.as_str(), &terminated).is_some(),
        "a fingerprint ending its own break must read back: {:?}",
        written.as_str()
    );

    // The same fingerprint with the break removed, rendered by hand, because
    // `render` is the only writer and it always emits a break-terminated text.
    let bare = terminated.as_str().trim_end_matches('\n');
    let handwritten = format!("{bare}\n@@ recorded-artefacts\n");
    assert!(
        parse(&handwritten, &terminated).is_some(),
        "a fingerprint with no terminating break must still read back: {handwritten:?}"
    );
}

#[test]
fn a_stamp_for_inputs_ending_in_the_separator_does_not_admit_the_prefix() {
    // The case the round trip above cannot reach, because there both sides
    // carry the same `inputs`. Here the *value* differs: `h@@ recorded-
    // artefacts` against `h`. `render` writes the longer value, then a newline,
    // then the separator line, so if the parser treats that newline as
    // optional it consumes `h`, then the `@@ recorded-artefacts` still inside
    // the value, and reports a match against a fingerprint for plain `h`. The
    // recorded artefacts of the longer build are usually still on disk, so the
    // gate accepts a preparation whose inputs genuinely changed.
    let target = std::env::temp_dir().join("target");
    let environment = environment_in(&target);
    let longer = compose(
        &preparation(&target, &environment, "v", "h@@ recorded-artefacts"),
        "release: 1.88.0 host: x",
    );
    let prefix = compose(
        &preparation(&target, &environment, "v", "h"),
        "release: 1.88.0 host: x",
    );
    assert_ne!(longer, prefix, "the two inputs must compose differently");

    let stamp = render(&longer, &[]);
    assert!(
        parse(stamp.as_str(), &prefix).is_none(),
        "a stamp written for `h@@ recorded-artefacts` must not admit `h`: {:?}",
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
