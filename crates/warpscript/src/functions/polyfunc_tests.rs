// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Oracle-checked tests of `POLYFUNC`, `RANDPDF` and `SRANDPDF`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const F: &str = r"[ 1 2 3 ] POLYFUNC 'f' STORE";
const N: &str = r"NEWGTS 'n' RENAME 3 1.0 2.0 4 'b' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE";
const B: &str = r"[ NEWGTS 'b' RENAME 10 NaN NaN NaN 5 ADDVALUE 30 1.0 2.0 7 6 ADDVALUE bucketizer.last 30 10 0 ] BUCKETIZE 0 GET";
const BE: &str = r"[ NEWGTS 'b' RENAME 10 NaN NaN NaN 5 ADDVALUE 30 NaN NaN 7 6 ADDVALUE bucketizer.last 30 10 0 ] BUCKETIZE 0 GET";
const BP: &str = r"[ NEWGTS 'b' RENAME 10 NaN NaN NaN 5 ADDVALUE 30 NaN NaN NaN 6 ADDVALUE bucketizer.last 30 10 0 ] BUCKETIZE 0 GET";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@BE", BE)
        .replace("@BP", BP)
        .replace("@F", F)
        .replace("@N", N)
        .replace("@B", B)
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
fn polyfunc() {
    check(&[
        (
            r"[ 1 2 3 ] POLYFUNC DUP TYPEOF SWAP SNAPSHOT",
            Ok(r#"["'AGGREGATOR' [] 1.0 +! 2.0 +! 3.0 +!  POLYFUNC "]"#),
        ),
        (
            r"[ 1 2 3 ] POLYFUNC ->JSON",
            Ok(r#"["\"[] 1.0 +! 2.0 +! 3.0 +!  POLYFUNC\""]"#),
        ),
        (
            r"[ 1 2 0 0.0 ] POLYFUNC SNAPSHOT",
            Ok(r#"["[] 1.0 +! 2.0 +!  POLYFUNC "]"#),
        ),
        (
            r"[ 0 0 ] POLYFUNC SNAPSHOT",
            Ok(r#"["[] 0.0 +!  POLYFUNC "]"#),
        ),
        (
            r"[ 1 -0.0 ] POLYFUNC SNAPSHOT",
            Ok(r#"["[] 1.0 +!  POLYFUNC "]"#),
        ),
        (
            r"[ 1.5 NaN ] POLYFUNC SNAPSHOT",
            Ok(r#"["[] 1.5 +! NaN +!  POLYFUNC "]"#),
        ),
        (r"@F 2 $f EVAL", Ok(r"[17.0]")),
        (r"@F 2.5 $f EVAL", Ok(r"[24.75]")),
        (r"@F [ 1 2.5 -1 ] $f EVAL", Ok(r"[[6.0,24.75,2.0]]")),
        (r"@F [ ] $f EVAL", Ok(r"[[]]")),
        (
            r"@F [ 1 'x' ] $f EVAL",
            Err(r"POLYFUNC can only be applied to numerical values."),
        ),
        (
            r"@F @N $f EVAL",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[3,0.9999999823048711,1.9999999646097422,4,34.0],[1,6.0]]}]"#,
            ),
        ),
        (
            r"@F @N $f EVAL DROP @N",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[3,0.9999999823048711,1.9999999646097422,4,"b"],[1,"a"]]}]"#,
            ),
        ),
        (
            r"@F @B $f EVAL",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,7,2761.0],[20,1241.0],[10,321.0]]}]"#,
            ),
        ),
        (
            r"@F @B true $f EVAL",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[2,17.0],[1,6.0],[0,1.0]]}]"#),
        ),
        (
            r"@F @B false $f EVAL",
            Ok(
                r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,0.9999999823048711,1.9999999646097422,7,2761.0],[20,1241.0],[10,321.0]]}]"#,
            ),
        ),
        (
            r"@F @BE $f EVAL",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,7,2761.0],[20,1241.0],[10,321.0]]}]"#),
        ),
        (
            r"@F @BP $f EVAL",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[30,2761.0],[20,1241.0],[10,321.0]]}]"#),
        ),
        (
            r"@F @BE true $f EVAL",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[2,17.0],[1,6.0],[0,1.0]]}]"#),
        ),
        (
            r"@F @N true $f EVAL",
            Err(r"POLYFUNC can only be applied to bucketized numerical Geo Time Series."),
        ),
        (
            r"@F 1 true $f EVAL",
            Err(r"POLYFUNC can only be applied to bucketized numerical Geo Time Series."),
        ),
        (
            r"@F 'x' $f EVAL",
            Err(
                r"POLYFUNC can only be applied to numerical values which may appear in LISTs, GTS or ENCODERs.",
            ),
        ),
        (
            r"@F NEWGTS $f EVAL",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (r"@F $f EVAL", Err(r"Empty stack.")),
        (r"@F true $f EVAL", Err(r"Empty stack.")),
        (
            r"@F [ @N $f 0 0 0 ] MAP",
            Ok(
                r#"[[{"c":"n","l":{},"a":{},"la":0,"v":[[1,6.0],[3,0.9999999823048711,1.9999999646097422,4,34.0]]}]]"#,
            ),
        ),
        (
            r"@F [ @B $f 0 0 0 ] MAP",
            Ok(
                r#"[[{"c":"b","l":{},"a":{},"la":0,"v":[[10,321.0],[20,1241.0],[30,0.9999999823048711,1.9999999646097422,7,2761.0]]}]]"#,
            ),
        ),
        (
            r"@F [ @N $f 1 0 0 ] MAP",
            Err(
                r"MAP was given invalid parameters. (POLYFUNC can only be applied to a single value.)",
            ),
        ),
        (
            r"@F [ @B $f 0 1 0 ] MAP",
            Ok(
                r#"[[{"c":"b","l":{},"a":{},"la":0,"v":[[10,321.0],[20,0.9999999823048711,1.9999999646097422,7,1241.0],[30,0.9999999823048711,1.9999999646097422,7,2761.0]]}]]"#,
            ),
        ),
        (
            r"@F [ @N NEWGTS 7 NaN NaN NaN 1 ADDVALUE ] $f MACROREDUCER",
            Err(r"MACROREDUCER expects a macro on top of the stack."),
        ),
        (
            r"@F [ @B $f 30 10 0 ] BUCKETIZE",
            Err(
                r"BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
            ),
        ),
        (
            r"@F [ [ @N ] [] $f ] REDUCE",
            Err(r"REDUCE expects a function as parameter number 3."),
        ),
        (
            r"[ ] POLYFUNC",
            Err(r"empty polynomials coefficients array"),
        ),
        (
            r"[ 1 'a' ] POLYFUNC",
            Err(r"POLYFUNC expects polynomial coefficients to be numerical."),
        ),
        (
            r"1 POLYFUNC",
            Err(r"POLYFUNC expects a list of polynomial coefficients on top of the stack."),
        ),
        (r"POLYFUNC", Err(r"Empty stack.")),
        (
            r"[ 1 NULL ] POLYFUNC",
            Err(r"POLYFUNC expects polynomial coefficients to be numerical."),
        ),
        (
            r"@F [ $f ] ->MACRO SNAPSHOT",
            Ok(r#"["<% [] 1.0 +! 2.0 +! 3.0 +!  POLYFUNC EVAL %> "]"#),
        ),
        (r"@F 3 [ $f ] ->MACRO EVAL", Ok(r"[34.0]")),
        (r"@F [ $f ] ->MACRO EVAL", Err(r"Empty stack.")),
    ]);
}

