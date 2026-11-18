// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Errors and control-flow signals of the WarpScript runtime.
//!
//! [`Error`] is a `thiserror` enum: every kind of failure is a variant and domain modules
//! contribute their own enums through `#[from]` (see `docs/RUST_RULES.md`). The `Display` of
//! a variant is the Warp 10 wording of the failure, because a script observes it (`TRY`,
//! `ERROR`, the HTTP exec endpoint): a variant displays itself only, and [`Error::Context`]
//! nests its cause the way Warp 10 renders an exception chain, `outer (cause (root))`.
//!
//! The Java exception class names a script can read live in [`compat`] and nowhere else.

pub mod aggregator;
pub mod collections;
pub mod compat;
pub mod crypto;
pub mod datastore;
pub mod fault;
pub mod format;
pub mod geo;
pub mod geojson;
pub mod geometry;
pub mod math;
pub mod pgp;
pub mod platform;
pub mod processing;
pub mod series;
pub mod stack;
pub mod text;
pub mod time;

use std::sync::Arc;
use std::{fmt, io};

use self::aggregator::AggregatorError;
use self::collections::CollectionError;
use self::crypto::CryptoError;
use self::datastore::{DatastoreError, TimeRangeError};
use self::fault::{
    ArithmeticError, BoundsError, IllegalArgumentError, NumberFormatError, ResourceError,
};
use self::format::FormatError;
use self::geo::GeoError;
use self::geojson::GeoJsonError;
use self::geometry::GeometryError;
use self::math::MathError;
use self::pgp::PgpError;
use self::platform::PlatformError;
use self::processing::ProcessingError;
use self::series::SeriesError;
use self::stack::StackError;
use self::text::TextError;
use self::time::TimeError;
use crate::encoder::EncoderError;
use crate::finite::NotFinite;
use crate::pickle::PickleError;
use crate::thrift::ThriftError;
use crate::unpickle::PyKind;
use crate::value::ValueKind;

/// Tail of the message of a `BREAK` / `CONTINUE` that escaped every loop.
const LOOP_ONLY: &str = "must be used in a one of FOR, FOREACH, FORSTEP, WHILE, UNTIL loop.";

/// An I/O failure, shared and comparable so that [`Error`] stays `Clone + Eq`.
#[derive(Debug, Clone)]
pub struct IoFailure(Arc<io::Error>);

impl From<io::Error> for IoFailure {
    fn from(err: io::Error) -> Self {
        IoFailure(Arc::new(err))
    }
}

impl PartialEq for IoFailure {
    fn eq(&self, other: &Self) -> bool {
        self.0.kind() == other.0.kind() && self.0.to_string() == other.0.to_string()
    }
}

impl Eq for IoFailure {}

impl fmt::Display for IoFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for IoFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

/// A class a value was cast from or to when the cast failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// The class of a value of this kind.
    Value(ValueKind),
    /// Any number (`LONG`, `DOUBLE` or `BIGDECIMAL`).
    Number,
    /// Anything an ordering can compare.
    Comparable,
    /// Any list (the interface, not the class of a `LIST` value).
    List,
    /// A 32-bit integer.
    Integer,
    /// An array of objects, the shape of a datapoint.
    ObjectArray,
    /// Any map (the interface).
    Map,
    /// A hash map.
    HashMap,
    /// A time zone.
    TimeZone,
    /// What Pyrolite builds an object with.
    ObjectConstructor,
    /// What Pyrolite builds an array with.
    ArrayConstructor,
    /// The class of an unpickled object.
    Pickled(PyKind),
    /// A Processing graphics context, as the type a cast asks for.
    Graphics,
    /// A Processing image, as the type a cast asks for.
    Image,
    /// A Processing shape, as the type a cast asks for.
    Shape,
    /// A Processing font, as the type a cast asks for.
    Font,
}

impl From<ValueKind> for Class {
    fn from(kind: ValueKind) -> Self {
        Class::Value(kind)
    }
}

