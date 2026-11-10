// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the Geo Time Series functions: filling, bucketizing, smoothing, statistics,
//! patterns, filters and interpolators, worded like Warp 10's because a script reads them.

use crate::format::java_double_to_string;

/// A failure of a Geo Time Series function.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SeriesError {
    /// `FILL` given parameters of the wrong types.
    #[error("FILL's argument types do not match the expected ones.")]
    FillArguments,
    /// `FILL` given a list whose last parameter is not a list.
    #[error("FILLexpects the last parameter of the input LIST to be a LIST")]
    FillLastParameter,
    /// A `ticks` parameter holding something else than longs: Warp 10's sentence, then the
    /// type found.
    #[error("{message} {found}")]
    FillTicks {
        /// What the function expects.
        message: &'static str,
        /// The type found.
        found: &'static str,
    },
    /// A list of series holding something else: Warp 10's sentence, then the type found.
    #[error("{message} {found}")]
    FillListElement {
        /// What the function expects.
        message: &'static str,
        /// The type found.
        found: &'static str,
    },
    /// `FILL` not given a series: Warp 10's sentence.
    #[error("{0}")]
    FillNotSeries(&'static str),
    /// `FILL` of a series with duplicate timestamps.
    #[error("Cannot fill Geo Time Series with duplicate timestamps.")]
    FillDuplicateTimestamps,
    /// A macro that did not return a latitude and a longitude of type `DOUBLE`.
    #[error("Macro MUST return a latitude and a longitude which are of type DOUBLE.")]
    MacroLocation,
    /// A macro that returned an elevation of type `DOUBLE` which is not NaN.
    #[error("Macro MUST return an elevations which is NaN if it is of type DOUBLE.")]
    MacroElevationNan,
    /// A macro that returned an elevation which is neither NaN nor a `LONG`.
    #[error("Macro MUST return an elevation which is either NaN or LONG.")]
    MacroElevationType,
    /// A macro that did not return a tick of type `LONG`.
    #[error("Macro MUST return a tick of type LONG.")]
    MacroTick,
    /// A bucketization with neither a bucket span nor a bucket count.
    #[error("One of bucketspan or bucketcount must be different from zero.")]
    BucketParameters,
    /// A bucketization of more buckets than the limit allows.
    #[error(
        "Bucket count ({count}) would exceed maximum value of {max}. Consider raising the limit or using capabilities."
    )]
    BucketLimit {
        /// The number of buckets.
        count: i32,
        /// The limit.
        max: i64,
    },
    /// A bucketization whose span cannot be determined.
    #[error("Undefined bucket span, check your GTS timestamps.")]
    UndefinedBucketSpan,
    /// A parameter that is not a mapper.
    #[error("Expected a mapper function.")]
    ExpectedMapper,
    /// `LOWESS` of a series that is not numeric.
    #[error("Can only smooth numeric Geo Time Series.")]
    LowessNotNumeric,
    /// A `LOWESS` bandwidth below one point.
    #[error("Bandwidth parameter must be greater than 0")]
    LowessBandwidth,
    /// A negative `LOWESS` robustness.
    #[error("Robustness parameter must be greater or equal to 0")]
    LowessRobustness,
    /// A `LOWESS` degree below one.
    #[error("Degree of polynomial fit must be greater than 0")]
    LowessDegreeTooSmall,
    /// A `LOWESS` degree above nine.
    #[error("Degree of polynomial fit should remain small (lower than 10)")]
    LowessDegreeTooLarge,
    /// A `LOWESS` of a series with too many missing values.
    #[error("More than 500000 missing values")]
    LowessMissingValues,
    /// A `LOWESS` of a series with duplicate ticks.
    #[error("Can't be applied on GTS with duplicate ticks")]
    DuplicateTicks,
    /// A downsampling threshold below three.
    #[error("Threshold MUST be >= 3.")]
    ThresholdTooSmall,
    /// A smoothing factor outside of ]0, 1[. The message starts with the name of the factor.
    #[error("{0} should be strictly between 0.0 and 1.0.")]
    FactorOutOfRange(String),
    /// An exponential smoothing of a series that is not numeric.
    #[error("Can only perform exponential smoothing on numeric Geo Time Series.")]
    ExponentialNotNumeric,
    /// An exponential smoothing of a series of fewer than two values.
    #[error(
        "Can only perform exponential smoothing on Geo Time Series containing at least two values."
    )]
    ExponentialTooShort,
    /// A normalization by a null deviation.
    #[error("{} Deviation is null", if *median { "Median Absolute" } else { "Standard" })]
    NullDeviation {
        /// `true` for the median absolute deviation, `false` for the standard one.
        median: bool,
    },
    /// `QUANTIFY` of a series that is not numeric.
    #[error("Can only quantify numeric Geo Time Series.")]
    QuantifyNotNumeric,
    /// A numerical operator given operands it does not accept.
    #[error(
        "{function} can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values{}.",
        if *reduces { ", or a list of numerical values" } else { "" }
    )]
    NumericalOperands {
        /// The operator.
        function: String,
        /// Whether the operator also reduces a single list.
        reduces: bool,
    },
    /// A selector that is not `class{labels}`.
    #[error("invalid syntax for selector.")]
    InvalidSelector,
    /// A selector that is not `class{labels}`, as the selector functions report it (sic:
    /// the message starts with a space).
    #[error(" invalid syntax for selector.")]
    InvalidSelectorSpaced,
    /// A conditional probability without a separator in the events.
    #[error("Separator not found, unable to isolate given events.")]
    SeparatorNotFound,
    /// A conditional probability of a series that is not made of strings.
    #[error("Can only compute conditional probabilities for String Geo Time Series.")]
    ConditionalProbabilityTypes,
    /// A comparison of series that are neither numeric nor strings (sic: no space after the name).
    #[error("{0}can only operate on two GTS with NUMBER or STRING values.")]
    ComparisonOperands(String),
    /// A datapoint given as a list of another size than one to five.
    #[error("Expected a list of 1 to 5 elements, got {0}")]
    DatapointListSize(usize),
    /// A filler that did not return a datapoint.
    #[error("Expected a [ ts lat lon elev value ] list as result of filler.")]
    FillerResult,
    /// A class selector that is not a string.
    #[error("Invalid class selector")]
    InvalidClassSelector,
    /// A labels or attributes selector that is not a map.
    #[error("Invalid labels/attributes selector")]
    InvalidLabelsSelector,
    /// A threshold of a type the filter cannot compare.
    #[error("Invalid threshold type.")]
    InvalidThresholdType,
    /// An interpolation with lists of different sizes.
    #[error("{0}: incoherent argument sizes, both input LIST should have the same size.")]
    InterpolatorSizes(String),
    /// A bicubic interpolation with a grid that is not rectangular.
    #[error(
        "INTERPOLATOR.2D.BICUBIC: incoherent argument sizes. Up to vector sizes, values should be a LIST (size {rows}) of LIST (size {columns})."
    )]
    BicubicSizes {
        /// The number of ordinates.
        rows: usize,
        /// The number of abscissas.
        columns: usize,
    },
    /// A tricubic interpolation with a grid that is not a cuboid.
    #[error(
        "INTERPOLATOR.3D.TRICUBIC: incoherent argument sizes. Up to vector sizes, values should be a LIST (size {x}) of LIST (size {y}) of LIST (size {z})."
    )]
    TricubicSizes {
        /// The number of abscissas.
        x: usize,
        /// The number of ordinates.
        y: usize,
        /// The number of depths.
        z: usize,
    },
    /// A rotation by something that is not a unit quaternion.
    #[error(
        "Input does not appear to be a unit quaternion, norm was {}.",
        java_double_to_string(*.0)
    )]
    NotUnitQuaternion(f64),
    /// A pattern function applied to a series that is not numeric, bucketized and filled.
    #[error("Function can only be applied to numeric, bucketized, filled Geo Time Series.")]
    PatternSeries,
    /// A word length that does not divide the window length.
    #[error("Wordlen MUST divide windowlen.")]
    WordlenDivide,
    /// An alphabet size that is not a power of two.
    #[error("Alphabet size MUST be a power of two.")]
    AlphabetPowerOfTwo,
    /// An alphabet size outside of the supported range.
    #[error("Alphabet size MUST be a power of two between 2 and 2^{0}")]
    AlphabetRange(u32),
    /// `FFT` or `FFTAP` of a series that is not numeric (the message always names `FFT`).
    #[error("FFT can only be applied to numeric Geo Time Series.")]
    FftNotNumeric,
    /// `FFT` or `FFTAP` of a series that is not bucketized.
    #[error("FFT can only be applied to bucketized Geo Time Series.")]
    FftNotBucketized,
    /// `FFT` of a series with a tick that is not on a bucket boundary.
    #[error("Found a tick not on a bucket boundary.")]
    TickOffBucketBoundary,
    /// `FFT` of a series with a duplicate tick.
    #[error("Found duplicate tick.")]
    FftDuplicateTick,
    /// `FFT` of a series with a gap between 2 ticks.
    #[error("Found a displaced tick, should have been 'bucketspan' away from previous.")]
    DisplacedTick,
    /// `IFFT` of real and imaginary parts of different lengths.
    #[error("Real and imaginary Geo Time Series MUST have the same number of values.")]
    PartsLength,
    /// `IFFT` of parts that are not `DOUBLE` series.
    #[error("{0} real and imaginary parts GTS MUST be of type DOUBLE.")]
    PartsType(String),
    /// `FFTWINDOW` of a series that is not numeric, with its selector.
    #[error("{function} expects numeric Geo Time Series. {selector} is not numeric.")]
    WindowNotNumeric {
        /// The function.
        function: String,
        /// The selector of the series.
        selector: String,
    },
    /// `FFTWINDOW` given an algorithm it does not know.
    #[error("{function} given an unknown windowing function '{algorithm}'.")]
    UnknownWindow {
        /// The function.
        function: String,
        /// The algorithm, lowercased.
        algorithm: String,
    },
    /// A wavelet transform given a wavelet it does not know.
    #[error("{function} could not find wavelet '{wavelet}'.")]
    UnknownWavelet {
        /// The function.
        function: String,
        /// The name of the wavelet.
        wavelet: String,
    },
    /// A wavelet function applied to a series that is not numeric.
    #[error("{0} can only be applied to numeric Geo Time Series.")]
    WaveletNotNumeric(String),
    /// A wavelet function applied to a series whose length is not a power of 2.
    #[error("{0} can only operate on Geo Time Series whose length is a power of 2.")]
    NotPowerOfTwo(String),
    /// `DWTSPLIT` of a series that already has the level label.
    #[error("{function} input cannot contain label '{label}'")]
    LevelLabelPresent {
        /// The function.
        function: String,
        /// The label.
        label: String,
    },
    /// A test or a split of a series that is not bucketized.
    #[error("GTS must be bucketized")]
    NotBucketized,
    /// `subCycleSerie` given a bucket that is not one of the series.
    #[error(
        "lasbucket parameter of subCycleSerie method must fall on an actual bucket of the gts input"
    )]
    CycleOffBucket,
    /// A piecewise outlier test with more suspected outliers than half of a piece.
    #[error(
        "Upper bound of number of outliers must be less than half of the number of observations per piece"
    )]
    OutliersUpperBound,
    /// A piecewise outlier test of a series shorter than a seasonal period.
    #[error("Not enough buckets to make up at least one seasonal period.")]
    NoSeasonalPeriod,
    /// A piecewise outlier test of a series shorter than a piece.
    #[error(
        "Not enough seasonal periods to make up at least one piece. Please use a lower number of periods per piece."
    )]
    NoPiece,
    /// A test that needs a series that is bucketized and has a value in every bucket.
    #[error("{0} operates on bucketized, filled Geo Time Series.")]
    NotBucketizedFilled(String),
    /// A piecewise outlier test of a series with a tick that is not on a bucket.
    #[error("Internal bug method {method}: can't find tick {tick} in seasonal.ticks")]
    TickNotSeasonal {
        /// The method of Warp 10.
        method: String,
        /// The tick.
        tick: i64,
    },
    /// `STL` of a series that is not numeric.
    #[error("Can only be applied on numeric Geo Time Series.")]
    StlNotNumeric,
    /// `STL` of a series that is not bucketized.
    #[error("Can only be applied on bucketized Geo Time Series")]
    StlNotBucketized,
    /// `STL` that finds no value to average in a step.
    #[error("STL found no value in its step {0}, is GTS empty?")]
    StlNoValue(String),
    /// `STL` smoothing a cycle that is not bucketized.
    #[error("lowess_stl method works with bucketized gts only")]
    StlCycleNotBucketized,
    /// A regression given robustness weights fewer than the datapoints it reads.
    #[error("Incoherent array lengths as input of pointwise_lowess")]
    IncoherentLowessArrays,
    /// A seasonal test whose period is not that of the decomposition it is given.
    #[error("Incoherence between PERIOD parameter of test and PERIOD parameter of STL")]
    PeriodMismatch,
    /// `INTERPOLATOR.ND.*` given series of different sizes.
    #[error("{0}: incoherent GTS sizes.")]
    MicrosphereSeriesSizes(String),
    /// `INTERPOLATOR.ND.*` given series that do not have the same ticks.
    #[error("{0}: GTS are not synchronized.")]
    MicrosphereNotSynchronized(String),
    /// `INTERPOLATOR.ND.*` given lists of different sizes.
    #[error("{0}: incoherent input list sizes.")]
    MicrosphereListSizes(String),
    /// `INTERPOLATOR.ND.*` given something that is neither series nor lists.
    #[error("{0}: wrong argument type")]
    MicrosphereArgument(String),
    /// `INTERPOLATOR.ND.*` given more elements than it may.
    #[error("{0}: argument elements is above the limit interpolation.microsphere.max.elements")]
    MicrosphereElementsLimit(String),
}
