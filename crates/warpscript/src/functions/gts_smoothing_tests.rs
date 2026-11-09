// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `LTTB`, `TLTTB`, `SINGLEEXPONENTIALSMOOTHING` and
//! `DOUBLEEXPONENTIALSMOOTHING`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const S: &str = r"NEWGTS 's' RENAME { 'k' 'v' } RELABEL 1 0 0 NaN 1 ADDVALUE 3 NaN NaN 10 4 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 7 NaN NaN NaN 3 ADDVALUE 5 NaN NaN NaN 9 ADDVALUE 6 NaN NaN NaN 8.5 ADDVALUE 9 NaN NaN NaN 1 ADDVALUE 10 NaN NaN NaN 0 ADDVALUE";
const D: &str = r"NEWGTS 'd' RENAME 1 NaN NaN NaN 1.5 ADDVALUE 3 NaN NaN NaN -4.25 ADDVALUE 2 NaN NaN NaN 2.0 ADDVALUE 7 NaN NaN NaN 3 ADDVALUE 5 NaN NaN NaN 9 ADDVALUE 6 NaN NaN NaN 8.5 ADDVALUE";
const W: &str = r"NEWGTS 'w' RENAME 0 NaN NaN NaN 0.0 ADDVALUE 11 NaN NaN NaN 3.522 ADDVALUE 22 NaN NaN NaN 6.684 ADDVALUE 30 NaN NaN NaN 9.165 ADDVALUE 41 NaN NaN NaN 10.719 ADDVALUE 52 NaN NaN NaN 11.204 ADDVALUE 60 NaN NaN NaN 10.593 ADDVALUE 71 NaN NaN NaN 8.981 ADDVALUE 82 NaN NaN NaN 6.573 ADDVALUE 90 NaN NaN NaN 3.661 ADDVALUE 101 NaN NaN NaN 0.594 ADDVALUE 112 NaN NaN NaN -2.263 ADDVALUE 120 NaN NaN NaN -4.568 ADDVALUE 131 NaN NaN NaN -6.04 ADDVALUE 142 NaN NaN NaN -6.49 ADDVALUE 150 NaN NaN NaN -5.839 ADDVALUE 161 NaN NaN NaN -4.133 ADDVALUE 172 NaN NaN NaN -1.532 ADDVALUE 180 NaN NaN NaN 1.706 ADDVALUE 191 NaN NaN NaN 5.251 ADDVALUE 202 NaN NaN NaN 8.742 ADDVALUE 210 NaN NaN NaN 11.82 ADDVALUE 221 NaN NaN NaN 14.175 ADDVALUE 232 NaN NaN NaN 15.575 ADDVALUE 240 NaN NaN NaN 15.894 ADDVALUE 251 NaN NaN NaN 15.123 ADDVALUE 262 NaN NaN NaN 13.376 ADDVALUE 270 NaN NaN NaN 10.871 ADDVALUE 281 NaN NaN NaN 7.913 ADDVALUE 292 NaN NaN NaN 4.855 ADDVALUE 300 NaN NaN NaN 2.06 ADDVALUE 311 NaN NaN NaN -0.136 ADDVALUE 322 NaN NaN NaN -1.464 ADDVALUE 330 NaN NaN NaN -1.75 ADDVALUE 341 NaN NaN NaN -0.935 ADDVALUE 352 NaN NaN NaN 0.919 ADDVALUE 360 NaN NaN NaN 3.634 ADDVALUE 371 NaN NaN NaN 6.941 ADDVALUE 382 NaN NaN NaN 10.501 ADDVALUE 390 NaN NaN NaN 13.952 ADDVALUE";
const I: &str = r"NEWGTS 'i' RENAME 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 5 ADDVALUE 3 NaN NaN NaN 2 ADDVALUE 50 NaN NaN NaN 7 ADDVALUE 51 NaN NaN NaN 3 ADDVALUE 52 NaN NaN NaN 9 ADDVALUE 53 NaN NaN NaN 1 ADDVALUE 200 NaN NaN NaN 4 ADDVALUE 201 NaN NaN NaN 8 ADDVALUE 400 NaN NaN NaN 2 ADDVALUE 401 NaN NaN NaN 6 ADDVALUE 402 NaN NaN NaN 0 ADDVALUE 1000 NaN NaN NaN 5 ADDVALUE";
const T2: &str = r"NEWGTS 't' RENAME 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 5.0 ADDVALUE 3 NaN NaN NaN 2.0 ADDVALUE";
const T4: &str = r"NEWGTS 't' RENAME 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 5.0 ADDVALUE 2 NaN NaN NaN 2.0 ADDVALUE 3 NaN NaN NaN 7.0 ADDVALUE";
const STR: &str = r"NEWGTS 'x' RENAME 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE 3 NaN NaN NaN 'c' ADDVALUE 4 NaN NaN NaN 'd' ADDVALUE";
const BOOL: &str = r"NEWGTS 'x' RENAME 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE 3 NaN NaN NaN true ADDVALUE 4 NaN NaN NaN true ADDVALUE";
const E: &str = r"NEWGTS 'e' RENAME";
const ONE: &str = r"NEWGTS 'o' RENAME 5 NaN NaN NaN 3 ADDVALUE";
const BK: &str = r"[ NEWGTS 'b' RENAME 10 NaN NaN NaN 1.0 ADDVALUE 20 NaN NaN NaN 3.0 ADDVALUE 40 NaN NaN NaN 2.0 ADDVALUE bucketizer.last 40 10 0 ] BUCKETIZE 0 GET";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@BOOL", BOOL)
        .replace("@STR", STR)
        .replace("@ONE", ONE)
        .replace("@T2", T2)
        .replace("@T4", T4)
        .replace("@BK", BK)
        .replace("@S", S)
        .replace("@D", D)
        .replace("@W", W)
        .replace("@I", I)
        .replace("@E", E)
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().expect("pop should succeed") {
        Value::String(json) => Ok(json.to_string()),
        other => panic!("not a string: {other:?}"),
    }
}

