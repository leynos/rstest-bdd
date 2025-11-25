//! Test helpers for asserting pattern parser outcomes.
use super::placeholder::{parse_placeholder, PlaceholderSpec};
use crate::errors::PatternError;

pub(crate) fn parse_ok(pattern: &str) -> (usize, PlaceholderSpec) {
    match parse_placeholder(pattern.as_bytes(), 0) {
        Ok(result) => result,
        Err(err) => panic!("placeholder should parse: {err}"),
    }
}

pub(crate) fn parse_err(pattern: &str) -> PatternError {
    match parse_placeholder(pattern.as_bytes(), 0) {
        Ok(_) => panic!("placeholder parsing should fail"),
        Err(err) => err,
    }
}
