// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `PIVOT` and `PIVOTSTRICT`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const D1: &str = r"NEWGTS 'd1' RENAME { 'h' 'x' } RELABEL { 'at' 'tr' } SETATTRIBUTES 5 NaN NaN NaN 50 ADDVALUE 1 NaN NaN NaN 10 ADDVALUE 2 NaN NaN NaN 20 ADDVALUE 3 NaN NaN NaN 30 ADDVALUE 4 NaN NaN NaN 40 ADDVALUE 7 NaN NaN NaN 70 ADDVALUE";
const D2: &str = r"NEWGTS 'd2' RENAME { 'h' 'y' } RELABEL 2 NaN NaN NaN 2.5 ADDVALUE 4 NaN NaN NaN 4.5 ADDVALUE 6 NaN NaN NaN 6.5 ADDVALUE 8 NaN NaN NaN 8.5 ADDVALUE";
const D3: &str = r"NEWGTS 'd3' RENAME  1 NaN NaN NaN 'a' ADDVALUE 1 NaN NaN NaN 'b' ADDVALUE 3 NaN NaN NaN 'c' ADDVALUE 9 NaN NaN NaN 'z' ADDVALUE";
const LA: &str = r"NEWGTS 'la' RENAME  1 NaN NaN NaN 'p' ADDVALUE 3 NaN NaN NaN 'q' ADDVALUE 4 NaN NaN NaN 'q' ADDVALUE 6 NaN NaN NaN 'r' ADDVALUE";
const LB: &str = r"NEWGTS 'lb' RENAME  2 NaN NaN NaN T ADDVALUE 3 NaN NaN NaN F ADDVALUE 4 NaN NaN NaN T ADDVALUE 7 NaN NaN NaN T ADDVALUE";
const LC: &str = r"NEWGTS 'lc' RENAME  3 NaN NaN NaN 1.5 ADDVALUE 5 NaN NaN NaN 2 ADDVALUE 100 NaN NaN NaN 3 ADDVALUE";
const LD: &str = r"NEWGTS 'ld' RENAME  0 NaN NaN NaN 1 ADDVALUE";
const LG: &str = r"NEWGTS 'lg' RENAME  3 48.0 2.0 7 9 ADDVALUE";
const G1: &str =
    r"NEWGTS 'd1' RENAME { 'k' 'v' } RELABEL 1 48.0 2.0 5 1 ADDVALUE 3 10.0 20.0 NaN 2 ADDVALUE";
