// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the mappers, reducers and bucketizers, worded like Warp 10's.

/// A failure of an aggregator.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AggregatorError {
    /// A builder given a parameter it cannot use.
    #[error("Invalid parameter for {0}")]
    InvalidParameter(String),
    /// `mapper.replace` given a constant of a type it cannot hold.
    #[error("Invalid value type.")]
    InvalidValueType,
    /// A language tag builder given something else than a string.
    #[error(
        "Invalid parameter type for {0}, expects a language tag (a string) on top of the stack."
    )]
    LanguageTagExpected(String),
    /// A concatenation of strings with numbers or booleans.
    #[error("Cannot {verb} strings and {kind}.")]
    StringMix {
        /// The operation.
        verb: &'static str,
        /// What the strings were mixed with.
        kind: &'static str,
    },
    /// A median or percentile of values that are not numbers.
    #[error("{0} can only operate on numeric Geo Time Series.")]
    NotNumeric(String),
    /// A median of an even number of values that are not numbers.
    #[error("Unable to compute median on an even number of non numeric values.")]
    EvenNonNumericMedian,
    /// A percentile outside of 0 to 100.
    #[error("Invalid percentile for {0}, MUST be between 0 and 100.")]
    InvalidPercentile(String),
    /// A lookup of the extremum of a window that retained nothing.
    #[error("Index -1 out of bounds for length {0}")]
    NothingRetained(usize),
    /// A resulting function applied to something else than a series.
    #[error("{0}'s resulting function can only be applied on a GTS.")]
    NotASeries(String),
    /// `op.mask` applied to series that are not a boolean mask and a series.
    #[error(
        "op.mask can only be applied to pairs of Geo Time Series, the first one being boolean."
    )]
    MaskOperands,
}