/// Check every case, reporting all the mismatches at once.
fn check(cases: &[(&str, Result<&str, &str>)]) {
    let mut failures = Vec::new();
    for (template, expected) in cases {
        let script = expand(template);
        match (expected, stack_json(&script)) {
            (Ok(json), Ok(actual)) if actual == *json => {}
            (Err(message), Err(actual)) if actual.contains(message) => {}
            (expected, actual) => failures.push(format!(
                "{template}\n  expected {expected:?}\n  actual   {actual:?}"
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn lttb() {
    check(&[
        (
            r"@W 3 LTTB",
            Ok(r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[52,11.204],[390,13.952]]}]"#),
        ),
        (
            r"@W 4 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[131,-6.04],[240,15.894],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 5 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[52,11.204],[142,-6.49],[240,15.894],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 7 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[52,11.204],[71,8.981],[142,-6.49],[240,15.894],[270,10.871],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 10 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[41,10.719],[52,11.204],[90,3.661],[142,-6.49],[180,1.706],[240,15.894],[270,10.871],[330,-1.75],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 20 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[22,6.684],[30,9.165],[52,11.204],[71,8.981],[90,3.661],[112,-2.263],[142,-6.49],[150,-5.839],[172,-1.532],[191,5.251],[232,15.575],[240,15.894],[262,13.376],[281,7.913],[300,2.06],[330,-1.75],[341,-0.935],[360,3.634],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 38 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[120,-4.568],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[240,15.894],[251,15.123],[262,13.376],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[322,-1.464],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 39 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[120,-4.568],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[210,11.82],[221,14.175],[232,15.575],[240,15.894],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[322,-1.464],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 40 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[120,-4.568],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[240,15.894],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[322,-1.464],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 41 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[120,-4.568],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[240,15.894],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[322,-1.464],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 42 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[120,-4.568],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[240,15.894],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[322,-1.464],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 100 LTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[120,-4.568],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[240,15.894],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[322,-1.464],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@S 4 LTTB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[5,9],[10,0]]}]"#,
            ),
        ),
        (
            r"@S 3 LTTB",
            Ok(r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[5,9],[10,0]]}]"#),
        ),
        (
            r"@S 10 LTTB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[2,2],[7,3],[5,9],[6,8],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"@S 9 LTTB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[2,2],[3,10,4],[5,9],[6,8],[7,3],[9,1],[9,1],[10,0]]}]"#,
            ),
        ),
        (r"@S 2 LTTB", Err(r"Threshold MUST be >= 3.")),
        (r"@S 0 LTTB", Err(r"Threshold MUST be >= 3.")),
        (r"@S -5 LTTB", Err(r"Threshold MUST be >= 3.")),
        (
            r"@S -2147483648 LTTB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[2,2],[7,3],[5,9],[6,8],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"@S 4294967300 LTTB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[5,9],[10,0]]}]"#,
            ),
        ),
        (
            r"@S 4.0 LTTB",
            Err(r"LTTB expects a threshold on top of the stack."),
        ),
        (
            r"@S 'x' LTTB",
            Err(r"LTTB expects a threshold on top of the stack."),
        ),
        (
            r"1 4 LTTB",
            Err(r"LTTB can only operate on Geo Time Series instances."),
        ),
        (
            r"[ @S 1 ] 4 LTTB",
            Err(r"LTTB can only operate on Geo Time Series instances."),
        ),
        (
            r"[ @S @D ] 4 LTTB",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[5,9],[10,0]]},{"c":"d","l":{},"a":{},"la":0,"v":[[1,1.5],[3,-4.25],[5,9.0],[7,3.0]]}]]"#,
            ),
        ),
        (
            r"@D 3 LTTB",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[1,1.5],[3,-4.25],[7,3.0]]}]"#),
        ),
        (
            r"@D 4 LTTB",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[1,1.5],[3,-4.25],[5,9.0],[7,3.0]]}]"#),
        ),
        (
            r"@I 4 LTTB",
            Ok(r#"[{"c":"i","l":{},"a":{},"la":0,"v":[[1,1],[50,7],[52,9],[1000,5]]}]"#),
        ),
        (
            r"@I 6 LTTB",
            Ok(
                r#"[{"c":"i","l":{},"a":{},"la":0,"v":[[1,1],[2,5],[52,9],[53,1],[201,8],[1000,5]]}]"#,
            ),
        ),
        (
            r"@STR 3 LTTB",
            Ok(r#"[{"c":"x","l":{},"a":{},"la":0,"v":[[1,"a"],[2,"b"],[3,"c"],[4,"d"]]}]"#),
        ),
        (
            r"@BOOL 3 LTTB",
            Ok(r#"[{"c":"x","l":{},"a":{},"la":0,"v":[[1,true],[2,false],[3,true],[4,true]]}]"#),
        ),
        (
            r"@STR 2 LTTB",
            Ok(r#"[{"c":"x","l":{},"a":{},"la":0,"v":[[1,"a"],[2,"b"],[3,"c"],[4,"d"]]}]"#),
        ),
        (
            r"@E 3 LTTB",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (r"@E 1 LTTB", Err(r"Threshold MUST be >= 3.")),
        (
            r"@E 2 LTTB",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@ONE 3 LTTB",
            Ok(r#"[{"c":"o","l":{},"a":{},"la":0,"v":[[5,3]]}]"#),
        ),
        (r"@ONE 2 LTTB", Err(r"Threshold MUST be >= 3.")),
        (
            r"@T2 3 LTTB",
            Ok(r#"[{"c":"t","l":{},"a":{},"la":0,"v":[[1,1.0],[2,5.0],[3,2.0]]}]"#),
        ),
        (
            r"@T4 3 LTTB",
            Ok(r#"[{"c":"t","l":{},"a":{},"la":0,"v":[[1,1.0],[2,5.0],[3,7.0]]}]"#),
        ),
        (
            r"@S 4 LTTB DROP @S",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[2,2],[7,3],[5,9],[6,8],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"@S DUP 4 LTTB DROP",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[2,2],[3,10,4],[5,9],[6,8],[7,3],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"@S DUP 20 LTTB DROP",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[2,2],[7,3],[5,9],[6,8],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"@BK 3 LTTB",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,1.0],[20,3.0],[40,2.0]]}]"#),
        ),
        (r"LTTB", Err(r"Empty stack.")),
        (
            r"@S LTTB",
            Err(r"LTTB expects a threshold on top of the stack."),
        ),
        (
            r"@S { 'a' 'b' } SETATTRIBUTES 4 LTTB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{"a":"b"},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[5,9],[10,0]]}]"#,
            ),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN NaN ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN NaN ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 3 LTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN NaN ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN NaN ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 4 LTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[4,5.0],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN NaN ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN NaN ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 5 LTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN Infinity ADDVALUE 3 NaN NaN NaN -Infinity ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN 3.0 ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 3 LTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[2,Infinity],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN Infinity ADDVALUE 3 NaN NaN NaN -Infinity ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN 3.0 ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 4 LTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[2,Infinity],[5,3.0],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN Infinity ADDVALUE 3 NaN NaN NaN -Infinity ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN 3.0 ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 5 LTTB",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[2,Infinity],[3,-Infinity],[5,3.0],[6,2.0]]}]"#,
            ),
        ),
    ]);
}

