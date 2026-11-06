// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Oracle-checked tests of the `INTERPOLATOR.*D.*` functions.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const X: &str = r"[ 1 2 3.5 4 6 7 ]";
const F: &str = r"[ 1.0 4 2.5 -1 3 0.5 ]";
const G: &str = r"NEWGTS 'g' RENAME 1 NaN NaN NaN 1.5 ADDVALUE 2 48.0 2.0 5 2.5 ADDVALUE 3 NaN NaN NaN 6.9 ADDVALUE 4 NaN NaN NaN 9.0 ADDVALUE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@X", X).replace("@F", F).replace("@G", G)
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
fn interpolators_1d() {
    check(&[
        (
            r"@X @F INTERPOLATOR.1D.LINEAR 'i' STORE [ 0.5 1 1.5 2 3 3.5 3.9 5 6.5 7 7.5 ] <% $i EVAL %> F LMAP",
            Ok(r"[[NaN,1.0,2.5,4.0,3.0,2.5,-0.2999999999999994,1.0,1.75,0.5,NaN]]"),
        ),
        (
            r"@X @F INTERPOLATOR.1D.LINEAR SNAPSHOT",
            Ok(r#"["[ 1 2 3.5 4 6 7 ] [ 1.0 4 2.5 -1 3 0.5 ] INTERPOLATOR.1D.LINEAR "]"#),
        ),
        (
            r"@X @F INTERPOLATOR.1D.LINEAR TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"[ 1 2 3 ] [ 4 5 6 ] INTERPOLATOR.1D.LINEAR 'i' STORE [ 1.5 2.5 ] <% $i EVAL %> F LMAP",
            Ok(r"[[4.5,5.5]]"),
        ),
        (
            r"[ 1 2 3 4 5 ] [ 4 5 6 1 0 ] INTERPOLATOR.1D.LINEAR 'i' STORE [ 1.5 4.5 ] <% $i EVAL %> F LMAP",
            Ok(r"[[4.5,0.5]]"),
        ),
        (
            r"[ 1 2 ] [ 4 5 ] INTERPOLATOR.1D.LINEAR 1.5 SWAP EVAL",
            Ok(r"[4.5]"),
        ),
        (
            r"[ 1 ] [ 4 ] INTERPOLATOR.1D.LINEAR 1 SWAP EVAL",
            Ok(r"[NaN]"),
        ),
        (r"[ ] [ ] INTERPOLATOR.1D.LINEAR 1 SWAP EVAL", Ok(r"[NaN]")),
        (
            r"[ ] [ ] INTERPOLATOR.1D.LINEAR SNAPSHOT",
            Ok(r#"["[ ] [ ] INTERPOLATOR.1D.LINEAR "]"#),
        ),
        (
            r"[ 1 3 2 ] [ 4 5 6 ] INTERPOLATOR.1D.LINEAR",
            Err(r"points 1 and 2 are not strictly increasing (3 >= 2)"),
        ),
        (
            r"[ 1 1 2 3 4 ] [ 4 5 6 7 8 ] INTERPOLATOR.1D.LINEAR",
            Err(r"points 0 and 1 are not strictly increasing (1 >= 1)"),
        ),
        (
            r"[ 1 2 ] [ 4 ] INTERPOLATOR.1D.LINEAR",
            Err(
                r"INTERPOLATOR.1D.LINEAR: incoherent argument sizes, both input LIST should have the same size.",
            ),
        ),
        (
            r"[ 1 'x' ] [ 4 5 ] INTERPOLATOR.1D.LINEAR",
            Err(r"INTERPOLATOR.1D.LINEAR expects the first argument to be a numeric LIST"),
        ),
        (
            r"[ 1 2 ] [ 4 'x' ] INTERPOLATOR.1D.LINEAR",
            Err(r"INTERPOLATOR.1D.LINEAR expects the last argument to be a numeric LIST"),
        ),
        (
            r"[ 'x' 2 ] [ 'y' 5 ] INTERPOLATOR.1D.LINEAR",
            Err(r"INTERPOLATOR.1D.LINEAR expects the first argument to be a numeric LIST"),
        ),
        (
            r"[ 1 2 ] 'x' INTERPOLATOR.1D.LINEAR",
            Err(r"INTERPOLATOR.1D.LINEAR expects a LIST as 2nd argument"),
        ),
        (
            r"'x' [ 1 2 ] INTERPOLATOR.1D.LINEAR",
            Err(r"INTERPOLATOR.1D.LINEAR expects a sorted LIST as 1st argument"),
        ),
        (r"INTERPOLATOR.1D.LINEAR", Err(r"Empty stack.")),
        (r"[ 1 ] INTERPOLATOR.1D.LINEAR", Err(r"Empty stack.")),
        (
            r"@X @F INTERPOLATOR.1D.LINEAR 'x' SWAP EVAL",
            Err(r"INTERPOLATOR.1D.LINEAR expects a DOUBLE or a LONG"),
        ),
        (r"@X @F INTERPOLATOR.1D.LINEAR EVAL", Err(r"Empty stack.")),
        (
            r"[ @G 0 -1 0 0 [ 1.0 2.0 3.0 4.0 5.0 9.0 ] [ 0.0 1.0 -1.0 2.0 0.5 3.0 ] INTERPOLATOR.1D.LINEAR ] MAP",
            Err(r"MAP expects a mapper function or a macro after Geo Time Series."),
        ),
        (
            r"[ @G [ 1.0 2.0 3.0 4.0 5.0 9.0 ] [ 0.0 1.0 -1.0 2.0 0.5 3.0 ] INTERPOLATOR.1D.LINEAR 1 1 0 ] MAP",
            Err(
                r"MAP was given invalid parameters. (INTERPOLATOR.1D.LINEAR expects 1 element but got 2)",
            ),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 'x' ADDVALUE [ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.LINEAR 0 0 0 ] MAP",
            Err(
                r"MAP was given invalid parameters. (INTERPOLATOR.1D.LINEAR expects a numeric GTS as argument when used as a mapper, but instead got a non numeric element)",
            ),
        ),
        (
            r"[ 1.5 2.5 ] [ 3 4 ] INTERPOLATOR.1D.LINEAR 2 SWAP EVAL",
            Ok(r"[3.5]"),
        ),
        (
            r"[ 1 NaN 3 ] [ 1 2 3 ] INTERPOLATOR.1D.LINEAR",
            Ok(r#"["[ 1 NaN 3 ] [ 1 2 3 ] INTERPOLATOR.1D.LINEAR"]"#),
        ),
        (
            r"[ @G [ 1.0 2.0 3.0 4.0 5.0 9.0 ] [ 0.0 1.0 -1.0 2.0 0.5 3.0 ] INTERPOLATOR.1D.LINEAR 0 0 0 ] MAP",
            Ok(
                r#"[[{"c":"g","l":{},"a":{},"la":0,"v":[[1,0.5],[2,47.99999998882413,1.9999999646097422,5,0.0],[3,1.6875000000000002],[4,3.0]]}]]"#,
            ),
        ),
        (
            r"[ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.LINEAR 'f' STORE [ $f ] ->MACRO SNAPSHOT",
            Ok(r#"["<% [ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.LINEAR EVAL %> "]"#),
        ),
        (
            r"[ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.LINEAR 'f' STORE 1.5 [ $f ] ->MACRO EVAL",
            Ok(r"[3.5]"),
        ),
        (
            r"[ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.LINEAR 'f' STORE 'x' [ $f ] ->MACRO EVAL",
            Err(r"INTERPOLATOR.1D.LINEAR expects a DOUBLE or a LONG"),
        ),
        (
            r"@X @F INTERPOLATOR.1D.SPLINE 'i' STORE [ 0.5 1 1.5 2 3 3.5 3.9 5 6.5 7 7.5 ] <% $i EVAL %> F LMAP",
            Ok(
                r"[[NaN,1.0,2.6063463656387666,4.0,4.6235315712187965,2.5,-0.4061365638766512,-0.4966960352422909,2.3589620044052864,0.5,NaN]]",
            ),
        ),
        (
            r"@X @F INTERPOLATOR.1D.SPLINE SNAPSHOT",
            Ok(r#"["[ 1 2 3.5 4 6 7 ] [ 1.0 4 2.5 -1 3 0.5 ] INTERPOLATOR.1D.SPLINE "]"#),
        ),
        (
            r"@X @F INTERPOLATOR.1D.SPLINE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"[ 1 2 3 ] [ 4 5 6 ] INTERPOLATOR.1D.SPLINE 'i' STORE [ 1.5 2.5 ] <% $i EVAL %> F LMAP",
            Ok(r"[[4.5,5.5]]"),
        ),
        (
            r"[ 1 2 3 4 5 ] [ 4 5 6 1 0 ] INTERPOLATOR.1D.SPLINE 'i' STORE [ 1.5 4.5 ] <% $i EVAL %> F LMAP",
            Ok(r"[[4.3125,-0.0625]]"),
        ),
        (
            r"[ 1 2 ] [ 4 5 ] INTERPOLATOR.1D.SPLINE 1.5 SWAP EVAL",
            Ok(r"[4.5]"),
        ),
        (
            r"[ 1 ] [ 4 ] INTERPOLATOR.1D.SPLINE 1 SWAP EVAL",
            Ok(r"[NaN]"),
        ),
        (r"[ ] [ ] INTERPOLATOR.1D.SPLINE 1 SWAP EVAL", Ok(r"[NaN]")),
        (
            r"[ ] [ ] INTERPOLATOR.1D.SPLINE SNAPSHOT",
            Ok(r#"["[ ] [ ] INTERPOLATOR.1D.SPLINE "]"#),
        ),
        (
            r"[ 1 3 2 ] [ 4 5 6 ] INTERPOLATOR.1D.SPLINE",
            Err(r"points 1 and 2 are not strictly increasing (3 >= 2)"),
        ),
        (
            r"[ 1 1 2 3 4 ] [ 4 5 6 7 8 ] INTERPOLATOR.1D.SPLINE",
            Err(r"points 0 and 1 are not strictly increasing (1 >= 1)"),
        ),
        (
            r"[ 1 2 ] [ 4 ] INTERPOLATOR.1D.SPLINE",
            Err(
                r"INTERPOLATOR.1D.SPLINE: incoherent argument sizes, both input LIST should have the same size.",
            ),
        ),
        (
            r"[ 1 'x' ] [ 4 5 ] INTERPOLATOR.1D.SPLINE",
            Err(r"INTERPOLATOR.1D.SPLINE expects the first argument to be a numeric LIST"),
        ),
        (
            r"[ 1 2 ] [ 4 'x' ] INTERPOLATOR.1D.SPLINE",
            Err(r"INTERPOLATOR.1D.SPLINE expects the last argument to be a numeric LIST"),
        ),
        (
            r"[ 'x' 2 ] [ 'y' 5 ] INTERPOLATOR.1D.SPLINE",
            Err(r"INTERPOLATOR.1D.SPLINE expects the first argument to be a numeric LIST"),
        ),
        (
            r"[ 1 2 ] 'x' INTERPOLATOR.1D.SPLINE",
            Err(r"INTERPOLATOR.1D.SPLINE expects a LIST as 2nd argument"),
        ),
        (
            r"'x' [ 1 2 ] INTERPOLATOR.1D.SPLINE",
            Err(r"INTERPOLATOR.1D.SPLINE expects a sorted LIST as 1st argument"),
        ),
        (r"INTERPOLATOR.1D.SPLINE", Err(r"Empty stack.")),
        (r"[ 1 ] INTERPOLATOR.1D.SPLINE", Err(r"Empty stack.")),
        (
            r"@X @F INTERPOLATOR.1D.SPLINE 'x' SWAP EVAL",
            Err(r"INTERPOLATOR.1D.SPLINE expects a DOUBLE or a LONG"),
        ),
        (r"@X @F INTERPOLATOR.1D.SPLINE EVAL", Err(r"Empty stack.")),
        (
            r"[ @G 0 -1 0 0 [ 1.0 2.0 3.0 4.0 5.0 9.0 ] [ 0.0 1.0 -1.0 2.0 0.5 3.0 ] INTERPOLATOR.1D.SPLINE ] MAP",
            Err(r"MAP expects a mapper function or a macro after Geo Time Series."),
        ),
        (
            r"[ @G [ 1.0 2.0 3.0 4.0 5.0 9.0 ] [ 0.0 1.0 -1.0 2.0 0.5 3.0 ] INTERPOLATOR.1D.SPLINE 1 1 0 ] MAP",
            Err(
                r"MAP was given invalid parameters. (INTERPOLATOR.1D.SPLINE expects 1 element but got 2)",
            ),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 'x' ADDVALUE [ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.SPLINE 0 0 0 ] MAP",
            Err(
                r"MAP was given invalid parameters. (INTERPOLATOR.1D.SPLINE expects a numeric GTS as argument when used as a mapper, but instead got a non numeric element)",
            ),
        ),
        (
            r"[ 1.5 2.5 ] [ 3 4 ] INTERPOLATOR.1D.SPLINE 2 SWAP EVAL",
            Ok(r"[3.5]"),
        ),
        (
            r"[ 1 NaN 3 ] [ 1 2 3 ] INTERPOLATOR.1D.SPLINE",
            Ok(r#"["[ 1 NaN 3 ] [ 1 2 3 ] INTERPOLATOR.1D.SPLINE"]"#),
        ),
        (
            r"[ @G [ 1.0 2.0 3.0 4.0 5.0 9.0 ] [ 0.0 1.0 -1.0 2.0 0.5 3.0 ] INTERPOLATOR.1D.SPLINE 0 0 0 ] MAP",
            Ok(
                r#"[[{"c":"g","l":{},"a":{},"la":0,"v":[[1,0.9679759174311926],[2,47.99999998882413,1.9999999646097422,5,-0.278927752293578],[3,-0.6515002293577974],[4,3.0]]}]]"#,
            ),
        ),
        (
            r"[ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.SPLINE 'f' STORE [ $f ] ->MACRO SNAPSHOT",
            Ok(r#"["<% [ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.SPLINE EVAL %> "]"#),
        ),
        (
            r"[ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.SPLINE 'f' STORE 1.5 [ $f ] ->MACRO EVAL",
            Ok(r"[3.5]"),
        ),
        (
            r"[ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.SPLINE 'f' STORE 'x' [ $f ] ->MACRO EVAL",
            Err(r"INTERPOLATOR.1D.SPLINE expects a DOUBLE or a LONG"),
        ),
        (
            r"@X @F INTERPOLATOR.1D.AKIMA 'i' STORE [ 0.5 1 1.5 2 3 3.5 3.9 5 6.5 7 7.5 ] <% $i EVAL %> F LMAP",
            Ok(
                r"[[NaN,1.0,2.9,4.0,3.676923076923077,2.5,-0.5626813186813182,0.785714285714286,2.125,0.5,NaN]]",
            ),
        ),
        (
            r"@X @F INTERPOLATOR.1D.AKIMA SNAPSHOT",
            Ok(r#"["[ 1 2 3.5 4 6 7 ] [ 1.0 4 2.5 -1 3 0.5 ] INTERPOLATOR.1D.AKIMA "]"#),
        ),
        (
            r"@X @F INTERPOLATOR.1D.AKIMA TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"[ 1 2 3 ] [ 4 5 6 ] INTERPOLATOR.1D.AKIMA 'i' STORE [ 1.5 2.5 ] <% $i EVAL %> F LMAP",
            Ok(r"[[4.5,5.5]]"),
        ),
        (
            r"[ 1 2 3 4 5 ] [ 4 5 6 1 0 ] INTERPOLATOR.1D.AKIMA 'i' STORE [ 1.5 4.5 ] <% $i EVAL %> F LMAP",
            Ok(r"[[4.5,0.0]]"),
        ),
        (
            r"[ 1 2 ] [ 4 5 ] INTERPOLATOR.1D.AKIMA 1.5 SWAP EVAL",
            Ok(r"[4.5]"),
        ),
        (
            r"[ 1 ] [ 4 ] INTERPOLATOR.1D.AKIMA 1 SWAP EVAL",
            Ok(r"[NaN]"),
        ),
        (r"[ ] [ ] INTERPOLATOR.1D.AKIMA 1 SWAP EVAL", Ok(r"[NaN]")),
        (
            r"[ ] [ ] INTERPOLATOR.1D.AKIMA SNAPSHOT",
            Ok(r#"["[ ] [ ] INTERPOLATOR.1D.AKIMA "]"#),
        ),
        (
            r"[ 1 3 2 ] [ 4 5 6 ] INTERPOLATOR.1D.AKIMA",
            Err(r"points 1 and 2 are not strictly increasing (3 >= 2)"),
        ),
        (
            r"[ 1 1 2 3 4 ] [ 4 5 6 7 8 ] INTERPOLATOR.1D.AKIMA",
            Err(r"points 0 and 1 are not strictly increasing (1 >= 1)"),
        ),
        (
            r"[ 1 2 ] [ 4 ] INTERPOLATOR.1D.AKIMA",
            Err(
                r"INTERPOLATOR.1D.AKIMA: incoherent argument sizes, both input LIST should have the same size.",
            ),
        ),
        (
            r"[ 1 'x' ] [ 4 5 ] INTERPOLATOR.1D.AKIMA",
            Err(r"INTERPOLATOR.1D.AKIMA expects the first argument to be a numeric LIST"),
        ),
        (
            r"[ 1 2 ] [ 4 'x' ] INTERPOLATOR.1D.AKIMA",
            Err(r"INTERPOLATOR.1D.AKIMA expects the last argument to be a numeric LIST"),
        ),
        (
            r"[ 'x' 2 ] [ 'y' 5 ] INTERPOLATOR.1D.AKIMA",
            Err(r"INTERPOLATOR.1D.AKIMA expects the first argument to be a numeric LIST"),
        ),
        (
            r"[ 1 2 ] 'x' INTERPOLATOR.1D.AKIMA",
            Err(r"INTERPOLATOR.1D.AKIMA expects a LIST as 2nd argument"),
        ),
        (
            r"'x' [ 1 2 ] INTERPOLATOR.1D.AKIMA",
            Err(r"INTERPOLATOR.1D.AKIMA expects a sorted LIST as 1st argument"),
        ),
        (r"INTERPOLATOR.1D.AKIMA", Err(r"Empty stack.")),
        (r"[ 1 ] INTERPOLATOR.1D.AKIMA", Err(r"Empty stack.")),
        (
            r"@X @F INTERPOLATOR.1D.AKIMA 'x' SWAP EVAL",
            Err(r"INTERPOLATOR.1D.AKIMA expects a DOUBLE or a LONG"),
        ),
        (r"@X @F INTERPOLATOR.1D.AKIMA EVAL", Err(r"Empty stack.")),
        (
            r"[ @G 0 -1 0 0 [ 1.0 2.0 3.0 4.0 5.0 9.0 ] [ 0.0 1.0 -1.0 2.0 0.5 3.0 ] INTERPOLATOR.1D.AKIMA ] MAP",
            Err(r"MAP expects a mapper function or a macro after Geo Time Series."),
        ),
        (
            r"[ @G [ 1.0 2.0 3.0 4.0 5.0 9.0 ] [ 0.0 1.0 -1.0 2.0 0.5 3.0 ] INTERPOLATOR.1D.AKIMA 1 1 0 ] MAP",
            Err(
                r"MAP was given invalid parameters. (INTERPOLATOR.1D.AKIMA expects 1 element but got 2)",
            ),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 'x' ADDVALUE [ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.AKIMA 0 0 0 ] MAP",
            Err(
                r"MAP was given invalid parameters. (INTERPOLATOR.1D.AKIMA expects a numeric GTS as argument when used as a mapper, but instead got a non numeric element)",
            ),
        ),
        (
            r"[ 1.5 2.5 ] [ 3 4 ] INTERPOLATOR.1D.AKIMA 2 SWAP EVAL",
            Ok(r"[3.5]"),
        ),
        (
            r"[ 1 NaN 3 ] [ 1 2 3 ] INTERPOLATOR.1D.AKIMA",
            Ok(r#"["[ 1 NaN 3 ] [ 1 2 3 ] INTERPOLATOR.1D.AKIMA"]"#),
        ),
        (
            r"[ @G [ 1.0 2.0 3.0 4.0 5.0 9.0 ] [ 0.0 1.0 -1.0 2.0 0.5 3.0 ] INTERPOLATOR.1D.AKIMA 0 0 0 ] MAP",
            Ok(
                r#"[[{"c":"g","l":{},"a":{},"la":0,"v":[[1,0.875],[2,47.99999998882413,1.9999999646097422,5,-0.0625],[3,-0.00824999999999998],[4,3.0]]}]]"#,
            ),
        ),
        (
            r"[ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.AKIMA 'f' STORE [ $f ] ->MACRO SNAPSHOT",
            Ok(r#"["<% [ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.AKIMA EVAL %> "]"#),
        ),
        (
            r"[ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.AKIMA 'f' STORE 1.5 [ $f ] ->MACRO EVAL",
            Ok(r"[3.5]"),
        ),
        (
            r"[ 1 2 ] [ 3 4 ] INTERPOLATOR.1D.AKIMA 'f' STORE 'x' [ $f ] ->MACRO EVAL",
            Err(r"INTERPOLATOR.1D.AKIMA expects a DOUBLE or a LONG"),
        ),
    ]);
}

#[test]
fn interpolator_2d() {
    check(&[
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC 'b' STORE [ [ 1 0 ] [ 1.5 0.75 ] [ 2 1.5 ] [ 3 2 ] [ 4.5 2.9 ] [ 5 3 ] [ 0 0 ] [ 5.5 1 ] [ 2 -1 ] [ 2 4 ] ] <% $b EVAL %> F LMAP",
            Ok(
                r"[[1.0,2.3424479166666665,4.0,1.3858024691358022,2.482,2.999999999999999,NaN,NaN,NaN,NaN]]",
            ),
        ),
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC [ NaN 1 ] SWAP EVAL",
            Err(r"NaN out of [1, 5] range"),
        ),
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC [ 2.2 NaN ] SWAP EVAL",
            Err(r"NaN out of [0, 3] range"),
        ),
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC EVAL",
            Err(r"Empty stack."),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN NaN ADDVALUE NEWGTS 1 NaN NaN NaN 0.5 ADDVALUE ] [] [ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC ] REDUCE",
            Err(r"NaN out of [1, 5] range"),
        ),
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC SNAPSHOT",
            Ok(
                r#"["[ 1.0 2.0 4.0 5.0 ] [ 0.0 1.5 3.0 ] [ [ 1.0 2.0 3.0 ] [ 2.0 4.0 1.0 ] [ 0.5 -1.0 2.0 ] [ 3.0 2.9999999999999982 2.999999999999999 ] ] INTERPOLATOR.2D.BICUBIC "]"#,
            ),
        ),
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"[ 1 2 ] [ 1 2 ] [ [ 1 2 ] [ 3 4 ] ] INTERPOLATOR.2D.BICUBIC [ 1.5 1.5 ] SWAP EVAL",
            Ok(r"[2.5]"),
        ),
        (r"[ ] [ 1 ] [ ] INTERPOLATOR.2D.BICUBIC", Err(r"no data")),
        (
            r"[ 1 ] [ ] [ [ ] ] INTERPOLATOR.2D.BICUBIC",
            Err(r"no data"),
        ),
        (
            r"[ 2 1 ] [ 1 2 ] [ [ 1 2 ] [ 3 4 ] ] INTERPOLATOR.2D.BICUBIC",
            Err(r"points 0 and 1 are not strictly increasing (2 >= 1)"),
        ),
        (
            r"[ 1 2 ] [ 2 1 ] [ [ 1 2 ] [ 3 4 ] ] INTERPOLATOR.2D.BICUBIC",
            Err(r"points 0 and 1 are not strictly increasing (2 >= 1)"),
        ),
        (
            r"[ 1 2 ] [ 1 2 ] [ [ 1 2 ] ] INTERPOLATOR.2D.BICUBIC",
            Err(
                r"INTERPOLATOR.2D.BICUBIC: incoherent argument sizes. Up to vector sizes, values should be a LIST (size 2) of LIST (size 2).",
            ),
        ),
        (
            r"[ 1 2 ] [ 1 2 ] [ [ 1 2 ] [ 3 ] ] INTERPOLATOR.2D.BICUBIC",
            Err(
                r"INTERPOLATOR.2D.BICUBIC: incoherent argument sizes. Up to vector sizes, values should be a LIST (size 2) of LIST (size 2).",
            ),
        ),
        (
            r"[ 1 2 ] [ 1 2 ] [ [ 1 2 ] 3 ] INTERPOLATOR.2D.BICUBIC",
            Err(
                r"INTERPOLATOR.2D.BICUBIC expects the last argument to be a LIST of LIST of numbers",
            ),
        ),
        (
            r"[ 1 2 ] [ 1 2 ] [ [ 1 2 ] [ 3 'x' ] ] INTERPOLATOR.2D.BICUBIC",
            Err(
                r"INTERPOLATOR.2D.BICUBIC expects the last argument to be a LIST of LIST of numbers",
            ),
        ),
        (
            r"[ 1 'x' ] [ 1 2 ] [ [ 1 2 ] [ 3 4 ] ] INTERPOLATOR.2D.BICUBIC",
            Err(
                r"INTERPOLATOR.2D.BICUBIC expects a numeric LIST as 1st argument, but it contains at least one non-numeric element.",
            ),
        ),
        (
            r"[ 1 2 ] [ 1 'x' ] [ [ 1 2 ] [ 3 4 ] ] INTERPOLATOR.2D.BICUBIC",
            Err(
                r"INTERPOLATOR.2D.BICUBIC expects a numeric LIST as 2nd argument, but it contains at least one non-numeric element.",
            ),
        ),
        (
            r"[ 1 2 ] [ 1 2 ] 'x' INTERPOLATOR.2D.BICUBIC",
            Err(r"INTERPOLATOR.2D.BICUBIC expects a LIST as 3rd argument"),
        ),
        (
            r"[ 1 2 ] 'x' [ ] INTERPOLATOR.2D.BICUBIC",
            Err(r"INTERPOLATOR.2D.BICUBIC expects a sorted LIST as 2nd argument"),
        ),
        (
            r"'x' [ 1 ] [ ] INTERPOLATOR.2D.BICUBIC",
            Err(r"INTERPOLATOR.2D.BICUBIC expects a sorted LIST as 1st argument"),
        ),
        (r"INTERPOLATOR.2D.BICUBIC", Err(r"Empty stack.")),
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC [ 1 ] SWAP EVAL",
            Err(r"BICUBE expects a LIST with 2 components 1"),
        ),
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC [ 1 'x' ] SWAP EVAL",
            Err(
                r"BICUBE expects a numeric LIST as argument, but it contains at least one non-numeric element.",
            ),
        ),
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC 1 SWAP EVAL",
            Err(r"BICUBE expects a LIST"),
        ),
        (
            r"[ 1 ] [ 1 ] [ [ 5 ] ] INTERPOLATOR.2D.BICUBIC [ 1 1 ] SWAP EVAL",
            Err(r"-1"),
        ),
        (
            r"[ 1 ] [ 1 2 ] [ [ 5 6 ] ] INTERPOLATOR.2D.BICUBIC [ 1 1.5 ] SWAP EVAL",
            Err(r"-1"),
        ),
        (
            r"[ 1 2 ] [ 1 ] [ [ 5 ] [ 6 ] ] INTERPOLATOR.2D.BICUBIC [ 1.5 1 ] SWAP EVAL",
            Err(r"-1"),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 2 48.0 2.0 5 4.5 ADDVALUE NEWGTS 1 NaN NaN NaN 0.5 ADDVALUE 2 NaN NaN NaN 2.5 ADDVALUE ] [] [ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC ] REDUCE",
            Ok(
                r#"[[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.9390432098765433],[2,47.99999998882413,1.9999999646097422,5,2.1296296296296298]]}]]"#,
            ),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE ] [] [ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC ] REDUCE",
            Err(r"BICUBE expects 2 components but got 1"),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE NEWGTS 1 NaN NaN NaN 'x' ADDVALUE ] [] [ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC ] REDUCE",
            Err(
                r"BICUBE expects two numeric GTS when used as a reducer, but at least one value was not numeric.",
            ),
        ),
        (
            r"[ 1 2 4 5 ] [ 0 1.5 3 ] [ [ 1 2 3 ] [ 2 4 1 ] [ 0.5 -1 2 ] [ 3 3 3 ] ] INTERPOLATOR.2D.BICUBIC 'f' STORE 'x' [ $f ] ->MACRO EVAL",
            Err(r"BICUBE expects a LIST"),
        ),
        (
            r"[ -1 -0.0 1 ] [ 0 1 2 ] [ [ 1 2 3 ] [ 4 5 6 ] [ 7 8 9 ] ] INTERPOLATOR.2D.BICUBIC 'b' STORE [ [ 0.0 1 ] [ -0.0 1 ] [ -0.5 0.5 ] ] <% $b EVAL %> F LMAP",
            Ok(r"[[5.0,5.0,2.75]]"),
        ),
        (
            r"[ -1 0.0 1 ] [ 0 1 2 ] [ [ 1 2 3 ] [ 4 5 6 ] [ 7 8 9 ] ] INTERPOLATOR.2D.BICUBIC 'b' STORE [ [ 0.0 1 ] [ -0.0 1 ] ] <% $b EVAL %> F LMAP",
            Ok(r"[[5.0,5.0]]"),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2 ] [ [ 1 2 3 ] [ 4 NaN 6 ] [ 7 8 9 ] ] INTERPOLATOR.2D.BICUBIC 'b' STORE [ [ 1.5 0.5 ] [ 3 2 ] ] <% $b EVAL %> F LMAP",
            Ok(r"[[NaN,NaN]]"),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2 ] [ [ 1 2 3 ] [ 4 Infinity 6 ] [ 7 8 9 ] ] INTERPOLATOR.2D.BICUBIC 'b' STORE [ [ 1.5 0.5 ] ] <% $b EVAL %> F LMAP",
            Ok(r"[[NaN]]"),
        ),
    ]);
}

