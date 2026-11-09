// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::error::Error as _;
use std::fmt::{self, Write};

use super::error::compat::{self, exception_class};
use super::error::{Class, Error, IoFailure};
use super::finite::NotFinite;
use super::value::ValueKind;

/// A writer that fails after `n` successful writes, to exercise the error paths of `Display`.
pub(crate) struct FailAfter(pub usize);

impl Write for FailAfter {
    fn write_str(&mut self, _s: &str) -> fmt::Result {
        if self.0 == 0 {
            return Err(fmt::Error);
        }
        self.0 -= 1;
        Ok(())
    }
}

#[test]
fn display_propagates_writer_failures() {
    let err = Error::with_cause("outer", Error::with_cause("middle", Error::new("root")));
    let outcomes: Vec<bool> = (0..9)
        .map(|n| write!(FailAfter(n), "{err}").is_ok())
        .collect();
    assert_eq!(
        outcomes,
        [false, false, false, false, false, false, false, true, true]
    );
}

#[test]
fn messages_and_causes() {
    let root = Error::new("root");
    let middle = Error::with_cause("middle", root.clone());
    let outer = Error::with_cause("outer", middle);
    assert_eq!(outer.message(), "outer");
    assert_eq!(outer.root_cause(), &root);
    assert_eq!(outer.messages(), vec!["outer", "middle", "root"]);
    assert_eq!(outer.to_string(), "outer (middle (root))");
    assert_eq!(root.to_string(), "root");
    assert!(!outer.is_signal());
    assert_eq!(root.cause(), None);
    assert_eq!(outer.cause().map(Error::message), Some("middle".to_owned()));
}

#[test]
fn signals() {
    assert!(Error::Return.is_signal());
    assert!(Error::Break.is_signal());
    assert!(Error::Continue.is_signal());
    assert!(Error::Stop("STOP".to_owned()).is_signal());
    assert!(Error::Abort("abort".to_owned()).is_signal());
    assert_eq!(Error::Return.message(), "RETURN");
    assert_eq!(Error::Stop("x".to_owned()).message(), "x");
    assert_eq!(Error::Abort("y".to_owned()).message(), "y");
    assert_eq!(
        Error::Break.message(),
        "Exception at BREAK: must be used in a one of FOR, FOREACH, FORSTEP, WHILE, UNTIL loop."
    );
    assert!(
        Error::Continue
            .message()
            .starts_with("Exception at CONTINUE:")
    );
    assert_eq!(Error::Break.root_cause(), &Error::Break);
    assert_eq!(Error::Break.messages().len(), 1);
    let wrapped = Error::with_cause("Line #1", Error::Break);
    assert_eq!(wrapped.messages().len(), 2);
    let as_std: &dyn std::error::Error = &wrapped;
    assert!(as_std.to_string().starts_with("Line #1 ("));
    assert_eq!(
        as_std.source().map(ToString::to_string),
        Some(Error::Break.to_string())
    );
}

#[test]
fn typed_failures() {
    assert_eq!(Error::empty_stack(), Error::EmptyStack);
    assert_eq!(Error::empty_stack().to_string(), "Empty stack.");
    assert_eq!(
        Error::class_cast(ValueKind::String, Class::Number).to_string(),
        "class java.lang.String cannot be cast to class java.lang.Number"
    );
    assert_eq!(
        Error::index_out_of_bounds(-1, 3).to_string(),
        "Index -1 out of bounds for length 3"
    );
    assert_eq!(Error::null_pointer().to_string(), "NullPointerException");
    for err in [
        Error::empty_stack(),
        Error::class_cast(ValueKind::String, Class::Number),
        Error::index_out_of_bounds(0, 0),
        Error::null_pointer(),
        Error::runtime("java.lang.ArithmeticException", "/ by zero"),
    ] {
        assert!(err.is_runtime(), "{err}");
        assert!(!err.is_signal(), "{err}");
    }
    for err in [
        Error::new("plain"),
        Error::with_cause("wrapped", Error::empty_stack()),
        Error::from(NotFinite::Nan),
    ] {
        assert!(!err.is_runtime(), "{err}");
    }
}

#[test]
fn not_finite_is_a_typed_failure() {
    let err = Error::from(NotFinite::NegativeInfinity);
    assert_eq!(err, Error::NotFinite(NotFinite::NegativeInfinity));
    assert_eq!(err.to_string(), "not a finite number: -Infinity");
    assert_eq!(err.message(), err.to_string());
    assert!(err.source().is_none());
}

