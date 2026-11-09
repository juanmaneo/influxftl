// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `GEO.COVER` and `GEO.COVER.RL` given an encoder whose last datapoint is cut short, which
//! scripts cannot build: reading it fails.

use std::cell::RefCell;
use std::rc::Rc;

use crate::encoder::{EncodedValue, EncoderError, GtsEncoder};
use crate::error::Error;

use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::tests::test_stack;
use crate::value::Value;

/// An encoder of 2 datapoints, the second one cut short.
fn corrupt() -> Value {
    let mut whole = GtsEncoder::new(0);
    whole.add_value(1, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(5));
    whole.add_value(2, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(6));
    let stream = whole.stream_bytes();
    let cut = GtsEncoder::with_stream(&stream[..stream.len() - 1]);
    Value::Encoder(Rc::new(RefCell::new(cut)))
}

/// The failure of `function` applied to the corrupt encoder.
fn failure(function: &str) -> Error {
    let mut stack = test_stack();
    stack.push(corrupt());
    stack.push(Value::Long(10));
    stack
        .exec_checked(function)
        .expect_err("the encoder is corrupt")
        .root_cause()
        .clone()
}

#[test]
fn a_cover_fails_on_a_corrupt_encoder() {
    let error = failure("GEO.COVER");
    assert_eq!(error, EncoderError::BufferUnderflow.into());
}

#[test]
fn a_polyline_cover_fails_on_a_corrupt_encoder() {
    let error = failure("GEO.COVER.RL");
    assert_eq!(error, EncoderError::BufferUnderflow.into());
}
