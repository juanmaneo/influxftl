// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `MUSIGMA`, `NSUMSUMSQ`, `LR`, `->Z`, `Z->` and `NPDF`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const D: &str = r"NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 2 NaN NaN NaN 2.25 ADDVALUE 3 NaN NaN NaN -0.75 ADDVALUE 5 NaN NaN NaN 10.125 ADDVALUE";
const L: &str = r"NEWGTS 10 NaN NaN NaN 3 ADDVALUE 20 NaN NaN NaN 7 ADDVALUE 30 NaN NaN NaN 4 ADDVALUE 40 NaN NaN NaN 12 ADDVALUE";
const ONE: &str = r"NEWGTS 7 NaN NaN NaN 2.5 ADDVALUE";
const S: &str = r"NEWGTS 1 NaN NaN NaN 'x' ADDVALUE";
const NAN: &str = r"NEWGTS 1 NaN NaN NaN NaN ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE";
const BIG: &str = r"NEWGTS 1 NaN NaN NaN 0.1 ADDVALUE 2 NaN NaN NaN 0.2 ADDVALUE 3 NaN NaN NaN 0.3 ADDVALUE 1000000000 NaN NaN NaN 1e15 ADDVALUE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@ONE", ONE)
        .replace("@NAN", NAN)
        .replace("@BIG", BIG)
        .replace("@D", D)
        .replace("@L", L)
        .replace("@S", S)
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
fn stats() {
    check(&[
        (r"@D true MUSIGMA", Ok(r"[4.737236043658088,3.281]")),
        (r"@D false MUSIGMA", Ok(r"[4.102566757531193,3.281]")),
        (r"@D NSUMSUMSQ", Ok(r"[110.390625,13.125,4]")),
        (r"@D LR", Ok(r"[2.0892857142857144,-2.4642857142857144]")),
        (r"@L true MUSIGMA", Ok(r"[4.041451884327381,6.5]")),
        (r"@L false MUSIGMA", Ok(r"[3.5,6.5]")),
        (r"@L NSUMSUMSQ", Ok(r"[218.0,26.0,4]")),
        (r"@L LR", Ok(r"[0.24,0.5]")),
        (r"@ONE true MUSIGMA", Ok(r"[0.0,2.5]")),
        (r"@ONE false MUSIGMA", Ok(r"[0.0,2.5]")),
        (r"@ONE NSUMSUMSQ", Ok(r"[6.25,2.5,1]")),
        (r"@ONE LR", Err(r"/ by zero")),
        (
            r"@BIG true MUSIGMA",
            Ok(r"[4.999999999999999E14,2.500000000000002E14]"),
        ),
        (
            r"@BIG false MUSIGMA",
            Ok(r"[4.3301270189221925E14,2.500000000000002E14]"),
        ),
        (r"@BIG NSUMSUMSQ", Ok(r"[1.0E30,1.0000000000000006E15,4]")),
        (r"@BIG LR", Ok(r"[1000000.0019999997,-1999999.803333333]")),
        (
            r"@S true MUSIGMA",
            Err(r"MUSIGMA can only compute mu and sigma for numerical series."),
        ),
        (
            r"@S NSUMSUMSQ",
            Err(r"NSUMSUMSQ can only compute mu and sigma for numerical series."),
        ),
        (
            r"@S LR",
            Err(r"LR can only compute simple linear regression parameters for numerical series."),
        ),
        (
            r"NEWGTS true MUSIGMA",
            Err(r"MUSIGMA can only compute mu and sigma for non empty series."),
        ),
        (
            r"NEWGTS NSUMSUMSQ",
            Err(r"NSUMSUMSQ can only compute mu and sigma for numerical series."),
        ),
        (
            r"NEWGTS LR",
            Err(r"LR can only compute simple linear regression parameters for numerical series."),
        ),
        (r"@NAN true MUSIGMA", Err(r"Infinite or NaN")),
        (r"@NAN NSUMSUMSQ", Err(r"Infinite or NaN")),
        (r"@NAN LR", Err(r"Infinite or NaN")),
        (
            r"@D 1 MUSIGMA",
            Err(
                r"MUSIGMA expects a boolean on top of the stack to determine if Bessel's correction should be applied or not.",
            ),
        ),
        (
            r"1 true MUSIGMA",
            Err(r"MUSIGMA expects a Geo Time Series instance below the top of the stack."),
        ),
        (
            r"1 NSUMSUMSQ",
            Err(r"NSUMSUMSQ expects a Geo Time Series instance on top of the stack."),
        ),
        (
            r"1 LR",
            Err(r"LR expects a Geo Time Series instance on top of the stack."),
        ),
        (r"MUSIGMA", Err(r"Empty stack.")),
        (r"true MUSIGMA", Err(r"Empty stack.")),
        (r"NSUMSUMSQ", Err(r"Empty stack.")),
        (r"LR", Err(r"Empty stack.")),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 2 ADDVALUE LR",
            Err(r"/ by zero"),
        ),
        (r"NEWGTS 0 NaN NaN NaN 0 ADDVALUE LR", Err(r"/ by zero")),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE 1000 1 TIMECLIP LR",
            Err(r"LR can only compute simple linear regression parameters for numerical series."),
        ),
        (
            r"[ 1 NaN NaN NaN 2 ] 1 ->LIST MAKEGTS 0 GET LR",
            Err(r"Empty stack."),
        ),
    ]);
}

