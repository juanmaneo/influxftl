// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Finite numbers at the edges.
//!
//! Inside the WarpScript evaluator `NaN` and the infinities are ordinary `DOUBLE` values, as in
//! Warp 10 (the `NaN` literal, the `NaN NaN` of a datapoint without location, `1.0 0.0 /`).
//! A number that enters from outside the evaluator (HTTP, a command line, a datastore) or that
//! leaves it for a format which cannot represent those values is checked with
//! [`ensure_finite`]: a non-finite number is a typed failure there, never a value to store,
//! encode or display.

/// A number which is not finite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NotFinite {
    /// Not a number.
    #[error("not a finite number: NaN")]
    Nan,
    /// Positive infinity.
    #[error("not a finite number: +Infinity")]
    PositiveInfinity,
    /// Negative infinity.
    #[error("not a finite number: -Infinity")]
    NegativeInfinity,
}

/// `value` when it is finite.
///
/// # Errors
///
/// [`NotFinite`] for `NaN` and the infinities.
pub fn ensure_finite(value: f64) -> Result<f64, NotFinite> {
    if value.is_nan() {
        Err(NotFinite::Nan)
    } else if value == f64::INFINITY {
        Err(NotFinite::PositiveInfinity)
    } else if value == f64::NEG_INFINITY {
        Err(NotFinite::NegativeInfinity)
    } else {
        Ok(value)
    }
}
