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
