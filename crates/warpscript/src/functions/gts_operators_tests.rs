// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the operators on Geo Time Series: arithmetic, comparisons, logical, bitwise and
//! bit conversion operators, and the numerical functions (`ABS`, `MAX`, `ATAN2`…).
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const L: &str = r"NEWGTS 'l' RENAME { 'k' 'v' } RELABEL 10 NaN NaN NaN 1 ADDVALUE 30 1.0 2.0 5 3 ADDVALUE 20 NaN NaN NaN -2 ADDVALUE 40 NaN NaN NaN 0 ADDVALUE";
const M: &str = r"NEWGTS 'm' RENAME 40 NaN NaN NaN 4 ADDVALUE 20 3.0 4.0 NaN 2 ADDVALUE 20 NaN NaN NaN 7 ADDVALUE 10 NaN NaN NaN 5 ADDVALUE 25 NaN NaN NaN 9 ADDVALUE";
const D: &str = r"NEWGTS 'd' RENAME 10 NaN NaN NaN 1.5 ADDVALUE 20 NaN NaN NaN NaN ADDVALUE 30 NaN NaN NaN -0.0 ADDVALUE 50 3.0 4.0 NaN 2.0 ADDVALUE";
const F: &str = r"NEWGTS 'f' RENAME 10 NaN NaN NaN NaN ADDVALUE 20 NaN NaN NaN NaN ADDVALUE 30 NaN NaN NaN 0.0 ADDVALUE 40 NaN NaN NaN 1.0 ADDVALUE";
const S: &str = r"NEWGTS 's' RENAME 10 NaN NaN NaN 'b' ADDVALUE 20 NaN NaN NaN 'a' ADDVALUE 30 1.0 1.0 NaN 'c' ADDVALUE";
const T: &str = r"NEWGTS 't' RENAME 10 NaN NaN NaN 'b' ADDVALUE 20 NaN NaN NaN 'b' ADDVALUE 30 NaN NaN NaN 'a' ADDVALUE";
const U: &str = r"NEWGTS 'u' RENAME 10 NaN NaN NaN '😀' ADDVALUE 20 NaN NaN NaN 'ﬀ' ADDVALUE";
const B: &str = r"NEWGTS 'b' RENAME 10 NaN NaN NaN true ADDVALUE 20 NaN NaN NaN false ADDVALUE 30 1.0 1.0 NaN true ADDVALUE";
const C: &str = r"NEWGTS 'c' RENAME 20 NaN NaN NaN true ADDVALUE 30 NaN NaN NaN false ADDVALUE 40 NaN NaN NaN true ADDVALUE";
const E: &str = r"NEWGTS 'e' RENAME";
const X: &str =
    r"NEWGTS 'x' RENAME 9 NaN NaN NaN 9223372036854775807 ADDVALUE 10 NaN NaN NaN 1 ADDVALUE";
