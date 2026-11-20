//! Counting the units a nested `cargo` reported compiling.
//!
//! The preparation log's unit count is the evidence that cold work left the
//! timed assertions, and CI's colour setting means the status lines arrive
//! behind escapes, so both plain and coloured output are pinned here — as two
//! cases of one transcript rather than as two tests, so the expected count and
//! the ignored line are stated once and the only thing that varies is whether
//! the escapes are present.

use rstest::rstest;

use crate::nested_cargo::count_compiling;

/// The escapes CI's `CARGO_TERM_COLOR=always` puts around a status line, and
/// the plain pair used for an uncoloured run.
const PLAIN: (&str, &str) = ("", "");
const ANSI: (&str, &str) = ("\u{1b}[1m\u{1b}[32m", "\u{1b}[0m\u{1b}[39m");

#[rstest]
#[case::plain(PLAIN)]
#[case::ansi(ANSI)]
fn compiling_lines_are_counted_whatever_the_colour_setting(#[case] (start, reset): (&str, &str)) {
    // A nested `cargo` that is not given `--color=never` emits each status
    // line behind a colour escape. Counting that output as zero would make a
    // cold preparation look like a reuse and let the prepared-fixture gate
    // pass on the wrong evidence.
    let stderr = format!(
        concat!(
            "   {start}Compiling{reset} foo v0.1.0\n",
            "warning: unused\n",
            "    Finished `dev` profile\n",
            "   {start}Compiling{reset} bar v0.2.0\n",
        ),
        start = start,
        reset = reset,
    );
    assert_eq!(
        count_compiling(stderr.as_bytes()),
        2,
        "two `Compiling` lines must be counted and the ignored `Finished` line must not be: \
         {stderr:?}"
    );
}
