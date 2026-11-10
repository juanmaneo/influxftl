// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the datastore functions (`FETCH`, `FIND`, `UPDATE`, `META`, `DELETE`): the
//! datastore itself, and the time range a fetch is asked for (`FETCH.computeTimeRange`, worded
//! like Warp 10's because a script reads it).

/// A failure of the datastore a script is connected to.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DatastoreError {
    /// A function that needs a datastore, on a stack that has none.
    #[error("{function} needs a datastore, and none is connected to this runtime.")]
    NotConnected {
        /// The function.
        function: String,
    },
    /// A request the datastore refused or could not serve.
    #[error("{operation} failed on the datastore: {message}")]
    Backend {
        /// What the datastore was asked.
        operation: &'static str,
        /// What it answered.
        message: String,
    },
    /// A parameter that this runtime does not support.
    #[error("{function} does not support the '{parameter}' parameter.")]
    Unsupported {
        /// The function.
        function: String,
        /// The parameter.
        parameter: String,
    },
    /// A series written without a class name.
    #[error("Cannot write a Geo Time Series without a class name.")]
    Unnamed,
}

/// A time range that cannot be computed from the parameters of a fetch.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimeRangeError {
    /// Neither a start nor an end.
    #[error("Missing either '{start}' or '{end}' parameter.")]
    MissingStartOrEnd {
        /// The name of the start parameter.
        start: String,
        /// The name of the end parameter.
        end: String,
    },
    /// A start, an end and a timespan.
    #[error(
        "Invalid time range specification: '{start}', '{end}' and '{timespan}' cannot all be defined. Only 2 out of those 3 parameters should be defined."
    )]
    TooManyBounds {
        /// The name of the start parameter.
        start: String,
        /// The name of the end parameter.
        end: String,
        /// The name of the timespan parameter.
        timespan: String,
    },
    /// A timestamp or a timespan in a form that is not one.
    #[error("Invalid format for parameter '{0}'.")]
    InvalidFormat(String),
    /// A timespan that is the empty text.
    #[error("Parameter '{0}' is empty.")]
    Empty(String),
    /// A negative timespan.
    #[error("'{0}' cannot be negative.")]
    Negative(String),
    /// A null timespan ending at the end of time.
    #[error("Cannot set '{timespan}' to 0 and '{end}' to MAX_VALUE.")]
    EmptyBeforeMaxEnd {
        /// The name of the timespan parameter.
        timespan: String,
        /// The name of the end parameter.
        end: String,
    },
    /// A null timespan starting at the beginning of time.
    #[error("Cannot set '{timespan}' to 0 and '{start}' to MIN_VALUE.")]
    EmptyAfterMinStart {
        /// The name of the timespan parameter.
        timespan: String,
        /// The name of the start parameter.
        start: String,
    },
    /// A start and a timespan without an end, or no bound at all.
    #[error("Missing '{end}' or '{start}' and '{timespan}' parameter.")]
    MissingEnd {
        /// The name of the end parameter.
        end: String,
        /// The name of the start parameter.
        start: String,
        /// The name of the timespan parameter.
        timespan: String,
    },
    /// An end without a start, a timespan or a count.
    #[error(
        "Invalid time range specification: '{count}' is mandatory if '{start}' and '{timespan}' are not specified."
    )]
    CountMandatory {
        /// The name of the count parameter.
        count: String,
        /// The name of the start parameter.
        start: String,
        /// The name of the timespan parameter.
        timespan: String,
    },
}