const BL: &str = r"NEWGTS 'bl' RENAME 100 NaN NaN NaN 1 ADDVALUE 200 NaN NaN NaN 2 ADDVALUE 300 NaN NaN NaN 3 ADDVALUE [ SWAP bucketizer.last 300 100 3 ] BUCKETIZE 0 GET";
const BM: &str = r"NEWGTS 'bm' RENAME 200 NaN NaN NaN 20 ADDVALUE 300 NaN NaN NaN 30 ADDVALUE 500 NaN NaN NaN 50 ADDVALUE [ SWAP bucketizer.last 500 100 4 ] BUCKETIZE 0 GET";
const BN: &str = r"NEWGTS 'bn' RENAME 150 NaN NaN NaN 1 ADDVALUE 250 NaN NaN NaN 2 ADDVALUE [ SWAP bucketizer.last 350 100 3 ] BUCKETIZE 0 GET";
const BS: &str = r"NEWGTS 'bs' RENAME 200 NaN NaN NaN 1 ADDVALUE 300 NaN NaN NaN 2 ADDVALUE [ SWAP bucketizer.last 300 50 4 ] BUCKETIZE 0 GET";
const BB: &str = r"NEWGTS 'bb' RENAME 100 NaN NaN NaN true ADDVALUE 200 NaN NaN NaN false ADDVALUE [ SWAP bucketizer.last 300 100 3 ] BUCKETIZE 0 GET";
const INFO: &str = r"'r' STORE [ $r $r BUCKETSPAN $r LASTBUCKET $r BUCKETCOUNT ]";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@INFO", INFO)
        .replace("@BL", BL)
        .replace("@BM", BM)
        .replace("@BN", BN)
        .replace("@BS", BS)
        .replace("@BB", BB)
        .replace("@L", L)
        .replace("@M", M)
        .replace("@D", D)
        .replace("@F", F)
        .replace("@S", S)
        .replace("@T", T)
        .replace("@U", U)
        .replace("@B", B)
        .replace("@C", C)
        .replace("@E", E)
        .replace("@X", X)
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().unwrap() {
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
fn add() {
    check(&[
        (
            r"@L 2 +",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[30,0.9999999823048711,1.9999999646097422,5,5],[20,0],[40,2]]}]"#,
            ),
        ),
        (
            r"2 @L +",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[30,0.9999999823048711,1.9999999646097422,5,5],[20,0],[40,2]]}]"#,
            ),
        ),
        (
            r"@L 2.5 +",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,3.5],[30,0.9999999823048711,1.9999999646097422,5,5.5],[20,0.5],[40,2.5]]}]"#,
            ),
        ),
        (
            r"@D 2 +",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,3.5],[20,NaN],[30,2.0],[50,2.999999988824129,3.9999999292194843,4.0]]}]"#,
            ),
        ),
        (
            r"@D 2.5 +",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,4.0],[20,NaN],[30,2.5],[50,2.999999988824129,3.9999999292194843,4.5]]}]"#,
            ),
        ),
        (
            r"@L 'x' +",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,"1x"],[30,0.9999999823048711,1.9999999646097422,5,"3x"],[20,"-2x"],[40,"0x"]]}]"#,
            ),
        ),
        (
            r"'x' @L +",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,"x1"],[30,0.9999999823048711,1.9999999646097422,5,"x3"],[20,"x-2"],[40,"x0"]]}]"#,
            ),
        ),
        (
            r"@D 'x' +",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,"1.5x"],[20,"NaNx"],[30,"-0.0x"],[50,2.999999988824129,3.9999999292194843,"2.0x"]]}]"#,
            ),
        ),
        (
            r"@S 1 +",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"b1"],[20,"a1"],[30,0.9999999823048711,0.9999999403953552,"c1"]]}]"#,
            ),
        ),
        (
            r"@S 1.5 +",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"b1.5"],[20,"a1.5"],[30,0.9999999823048711,0.9999999403953552,"c1.5"]]}]"#,
            ),
        ),
        (
            r"@S 'z' +",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"bz"],[20,"az"],[30,0.9999999823048711,0.9999999403953552,"cz"]]}]"#,
            ),
        ),
        (
            r"'z' @S +",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"zb"],[20,"za"],[30,0.9999999823048711,0.9999999403953552,"zc"]]}]"#,
            ),
        ),
        (
            r"@B 1 +",
            Err(r"+ cannot operate on BOOLEAN Geo Time Series."),
        ),
        (
            r"@B 1.5 +",
            Err(r"+ cannot operate on BOOLEAN Geo Time Series."),
        ),
        (
            r"@B 'x' +",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,"truex"],[20,"falsex"],[30,0.9999999823048711,0.9999999403953552,"truex"]]}]"#,
            ),
        ),
        (
            r"'x' @B +",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,"xtrue"],[20,"xfalse"],[30,0.9999999823048711,0.9999999403953552,"xtrue"]]}]"#,
            ),
        ),
        (
            r"@L [ 'x' ] +",
            Err(r"+ can only be used with String or numeric types with a GTS."),
        ),
        (r"[ 'x' ] @L + SIZE", Ok(r"[2]")),
        (
            r"@L true +",
            Err(r"+ can only be used with String or numeric types with a GTS."),
        ),
        (
            r"@L NULL +",
            Err(r"+ can only be used with String or numeric types with a GTS."),
        ),
        (
            r"<% %> @L +",
            Err(r"+ can only be used with String or numeric types with a GTS."),
        ),
        (
            r"@L <% %> +",
            Err(r"+ can only be used with String or numeric types with a GTS."),
        ),
        (r"@E 1 +", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@E 1.5 +",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E 'x' +",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E [ ] +",
            Err(r"+ can only be used with String or numeric types with a GTS."),
        ),
        (
            r"@L @M +",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,6],[20,5],[40,4]]}]"#),
        ),
        (
            r"@M @L +",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,6],[20,5],[40,4]]}]"#),
        ),
        (
            r"@L @D +",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,2.5],[20,NaN],[30,3.0]]}]"#),
        ),
        (
            r"@D @L +",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,2.5],[20,NaN],[30,3.0]]}]"#),
        ),
        (
            r"@L @S +",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"1b"],[20,"-2a"],[30,"3c"]]}]"#),
        ),
        (
            r"@S @L +",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"b1"],[20,"a-2"],[30,"c3"]]}]"#),
        ),
        (
            r"@S @T +",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"bb"],[20,"ab"],[30,"ca"]]}]"#),
        ),
        (
            r"@D @S +",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"1.5b"],[20,"NaNa"],[30,"-0.0c"]]}]"#),
        ),
        (
            r"@L @B +",
            Err(r"+ cannot operate on BOOLEAN Geo Time Series."),
        ),
        (
            r"@B @L +",
            Err(r"+ cannot operate on BOOLEAN Geo Time Series."),
        ),
        (r"@E @E +", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @L +", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@L @E +", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@B @E +",
            Err(r"+ cannot operate on BOOLEAN Geo Time Series."),
        ),
        (r"@E @S +", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L 'a' STORE $a $a + $a",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[20,-2],[30,0.9999999823048711,1.9999999646097422,5,3],[40,0]]},{"c":"","l":{},"a":{},"la":0,"v":[[10,2],[20,-4],[30,6],[40,0]]}]"#,
            ),
        ),
        (
            r"@L 'a' STORE @M 'b' STORE $a $b + DROP $a $b",
            Ok(
                r#"[{"c":"m","l":{},"a":{},"la":0,"v":[[10,5],[20,7],[20,2.999999988824129,3.9999999292194843,2],[25,9],[40,4]]},{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[20,-2],[30,0.9999999823048711,1.9999999646097422,5,3],[40,0]]}]"#,
            ),
        ),
        (
            r"@X 1 +",
            Ok(r#"[{"c":"x","l":{},"a":{},"la":0,"v":[[9,-9223372036854775808],[10,2]]}]"#),
        ),
        (
            r"@X @X +",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[9,-2],[10,2]]}]"#),
        ),
        (r"1 2 +", Ok(r"[3]")),
        (r"'a' 'b' +", Ok(r#"["ab"]"#)),
        (r"[ ] 1 +", Ok(r"[[1]]")),
        (
            r"1 'a' +",
            Err(
                r"+ can only operate on numeric, string, lists, matrices, vectors, Geo Time Series, byte array and macro values.",
            ),
        ),
        (
            r"@L 'ab' 'UTF-8' ->BYTES +",
            Err(r"+ can only be used with String or numeric types with a GTS."),
        ),
    ]);
}