const E1: &str = r"[ [ 2 'm' ] [ 3 'n' ] ] ->ENCODER 'le' RENAME";
const E2: &str = r"[ [ 1 1 ] [ 3 3 ] [ 4 4 ] ] ->ENCODER 'de' RENAME { 'u' 'w' } RELABEL";
const BAD: &str = r"[ '78044c02044c00' HEX-> ] ->ENCODER";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@BAD", BAD)
        .replace("@D1", D1)
        .replace("@D2", D2)
        .replace("@D3", D3)
        .replace("@LA", LA)
        .replace("@LB", LB)
        .replace("@LC", LC)
        .replace("@LD", LD)
        .replace("@LG", LG)
        .replace("@G1", G1)
        .replace("@E1", E1)
        .replace("@E2", E2)
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
fn pivot() {
    check(&[
        (
            r"[ @D1 ] [ @LA ] PIVOT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","la":"q"},"a":{"at":"tr"},"la":0,"v":[[3,30],[4,40]]},{"c":"d1","l":{"h":"x","la":"p"},"a":{"at":"tr"},"la":0,"v":[[1,10]]},{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[2,20],[5,50],[7,70]]}]]"#,
            ),
        ),
        (
            r"[ @D1 @D2 ] [ @LA @LB ] PIVOT",
            Ok(
                r#"[[{"c":"d2","l":{"h":"y","lb":"true"},"a":{},"la":0,"v":[[2,2.5]]},{"c":"d1","l":{"h":"x","la":"p"},"a":{"at":"tr"},"la":0,"v":[[1,10]]},{"c":"d1","l":{"h":"x","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[2,20],[7,70]]},{"c":"d2","l":{"h":"y","la":"r"},"a":{},"la":0,"v":[[6,6.5]]},{"c":"d1","l":{"h":"x","la":"q","lb":"false"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"d2","l":{"h":"y","la":"q","lb":"true"},"a":{},"la":0,"v":[[4,4.5]]},{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[5,50]]},{"c":"d1","l":{"h":"x","la":"q","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[4,40]]},{"c":"d2","l":{"h":"y"},"a":{},"la":0,"v":[[8,8.5]]}]]"#,
            ),
        ),
        (
            r"[ @D1 @D2 @D3 ] [ @LA @LB @LC ] PIVOT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","lc":"2.0"},"a":{"at":"tr"},"la":0,"v":[[5,50]]},{"c":"d2","l":{"h":"y","lb":"true"},"a":{},"la":0,"v":[[2,2.5]]},{"c":"d1","l":{"h":"x","la":"p"},"a":{"at":"tr"},"la":0,"v":[[1,10]]},{"c":"d1","l":{"h":"x","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[2,20],[7,70]]},{"c":"d3","l":{"la":"p"},"a":{},"la":0,"v":[[1,"a"],[1,"b"]]},{"c":"d2","l":{"h":"y","la":"r"},"a":{},"la":0,"v":[[6,6.5]]},{"c":"d3","l":{"la":"q","lb":"false","lc":"1.5"},"a":{},"la":0,"v":[[3,"c"]]},{"c":"d2","l":{"h":"y","la":"q","lb":"true"},"a":{},"la":0,"v":[[4,4.5]]},{"c":"d1","l":{"h":"x","la":"q","lb":"false","lc":"1.5"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"d1","l":{"h":"x","la":"q","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[4,40]]},{"c":"d2","l":{"h":"y"},"a":{},"la":0,"v":[[8,8.5]]},{"c":"d3","l":{},"a":{},"la":0,"v":[[9,"z"]]}]]"#,
            ),
        ),
        (
            r"[ @D1 @D3 ] [ @LC ] PIVOT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","lc":"2.0"},"a":{"at":"tr"},"la":0,"v":[[5,50]]},{"c":"d1","l":{"h":"x","lc":"1.5"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[1,10],[2,20],[4,40],[7,70]]},{"c":"d3","l":{"lc":"1.5"},"a":{},"la":0,"v":[[3,"c"]]},{"c":"d3","l":{},"a":{},"la":0,"v":[[1,"a"],[1,"b"],[9,"z"]]}]]"#,
            ),
        ),
        (
            r"[ @D1 ] [ @LD ] PIVOT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[1,10],[2,20],[3,30],[4,40],[5,50],[7,70]]}]]"#,
            ),
        ),
        (
            r"[ @D2 ] [ @LC @LD ] PIVOT",
            Ok(
                r#"[[{"c":"d2","l":{"h":"y"},"a":{},"la":0,"v":[[2,2.5],[4,4.5],[6,6.5],[8,8.5]]}]]"#,
            ),
        ),
        (
            r"[ @G1 ] [ @LG @LA ] PIVOT",
            Ok(
                r#"[[{"c":"d1","l":{"k":"v","lg":"9","la":"q"},"a":{},"la":0,"v":[[3,9.999999990686774,19.99999998137355,2]]},{"c":"d1","l":{"k":"v","la":"p"},"a":{},"la":0,"v":[[1,47.99999998882413,1.9999999646097422,5,1]]}]]"#,
            ),
        ),
        (
            r"[ @D1 @E2 ] [ @E1 @LB ] PIVOT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","le":"n","lb":"false"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"de","l":{"u":"w"},"a":{},"la":0,"v":[[1,1]]},{"c":"d1","l":{"h":"x","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[4,40],[7,70]]},{"c":"de","l":{"le":"n","u":"w","lb":"false"},"a":{},"la":0,"v":[[3,3]]},{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[1,10],[5,50]]},{"c":"de","l":{"u":"w","lb":"true"},"a":{},"la":0,"v":[[4,4]]},{"c":"d1","l":{"h":"x","le":"m","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[2,20]]}]]"#,
            ),
        ),
        (
            r"[ @D3 @D3 ] [ @LA ] PIVOT",
            Ok(
                r#"[[{"c":"d3","l":{"la":"q"},"a":{},"la":0,"v":[[3,"c"],[3,"c"]]},{"c":"d3","l":{"la":"p"},"a":{},"la":0,"v":[[1,"a"],[1,"b"],[1,"a"],[1,"b"]]},{"c":"d3","l":{},"a":{},"la":0,"v":[[9,"z"],[9,"z"]]}]]"#,
            ),
        ),
        (
            r"[ @D1 ] [ @LA ] PIVOT DROP @D1",
            Ok(
                r#"[{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[5,50],[1,10],[2,20],[3,30],[4,40],[7,70]]}]"#,
            ),
        ),
        (
            r"[ @D1 ] [ @LC @LA @LB ] PIVOT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","lc":"2.0"},"a":{"at":"tr"},"la":0,"v":[[5,50]]},{"c":"d1","l":{"h":"x","la":"p"},"a":{"at":"tr"},"la":0,"v":[[1,10]]},{"c":"d1","l":{"h":"x","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[2,20],[7,70]]},{"c":"d1","l":{"h":"x","la":"q","lb":"false","lc":"1.5"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"d1","l":{"h":"x","la":"q","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[4,40]]}]]"#,
            ),
        ),
        (
            r"[ @D2 @D1 ] [ @LB @LA ] PIVOT",
            Ok(
                r#"[[{"c":"d2","l":{"h":"y","lb":"true"},"a":{},"la":0,"v":[[2,2.5]]},{"c":"d1","l":{"h":"x","la":"p"},"a":{"at":"tr"},"la":0,"v":[[1,10]]},{"c":"d1","l":{"h":"x","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[2,20],[7,70]]},{"c":"d2","l":{"h":"y","la":"r"},"a":{},"la":0,"v":[[6,6.5]]},{"c":"d1","l":{"h":"x","la":"q","lb":"false"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"d2","l":{"h":"y","la":"q","lb":"true"},"a":{},"la":0,"v":[[4,4.5]]},{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[5,50]]},{"c":"d1","l":{"h":"x","la":"q","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[4,40]]},{"c":"d2","l":{"h":"y"},"a":{},"la":0,"v":[[8,8.5]]}]]"#,
            ),
        ),
        (
            r"[ @D1 ] [ ] PIVOT",
            Err(r"PIVOT expects a non empty list of labeling Geo Time Series."),
        ),
        (
            r"[ @D1 ] 'x' PIVOT",
            Err(r"PIVOT expects a non empty list of labeling Geo Time Series."),
        ),
        (
            r"[ @D1 ] [ 1 ] PIVOT",
            Err(r"PIVOT expects a list of labeling Geo Time Series."),
        ),
        (
            r"[ @D1 ] [ NEWGTS 'e' RENAME ] PIVOT",
            Err(r"PIVOT expects labeling Geo Time Series to be non empty."),
        ),
        (
            r"[ @D1 ] [ @LA @LA ] PIVOT",
            Err(r"PIVOT labeling Geo Time Series must all have different class names."),
        ),
        (
            r"[ ] [ @LA ] PIVOT",
            Err(r"PIVOT operates on a non empty list of Geo Time Series or GTS Encoders."),
        ),
        (
            r"'x' [ @LA ] PIVOT",
            Err(r"PIVOT operates on a non empty list of Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ 1 ] [ @LA ] PIVOT",
            Err(r"PIVOT operates on a list Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ NEWGTS ] [ @LA ] PIVOT",
            Err(r"PIVOT operates on non empty Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ @D1 ] [ @LA 'h' RENAME ] PIVOT",
            Err(
                r"PIVOT labeling class 'h' is already a label of a Geo Time Series or GTS Encoder to label.",
            ),
        ),
        (
            r"[ @D1 ] [ @BAD ] PIVOT",
            Err(r"PIVOT encountered an error while decoding encoder. (BufferUnderflowException)"),
        ),
        (
            r"[ @BAD ] [ @LA ] PIVOT",
            Err(r"PIVOT encountered an error while decoding encoder. (BufferUnderflowException)"),
        ),
        (r"[ @D1 ] PIVOT", Err(r"Empty stack.")),
        (r"PIVOT", Err(r"Empty stack.")),
        (
            r"[ @D1 @D2 ] [ @LA @LB ] PIVOT <% LABELS %> F LMAP",
            Ok(
                r#"[[{"h":"y","lb":"true"},{"h":"x","la":"p"},{"h":"x","lb":"true"},{"h":"y","la":"r"},{"h":"x","la":"q","lb":"false"},{"h":"y","la":"q","lb":"true"},{"h":"x"},{"h":"x","la":"q","lb":"true"},{"h":"y"}]]"#,
            ),
        ),
        (
            r"[ @D1 ] [ [ [ 2 'm' ] ] ->ENCODER @LB ] PIVOT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[2,20],[4,40],[7,70]]},{"c":"d1","l":{"h":"x","lb":"false"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[1,10],[5,50]]}]]"#,
            ),
        ),
        (
            r"[ @D1 ] [ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE ] PIVOT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[3,30],[4,40],[5,50],[7,70]]},{"c":"d1","l":{"h":"x","":"1"},"a":{"at":"tr"},"la":0,"v":[[1,10]]},{"c":"d1","l":{"h":"x","":"2"},"a":{"at":"tr"},"la":0,"v":[[2,20]]}]]"#,
            ),
        ),
        (
            r"[ NEWGTS 'a' RENAME 1 NaN NaN NaN 1 ADDVALUE NEWGTS 'b' RENAME 2 NaN NaN NaN 2 ADDVALUE NEWGTS 'c' RENAME 3 NaN NaN NaN 3 ADDVALUE NEWGTS 'd' RENAME { 'z' '1' } RELABEL 9 NaN NaN NaN 3 ADDVALUE ] [ @LC ] PIVOT",
            Ok(
                r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,1]]},{"c":"b","l":{},"a":{},"la":0,"v":[[2,2]]},{"c":"c","l":{"lc":"1.5"},"a":{},"la":0,"v":[[3,3]]},{"c":"d","l":{"z":"1"},"a":{},"la":0,"v":[[9,3]]}]]"#,
            ),
        ),
    ]);
}

