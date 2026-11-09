// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Warp 10 compatibility of the errors: the Java exception class a script reads in the `type`
//! of `ERROR`. Nothing else in the crate names a Java class of an error.

use super::fault::BoundsError;
use super::math::MathError;
use super::{Class, Error};
use crate::encoder::EncoderError;
use crate::pickle::PickleError;
use crate::thrift::ThriftError;
use crate::unpickle::PyKind;
use crate::value::ValueKind;

/// `WarpScriptException`, the exception of every failure Warp 10 reports itself.
const WARP_SCRIPT_EXCEPTION: &str = "io.warp10.script.WarpScriptException";

/// The Java class of the exception `err` stands for.
pub fn exception_class(err: &Error) -> &'static str {
    match err {
        Error::Legacy {
            runtime: Some(class),
            ..
        } => class,
        Error::Legacy { runtime: None, .. }
        | Error::Context { .. }
        | Error::Usage { .. }
        | Error::Stack(_)
        | Error::Time(_)
        | Error::Text(_)
        | Error::Format(_)
        | Error::Wrapped { .. }
        | Error::NotFinite(_) => WARP_SCRIPT_EXCEPTION,
        Error::Thrift(err) => thrift_class(err),
        Error::Encoder(err) => encoder_class(err),
        Error::Math(err) => math_class(err),
        Error::Pickle(err) => pickle_class(err),
        Error::Arithmetic(_) => "java.lang.ArithmeticException",
        Error::NumberFormat(_) => "java.lang.NumberFormatException",
        Error::Bounds(err) => bounds_class(err),
        Error::IllegalArgument(_) => "java.lang.IllegalArgumentException",
        Error::ResourceExhausted(_) => "java.lang.OutOfMemoryError",
        Error::Immutable => "java.lang.UnsupportedOperationException",
        Error::Parse { .. } => "java.text.ParseException",
        Error::Return => "io.warp10.script.WarpScriptReturnException",
        Error::Stop(_) => "io.warp10.script.WarpScriptStopException",
        Error::Break => "io.warp10.script.WarpScriptLoopBreakException",
        Error::Continue => "io.warp10.script.WarpScriptLoopContinueException",
        Error::Abort(_) => "io.warp10.script.WarpScriptATCException",
        Error::EmptyStack => "io.warp10.script.InformativeEmptyStackException",
        Error::ClassCast { .. } => "java.lang.ClassCastException",
        Error::NullPointer => "java.lang.NullPointerException",
    }
}

/// The class of the `libthrift` / JDK exception a reader failure stands for.
fn thrift_class(err: &ThriftError) -> &'static str {
    match err {
        ThriftError::EndOfInput { .. } | ThriftError::MessageTooLarge => {
            "org.apache.thrift.transport.TTransportException"
        }
        ThriftError::NegativeLength(_)
        | ThriftError::UnrecognizedType(_)
        | ThriftError::UnknownCompactType(_) => "org.apache.thrift.protocol.TProtocolException",
        ThriftError::VarintOverrun { .. } => "java.lang.ArrayIndexOutOfBoundsException",
        ThriftError::NegativeBinaryLength => "java.lang.IndexOutOfBoundsException",
        ThriftError::SkipTooDeep => "java.lang.StackOverflowError",
    }
}

/// The name of `class`, as a `ClassCastException` message gives it.
pub fn class_name(class: Class) -> &'static str {
    match class {
        Class::Value(kind) => value_class(kind),
        Class::Number => "java.lang.Number",
        Class::Comparable => "java.lang.Comparable",
        Class::List => "java.util.List",
        Class::Integer => "java.lang.Integer",
        Class::ObjectArray => "[Ljava.lang.Object;",
        Class::Map => "java.util.Map",
        Class::HashMap => "java.util.HashMap",
        Class::TimeZone => "java.util.TimeZone",
        Class::ObjectConstructor => "net.razorvine.pickle.IObjectConstructor",
        Class::ArrayConstructor => "net.razorvine.pickle.objects.ArrayConstructor",
        Class::Pickled(kind) => py_class(kind),
    }
}