/// A WarpScript error or control-flow signal.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    /// `RETURN` / `NRETURN`: leave the current macro(s).
    #[error("RETURN")]
    Return,
    /// `STOP` (or a stop signal): stop executing the script, the stack is kept as is.
    #[error("{0}")]
    Stop(String),
    /// `BREAK`: leave the innermost loop.
    #[error("Exception at BREAK: {}", LOOP_ONLY)]
    Break,
    /// `CONTINUE`: skip to the next iteration of the innermost loop.
    #[error("Exception at CONTINUE: {}", LOOP_ONLY)]
    Continue,
    /// A failure inside a `GUARD`ed macro. Warp 10 reports the same text whatever failed, and
    /// the failure stays available as the source. It propagates unwrapped like the other
    /// signals, and is a failure at the top level.
    #[error("Exception in GUARDed macro.")]
    Guarded(#[source] Box<Error>),
    /// A failure reported with more context by the code that handled it (the
    /// `Exception at '…' in section …` of the interpreter, `<FUNCTION> failed to …`).
    #[error("{context} ({source})")]
    Context {
        /// The message of the handler.
        context: String,
        /// The failure that was handled.
        #[source]
        source: Box<Error>,
    },
    /// A failure on the line `line` of a script (the `Line #N` of the interpreter).
    #[error("Line #{line} ({source})")]
    Line {
        /// The line, counted from 1.
        line: usize,
        /// The failure on the line.
        #[source]
        source: Box<Error>,
    },
    /// A `pop` or `peek` on an empty stack.
    #[error("Empty stack.")]
    EmptyStack,
    /// A value of a type other than the one a function operates on.
    #[error(
        "class {} cannot be cast to class {}",
        compat::class_name(*from),
        compat::class_name(*to)
    )]
    ClassCast {
        /// The class of the value found.
        from: Class,
        /// The class expected.
        to: Class,
    },
    /// A dereference of a missing value. Warp 10 renders its message-less exception as the
    /// simple name of its class.
    #[error("NullPointerException")]
    NullPointer,
    /// A number which is not finite entered where only finite numbers are accepted.
    #[error(transparent)]
    NotFinite(#[from] NotFinite),
    /// An arithmetic failure.
    #[error(transparent)]
    Arithmetic(#[from] ArithmeticError),
    /// A text that is not the number it must be.
    #[error(transparent)]
    NumberFormat(#[from] NumberFormatError),
    /// A size or an index outside of what it must be.
    #[error(transparent)]
    Bounds(#[from] BoundsError),
    /// An argument a platform routine refuses.
    #[error(transparent)]
    IllegalArgument(#[from] IllegalArgumentError),
    /// A limit of the machine a request exceeds.
    #[error(transparent)]
    ResourceExhausted(#[from] ResourceError),
    /// A modification of a collection that was frozen (`IMMUTABLE`).
    #[error("UnsupportedOperationException")]
    Immutable,
    /// A function given arguments it does not accept: `<function> <requirement>`, where the
    /// requirement is Warp 10's wording of what the function expects. Rust code tells these
    /// failures apart by function, never by the text of the requirement.
    #[error("{function} {requirement}")]
    Usage {
        /// The WarpScript function (or mapper, filter, aggregator...) that refused.
        function: String,
        /// What it expects.
        requirement: String,
    },
    /// A failure of a format specifier.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// A text that could not be parsed (`java.text.ParseException`): the message and the
    /// offset of the error.
    #[error("{message}")]
    Parse {
        /// The message.
        message: String,
        /// The offset of the error in the parsed text.
        offset: usize,
    },
    /// A failure wrapped by a `WarpScriptException` built from it alone (`new
    /// WarpScriptException(cause)`): its message is the `toString()` of the cause.
    #[error("{}: {} ({source})", compat::exception_class(source), source.message())]
    Wrapped {
        /// The failure wrapped.
        #[source]
        source: Box<Error>,
    },
    /// A failure of a text or binary function.
    #[error(transparent)]
    Text(#[from] TextError),
    /// A failure of an aggregator.
    #[error(transparent)]
    Aggregator(#[from] AggregatorError),
    /// A failure of a collection function.
    #[error(transparent)]
    Collection(#[from] CollectionError),
    /// A failure of a crypto function.
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    /// A failure of a geo function.
    #[error(transparent)]
    Geo(#[from] GeoError),
    /// A failure of a geometry reader, writer or predicate.
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    /// A failure of the GeoJSON reader.
    #[error(transparent)]
    GeoJson(#[from] GeoJsonError),
    /// A failure of a platform function.
    #[error(transparent)]
    Platform(#[from] PlatformError),
    /// A failure of the datastore.
    #[error(transparent)]
    Datastore(#[from] DatastoreError),
    /// A time range that cannot be computed.
    #[error(transparent)]
    TimeRange(#[from] TimeRangeError),
    /// An input or output failure.
    #[error(transparent)]
    Io(#[from] IoFailure),
    /// A failure of a Geo Time Series function.
    #[error(transparent)]
    Series(#[from] SeriesError),
    /// A failure of a time function.
    #[error(transparent)]
    Time(#[from] TimeError),
    /// A failure of the interpreter.
    #[error(transparent)]
    Stack(#[from] StackError),
    /// A failure of a Commons Math routine.
    #[error(transparent)]
    Math(#[from] MathError),
    /// A failure of a Processing function.
    #[error(transparent)]
    Processing(#[from] ProcessingError),
    /// A failure of an OpenPGP function or of the decoding under it.
    #[error(transparent)]
    Pgp(#[from] PgpError),
    /// A failure of the pickle encoder or decoder.
    #[error(transparent)]
    Pickle(#[from] PickleError),
    /// A failure of the binary encoding of datapoints.
    #[error(transparent)]
    Encoder(#[from] EncoderError),
    /// A malformed Thrift payload, or a defect of the reader.
    #[error(transparent)]
    Thrift(#[from] ThriftError),
}

impl Error {
    /// A failure with `message` caused by `cause`.
    pub fn with_cause(message: impl Into<String>, cause: Error) -> Self {
        Error::Context {
            context: message.into(),
            source: Box::new(cause),
        }
    }

    /// The failure of `function` given arguments it does not accept, because of `requirement`.
    pub fn usage(function: impl AsRef<str>, requirement: impl Into<String>) -> Self {
        Error::Usage {
            function: function.as_ref().to_owned(),
            requirement: requirement.into(),
        }
    }

    /// The failure of parsing a text, at `offset`.
    pub fn parse(message: impl Into<String>, offset: usize) -> Self {
        Error::Parse {
            message: message.into(),
            offset,
        }
    }

    /// `new WarpScriptException(cause)`: `cause` wrapped.
    pub fn wrapped(cause: impl Into<Error>) -> Self {
        Error::Wrapped {
            source: Box::new(cause.into()),
        }
    }

    /// The failure of a `pop` on an empty stack.
    pub fn empty_stack() -> Self {
        Error::EmptyStack
    }

    /// The failure of casting a `from` object to class `to`.
    pub fn class_cast(from: impl Into<Class>, to: impl Into<Class>) -> Self {
        Error::ClassCast {
            from: from.into(),
            to: to.into(),
        }
    }

    /// The failure of dereferencing a missing value.
    pub fn null_pointer() -> Self {
        Error::NullPointer
    }

    /// `true` for the "abort the call" signals that macro execution lets through unwrapped.
    pub fn is_signal(&self) -> bool {
        matches!(
            self,
            Error::Return | Error::Stop(_) | Error::Break | Error::Continue | Error::Guarded(_)
        )
    }

    /// `true` for the failures that are defects of the code rather than of the script (Warp 10
    /// raises them as runtime exceptions): the functions that wrap script failures let them
    /// through unwrapped.
    pub fn is_runtime(&self) -> bool {
        match self {
            Error::Aggregator(err) => matches!(err, AggregatorError::NotNumeric(_)),
            Error::Geo(err) => matches!(err, GeoError::InvalidGeoHash { .. }),
            Error::Geometry(err) => err.is_runtime(),
            Error::Thrift(err) => !err.is_malformed_input(),
            Error::Encoder(err) => err.is_runtime(),
            Error::Pickle(err) => err.is_runtime(),
            Error::Text(err) => err.is_runtime(),
            Error::Processing(err) => err.is_runtime(),
            Error::Math(_)
            | Error::Arithmetic(_)
            | Error::NumberFormat(_)
            | Error::Bounds(_)
            | Error::IllegalArgument(_)
            | Error::ResourceExhausted(_)
            | Error::Immutable
            | Error::EmptyStack
            | Error::ClassCast { .. }
            | Error::NullPointer => true,
            Error::Return
            | Error::Stop(_)
            | Error::Break
            | Error::Continue
            | Error::Guarded(_)
            | Error::Context { .. }
            | Error::Line { .. }
            | Error::Usage { .. }
            | Error::Stack(_)
            | Error::Time(_)
            | Error::Series(_)
            | Error::Collection(_)
            | Error::Crypto(_)
            | Error::Pgp(_)
            | Error::Platform(_)
            | Error::Datastore(_)
            | Error::TimeRange(_)
            | Error::Io(_)
            | Error::Format(_)
            | Error::Parse { .. }
            | Error::Wrapped { .. }
            | Error::GeoJson(_)
            | Error::NotFinite(_) => false,
        }
    }

    /// The message of this failure alone, without its causes.
    pub fn message(&self) -> String {
        match self {
            Error::Context { context, .. } => context.clone(),
            Error::Line { line, .. } => format!("Line #{line}"),
            Error::Wrapped { source } => {
                format!("{}: {}", compat::exception_class(source), source.message())
            }
            other => other.to_string(),
        }
    }

    /// The failure this one reports with more context.
    pub fn cause(&self) -> Option<&Error> {
        match self {
            Error::Context { source, .. }
            | Error::Line { source, .. }
            | Error::Wrapped { source } => Some(source),
            _ => None,
        }
    }

    /// The innermost cause of the chain (the original failure).
    pub fn root_cause(&self) -> &Error {
        let mut current = self;
        while let Some(cause) = current.cause() {
            current = cause;
        }
        current
    }

    /// Messages of the whole cause chain, outermost first.
    pub fn messages(&self) -> Vec<String> {
        std::iter::successors(Some(self), |err| err.cause())
            .map(Error::message)
            .collect()
    }
}

/// Convenience alias used throughout the runtime.
pub type Result<T> = std::result::Result<T, Error>;