#[test]
fn morton() {
    check(&[
        (r"[ 1 2 3 ] 4 ->Z ->HEX", Ok(r#"["01d0"]"#)),
        (r"[ 5 10 ] 8 ->Z ->HEX", Ok(r#"["0066"]"#)),
        (r"[ 255 0 255 ] 8 ->Z ->HEX", Ok(r#"["b6db6d"]"#)),
        (r"[ 1 ] 63 ->Z ->HEX", Ok(r#"["0000000000000002"]"#)),
        (r"[ ] 8 ->Z ->HEX", Ok(r#"[""]"#)),
        (r"[ 3 ] 0 ->Z ->HEX", Ok(r#"[""]"#)),
        (r"[ 1 2 3 ] 4 ->Z 3 4 Z->", Ok(r"[[1,2,3]]")),
        (r"[ 5 10 ] 8 ->Z 2 8 Z->", Ok(r"[[5,10]]")),
        (
            r"[ 123456789 987654321 ] 31 ->Z 2 31 Z->",
            Ok(r"[[123456789,987654321]]"),
        ),
        (r"[ 7 7 7 7 7 ] 3 ->Z ->HEX", Ok(r#"["fffe"]"#)),
        (
            r"[ 1 -1 ] 4 ->Z",
            Err(r"->Z operates on a list of positive LONGs."),
        ),
        (
            r"[ 1 'x' ] 4 ->Z",
            Err(r"->Z operates on a list of positive LONGs."),
        ),
        (
            r"1 4 ->Z",
            Err(r"->Z operates on a list of positive LONGs."),
        ),
        (r"[ 1 ] 64 ->Z", Err(r"->Z expects a bit width <= 63.")),
        (r"[ 1 ] -1 ->Z", Err(r"->Z expects a bit width <= 63.")),
        (
            r"[ 1 ] 'x' ->Z",
            Err(r"->Z expects a bit width on top of the stack."),
        ),
        (r"[ 1 ] 4294967300 ->Z ->HEX", Ok(r#"["10"]"#)),
        (r"->Z", Err(r"Empty stack.")),
        (r"4 ->Z", Err(r"Empty stack.")),
        (r"'00' HEX-> 3 8 Z->", Err(r"1")),
        (r"'00' HEX-> -1 4 Z->", Err(r"-1")),
        (
            r"'00' HEX-> 'x' 4 Z->",
            Err(r"Z-> expects a number of components below the bit width."),
        ),
        (
            r"1 2 4 Z->",
            Err(r"Z-> operates on a Morton code encoded as a byte array."),
        ),
        (
            r"'00' HEX-> 2 64 Z->",
            Err(r"Z-> expects a bit width <= 63."),
        ),
        (r"Z->", Err(r"Empty stack.")),
        (r"4 Z->", Err(r"Empty stack.")),
        (r"2 4 Z->", Err(r"Empty stack.")),
        (r"'ff00' HEX-> 2 4 Z->", Ok(r"[[15,15]]")),
        (r"'' HEX-> 0 8 Z->", Ok(r"[[]]")),
    ]);
}

#[test]
fn npdf() {
    check(&[
        (r"0 1 NPDF 0 SWAP EVAL", Ok(r"[0.3989422804014327]")),
        (r"0 1 NPDF 1.5 SWAP EVAL", Ok(r"[0.12951759566589174]")),
        (r"10 2.5 NPDF 7 SWAP EVAL", Ok(r"[0.07767442199328518]")),
        (r"0 1 NPDF SNAPSHOT", Ok(r#"["0.0 1.0 NPDF "]"#)),
        (r"1 2 NPDF TYPEOF", Ok(r#"["FUNCTION"]"#)),
        (
            r"0 0 NPDF",
            Err(r"NPDF expects a standard deviation (sigma) on top of the stack."),
        ),
        (
            r"0 -1 NPDF",
            Err(r"NPDF expects a standard deviation (sigma) on top of the stack."),
        ),
        (
            r"0 'x' NPDF",
            Err(r"NPDF expects a standard deviation (sigma) on top of the stack."),
        ),
        (
            r"'x' 1 NPDF",
            Err(r"NPDF expects a mean (mu) below the standard deviation."),
        ),
        (r"0 NaN NPDF 1 SWAP EVAL", Ok(r"[NaN]")),
        (
            r"0 1 NPDF 'x' SWAP EVAL",
            Err(r"NPDF expects a numeric value on top of the stack."),
        ),
        (r"NPDF", Err(r"Empty stack.")),
        (r"1 NPDF", Err(r"Empty stack.")),
        (
            r"0 1 NPDF 'f' STORE [ $f ] ->MACRO SNAPSHOT",
            Ok(r#"["<% 0.0 1.0 NPDF EVAL %> "]"#),
        ),
        (
            r"0 1 NPDF 'f' STORE 'x' [ $f ] ->MACRO EVAL",
            Err(r"NPDF expects a numeric value on top of the stack."),
        ),
        (r"0 1 NPDF ->JSON", Ok(r#"["\"0.0 1.0 NPDF\""]"#)),
        (r"0 1 NPDF EVAL", Err(r"Empty stack.")),
    ]);
}