#[test]
fn exception_classes() {
    let cases = [
        (Error::new("plain"), "io.warp10.script.WarpScriptException"),
        (
            Error::with_cause("wrapped", Error::new("root")),
            "io.warp10.script.WarpScriptException",
        ),
        (
            Error::from(NotFinite::Nan),
            "io.warp10.script.WarpScriptException",
        ),
        (
            Error::runtime("java.lang.ArithmeticException", "/ by zero"),
            "java.lang.ArithmeticException",
        ),
        (Error::Return, "io.warp10.script.WarpScriptReturnException"),
        (
            Error::Stop(String::new()),
            "io.warp10.script.WarpScriptStopException",
        ),
        (
            Error::Break,
            "io.warp10.script.WarpScriptLoopBreakException",
        ),
        (
            Error::Continue,
            "io.warp10.script.WarpScriptLoopContinueException",
        ),
        (
            Error::Abort(String::new()),
            "io.warp10.script.WarpScriptATCException",
        ),
        (
            Error::empty_stack(),
            "io.warp10.script.InformativeEmptyStackException",
        ),
        (
            Error::class_cast(ValueKind::String, Class::Number),
            "java.lang.ClassCastException",
        ),
        (
            Error::index_out_of_bounds(1, 0),
            "java.lang.ArrayIndexOutOfBoundsException",
        ),
        (Error::null_pointer(), "java.lang.NullPointerException"),
    ];
    for (err, class) in cases {
        assert_eq!(exception_class(&err), class, "{err}");
    }
}

#[test]
fn classes_of_every_kind_of_value() {
    for (kind, class) in [
        (ValueKind::Null, "null"),
        (ValueKind::Boolean, "java.lang.Boolean"),
        (ValueKind::Long, "java.lang.Long"),
        (ValueKind::Double, "java.lang.Double"),
        (ValueKind::BigDecimal, "java.math.BigDecimal"),
        (
            ValueKind::Matrix,
            "org.apache.commons.math3.linear.Array2DRowRealMatrix",
        ),
        (
            ValueKind::Vector,
            "org.apache.commons.math3.linear.ArrayRealVector",
        ),
        (ValueKind::String, "java.lang.String"),
        (ValueKind::Bytes, "[B"),
        (ValueKind::BitSet, "java.util.BitSet"),
        (ValueKind::List, "java.util.ArrayList"),
        (ValueKind::Map, "java.util.LinkedHashMap"),
        (ValueKind::Set, "java.util.HashSet"),
        (ValueKind::Macro, "io.warp10.script.WarpScriptStack$Macro"),
        (
            ValueKind::Function,
            "io.warp10.script.NamedWarpScriptFunction",
        ),
        (ValueKind::Mark, "io.warp10.script.WarpScriptStack$Mark"),
        (
            ValueKind::Context,
            "io.warp10.script.MemoryWarpScriptStack$StackContext",
        ),
        (ValueKind::Counter, "java.util.concurrent.atomic.AtomicLong"),
        (ValueKind::Gts, "io.warp10.continuum.gts.GeoTimeSerie"),
        (ValueKind::Encoder, "io.warp10.continuum.gts.GTSEncoder"),
    ] {
        assert_eq!(compat::class_name(Class::Value(kind)), class);
        assert_eq!(compat::value_class(kind), class);
    }
    assert_eq!(compat::class_name(Class::Number), "java.lang.Number");
    assert_eq!(
        compat::class_name(Class::Comparable),
        "java.lang.Comparable"
    );
    assert_eq!(compat::class_name(Class::List), "java.util.List");
    assert_eq!(compat::class_name(Class::Integer), "java.lang.Integer");
    assert_eq!(
        compat::class_name(Class::ObjectArray),
        "[Ljava.lang.Object;"
    );
    assert_eq!(Class::from(ValueKind::Long), Class::Value(ValueKind::Long));
}

#[test]
fn every_value_has_a_kind() {
    use std::rc::Rc;

    use super::value::Value;

    let values = [
        (Value::Null, ValueKind::Null),
        (Value::Boolean(true), ValueKind::Boolean),
        (Value::Long(1), ValueKind::Long),
        (Value::Double(1.0), ValueKind::Double),
        (Value::string("s"), ValueKind::String),
        (Value::bytes([1]), ValueKind::Bytes),
        (Value::list(Vec::new()), ValueKind::List),
        (Value::Mark, ValueKind::Mark),
        (Value::Vector(Rc::from(vec![1.0])), ValueKind::Vector),
    ];
    for (value, kind) in values {
        assert_eq!(value.kind(), kind);
    }
}

#[test]
fn io_failures_compare_by_kind_and_message() {
    let failure =
        |kind, message: &str| IoFailure::from(std::io::Error::new(kind, message.to_owned()));
    let a = failure(std::io::ErrorKind::UnexpectedEof, "short");
    assert_eq!(a, a.clone());
    assert_eq!(a, failure(std::io::ErrorKind::UnexpectedEof, "short"));
    assert_ne!(a, failure(std::io::ErrorKind::InvalidData, "short"));
    assert_ne!(a, failure(std::io::ErrorKind::UnexpectedEof, "long"));
    assert_eq!(a.to_string(), "short");
    assert!(a.source().is_none());
}

