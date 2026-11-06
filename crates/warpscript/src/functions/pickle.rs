// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.
// Derived from Pyrolite 4.30 (Copyright Irmen de Jong, MIT License).

//! `->PICKLE` and `PICKLE->`: Python pickle serialization.

use super::Entry;
use super::args::fail;
use super::encoders::togts;
use crate::error::Result;
use crate::pickle::dumps;
use crate::stack::Stack;
use crate::unpickle::loads;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("->PICKLE", Entry::Stack(topickle)),
    ("PICKLE->", Entry::Stack(pickleto)),
];

/// `->PICKLE`: the protocol 2 pickle of the top of the stack, encoders converted by `->GTS`.
fn topickle(stack: &mut Stack) -> Result<()> {
    if matches!(stack.peek()?, Value::Encoder(_)) {
        // `->GTS` cannot fail on an encoder, and leaves its result on the stack.
        togts(stack, "->GTS").unwrap_or(());
    }
    let value = stack.pop().unwrap_or(Value::Null);
    stack.push(Value::bytes(dumps(&value)?));
    Ok(())
}

/// `PICKLE->`: the value of a pickle.
fn pickleto(stack: &mut Stack) -> Result<()> {
    let Value::Bytes(bytes) = stack.pop()? else {
        return Err(fail(
            "PICKLE->",
            "expects a byte array on top of the stack.",
        ));
    };
    stack.push(loads(&bytes)?);
    Ok(())
}
