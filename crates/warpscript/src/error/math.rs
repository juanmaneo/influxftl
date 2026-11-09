// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The failures of the Commons Math routines WarpLib calls (interpolation, curve fitting,
//! distributions, matrices), worded like the (en_US) messages of Commons Math 3.6.1 because a
//! script reads them.

/// A failure of a Commons Math routine.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MathError {
    /// An optimizer ran out of iterations.
    #[error("illegal state: maximal count ({}) exceeded: iterations", group(&max.unsigned_abs().to_string()))]
    TooManyIterations {
        /// The number of iterations allowed.
        max: i32,
    },
    /// The jacobian of a least-squares problem could not be decomposed.
    #[error(
        "illegal state: unable to perform Q.R decomposition on the {}x{} jacobian matrix",
        group(&rows.to_string()),
        group(&cols.to_string())
    )]
    QrDecomposition {
        /// The number of rows of the jacobian.
        rows: usize,
        /// The number of columns of the jacobian.
        cols: usize,
    },
    /// A size that must be positive.
    #[error("{value} is smaller than, or equal to, the minimum ({minimum})")]
    NotStrictlyPositive {
        /// The size given.
        value: i64,
        /// The bound it must exceed.
        minimum: i64,
    },
    /// A function given no data at all.
    #[error("no data")]
    NoData,
    /// A polynomial given no coefficient.
    #[error("empty polynomials coefficients array")]
    EmptyPolynomialCoefficients,
    /// A matrix given rows without any column.
    #[error("matrix must have at least one column")]
    MatrixWithoutColumn,
    /// A value outside of the range it must be in.
    #[error(
        "{} out of [{}, {}] range",
        message_decimal(*value),
        message_decimal(*low),
        message_decimal(*high)
    )]
    OutOfRange {
        /// The value given.
        value: f64,
        /// The lower bound of the range.
        low: f64,
        /// The upper bound of the range.
        high: f64,
    },
    /// A root-finding interval whose ends have the same sign.
    #[error(
        "function values at endpoints do not have different signs, endpoints: [{}, {}], values: [{}, {}]",
        message_decimal(*lower),
        message_decimal(*upper),
        message_decimal(*lower_value),
        message_decimal(*upper_value)
    )]
    NoBracketing {
        /// The lower end of the interval.
        lower: f64,
        /// The upper end of the interval.
        upper: f64,
        /// The function at the lower end.
        lower_value: f64,
        /// The function at the upper end.
        upper_value: f64,
    },
    /// Two sizes that must be equal.
    #[error("{} != {}", group(&wrong.to_string()), group(&expected.to_string()))]
    DimensionMismatch {
        /// The size given.
        wrong: usize,
        /// The size expected.
        expected: usize,
    },
    /// A matrix that must be square.
    #[error(
        "non square ({}x{}) matrix",
        group(&rows.to_string()),
        group(&cols.to_string())
    )]
    NonSquareMatrix {
        /// The number of rows.
        rows: usize,
        /// The number of columns.
        cols: usize,
    },
    /// Two matrices whose dimensions differ.
    #[error(
        "got {}x{} but expected {}x{}",
        group(&rows.to_string()),
        group(&cols.to_string()),
        group(&expected_rows.to_string()),
        group(&expected_cols.to_string())
    )]
    MatrixDimensionMismatch {
        /// The number of rows given.
        rows: usize,
        /// The number of columns given.
        cols: usize,
        /// The number of rows expected.
        expected_rows: usize,
        /// The number of columns expected.
        expected_cols: usize,
    },
    /// A matrix that cannot be inverted.
    #[error("matrix is singular")]
    SingularMatrix,
    /// Abscissas that do not strictly increase.
    #[error(
        "points {} and {} are not strictly increasing ({} >= {})",
        group(&(index - 1).to_string()),
        group(&index.to_string()),
        message_number(*previous),
        message_number(*wrong)
    )]
    NonMonotonic {
        /// The index of the abscissa which does not exceed its predecessor.
        index: usize,
        /// The abscissa before it.
        previous: f64,
        /// The abscissa which does not exceed `previous`.
        wrong: f64,
    },
    /// An interpolation through too few points.
    #[error("number of points ({})", group(&.0.to_string()))]
    TooFewPoints(usize),
    /// A divided-difference interpolation through fewer than two points.
    #[error("2 points are required, got only {}", group(&.0.to_string()))]
    TwoPointsRequired(usize),
    /// A LOESS bandwidth that covers fewer than two points.
    #[error("bandwidth ({0})")]
    BandwidthTooSmall(i32),
    /// A value which is NaN or infinite where only finite values are accepted.
    #[error("{} is not a finite number", message_number(*.0))]
    NotFiniteNumber(f64),
}

/// Digits grouped by three with `,`.
pub(crate) fn group(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() * 4 / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The `MessageFormat` (en_US) rendering of a number argument: digits grouped by three,
/// `∞` for infinities. Integral values only (ticks, counts and indexes), which `{}` renders
/// with the shortest digits padded with zeros like `DecimalFormat`.
pub(crate) fn message_number(d: f64) -> String {
    if d.is_nan() {
        return "NaN".to_owned();
    }
    let digits = if d.is_infinite() {
        "∞".to_owned()
    } else {
        group(&format!("{}", d.abs()))
    };
    if d.is_sign_negative() {
        format!("-{digits}")
    } else {
        digits
    }
}

/// The `MessageFormat` (en_US) rendering of a double argument: at most three fraction digits
/// (half even), digits grouped by three.
pub(crate) fn message_decimal(d: f64) -> String {
    let rounded = format!("{:.3}", d.abs());
    let rounded = rounded.trim_end_matches('0').trim_end_matches('.');
    let (int, frac) = rounded.split_once('.').unwrap_or((rounded, ""));
    let int: f64 = int.parse().unwrap_or_default();
    let mut out = message_number(int);
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    if d.is_sign_negative() && !d.is_nan() {
        out.insert(0, '-');
    }
    out
}
