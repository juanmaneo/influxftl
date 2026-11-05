// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `NEWENCODER`, the `WRAP` family and the `UNWRAP` functions, with the answers of
//! Warp 10 3.5.0.

use std::cell::RefCell;
use std::rc::Rc;

use super::compression::gzip_bytes;
use crate::encoder::GtsEncoder;
use crate::encoding::hex_encode;
use crate::tests::{assert_fails, assert_top, fail, fail_msg};
use crate::thrift::{GtsWrapper, serialize_wrapper};
use crate::value::Value;

const LONGS: &str = "NEWGTS 1 NaN NaN NaN 42 ADDVALUE 2 NaN NaN NaN 42 ADDVALUE \
                     3 NaN NaN NaN 43 ADDVALUE 100 NaN NaN NaN -5 ADDVALUE";
const NAMED: &str = "NEWGTS 'n' RENAME 1 NaN NaN NaN 2 ADDVALUE";
const EMPTY_A: &str = r#"{"c":"a","l":{},"a":{},"la":0,"v":[]}"#;
const EMPTY_B: &str = r#"{"c":"b","l":{},"a":{},"la":0,"v":[]}"#;

#[test]
fn wraps() {
    assert_top(
        &format!("{LONGS} WRAPRAW ->HEX"),
        "780c4c025409020e02020cc20109160800",
    );
    assert_top(&format!("{NAMED} WRAPMV ->HEX"), "78034c020400");
    assert_top(&format!("{NAMED} 'WRAPMV!' EVAL ->HEX"), "78034c020400");
    let fast = "1c18016e1b00160016001b000068034c0204160200";
    assert_top(&format!("{NAMED} WRAPFAST ->HEX"), fast);
    assert_top(&format!("{NAMED} WRAP"), "60V0QWg.4V.L.0g..5V2I.734V7.");
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 2 NaN NaN NaN 1.5 ADDVALUE 3 NaN NaN NaN 0.1 ADDVALUE \
         4 NaN NaN NaN 1e300 ADDVALUE WRAPRAWOPT ->HEX",
        "78145002011e11021002010214027e37e43c8800759c160800",
    );
    assert_top("NEWGTS WRAPOPT", "T..L...");
    assert_top("[ ] WRAP SIZE", 0);
    assert_fails(
        "42 WRAP",
        "WRAP expects a Geo Time Series, a GTSEncoder or a list thereof.",
    );
    assert_fails(
        "[ 42 ] WRAPRAW",
        "WRAPRAW expects a Geo Time Series, a GTSEncoder or a list thereof.",
    );
}

#[test]
fn encoders() {
    assert_top("NEWENCODER TYPEOF", "GTSENCODER");
    assert_top("NEWENCODER WRAPRAW ->HEX", "7800160000");
    assert_top("NEWENCODER WRAP", "T..L...");
    assert_top("NEWENCODER WRAPOPT", "T..L...");
    assert_top(
        &format!("{NAMED} WRAP UNWRAPENCODER WRAPRAW ->HEX"),
        "1c18016e1b00160016001b000068034c0204160200",
    );
    assert_top(
        &format!("{NAMED} 2 NaN NaN NaN 3 ADDVALUE WRAPRAW UNWRAPENCODER WRAPRAW ->HEX"),
        "1c18016e1b00160016001b000068064c02040e0202160400",
    );
    assert_top(
        "NEWGTS 'a' RENAME 1 NaN NaN NaN 2 ADDVALUE WRAP UNWRAPENCODER SNAPSHOT",
        "'60V0NGg.4V.L.0g..5V2I.734V7.' UNWRAPENCODER ",
    );
    assert_top(
        "NEWGTS 'a' RENAME 1 NaN NaN NaN 2 ADDVALUE WRAP UNWRAPENCODER SNAPSHOT EVAL WRAP",
        "60V0NGg.4V.L.0g..5V2I.734V7.",
    );
    assert_top(
        "NEWENCODER ->JSON",
        r#"{"c":"","l":{},"a":{},"la":0,"v":[]}"#,
    );
    assert_top(
        "'78039101029100' HEX-> UNWRAPENCODER ->JSON",
        r#"{"c":"","l":{},"a":{},"la":0,"v":[[1,NaN]]}"#,
    );
    assert_top(
        &format!("{NAMED} WRAP UNWRAPENCODER ->JSON"),
        r#"{"c":"n","l":{},"a":{},"la":0,"v":[[1,2]]}"#,
    );
    assert_top(&format!("{NAMED} WRAP UNWRAPENCODER DUP =="), true);
    assert_top("NEWENCODER NEWENCODER ==", false);
    assert_top("NEWENCODER DUP 2 ->LIST ->SET SIZE", 1);
    let encoder = Value::Encoder(Rc::new(RefCell::new(GtsEncoder::new(0))));
    assert_eq!(encoder.java_class(), "io.warp10.continuum.gts.GTSEncoder");
    assert_eq!(encoder.to_string(), "io.warp10.continuum.gts.GTSEncoder@0");
}

