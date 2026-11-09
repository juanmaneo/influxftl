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

pub mod compat;

use std::sync::Arc;
use std::{fmt, io};

use crate::encoder::EncoderError;
use crate::finite::NotFinite;
use crate::thrift::ThriftError;
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
}

impl From<ValueKind> for Class {
    fn from(kind: ValueKind) -> Self {
        Class::Value(kind)
    }
}

/// A WarpScript error or control-flow signal.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
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
    /// A plain `WarpScriptATCException` (`GUARD` failures): propagates unwrapped like the
    /// other signals but is a failure at the top level.
    #[error("{0}")]
    Abort(String),
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
    /// An access to `index` in an array of `length` elements.
    #[error("Index {index} out of bounds for length {length}")]
    IndexOutOfBounds {
        /// The index accessed.
        index: i64,
        /// The number of elements.
        length: usize,
    },
    /// A dereference of a missing value. Warp 10 renders its message-less exception as the
    /// simple name of its class.
    #[error("NullPointerException")]
    NullPointer,
    /// A number which is not finite entered where only finite numbers are accepted.
    #[error(transparent)]
    NotFinite(#[from] NotFinite),
    /// A failure of the binary encoding of datapoints.
    #[error(transparent)]
    Encoder(#[from] EncoderError),
    /// A malformed Thrift payload, or a defect of the reader.
    #[error(transparent)]
    Thrift(#[from] ThriftError),
    /// A failure which has no variant of its own yet: its Warp 10 wording, and the class of
    /// the Java `RuntimeException` it stands for when Warp 10 lets one escape rather than
    /// raising a `WarpScriptException` (the functions that catch and wrap those let them
    /// through unwrapped). Being replaced, module by module, by typed variants.
    #[error("{message}")]
    Legacy {
        /// The message, worded like Warp 10's.
        message: String,
        /// The class of the runtime exception, if it is one.
        runtime: Option<&'static str>,
    },
}

impl Error {
    /// A failure with `message` and no variant of its own yet.
    pub fn new(message: impl Into<String>) -> Self {
        Error::Legacy {
            message: message.into(),
            runtime: None,
        }
    }

    /// A failure with `message` caused by `cause`.
    pub fn with_cause(message: impl Into<String>, cause: Error) -> Self {
        Error::Context {
            context: message.into(),
            source: Box::new(cause),
        }
    }

    /// A runtime failure of Java class `class` with `message`, with no variant of its own yet.
    pub fn runtime(class: &'static str, message: impl Into<String>) -> Self {
        Error::Legacy {
            message: message.into(),
            runtime: Some(class),
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

    /// The failure of reading `index` in an array of `length`.
    pub fn index_out_of_bounds(index: i64, length: usize) -> Self {
        Error::IndexOutOfBounds { index, length }
    }

    /// The failure of dereferencing a missing value.
    pub fn null_pointer() -> Self {
        Error::NullPointer
    }

    /// `true` for the "abort the call" signals that macro execution lets through unwrapped.
    pub fn is_signal(&self) -> bool {
        matches!(
            self,
            Error::Return | Error::Stop(_) | Error::Break | Error::Continue | Error::Abort(_)
        )
    }

    /// `true` for the failures that are defects of the code rather than of the script (Warp 10
    /// raises them as runtime exceptions): the functions that wrap script failures let them
    /// through unwrapped.
    pub fn is_runtime(&self) -> bool {
        match self {
            Error::Legacy { runtime, .. } => runtime.is_some(),
            Error::Thrift(err) => !err.is_malformed_input(),
            Error::Encoder(err) => err.is_runtime(),
            Error::EmptyStack
            | Error::ClassCast { .. }
            | Error::IndexOutOfBounds { .. }
            | Error::NullPointer => true,
            Error::Return
            | Error::Stop(_)
            | Error::Break
            | Error::Continue
            | Error::Abort(_)
            | Error::Context { .. }
            | Error::NotFinite(_) => false,
        }
    }

    /// The message of this failure alone, without its causes.
    pub fn message(&self) -> String {
        match self {
            Error::Context { context, .. } => context.clone(),
            other => other.to_string(),
        }
    }

    /// The failure this one reports with more context.
    pub fn cause(&self) -> Option<&Error> {
        match self {
            Error::Context { source, .. } => Some(source),
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
