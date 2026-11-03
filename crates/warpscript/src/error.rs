// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Errors and control-flow signals of the WarpScript runtime.
//!
//! Warp 10 models everything as Java exceptions: `WarpScriptException` is the root, and the
//! "abort the call" (`WarpScriptATCException`) subclasses — return, stop, loop break / continue —
//! unwind through macro execution without being wrapped. [`Error`] mirrors that hierarchy.

use std::fmt;

/// Tail of the message of a `BREAK` / `CONTINUE` that escaped every loop.
const LOOP_ONLY: &str = "must be used in a one of FOR, FOREACH, FORSTEP, WHILE, UNTIL loop.";

/// A WarpScript error or control-flow signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A plain `WarpScriptException`: something went wrong.
    Script(ScriptError),
    /// `RETURN` / `NRETURN`: leave the current macro(s).
    Return,
    /// `STOP` (or a stop signal): stop executing the script, the stack is kept as is.
    Stop(String),
    /// `BREAK`: leave the innermost loop.
    Break,
    /// `CONTINUE`: skip to the next iteration of the innermost loop.
    Continue,
    /// A plain `WarpScriptATCException` (`GUARD` failures): propagates unwrapped like the
    /// other signals but is a failure at the top level.
    Abort(String),
}

/// The payload of a `WarpScriptException`: a message and an optional cause chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptError {
    /// Human readable message, worded like Warp 10's.
    pub message: String,
    /// The error that caused this one (Java `getCause()`).
    pub cause: Option<Box<Error>>,
    /// The class of the Java `RuntimeException` this error stands for (`ClassCastException`,
    /// `ArrayIndexOutOfBoundsException`, `InformativeEmptyStackException`…) when Warp 10 lets
    /// one escape rather than raising a `WarpScriptException`: the functions that catch and
    /// wrap `WarpScriptException`s let these through unwrapped.
    pub runtime: Option<&'static str>,
}

impl Error {
    /// Create a plain script error with `message`.
    pub fn new(message: impl Into<String>) -> Self {
        Error::Script(ScriptError {
            message: message.into(),
            cause: None,
            runtime: None,
        })
    }

    /// Create a script error with `message` caused by `cause`.
    pub fn with_cause(message: impl Into<String>, cause: Error) -> Self {
        Error::Script(ScriptError {
            message: message.into(),
            cause: Some(Box::new(cause)),
            runtime: None,
        })
    }

    /// Create the error standing for the Java `RuntimeException` of class `class` (a fully
    /// qualified name) with `message`.
    pub fn runtime(class: &'static str, message: impl Into<String>) -> Self {
        Error::Script(ScriptError {
            message: message.into(),
            cause: None,
            runtime: Some(class),
        })
    }

    /// The `InformativeEmptyStackException` of a `pop` on an empty stack.
    pub fn empty_stack() -> Self {
        Error::runtime(
            "io.warp10.script.InformativeEmptyStackException",
            "Empty stack.",
        )
    }

    /// The `ClassCastException` of a Java cast of a `from` object to class `to`.
    pub fn class_cast(from: &str, to: &str) -> Self {
        Error::runtime(
            "java.lang.ClassCastException",
            format!("class {from} cannot be cast to class {to}"),
        )
    }

    /// The `ArrayIndexOutOfBoundsException` of reading `index` in an array of `length`.
    pub fn index_out_of_bounds(index: i64, length: usize) -> Self {
        Error::runtime(
            "java.lang.ArrayIndexOutOfBoundsException",
            format!("Index {index} out of bounds for length {length}"),
        )
    }

    /// The `NullPointerException` of dereferencing a `null`. Java's has no message, which
    /// Warp 10 renders as the simple class name (`ThrowableUtils.getErrorMessage`); that name
    /// stands for the message here.
    pub fn null_pointer() -> Self {
        Error::runtime("java.lang.NullPointerException", "NullPointerException")
    }

    /// `true` for the "abort the call" signals that macro execution lets through unwrapped.
    pub fn is_signal(&self) -> bool {
        !matches!(self, Error::Script(_))
    }

    /// `true` for the errors standing for a Java `RuntimeException` (see
    /// [`ScriptError::runtime`]).
    pub fn is_runtime(&self) -> bool {
        matches!(
            self,
            Error::Script(ScriptError {
                runtime: Some(_),
                ..
            })
        )
    }

    /// The Java class of the exception this error stands for (`ERROR`'s `type`).
    pub fn java_class(&self) -> &'static str {
        match self {
            Error::Script(ScriptError {
                runtime: Some(class),
                ..
            }) => class,
            Error::Script(_) => "io.warp10.script.WarpScriptException",
            Error::Return => "io.warp10.script.WarpScriptReturnException",
            Error::Stop(_) => "io.warp10.script.WarpScriptStopException",
            Error::Break => "io.warp10.script.WarpScriptLoopBreakException",
            Error::Continue => "io.warp10.script.WarpScriptLoopContinueException",
            Error::Abort(_) => "io.warp10.script.WarpScriptATCException",
        }
    }

    /// The message of this error (Java `getMessage()`).
    pub fn message(&self) -> String {
        match self {
            Error::Script(err) => err.message.clone(),
            Error::Return => "RETURN".to_owned(),
            Error::Stop(msg) | Error::Abort(msg) => msg.clone(),
            Error::Break => format!("Exception at BREAK: {LOOP_ONLY}"),
            Error::Continue => format!("Exception at CONTINUE: {LOOP_ONLY}"),
        }
    }

    /// The innermost cause of the chain (the original failure).
    pub fn root_cause(&self) -> &Error {
        let mut current = self;
        while let Error::Script(ScriptError {
            cause: Some(cause), ..
        }) = current
        {
            current = cause;
        }
        current
    }

    /// Messages of the whole cause chain, outermost first.
    pub fn messages(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut current = Some(self);
        while let Some(err) = current {
            out.push(err.message());
            current = match err {
                Error::Script(ScriptError {
                    cause: Some(cause), ..
                }) => Some(cause),
                _ => None,
            };
        }
        out
    }
}

impl fmt::Display for Error {
    /// Formats the error like Warp 10's `ThrowableUtils.getErrorMessage`: the message of each
    /// exception of the cause chain, nested in parentheses.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let messages = self.messages();
        for (idx, message) in messages.iter().enumerate() {
            if idx > 0 {
                f.write_str(" (")?;
            }
            f.write_str(message)?;
        }
        for _ in 1..messages.len() {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

/// Convenience alias used throughout the runtime.
pub type Result<T> = std::result::Result<T, Error>;