#[test]
fn unwraps() {
    assert_top(
        "[ NEWGTS 'a' RENAME NEWGTS 'b' RENAME ] WRAP UNWRAP ->JSON",
        format!("[{EMPTY_A},{EMPTY_B}]"),
    );
    assert_top(
        "[ NEWGTS 'a' RENAME 1 NaN NaN NaN 2 ADDVALUE NEWGTS 'b' RENAME ] WRAPRAW UNWRAPEMPTY ->JSON",
        format!("[{EMPTY_A},{EMPTY_B}]"),
    );
    assert_top("[ ] UNWRAP SIZE", 0);
    assert_top(
        &format!("{LONGS} WRAP UNWRAP ->JSON"),
        r#"{"c":"","l":{},"a":{},"la":0,"v":[[1,42],[2,42],[3,43],[100,-5]]}"#,
    );
    assert_top("NEWGTS 1 NaN NaN NaN 2 ADDVALUE WRAP UNWRAPSIZE", 1);
    assert_top("[ NEWGTS WRAP ] UNWRAPSIZE 0 GET", 0);
}

#[test]
fn unwrap_errors() {
    let invalid = "operates on a string or byte array or a list thereof.";
    assert_fails("42 UNWRAP", &format!("UNWRAP {invalid}"));
    assert_fails("UNWRAP", "Empty stack.");
    assert_fails("UNWRAPENCODER", "Empty stack.");
    assert_fails(
        "[ 'T..L...' 42 ] UNWRAPSIZE",
        &format!("UNWRAPSIZE {invalid}"),
    );
    assert_fails(
        "[ ] UNWRAPENCODER",
        "UNWRAPENCODER operates on a string or byte array.",
    );
    assert_fails("'@@' UNWRAP", "Invalid OPB64 character '@' at offset 0.");
    assert_fails("'é' UNWRAP", "Invalid OPB64 character '?' at offset 0.");
    let eof2 = "Cannot read. Remote side has closed. Tried to read 2 bytes, but only got 0 bytes.";
    assert_fails(
        "'7802' HEX-> UNWRAPENCODER",
        "UNWRAPENCODER failed to unwrap encoder.",
    );
    assert_fails("'7802' HEX-> UNWRAPENCODER", eof2);
    assert_fails(
        "'7802' HEX-> UNWRAPSIZE",
        "UNWRAPSIZE failed to unwrap GTS.",
    );
    assert_fails("'1f' HEX-> UNWRAP", "don't know what type: 15");
    // Runtime exceptions go through unwrapped.
    for script in [
        "'7801012100' HEX-> UNWRAP",
        "'7801012100' HEX-> UNWRAPENCODER",
    ] {
        let message = fail_msg(script);
        assert!(message.contains("Invalid compressed content."), "{message}");
        assert!(!message.contains("failed to unwrap"), "{message}");
        assert_eq!(
            fail(script).root_cause().java_class(),
            "java.lang.RuntimeException"
        );
    }
    // A compressed empty content has no first datapoint to start an encoder from.
    let wrapper = GtsWrapper {
        encoded: Some(gzip_bytes(&[]).unwrap()),
        compressed: Some(true),
        ..GtsWrapper::default()
    };
    let hex = hex_encode(&serialize_wrapper(&wrapper));
    assert_fails(
        &format!("'{hex}' HEX-> UNWRAPENCODER"),
        "UNWRAPENCODER failed to unwrap encoder.",
    );
    assert_fails(
        &format!("'{hex}' HEX-> UNWRAPENCODER"),
        "Can only get an encoder for a decoder on which 'next' was called at least once.",
    );
}