/// `getClass().getName()` of a value of `kind`.
pub fn value_class(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Null => "null",
        ValueKind::Boolean => "java.lang.Boolean",
        ValueKind::Long => "java.lang.Long",
        ValueKind::Double => "java.lang.Double",
        ValueKind::BigDecimal => "java.math.BigDecimal",
        ValueKind::Matrix => "org.apache.commons.math3.linear.Array2DRowRealMatrix",
        ValueKind::Vector => "org.apache.commons.math3.linear.ArrayRealVector",
        ValueKind::String => "java.lang.String",
        ValueKind::Bytes => "[B",
        ValueKind::BitSet => "java.util.BitSet",
        ValueKind::List => "java.util.ArrayList",
        ValueKind::Map => "java.util.LinkedHashMap",
        ValueKind::Set => "java.util.HashSet",
        ValueKind::Macro => "io.warp10.script.WarpScriptStack$Macro",
        ValueKind::Function => "io.warp10.script.NamedWarpScriptFunction",
        ValueKind::Mark => "io.warp10.script.WarpScriptStack$Mark",
        ValueKind::Context => "io.warp10.script.MemoryWarpScriptStack$StackContext",
        ValueKind::Counter => "java.util.concurrent.atomic.AtomicLong",
        ValueKind::Gts => "io.warp10.continuum.gts.GeoTimeSerie",
        ValueKind::Encoder => "io.warp10.continuum.gts.GTSEncoder",
    }
}

/// The class of the exception a decoder failure stands for.
fn encoder_class(err: &EncoderError) -> &'static str {
    match err {
        EncoderError::UnsupportedValue(_)
        | EncoderError::InvalidStringLength
        | EncoderError::InvalidBoolean
        | EncoderError::InvalidCompressedContent(_) => "java.lang.RuntimeException",
        EncoderError::BufferUnderflow => "java.nio.BufferUnderflowException",
        EncoderError::VarintTooLong
        | EncoderError::PositionBeforeStart { .. }
        | EncoderError::PositionAfterLimit { .. } => "java.lang.IllegalArgumentException",
        EncoderError::NegativeStringLength(_) => "java.lang.NegativeArraySizeException",
        EncoderError::DecoderNotStarted => "java.io.IOException",
    }
}

/// The class of the Commons Math exception a failure stands for.
fn math_class(err: &MathError) -> &'static str {
    match err {
        MathError::TooManyIterations { .. } => {
            "org.apache.commons.math3.exception.TooManyIterationsException"
        }
        MathError::QrDecomposition { .. } => {
            "org.apache.commons.math3.exception.ConvergenceException"
        }
        MathError::NotStrictlyPositive { .. } => {
            "org.apache.commons.math3.exception.NotStrictlyPositiveException"
        }
        MathError::NoData
        | MathError::EmptyPolynomialCoefficients
        | MathError::MatrixWithoutColumn => "org.apache.commons.math3.exception.NoDataException",
        MathError::OutOfRange { .. } => "org.apache.commons.math3.exception.OutOfRangeException",
        MathError::NoBracketing { .. } => {
            "org.apache.commons.math3.exception.NoBracketingException"
        }
        MathError::DimensionMismatch { .. } => {
            "org.apache.commons.math3.exception.DimensionMismatchException"
        }
        MathError::NonSquareMatrix { .. } => {
            "org.apache.commons.math3.linear.NonSquareMatrixException"
        }
        MathError::MatrixDimensionMismatch { .. } => {
            "org.apache.commons.math3.linear.MatrixDimensionMismatchException"
        }
        MathError::SingularMatrix => "org.apache.commons.math3.linear.SingularMatrixException",
        MathError::NonMonotonic { .. } => {
            "org.apache.commons.math3.exception.NonMonotonicSequenceException"
        }
        MathError::TooFewPoints(_)
        | MathError::TwoPointsRequired(_)
        | MathError::BandwidthTooSmall(_) => {
            "org.apache.commons.math3.exception.NumberIsTooSmallException"
        }
        MathError::NotFiniteNumber(_) => {
            "org.apache.commons.math3.exception.NotFiniteNumberException"
        }
    }
}

