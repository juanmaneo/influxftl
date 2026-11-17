// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

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

/// A plain failure of a Warp 10 function (`WarpScriptException`, not a runtime one).
fn plain() -> Error {
    super::error::series::SeriesError::DuplicateTicks.into()
}

const PLAIN: &str = "Can't be applied on GTS with duplicate ticks";

#[test]
fn display_propagates_writer_failures() {
    let err = Error::with_cause("outer", Error::with_cause("middle", plain()));
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
    let root = plain();
    let middle = Error::with_cause("middle", root.clone());
    let outer = Error::with_cause("outer", middle);
    assert_eq!(outer.message(), "outer");
    assert_eq!(outer.root_cause(), &root);
    assert_eq!(outer.messages(), vec!["outer", "middle", PLAIN]);
    assert_eq!(outer.to_string(), format!("outer (middle ({PLAIN}))"));
    assert_eq!(root.to_string(), PLAIN);
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
    assert!(Error::Guarded(Box::new(Error::Break)).is_signal());
    assert_eq!(Error::Return.message(), "RETURN");
    assert_eq!(Error::Stop("x".to_owned()).message(), "x");
    assert_eq!(
        Error::Guarded(Box::new(Error::Break)).message(),
        "Exception in GUARDed macro."
    );
    let guarded = Error::Guarded(Box::new(Error::empty_stack()));
    assert!(std::error::Error::source(&guarded).is_some());
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
fn a_line_failure_names_its_line_and_keeps_its_cause() {
    let line = Error::Line {
        line: 3,
        source: Box::new(Error::Break),
    };
    assert_eq!(line.message(), "Line #3");
    assert_eq!(line.to_string(), format!("Line #3 ({})", Error::Break));
    assert_eq!(line.cause(), Some(&Error::Break));
    assert_eq!(line.messages().len(), 2);
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
        Error::division_by_zero(),
    ] {
        assert!(err.is_runtime(), "{err}");
        assert!(!err.is_signal(), "{err}");
    }
    for err in [
        plain(),
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
        (plain(), "io.warp10.script.WarpScriptException"),
        (
            Error::with_cause("wrapped", plain()),
            "io.warp10.script.WarpScriptException",
        ),
        (
            Error::from(NotFinite::Nan),
            "io.warp10.script.WarpScriptException",
        ),
        (Error::division_by_zero(), "java.lang.ArithmeticException"),
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
            Error::Guarded(Box::new(Error::empty_stack())),
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
        (ValueKind::GeoShape, "com.geoxp.GeoXPLib$GeoXPShape"),
        (ValueKind::Key, "io.warp10.script.functions.ECPRIVATE$1"),
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
            NumberFormatError::InvalidRadixInput {
                input: "x".to_owned(),
                radix: 10,
            }
            .into(),
            "For input string: \"x\"",
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
            BoundsError::StringSlice {
                begin: 9,
                end: 8,
                length: 8,
            }
            .into(),
            "begin 9, end 8, length 8",
            "java.lang.StringIndexOutOfBoundsException",
        ),
        (
            IllegalArgumentError::InvalidCodePoint(0x11_0000).into(),
            "Not a valid Unicode code point: 0x110000",
            "java.lang.IllegalArgumentException",
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
            MathError::NotPositive { value: -1.5 },
            "-1.5 is smaller than the minimum (0)",
            format!("{EXCEPTION}NotPositiveException"),
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

#[test]
fn pickle_failures_have_the_wording_and_class_of_pyrolite() {
    use super::pickle::PickleError;
    use super::unpickle::PyKind;

    const PICKLE: &str = "net.razorvine.pickle.PickleException";
    let failures = [
        (
            PickleError::PrematureEof,
            "java.io.IOException: premature end of file",
            "java.io.IOException",
            false,
        ),
        (
            PickleError::ExpectedMoreBytes,
            "java.io.IOException: expected more bytes in input stream",
            "java.io.IOException",
            false,
        ),
        (
            PickleError::HashRecursion,
            "StackOverflowError",
            "java.lang.StackOverflowError",
            true,
        ),
        (
            PickleError::ConversionTooDeep,
            "StackOverflowError",
            "java.lang.StackOverflowError",
            true,
        ),
        (
            PickleError::SaveRecursion,
            "recursion too deep in Pickler.save (>1000)",
            "java.lang.StackOverflowError",
            true,
        ),
        (
            PickleError::PersistentIdNull,
            "Cannot invoke \"Object.toString()\" because the return value of \"net.razorvine.pickle.UnpickleStack.pop()\" is null",
            "java.lang.NullPointerException",
            true,
        ),
        (
            PickleError::NullArrayConstructor,
            "Cannot invoke \"net.razorvine.pickle.objects.ArrayConstructor.construct(char, int, byte[])\" because \"constructor\" is null",
            "java.lang.NullPointerException",
            true,
        ),
        (
            PickleError::SetState {
                target: PyKind::List,
                state: PyKind::Str,
            },
            "failed to __setstate__() (java.util.ArrayList.__setstate__(java.lang.String))",
            PICKLE,
            true,
        ),
        (
            PickleError::NoSuchConstructor {
                class: PyKind::Complex,
                arguments: vec![PyKind::Int, PyKind::Float],
            },
            "problem construction object: java.lang.NoSuchMethodException: net.razorvine.pickle.objects.ComplexNumber.<init>(java.lang.Integer, java.lang.Double)",
            PICKLE,
            true,
        ),
        (
            PickleError::ExceptionMessageNotString(PyKind::Bool),
            "problem construction object: java.lang.ClassCastException: class java.lang.Boolean cannot be cast to class java.lang.String",
            PICKLE,
            true,
        ),
        (
            PickleError::Reconstruct(PyKind::Dict),
            "failed to reconstruct() (java.util.HashMap.reconstruct(java.lang.Object, java.lang.Object))",
            PICKLE,
            true,
        ),
        (
            PickleError::Arity {
                what: "date",
                expected: "1 arg",
                got: 2,
            },
            "invalid pickle data for date; expected 1 arg, got 2",
            PICKLE,
            true,
        ),
        (
            PickleError::PackedLength {
                what: "time",
                expected: 6,
                got: 5,
            },
            "invalid pickle data for time; expected arg of length 6, got length 5",
            PICKLE,
            true,
        ),
        (
            PickleError::InvalidEscape {
                c: 'q',
                shown: "ab".to_owned(),
            },
            "invalid escape sequence char 'q' in string \"ab [...]\" (possibly truncated)",
            PICKLE,
            true,
        ),
        (
            PickleError::BeanIntrospection { expected: 2 },
            "couldn't introspect javabean: java.lang.IllegalArgumentException: wrong number of arguments: 0 expected: 2",
            PICKLE,
            true,
        ),
        (
            PickleError::Unpicklable(ValueKind::Matrix),
            "couldn't pickle object of type class org.apache.commons.math3.linear.Array2DRowRealMatrix",
            PICKLE,
            true,
        ),
    ];
    for (failure, message, class, runtime) in failures {
        let error = Error::from(failure);
        assert_eq!(error.to_string(), message);
        assert_eq!(exception_class(&error), class, "{message}");
        assert_eq!(error.is_runtime(), runtime, "{message}");
    }
}

#[test]
fn classes_of_every_kind_of_unpickled_object() {
    use super::unpickle::PyKind;

    for (kind, class) in [
        (PyKind::Bool, "java.lang.Boolean"),
        (PyKind::Int, "java.lang.Integer"),
        (PyKind::Long, "java.lang.Long"),
        (PyKind::BigInteger, "java.math.BigInteger"),
        (PyKind::Float, "java.lang.Double"),
        (PyKind::Str, "java.lang.String"),
        (PyKind::Bytes, "[B"),
        (PyKind::Tuple, "[Ljava.lang.Object;"),
        (PyKind::IntArray, "[I"),
        (PyKind::List, "java.util.ArrayList"),
        (PyKind::Dict, "java.util.HashMap"),
        (PyKind::ClassDict, "net.razorvine.pickle.objects.ClassDict"),
        (PyKind::Set, "java.util.HashSet"),
        (PyKind::Decimal, "java.math.BigDecimal"),
        (
            PyKind::Complex,
            "net.razorvine.pickle.objects.ComplexNumber",
        ),
        (PyKind::Calendar, "java.util.GregorianCalendar"),
        (PyKind::Time, "net.razorvine.pickle.objects.Time"),
        (PyKind::TimeDelta, "net.razorvine.pickle.objects.TimeDelta"),
        (PyKind::Zone, "sun.util.calendar.ZoneInfo"),
        (PyKind::Tzinfo, "net.razorvine.pickle.objects.Tzinfo"),
        (
            PyKind::ClassDictConstructor,
            "net.razorvine.pickle.objects.ClassDictConstructor",
        ),
        (
            PyKind::ExceptionConstructor,
            "net.razorvine.pickle.objects.ExceptionConstructor",
        ),
        (
            PyKind::AnyClassConstructor,
            "net.razorvine.pickle.objects.AnyClassConstructor",
        ),
        (
            PyKind::ArrayConstructor,
            "net.razorvine.pickle.objects.ArrayConstructor",
        ),
        (
            PyKind::ByteArrayConstructor,
            "net.razorvine.pickle.objects.ByteArrayConstructor",
        ),
        (
            PyKind::SetConstructor,
            "net.razorvine.pickle.objects.SetConstructor",
        ),
        (
            PyKind::DateTimeConstructor,
            "net.razorvine.pickle.objects.DateTimeConstructor",
        ),
        (
            PyKind::TimeZoneConstructor,
            "net.razorvine.pickle.objects.TimeZoneConstructor",
        ),
        (
            PyKind::Reconstructor,
            "net.razorvine.pickle.objects.Reconstructor",
        ),
        (
            PyKind::AttrGetter,
            "net.razorvine.pickle.objects.OperatorAttrGetterForCalendarTz",
        ),
        (
            PyKind::AttrGetterForTz,
            "net.razorvine.pickle.objects.OperatorAttrGetterForCalendarTz$AttrGetterForTz",
        ),
        (
            PyKind::CalendarLocalizer,
            "net.razorvine.pickle.objects.OperatorAttrGetterForCalendarTz$CalendarLocalizer",
        ),
        (PyKind::Exception, "net.razorvine.pickle.PythonException"),
        (PyKind::Object, "java.lang.Object"),
    ] {
        assert_eq!(compat::py_class(kind), class);
        assert_eq!(compat::class_name(Class::Pickled(kind)), class);
    }
    assert_eq!(compat::class_name(Class::Map), "java.util.Map");
    assert_eq!(compat::class_name(Class::HashMap), "java.util.HashMap");
    assert_eq!(compat::class_name(Class::TimeZone), "java.util.TimeZone");
    assert_eq!(
        compat::class_name(Class::ObjectConstructor),
        "net.razorvine.pickle.IObjectConstructor"
    );
    assert_eq!(
        compat::class_name(Class::ArrayConstructor),
        "net.razorvine.pickle.objects.ArrayConstructor"
    );
    assert_eq!(compat::py_classes(&[]), "");
}

#[test]
fn interpreter_failures_have_the_wording_of_warp10() {
    use super::error::stack::StackError;

    let failures = [
        (
            StackError::DepthLimit { max: 4 },
            "Stack depth would exceed set limit of 4. Consider raising the limit or using capabilities.",
        ),
        (StackError::IndexOutOfBound, "Index out of bound."),
        (StackError::InvalidLevel, "Invalid level."),
        (
            StackError::SymbolLimit { max: 3 },
            "Symbol table has reached its maximum number of entries: 3",
        ),
        (
            StackError::InvalidRegister { max: 255 },
            "Invalid register number, must be between 0 and 255",
        ),
        (
            StackError::OperationLimit { count: 11, max: 10 },
            "Operation count (11) exceeded maximum of 10. Consider raising the limit or using capabilities.",
        ),
        (
            StackError::RecursionLimit { level: 5 },
            "Maximum recursion level reached (5)",
        ),
        (StackError::Deadline, "Execution time limit reached."),
        (StackError::UnbalancedMultiline, "Unbalanced <' construct."),
        (StackError::UnbalancedComment, "Unbalanced /* construct."),
        (
            StackError::UnbalancedSecureScript,
            "Unbalanced <S construct.",
        ),
        (StackError::UnbalancedMacro, "Unbalanced <% construct."),
        (StackError::NotInComment, "Not inside a comment."),
        (StackError::UnterminatedString, "Cannot find end of string"),
        (
            StackError::MultilineNotAlone,
            "Can only start multiline strings by using <' on a line by itself.",
        ),
        (
            StackError::NotInSecureScript,
            "Not inside a secure script definition.",
        ),
        (
            StackError::AlreadyInSecureScript,
            "Already inside a secure script definition.",
        ),
        (StackError::NotInMacro, "Not inside a macro definition."),
        (
            StackError::EarlyBindingInMacro,
            "Early binding is not possible inside a macro.",
        ),
        (
            StackError::UnknownSymbol("s".to_owned()),
            "Unknown symbol 's'",
        ),
        (
            StackError::RedefinedFunction("f".to_owned()),
            "Disallowed redefined function 'f'.",
        ),
        (
            StackError::UnknownFunction("f".to_owned()),
            "Unknown function 'f'",
        ),
        (StackError::NotAMacro("m".to_owned()), "'m' is not a macro."),
        (
            StackError::UnknownMacro("m".to_owned()),
            "Unknown macro 'm'",
        ),
        (StackError::AlreadyForcedMacro, "Already in a forced Macro."),
        (
            StackError::ForcedMacroLevel,
            "Invalid level for closing forced Macro, check that all macros are correctly closed.",
        ),
    ];
    for (failure, message) in failures {
        let error = Error::from(failure);
        assert_eq!(error.to_string(), message);
        assert_eq!(
            exception_class(&error),
            "io.warp10.script.WarpScriptException"
        );
        assert!(!error.is_runtime(), "{message}");
    }
    let usage = Error::usage("ABS", "expects a number.");
    assert_eq!(usage.to_string(), "ABS expects a number.");
    assert_eq!(
        exception_class(&usage),
        "io.warp10.script.WarpScriptException"
    );
    assert!(!usage.is_runtime());
}

#[test]
fn time_failures_have_the_wording_of_warp10() {
    use super::error::time::TimeError;

    let failures = [
        (
            TimeError::UnknownZone("Mars/Base".to_owned()),
            "The datetime zone id 'Mars/Base' is not recognised",
        ),
        (
            TimeError::ZoneGap {
                local: "2024-03-31T02:30:00.000".to_owned(),
                zone: "Europe/Paris".to_owned(),
            },
            "Illegal instant due to time zone offset transition (daylight savings time 'gap'): 2024-03-31T02:30:00.000 (Europe/Paris)",
        ),
        (
            TimeError::MalformedOffset {
                id: "+2:00".to_owned(),
                rest: "2:00".to_owned(),
            },
            "Invalid format: \"+2:00\" is malformed at \"2:00\"",
        ),
        (
            TimeError::InstantOutOfRange(i64::MAX),
            "The instant 9223372036854775807 is out of range.",
        ),
        (
            TimeError::FieldOutOfRange {
                value: 13,
                field: "monthOfYear".to_owned(),
                min: 1,
                max: 12,
                suffix: String::new(),
            },
            "Value 13 for monthOfYear must be in the range [1,12]",
        ),
        (
            TimeError::CalculationOverflow,
            "The calculation caused an overflow.",
        ),
        (TimeError::InvalidIsoDuration, "invalid ISO8601 duration."),
        (TimeError::InvalidDuration, "Invalid ISO8601 duration"),
        (
            TimeError::AmbiguousDuration,
            "no support for ambiguous durations containing years or months, please convert those to days.",
        ),
        (
            TimeError::UnparsableDuration("P1".to_owned()),
            "Parsing of duration without sub second precision has failed. Tried to parse: P1",
        ),
        (
            TimeError::Unparsable {
                text: "x".to_owned(),
                index: 0,
            },
            "Text 'x' could not be parsed at index 0",
        ),
        (
            TimeError::UnparsableBecause {
                text: "x".to_owned(),
                detail: "why".to_owned(),
            },
            "Text 'x' could not be parsed: why",
        ),
        (
            TimeError::UnparsedText {
                text: "x".to_owned(),
                index: 1,
            },
            "Text 'x' could not be parsed, unparsed text found at index 1",
        ),
        (
            TimeError::WrongTime {
                before: true,
                instant: "2020".to_owned(),
            },
            "Current time is before '2020'",
        ),
        (
            TimeError::WrongTime {
                before: false,
                instant: "2020".to_owned(),
            },
            "Current time is after '2020'",
        ),
        (
            TimeError::TimescaleOnBucketized,
            "Cannot apply timescale on a bucketized GTS. Unbucketize it first.",
        ),
        (
            TimeError::OverlapExceedsWidth,
            "Overlap cannot exceed chunk width.",
        ),
        (
            TimeError::ChunkLabelTaken("chunk".to_owned()),
            "Cannot operate on Geo Time Series which already have a label named 'chunk'",
        ),
        (
            TimeError::EncoderChunkLabelTaken("chunk".to_owned()),
            "Cannot operate on encoders which already have a label named 'chunk'",
        ),
        (
            TimeError::BucketspanExceedsChunk,
            "Cannot operate on Geo Time Series with a bucketspan greater than the chunk width.",
        ),
        (
            TimeError::NoTicks,
            "Cannot load from long array because \"cropped.ticks\" is null",
        ),
        (
            TimeError::NotSorted("a{}".to_owned()),
            "GTS a{} is not sorted.",
        ),
        (
            TimeError::WrongOrder("a{}".to_owned()),
            "GTS a{} is not sorted in the expected order.",
        ),
        (
            TimeError::EncoderWrongOrder("a{}".to_owned()),
            "ENCODER a{} is not sorted in expected order.",
        ),
        (
            TimeError::NotSeriesOrEncoders,
            "expects a list of Geo Time Series or encoders or of lists thereof.",
        ),
        (
            TimeError::InconsistentChunkTypes,
            "Inconsistent types for chunks to fuse.",
        ),
        (
            TimeError::NegativePeriod {
                origin: 5,
                times: 2,
                period: "P1D".to_owned(),
                offset: 3,
            },
            "Period is negative from timestamp 5. Can not add a negative period. Period is 2 times P1D plus 3 time unit(s).",
        ),
        (
            TimeError::CalendarBucketLimit { count: 9, max: 8 },
            "Bucket count (9) is exceeding maximum value of 8. Consider raising the limit or using capabilities.",
        ),
    ];
    for (failure, message) in failures {
        let error = Error::from(failure);
        assert_eq!(error.to_string(), message);
        assert_eq!(
            exception_class(&error),
            "io.warp10.script.WarpScriptException"
        );
        assert!(!error.is_runtime(), "{message}");
    }
}

#[test]
fn text_failures_have_the_wording_of_warp10() {
    use super::error::text::{RegexFailure, TextError};

    let regex = fancy_regex::Regex::new("(").expect_err("an unclosed group is a syntax error");
    let failures = [
        (
            TextError::SliceReversed { start: 5, end: 2 },
            "5 > 2".to_owned(),
        ),
        (
            TextError::Base64Length(5),
            "Invalid input length 5".to_owned(),
        ),
        (
            TextError::Base64Character('!'),
            "Unrecognized character: !".to_owned(),
        ),
        (
            TextError::Base64CharacterCode(0xe9),
            "Unrecognized character: 0xe9".to_owned(),
        ),
        (
            TextError::InvalidOpb64Character { c: '#', offset: 3 },
            "Invalid OPB64 character '#' at offset 3.".to_owned(),
        ),
        (
            TextError::OddHexDigits,
            "Odd number of hexadecimal digits.".to_owned(),
        ),
        (
            TextError::InvalidHexCharacter,
            "Invalid hex character.".to_owned(),
        ),
        (
            TextError::UnsupportedCharset("KLINGON".to_owned()),
            "KLINGON".to_owned(),
        ),
        (
            TextError::MissingEscaped,
            "character to be escaped is missing".to_owned(),
        ),
        (
            TextError::MissingGroupIndex,
            "Illegal group reference: group index is missing".to_owned(),
        ),
        (
            TextError::EmptyGroupName,
            "named capturing group has 0 length name".to_owned(),
        ),
        (
            TextError::UnclosedGroupName,
            "named capturing group is missing trailing '}'".to_owned(),
        ),
        (
            TextError::DigitGroupName("1a".to_owned()),
            "capturing group name {1a} starts with digit character".to_owned(),
        ),
        (
            TextError::NoGroupNamed("x".to_owned()),
            "No group with name {x}".to_owned(),
        ),
        (
            TextError::IllegalGroupReference,
            "Illegal group reference".to_owned(),
        ),
        (TextError::NoGroup(3), "No group 3".to_owned()),
        (
            TextError::LongElementExpected("STRING".to_owned()),
            "Invalid element type STRING, only LONG elements are supported.".to_owned(),
        ),
        (
            TextError::RlpBufferUnderflow,
            "Buffer underflow.".to_owned(),
        ),
        (
            TextError::RlpItemType,
            "Invalid item type, only LIST, LONG, BYTES and STRING are supported.".to_owned(),
        ),
        (
            TextError::RlpSizeTooLarge,
            "Unable to support size over 2**32.".to_owned(),
        ),
        (
            TextError::Regex(RegexFailure::from(regex.clone())),
            regex.to_string(),
        ),
    ];
    for (failure, message) in failures {
        // `Matcher.appendReplacement` raises runtime exceptions.
        let (class, runtime) = match &failure {
            TextError::NoGroup(_) => ("java.lang.IndexOutOfBoundsException", true),
            TextError::MissingEscaped
            | TextError::MissingGroupIndex
            | TextError::EmptyGroupName
            | TextError::UnclosedGroupName
            | TextError::DigitGroupName(_)
            | TextError::NoGroupNamed(_)
            | TextError::IllegalGroupReference => ("java.lang.IllegalArgumentException", true),
            _ => ("io.warp10.script.WarpScriptException", false),
        };
        let error = Error::from(failure);
        assert_eq!(error.to_string(), message);
        assert_eq!(exception_class(&error), class, "{message}");
        assert_eq!(error.is_runtime(), runtime, "{message}");
    }
    assert_eq!(
        RegexFailure::from(regex.clone()),
        RegexFailure::from(regex.clone())
    );
    let other = fancy_regex::Regex::new("[").expect_err("an unclosed class is a syntax error");
    assert_ne!(RegexFailure::from(regex.clone()), RegexFailure::from(other));
    assert!(std::error::Error::source(&RegexFailure::from(regex)).is_none());
    let base64 = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, "a")
        .expect_err("one character is not base64");
    let error = Error::from(TextError::from(base64.clone()));
    assert_eq!(error.to_string(), base64.to_string());
}

#[test]
fn format_failures_have_the_wording_of_the_jdk() {
    use super::error::format::FormatError;

    let failures = [
        (FormatError::DuplicateFlag('-'), "Flags = '-'"),
        (
            FormatError::FlagMismatch {
                conversion: 's',
                flag: '#',
            },
            "Conversion = s, Flags = #",
        ),
        (FormatError::IllegalFlags("+ ".to_owned()), "Flags = '+ '"),
        (FormatError::MissingWidth("%-d".to_owned()), "%-d"),
        (FormatError::IllegalPrecision(2), "2"),
        (FormatError::IllegalWidth(5), "5"),
        (
            FormatError::UnknownDateTimeConversion('q'),
            "Conversion = 'tq'",
        ),
        (FormatError::UnknownConversion('y'), "Conversion = 'y'"),
        (
            FormatError::ZeroArgumentIndex,
            "Illegal format argument index = 0",
        ),
        (
            FormatError::MissingArgument("%s".to_owned()),
            "Format specifier '%s'",
        ),
        (
            FormatError::DateTimeUnsupported,
            "Date/time conversions (%t) are not supported.",
        ),
        (
            FormatError::ConversionMismatch {
                conversion: 'd',
                found: ValueKind::Double,
            },
            "d != java.lang.Double",
        ),
    ];
    for (failure, message) in failures {
        let error = Error::from(failure);
        assert_eq!(error.to_string(), message);
        assert_eq!(
            exception_class(&error),
            "io.warp10.script.WarpScriptException"
        );
        assert!(!error.is_runtime(), "{message}");
    }
}

#[test]
fn parse_and_wrapped_failures() {
    let parse = Error::parse("tok", 3);
    assert_eq!(parse.to_string(), "tok");
    assert_eq!(exception_class(&parse), "java.text.ParseException");
    assert!(!parse.is_runtime());
    let wrapped = Error::wrapped(parse.clone());
    assert_eq!(wrapped.to_string(), "java.text.ParseException: tok (tok)");
    assert_eq!(wrapped.message(), "java.text.ParseException: tok");
    assert_eq!(
        exception_class(&wrapped),
        "io.warp10.script.WarpScriptException"
    );
    assert!(!wrapped.is_runtime());
    assert_eq!(wrapped.cause(), Some(&parse));
    assert_eq!(wrapped.root_cause(), &parse);
    assert!(wrapped.source().is_some());
}

#[test]
fn series_failures_have_the_wording_of_warp10() {
    use super::error::series::SeriesError;

    let failures: Vec<(SeriesError, &str)> = vec![
        (
            SeriesError::FillArguments,
            "FILL's argument types do not match the expected ones.",
        ),
        (
            SeriesError::FillLastParameter,
            "FILLexpects the last parameter of the input LIST to be a LIST",
        ),
        (
            SeriesError::FillTicks {
                message: "FILL expects a LIST of LONG, but instead the list contains a",
                found: "STRING",
            },
            "FILL expects a LIST of LONG, but instead the list contains a STRING",
        ),
        (
            SeriesError::FillListElement {
                message: "FILL expects a LIST of GTS, but instead the list contains a",
                found: "LONG",
            },
            "FILL expects a LIST of GTS, but instead the list contains a LONG",
        ),
        (
            SeriesError::FillNotSeries("FILLexpects a GTS"),
            "FILLexpects a GTS",
        ),
        (
            SeriesError::FillDuplicateTimestamps,
            "Cannot fill Geo Time Series with duplicate timestamps.",
        ),
        (
            SeriesError::MacroLocation,
            "Macro MUST return a latitude and a longitude which are of type DOUBLE.",
        ),
        (
            SeriesError::MacroElevationNan,
            "Macro MUST return an elevations which is NaN if it is of type DOUBLE.",
        ),
        (
            SeriesError::MacroElevationType,
            "Macro MUST return an elevation which is either NaN or LONG.",
        ),
        (
            SeriesError::MacroTick,
            "Macro MUST return a tick of type LONG.",
        ),
        (
            SeriesError::BucketParameters,
            "One of bucketspan or bucketcount must be different from zero.",
        ),
        (
            SeriesError::BucketLimit { count: 9, max: 8 },
            "Bucket count (9) would exceed maximum value of 8. Consider raising the limit or using capabilities.",
        ),
        (
            SeriesError::UndefinedBucketSpan,
            "Undefined bucket span, check your GTS timestamps.",
        ),
        (SeriesError::ExpectedMapper, "Expected a mapper function."),
        (
            SeriesError::LowessNotNumeric,
            "Can only smooth numeric Geo Time Series.",
        ),
        (
            SeriesError::LowessBandwidth,
            "Bandwidth parameter must be greater than 0",
        ),
        (
            SeriesError::LowessRobustness,
            "Robustness parameter must be greater or equal to 0",
        ),
        (
            SeriesError::LowessDegreeTooSmall,
            "Degree of polynomial fit must be greater than 0",
        ),
        (
            SeriesError::LowessDegreeTooLarge,
            "Degree of polynomial fit should remain small (lower than 10)",
        ),
        (
            SeriesError::LowessMissingValues,
            "More than 500000 missing values",
        ),
        (
            SeriesError::DuplicateTicks,
            "Can't be applied on GTS with duplicate ticks",
        ),
        (SeriesError::ThresholdTooSmall, "Threshold MUST be >= 3."),
        (
            SeriesError::FactorOutOfRange("Alpha".to_owned()),
            "Alpha should be strictly between 0.0 and 1.0.",
        ),
        (
            SeriesError::ExponentialNotNumeric,
            "Can only perform exponential smoothing on numeric Geo Time Series.",
        ),
        (
            SeriesError::ExponentialTooShort,
            "Can only perform exponential smoothing on Geo Time Series containing at least two values.",
        ),
        (
            SeriesError::NullDeviation { median: true },
            "Median Absolute Deviation is null",
        ),
        (
            SeriesError::NullDeviation { median: false },
            "Standard Deviation is null",
        ),
        (
            SeriesError::QuantifyNotNumeric,
            "Can only quantify numeric Geo Time Series.",
        ),
        (
            SeriesError::SeparatorNotFound,
            "Separator not found, unable to isolate given events.",
        ),
        (
            SeriesError::ConditionalProbabilityTypes,
            "Can only compute conditional probabilities for String Geo Time Series.",
        ),
        (
            SeriesError::ComparisonOperands("op.eq".to_owned()),
            "op.eqcan only operate on two GTS with NUMBER or STRING values.",
        ),
        (
            SeriesError::DatapointListSize(7),
            "Expected a list of 1 to 5 elements, got 7",
        ),
        (
            SeriesError::FillerResult,
            "Expected a [ ts lat lon elev value ] list as result of filler.",
        ),
        (SeriesError::InvalidClassSelector, "Invalid class selector"),
        (
            SeriesError::InvalidLabelsSelector,
            "Invalid labels/attributes selector",
        ),
        (SeriesError::InvalidThresholdType, "Invalid threshold type."),
        (
            SeriesError::InterpolatorSizes("INTERPOLATOR.LINEAR".to_owned()),
            "INTERPOLATOR.LINEAR: incoherent argument sizes, both input LIST should have the same size.",
        ),
        (
            SeriesError::BicubicSizes {
                rows: 2,
                columns: 3,
            },
            "INTERPOLATOR.2D.BICUBIC: incoherent argument sizes. Up to vector sizes, values should be a LIST (size 2) of LIST (size 3).",
        ),
        (
            SeriesError::TricubicSizes { x: 2, y: 3, z: 4 },
            "INTERPOLATOR.3D.TRICUBIC: incoherent argument sizes. Up to vector sizes, values should be a LIST (size 2) of LIST (size 3) of LIST (size 4).",
        ),
        (
            SeriesError::NotUnitQuaternion(2.5),
            "Input does not appear to be a unit quaternion, norm was 2.5.",
        ),
        (
            SeriesError::PatternSeries,
            "Function can only be applied to numeric, bucketized, filled Geo Time Series.",
        ),
        (SeriesError::WordlenDivide, "Wordlen MUST divide windowlen."),
        (
            SeriesError::AlphabetPowerOfTwo,
            "Alphabet size MUST be a power of two.",
        ),
        (
            SeriesError::AlphabetRange(26),
            "Alphabet size MUST be a power of two between 2 and 2^26",
        ),
    ];
    for (failure, message) in failures {
        let error = Error::from(failure);
        assert_eq!(error.to_string(), message);
        assert_eq!(
            exception_class(&error),
            "io.warp10.script.WarpScriptException"
        );
        assert!(!error.is_runtime(), "{message}");
    }
}

#[test]
fn remaining_domain_failures_have_the_wording_of_warp10() {
    use super::error::aggregator::AggregatorError;
    use super::error::collections::CollectionError;
    use super::error::crypto::{CryptoError, SplitError};
    use super::error::fault::{BoundsError, IllegalArgumentError, ResourceError};
    use super::error::geo::GeoError;
    use super::error::platform::PlatformError;
    use super::error::series::SeriesError;
    use super::error::stack::StackError;
    use super::error::text::TextError;

    let failures: Vec<(Error, String)> = vec![
        (BoundsError::NegativeBitIndex(-1).into(), "bitIndex < 0: -1".to_owned()),
        (
            IllegalArgumentError::NegativeInitialSize(-5).into(),
            "Negative initial size: -5".to_owned(),
        ),
        (ResourceError::StackOverflow.into(), "StackOverflowError".to_owned()),
        (
            StackError::IndexAboveStack { index: 5, size: 3 }.into(),
            "Index out of bound, 5 >= 3".to_owned(),
        ),
        (
            StackError::IndexBelowStack { shifted: -7, size: 3 }.into(),
            "Index out of bound, -7 < -3".to_owned(),
        ),
        (
            StackError::NotNumeric.into(),
            "Unexpected type, expecting a numerical value.".to_owned(),
        ),
        (StackError::InvalidContext.into(), "Invalid stack context.".to_owned()),
        (
            StackError::NonExplicitSymbol.into(),
            "Encountered a LOAD/STORE or CSTORE operation with a non explicit symbol name.".to_owned(),
        ),
        (
            TextError::JsonLocated {
                message: "Unexpected end-of-input".to_owned(),
                location: "[line: 1, column: 2]".to_owned(),
            }
            .into(),
            "Unexpected end-of-input\n at [line: 1, column: 2]".to_owned(),
        ),
        (
            TextError::JsonNestingDepth { depth: 1001, max: 1000 }.into(),
            "Document nesting depth (1001) exceeds the maximum allowed (1000, from `StreamReadConstraints.getMaxNestingDepth()`)".to_owned(),
        ),
        (
            TextError::JsonWriteLimit(10).into(),
            "Cannot write, maximum number of characters written :10".to_owned(),
        ),
        (
            TextError::MustacheAt { message: "bad".to_owned(), line: 3 }.into(),
            "bad @[:3]".to_owned(),
        ),
        (
            TextError::InclusionDisabled.into(),
            "Template inclusion is disabled.".to_owned(),
        ),
        (
            TextError::IllegalExtendCode("PartialCode").into(),
            "Illegal code in extend section: com.github.mustachejava.codes.PartialCode".to_owned(),
        ),
        (
            TextError::TemplateSubstitutionType.into(),
            "Invalid type for template substitution.".to_owned(),
        ),
        (
            SeriesError::NumericalOperands { function: "ADDEXACT".to_owned(), reduces: false }.into(),
            "ADDEXACT can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values.".to_owned(),
        ),
        (
            SeriesError::NumericalOperands { function: "MAX".to_owned(), reduces: true }.into(),
            "MAX can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values, or a list of numerical values.".to_owned(),
        ),
        (SeriesError::InvalidSelector.into(), "invalid syntax for selector.".to_owned()),
        (
            SeriesError::InvalidSelectorSpaced.into(),
            " invalid syntax for selector.".to_owned(),
        ),
        (
            CollectionError::MapExpected.into(),
            "Invalid type, expected a map.".to_owned(),
        ),
        (
            CollectionError::ListKeyNotLong.into(),
            "Getting on LIST requires a LONG.".to_owned(),
        ),
        (
            CollectionError::StringKeyNotLong.into(),
            "Getting on STRING requires a LONG.".to_owned(),
        ),
        (
            CollectionError::BytesKeyNotLong.into(),
            "Getting on BYTES requires a LONG.".to_owned(),
        ),
        (
            CollectionError::InvalidGetTarget.into(),
            "Invalid OBJECT to GET on.".to_owned(),
        ),
        (
            CollectionError::NestedPathMissing.into(),
            "Tried to get an element at a nested path that does not exist in the input list.".to_owned(),
        ),
        (
            CollectionError::OddMapElements.into(),
            "Odd number of elements, cannot create map.".to_owned(),
        ),
        (
            CollectionError::DuplicateMapKey("a".to_owned()).into(),
            "Duplicate map key 'a'".to_owned(),
        ),
        (PlatformError::InvalidToken.into(), "Invalid token.".to_owned()),
        (
            PlatformError::InvalidCapabilityToken.into(),
            "invalid token.".to_owned(),
        ),
        (
            PlatformError::CapabilityParse.into(),
            "Error parsing capability.".to_owned(),
        ),
        (
            PlatformError::DepthAboveMax.into(),
            "The stack depth is over the requested maximum depth.".to_owned(),
        ),
        (
            PlatformError::NoSecureKey.into(),
            "You need to set the secure key first.".to_owned(),
        ),
        (
            PlatformError::NoSecureScriptKey.into(),
            "Missing secure script encryption key.".to_owned(),
        ),
        (PlatformError::InvalidSecureKey.into(), "Invalid secure key.".to_owned()),
        (
            PlatformError::MacroConfigNotFound {
                function: "MACROCONFIG".to_owned(),
                key: "k".to_owned(),
                attempts: vec!["a".to_owned(), "b".to_owned()],
            }
            .into(),
            "MACROCONFIG macro configuration 'k' not found, need to set one of [ 'a' 'b' ].".to_owned(),
        ),
        (
            PlatformError::RecursiveStructure.into(),
            "Recursive data structures exceeded 16 levels.".to_owned(),
        ),
        (
            PlatformError::Msgfail("because".to_owned()).into(),
            "MSGFAIL because".to_owned(),
        ),
        (
            PlatformError::Entropy(getrandom::Error::UNSUPPORTED).into(),
            format!(
                "Unable to obtain random data from the operating system ({}).",
                getrandom::Error::UNSUPPORTED
            ),
        ),
        (
            GeoError::HexHhcodeTooLong.into(),
            "Hexadecimal HHCode string of must be of length <= 16".to_owned(),
        ),
        (
            GeoError::BytesHhcodeTooLong.into(),
            "Byte array HHCode must be of length <= 8".to_owned(),
        ),
        (
            GeoError::HhcodeType.into(),
            "HHCode must be represented as a long, a string or a byte array.".to_owned(),
        ),
        (CryptoError::NegativeOutputLength(-4).into(), "-4".to_owned()),
        (
            CryptoError::LanesTooFew { min: 1 }.into(),
            "lanes must be greater than 1".to_owned(),
        ),
        (
            CryptoError::LanesTooMany { max: 9 }.into(),
            "lanes must be less than 9".to_owned(),
        ),
        (
            CryptoError::MemoryTooSmall { min: 16 }.into(),
            "memory is less than: 16 expected 16".to_owned(),
        ),
        (
            CryptoError::IterationsTooFew { min: 1 }.into(),
            "iterations is less than: 1".to_owned(),
        ),
        (
            CryptoError::OutputTooShort { min: 4 }.into(),
            "output length less than 4".to_owned(),
        ),
        (
            CryptoError::InvalidBase58Character { character: "0".to_owned(), position: 2 }.into(),
            "Invalid input '0' at position 2.".to_owned(),
        ),
        (
            CryptoError::UnwrappedTooShort { nonce: 8, unwrapped: 3 }.into(),
            "8 > 3".to_owned(),
        ),
        (
            CryptoError::Split(SplitError::InvalidParameters { shares: 300, needed: 2 }).into(),
            "cannot split a secret in 300 shares of which 2 are needed: expected 2 <= needed <= shares <= 255"
                .to_owned(),
        ),
        (
            AggregatorError::InvalidParameter("mapper.add".to_owned()).into(),
            "Invalid parameter for mapper.add".to_owned(),
        ),
        (AggregatorError::InvalidValueType.into(), "Invalid value type.".to_owned()),
        (
            AggregatorError::LanguageTagExpected("mapper.tolower".to_owned()).into(),
            "Invalid parameter type for mapper.tolower, expects a language tag (a string) on top of the stack.".to_owned(),
        ),
        (
            AggregatorError::StringMix { verb: "add", kind: "longs" }.into(),
            "Cannot add strings and longs.".to_owned(),
        ),
        (
            AggregatorError::EvenNonNumericMedian.into(),
            "Unable to compute median on an even number of non numeric values.".to_owned(),
        ),
        (
            AggregatorError::InvalidPercentile("mapper.percentile".to_owned()).into(),
            "Invalid percentile for mapper.percentile, MUST be between 0 and 100.".to_owned(),
        ),
        (
            AggregatorError::NothingRetained(3).into(),
            "Index -1 out of bounds for length 3".to_owned(),
        ),
        (
            AggregatorError::NotASeries("mapper.x".to_owned()).into(),
            "mapper.x's resulting function can only be applied on a GTS.".to_owned(),
        ),
        (
            AggregatorError::MaskOperands.into(),
            "op.mask can only be applied to pairs of Geo Time Series, the first one being boolean.".to_owned(),
        ),
    ];
    for (error, message) in failures {
        assert_eq!(error.to_string(), message);
    }
    let io = Error::from(IoFailure::from(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "pipe closed",
    )));
    assert_eq!(io.to_string(), "pipe closed");
    assert_eq!(exception_class(&io), "java.io.IOException");
    assert!(!io.is_runtime());
    let not_numeric = Error::from(AggregatorError::NotNumeric("mapper.median".to_owned()));
    assert_eq!(
        not_numeric.to_string(),
        "mapper.median can only operate on numeric Geo Time Series."
    );
    assert_eq!(exception_class(&not_numeric), "java.lang.RuntimeException");
    assert!(not_numeric.is_runtime());
    assert!(!Error::from(AggregatorError::InvalidValueType).is_runtime());
    assert_eq!(
        exception_class(&ResourceError::StackOverflow.into()),
        "java.lang.StackOverflowError"
    );
    assert_eq!(
        exception_class(&BoundsError::NegativeBitIndex(-1).into()),
        "java.lang.IndexOutOfBoundsException"
    );
}