#[test]
fn platform_failures_have_the_wording_and_class_of_warp10() {
    use super::error::fault::{
        ArithmeticError, BoundsError, IllegalArgumentError, NumberFormatError, ResourceError,
    };

    let failures: Vec<(Error, &str, &str)> = vec![
        (
            ArithmeticError::DivisionByZero.into(),
            "/ by zero",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::LongOverflow.into(),
            "long overflow",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::IntegerOverflow.into(),
            "integer overflow",
            "java.lang.ArithmeticException",
        ),
        (
            NumberFormatError::Empty.into(),
            "empty String",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::InvalidInput("x".to_owned()).into(),
            "For input string: \"x\"",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::InvalidRadixInput {
                input: "g".to_owned(),
                radix: 16,
            }
            .into(),
            "For input string: \"g\" under radix 16",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::NegativeUnsigned("-1".to_owned()).into(),
            "Illegal leading minus sign on unsigned string -1.",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::NotFinite.into(),
            "Infinite or NaN",
            "java.lang.NumberFormatException",
        ),
        (
            BoundsError::NegativeSize(-3).into(),
            "-3",
            "java.lang.NegativeArraySizeException",
        ),
        (
            Error::index_out_of_bounds(4, 2),
            "Index 4 out of bounds for length 2",
            "java.lang.ArrayIndexOutOfBoundsException",
        ),
        (
            BoundsError::ListIndex {
                index: -1,
                length: 0,
            }
            .into(),
            "Index -1 out of bounds for length 0",
            "java.lang.IndexOutOfBoundsException",
        ),
        (
            BoundsError::ListEnd(9).into(),
            "toIndex = 9",
            "java.lang.IndexOutOfBoundsException",
        ),
        (
            BoundsError::StringIndex {
                index: 1,
                length: 1,
            }
            .into(),
            "Index 1 out of bounds for length 1",
            "java.lang.StringIndexOutOfBoundsException",
        ),
        (
            BoundsError::StringRange {
                start: 1,
                end: 0,
                length: 1,
            }
            .into(),
            "Range [1, 0) out of bounds for length 1",
            "java.lang.StringIndexOutOfBoundsException",
        ),
        (
            IllegalArgumentError::Capacity(-2).into(),
            "Illegal Capacity: -2",
            "java.lang.IllegalArgumentException",
        ),
        (
            IllegalArgumentError::MillisOutOfRange(90_000_000).into(),
            "Millis out of range: 90000000",
            "java.lang.IllegalArgumentException",
        ),
        (
            IllegalArgumentError::EscapeNotHexadecimal {
                digit: 'g',
                code: 103,
            }
            .into(),
            "URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"g\" = 103",
            "java.lang.IllegalArgumentException",
        ),
        (
            IllegalArgumentError::IncompleteEscape.into(),
            "URLDecoder: Incomplete trailing escape (%) pattern",
            "java.lang.IllegalArgumentException",
        ),
        (
            ResourceError::ArrayTooLong.into(),
            "Requested array size exceeds VM limit",
            "java.lang.OutOfMemoryError",
        ),
        (
            ResourceError::OutOfMemory.into(),
            "Java heap space",
            "java.lang.OutOfMemoryError",
        ),
        (
            ArithmeticError::Underflow.into(),
            "Underflow",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::Overflow.into(),
            "Overflow",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::RoundingNecessary.into(),
            "Rounding necessary",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::InvalidOperation.into(),
            "Invalid operation",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::NonTerminatingExpansion.into(),
            "Non-terminating decimal expansion; no exact representable decimal result.",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::DivisionUndefined.into(),
            "Division undefined",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::DecimalDivisionByZero.into(),
            "Division by zero",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::NegativeBitAddress.into(),
            "Negative bit address",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::ModulusNotPositive.into(),
            "BigInteger: modulus not positive",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::NotInvertible.into(),
            "BigInteger not invertible.",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::BigIntegerOutOfIntRange.into(),
            "BigInteger out of int range",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::BitLengthTooSmall.into(),
            "bitLength < 2",
            "java.lang.ArithmeticException",
        ),
        (
            ArithmeticError::NegativeStart(num_bigint::BigInt::from(-5)).into(),
            "start < 0: -5",
            "java.lang.ArithmeticException",
        ),
        (
            NumberFormatError::Bare.into(),
            "NumberFormatException",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::MultipleDecimalPoints.into(),
            "Character array contains more than one decimal point.",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::NoDigits.into(),
            "No digits found.",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::NotADigit.into(),
            "Not a digit.",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::ExponentTooLong.into(),
            "Too many nonzero exponent digits.",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::ScaleOutOfRange.into(),
            "Scale out of range.",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::ZeroLengthBigInteger.into(),
            "Zero length BigInteger",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::EmbeddedSign.into(),
            "Illegal embedded sign character",
            "java.lang.NumberFormatException",
        ),
        (
            NumberFormatError::InvalidDecimalCharacter('x').into(),
            "Character x is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark.",
            "java.lang.NumberFormatException",
        ),
        (
            Error::Immutable,
            "UnsupportedOperationException",
            "java.lang.UnsupportedOperationException",
        ),
        (
            Error::division_by_zero(),
            "/ by zero",
            "java.lang.ArithmeticException",
        ),
    ];
    for (error, message, class) in failures {
        assert_eq!(error.to_string(), message);
        assert_eq!(exception_class(&error), class, "{message}");
        assert!(error.is_runtime(), "{message}");
    }
}