#[test]
fn sub_mul_div() {
    check(&[
        (
            r"@L 2 -",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,-1],[30,0.9999999823048711,1.9999999646097422,5,1],[20,-4],[40,-2]]}]"#,
            ),
        ),
        (
            r"2 @L -",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,-1],[20,4],[40,2]]}]"#,
            ),
        ),
        (
            r"@L 2.5 -",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,-1.5],[30,0.9999999823048711,1.9999999646097422,5,0.5],[20,-4.5],[40,-2.5]]}]"#,
            ),
        ),
        (
            r"2.5 @L -",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1.5],[30,0.9999999823048711,1.9999999646097422,5,-0.5],[20,4.5],[40,2.5]]}]"#,
            ),
        ),
        (
            r"@D 1 -",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,0.5],[20,NaN],[30,-1.0],[50,2.999999988824129,3.9999999292194843,1.0]]}]"#,
            ),
        ),
        (
            r"1 @D -",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,-0.5],[20,NaN],[30,1.0],[50,2.999999988824129,3.9999999292194843,-1.0]]}]"#,
            ),
        ),
        (
            r"@S 1 -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@B 1 -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (r"@E 1 -", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"1 @E -", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@E 'x' -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@L 'x' -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"'x' @L -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@L @M -",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,-4],[20,-9],[40,-4]]}]"#),
        ),
        (
            r"@M @L -",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,4],[20,9],[40,4]]}]"#),
        ),
        (
            r"@L @D -",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,-0.5],[20,NaN],[30,3.0]]}]"#),
        ),
        (
            r"@D @L -",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,0.5],[20,NaN],[30,-3.0]]}]"#),
        ),
        (
            r"@L @S -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@S @E -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (r"@E @E -", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@E @S -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@B @L -",
            Err(
                r"- can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (r"@L @E -", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @L -", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L 3 *",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[30,0.9999999823048711,1.9999999646097422,5,9],[20,-6],[40,0]]}]"#,
            ),
        ),
        (
            r"3 @L *",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[30,0.9999999823048711,1.9999999646097422,5,9],[20,-6],[40,0]]}]"#,
            ),
        ),
        (
            r"@L 0.5 *",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,0.5],[30,0.9999999823048711,1.9999999646097422,5,1.5],[20,-1.0],[40,0.0]]}]"#,
            ),
        ),
        (
            r"@D 2 *",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,3.0],[20,NaN],[30,-0.0],[50,2.999999988824129,3.9999999292194843,4.0]]}]"#,
            ),
        ),
        (
            r"@L @M *",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,5],[20,-14],[40,0]]}]"#),
        ),
        (
            r"@L @D *",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1.5],[20,NaN],[30,-0.0]]}]"#),
        ),
        (
            r"@S 2 *",
            Err(
                r"* can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@L 'x' *",
            Err(
                r"* can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@X 2 *",
            Ok(r#"[{"c":"x","l":{},"a":{},"la":0,"v":[[9,-2],[10,2]]}]"#),
        ),
        (
            r"[ 1 ] 2 *",
            Err(
                r"* can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@L [ 1 ] *",
            Err(
                r"* can only operate on numeric values, vectors, matrices and numeric Geo Time Series.",
            ),
        ),
        (
            r"@L 2 /",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,0],[30,0.9999999823048711,1.9999999646097422,5,1],[20,-1],[40,0]]}]"#,
            ),
        ),
        (
            r"120 @M /",
            Ok(
                r#"[{"c":"m","l":{},"a":{},"la":0,"v":[[40,30],[20,2.999999988824129,3.9999999292194843,60],[20,17],[10,24],[25,13]]}]"#,
            ),
        ),
        (
            r"@L 2.0 /",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,0.5],[30,0.9999999823048711,1.9999999646097422,5,1.5],[20,-1.0],[40,0.0]]}]"#,
            ),
        ),
        (
            r"1.0 @L /",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1.0],[30,0.9999999823048711,1.9999999646097422,5,0.3333333333333333],[20,-0.5],[40,Infinity]]}]"#,
            ),
        ),
        (
            r"@D 2 /",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,0.75],[20,NaN],[30,-0.0],[50,2.999999988824129,3.9999999292194843,1.0]]}]"#,
            ),
        ),
        (
            r"2 @D /",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.3333333333333333],[20,NaN],[30,-Infinity],[50,2.999999988824129,3.9999999292194843,1.0]]}]"#,
            ),
        ),
        (r"@L 0 /", Err(r"/ by zero")),
        (r"1 @L /", Err(r"/ by zero")),
        (
            r"@L 0.0 /",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,Infinity],[30,0.9999999823048711,1.9999999646097422,5,Infinity],[20,-Infinity],[40,NaN]]}]"#,
            ),
        ),
        (
            r"@L @M /",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,0],[20,0],[40,0]]}]"#),
        ),
        (r"@M @L /", Err(r"/ by zero")),
        (
            r"@L @D /",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,0.6666666666666666],[20,NaN],[30,-Infinity]]}]"#,
            ),
        ),
        (
            r"@D @L /",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1.5],[20,NaN],[30,-0.0]]}]"#),
        ),
        (
            r"@S 1 /",
            Err(r"/ can only operate on numeric values, vectors and numeric Geo Time Series."),
        ),
        (
            r"@L 'x' /",
            Err(r"/ can only operate on numeric values, vectors and numeric Geo Time Series."),
        ),
        (
            r"[ 1 ] @L /",
            Err(r"/ can only operate on numeric values, vectors and numeric Geo Time Series."),
        ),
        (
            r"@L [ 1 ] /",
            Err(r"/ can only operate on numeric values, vectors and numeric Geo Time Series."),
        ),
        (r"@E 0 /", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @E /", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@X -1 /",
            Ok(r#"[{"c":"x","l":{},"a":{},"la":0,"v":[[9,-9223372036854775807],[10,-1]]}]"#),
        ),
        (r"1 0 /", Err(r"/ by zero")),
        (r"7 2 /", Ok(r"[3]")),
        (r"7 2.0 /", Ok(r"[3.5]")),
        (
            r"'a' 1 /",
            Err(r"/ can only operate on numeric values, vectors and numeric Geo Time Series."),
        ),
        (
            r"@L 'a' STORE $a @M - DROP $a",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[20,-2],[30,0.9999999823048711,1.9999999646097422,5,3],[40,0]]}]"#,
            ),
        ),
    ]);
}

#[test]
fn bucketization() {
    check(&[
        (
            r"@BL @BM + @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[[200,22],[300,33]]},100,500,5]]"#),
        ),
        (
            r"@BL @BN + @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},0,0,0]]"#),
        ),
        (
            r"@BL @BS + @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[[200,3],[300,5]]},0,0,0]]"#),
        ),
        (
            r"@BL @L + @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},0,0,0]]"#),
        ),
        (
            r"@L @BL + @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},0,0,0]]"#),
        ),
        (
            r"@BL 1 + @INFO",
            Ok(r#"[[{"c":"bl","l":{},"a":{},"la":0,"v":[[300,4],[200,3],[100,2]]},100,300,3]]"#),
        ),
        (
            r"1 @BL - @INFO",
            Ok(r#"[[{"c":"bl","l":{},"a":{},"la":0,"v":[[300,-2],[200,-1],[100,0]]},100,300,3]]"#),
        ),
        (
            r"@BL @BM == @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},100,500,5]]"#),
        ),
        (
            r"@BL @E == @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},0,0,0]]"#),
        ),
        (
            r"@BL @BL < @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},100,300,3]]"#),
        ),
        (
            r"@BL @BM & @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[[200,0],[300,2]]},100,500,5]]"#),
        ),
        (
            r"@BL 1000 > @BM == @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},100,500,5]]"#),
        ),
        (
            r"@BL 1000 > @BM 1000 > == @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},100,500,5]]"#),
        ),
        (
            r"@BB @BB && @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[[100,true],[200,false]]},100,300,3]]"#),
        ),
        (
            r"@BB 1000 > @BB || @INFO",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@BL @BM - @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[[200,-18],[300,-27]]},100,500,5]]"#),
        ),
        (
            r"@BL 1000 > @BM * @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},100,500,5]]"#),
        ),
        (
            r"@E @E + @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},0,0,0]]"#),
        ),
        (
            r"@BL @BL 1000 > + @INFO",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]},100,300,3]]"#),
        ),
        (
            r"@BL ->DOUBLEBITS @INFO",
            Ok(
                r#"[[{"c":"bl","l":{},"a":{},"la":0,"v":[[300,4613937818241073152],[200,4611686018427387904],[100,4607182418800017408]]},0,0,0]]"#,
            ),
        ),
        (
            r"@BL ABS @INFO",
            Ok(r#"[[{"c":"bl","l":{},"a":{},"la":0,"v":[[300,3],[200,2],[100,1]]},100,300,3]]"#),
        ),
        (
            r"@BL 2 ** @INFO",
            Ok(r#"[[{"c":"bl","l":{},"a":{},"la":0,"v":[[300,9],[200,4],[100,1]]},100,300,3]]"#),
        ),
        (
            r"@BL NOT",
            Err(r"NOT can only operate on a boolean value or a boolean GTS."),
        ),
        (
            r"@BB NOT @INFO",
            Ok(r#"[[{"c":"bb","l":{},"a":{},"la":0,"v":[[200,true],[100,false]]},100,300,3]]"#),
        ),
        (
            r"@BL 1 & @INFO",
            Ok(r#"[[{"c":"bl","l":{},"a":{},"la":0,"v":[[300,1],[200,0],[100,1]]},100,300,3]]"#),
        ),
    ]);
}

#[test]
fn compare_eq() {
    check(&[
        (
            r"@L 1 ==",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1]]}]"#),
        ),
        (
            r"1 @L ==",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1]]}]"#),
        ),
        (
            r"@L 1.0 ==",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1]]}]"#),
        ),
        (
            r"@L NaN ==",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"NaN @L ==",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@D 1.5 ==",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5]]}]"#),
        ),
        (
            r"1.5 @D ==",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5]]}]"#),
        ),
        (
            r"@D NaN ==",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"NaN @D ==",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"@D 2 ==",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D 0 ==",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[30,-0.0]]}]"#),
        ),
        (
            r"@D 0.0 ==",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@S 'b' ==",
            Ok(r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"b"]]}]"#),
        ),
        (
            r"'b' @S ==",
            Ok(r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"b"]]}]"#),
        ),
        (
            r"@B true ==",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,true],[30,0.9999999823048711,0.9999999403953552,true]]}]"#,
            ),
        ),
        (
            r"true @B ==",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,true],[30,0.9999999823048711,0.9999999403953552,true]]}]"#,
            ),
        ),
        (r"@L @M ==", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@L @D ==", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@D @L ==", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@D @F ==",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"@F @D ==",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"@S @T ==",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"b"]]}]"#),
        ),
        (
            r"@T @S ==",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"b"]]}]"#),
        ),
        (
            r"@U 'ﬀ' ==",
            Ok(r#"[{"c":"u","l":{},"a":{},"la":0,"v":[[20,"\uFB00"]]}]"#),
        ),
        (
            r"@L 'x' ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@S 1 ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@B 1 ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L true ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (r"@E 1 ==", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"1 @E ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@E 'x' ==",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E true ==",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (r"@E [ ] ==", Ok(r"[false]")),
        (r"@L [ ] ==", Ok(r"[false]")),
        (r"[ ] @L ==", Ok(r"[false]")),
        (
            r"@L @S ==",
            Err(r"==can only operate on two GTS with NUMBER or STRING values."),
        ),
        (
            r"@B @C ==",
            Err(r"==can only operate on two GTS with NUMBER or STRING values."),
        ),
        (r"@L @E ==", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @B ==", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L NaN < true ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (r"@L NULL ==", Ok(r"[false]")),
    ]);
}

