// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::error::Error as _;
use std::fmt::{self, Write};

use super::error::Error;
use super::error::compat::exception_class;
use super::finite::NotFinite;

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
        Error::class_cast("a.B", "c.D").to_string(),
        "class a.B cannot be cast to class c.D"
    );
    assert_eq!(
        Error::index_out_of_bounds(-1, 3).to_string(),
        "Index -1 out of bounds for length 3"
    );
    assert_eq!(Error::null_pointer().to_string(), "NullPointerException");
    for err in [
        Error::empty_stack(),
        Error::class_cast("a.B", "c.D"),
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
            Error::class_cast("a.B", "c.D"),
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
