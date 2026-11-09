// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Warp 10 compatibility of the errors: the Java exception class a script reads in the `type`
//! of `ERROR`. Nothing else in the crate names a Java class of an error.

use super::Error;

/// `WarpScriptException`, the exception of every failure Warp 10 reports itself.
const WARP_SCRIPT_EXCEPTION: &str = "io.warp10.script.WarpScriptException";

/// The Java class of the exception `err` stands for.
pub fn exception_class(err: &Error) -> &'static str {
    match err {
        Error::Legacy {
            runtime: Some(class),
            ..
        } => class,
        Error::Legacy { runtime: None, .. } | Error::Context { .. } | Error::NotFinite(_) => {
            WARP_SCRIPT_EXCEPTION
        }
        Error::Return => "io.warp10.script.WarpScriptReturnException",
        Error::Stop(_) => "io.warp10.script.WarpScriptStopException",
        Error::Break => "io.warp10.script.WarpScriptLoopBreakException",
        Error::Continue => "io.warp10.script.WarpScriptLoopContinueException",
        Error::Abort(_) => "io.warp10.script.WarpScriptATCException",
        Error::EmptyStack => "io.warp10.script.InformativeEmptyStackException",
        Error::ClassCast { .. } => "java.lang.ClassCastException",
        Error::IndexOutOfBounds { .. } => "java.lang.ArrayIndexOutOfBoundsException",
        Error::NullPointer => "java.lang.NullPointerException",
    }
}