#[test]
fn tlttb() {
    check(&[
        (
            r"@W 3 TLTTB",
            Ok(r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[330,-1.75],[390,13.952]]}]"#),
        ),
        (
            r"@W 4 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[142,-6.49],[330,-1.75],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 5 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[52,11.204],[240,15.894],[330,-1.75],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 7 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[52,11.204],[142,-6.49],[161,-4.133],[240,15.894],[330,-1.75],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 10 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[41,10.719],[52,11.204],[142,-6.49],[150,-5.839],[240,15.894],[251,15.123],[330,-1.75],[352,0.919],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 20 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[22,6.684],[30,9.165],[52,11.204],[71,8.981],[90,3.661],[112,-2.263],[142,-6.49],[161,-4.133],[180,1.706],[202,8.742],[232,15.575],[251,15.123],[270,10.871],[292,4.855],[330,-1.75],[341,-0.935],[360,3.634],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 38 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 39 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 40 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 41 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[30,9.165],[41,10.719],[52,11.204],[71,8.981],[82,6.573],[101,0.594],[112,-2.263],[131,-6.04],[142,-6.49],[161,-4.133],[172,-1.532],[191,5.251],[202,8.742],[221,14.175],[232,15.575],[251,15.123],[262,13.376],[281,7.913],[292,4.855],[311,-0.136],[330,-1.75],[341,-0.935],[352,0.919],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 42 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[120,-4.568],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[240,15.894],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[322,-1.464],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@W 100 TLTTB",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,3.522],[22,6.684],[30,9.165],[41,10.719],[52,11.204],[60,10.593],[71,8.981],[82,6.573],[90,3.661],[101,0.594],[112,-2.263],[120,-4.568],[131,-6.04],[142,-6.49],[150,-5.839],[161,-4.133],[172,-1.532],[180,1.706],[191,5.251],[202,8.742],[210,11.82],[221,14.175],[232,15.575],[240,15.894],[251,15.123],[262,13.376],[270,10.871],[281,7.913],[292,4.855],[300,2.06],[311,-0.136],[322,-1.464],[330,-1.75],[341,-0.935],[352,0.919],[360,3.634],[371,6.941],[382,10.501],[390,13.952]]}]"#,
            ),
        ),
        (
            r"@S 4 TLTTB",
            Ok(r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[5,9],[6,8],[10,0]]}]"#),
        ),
        (
            r"@S 3 TLTTB",
            Ok(r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[5,9],[9,1],[10,0]]}]"#),
        ),
        (
            r"@S 5 TLTTB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[5,9],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"@S 10 TLTTB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[2,2],[7,3],[5,9],[6,8],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"@S 9 TLTTB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[2,2],[3,10,4],[5,9],[6,8],[7,3],[9,1],[10,0]]}]"#,
            ),
        ),
        (r"@S 2 TLTTB", Err(r"Threshold MUST be >= 3.")),
        (
            r"@S 4.0 TLTTB",
            Err(r"TLTTB expects a threshold on top of the stack."),
        ),
        (
            r"@D 3 TLTTB",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[1,1.5],[3,-4.25],[6,8.5],[7,3.0]]}]"#),
        ),
        (
            r"@D 4 TLTTB",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[1,1.5],[3,-4.25],[5,9.0],[6,8.5],[7,3.0]]}]"#,
            ),
        ),
        (
            r"@I 3 TLTTB",
            Ok(r#"[{"c":"i","l":{},"a":{},"la":0,"v":[[1,1],[52,9],[1000,5]]}]"#),
        ),
        (
            r"@I 4 TLTTB",
            Ok(r#"[{"c":"i","l":{},"a":{},"la":0,"v":[[1,1],[52,9],[1000,5]]}]"#),
        ),
        (
            r"@I 5 TLTTB",
            Ok(r#"[{"c":"i","l":{},"a":{},"la":0,"v":[[1,1],[52,9],[401,6],[1000,5]]}]"#),
        ),
        (
            r"@I 6 TLTTB",
            Ok(r#"[{"c":"i","l":{},"a":{},"la":0,"v":[[1,1],[52,9],[401,6],[1000,5]]}]"#),
        ),
        (
            r"@I 8 TLTTB",
            Ok(r#"[{"c":"i","l":{},"a":{},"la":0,"v":[[1,1],[52,9],[201,8],[401,6],[1000,5]]}]"#),
        ),
        (
            r"@I 12 TLTTB",
            Ok(
                r#"[{"c":"i","l":{},"a":{},"la":0,"v":[[1,1],[52,9],[201,8],[401,6],[402,0],[1000,5]]}]"#,
            ),
        ),
        (
            r"@STR 3 TLTTB",
            Ok(r#"[{"c":"x","l":{},"a":{},"la":0,"v":[[1,"a"],[2,"b"],[3,"c"],[4,"d"]]}]"#),
        ),
        (
            r"@E 3 TLTTB",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@ONE 3 TLTTB",
            Ok(r#"[{"c":"o","l":{},"a":{},"la":0,"v":[[5,3]]}]"#),
        ),
        (r"@T2 3 TLTTB", Err(r"/ by zero")),
        (r"@T4 3 TLTTB", Err(r"/ by zero")),
        (r"@T4 4 TLTTB", Err(r"/ by zero")),
        (
            r"@BK 3 TLTTB",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,1.0],[20,3.0],[40,2.0]]}]"#),
        ),
        (
            r"[ @S @I ] 3 TLTTB",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[5,9],[9,1],[10,0]]},{"c":"i","l":{},"a":{},"la":0,"v":[[1,1],[52,9],[1000,5]]}]]"#,
            ),
        ),
        (
            r"1 3 TLTTB",
            Err(r"TLTTB can only operate on Geo Time Series instances."),
        ),
        (r"TLTTB", Err(r"Empty stack.")),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 4 NaN NaN NaN 3 ADDVALUE 5 NaN NaN NaN 4 ADDVALUE 3 TLTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1],[2,2],[4,3],[5,4]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 2 ADDVALUE 1 NaN NaN NaN 3 ADDVALUE 1 NaN NaN NaN 4 ADDVALUE 3 TLTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1],[1,3],[1,4]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 3 ADDVALUE 2 NaN NaN NaN 4 ADDVALUE 3 TLTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1],[1,2],[2,3],[2,4]]}]"#),
        ),
        (
            r"NEWGTS -9223372036854775807 NaN NaN NaN 1 ADDVALUE 0 NaN NaN NaN 2 ADDVALUE 9223372036854775807 NaN NaN NaN 3 ADDVALUE 1 NaN NaN NaN 4 ADDVALUE 3 TLTTB",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[-9223372036854775807,1],[1,4],[9223372036854775807,3]]}]"#,
            ),
        ),
        (
            r"NEWGTS -9223372036854775807 NaN NaN NaN 1 ADDVALUE 0 NaN NaN NaN 2 ADDVALUE 9223372036854775807 NaN NaN NaN 3 ADDVALUE 1 NaN NaN NaN 4 ADDVALUE 3 LTTB",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[-9223372036854775807,1],[1,4],[9223372036854775807,3]]}]"#,
            ),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN NaN ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN NaN ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 3 TLTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN NaN ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN NaN ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 4 TLTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[4,5.0],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN NaN ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN NaN ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 5 TLTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN Infinity ADDVALUE 3 NaN NaN NaN -Infinity ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN 3.0 ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 3 TLTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[2,Infinity],[5,3.0],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN Infinity ADDVALUE 3 NaN NaN NaN -Infinity ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN 3.0 ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 4 TLTTB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[2,Infinity],[4,5.0],[6,2.0]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN Infinity ADDVALUE 3 NaN NaN NaN -Infinity ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN 3.0 ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 5 TLTTB",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[2,Infinity],[3,-Infinity],[4,5.0],[5,3.0],[6,2.0]]}]"#,
            ),
        ),
    ]);
}