#[test]
fn geometry_and_geojson_failures_have_the_classes_of_warp10() {
    use super::error::geojson::{Chain, Found, GeoJsonError, Step, StringTarget};
    use super::error::geometry::{GeometryError, NumberError};

    let geometry = [
        (
            GeometryError::UnknownType {
                name: "POINTS".to_owned(),
                line: 1,
            },
            "com.vividsolutions.jts.io.ParseException",
            false,
        ),
        (
            GeometryError::Expected {
                expected: "number",
                found: "';'".to_owned(),
                line: 1,
            },
            "com.vividsolutions.jts.io.ParseException",
            false,
        ),
        (
            GeometryError::InvalidNumber {
                text: "x".to_owned(),
                line: 1,
                source: NumberError::NotDecimal,
            },
            "com.vividsolutions.jts.io.ParseException",
            false,
        ),
        (
            GeometryError::UnknownWkbType(8),
            "com.vividsolutions.jts.io.ParseException",
            false,
        ),
        (
            GeometryError::WkbMemberType("MultiPoint"),
            "com.vividsolutions.jts.io.ParseException",
            false,
        ),
        (
            GeometryError::LineStringTooShort,
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            GeometryError::RingNotClosed,
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            GeometryError::RingTooShort(2),
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            GeometryError::HolesWithoutShell,
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            GeometryError::NullElement,
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            GeometryError::CollectionArgument,
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            GeometryError::EmptyPointWkb,
            "java.lang.IllegalArgumentException",
            true,
        ),
        (
            GeometryError::UnknownGeometryType,
            "com.vividsolutions.jts.util.AssertionFailedException",
            true,
        ),
        (
            GeometryError::Unsupported,
            "java.lang.UnsupportedOperationException",
            true,
        ),
        (
            GeometryError::UnknownFormat,
            "io.warp10.script.WarpScriptException",
            false,
        ),
    ];
    for (err, class, runtime) in geometry {
        assert_eq!(err.is_runtime(), runtime, "{err}");
        assert_eq!(Error::from(err.clone()).is_runtime(), runtime, "{err}");
        assert_eq!(exception_class(&Error::from(err.clone())), class, "{err}");
    }

    let text = |end_of_input| GeoJsonError::Syntax {
        message: "Unexpected end-of-input".to_owned(),
        end_of_input,
    };
    assert_eq!(
        exception_class(&text(true).into()),
        "com.fasterxml.jackson.core.io.JsonEOFException"
    );
    assert_eq!(
        exception_class(&text(false).into()),
        "com.fasterxml.jackson.core.JsonParseException"
    );
    assert_eq!(
        Found::Null.to_string(),
        "Null value (token `JsonToken.VALUE_NULL`)"
    );
    assert_eq!(
        Found::String.to_string(),
        "String value (token `JsonToken.VALUE_STRING`)"
    );
    let chain = Chain(vec![Step::Class("Feature"), Step::Class("Point")]);
    assert_eq!(
        chain.through(None),
        " (through reference chain: org.wololo.geojson.Feature->org.wololo.geojson.Point)"
    );
    let geometries = GeoJsonError::NoStringConstructor {
        target: StringTarget::Geometries,
        text: "x".to_owned(),
        chain: Chain::default(),
    };
    assert_eq!(
        geometries.to_string(),
        "Cannot construct instance of `org.wololo.geojson.Geometry[]`: no String-argument \
         constructor/factory method to deserialize from String value ('x')  at [Source: \
         UNKNOWN; byte offset: #UNKNOWN]"
    );
}