#[test]
fn compare_ne() {
    check(&[
        (
            r"@L 1 !=",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,5,3],[20,-2],[40,0]]}]"#,
            ),
        ),
        (
            r"1 @L !=",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,5,3],[20,-2],[40,0]]}]"#,
            ),
        ),
        (
            r"@L 1.0 !=",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,5,3],[20,-2],[40,0]]}]"#,
            ),
        ),
        (
            r"@L NaN !=",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,3],[20,-2],[40,0]]}]"#,
            ),
        ),
        (
            r"NaN @L !=",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,3],[20,-2],[40,0]]}]"#,
            ),
        ),
        (
            r"@D 1.5 !=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[20,NaN],[30,-0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"1.5 @D !=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[20,NaN],[30,-0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D NaN !=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[30,-0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"NaN @D !=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[30,-0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D 2 !=",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[20,NaN],[30,-0.0]]}]"#),
        ),
        (
            r"@D 0 !=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[20,NaN],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D 0.0 !=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[20,NaN],[30,-0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@S 'b' !=",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[20,"a"],[30,0.9999999823048711,0.9999999403953552,"c"]]}]"#,
            ),
        ),
        (
            r"'b' @S !=",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[20,"a"],[30,0.9999999823048711,0.9999999403953552,"c"]]}]"#,
            ),
        ),
        (
            r"@B true !=",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[20,false]]}]"#),
        ),
        (
            r"true @B !=",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[20,false]]}]"#),
        ),
        (
            r"@L @M !=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1],[20,-2],[40,0]]}]"#),
        ),
        (
            r"@L @D !=",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1.0],[20,-2.0],[30,0.9999999823048711,1.9999999646097422,5,3.0]]}]"#,
            ),
        ),
        (
            r"@D @L !=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1.5],[20,NaN],[30,-0.0]]}]"#),
        ),
        (
            r"@D @F !=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1.5],[30,-0.0]]}]"#),
        ),
        (
            r"@F @D !=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,NaN],[30,0.0]]}]"#),
        ),
        (
            r"@S @T !=",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,"a"],[30,0.9999999823048711,0.9999999403953552,"c"]]}]"#,
            ),
        ),
        (
            r"@T @S !=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,"b"],[30,"a"]]}]"#),
        ),
        (
            r"@U 'ﬀ' !=",
            Ok(r#"[{"c":"u","l":{},"a":{},"la":0,"v":[[10,"\uD83D\uDE00"]]}]"#),
        ),
        (
            r"@L 'x' !=",
            Err(
                r"!= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@S 1 !=",
            Err(
                r"!= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@B 1 !=",
            Err(
                r"!= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L true !=",
            Err(
                r"!= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (r"@E 1 !=", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"1 @E !=",
            Err(
                r"!= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@E 'x' !=",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E true !=",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (r"@E [ ] !=", Ok(r"[true]")),
        (r"@L [ ] !=", Ok(r"[true]")),
        (r"[ ] @L !=", Ok(r"[true]")),
        (
            r"@L @S !=",
            Err(r"!=can only operate on two GTS with NUMBER or STRING values."),
        ),
        (
            r"@B @C !=",
            Err(r"!=can only operate on two GTS with NUMBER or STRING values."),
        ),
        (r"@L @E !=", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @B !=", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L NaN < true !=",
            Err(
                r"!= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (r"@L NULL !=", Ok(r"[true]")),
    ]);
}

#[test]
fn compare_lt() {
    check(&[
        (
            r"@L 1 <",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[20,-2],[40,0]]}]"#),
        ),
        (
            r"1 @L <",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,5,3]]}]"#,
            ),
        ),
        (
            r"@L 1.0 <",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[20,-2],[40,0]]}]"#),
        ),
        (
            r"@L NaN <",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"NaN @L <",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@D 1.5 <",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[30,-0.0]]}]"#),
        ),
        (
            r"1.5 @D <",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D NaN <",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"NaN @D <",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@D 2 <",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[30,-0.0]]}]"#),
        ),
        (r"@D 0 <", Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@D 0.0 <",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[30,-0.0]]}]"#),
        ),
        (
            r"@S 'b' <",
            Ok(r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[20,"a"]]}]"#),
        ),
        (
            r"'b' @S <",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[30,0.9999999823048711,0.9999999403953552,"c"]]}]"#,
            ),
        ),
        (
            r"@B true <",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[20,false]]}]"#),
        ),
        (
            r"true @B <",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@L @M <",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1],[20,-2],[40,0]]}]"#),
        ),
        (
            r"@L @D <",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1.0]]}]"#),
        ),
        (
            r"@D @L <",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,-0.0]]}]"#),
        ),
        (
            r"@D @F <",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,-0.0]]}]"#),
        ),
        (r"@F @D <", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@S @T <",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,"a"]]}]"#),
        ),
        (
            r"@T @S <",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,"a"]]}]"#),
        ),
        (
            r"@U 'ﬀ' <",
            Ok(r#"[{"c":"u","l":{},"a":{},"la":0,"v":[[10,"\uD83D\uDE00"]]}]"#),
        ),
        (
            r"@L 'x' <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@S 1 <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@B 1 <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L true <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (r"@E 1 <", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"1 @E <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@E 'x' <",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E true <",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E [ ] <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L [ ] <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"[ ] @L <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L @S <",
            Err(r"<can only operate on two GTS with NUMBER or STRING values."),
        ),
        (
            r"@B @C <",
            Err(r"<can only operate on two GTS with NUMBER or STRING values."),
        ),
        (r"@L @E <", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @B <", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L NaN < true <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L NULL <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn compare_gt() {
    check(&[
        (
            r"@L 1 >",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,5,3]]}]"#,
            ),
        ),
        (
            r"1 @L >",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[20,-2],[40,0]]}]"#),
        ),
        (
            r"@L 1.0 >",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,5,3]]}]"#,
            ),
        ),
        (
            r"@L NaN >",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"NaN @L >",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@D 1.5 >",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"1.5 @D >",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[30,-0.0]]}]"#),
        ),
        (
            r"@D NaN >",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"NaN @D >",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (r"@D 2 >", Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@D 0 >",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D 0.0 >",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@S 'b' >",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[30,0.9999999823048711,0.9999999403953552,"c"]]}]"#,
            ),
        ),
        (
            r"'b' @S >",
            Ok(r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[20,"a"]]}]"#),
        ),
        (
            r"@B true >",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"true @B >",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[20,false]]}]"#),
        ),
        (r"@L @M >", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L @D >",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,5,3.0]]}]"#,
            ),
        ),
        (
            r"@D @L >",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1.5]]}]"#),
        ),
        (r"@D @F >", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@F @D >",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,0.0]]}]"#),
        ),
        (
            r"@S @T >",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,0.9999999823048711,0.9999999403953552,"c"]]}]"#,
            ),
        ),
        (
            r"@T @S >",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,"b"]]}]"#),
        ),
        (
            r"@U 'ﬀ' >",
            Ok(r#"[{"c":"u","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@L 'x' >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@S 1 >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@B 1 >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L true >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (r"@E 1 >", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"1 @E >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@E 'x' >",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E true >",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E [ ] >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L [ ] >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"[ ] @L >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L @S >",
            Err(r">can only operate on two GTS with NUMBER or STRING values."),
        ),
        (
            r"@B @C >",
            Err(r">can only operate on two GTS with NUMBER or STRING values."),
        ),
        (r"@L @E >", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @B >", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L NaN < true >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L NULL >",
            Err(
                r"> can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn compare_le() {
    check(&[
        (
            r"@L 1 <=",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[20,-2],[40,0]]}]"#),
        ),
        (
            r"1 @L <=",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,3]]}]"#,
            ),
        ),
        (
            r"@L 1.0 <=",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[20,-2],[40,0]]}]"#),
        ),
        (
            r"@L NaN <=",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"NaN @L <=",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@D 1.5 <=",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[30,-0.0]]}]"#),
        ),
        (
            r"1.5 @D <=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D NaN <=",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"NaN @D <=",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"@D 2 <=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[30,-0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D 0 <=",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[30,-0.0]]}]"#),
        ),
        (
            r"@D 0.0 <=",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[30,-0.0]]}]"#),
        ),
        (
            r"@S 'b' <=",
            Ok(r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"b"],[20,"a"]]}]"#),
        ),
        (
            r"'b' @S <=",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"b"],[30,0.9999999823048711,0.9999999403953552,"c"]]}]"#,
            ),
        ),
        (
            r"@B true <=",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,true],[20,false],[30,0.9999999823048711,0.9999999403953552,true]]}]"#,
            ),
        ),
        (
            r"true @B <=",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,true],[30,0.9999999823048711,0.9999999403953552,true]]}]"#,
            ),
        ),
        (
            r"@L @M <=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1],[20,-2],[40,0]]}]"#),
        ),
        (
            r"@L @D <=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1.0]]}]"#),
        ),
        (
            r"@D @L <=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,-0.0]]}]"#),
        ),
        (
            r"@D @F <=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,NaN],[30,-0.0]]}]"#),
        ),
        (
            r"@F @D <=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"@S @T <=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"b"],[20,"a"]]}]"#),
        ),
        (
            r"@T @S <=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"b"],[30,"a"]]}]"#),
        ),
        (
            r"@U 'ﬀ' <=",
            Ok(r#"[{"c":"u","l":{},"a":{},"la":0,"v":[[10,"\uD83D\uDE00"],[20,"\uFB00"]]}]"#),
        ),
        (
            r"@L 'x' <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@S 1 <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@B 1 <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L true <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (r"@E 1 <=", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"1 @E <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@E 'x' <=",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E true <=",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E [ ] <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L [ ] <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"[ ] @L <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L @S <=",
            Err(r"<=can only operate on two GTS with NUMBER or STRING values."),
        ),
        (
            r"@B @C <=",
            Err(r"<=can only operate on two GTS with NUMBER or STRING values."),
        ),
        (r"@L @E <=", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @B <=", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L NaN < true <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L NULL <=",
            Err(
                r"<= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn compare_ge() {
    check(&[
        (
            r"@L 1 >=",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,3]]}]"#,
            ),
        ),
        (
            r"1 @L >=",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[20,-2],[40,0]]}]"#),
        ),
        (
            r"@L 1.0 >=",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,3]]}]"#,
            ),
        ),
        (
            r"@L NaN >=",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"NaN @L >=",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@D 1.5 >=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"1.5 @D >=",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[30,-0.0]]}]"#),
        ),
        (
            r"@D NaN >=",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"NaN @D >=",
            Ok(r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"@D 2 >=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D 0 >=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[30,-0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@D 0.0 >=",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@S 'b' >=",
            Ok(
                r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"b"],[30,0.9999999823048711,0.9999999403953552,"c"]]}]"#,
            ),
        ),
        (
            r"'b' @S >=",
            Ok(r#"[{"c":"s","l":{},"a":{},"la":0,"v":[[10,"b"],[20,"a"]]}]"#),
        ),
        (
            r"@B true >=",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,true],[30,0.9999999823048711,0.9999999403953552,true]]}]"#,
            ),
        ),
        (
            r"true @B >=",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,true],[20,false],[30,0.9999999823048711,0.9999999403953552,true]]}]"#,
            ),
        ),
        (r"@L @M >=", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L @D >=",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,5,3.0]]}]"#,
            ),
        ),
        (
            r"@D @L >=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1.5]]}]"#),
        ),
        (
            r"@D @F >=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,NaN]]}]"#),
        ),
        (
            r"@F @D >=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,NaN],[30,0.0]]}]"#),
        ),
        (
            r"@S @T >=",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"b"],[30,0.9999999823048711,0.9999999403953552,"c"]]}]"#,
            ),
        ),
        (
            r"@T @S >=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,"b"],[20,"b"]]}]"#),
        ),
        (
            r"@U 'ﬀ' >=",
            Ok(r#"[{"c":"u","l":{},"a":{},"la":0,"v":[[20,"\uFB00"]]}]"#),
        ),
        (
            r"@L 'x' >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@S 1 >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@B 1 >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L true >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (r"@E 1 >=", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"1 @E >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@E 'x' >=",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E true >=",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E [ ] >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L [ ] >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"[ ] @L >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L @S >=",
            Err(r">=can only operate on two GTS with NUMBER or STRING values."),
        ),
        (
            r"@B @C >=",
            Err(r">=can only operate on two GTS with NUMBER or STRING values."),
        ),
        (r"@L @E >=", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @B >=", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L NaN < true >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (
            r"@L NULL >=",
            Err(
                r">= can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn compare_misc() {
    check(&[
        (
            r"@L 'a' STORE $a $a == DROP $a",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[20,-2],[30,0.9999999823048711,1.9999999646097422,5,3],[40,0]]}]"#,
            ),
        ),
        (
            r"@L DUP ==",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1],[20,-2],[30,0.9999999823048711,1.9999999646097422,5,3],[40,0]]}]"#,
            ),
        ),
        (
            r"@L DUP !=",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (r"NULL @L !=", Ok(r"[true]")),
        (r"@L @E !=", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"'😀' 'ﬀ' <", Ok(r"[true]")),
        (r"'😀' 'ﬀ' >", Ok(r"[false]")),
        (r"@L NaN != SIZE", Ok(r"[4]")),
        (r"@L NaN == TYPEOF", Ok(r#"["GTS"]"#)),
        (
            r"@L NaN < 1 ==",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@L NaN < NaN !=",
            Ok(r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[]}]"#),
        ),
        (r"1 1.0 ==", Ok(r"[true]")),
        (r"NaN NaN ==", Ok(r"[true]")),
        (r"NaN 'a' <", Ok(r"[false]")),
        (r"'a' NaN <", Ok(r"[false]")),
        (r"'a' 'b' <", Ok(r"[true]")),
        (
            r"'a' 1 <",
            Err(
                r"< can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
        (r"[ ] [ ] ==", Ok(r"[true]")),
    ]);
}

#[test]
fn logical() {
    check(&[
        (
            r"@B @C &&",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,false],[30,false]]}]"#),
        ),
        (
            r"@B @C ||",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,true],[30,true]]}]"#),
        ),
        (
            r"@B @C AND",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,false],[30,false]]}]"#),
        ),
        (
            r"@B @C OR",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[20,true],[30,true]]}]"#),
        ),
        (r"@B @E &&", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @E ||", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@E @B AND",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@B @L &&",
            Err(r"&& can only operate on boolean values or boolean GTS."),
        ),
        (
            r"@L @L ||",
            Err(r"|| can only operate on boolean values or boolean GTS."),
        ),
        (
            r"@L @E &&",
            Err(r"&& can only operate on boolean values or boolean GTS."),
        ),
        (
            r"@B true &&",
            Err(
                r"&& can only operate on two boolean values, or two boolean GTS, or a list of booleans or macros, each macro putting a single boolean on top of the stack.",
            ),
        ),
        (
            r"true @B ||",
            Err(
                r"|| can only operate on two boolean values, or two boolean GTS, or a list of booleans or macros, each macro putting a single boolean on top of the stack.",
            ),
        ),
        (
            r"@B NOT",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,false],[20,true],[30,0.9999999823048711,0.9999999403953552,false]]}]"#,
            ),
        ),
        (
            r"@B !",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,false],[20,true],[30,0.9999999823048711,0.9999999403953552,false]]}]"#,
            ),
        ),
        (r"@E NOT", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@L NOT",
            Err(r"NOT can only operate on a boolean value or a boolean GTS."),
        ),
        (
            r"@L !",
            Err(r"! can only operate on a boolean value or a boolean GTS."),
        ),
        (
            r"@S NOT",
            Err(r"NOT can only operate on a boolean value or a boolean GTS."),
        ),
        (
            r"true 1 &&",
            Err(
                r"&& can only operate on two boolean values, or two boolean GTS, or a list of booleans or macros, each macro putting a single boolean on top of the stack.",
            ),
        ),
        (
            r"true 1 AND",
            Err(
                r"AND can only operate on two boolean values, or two boolean GTS, or a list of booleans or macros, each macro putting a single boolean on top of the stack.",
            ),
        ),
        (
            r"true 1 ||",
            Err(
                r"|| can only operate on two boolean values, or two boolean GTS, or a list of booleans or macros, each macro putting a single boolean on top of the stack.",
            ),
        ),
        (
            r"true 1 OR",
            Err(
                r"OR can only operate on two boolean values, or two boolean GTS, or a list of booleans or macros, each macro putting a single boolean on top of the stack.",
            ),
        ),
        (
            r"1 NOT",
            Err(r"NOT can only operate on a boolean value or a boolean GTS."),
        ),
        (
            r"1 !",
            Err(r"! can only operate on a boolean value or a boolean GTS."),
        ),
        (
            r"[ true @B ] &&",
            Err(
                r"&& can only operate on two boolean values, or two boolean GTS, or a list of booleans or macros, each macro putting a single boolean on top of the stack.",
            ),
        ),
        (
            r"[ false @B ] ||",
            Err(
                r"|| can only operate on two boolean values, or two boolean GTS, or a list of booleans or macros, each macro putting a single boolean on top of the stack.",
            ),
        ),
        (r"true false &&", Ok(r"[false]")),
        (r"true false ||", Ok(r"[true]")),
    ]);
}

