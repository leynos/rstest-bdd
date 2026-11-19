//! What the gate and `invalidate` say while they work.
//!
//! The diagnostics are part of this module's contract, not a courtesy. An
//! unreadable stamp under `cargo test` is *prepared over*, so the message is
//! the only thing that distinguishes that recovery from a silent rebuild — and
//! a message written straight to stderr cannot be seen by a test. The cases
//! here drive [`ensure_prepared_reported`], [`prepare_now_reported`] and
//! [`invalidate_reported`], the boundaries production's `note` passes through,
//! so both halves of the promise are checked: that the message names what was
//! wrong, and that the family is prepared again afterwards.

use std::fs;

use super::{super::stamp::stamp_path, scratch_preparation};
use crate::nested_cargo::{
    Preparation,
    PreparationSummary,
    ensure_prepared_reported,
    invalidate_reported,
    prepare_now_reported,
};

/// Write a stamp that exists but cannot be read as text.
///
/// Invalid UTF-8 is a portable "present but unreadable": a mode of zero would
/// be readable by a suite running as root.
fn write_unreadable_stamp(preparation: &Preparation) -> std::io::Result<()> {
    fs::write(stamp_path(preparation), [0xff, 0xfe])
}

/// The messages a case's reporter collected, in order.
fn collecting(messages: &mut Vec<String>) -> impl FnMut(&str) + '_ {
    move |message: &str| messages.push(message.to_owned())
}

#[test]
fn the_cargo_test_recovery_names_the_unreadable_stamp_and_its_second_read() {
    // Both messages matter. The first explains why the gate did not reuse a
    // tree that looks present, and the second explains why the pre-lock check
    // is repeated: without it, a reader seeing two reads of the same file has
    // no way to tell a deliberate re-check from a bug.
    let (_directory, preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    write_unreadable_stamp(&preparation).expect("unreadable stamp");
    let mut messages = Vec::new();

    ensure_prepared_reported(
        &preparation,
        false,
        || Ok("rustc 1.0\n".to_owned()),
        collecting(&mut messages),
        || Ok(PreparationSummary { compiled_units: 1 }),
    )
    .expect("cargo test must recover from an unreadable stamp by preparing");

    let [before, after, completed] = messages.as_slice() else {
        panic!(
            "the gate reports the unreadable stamp twice and the completed build once: \
             {messages:?}"
        );
    };
    assert!(
        before.contains("unreadable"),
        "the first message must say the stamp could not be read: {messages:?}"
    );
    assert!(
        !before.contains("after waiting for the lock"),
        "the pre-lock message must not claim to have taken the lock: {messages:?}"
    );
    assert!(
        after.contains("after waiting for the lock"),
        "the second read must say where it happened: {messages:?}"
    );
    assert!(
        completed.contains("prepared in"),
        "the completed build must be reported with its elapsed time: {messages:?}"
    );
    assert!(
        completed.contains("1 Compiling lines"),
        "the report must carry the compile count, which is the evidence cold work moved: \
         {messages:?}"
    );
}

#[test]
fn an_unreadable_stamp_before_the_expected_compile_is_not_reported_as_a_match() {
    // The negative control for the messages above: nothing is reported when the
    // stamp is simply absent, because an absent stamp costs a build and needs
    // no explanation. If this case ever reported an "unreadable" message, the
    // cases above would pass for the wrong reason — a reporter that always
    // fires would satisfy them.
    let (_directory, preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    let mut messages = Vec::new();

    ensure_prepared_reported(
        &preparation,
        false,
        || Ok("rustc 1.0\n".to_owned()),
        collecting(&mut messages),
        || Ok(PreparationSummary::default()),
    )
    .expect("a family with no stamp is prepared without a diagnostic");

    assert_eq!(
        messages.len(),
        1,
        "only the completed build is news on a cold tree: {messages:?}"
    );
    assert!(
        !messages
            .iter()
            .any(|message| message.contains("unreadable")),
        "a missing stamp is not an unreadable one: {messages:?}"
    );
}

#[test]
fn a_warm_family_is_reported_as_prepared_without_running_the_build() {
    // The other side of the same boundary. A gate that admitted a stamp must
    // say so, because the reader's question is whether the timed command reused
    // the tree; a silent success is indistinguishable from a gate that never
    // ran.
    let (_directory, preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    let first = prepare_now_reported(
        &preparation,
        || Ok("rustc 1.0\n".to_owned()),
        |_| {},
        || Ok(PreparationSummary::default()),
    );
    first.expect("the scratch family prepares");

    let mut messages = Vec::new();
    ensure_prepared_reported(
        &preparation,
        false,
        || Ok("rustc 1.0\n".to_owned()),
        collecting(&mut messages),
        || panic!("a warm family must not be prepared again"),
    )
    .expect("the stamp written above must be honoured");

    let [prepared] = messages.as_slice() else {
        panic!("one message: the family is prepared, got {messages:?}");
    };
    assert_eq!(
        prepared,
        &format!(
            "family={} is prepared ({})",
            preparation.family,
            stamp_path(&preparation).display()
        ),
        "the message must name the family and the stamp that admitted it"
    );
}

#[test]
fn invalidating_reports_the_stamp_it_removed_and_says_nothing_about_an_absent_one() {
    // `invalidate` withdraws a claim, and the withdrawal is what a reader needs
    // to see: the next run will build for a reason, and without this line that
    // reason is invisible. Removing an already-absent stamp is deliberately
    // quiet — the caller is refuting a claim, not reporting a file.
    let (_directory, preparation, _environment) =
        scratch_preparation().expect("the scratch preparation fixture");
    prepare_now_reported(
        &preparation,
        || Ok("rustc 1.0\n".to_owned()),
        |_| {},
        || Ok(PreparationSummary::default()),
    )
    .expect("the scratch family prepares");

    let mut messages = Vec::new();
    invalidate_reported(&preparation, collecting(&mut messages)).expect("the stamp is removed");
    let [removed] = messages.as_slice() else {
        panic!("removing a stamp is one message, got {messages:?}");
    };
    assert!(
        removed.contains("removed the stamp"),
        "the message must say the stamp was removed: {messages:?}"
    );
    assert!(
        removed.contains(&stamp_path(&preparation).display().to_string()),
        "the message must name which stamp went: {messages:?}"
    );

    let mut quiet = Vec::new();
    invalidate_reported(&preparation, collecting(&mut quiet))
        .expect("removing an absent stamp is not an error");
    assert!(
        quiet.is_empty(),
        "an already-absent stamp is not news: {quiet:?}"
    );
}