#[test]
fn math_failures_have_the_wording_and_class_of_commons_math() {
    use super::error::math::MathError;

    const EXCEPTION: &str = "org.apache.commons.math3.exception.";
    const LINEAR: &str = "org.apache.commons.math3.linear.";
    let failures = [
        (
            MathError::TooManyIterations { max: 1000 },
            "illegal state: maximal count (1,000) exceeded: iterations",
            format!("{EXCEPTION}TooManyIterationsException"),
        ),
        (
            MathError::QrDecomposition {
                rows: 2000,
                cols: 3,
            },
            "illegal state: unable to perform Q.R decomposition on the 2,000x3 jacobian matrix",
            format!("{EXCEPTION}ConvergenceException"),
        ),
        (
            MathError::NotStrictlyPositive {
                value: 0,
                minimum: 0,
            },
            "0 is smaller than, or equal to, the minimum (0)",
            format!("{EXCEPTION}NotStrictlyPositiveException"),
        ),
        (
            MathError::NoData,
            "no data",
            format!("{EXCEPTION}NoDataException"),
        ),
        (
            MathError::EmptyPolynomialCoefficients,
            "empty polynomials coefficients array",
            format!("{EXCEPTION}NoDataException"),
        ),
        (
            MathError::MatrixWithoutColumn,
            "matrix must have at least one column",
            format!("{EXCEPTION}NoDataException"),
        ),
        (
            MathError::OutOfRange {
                value: 1.5,
                low: 0.0,
                high: 1.0,
            },
            "1.5 out of [0, 1] range",
            format!("{EXCEPTION}OutOfRangeException"),
        ),
        (
            MathError::NoBracketing {
                lower: 0.0,
                upper: 1.0,
                lower_value: 2.0,
                upper_value: 3.0,
            },
            "function values at endpoints do not have different signs, endpoints: [0, 1], values: [2, 3]",
            format!("{EXCEPTION}NoBracketingException"),
        ),
        (
            MathError::DimensionMismatch {
                wrong: 2,
                expected: 3,
            },
            "2 != 3",
            format!("{EXCEPTION}DimensionMismatchException"),
        ),
        (
            MathError::NonSquareMatrix { rows: 2, cols: 3 },
            "non square (2x3) matrix",
            format!("{LINEAR}NonSquareMatrixException"),
        ),
        (
            MathError::MatrixDimensionMismatch {
                rows: 1,
                cols: 2,
                expected_rows: 3,
                expected_cols: 4,
            },
            "got 1x2 but expected 3x4",
            format!("{LINEAR}MatrixDimensionMismatchException"),
        ),
        (
            MathError::SingularMatrix,
            "matrix is singular",
            format!("{LINEAR}SingularMatrixException"),
        ),
        (
            MathError::NonMonotonic {
                index: 2,
                previous: 5.0,
                wrong: 4.0,
            },
            "points 1 and 2 are not strictly increasing (5 >= 4)",
            format!("{EXCEPTION}NonMonotonicSequenceException"),
        ),
        (
            MathError::TooFewPoints(1),
            "number of points (1)",
            format!("{EXCEPTION}NumberIsTooSmallException"),
        ),
        (
            MathError::TwoPointsRequired(1),
            "2 points are required, got only 1",
            format!("{EXCEPTION}NumberIsTooSmallException"),
        ),
        (
            MathError::BandwidthTooSmall(1),
            "bandwidth (1)",
            format!("{EXCEPTION}NumberIsTooSmallException"),
        ),
        (
            MathError::NotFiniteNumber(f64::NAN),
            "NaN is not a finite number",
            format!("{EXCEPTION}NotFiniteNumberException"),
        ),
    ];
    for (failure, message, class) in failures {
        let error = Error::from(failure);
        assert_eq!(error.to_string(), message);
        assert_eq!(exception_class(&error), class, "{message}");
        assert!(error.is_runtime(), "{message}");
    }
}