#[test]
fn bitwise_and() {
    check(&[
        (
            r"@L 3 &",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,3],[20,2],[40,0]]}]"#,
            ),
        ),
        (
            r"@L @M &",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,1],[20,6],[40,0]]}]"#),
        ),
        (r"@E 3 &", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@L @E &", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @L &", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@E @E &",
            Err(
                r"& can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"3 @L &",
            Err(
                r"& can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@D 3 &",
            Err(
                r"& can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L 3.0 &",
            Err(
                r"& can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L @D &",
            Err(
                r"& can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@S 1 &",
            Err(
                r"& can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L NaN < @L & true ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn bitwise_or() {
    check(&[
        (
            r"@L 3 |",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[30,0.9999999823048711,1.9999999646097422,5,3],[20,-1],[40,3]]}]"#,
            ),
        ),
        (
            r"@L @M |",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,5],[20,-1],[40,4]]}]"#),
        ),
        (r"@E 3 |", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@L @E |", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @L |", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@E @E |",
            Err(
                r"| can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"3 @L |",
            Err(
                r"| can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@D 3 |",
            Err(
                r"| can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L 3.0 |",
            Err(
                r"| can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L @D |",
            Err(
                r"| can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@S 1 |",
            Err(
                r"| can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L NaN < @L | true ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn bitwise_xor() {
    check(&[
        (
            r"@L 3 ^",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[30,0.9999999823048711,1.9999999646097422,5,0],[20,-3],[40,3]]}]"#,
            ),
        ),
        (
            r"@L @M ^",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,4],[20,-7],[40,4]]}]"#),
        ),
        (r"@E 3 ^", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@L @E ^", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @L ^", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@E @E ^",
            Err(
                r"^ can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"3 @L ^",
            Err(
                r"^ can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@D 3 ^",
            Err(
                r"^ can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L 3.0 ^",
            Err(
                r"^ can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L @D ^",
            Err(
                r"^ can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@S 1 ^",
            Err(
                r"^ can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L NaN < @L ^ true ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn bitwise_shl() {
    check(&[
        (
            r"@L 3 <<",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,8],[30,0.9999999823048711,1.9999999646097422,5,24],[20,-16],[40,0]]}]"#,
            ),
        ),
        (
            r"@L @M <<",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,32],[20,-256],[40,0]]}]"#),
        ),
        (r"@E 3 <<", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@L @E <<", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @L <<", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@E @E <<",
            Err(
                r"<< can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"3 @L <<",
            Err(
                r"<< can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@D 3 <<",
            Err(
                r"<< can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L 3.0 <<",
            Err(
                r"<< can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L @D <<",
            Err(
                r"<< can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@S 1 <<",
            Err(
                r"<< can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L NaN < @L << true ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn bitwise_shr() {
    check(&[
        (
            r"@L 3 >>",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,0],[30,0.9999999823048711,1.9999999646097422,5,0],[20,-1],[40,0]]}]"#,
            ),
        ),
        (
            r"@L @M >>",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,0],[20,-1],[40,0]]}]"#),
        ),
        (r"@E 3 >>", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@L @E >>", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (r"@E @L >>", Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@E @E >>",
            Err(
                r">> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"3 @L >>",
            Err(
                r">> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@D 3 >>",
            Err(
                r">> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L 3.0 >>",
            Err(
                r">> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L @D >>",
            Err(
                r">> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@S 1 >>",
            Err(
                r">> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L NaN < @L >> true ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn bitwise_ushr() {
    check(&[
        (
            r"@L 3 >>>",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,0],[30,0.9999999823048711,1.9999999646097422,5,0],[20,2305843009213693951],[40,0]]}]"#,
            ),
        ),
        (
            r"@L @M >>>",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[10,0],[20,144115188075855871],[40,0]]}]"#),
        ),
        (
            r"@E 3 >>>",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@L @E >>>",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E @L >>>",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E @E >>>",
            Err(
                r">>> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"3 @L >>>",
            Err(
                r">>> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@D 3 >>>",
            Err(
                r">>> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L 3.0 >>>",
            Err(
                r">>> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L @D >>>",
            Err(
                r">>> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@S 1 >>>",
            Err(
                r">>> can only operate on two LONG, or two LONG GTS, or one LONG GTS and a LONG, or a BIGDECIMAL/LONG combination .",
            ),
        ),
        (
            r"@L NaN < @L >>> true ==",
            Err(
                r"== can only operate when GTS values and the top stack operand have the same type.",
            ),
        ),
    ]);
}

#[test]
fn bits() {
    check(&[
        (
            r"@L ->DOUBLEBITS",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,4607182418800017408],[30,0.9999999823048711,1.9999999646097422,5,4613937818241073152],[20,-4611686018427387904],[40,0]]}]"#,
            ),
        ),
        (
            r"@D ->DOUBLEBITS",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,4609434218613702656],[20,9221120237041090560],[30,-9223372036854775808],[50,2.999999988824129,3.9999999292194843,4611686018427387904]]}]"#,
            ),
        ),
        (
            r"@D ->FLOATBITS",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1069547520],[20,2143289344],[30,2147483648],[50,2.999999988824129,3.9999999292194843,1073741824]]}]"#,
            ),
        ),
        (
            r"@L ->FLOATBITS",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1065353216],[30,0.9999999823048711,1.9999999646097422,5,1077936128],[20,3221225472],[40,0]]}]"#,
            ),
        ),
        (
            r"@S ->DOUBLEBITS",
            Err(r"->DOUBLEBITS operates on a DOUBLE or LONG Geo Time Series."),
        ),
        (
            r"@E ->DOUBLEBITS",
            Err(r"->DOUBLEBITS operates on a DOUBLE or LONG Geo Time Series."),
        ),
        (
            r"@B ->FLOATBITS",
            Err(r"->FLOATBITS operates on a DOUBLE or LONG Geo Time Series."),
        ),
        (
            r"@L DOUBLEBITS->",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,4.9E-324],[30,0.9999999823048711,1.9999999646097422,5,1.5E-323],[20,NaN],[40,0.0]]}]"#,
            ),
        ),
        (
            r"@L FLOATBITS->",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1.401298464324817E-45],[30,0.9999999823048711,1.9999999646097422,5,4.203895392974451E-45],[20,NaN],[40,0.0]]}]"#,
            ),
        ),
        (
            r"@D DOUBLEBITS->",
            Err(r"DOUBLEBITS-> operates on a LONG Geo Time Series."),
        ),
        (
            r"@E FLOATBITS->",
            Err(r"FLOATBITS-> operates on a LONG Geo Time Series."),
        ),
        (
            r"@S FLOATBITS->",
            Err(r"FLOATBITS-> operates on a LONG Geo Time Series."),
        ),
        (
            r"@L ->DOUBLEBITS DOUBLEBITS->",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1.0],[30,0.9999999823048711,1.9999999646097422,5,3.0],[20,-2.0],[40,0.0]]}]"#,
            ),
        ),
        (
            r"@D ->FLOATBITS FLOATBITS->",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[20,NaN],[30,-0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (r"1152921573326323713 ->FLOATBITS", Ok(r"[1568669697]")),
        (
            r"NEWGTS 1 NaN NaN NaN 1152921573326323713 ADDVALUE ->FLOATBITS",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,1568669697]]}]"#),
        ),
        (
            r"'a' ->DOUBLEBITS",
            Err(r"->DOUBLEBITS operates on a DOUBLE, LONG or Geo Time Series thereof."),
        ),
        (
            r"'a' DOUBLEBITS->",
            Err(r"DOUBLEBITS-> operates on a LONG or a Geo Time Series thereof."),
        ),
        (
            r"1.0 DOUBLEBITS->",
            Err(r"DOUBLEBITS-> operates on a LONG or a Geo Time Series thereof."),
        ),
        (
            r"@X ->DOUBLEBITS",
            Ok(
                r#"[{"c":"x","l":{},"a":{},"la":0,"v":[[9,4890909195324358656],[10,4607182418800017408]]}]"#,
            ),
        ),
    ]);
}