#[test]
fn interpolator_3d() {
    check(&[
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC 't' STORE [ [ 1 0 0 ] [ 1.5 0.5 1 ] [ 2 1 3 ] [ 2.5 0.25 4 ] [ 3 1 5 ] [ 0 0 0 ] [ 2 2 2 ] [ 2 0.5 6 ] [ NaN 0 0 ] [ 2 0.5 2.5 ] ] <% $t EVAL %> F LMAP",
            Ok(
                r"[[1.0,1.3752604166666669,0.9999999999999831,1.9724283854166682,2.9999999999999973,NaN,-0.57037037037037,NaN,NaN,2.1124999999999994]]",
            ),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC SNAPSHOT",
            Ok(
                r#"["[ 1.0 2.0 3.0 ] [ 0.0 1.0 2.5 ] [ 0.0 2.0 3.0 5.0 ] [ [ [ 1.0 2.0 3.0 4.0 ] [ 2.0 3.0 1.0 0.0 ] [ 0.0 0.0 1.0 1.0 ] ] [ [ 0.0 1.0 5.0 2.0 ] [ 1.0 1.000000000000007 0.9999999999999831 0.9999999999999716 ] [ 2.0 -1.0000000000000036 2.9999999999999982 2.0000000000000178 ] ] [ [ 3.0 2.0 1.0 0.0 ] [ 4.0 3.9999999999999893 1.9999999999999893 2.9999999999999973 ] [ 1.0 2.0000000000000284 2.9999999999999956 4.000000000000002 ] ] ] INTERPOLATOR.3D.TRICUBIC "]"#,
            ),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"[ ] [ 1 ] [ 1 ] [ ] INTERPOLATOR.3D.TRICUBIC",
            Err(r"no data"),
        ),
        (
            r"[ 1 ] [ ] [ 1 ] [ [ ] ] INTERPOLATOR.3D.TRICUBIC",
            Err(r"no data"),
        ),
        (
            r"[ 1 ] [ 1 ] [ ] [ [ [ ] ] ] INTERPOLATOR.3D.TRICUBIC",
            Err(r"no data"),
        ),
        (
            r"[ 2 1 ] [ 1 ] [ 1 ] [ [ [ 1 ] ] [ [ 2 ] ] ] INTERPOLATOR.3D.TRICUBIC",
            Err(r"points 0 and 1 are not strictly increasing (2 >= 1)"),
        ),
        (
            r"[ 1 2 ] [ 2 1 ] [ 1 ] [ [ [ 1 ] [ 1 ] ] [ [ 2 ] [ 2 ] ] ] INTERPOLATOR.3D.TRICUBIC",
            Err(r"points 0 and 1 are not strictly increasing (2 >= 1)"),
        ),
        (
            r"[ 1 2 ] [ 1 ] [ 2 1 ] [ [ [ 1 1 ] ] [ [ 2 2 ] ] ] INTERPOLATOR.3D.TRICUBIC",
            Err(r"points 0 and 1 are not strictly increasing (2 >= 1)"),
        ),
        (
            r"[ 1 ] [ 1 ] [ 1 ] [ ] INTERPOLATOR.3D.TRICUBIC",
            Err(
                r"INTERPOLATOR.3D.TRICUBIC: incoherent argument sizes. Up to vector sizes, values should be a LIST (size 1) of LIST (size 1) of LIST (size 1).",
            ),
        ),
        (
            r"[ 1 ] [ 1 ] [ 1 ] [ 1 ] INTERPOLATOR.3D.TRICUBIC",
            Err(
                r"INTERPOLATOR.3D.TRICUBIC expects the last argument to be a LIST of LIST of LIST of numbers.",
            ),
        ),
        (
            r"[ 1 ] [ 1 ] [ 1 ] [ [ ] ] INTERPOLATOR.3D.TRICUBIC",
            Err(
                r"INTERPOLATOR.3D.TRICUBIC: incoherent argument sizes. Up to vector sizes, values should be a LIST (size 1) of LIST (size 1) of LIST (size 1).",
            ),
        ),
        (
            r"[ 1 ] [ 1 ] [ 1 ] [ [ 1 ] ] INTERPOLATOR.3D.TRICUBIC",
            Err(
                r"INTERPOLATOR.3D.TRICUBIC expects the last argument to be a LIST of LIST of LIST of numbers",
            ),
        ),
        (
            r"[ 1 ] [ 1 ] [ 1 ] [ [ [ ] ] ] INTERPOLATOR.3D.TRICUBIC",
            Err(
                r"INTERPOLATOR.3D.TRICUBIC: incoherent argument sizes. Up to vector sizes, values should be a LIST (size 1) of LIST (size 1) of LIST (size 1).",
            ),
        ),
        (
            r"[ 1 ] [ 1 ] [ 1 ] [ [ [ 'x' ] ] ] INTERPOLATOR.3D.TRICUBIC",
            Err(
                r"INTERPOLATOR.3D.TRICUBIC expects the last argument to be a LIST of LIST of LIST of numbers",
            ),
        ),
        (
            r"[ 'x' ] [ 1 ] [ 1 ] [ ] INTERPOLATOR.3D.TRICUBIC",
            Err(
                r"INTERPOLATOR.3D.TRICUBIC expects a numeric LIST as 1st argument, instead got at least one non numeric element",
            ),
        ),
        (
            r"[ 1 ] [ 'x' ] [ 1 ] [ ] INTERPOLATOR.3D.TRICUBIC",
            Err(
                r"INTERPOLATOR.3D.TRICUBIC expects a numeric LIST as 2nd argument, instead got at least one non numeric element",
            ),
        ),
        (
            r"[ 1 ] [ 1 ] [ 'x' ] [ ] INTERPOLATOR.3D.TRICUBIC",
            Err(
                r"INTERPOLATOR.3D.TRICUBIC expects a numeric LIST as 3rd argument, instead got at least one non numeric element",
            ),
        ),
        (
            r"[ 1 ] [ 1 ] [ 1 ] 'x' INTERPOLATOR.3D.TRICUBIC",
            Err(r"INTERPOLATOR.3D.TRICUBIC expects a LIST as 4th argument"),
        ),
        (
            r"[ 1 ] [ 1 ] 'x' [ ] INTERPOLATOR.3D.TRICUBIC",
            Err(r"INTERPOLATOR.3D.TRICUBIC expects a sorted LIST as 3rd argument"),
        ),
        (
            r"[ 1 ] 'x' [ 1 ] [ ] INTERPOLATOR.3D.TRICUBIC",
            Err(r"INTERPOLATOR.3D.TRICUBIC expects a sorted LIST as 2nd argument"),
        ),
        (
            r"'x' [ 1 ] [ 1 ] [ ] INTERPOLATOR.3D.TRICUBIC",
            Err(r"INTERPOLATOR.3D.TRICUBIC expects a sorted LIST as 1st argument"),
        ),
        (r"INTERPOLATOR.3D.TRICUBIC", Err(r"Empty stack.")),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC [ 1 2 ] SWAP EVAL",
            Err(r"TRICUBE expects a LIST with 3 components 2"),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC [ 1 2 'x' ] SWAP EVAL",
            Err(
                r"TRICUBE expects a numeric LIST as argument, instead got at least one non numeric element",
            ),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC 1 SWAP EVAL",
            Err(r"TRICUBE expects a LIST"),
        ),
        (
            r"[ 1 ] [ 1 ] [ 1 ] [ [ [ 7 ] ] ] INTERPOLATOR.3D.TRICUBIC [ 1 1 1 ] SWAP EVAL",
            Ok(r"[NaN]"),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 2 48.0 2.0 5 2.5 ADDVALUE NEWGTS 1 NaN NaN NaN 0.5 ADDVALUE 2 NaN NaN NaN 0.25 ADDVALUE NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 4 ADDVALUE ] [] [ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC ] REDUCE",
            Ok(
                r#"[[{"c":"","l":{},"a":{},"la":0,"v":[[1,1.3752604166666669],[2,47.99999998882413,1.9999999646097422,5,1.9724283854166682]]}]]"#,
            ),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE NEWGTS 1 NaN NaN NaN 0.5 ADDVALUE ] [] [ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC ] REDUCE",
            Err(r"TRICUBE expects 3 components but got 2"),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE NEWGTS 1 NaN NaN NaN 0.5 ADDVALUE NEWGTS 1 NaN NaN NaN 'x' ADDVALUE ] [] [ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC ] REDUCE",
            Err(
                r"TRICUBE expects a numeric GTS as argument when used as a reducer, instead got at least one non numeric element",
            ),
        ),
        (
            r"[ [ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE NEWGTS 1 NaN NaN NaN 0.5 ADDVALUE 2 NaN NaN NaN 0.5 ADDVALUE NEWGTS 2 NaN NaN NaN 1 ADDVALUE ] [] [ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC ] REDUCE",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[]}]]"#),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC 'f' STORE 'x' [ $f ] ->MACRO EVAL",
            Err(r"TRICUBE expects a LIST"),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC EVAL",
            Err(r"Empty stack."),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC [ 3 2.5 5 ] SWAP EVAL",
            Ok(r"[4.000000000000002]"),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC [ 1 0 0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC [ 2 0 NaN ] SWAP EVAL",
            Ok(r"[NaN]"),
        ),
        (
            r"[ 1 2 3 ] [ 0 1 2.5 ] [ 0 2 3 5 ] [ [ [ 1 2 3 4 ] [ 2 3 1 0 ] [ 0 0 1 1 ] ] [ [ 0 1 5 2 ] [ 1 1 1 1 ] [ 2 -1 3 2 ] ] [ [ 3 2 1 0 ] [ 4 4 2 3 ] [ 1 2 3 4 ] ] ] INTERPOLATOR.3D.TRICUBIC [ 2 NaN 1 ] SWAP EVAL",
            Ok(r"[NaN]"),
        ),
    ]);
}