#[test]
fn pivotstrict() {
    check(&[
        (
            r"[ @D1 ] [ @LA ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","la":"q"},"a":{"at":"tr"},"la":0,"v":[[3,30],[4,40]]},{"c":"d1","l":{"h":"x","la":"p"},"a":{"at":"tr"},"la":0,"v":[[1,10]]}]]"#,
            ),
        ),
        (
            r"[ @D1 @D2 ] [ @LA @LB ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","la":"q","lb":"false"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"d2","l":{"h":"y","la":"q","lb":"true"},"a":{},"la":0,"v":[[4,4.5]]},{"c":"d1","l":{"h":"x","la":"q","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[4,40]]}]]"#,
            ),
        ),
        (
            r"[ @D1 @D2 @D3 ] [ @LA @LB @LC ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d3","l":{"la":"q","lb":"false","lc":"1.5"},"a":{},"la":0,"v":[[3,"c"]]},{"c":"d1","l":{"h":"x","la":"q","lb":"false","lc":"1.5"},"a":{"at":"tr"},"la":0,"v":[[3,30]]}]]"#,
            ),
        ),
        (
            r"[ @D1 @D3 ] [ @LC ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","lc":"2.0"},"a":{"at":"tr"},"la":0,"v":[[5,50]]},{"c":"d1","l":{"h":"x","lc":"1.5"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"d3","l":{"lc":"1.5"},"a":{},"la":0,"v":[[3,"c"]]}]]"#,
            ),
        ),
        (r"[ @D1 ] [ @LD ] PIVOTSTRICT", Ok(r"[[]]")),
        (r"[ @D2 ] [ @LC @LD ] PIVOTSTRICT", Ok(r"[[]]")),
        (
            r"[ @G1 ] [ @LG @LA ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d1","l":{"k":"v","lg":"9","la":"q"},"a":{},"la":0,"v":[[3,9.999999990686774,19.99999998137355,2]]}]]"#,
            ),
        ),
        (
            r"[ @D1 @E2 ] [ @E1 @LB ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","le":"n","lb":"false"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"de","l":{"le":"n","u":"w","lb":"false"},"a":{},"la":0,"v":[[3,3]]},{"c":"d1","l":{"h":"x","le":"m","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[2,20]]}]]"#,
            ),
        ),
        (
            r"[ @D3 @D3 ] [ @LA ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d3","l":{"la":"q"},"a":{},"la":0,"v":[[3,"c"],[3,"c"]]},{"c":"d3","l":{"la":"p"},"a":{},"la":0,"v":[[1,"a"],[1,"b"],[1,"a"],[1,"b"]]}]]"#,
            ),
        ),
        (
            r"[ @D1 ] [ @LA ] PIVOTSTRICT DROP @D1",
            Ok(
                r#"[{"c":"d1","l":{"h":"x"},"a":{"at":"tr"},"la":0,"v":[[5,50],[1,10],[2,20],[3,30],[4,40],[7,70]]}]"#,
            ),
        ),
        (
            r"[ @D1 ] [ @LC @LA @LB ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","la":"q","lb":"false","lc":"1.5"},"a":{"at":"tr"},"la":0,"v":[[3,30]]}]]"#,
            ),
        ),
        (
            r"[ @D2 @D1 ] [ @LB @LA ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","la":"q","lb":"false"},"a":{"at":"tr"},"la":0,"v":[[3,30]]},{"c":"d2","l":{"h":"y","la":"q","lb":"true"},"a":{},"la":0,"v":[[4,4.5]]},{"c":"d1","l":{"h":"x","la":"q","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[4,40]]}]]"#,
            ),
        ),
        (
            r"[ @D1 ] [ ] PIVOTSTRICT",
            Err(r"PIVOTSTRICT expects a non empty list of labeling Geo Time Series."),
        ),
        (
            r"[ @D1 ] 'x' PIVOTSTRICT",
            Err(r"PIVOTSTRICT expects a non empty list of labeling Geo Time Series."),
        ),
        (
            r"[ @D1 ] [ 1 ] PIVOTSTRICT",
            Err(r"PIVOTSTRICT expects a list of labeling Geo Time Series."),
        ),
        (
            r"[ @D1 ] [ NEWGTS 'e' RENAME ] PIVOTSTRICT",
            Err(r"PIVOTSTRICT expects labeling Geo Time Series to be non empty."),
        ),
        (
            r"[ @D1 ] [ @LA @LA ] PIVOTSTRICT",
            Err(r"PIVOTSTRICT labeling Geo Time Series must all have different class names."),
        ),
        (
            r"[ ] [ @LA ] PIVOTSTRICT",
            Err(r"PIVOTSTRICT operates on a non empty list of Geo Time Series or GTS Encoders."),
        ),
        (
            r"'x' [ @LA ] PIVOTSTRICT",
            Err(r"PIVOTSTRICT operates on a non empty list of Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ 1 ] [ @LA ] PIVOTSTRICT",
            Err(r"PIVOTSTRICT operates on a list Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ NEWGTS ] [ @LA ] PIVOTSTRICT",
            Err(r"PIVOTSTRICT operates on non empty Geo Time Series or GTS Encoders."),
        ),
        (
            r"[ @D1 ] [ @LA 'h' RENAME ] PIVOTSTRICT",
            Err(
                r"PIVOTSTRICT labeling class 'h' is already a label of a Geo Time Series or GTS Encoder to label.",
            ),
        ),
        (
            r"[ @D1 ] [ @BAD ] PIVOTSTRICT",
            Err(
                r"PIVOTSTRICT encountered an error while decoding encoder. (BufferUnderflowException)",
            ),
        ),
        (
            r"[ @BAD ] [ @LA ] PIVOTSTRICT",
            Err(
                r"PIVOTSTRICT encountered an error while decoding encoder. (BufferUnderflowException)",
            ),
        ),
        (r"[ @D1 ] PIVOTSTRICT", Err(r"Empty stack.")),
        (r"PIVOTSTRICT", Err(r"Empty stack.")),
        (
            r"[ @D1 @D2 ] [ @LA @LB ] PIVOTSTRICT <% LABELS %> F LMAP",
            Ok(
                r#"[[{"h":"x","la":"q","lb":"false"},{"h":"y","la":"q","lb":"true"},{"h":"x","la":"q","lb":"true"}]]"#,
            ),
        ),
        (
            r"[ @D1 ] [ [ [ 2 'm' ] ] ->ENCODER @LB ] PIVOTSTRICT",
            Ok(r#"[[{"c":"d1","l":{"h":"x","lb":"true"},"a":{"at":"tr"},"la":0,"v":[[2,20]]}]]"#),
        ),
        (
            r"[ @D1 ] [ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE ] PIVOTSTRICT",
            Ok(
                r#"[[{"c":"d1","l":{"h":"x","":"1"},"a":{"at":"tr"},"la":0,"v":[[1,10]]},{"c":"d1","l":{"h":"x","":"2"},"a":{"at":"tr"},"la":0,"v":[[2,20]]}]]"#,
            ),
        ),
        (
            r"[ NEWGTS 'a' RENAME 1 NaN NaN NaN 1 ADDVALUE NEWGTS 'b' RENAME 2 NaN NaN NaN 2 ADDVALUE NEWGTS 'c' RENAME 3 NaN NaN NaN 3 ADDVALUE NEWGTS 'd' RENAME { 'z' '1' } RELABEL 9 NaN NaN NaN 3 ADDVALUE ] [ @LC ] PIVOTSTRICT",
            Ok(r#"[[{"c":"c","l":{"lc":"1.5"},"a":{},"la":0,"v":[[3,3]]}]]"#),
        ),
    ]);
}
