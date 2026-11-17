// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the curve functions of the Processing API.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const G: &str = r"4 4 '2D' PGraphics 0 Pbackground";
const P: &str = r"4 4 '2D' PGraphics 0 Pbackground 255 Pstroke";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@G", G).replace("@P", P)
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
fn curve() {
    check(&[
        (
            r"@P 1 2 3 4 5 6 7 Pbezier",
            Err(r"Invalid number of parameters, expected one of [8, 12] but found 7"),
        ),
        (
            r"@P 1 2 3 4 5 6 7 8 9 10 11 Pbezier",
            Err(r"Invalid number of parameters, expected one of [8, 12] but found 11"),
        ),
        (
            r"@P 1 2 3 4 5 6 7 8 9 10 11 12 Pbezier",
            Err(r"Pbezier need a 3D container to draw a 3D Bezier curve."),
        ),
        (
            r"@P 1 2 3 4 5 6 7 8 9 10 11 12 Pcurve",
            Err(r"Pcurve need a 3D container to draw a 3D Bezier curve."),
        ),
        (
            r"@P 1 2 3 4 5 6 7 'x' Pbezier",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@P 1 2 3 4 5 6 7 8 Pcurve 0 0 Pget",
            Ok(r"[-16777216,null]"),
        ),
        (r"@G 0 10 10 20 0.5 PbezierPoint", Ok(r"[10.0,null]")),
        (r"@G 0 10 10 20 0.25 PbezierPoint", Ok(r"[5.9375,null]")),
        (r"@G 0 10 10 20 1.5 PbezierPoint", Ok(r"[45.0,null]")),
        (r"@G 0 10 10 20 0.5 PbezierTangent", Ok(r"[15.0,null]")),
        (r"@G 0 10 10 20 0.25 PbezierTangent", Ok(r"[18.75,null]")),
        (r"@G 0 10 10 20 0.5 PcurvePoint", Ok(r"[10.0,null]")),
        (
            r"@G 0 PcurveTightness 0 10 10 20 0.25 PcurvePoint",
            Ok(r"[10.46875,null]"),
        ),
        (
            r"@G 0.5 PcurveTightness 0 10 10 20 0.25 PcurvePoint",
            Ok(r"[10.234375,null]"),
        ),
        (
            r"@G 0.5 PcurveTightness 0 10 10 20 0.25 PcurveTangent",
            Ok(r"[-0.3125,null]"),
        ),
        (r"@G 0 10 10 20 0.5 PcurveTangent", Ok(r"[-2.5,null]")),
        (
            r"@G 0 10 10 20 NULL PbezierPoint",
            Err(r"NullPointerException"),
        ),
        (
            r"@G 0 10 10 'x' 0.5 PbezierPoint",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 0 10 10 20 PbezierPoint",
            Err(r"Invalid number of parameters, expected one of [5] but found 4"),
        ),
        (
            r"@G 0 10 10 20 0.5 6 PbezierPoint",
            Err(r"Invalid number of parameters, expected one of [5] but found 6"),
        ),
        (
            r"@G 0 10 10 20 1e30 PbezierPoint",
            Err(r"not a finite number"),
        ),
        (r"@G 0 10 10 20 0.5 PcurveTangent", Ok(r"[-2.5,null]")),
        (
            r"@G 0 10 10 20 0.5 'x' PcurvePoint",
            Err(r"Invalid number of parameters, expected one of [5] but found 6"),
        ),
        (
            r"@G 0 10 10 20 0.5 'x' PcurveTangent",
            Err(r"Invalid number of parameters, expected one of [5] but found 6"),
        ),
    ]);
}
