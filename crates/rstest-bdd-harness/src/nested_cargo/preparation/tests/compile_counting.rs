//! Counting the units a nested `cargo` reported compiling.
//!
//! The preparation log's unit count is the evidence that cold work left the
//! timed assertions, and CI's colour setting means the status lines arrive
//! behind escapes, so both plain and coloured output are pinned here.

use crate::nested_cargo::count_compiling;

#[test]
fn compiling_lines_are_counted_from_stderr() {
    let stderr = b"   Compiling foo v0.1.0\nwarning: unused\n    Finished `dev` profile\n   \
                   Compiling bar v0.2.0\n";
    assert_eq!(count_compiling(stderr), 2);
}

#[test]
fn ansi_coloured_compiling_lines_are_counted() {
    // CI sets `CARGO_TERM_COLOR=always`, so a nested `cargo` that is not given
    // `--color=never` emits each status line behind a colour escape. Counting
    // that output as zero would make a cold preparation look like a reuse and
    // let the prepared-fixture gate pass on the wrong evidence.
    let green = "\u{1b}[1m\u{1b}[32m";
    let reset = "\u{1b}[0m\u{1b}[39m";
    let stderr = format!(
        "   {green}Compiling{reset} foo v0.1.0\nwarning: unused\n   {green}Compiling{reset} bar \
         v0.2.0\n"
    );
    assert_eq!(count_compiling(stderr.as_bytes()), 2);
}