#[test]
fn randpdf() {
    check(&[
        (
            r"{ 'a' 1 'b' 3 } RANDPDF DUP TYPEOF SWAP SNAPSHOT",
            Ok(r#"["'FUNCTION' { 'a' 0.25 'b' 0.75  } RANDPDF "]"#),
        ),
        (
            r"{ 'b' 1 'a' 3 } RANDPDF SNAPSHOT",
            Ok(r#"["{ 'a' 0.75 'b' 0.25  } RANDPDF "]"#),
        ),
        (
            r"{ 2 1 1 3 } RANDPDF SNAPSHOT",
            Ok(r#"["{ 1 0.75 2 0.25  } RANDPDF "]"#),
        ),
        (
            r"{ 2.5 1 -1.0 3 NaN 1 } RANDPDF SNAPSHOT",
            Ok(r#"["{ -1.0 0.6 2.5 0.2 NaN 0.2  } RANDPDF "]"#),
        ),
        (
            r"{ true 1 false 3 } RANDPDF SNAPSHOT",
            Ok(r#"["{ false 0.75 true 0.25  } RANDPDF "]"#),
        ),
        (
            r"{ 'a' 1 'b' 1.5 } RANDPDF SNAPSHOT",
            Ok(r#"["{ 'a' 0.4 'b' 0.6  } RANDPDF "]"#),
        ),
        (
            r"{ 'é' 1 'z' 1 'a' 2 } RANDPDF SNAPSHOT",
            Ok(r#"["{ 'a' 0.5 'z' 0.25 '%C3%A9' 0.25  } RANDPDF "]"#),
        ),
        (
            r"{ 'a' 1 'b' 3 } SRANDPDF ->JSON",
            Ok(r#"["\"{ 'a' 0.25 'b' 0.75  } SRANDPDF\""]"#),
        ),
        (
            r"42 PRNG { 'a' 1 'b' 3 'c' 2 } SRANDPDF 'f' STORE [ 1 10 <% DROP 'x' $f EVAL %> FOR ]",
            Ok(
                r#"[["x","c","x","c","x","b","x","b","x","b","x","c","x","b","x","b","x","b","x","c"]]"#,
            ),
        ),
        (
            r"7 PRNG { 1 1 2 2 3 3 4 0 } SRANDPDF 'f' STORE [ 1 12 <% DROP $f EVAL %> FOR ]",
            Ok(r"[[3,3,2,3,3,2,1,3,1,3,3,3]]"),
        ),
        (
            r"7 PRNG { 'a' 1 'b' -1 'c' 2 } SRANDPDF 'f' STORE [ 1 12 <% DROP $f EVAL %> FOR ]",
            Ok(r#"[["c","c","c","c","c","c","c","c","c","c","c","c"]]"#),
        ),
        (
            r"7 PRNG { 'a' 0 'b' 0 'c' 2 } SRANDPDF 'f' STORE [ 1 12 <% DROP $f EVAL %> FOR ]",
            Ok(r#"[["c","c","c","c","c","c","c","c","c","c","c","c"]]"#),
        ),
        (
            r"7 PRNG { 'a' 1 } SRANDPDF 'f' STORE [ 1 3 <% DROP $f EVAL %> FOR ] SRAND",
            Ok(r#"[0.8972771427421047,["a","a","a"]]"#),
        ),
        (
            r"{ 'a' 1 'b' 3 } SRANDPDF 'f' STORE 'x' $f EVAL",
            Err(r"SRANDPDF seeded PRNG was not initialized."),
        ),
        (r"{ 'a' 0 } RANDPDF 'f' STORE 1 $f EVAL", Ok(r#"["a",1]"#)),
        (r"{ 'a' 1 } RANDPDF 'f' STORE $f EVAL", Ok(r#"["a"]"#)),
        (
            r"{ } RANDPDF 'f' STORE 1 $f EVAL",
            Err(r"Index -1 out of bounds for length 0"),
        ),
        (r"{ } RANDPDF SNAPSHOT", Ok(r#"["{  } RANDPDF "]"#)),
        (
            r"{ 'a' 1 2 3 } RANDPDF",
            Err(r"class java.lang.String cannot be cast to class java.lang.Long"),
        ),
        (
            r"{ 2 1 'a' 3 } RANDPDF",
            Err(r"class java.lang.Long cannot be cast to class java.lang.String"),
        ),
        (
            r"{ 2 1 2.0 3 } RANDPDF",
            Err(r"class java.lang.Long cannot be cast to class java.lang.Double"),
        ),
        (
            r"{ 'a' 'x' } RANDPDF",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"{ 'a' NULL } RANDPDF", Err(r"NullPointerException")),
        (r"{ NULL 1 } RANDPDF", Err(r"NullPointerException")),
        (
            r"{ [ 1 ] 1 } RANDPDF",
            Err(r"class java.util.ArrayList cannot be cast to class java.lang.Comparable"),
        ),
        (
            r"{ 'a' 1 [ 1 ] 1 } RANDPDF",
            Err(r"class java.util.ArrayList cannot be cast to class java.lang.Comparable"),
        ),
        (
            r"[ 1 ] RANDPDF",
            Err(r"RANDPDF expects a value histogram on top of the stack."),
        ),
        (r"RANDPDF", Err(r"Empty stack.")),
        (r"SRANDPDF", Err(r"Empty stack.")),
        (
            r"{ 'a' 2 'b' 2 } RANDPDF 'f' STORE [ 1 5 <% DROP $f EVAL %> FOR ] SIZE",
            Ok(r"[5]"),
        ),
        (
            r"7 PRNG { 'a' 1 } SRANDPDF [ SWAP ] ->MACRO SNAPSHOT",
            Ok(r#"["<% { 'a' 1.0  } SRANDPDF EVAL %> "]"#),
        ),
        (
            r"[ { } RANDPDF ] ->MACRO EVAL",
            Err(r"Index -1 out of bounds for length 0"),
        ),
        (
            r"{ 'a' 1 'b' 2 'c' 1 } RANDPDF [ SWAP ] ->MACRO EVAL TYPEOF",
            Ok(r#"["STRING"]"#),
        ),
        (
            r"[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE { 'a' 1 } RANDPDF 0 0 0 ] MAP",
            Err(r"MAP expects a mapper function or a macro after Geo Time Series."),
        ),
        (
            r"[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 'NOW' FUNCREF 0 0 0 ] MAP",
            Err(r"MAP expects a mapper function or a macro after Geo Time Series."),
        ),
    ]);
}

/// The unseeded `RANDPDF` draws from the system entropy, which may fail.
#[test]
fn randpdf_entropy() {
    let mut stack = test_stack();
    stack.exec_checked("{ 'a' 1 'b' 0 } RANDPDF EVAL").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::from("a"));
    let error = super::polyfunc::unseeded_draw(&mut stack, || Err(getrandom::Error::UNSUPPORTED))
        .unwrap_err();
    assert!(error.to_string().contains("random data"), "{error}");
    // An exact match is the index Java's binary search lands on, not the first equal one.
    assert_eq!(
        super::polyfunc::pick(&[0.5, 0.5, 1.0], 0.5).unwrap(),
        Value::Long(1)
    );
    assert_eq!(
        super::polyfunc::pick(&[0.5, 0.5, 1.0], 0.75).unwrap(),
        Value::Long(2)
    );
}
