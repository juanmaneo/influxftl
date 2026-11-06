// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `NEWENCODER`, the `WRAP` family and the `UNWRAP` functions, with the answers of
//! Warp 10 3.5.0.

use std::cell::RefCell;
use std::rc::Rc;

use super::compression::gzip_bytes;
use crate::encoder::{EncodedValue, GtsEncoder};
use crate::encoding::hex_encode;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_stack, assert_top, fail, fail_msg, top};
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

const POINTS: &str = "[ [ 1 2 ] [ 2 3 4.5 ] [ 3 48.0 -4.5 'x' ] [ 4 1.0 2.0 3 true ] ] ->ENCODER";
const SERIES: &str = "NEWGTS 'n' RENAME { 'a' 'b' } RELABEL { 'x' 'y' } SETATTRIBUTES \
                      1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 3.5 ADDVALUE";
const MIXED: &str = "[ [ 1 2 ] [ 2 3.5 ] [ 3 'x' ] [ 4 true ] [ 5 '00ff' HEX-> ] [ 6 2 ] \
                     [ 7 1.0 2.0 3 4 ] ] ->ENCODER 'n' RENAME { 'a' 'b' } RELABEL";

/// The hex of a serialized wrapper.
fn wrapper_hex(wrapper: &GtsWrapper) -> String {
    hex_encode(&serialize_wrapper(wrapper))
}

#[test]
fn toencoder() {
    assert_top(
        "[ ] ->ENCODER ->JSON",
        r#"{"c":"","l":{},"a":{},"la":0,"v":[]}"#,
    );
    assert_top(
        "[ [ 1 2 ] [ 2 3 4.5 ] [ 3 48.0 -4.5 'x' ] [ 4 1.0 2.0 3 true ] \
         [ 5 NaN NaN NaN NULL ] ] ->ENCODER ->JSON",
        r#"{"c":"","l":{},"a":{},"la":0,"v":[[1,2],[2,3,4.5],[3,47.99999998882413,-4.500000067055225,"x"],[4,0.9999999823048711,1.9999999646097422,3,true],[5,null]]}"#,
    );
    assert_top(
        &format!("{POINTS} WRAPRAW ->HEX"),
        "78284c0204940c02064012000000000000984002b5707070707070700178844c02c0033cf0033cf00306160800",
    );
    assert_top("[ [ 1 2 ] ] ->ENCODER TYPEOF", "GTSENCODER");
    assert_top("[ [ 1 2 ] ] ->ENCODER WRAPRAW ->HEX", "78034c0204160200");
    // A series loses its metadata, a wrapped one starting an empty encoder brings its own.
    let values = r#""la":0,"v":[[1,2],[2,3]]}"#;
    assert_top(
        &format!("[ {SERIES} ] ->ENCODER ->JSON"),
        format!(r#"{{"c":"","l":{{}},"a":{{}},{values}"#),
    );
    for wrap in ["WRAP", "WRAPRAW"] {
        assert_top(
            &format!("[ {SERIES} {wrap} ] ->ENCODER ->JSON"),
            format!(r#"{{"c":"n","l":{{"a":"b"}},"a":{{"x":"y"}},{values}"#),
        );
    }
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 2.5 ADDVALUE ] ->ENCODER WRAPRAW ->HEX",
        "780a54024004000000000000160200",
    );
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 2.5 ADDVALUE WRAPOPT ] ->ENCODER WRAPRAW ->HEX",
        "780450020132160200",
    );
    assert_top(
        "[ [ 1 2 ] [ 2 3 ] ] ->ENCODER 'e' STORE [ $e WRAP $e WRAP ] ->ENCODER WRAPRAW ->HEX",
        "780c4c02040e02024c02040e0202160800",
    );
    assert_top(
        "[ [ 1 2 ] ] ->ENCODER WRAP 'w' STORE [ $w $w ] ->ENCODER WRAPRAW ->HEX",
        "78064c02044c0204160400",
    );
    assert_top(
        "[ NEWGTS 'a' RENAME 1 NaN NaN NaN 2 ADDVALUE WRAP [ 3 4 ] ] ->ENCODER WRAPRAW ->HEX",
        "1c1801611b00160016001b0000680d4c02046c000000000000000308160400",
    );
    assert_top(
        "[ [ 3 4 ] NEWGTS 'a' RENAME 1 NaN NaN NaN 2 ADDVALUE WRAP ] ->ENCODER WRAPRAW ->HEX",
        "78064c06084c0204160400",
    );
    // A corrupt datapoint after the first one is appended as is.
    assert_top(
        "[ '78044c02044c00' HEX-> ] ->ENCODER WRAPRAW ->HEX",
        "78044c02044c160000",
    );
    // NaN coordinates and elevations are absent, an elevation is truncated.
    assert_top(
        "[ [ 1 NaN 2 ] [ 2 1 NaN 3 ] [ 3 1 2 NaN 4 ] [ 4 1 2 3.7 5 ] [ 5 MAXLONG 6 ] ] \
         ->ENCODER ->JSON",
        r#"{"c":"","l":{},"a":{},"la":0,"v":[[1,2],[2,3],[3,0.9999999823048711,1.9999999646097422,4],[4,0.9999999823048711,1.9999999646097422,3,5],[5,9223372036854775807,6]]}"#,
    );
}