/// The class of the exception an index or size failure stands for.
fn bounds_class(err: &BoundsError) -> &'static str {
    match err {
        BoundsError::NegativeSize(_) => "java.lang.NegativeArraySizeException",
        BoundsError::ArrayIndex { .. } => "java.lang.ArrayIndexOutOfBoundsException",
        BoundsError::ListIndex { .. } | BoundsError::ListEnd(_) => {
            "java.lang.IndexOutOfBoundsException"
        }
        BoundsError::StringIndex { .. }
        | BoundsError::StringSlice { .. }
        | BoundsError::StringRange { .. } => "java.lang.StringIndexOutOfBoundsException",
    }
}

/// The class of the exception a pickle failure stands for.
fn pickle_class(err: &PickleError) -> &'static str {
    match err {
        PickleError::PrematureEof | PickleError::ExpectedMoreBytes => "java.io.IOException",
        PickleError::HashRecursion
        | PickleError::ConversionTooDeep
        | PickleError::SaveRecursion => "java.lang.StackOverflowError",
        PickleError::PersistentIdNull | PickleError::NullArrayConstructor => {
            "java.lang.NullPointerException"
        }
        _ => "net.razorvine.pickle.PickleException",
    }
}

/// The class of an object Pyrolite unpickled.
pub fn py_class(kind: PyKind) -> &'static str {
    match kind {
        PyKind::Bool => "java.lang.Boolean",
        PyKind::Int => "java.lang.Integer",
        PyKind::Long => "java.lang.Long",
        PyKind::BigInteger => "java.math.BigInteger",
        PyKind::Float => "java.lang.Double",
        PyKind::Str => "java.lang.String",
        PyKind::Bytes => "[B",
        PyKind::Tuple => "[Ljava.lang.Object;",
        PyKind::IntArray => "[I",
        PyKind::List => "java.util.ArrayList",
        PyKind::Dict => "java.util.HashMap",
        PyKind::ClassDict => "net.razorvine.pickle.objects.ClassDict",
        PyKind::Set => "java.util.HashSet",
        PyKind::Decimal => "java.math.BigDecimal",
        PyKind::Complex => "net.razorvine.pickle.objects.ComplexNumber",
        PyKind::Calendar => "java.util.GregorianCalendar",
        PyKind::Time => "net.razorvine.pickle.objects.Time",
        PyKind::TimeDelta => "net.razorvine.pickle.objects.TimeDelta",
        PyKind::Zone => "sun.util.calendar.ZoneInfo",
        PyKind::Tzinfo => "net.razorvine.pickle.objects.Tzinfo",
        PyKind::ClassDictConstructor => "net.razorvine.pickle.objects.ClassDictConstructor",
        PyKind::ExceptionConstructor => "net.razorvine.pickle.objects.ExceptionConstructor",
        PyKind::AnyClassConstructor => "net.razorvine.pickle.objects.AnyClassConstructor",
        PyKind::ArrayConstructor => "net.razorvine.pickle.objects.ArrayConstructor",
        PyKind::ByteArrayConstructor => "net.razorvine.pickle.objects.ByteArrayConstructor",
        PyKind::SetConstructor => "net.razorvine.pickle.objects.SetConstructor",
        PyKind::DateTimeConstructor => "net.razorvine.pickle.objects.DateTimeConstructor",
        PyKind::TimeZoneConstructor => "net.razorvine.pickle.objects.TimeZoneConstructor",
        PyKind::Reconstructor => "net.razorvine.pickle.objects.Reconstructor",
        PyKind::AttrGetter => "net.razorvine.pickle.objects.OperatorAttrGetterForCalendarTz",
        PyKind::AttrGetterForTz => {
            "net.razorvine.pickle.objects.OperatorAttrGetterForCalendarTz$AttrGetterForTz"
        }
        PyKind::CalendarLocalizer => {
            "net.razorvine.pickle.objects.OperatorAttrGetterForCalendarTz$CalendarLocalizer"
        }
        PyKind::Exception => "net.razorvine.pickle.PythonException",
        PyKind::Object => "java.lang.Object",
    }
}

/// The classes of `kinds`, comma separated like a Java signature.
pub fn py_classes(kinds: &[PyKind]) -> String {
    kinds
        .iter()
        .map(|kind| py_class(*kind))
        .collect::<Vec<_>>()
        .join(", ")
}
