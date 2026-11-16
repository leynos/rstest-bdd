//! The users' guide's globbed prelude example, compiled as written.
//!
//! The import below is character-identical to the fenced example marked
//! `prelude-glob-import` in `docs/users-guide.md`. It is a module of its own so
//! the compiler checks the guide's text rather than a paraphrase of it, and so
//! a name the guide advertises but the crate cannot resolve fails a gate
//! instead of a reader's first build.
#![expect(unused_imports, reason = "resolution is the property under test")]

use rstest_bdd::prelude::*;