#[test]
fn toencoder_errors() {
    assert_fails(
        "42 ->ENCODER",
        "->ENCODER expects a list on top of the stack.",
    );
    assert_fails("->ENCODER", "Empty stack.");
    let invalid = "->ENCODER encountered an invalid element.";
    for script in [
        "[ NEWENCODER ]",
        "[ 42 ]",
        "[ [ ] ]",
        "[ [ 1 ] ]",
        "[ [ 'a' ] ]",
        "[ [ 1 2 3 4 5 6 ] ]",
        "[ [ 1 'a' 2 ] ]",
        "[ [ 1 NULL 2 ] ]",
        "[ [ 1 'a' 2 3 ] ]",
        "[ [ 1 1 'a' 3 ] ]",
        "[ [ 1 1 NULL 2 ] ]",
        "[ [ 1 1 2 'a' 3 ] ]",
        "[ [ 1 'a' 2 3 3 ] ]",
    ] {
        assert_fails(&format!("{script} ->ENCODER"), invalid);
    }
    assert_fails(
        "[ [ 1.0 2 ] ] ->ENCODER",
        "->ENCODER encountered an invalid timestamp.",
    );
    assert_fails(
        "[ [ 1 [ ] ] ] ->ENCODER",
        "Unsuported value type 'class java.util.ArrayList'",
    );
    assert_fails(
        "[ 'é' ] ->ENCODER",
        "Invalid OPB64 character '?' at offset 0.",
    );
    assert_fails(
        "[ '7802' HEX-> ] ->ENCODER",
        "->ENCODER failed to unwrap encoder.",
    );
    assert_fails("[ '00' HEX-> ] ->ENCODER", "NullPointerException");
    assert_fails("[ '78014c00' HEX-> ] ->ENCODER", "BufferUnderflowException");
    for script in [
        "[ NEWENCODER WRAP ] ->ENCODER",
        "[ [ 1 2 ] NEWENCODER WRAP ] ->ENCODER",
    ] {
        assert_fails(script, "->ENCODER cannot get encoder from wrapper.");
        assert_fails(
            script,
            "Can only get an encoder for a decoder on which 'next' was called at least once.",
        );
    }
    // Merging an encoder of another base timestamp decodes it, up to its corrupt datapoint.
    let mut encoder = GtsEncoder::new(5);
    encoder.add_value(1, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(2));
    let mut encoded = encoder.bytes().to_vec();
    encoded.push(0x4c);
    let wrapper = GtsWrapper {
        encoded: Some(encoded),
        base: Some(5),
        ..GtsWrapper::default()
    };
    let hex = wrapper_hex(&wrapper);
    assert_fails(
        &format!("[ '{hex}' HEX-> ] ->ENCODER"),
        "BufferUnderflowException",
    );
}