#[test]
fn numerical() {
    check(&[
        (
            r"@L ABS",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,3],[20,2],[40,0]]}]"#,
            ),
        ),
        (
            r"@D ABS",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[20,NaN],[30,0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@L SIN",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,0.8414709848078965],[30,0.9999999823048711,1.9999999646097422,5,0.1411200080598672],[20,-0.9092974268256817],[40,0.0]]}]"#,
            ),
        ),
        (
            r"@D FLOOR",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.0],[20,NaN],[30,-0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}]"#,
            ),
        ),
        (
            r"@L ROUND",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,3],[20,-2],[40,0]]}]"#,
            ),
        ),
        (
            r"@D ROUND",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,2],[20,0],[30,0],[50,2.999999988824129,3.9999999292194843,2]]}]"#,
            ),
        ),
        (
            r"@E ROUND",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (r"@E SQRT", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"@S SQRT",
            Err(r"SQRT can only operate on LONG, DOUBLE or empty GTSs."),
        ),
        (
            r"@B ABS",
            Err(r"ABS can only operate on LONG, DOUBLE or empty GTSs."),
        ),
        (
            r"[ @L [ @D ] 1 ] ABS",
            Ok(
                r#"[[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,3],[20,2],[40,0]]},[{"c":"d","l":{},"a":{},"la":0,"v":[[10,1.5],[20,NaN],[30,0.0],[50,2.999999988824129,3.9999999292194843,2.0]]}],1]]"#,
            ),
        ),
        (
            r"[ @S ] ABS",
            Err(r"ABS can only operate on LONG, DOUBLE or empty GTSs."),
        ),
        (
            r"@L INCREMENTEXACT",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[30,0.9999999823048711,1.9999999646097422,5,4],[20,-1],[40,1]]}]"#,
            ),
        ),
        (r"@X INCREMENTEXACT", Err(r"long overflow")),
        (
            r"@D INCREMENTEXACT",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,2],[20,1],[30,1],[50,2.999999988824129,3.9999999292194843,3]]}]"#,
            ),
        ),
        (
            r"@L TODEGREES",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,57.29577951308232],[30,0.9999999823048711,1.9999999646097422,5,171.88733853924697],[20,-114.59155902616465],[40,0.0]]}]"#,
            ),
        ),
        (
            r"@L 2 **",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[30,0.9999999823048711,1.9999999646097422,5,9],[20,4],[40,0]]}]"#,
            ),
        ),
        (
            r"@L 2.0 **",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1.0],[30,0.9999999823048711,1.9999999646097422,5,9.0],[20,4.0],[40,0.0]]}]"#,
            ),
        ),
        (
            r"@D 2 **",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,2.25],[20,NaN],[30,0.0],[50,2.999999988824129,3.9999999292194843,4.0]]}]"#,
            ),
        ),
        (
            r"2 @L **",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[30,0.9999999823048711,1.9999999646097422,5,8],[20,0],[40,1]]}]"#,
            ),
        ),
        (
            r"@L 2 MAX",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[30,0.9999999823048711,1.9999999646097422,5,3],[20,2],[40,2]]}]"#,
            ),
        ),
        (r"@L MAX", Ok(r"[3]")),
        (r"@D MAX", Ok(r"[NaN]")),
        (r"@E MAX", Ok(r"[null]")),
        (r"@L MIN", Ok(r"[-2]")),
        (r"@D MIN", Ok(r"[NaN]")),
        (r"@L ADDEXACT", Ok(r"[2]")),
        (r"@X ADDEXACT", Err(r"long overflow")),
        (
            r"@S MAX",
            Err(
                r"MAX can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values, or a list of numerical values.",
            ),
        ),
        (
            r"@B MIN",
            Err(
                r"MIN can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values, or a list of numerical values.",
            ),
        ),
        (
            r"@L 3 FLOORDIV",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,0],[30,0.9999999823048711,1.9999999646097422,5,1],[20,-1],[40,0]]}]"#,
            ),
        ),
        (r"3 @L FLOORDIV", Err(r"/ by zero")),
        (r"@L 0 FLOORDIV", Err(r"/ by zero")),
        (
            r"@L 2 ATAN2",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,0.4636476090008061],[30,0.9999999823048711,1.9999999646097422,5,0.982793723247329],[20,-0.7853981633974483],[40,0.0]]}]"#,
            ),
        ),
        (
            r"2 @L ATAN2",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1.1071487177940904],[30,0.9999999823048711,1.9999999646097422,5,0.5880026035475675],[20,2.356194490192345],[40,1.5707963267948966]]}]"#,
            ),
        ),
        (
            r"@D 2 HYPOT",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[10,2.5],[20,NaN],[30,2.0],[50,2.999999988824129,3.9999999292194843,2.8284271247461903]]}]"#,
            ),
        ),
        (
            r"'x' @L ATAN2",
            Err(
                r"ATAN2 can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values.",
            ),
        ),
        (
            r"@S 2 ATAN2",
            Err(
                r"ATAN2 can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values.",
            ),
        ),
        (
            r"2 @S ATAN2",
            Err(
                r"ATAN2 can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values.",
            ),
        ),
        (
            r"@L @L ATAN2",
            Err(
                r"ATAN2 can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values.",
            ),
        ),
        (
            r"@E 2 ATAN2",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"2 @E ATAN2",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@L 2 COPYSIGN",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,1.0],[30,0.9999999823048711,1.9999999646097422,5,3.0],[20,2.0],[40,0.0]]}]"#,
            ),
        ),
        (
            r"@L -1.5 COPYSIGN",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,-1.0],[30,0.9999999823048711,1.9999999646097422,5,-3.0],[20,-2.0],[40,-0.0]]}]"#,
            ),
        ),
        (
            r"@L 2.0 MAX",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,2.0],[30,0.9999999823048711,1.9999999646097422,5,3.0],[20,2.0],[40,2.0]]}]"#,
            ),
        ),
        (
            r"@L 1 ADDEXACT",
            Ok(
                r#"[{"c":"l","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[30,0.9999999823048711,1.9999999646097422,5,4],[20,-1],[40,1]]}]"#,
            ),
        ),
        (r"@X 1 ADDEXACT", Err(r"long overflow")),
        (r"1 @X ADDEXACT", Err(r"long overflow")),
        (
            r"@L ->LIST 2 MAX",
            Err(r"->LIST expects a LIST, SET or a LONG number of elements to pack into a LIST."),
        ),
        (r"@L ATAN2", Err(r"Empty stack.")),
        (r"@L FLOORDIV", Err(r"Empty stack.")),
    ]);
}