#[test]
fn single_exponential_smoothing() {
    check(&[
        (
            r"@S 0.5 SINGLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1.0],[2,1.5],[3,10,2.75],[5,5.875],[6,6.9375],[7,4.96875],[9,2.984375],[10,1.4921875]]}]"#,
            ),
        ),
        (
            r"@S 0.1 SINGLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1.0],[2,1.1],[3,10,1.3900000000000001],[5,2.1510000000000002],[6,2.7359],[7,2.7623100000000003],[9,2.5860790000000002],[10,2.3274711000000003]]}]"#,
            ),
        ),
        (
            r"@D 0.9 SINGLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[1,1.5],[2,1.95],[3,-3.6300000000000003],[5,7.737],[6,8.4237],[7,3.54237]]}]"#,
            ),
        ),
        (
            r"@W 0.3 SINGLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[{"c":"w","l":{},"a":{},"la":0,"v":[[0,0.0],[11,1.0566],[22,2.74482],[30,4.6708739999999995],[41,6.485311799999999],[52,7.900918259999999],[60,8.708542781999999],[71,8.790279947399998],[82,8.125095963179998],[90,6.785867174225999],[101,4.928307021958199],[112,2.770914915370739],[120,0.5692404407595173],[131,-1.4135316914683378],[142,-2.9364721840278367],[150,-3.8072305288194856],[161,-3.90496137017364],[172,-3.193072959121548],[180,-1.7233510713850833],[191,0.3689542500304419],[202,2.8808679750213098],[210,5.562607582514916],[221,8.146325307760442],[232,10.374927715432309],[240,12.030649400802616],[251,12.95835458056183],[262,13.083648206393281],[270,12.419853744475297],[281,11.067797621132708],[292,9.203958334792896],[300,7.060770834355027],[311,4.901739584048518],[322,2.9920177088339623],[330,1.5694123961837736],[341,0.8180886773286415],[352,0.848362074130049],[360,1.6840534518910342],[371,3.2611374163237237],[382,5.433096191426606],[390,7.988767333998624]]}]"#,
            ),
        ),
        (
            r"@S 1 SINGLEEXPONENTIALSMOOTHING",
            Err(r"'alpha' (smoothing factor) should be strictly between 0.0 and 1.0."),
        ),
        (
            r"@S 0 SINGLEEXPONENTIALSMOOTHING",
            Err(r"'alpha' (smoothing factor) should be strictly between 0.0 and 1.0."),
        ),
        (
            r"@S 1.5 SINGLEEXPONENTIALSMOOTHING",
            Err(r"'alpha' (smoothing factor) should be strictly between 0.0 and 1.0."),
        ),
        (
            r"@S -0.5 SINGLEEXPONENTIALSMOOTHING",
            Err(r"'alpha' (smoothing factor) should be strictly between 0.0 and 1.0."),
        ),
        (
            r"@S NaN SINGLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1.0],[2,NaN],[3,10,NaN],[5,NaN],[6,NaN],[7,NaN],[9,NaN],[10,NaN]]}]"#,
            ),
        ),
        (
            r"@S 'x' SINGLEEXPONENTIALSMOOTHING",
            Err(
                r"SINGLEEXPONENTIALSMOOTHING expects the 'alpha' parameter (smoothing factor) on the top of the stack.",
            ),
        ),
        (
            r"@STR 0.5 SINGLEEXPONENTIALSMOOTHING",
            Err(r"Can only perform exponential smoothing on numeric Geo Time Series."),
        ),
        (
            r"@BOOL 0.5 SINGLEEXPONENTIALSMOOTHING",
            Err(r"Can only perform exponential smoothing on numeric Geo Time Series."),
        ),
        (
            r"@E 0.5 SINGLEEXPONENTIALSMOOTHING",
            Err(r"Can only perform exponential smoothing on numeric Geo Time Series."),
        ),
        (
            r"@ONE 0.5 SINGLEEXPONENTIALSMOOTHING",
            Err(
                r"Can only perform exponential smoothing on Geo Time Series containing at least two values.",
            ),
        ),
        (
            r"@T2 0.5 SINGLEEXPONENTIALSMOOTHING",
            Ok(r#"[{"c":"t","l":{},"a":{},"la":0,"v":[[1,1.0],[2,3.0],[3,2.5]]}]"#),
        ),
        (
            r"[ @S @D ] 0.5 SINGLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1.0],[2,1.5],[3,10,2.75],[5,5.875],[6,6.9375],[7,4.96875],[9,2.984375],[10,1.4921875]]},{"c":"d","l":{},"a":{},"la":0,"v":[[1,1.5],[2,1.75],[3,-1.25],[5,3.875],[6,6.1875],[7,4.59375]]}]]"#,
            ),
        ),
        (
            r"[ @S 1 ] 0.5 SINGLEEXPONENTIALSMOOTHING",
            Err(r"SINGLEEXPONENTIALSMOOTHING can only operate on Geo Time Series instances."),
        ),
        (
            r"1 0.5 SINGLEEXPONENTIALSMOOTHING",
            Err(r"SINGLEEXPONENTIALSMOOTHING can only operate on Geo Time Series instances."),
        ),
        (
            r"@S 0.5 SINGLEEXPONENTIALSMOOTHING DROP @S",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[2,2],[7,3],[5,9],[6,8],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"@BK 0.5 SINGLEEXPONENTIALSMOOTHING",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,1.0],[20,2.0],[40,2.0]]}]"#),
        ),
        (
            r"@BK 0.5 SINGLEEXPONENTIALSMOOTHING DUP BUCKETSPAN SWAP DUP BUCKETCOUNT SWAP LASTBUCKET",
            Ok(r"[40,4,10]"),
        ),
        (r"SINGLEEXPONENTIALSMOOTHING", Err(r"Empty stack.")),
        (
            r"@S { 'a' 'b' } SETATTRIBUTES 0.5 SINGLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{"a":"b"},"la":0,"v":[[1,0.0,0.0,1.0],[2,1.5],[3,10,2.75],[5,5.875],[6,6.9375],[7,4.96875],[9,2.984375],[10,1.4921875]]}]"#,
            ),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN NaN ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN NaN ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 0.5 SINGLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[2,NaN],[3,NaN],[4,NaN],[5,NaN],[6,NaN]]}]"#,
            ),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN Infinity ADDVALUE 3 NaN NaN NaN -Infinity ADDVALUE 4 NaN NaN NaN 5.0 ADDVALUE 5 NaN NaN NaN 3.0 ADDVALUE 6 NaN NaN NaN 2.0 ADDVALUE 0.5 SINGLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.0],[2,Infinity],[3,NaN],[4,NaN],[5,NaN],[6,NaN]]}]"#,
            ),
        ),
    ]);
}