#[test]
fn encoderto() {
    assert_top(
        &format!("{POINTS} ENCODER-> 4 ->LIST ->JSON"),
        r#"[null,{},{},[[1,NaN,NaN,NaN,2],[2,NaN,NaN,3,4.5],[3,47.99999998882413,-4.500000067055225,NaN,"x"],[4,0.9999999823048711,1.9999999646097422,3,true]]]"#,
    );
    let decoded = r#"["n",{"a":"b"},{"x":"y"},[[1,NaN,NaN,NaN,2],[2,NaN,NaN,NaN,3]]]"#;
    for wrap in ["WRAP", "WRAPRAW", "WRAPOPT", "ASENCODERS"] {
        assert_top(
            &format!("{SERIES} {wrap} ENCODER-> 4 ->LIST ->JSON"),
            decoded,
        );
    }
    assert_top(
        "[ [ 1 2 ] [ 7 NULL ] [ 8 '00ff' HEX-> ] [ 9 MAXLONG ] ] ->ENCODER WRAP ENCODER-> \
         4 ->LIST ->JSON",
        r#"[null,{},{},[[1,NaN,NaN,NaN,2],[7,NaN,NaN,NaN,null],[8,NaN,NaN,NaN,"\u0000\u00FF"],[9,NaN,NaN,NaN,9223372036854775807]]]"#,
    );
    assert_top(
        "[ [ 1 '00ff' HEX-> ] ] ->ENCODER ENCODER-> 0 GET 4 GET TYPEOF",
        "BYTES",
    );
    assert_top("NEWENCODER ENCODER-> 4 ->LIST ->JSON", r#"["",{},{},[]]"#);
    assert_top(
        "NEWGTS 'n' RENAME 1 NaN NaN NaN 2 ADDVALUE WRAPMV ENCODER-> 4 ->LIST ->JSON",
        r"[null,{},{},[[1,NaN,NaN,NaN,2]]]",
    );
    let invalid = "ENCODER-> operates on a string, byte array or encoder.";
    assert_fails("42 ENCODER->", invalid);
    assert_fails(&format!("{SERIES} ENCODER->"), invalid);
    assert_fails("ENCODER->", "Empty stack.");
    assert_fails(
        "'7802' HEX-> ENCODER->",
        "ENCODER-> failed to unwrap encoder.",
    );
    assert_fails("'@@' ENCODER->", "Invalid OPB64 character '@' at offset 0.");
    assert_fails("'00' HEX-> ENCODER->", "NullPointerException");
    assert_fails("'78014c00' HEX-> ENCODER->", "BufferUnderflowException");
}

#[test]
fn asencoders() {
    let encoded = r#"{"c":"n","l":{"a":"b"},"a":{"x":"y"},"la":0,"v":[[1,2],[2,3]]}"#;
    assert_top(&format!("{SERIES} ASENCODERS ->JSON"), encoded);
    assert_top(
        &format!("[ {SERIES} {SERIES} WRAP {SERIES} WRAPRAW NEWENCODER ] ASENCODERS ->JSON"),
        format!(r#"[{encoded},{encoded},{encoded},{{"c":"","l":{{}},"a":{{}},"la":0,"v":[]}}]"#),
    );
    assert_top(
        &format!("{SERIES} ASENCODERS WRAPRAW ->HEX"),
        "1c18016e1b018801610162160016001b0188017801790068064c02040e0202160400",
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 2.5 ADDVALUE ASENCODERS WRAPRAW ->HEX",
        "780450020132160200",
    );
    assert_top(
        "[ [ 1 2.5 ] ] ->ENCODER ASENCODERS WRAPRAW ->HEX",
        "780a54024004000000000000160200",
    );
    assert_top("NEWENCODER DUP ASENCODERS ==", true);
    assert_top(
        "NEWENCODER WRAP ASENCODERS ->JSON",
        r#"{"c":"","l":{},"a":{},"la":0,"v":[]}"#,
    );
    let invalid = "ASENCODERS expects a Geo Time Series or a list thereof on top on the stack.";
    assert_fails("42 ASENCODERS", invalid);
    assert_fails("[ 42 ] ASENCODERS", invalid);
    assert_fails(
        "'7800' HEX-> ASENCODERS",
        "ASENCODERS failed to unwrap encoder.",
    );
    assert_fails("'00' HEX-> ASENCODERS", "NullPointerException");
    assert_fails("'78014c00' HEX-> ASENCODERS", "BufferUnderflowException");
    let wrapper = GtsWrapper {
        encoded: Some(gzip_bytes(&[]).unwrap()),
        compressed: Some(true),
        ..GtsWrapper::default()
    };
    let hex = wrapper_hex(&wrapper);
    assert_fails(
        &format!("'{hex}' HEX-> ASENCODERS"),
        "ASENCODERS failed to convert wrapper to encoder.",
    );
}

#[test]
fn toencoders() {
    let prefix = r#""c":"","l":{},"a":{},"la":0"#;
    let split = format!(
        r#"{{"LONG":{{{prefix},"v":[[1,2],[6,2]]}},"DOUBLE":{{{prefix},"v":[[2,3.5],[7,null]]}},"STRING":{{{prefix},"v":[[3,"x"]]}},"BOOLEAN":{{{prefix},"v":[[4,true]]}},"BINARY":{{{prefix},"v":[[5,"\u0000\u00FF"]]}}}}"#
    );
    let points = "[ [ 1 2 ] [ 2 3.5 ] [ 3 'x' ] [ 4 true ] [ 5 '00ff' HEX-> ] [ 6 2 ] \
                  [ 7 NULL ] ] ->ENCODER";
    assert_top(&format!("{points} ->ENCODERS ->JSON"), split.as_str());
    assert_top(&format!("{points} WRAP ->ENCODERS ->JSON"), split.as_str());
    assert_top(
        &format!("{points} ->ENCODERS 'DOUBLE' GET WRAPRAW ->HEX"),
        "780c5404400c000000000000070a160400",
    );
    assert_top(
        &format!("{SERIES} WRAPOPT ->ENCODERS ->JSON"),
        r#"{"LONG":{"c":"n","l":{"a":"b"},"a":{"x":"y"},"la":0,"v":[[1,2],[2,3]]}}"#,
    );
    assert_top("NEWENCODER ->ENCODERS ->JSON", "{}");
    assert_top(
        "[ [ 1 2 ] ] ->ENCODER DUP ->ENCODERS 'LONG' GET WRAPRAW ->HEX SWAP WRAPRAW ->HEX ==",
        true,
    );
    assert_fails(
        "42 ->ENCODERS",
        "->ENCODERS operates on a string, byte array or encoder.",
    );
    assert_fails(
        "'7802' HEX-> ->ENCODERS",
        "->ENCODERS failed to unwrap encoder.",
    );
    for script in [
        "'78014c00' HEX-> ->ENCODERS",
        "'78044c02044c00' HEX-> ->ENCODERS",
    ] {
        assert_fails(
            script,
            "->ENCODERS encountered an exception during conversion.",
        );
        assert_fails(script, "BufferUnderflowException");
    }
}

#[test]
fn togts() {
    let meta = r#""c":"n","l":{"a":"b"},"a":{},"la":0"#;
    let longs = r"[[1,2],[6,2],[7,0.9999999823048711,1.9999999646097422,3,4]]";
    assert_top(
        &format!("{MIXED} ->GTS ->JSON"),
        format!(
            r#"{{"LONG":{{{meta},"v":{longs}}},"DOUBLE":{{{meta},"v":[[2,3.5]]}},"BOOLEAN":{{{meta},"v":[[4,true]]}},"STRING":{{{meta},"v":[[3,"x"]]}},"BINARY":{{{meta},"v":[[5,"\u0000\u00FF"]]}}}}"#
        ),
    );
    assert_top(
        &format!("[ {MIXED} {MIXED} WRAP {MIXED} WRAPRAW ] ->GTS 'LONG' GET ->JSON"),
        format!(r#"[{{{meta},"v":{longs}}},{{{meta},"v":{longs}}},{{{meta},"v":{longs}}}]"#),
    );
    assert_top(
        &format!("[ {MIXED} '78034c020400' HEX-> ] ->GTS KEYLIST ->JSON"),
        r#"["LONG","DOUBLE","BOOLEAN","STRING","BINARY"]"#,
    );
    assert_top(
        "[ [ 1 '00ff' HEX-> ] [ 2 'x' ] ] ->ENCODER 'e' STORE [ $e [ [ 1 2 ] ] ->ENCODER ] ->GTS \
         KEYLIST ->JSON",
        r#"["STRING","BINARY","LONG"]"#,
    );
    assert_top("[ ] ->GTS ->JSON", "{}");
    assert_top("NEWENCODER ->GTS ->JSON", "{}");
    assert_top("[ NEWENCODER ] ->GTS ->JSON", "{}");
    assert_top(
        "[ [ 1 2.0 ] [ 2 NULL ] ] ->ENCODER ->GTS ->JSON",
        r#"{"DOUBLE":{"c":"","l":{},"a":{},"la":0,"v":[[1,2.0]]}}"#,
    );
    assert_top(
        "'78034c020400' HEX-> ->GTS ->JSON",
        r#"{"LONG":{"c":"","l":{},"a":{},"la":0,"v":[[1,2]]}}"#,
    );
    let invalid = "->GTS operates on a string, a byte array, an encoder, or list thereof.";
    assert_fails("42 ->GTS", invalid);
    assert_fails("[ 42 ] ->GTS", invalid);
    assert_fails("{ } ->GTS", "Empty stack.");
    assert_fails("'7802' HEX-> ->GTS", "->GTS failed to unwrap encoder.");
    assert_fails("'00' HEX-> ->GTS", "NullPointerException");
    assert_fails("'78014c00' HEX-> ->GTS", "BufferUnderflowException");
    assert_fails("'78014c00' HEX-> { } ->GTS", "BufferUnderflowException");
}

#[test]
fn togts_typed() {
    let typed = |kind: &str, values: &str| {
        format!(r#"{{"c":"n","l":{{"a":"b",".type":"{kind}"}},"a":{{}},"la":0,"v":{values}}}"#)
    };
    let loc = "0.9999999823048711,1.9999999646097422,3";
    for (kind, values) in [
        (
            "DOUBLE",
            format!("[[1,2.0],[2,3.5],[3,0.0],[4,1.0],[5,0.0],[6,2.0],[7,{loc},4.0]]"),
        ),
        (
            "LONG",
            format!("[[1,2],[2,3],[3,0],[4,1],[5,0],[6,2],[7,{loc},4]]"),
        ),
        (
            "STRING",
            format!(
                r#"[[1,"2"],[2,"3.5"],[3,"x"],[4,"T"],[5,"\u0000\u00FF"],[6,"2"],[7,{loc},"4"]]"#
            ),
        ),
        (
            "BOOLEAN",
            format!("[[1,true],[2,true],[3,true],[4,true],[5,true],[6,true],[7,{loc},true]]"),
        ),
    ] {
        assert_top(
            &format!("{MIXED} {{ '{kind}' 'n{{}}' }} ->GTS ->JSON"),
            typed(kind, &values),
        );
    }
    let binary =
        format!(r#"[[1,"2"],[2,"3.5"],[3,"x"],[4,"T"],[5,"\u0000\u00FF"],[6,"2"],[7,{loc},"4"]]"#);
    assert_top(
        &format!("{MIXED} {{ 'BINARY' 'n{{}}' }} ->GTS ->JSON"),
        typed("BINARY", &binary),
    );
    // Without a matching selector, the first value gives the type.
    let longs = format!("[[1,2],[2,3],[3,0],[4,1],[5,0],[6,2],[7,{loc},4]]");
    assert_top(
        &format!("{MIXED} {{ 'DOUBLE' 'm{{}}' }} ->GTS ->JSON"),
        typed("LONG", &longs),
    );
    let doubles = format!("[[1,2.0],[2,3.5],[3,0.0],[4,1.0],[5,0.0],[6,2.0],[7,{loc},4.0]]");
    assert_top(
        &format!("{MIXED} {{ 'DOUBLE' [ 'm{{}}' 'n{{a=b}}' ] 'label.type' 't' }} ->GTS ->JSON"),
        format!(r#"{{"c":"n","l":{{"a":"b","t":"DOUBLE"}},"a":{{}},"la":0,"v":{doubles}}}"#),
    );
    assert_top(
        &format!("{MIXED} {{ 'DOUBLE' [ 'm{{}}' 'n{{a=b}}' ] 'label.type' NULL }} ->GTS ->JSON"),
        format!(r#"{{"c":"n","l":{{"a":"b"}},"a":{{}},"la":0,"v":{doubles}}}"#),
    );
    assert_top(
        &format!("[ {MIXED} NEWENCODER ] {{ }} ->GTS ->JSON"),
        format!(
            r#"[{},{{"c":"","l":{{".type":"UNDEFINED"}},"a":{{}},"la":0,"v":[]}}]"#,
            typed("LONG", &longs)
        ),
    );
    assert_top(
        &format!("[ {MIXED} NEWENCODER ] {{ 'LONG' '~.*{{}}' }} ->GTS 1 GET ->JSON"),
        r#"{"c":"","l":{".type":"LONG"},"a":{},"la":0,"v":[]}"#,
    );
    // An unnamed series matches no selector.
    for (map, kind, values) in [
        ("{ 'LONG' '~.*{}' }", "DOUBLE", "[[1,2.5],[2,0.0]]"),
        ("{ }", "DOUBLE", "[[1,2.5],[2,0.0]]"),
    ] {
        assert_top(
            &format!("[ [ 1 2.5 ] [ 2 'x' ] ] ->ENCODER {map} ->GTS ->JSON"),
            format!(r#"{{"c":"","l":{{".type":"{kind}"}},"a":{{}},"la":0,"v":{values}}}"#),
        );
    }
    for (points, kind, values) in [
        ("[ 1 'a' ] [ 2 3 ]", "STRING", r#"[[1,"a"],[2,"3"]]"#),
        (
            "[ 1 'a' ] [ 2 true ] [ 3 2.5 ]",
            "STRING",
            r#"[[1,"a"],[2,"T"],[3,"2.5"]]"#,
        ),
        (
            "[ 1 false ] [ 2 3 ] [ 3 '00ff' HEX-> ]",
            "BOOLEAN",
            "[[1,false],[2,true],[3,true]]",
        ),
        ("[ 1 '00ff' HEX-> ]", "BINARY", r#"[[1,"\u0000\u00FF"]]"#),
        ("[ 1 NULL ] [ 2 3 ]", "UNDEFINED", "[[2,3]]"),
        ("[ 1 NULL ]", "UNDEFINED", "[]"),
        ("[ 1 2 ]", "LONG", "[[1,2]]"),
    ] {
        assert_top(
            &format!("[ {points} ] ->ENCODER {{ }} ->GTS ->JSON"),
            format!(r#"{{"c":"","l":{{".type":"{kind}"}},"a":{{}},"la":0,"v":{values}}}"#),
        );
    }
    assert_top(
        "[ [ 1 2 ] ] ->ENCODER WRAP { } ->GTS ->JSON",
        r#"{"c":"","l":{".type":"LONG"},"a":{},"la":0,"v":[[1,2]]}"#,
    );
    let selectors = "->GTS type MAP input must contains selector or list of thereof for each type.";
    assert_fails(
        &format!("{MIXED} {{ 'DOUBLE' [ 'm{{}}' 42 ] }} ->GTS"),
        selectors,
    );
    assert_fails(&format!("{MIXED} {{ 'DOUBLE' 42 }} ->GTS"), selectors);
    let keys = "->GTS type MAP input must contains valid types as key (LONG, DOUBLE, BOOLEAN, \
                STRING or BINARY) or 'label.type' to override '.type' label.";
    assert_fails(&format!("{MIXED} {{ 'FOO' 'n{{}}' }} ->GTS"), keys);
    assert_fails(&format!("{MIXED} {{ 42 'n{{}}' }} ->GTS"), keys);
    assert_fails(
        &format!("{MIXED} {{ 'label.type' 42 }} ->GTS"),
        "->GTS extra 'label' input in the MAP must be either null or a string.",
    );
    assert_fails(
        &format!("{MIXED} {{ 'label.type' 'a' }} ->GTS"),
        "->GTS the input already has label a",
    );
    assert_fails(
        &format!("{MIXED} {{ 'DOUBLE' 'n{{' }} ->GTS"),
        "invalid syntax for selector.",
    );
    assert_fails(
        &format!("{MIXED} {{ 'DOUBLE' '~[{{}}' }} ->GTS"),
        "Invalid character class",
    );
}

#[test]
fn togts_failures() {
    // The backtracking limit of the regular expression engine of the selectors.
    let long = "a".repeat(46);
    assert_fails(
        &format!("[ [ 1 2 ] ] ->ENCODER '{long}' RENAME {{ 'LONG' '~(a|a)*\\1b{{}}' }} ->GTS"),
        "Max limit for backtracking count exceeded",
    );
    assert_fails("{ 'LONG' 'n{}' } ->GTS", "Empty stack.");
    assert_fails("->GTS", "Empty stack.");
}

#[test]
fn optimize() {
    assert_top(
        "NEWGTS 1 NaN NaN NaN 2 ADDVALUE 10 OPTIMIZE ->JSON",
        r#"{"c":"","l":{},"a":{},"la":0,"v":[[1,2]]}"#,
    );
    assert_top("NEWENCODER DUP 1.5 OPTIMIZE ==", true);
    assert_top("[ NEWGTS NEWENCODER ] 1.5 OPTIMIZE SIZE", 2);
    let invalid = "OPTIMIZE  operates on a Geo Time Series or encoder instance or a list thereof.";
    assert_fails("42 1 OPTIMIZE", invalid);
    assert_fails("[ 42 ] 1 OPTIMIZE", invalid);
    assert_fails(
        "NEWGTS 'a' OPTIMIZE",
        "OPTIMIZE expects a ratio on top of the stack.",
    );
    assert_fails("OPTIMIZE", "Empty stack.");
}

#[test]
fn encoder_metadata() {
    assert_top(
        "[ [ 1 2 ] ] ->ENCODER DUP NAME SWAP '+x' RENAME DUP NAME SWAP 'a' RENAME '+b' RENAME \
         { 'k' 'v' 'l' 'w' } RELABEL { 'l' '' } RELABEL { 'at' 'v' } SETATTRIBUTES DUP LABELS \
         SWAP DUP ATTRIBUTES SWAP DUP NAME SWAP LASTACTIVITY 6 ->LIST ->JSON",
        r#"[null,"nullx",{"k":"v"},{"at":"v"},"ab",0]"#,
    );
    assert_top(
        "[ [ 1 2 ] ] ->ENCODER { NULL 'x' 'a' 'b' } SETATTRIBUTES { 'c' 'd' } SETATTRIBUTES \
         DUP ATTRIBUTES SWAP { NULL NULL } SETATTRIBUTES ATTRIBUTES 2 ->LIST ->JSON",
        r#"[{"a":"b","c":"d"},{}]"#,
    );
    assert_top("[ [ 1 2 ] ] ->ENCODER 1 ->LIST 'x' RENAME 0 GET NAME", "x");
}

#[test]
fn parsevalue() {
    let hex =
        |script: &str, expected: &str| assert_top(&format!("{script} PARSEVALUE ->HEX"), expected);
    assert_top("' 42 ' PARSEVALUE", 42);
    assert_top("'+3' PARSEVALUE", 3);
    assert_top("'1.5' PARSEVALUE", 1.5);
    assert_top("'-1.50' PARSEVALUE", -1.5);
    assert_top("'1.' PARSEVALUE", 1.0);
    assert_top("'.5' PARSEVALUE", 0.5);
    assert_top("'12345678901234.5' PARSEVALUE", 1.234_567_890_123_45E13);
    assert_top("'1e3' PARSEVALUE", 1000.0);
    assert_top("'1f' PARSEVALUE", 1.0);
    assert_top("'1.0E400' PARSEVALUE", f64::INFINITY);
    assert_top("'+NaN' PARSEVALUE TOSTRING", "NaN");
    for (text, expected) in [("T", true), ("true", true), ("TRUE", true), ("t", true)] {
        assert_top(&format!("'{text}' PARSEVALUE"), expected);
    }
    for (text, expected) in [("F", false), ("false", false), ("f", false)] {
        assert_top(&format!("'{text}' PARSEVALUE"), expected);
    }
    assert_top(r#""'%C3%A9'" PARSEVALUE"#, "\u{e9}");
    assert_top(r#""'a+b'" PARSEVALUE"#, "a+b");
    assert_top(r#""''" PARSEVALUE"#, "");
    assert_top(r#"'"x"' PARSEVALUE"#, "x");
    assert_top("'HH:48.0:2.0' PARSEVALUE", -2_296_496_678_784_176_095_i64);
    assert_top("'HH:91:0' PARSEVALUE", -6_149_522_308_886_033_752_i64);
    assert_top("'Q:1:0:0:0' PARSEVALUE", -140_739_635_871_745_i64);
    assert_top("'Q:2:0:0:0' PARSEVALUE", -140_739_635_871_745_i64);
    hex("'hex:00ff'", "00ff");
    for (text, expected) in [
        ("b64:AAA=", "0000"),
        ("b64:AAA", "0000"),
        ("b64:A", ""),
        ("b64:AA", "00"),
        ("b64:-_8", "fbff"),
        ("b64:A A=A", "00"),
        ("b64:AAAA====AAAA", "000000"),
        ("b64:/+8=", "ffef"),
        ("b64:az09", "6b3d3d"),
        ("b64:@@", ""),
    ] {
        hex(&format!("'{text}'"), expected);
    }
    // Nested encoders, serialized as their (uncounted) wrapper.
    assert_top("'[ 1 2 ]' PARSEVALUE TYPEOF", "BYTES");
    hex("'[ 1 2 ]'", "78042c022e0200");
    hex("'[! 1 2 ]'", "78042c022e0200");
    hex("'[1 2]'", "78042c022e0200");
    hex("'[ ]'", "780000");
    hex(
        "'[ [ 1 ] 2/[! 3 ] ]'",
        "780f3a0578022c02005a040578022c060000",
    );
    hex("'[ 1//2/[ 3 ] ]'", "780ada0c02040578022c060000");
    hex(
        "'[ 1/1:2/[ 2 ] ]'",
        "7811da4002c0033cf0033cf0030578022c040000",
    );
    hex("'[ 1/2/3/4 ]'", "780dcc4c020000000000000002060800");
    hex("'[ 1/2//4 ]'", "780ccc400200000000000000020800");
    hex("'[ [ [ 1 ] ] ]'", "780c3a0a78073a0578022c02000000");
    hex(
        r#""[ 10/1 20/48.0:2.0/3 30//5/'x' 40/1234/7/T 50/48:2/-1/1.5 60/1.5 ]""#,
        "78314c14028e4014e02134702134702104980c140a0178844e1400000000000004d204904e14e0213470213470210f011e111400",
    );
}

#[test]
fn parsevalue_errors() {
    let error = |text: &str, index: usize, cause: &str| {
        let shown = text.trim();
        assert_fails(
            &format!("{text} PARSEVALUE"),
            &format!(
                "PARSEVALUE encountered a parse error at index {index} for '{}'. ({cause})",
                shown[1..shown.len() - 1].trim().replace("%25", "%")
            ),
        );
    };
    assert_fails("PARSEVALUE", "Empty stack.");
    assert_fails("1 PARSEVALUE", "PARSEVALUE operates on a STRING.");
    for (text, index, cause) in [
        (
            "'1.5 2 *'",
            0,
            "Cannot parse value. (For input string: \"1.5 2 *\")",
        ),
        (
            "'99999999999999999999'",
            0,
            "Cannot parse value. (For input string: \"99999999999999999999\")",
        ),
        ("'tr'", 0, "Cannot parse value. (For input string: \"tr\")"),
        (
            "'abc'",
            0,
            "Cannot parse value. (For input string: \"abc\")",
        ),
        (
            "'  '",
            0,
            "Cannot parse value. (Index 0 out of bounds for length 0)",
        ),
        (
            "\"'\"",
            0,
            "Cannot parse value. (Range [1, 0) out of bounds for length 1)",
        ),
        (
            "\"'%25zz'\"",
            0,
            "Cannot parse value. (URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: \"z\" = 122)",
        ),
        (
            "'HH:48.0'",
            0,
            "Invalid value for lat,lon conversion to HHCode.",
        ),
        (
            "'HH:x:2'",
            3,
            "Cannot parse latitude. (For input string: \"x\")",
        ),
        (
            "'HH:1:y'",
            5,
            "Cannot parse longitude. (For input string: \"y\")",
        ),
        (
            "'Q:1:0:0'",
            0,
            "Invalid value for Quaternion, expected Q:w:x:y:z",
        ),
        (
            "'Q:a:0:0:0'",
            2,
            "Cannot parse quaternion component. (For input string: \"a\")",
        ),
        (
            "'Q:1:0:0:x'",
            8,
            "Cannot parse quaternion component. (For input string: \"x\")",
        ),
        (
            "'Q:NaN:0:0:0'",
            0,
            "Quaternion values require finite elements.",
        ),
        (
            "'hex:0'",
            0,
            "Cannot parse value. (Odd number of hexadecimal digits.)",
        ),
        ("':x:1'", 0, "Unsupported value ':x:1'"),
        (
            "'1.2.3'",
            0,
            "Cannot parse value. (Character array contains more than one decimal point.)",
        ),
        ("'+'", 0, "Cannot parse value. (No digits found.)"),
        (
            "'1-2'",
            0,
            "Cannot parse value. (Character - is neither a decimal digit number, decimal point, nor \"e\" notation exponential mark.)",
        ),
        ("'[ 1 2'", 0, "Missing closing bracket."),
        (
            "'['",
            0,
            "Cannot parse value. (Index 1 out of bounds for length 1)",
        ),
        ("'[!'", 0, "Missing closing bracket."),
        ("'[ [ 1 ]'", 2, "Missing closing bracket."),
        ("'[ 1/[ 2 ]'", 4, "Missing closing bracket."),
        (
            "'[ x/1 ]'",
            0,
            "Cannot parse value. (For input string: \"x\")",
        ),
        (
            "'[ 1/a:2/3 ]'",
            0,
            "Cannot parse value. (For input string: \"a\")",
        ),
        (
            "'[ 1/1:y/3 ]'",
            0,
            "Cannot parse value. (For input string: \"y\")",
        ),
        (
            "'[ 1/z/3 ]'",
            0,
            "Cannot parse value. (For input string: \"z\")",
        ),
        (
            "'[ 1//z/3 ]'",
            0,
            "Cannot parse value. (For input string: \"z\")",
        ),
        (
            "'[ 1/abc ]'",
            4,
            "Cannot parse value. (For input string: \"abc\")",
        ),
        (
            "'[ 1// ]'",
            5,
            "Cannot parse value. (Index 0 out of bounds for length 0)",
        ),
        (
            "'[ 1/ ]'",
            4,
            "Cannot parse value. (Index 0 out of bounds for length 0)",
        ),
        (
            "'[ 1/2/3/q ]'",
            8,
            "Cannot parse value. (For input string: \"q\")",
        ),
        (
            "\"[ 'a b' ]\"",
            2,
            "Cannot parse value. (For input string: \"'a\")",
        ),
    ] {
        error(text, index, cause);
    }
}

#[test]
fn addvalue_encoders() {
    assert_top(
        "NEWENCODER 1 NaN NaN NaN 42 ADDVALUE 2 48.0 2.0 5 'x' ADDVALUE 3 NaN NaN NaN T SETVALUE \
         4 NaN NaN NaN 1.5 ADDVALUE ->JSON",
        "{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[1,42],[2,47.99999998882413,1.9999999646097422,5,\"x\"],[3,true],[4,1.5]]}",
    );
    assert_top(
        "NEWENCODER [ 1 2 ] ADDVALUE WRAPRAW ->HEX",
        "78034c0204160200",
    );
    // Series and encoders as values are stored as their serialized wrapper.
    assert_top(
        "NEWGTS 1 NaN NaN NaN NEWGTS 'g' RENAME 1 NaN NaN NaN 3 ADDVALUE ADDVALUE VALUES 0 GET ->HEX",
        "1c1801671b00160016001b000068034c020600",
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN NEWENCODER 1 NaN NaN NaN 3 ADDVALUE ADDVALUE VALUES 0 GET ->HEX",
        "78034c020600",
    );
    assert_top(
        "NEWENCODER 1 NaN NaN NaN NEWGTS ADDVALUE WRAPRAW ->HEX",
        "78065a0203780000160200",
    );
    assert_top("NEWGTS [ 1 NEWGTS ] ADDVALUE VALUES 0 GET ->HEX", "780000");
    assert_fails(
        "NEWENCODER 1 NaN NaN NaN COUNTER ADDVALUE",
        "Unsuported value type 'class java.util.concurrent.atomic.AtomicLong'",
    );
    assert_fails(
        "NEWENCODER 1 NaN NaN NaN NULL ADDVALUE",
        "ADDVALUE expects a LONG, DOUBLE, STRING, byte array or BOOLEAN value or a Geo Time Series or ENCODER.",
    );
}

const ENCODER: &str = "NEWENCODER 'e' RENAME { 'k' 'v' } RELABEL 5 NaN NaN NaN 42 ADDVALUE \
    7 48.0 2.0 3 'x' ADDVALUE 9 NaN NaN NaN 1.5 ADDVALUE \
    11 NaN NaN NaN 'hex:00ff' PARSEVALUE ADDVALUE";

#[test]
fn gts_functions_on_encoders() {
    assert_top(&format!("{ENCODER} SIZE"), 4);
    assert_top("NEWENCODER SIZE", 0);
    assert_stack(
        &format!("{ENCODER} CLONE 1 NaN NaN NaN 2 ADDVALUE SIZE SWAP SIZE"),
        &[Value::Long(5), Value::Long(4)],
    );
    assert_top(
        &format!("{ENCODER} CLONEEMPTY ->JSON"),
        "{\"c\":\"e\",\"l\":{\"k\":\"v\"},\"a\":{},\"la\":0,\"v\":[]}",
    );
    assert_top(&format!("[ {ENCODER} ] CLONEEMPTY 0 GET SIZE"), 0);
    assert_top(
        &format!("{ENCODER} TICKLIST"),
        Value::list(vec![
            Value::Long(5),
            Value::Long(7),
            Value::Long(9),
            Value::Long(11),
        ]),
    );
    assert_top(
        &format!("{ENCODER} VALUES"),
        Value::list(vec![
            Value::Long(42),
            Value::from("x"),
            Value::Double(1.5),
            Value::Bytes(vec![0, 255].into()),
        ]),
    );
    assert_top(&format!("{ENCODER} TOSELECTOR"), "e{k=v}");
    assert_top(
        &format!("[ {ENCODER} ] TOSELECTOR"),
        Value::list(vec![Value::from("e{k=v}")]),
    );
    assert_top(
        &format!("{ENCODER} TICKINDEX ->JSON"),
        "{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[[0,42],[1,47.99999998882413,1.9999999646097422,3,\"x\"],[2,1.5],[3,\"\\u0000\\u00FF\"]]}",
    );
    assert_top(&format!("{ENCODER} TICKINDEX NAME"), Value::Null);
}

const CHUNKED: &str = "NEWENCODER 'e' RENAME { 'k' 'v' } RELABEL 5 NaN NaN NaN 42 ADDVALUE \
    7 48.0 2.0 3 'x' ADDVALUE 9 NaN NaN NaN 1.5 ADDVALUE \
    11 NaN NaN NaN 'hex:00ff' PARSEVALUE ADDVALUE 3 NaN NaN NaN T ADDVALUE";

/// An encoder whose second datapoint is truncated.
const CORRUPT: &str = "[ '78044c02044c00' HEX-> ] ->ENCODER";

#[test]
fn time_functions_on_encoders() {
    assert_top(
        &format!("{CHUNKED} 10 4 TIMECLIP ->JSON"),
        "{\"c\":\"e\",\"l\":{\"k\":\"v\"},\"a\":{},\"la\":0,\"v\":[[7,47.99999998882413,1.9999999646097422,3,\"x\"],[9,1.5]]}",
    );
    assert_top(
        &format!(
            "{CHUNKED} '1970-01-01T00:00:00.000009Z' '1970-01-01T00:00:00.000005Z' TIMECLIP \
             TICKLIST ->JSON"
        ),
        "[5,7,9]",
    );
    assert_top(
        &format!("{CHUNKED} [ [ 3 5 ] [ 11 9 ] ] CLIP TICKLIST ->JSON"),
        "[[5,3],[9,11]]",
    );
    assert_fails(
        &format!("{CORRUPT} 10 4 TIMECLIP"),
        "BufferUnderflowException",
    );
    assert_fails(
        &format!("{CORRUPT} [ [ 3 5 ] ] CLIP"),
        "BufferUnderflowException",
    );
    let chunks = |args: &str| {
        top(&format!(
            "{CHUNKED} {args} CHUNK <% DUP LABELS 'c' GET SWAP TICKLIST 2 ->LIST %> F LMAP ->JSON"
        ))
        .to_string()
    };
    for (args, expected) in [
        (
            "0 4 0 0 'c' false",
            r#"[["4",[3]],["8",[5,7]],["12",[9,11]]]"#,
        ),
        (
            "12 4 0 0 'c' true",
            r#"[["4",[3]],["8",[5,7]],["12",[9,11]]]"#,
        ),
        (
            "20 4 0 3 'c' true",
            r#"[["12",[9,11]],["16",[]],["20",[]]]"#,
        ),
        ("20 4 0 3 'c' false", r#"[["12",[9,11]]]"#),
        (
            "0 4 1 0 'c' false",
            r#"[["4",[5,3]],["8",[5,7,9]],["12",[9,11]]]"#,
        ),
        ("8 4 2 2 'c' true", r#"[["4",[5,3]],["8",[5,7,9,3]]]"#),
        ("-3 4 0 0 'c' false", "[]"),
    ] {
        assert_eq!(chunks(args), expected, "{args}");
    }
    assert_top(
        &format!("{CHUNKED} 0 4 0 0 'c' false CHUNK 0 GET ->JSON"),
        "{\"c\":\"e\",\"l\":{\"k\":\"v\",\"c\":\"4\"},\"a\":{},\"la\":0,\"v\":[[3,true]]}",
    );
    assert_top("NEWENCODER 0 4 0 0 'c' false CHUNK SIZE", 0);
    assert_top("NEWENCODER 8 4 0 0 'c' true CHUNK SIZE", 0);
    assert_fails(
        &format!("{CHUNKED} 0 4 5 0 'c' false CHUNK"),
        "Overlap cannot exceed chunk width.",
    );
    assert_fails(
        &format!("{CHUNKED} 0 4 0 0 'k' false CHUNK"),
        "Cannot operate on encoders which already have a label named 'k'",
    );
    assert_fails(&format!("{CHUNKED} 8 0 0 0 'c' false CHUNK"), "/ by zero");
    assert_fails(
        &format!("{CORRUPT} 8 4 0 0 'c' false CHUNK"),
        "BufferUnderflowException",
    );
    assert_eq!(
        chunks("0 2 0 0 'c' false"),
        r#"[["4",[3]],["6",[5]],["8",[7]],["10",[9]],["12",[11]]]"#
    );
    assert_top(
        "NEWENCODER 1 NaN NaN NaN 1 ADDVALUE 9 NaN NaN NaN 1 ADDVALUE 0 2 0 0 'c' false CHUNK \
         <% LABELS 'c' GET %> F LMAP ->JSON",
        r#"["2","10"]"#,
    );
    assert_eq!(
        chunks("20 4 0 0 'c' false"),
        r#"[["4",[3]],["8",[5,7]],["12",[9,11]]]"#
    );
    for function in ["TICKLIST", "VALUES", "TICKINDEX"] {
        assert_fails(&format!("{CORRUPT} {function}"), "BufferUnderflowException");
    }
    assert_fails(
        &format!("{CORRUPT} TICKINDEX"),
        "TICKINDEX caught an exception while converting encoder. (BufferUnderflowException)",
    );
    // Warp 10 loops forever on a chunk at Long.MIN_VALUE (DIVERGENCES §18).
    assert_top(
        "NEWENCODER MINLONG NaN NaN NaN 1 ADDVALUE 0 1 0 0 'c' false CHUNK SIZE",
        1,
    );
    // A chunk id walk stopping at the bottom of the tick range.
    assert_top(
        &format!("{CHUNKED} MINLONG 2 / 4 0 0 'c' true CHUNK SIZE"),
        0,
    );
}

const MA: &str = "NEWENCODER 'a' RENAME { 'x' '1' 'y' '2' } RELABEL 1 NaN NaN NaN 10 ADDVALUE \
    3 NaN NaN NaN 30 ADDVALUE 5 NaN NaN NaN 50 ADDVALUE";
const MB: &str = "NEWENCODER 'b' RENAME 2 NaN NaN NaN 20 ADDVALUE 3 NaN NaN NaN 31 ADDVALUE \
    6 NaN NaN NaN 60 ADDVALUE";
const MG: &str = "NEWGTS 'g' RENAME 3 NaN NaN NaN 32 ADDVALUE 4 NaN NaN NaN 40 ADDVALUE SORT";

#[test]
fn merge_encoders() {
    assert_top(
        &format!("[ {MA} {MB} ] MERGE ->JSON"),
        "{\"c\":\"a\",\"l\":{\"x\":\"1\",\"y\":\"2\"},\"a\":{},\"la\":0,\"v\":[[1,10],[3,30],[5,50],[2,20],[3,31],[6,60]]}",
    );
    assert_top(
        &format!("[ [ {MA} ] {MB} ] MERGE TICKLIST ->JSON"),
        "[1,3,5,2,3,6]",
    );
    assert_fails(
        &format!("[ {MA} {MG} ] MERGE"),
        "MERGE can only operate on homogeneous lists of Geo Time Series or encoders.",
    );
    assert_fails(
        &format!("[ [ {MA} 1 ] ] MERGE"),
        "MERGE expects a list of Geo Time Series or encoders as first parameter.",
    );
    let message = fail_msg(&format!("[ {CORRUPT} ] false MERGE"));
    assert!(message.contains("BufferUnderflowException"), "{message}");
    assert!(!message.contains("merging"), "{message}");
    let mut based = GtsEncoder::new(5);
    based.add_value(7, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(1));
    let mut encoded = based.bytes().to_vec();
    encoded.push(0x4c);
    let wrapper = GtsWrapper {
        encoded: Some(encoded),
        base: Some(5),
        ..GtsWrapper::default()
    };
    let hex = hex_encode(&serialize_wrapper(&wrapper));
    assert_fails(
        &format!("[ NEWENCODER '{hex}' HEX-> UNWRAPENCODER ] MERGE"),
        "BufferUnderflowException",
    );
    // Sorted merges, through a priority queue.
    assert_top(
        "[ 4 3 2 1 ] <% 'i' STORE NEWENCODER $i NaN NaN NaN $i ADDVALUE 5 NaN NaN NaN $i ADDVALUE \
         9 $i - NaN NaN NaN $i ADDVALUE %> FOREACH 4 ->LIST false MERGE VALUES ->JSON",
        "[1,2,3,4,4,4,2,1,3,3,2,1]",
    );
    assert_top(
        &format!("[ {MA} {MB} {MG} ] false MERGE ->JSON"),
        "{\"c\":\"a\",\"l\":{\"x\":\"1\",\"y\":\"2\"},\"a\":{},\"la\":0,\"v\":[[1,10],[2,20],[3,31],[3,30],[3,32],[4,40],[5,50],[6,60]]}",
    );
    assert_top(
        &format!("[ {MG} {MA} [ {MB} ] ] false MERGE VALUES ->JSON"),
        "[10,20,31,30,32,40,50,60]",
    );
    assert_top(
        &format!("[ {MA} {MB} 'x' ] false MERGE TICKLIST ->JSON"),
        "[1,2,3,3,5,6]",
    );
    assert_top(
        "[ ] false MERGE ->JSON",
        r#"{"c":"","l":{},"a":{},"la":0,"v":[]}"#,
    );
    assert_top(
        "[ NEWGTS 'n' RENAME NEWENCODER ] false MERGE ->JSON",
        r#"{"c":"","l":{},"a":{},"la":0,"v":[]}"#,
    );
    assert_top(
        &format!("[ {MG} ] false MERGE ->JSON"),
        r#"{"c":"g","l":{},"a":{},"la":0,"v":[[3,32],[4,40]]}"#,
    );
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE ] true MERGE TICKLIST ->JSON",
        "[1]",
    );
    for (script, cause) in [
        (
            format!("[ {MA} {MB} ] true MERGE"),
            "ENCODER b{} is not sorted in expected order.",
        ),
        (
            format!("[ {MA} 7 NaN NaN NaN 1 ADDVALUE 6 NaN NaN NaN 1 ADDVALUE ] false MERGE"),
            "ENCODER a{x=1,y=2} is not sorted in expected order.",
        ),
        (
            format!("[ [ {MA} 'x' ] ] false MERGE"),
            "expects a list of Geo Time Series or encoders or of lists thereof.",
        ),
        (
            "[ NEWGTS 'u' RENAME { 'b' '1' 'a' '2' } RELABEL 2 NaN NaN NaN 1 ADDVALUE \
             1 NaN NaN NaN 1 ADDVALUE ] false MERGE"
                .to_owned(),
            "GTS u{b=1,a=2} is not sorted in the expected order.",
        ),
        (
            format!("[ {MG} ] true MERGE"),
            "GTS g{} is not sorted in the expected order.",
        ),
        (
            format!("[ {MG} 1 NaN NaN NaN 1 ADDVALUE ] false MERGE"),
            "GTS g{} is not sorted.",
        ),
    ] {
        assert_fails(&script, "MERGE encountered an error while merging.");
        assert_fails(&script, cause);
    }
}
