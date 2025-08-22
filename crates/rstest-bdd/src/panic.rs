//! Panic payload formatting helpers.
//!
//! This module provides utilities for extracting human-readable messages from
//! panic payloads. It prefers string payloads when available and falls back to
//! `Debug` formatting for all other types.

use std::any::Any;

/// Formats a panic payload into a readable message.
///
/// String payloads are extracted directly, while all other types are rendered
/// using their [`Debug`](core::fmt::Debug) implementation.
///
/// # Examples
///
/// ```
/// use rstest_bdd::panic_message;
/// use std::any::Any;
///
/// let payload: Box<dyn Any + Send> = Box::new("boom");
/// assert_eq!(panic_message(&payload), "boom");
/// ```
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .or_else(|| payload.downcast_ref::<u8>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<u16>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<u32>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<u64>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<usize>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<i8>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<i16>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<i32>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<i64>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<isize>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<f32>().map(ToString::to_string))
        .or_else(|| payload.downcast_ref::<f64>().map(ToString::to_string))
        .unwrap_or_else(|| format!("{payload:?}"))
}