#[test]
fn double_exponential_smoothing() {
    check(&[
        (
            r"@S 0.5 0.3 DOUBLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2.0],[3,10,3.5],[5,6.825],[6,8.313749999999999],[7,6.5110624999999995],[9,4.0830593749999995],[10,1.9065989062499997]]},{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,1.0],[3,10,1.15],[5,1.8025],[6,1.7083749999999995],[7,0.6550562499999998],[9,-0.2698615625000002],[10,-0.841841234375]]}]]"#,
            ),
        ),
        (
            r"@D 0.9 0.1 DOUBLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[[{"c":"d","l":{},"a":{},"la":0,"v":[[2,2.0],[3,-3.575],[5,7.73175],[6,8.5265675],[7,3.653658175]]},{"c":"d","l":{},"a":{},"la":0,"v":[[2,0.5],[3,-0.10749999999999998],[5,1.0339250000000002],[6,1.0100142500000002],[7,0.4217218925000002]]}]]"#,
            ),
        ),
        (
            r"@W 0.3 0.6 DOUBLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[[{"c":"w","l":{},"a":{},"la":0,"v":[[11,3.522],[22,6.936],[30,10.02474],[41,12.4983048],[52,14.055025295999998],[60,14.448245089919999],[71,13.545954829478399],[82,11.370259777863167],[90,8.110166481717133],[101,4.107151207705827],[112,-0.1844267014891366],[120,-4.254174431657566],[131,-7.6010864450771045],[142,-9.797929294356901],[150,-10.544992015868516],[161,-9.709057358070314],[172,-7.339912773158916],[180,-3.6646872645523327],[191,0.9381942990916956],[202,5.98381641980601],[210,10.935624948740948],[221,15.267578428222034],[232,18.523281746778828],[240,20.36728335534839],[251,20.621593477384376],[262,19.285763736880376],[270,16.535425445889107],[281,12.70319206193518],[292,8.240994122019098],[300,3.669476622114402],[311,-0.4790914197994767],[322,-3.7197325935752854],[330,-5.6679495483748],[341,-6.081970498026996],[352,-4.889130473138674],[360,-2.194178970551887],[371,1.7334592959582036],[382,6.488163409242789],[390,11.574066874878298]]},{"c":"w","l":{},"a":{},"la":0,"v":[[11,3.522],[22,3.4572000000000003],[30,3.236124],[41,2.77858848],[52,2.0454676895999993],[60,1.0541189521919998],[71,-0.11972657538815984],[82,-1.3533076611244033],[90,-2.497379042137381],[101,-3.400760781261736],[112,-3.935251058021672],[120,-4.015949061309726],[131,-3.6145268325756135],[142,-2.7639164425981235],[150,-1.5538042099462182],[161,-0.11996088929956628],[172,1.3735023952270122],[180,2.7545362632547548],[191,3.863543443488319],[202,4.572790649823917],[210,4.800201377290529],[221,4.519252638604863],[232,3.7611230465760217],[240,2.610850183772147],[251,1.19692614673045],[262,-0.3227273856102199],[270,-1.7792939288388496],[281,-3.011057601907896],[292,-3.881741804712808],[300,-4.29560722182794],[311,-4.207383713879503],[322,-3.6273381898172863],[330,-2.619865448806623],[341,-1.2963587493139668],[352,0.1971605152074064],[360,1.6958351076350346],[371,3.034917002960068],[382,4.0667892691547785],[390,4.678257787043217]]}]]"#,
            ),
        ),
        (
            r"@T2 0.5 0.5 DOUBLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[[{"c":"t","l":{},"a":{},"la":0,"v":[[2,5.0],[3,5.5]]},{"c":"t","l":{},"a":{},"la":0,"v":[[2,4.0],[3,2.25]]}]]"#,
            ),
        ),
        (
            r"@S 1 0.3 DOUBLEEXPONENTIALSMOOTHING",
            Err(r"'alpha' (smoothing factor) should be strictly between 0.0 and 1.0."),
        ),
        (
            r"@S 0.5 1 DOUBLEEXPONENTIALSMOOTHING",
            Err(r"'beta' (trend factor) should be strictly between 0.0 and 1.0."),
        ),
        (
            r"@S 0.5 0 DOUBLEEXPONENTIALSMOOTHING",
            Err(r"'beta' (trend factor) should be strictly between 0.0 and 1.0."),
        ),
        (
            r"@S 0 0 DOUBLEEXPONENTIALSMOOTHING",
            Err(r"'alpha' (smoothing factor) should be strictly between 0.0 and 1.0."),
        ),
        (
            r"@S NaN NaN DOUBLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2.0],[3,10,NaN],[5,NaN],[6,NaN],[7,NaN],[9,NaN],[10,NaN]]},{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,1.0],[3,10,NaN],[5,NaN],[6,NaN],[7,NaN],[9,NaN],[10,NaN]]}]]"#,
            ),
        ),
        (
            r"@S 0.5 'x' DOUBLEEXPONENTIALSMOOTHING",
            Err(
                r"DOUBLEEXPONENTIALSMOOTHING expects the 'beta' parameter (trend factor) on the top of the stack.",
            ),
        ),
        (
            r"@S 'x' 0.5 DOUBLEEXPONENTIALSMOOTHING",
            Err(
                r"DOUBLEEXPONENTIALSMOOTHING expects the 'alpha' parameter (smoothing factor) at the second level of the stack.",
            ),
        ),
        (
            r"@S 'x' 'y' DOUBLEEXPONENTIALSMOOTHING",
            Err(
                r"DOUBLEEXPONENTIALSMOOTHING expects the 'beta' parameter (trend factor) on the top of the stack.",
            ),
        ),
        (
            r"@S 'x' 2 DOUBLEEXPONENTIALSMOOTHING",
            Err(
                r"DOUBLEEXPONENTIALSMOOTHING expects the 'alpha' parameter (smoothing factor) at the second level of the stack.",
            ),
        ),
        (
            r"@STR 0.5 0.5 DOUBLEEXPONENTIALSMOOTHING",
            Err(r"Can only perform exponential smoothing on numeric Geo Time Series."),
        ),
        (
            r"@E 0.5 0.5 DOUBLEEXPONENTIALSMOOTHING",
            Err(r"Can only perform exponential smoothing on numeric Geo Time Series."),
        ),
        (
            r"@ONE 0.5 0.5 DOUBLEEXPONENTIALSMOOTHING",
            Err(
                r"Can only perform exponential smoothing on Geo Time Series containing at least two values.",
            ),
        ),
        (
            r"[ @S @D ] 0.5 0.5 DOUBLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[[[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2.0],[3,10,3.5],[5,6.875],[6,8.59375],[7,6.8046875],[9,3.958984375],[10,1.29638671875]]},{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,1.0],[3,10,1.25],[5,2.3125],[6,2.015625],[7,0.11328125],[9,-1.3662109375],[10,-2.014404296875]]}],[{"c":"d","l":{},"a":{},"la":0,"v":[[2,2.0],[3,-0.875],[5,3.46875],[6,6.7734375],[7,6.107421875]]},{"c":"d","l":{},"a":{},"la":0,"v":[[2,0.5],[3,-1.1875],[5,1.578125],[6,2.44140625],[7,0.8876953125]]}]]]"#,
            ),
        ),
        (
            r"[ @S 1 ] 0.5 0.5 DOUBLEEXPONENTIALSMOOTHING",
            Err(r"DOUBLEEXPONENTIALSMOOTHING can only operate on Geo Time Series instances."),
        ),
        (
            r"1 0.5 0.5 DOUBLEEXPONENTIALSMOOTHING",
            Err(r"DOUBLEEXPONENTIALSMOOTHING can only operate on Geo Time Series instances."),
        ),
        (
            r"@S 0.5 0.5 DOUBLEEXPONENTIALSMOOTHING DROP @S",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[2,2],[7,3],[5,9],[6,8],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"@BK 0.5 0.5 DOUBLEEXPONENTIALSMOOTHING",
            Ok(
                r#"[[{"c":"b","l":{},"a":{},"la":0,"v":[[20,3.0],[40,3.5]]},{"c":"b","l":{},"a":{},"la":0,"v":[[20,2.0],[40,1.25]]}]]"#,
            ),
        ),
        (r"DOUBLEEXPONENTIALSMOOTHING", Err(r"Empty stack.")),
        (r"0.5 DOUBLEEXPONENTIALSMOOTHING", Err(r"Empty stack.")),
    ]);
}
