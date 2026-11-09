// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the time functions: zones, ISO 8601 parsing, durations, chunks and calendar
//! buckets, worded like Warp 10's (and like Joda-Time's and `java.time`'s, whose messages
//! they repeat) because a script reads them.

/// A failure of a time function.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimeError {
    /// A time zone identifier that does not exist.
    #[error("The datetime zone id '{0}' is not recognised")]
    UnknownZone(String),
    /// A local time in the gap of a daylight saving time transition.
    #[error(
        "Illegal instant due to time zone offset transition (daylight savings time 'gap'): {local} ({zone})"
    )]
    ZoneGap {
        /// The local date and time.
        local: String,
        /// The time zone.
        zone: String,
    },
    /// A time zone offset that is not written like `+HH:mm`.
    #[error("Invalid format: \"{id}\" is malformed at \"{rest}\"")]
    MalformedOffset {
        /// The identifier given.
        id: String,
        /// The part of it that could not be read.
        rest: String,
    },
    /// An instant out of the range of dates.
    #[error("The instant {0} is out of range.")]
    InstantOutOfRange(i64),
    /// A date or time field out of its range.
    #[error("Value {value} for {field} must be in the range [{min},{max}]{suffix}")]
    FieldOutOfRange {
        /// The value given.
        value: i32,
        /// The name of the field.
        field: String,
        /// The smallest value allowed.
        min: i32,
        /// The largest value allowed.
        max: i32,
        /// What follows the range in the message.
        suffix: String,
    },
    /// A date calculation that overflows.
    #[error("The calculation caused an overflow.")]
    CalculationOverflow,
    /// A duration with more than one fraction separator.
    #[error("invalid ISO8601 duration.")]
    InvalidIsoDuration,
    /// A duration with more than one fraction separator, as `DURATION` reports it.
    #[error("Invalid ISO8601 duration")]
    InvalidDuration,
    /// A duration in years or months, which have no fixed length.
    #[error(
        "no support for ambiguous durations containing years or months, please convert those to days."
    )]
    AmbiguousDuration,
    /// A duration that could not be parsed.
    #[error("Parsing of duration without sub second precision has failed. Tried to parse: {0}")]
    UnparsableDuration(String),
    /// A timestamp that could not be parsed at an index.
    #[error("Text '{text}' could not be parsed at index {index}")]
    Unparsable {
        /// The text given.
        text: String,
        /// Where reading failed.
        index: usize,
    },
    /// A timestamp that could not be parsed, for a reason.
    #[error("Text '{text}' could not be parsed: {detail}")]
    UnparsableBecause {
        /// The text given.
        text: String,
        /// Why.
        detail: String,
    },
    /// A timestamp followed by text.
    #[error("Text '{text}' could not be parsed, unparsed text found at index {index}")]
    UnparsedText {
        /// The text given.
        text: String,
        /// Where the unread text starts.
        index: usize,
    },
    /// `NOTBEFORE` / `NOTAFTER` evaluated at the wrong time.
    #[error("Current time is {} '{instant}'", if *before { "before" } else { "after" })]
    WrongTime {
        /// `true` when the current time is before the instant.
        before: bool,
        /// The instant, as the script wrote it.
        instant: String,
    },
    /// A timescale applied to a bucketized series.
    #[error("Cannot apply timescale on a bucketized GTS. Unbucketize it first.")]
    TimescaleOnBucketized,
    /// A chunk overlap larger than the chunk width.
    #[error("Overlap cannot exceed chunk width.")]
    OverlapExceedsWidth,
    /// A series which already has the label chunks are numbered with.
    #[error("Cannot operate on Geo Time Series which already have a label named '{0}'")]
    ChunkLabelTaken(String),
    /// An encoder which already has the label chunks are numbered with.
    #[error("Cannot operate on encoders which already have a label named '{0}'")]
    EncoderChunkLabelTaken(String),
    /// A bucketized series whose buckets are wider than the chunks.
    #[error("Cannot operate on Geo Time Series with a bucketspan greater than the chunk width.")]
    BucketspanExceedsChunk,
    /// A chunking that reads the ticks of a series that has none.
    #[error("Cannot load from long array because \"cropped.ticks\" is null")]
    NoTicks,
    /// A series that is not sorted where sorting is required.
    #[error("GTS {0} is not sorted.")]
    NotSorted(String),
    /// A series sorted the other way round.
    #[error("GTS {0} is not sorted in the expected order.")]
    WrongOrder(String),
    /// An encoder sorted the other way round.
    #[error("ENCODER {0} is not sorted in expected order.")]
    EncoderWrongOrder(String),
    /// A list that holds something else than series, encoders or lists of them.
    #[error("expects a list of Geo Time Series or encoders or of lists thereof.")]
    NotSeriesOrEncoders,
    /// Chunks of different types to fuse.
    #[error("Inconsistent types for chunks to fuse.")]
    InconsistentChunkTypes,
    /// A calendar period that moves backwards.
    #[error(
        "Period is negative from timestamp {origin}. Can not add a negative period. Period is {times} times {period} plus {offset} time unit(s)."
    )]
    NegativePeriod {
        /// The timestamp the period is added to.
        origin: i64,
        /// How many periods are added.
        times: i64,
        /// The period.
        period: String,
        /// The offset added to it.
        offset: i64,
    },
    /// More calendar buckets than the limit allows.
    #[error(
        "Bucket count ({count}) is exceeding maximum value of {max}. Consider raising the limit or using capabilities."
    )]
    CalendarBucketLimit {
        /// The number of buckets.
        count: i64,
        /// The limit.
        max: i64,
    },
}
