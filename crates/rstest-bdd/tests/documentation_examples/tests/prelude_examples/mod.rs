//! The users' guide's prelude examples, held where the compiler can see them.
//!
//! `glob` and `spelled` each carry one fenced example from
//! `docs/users-guide.md`, character for character. Keeping them beside the test
//! that compares them with the guide is what turns the guide's import lines
//! into compiled code: a Markdown fence cannot be handed to `include!`, so the
//! text is copied once here and pinned to the guide by a test.

/// The fenced example marked `prelude-glob-import`.
mod glob;
/// The fenced example marked `prelude-spelled-import`.
mod spelled;
