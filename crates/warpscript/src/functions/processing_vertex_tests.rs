// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the vertex functions of the Processing API.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const F: &str = r"4 4 '2D' PGraphics 0 Pbackground 255 Pfill PnoStroke";
const P: &str = r"4 4 '2D' PGraphics 0 Pbackground 255 Pstroke";
const E: &str = r"8 8 '2D' PGraphics 0 Pbackground 255 Pfill PnoStroke";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@F", F).replace("@P", P).replace("@E", E)
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
fn shape() {
    check(&[
        (
            r"@F PbeginShape 1 1 Pvertex 4 1 Pvertex 4 4 Pvertex 1 4 Pvertex PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]",
            ),
        ),
        (
            r"@F PbeginShape 1 1 Pvertex 4 1 Pvertex 4 4 Pvertex 1 4 Pvertex 'CLOSE' PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]",
            ),
        ),
        (
            r"@F PbeginShape 1 1 Pvertex 4 1 Pvertex 4 4 Pvertex 1 4 Pvertex 'OPEN' PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]",
            ),
        ),
        (
            r"@F 'POLYGON' PbeginShape 1 1 Pvertex 4 1 Pvertex 4 4 Pvertex 1 4 Pvertex PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]",
            ),
        ),
        (
            r"@F 'QUADS' PbeginShape 0 0 Pvertex 2 0 Pvertex 2 2 Pvertex 0 2 Pvertex PendShape Ppixels",
            Ok(
                r"[[-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F 'QUAD_STRIP' PbeginShape 0 0 Pvertex 0 2 Pvertex 2 0 Pvertex 2 2 Pvertex PendShape Ppixels",
            Ok(
                r"[[-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F 'TRIANGLES' PbeginShape 0 0 Pvertex 4 0 Pvertex 0 4 Pvertex PendShape 1 1 Pget",
            Ok(r"[-1,null]"),
        ),
        (
            r"@F 'TRIANGLES' PbeginShape 0 0 Pvertex 4 0 Pvertex 0 4 Pvertex PendShape 3 3 Pget",
            Ok(r"[-16777216,null]"),
        ),
        (
            r"@P 'POINTS' PbeginShape 1 1 Pvertex 2 2 Pvertex PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@P 'LINES' PbeginShape 1 1 Pvertex 4 1 Pvertex PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F PbeginShape 0 0 Pvertex 2 2 Pvertex PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F PbeginShape PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F 1 1 Pvertex Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F 'X' PbeginShape",
            Err(
                r"PbeginShape: invalid mode, should be 'POLYGON', 'POINTS', 'LINES', 'TRIANGLES', 'TRIANGLE_STRIP', 'TRIANGLE_FAN', 'QUADS' or 'QUAD_STRIP'.",
            ),
        ),
        (
            r"@F 1 PbeginShape",
            Err(
                r"PbeginShape: invalid mode, should be 'POLYGON', 'POINTS', 'LINES', 'TRIANGLES', 'TRIANGLE_STRIP', 'TRIANGLE_FAN', 'QUADS' or 'QUAD_STRIP'.",
            ),
        ),
        (r"@F PbeginShape 'CLOSE' PendShape", Ok(r"[null]")),
        (
            r"@F PbeginShape 1 1 Pvertex 'X' PendShape",
            Err(r"PendShape: invalid mode, should be 'OPEN' or 'CLOSE'."),
        ),
        (
            r"@F PbeginShape 1 Pvertex PendShape Ppixels",
            Err(r"Invalid number of parameters, expected one of [2, 3, 4, 5] but found 1"),
        ),
        (
            r"@F PbeginShape 1 2 3 4 5 6 Pvertex PendShape Ppixels",
            Err(r"Invalid number of parameters, expected one of [2, 3, 4, 5] but found 6"),
        ),
        (
            r"@F PbeginShape 1 2 3 4 5 6 7 Pvertex PendShape Ppixels",
            Err(r"Invalid number of parameters, expected one of [2, 3, 4, 5] but found 6"),
        ),
        (
            r"@F PbeginShape 1 2 3 4 5 Pvertex 'CLOSE' PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F PbeginShape 1 2 3 Pvertex 'CLOSE' PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F PbeginContour PendContour Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F PbeginShape PbeginContour PendContour PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F 'POINTS' PbeginShape PbeginContour 1 1 Pvertex PendContour PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
    ]);
}

#[test]
fn contour() {
    check(&[
        (
            r"@E PbeginShape 0 0 Pvertex 8 0 Pvertex 8 8 Pvertex 0 8 Pvertex PbeginContour 2 2 Pvertex 6 2 Pvertex 6 6 Pvertex 2 6 Pvertex PendContour 'CLOSE' PendShape Ppixels",
            Ok(
                r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]",
            ),
        ),
        (
            r"@E PbeginShape 0 0 Pvertex 8 0 Pvertex 8 8 Pvertex 0 8 Pvertex PbeginContour 2 2 Pvertex 6 2 Pvertex 6 6 Pvertex 2 6 Pvertex PendContour PendShape Ppixels",
            Ok(
                r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]",
            ),
        ),
        (
            r"@E PbeginShape 0 0 Pvertex 8 0 Pvertex 8 8 Pvertex 0 8 Pvertex PbeginContour 2 2 Pvertex 6 2 Pvertex 6 6 Pvertex 2 6 Pvertex PendShape Ppixels",
            Ok(
                r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]",
            ),
        ),
        (
            r"@E PbeginShape 0 0 Pvertex 8 0 Pvertex 8 8 Pvertex 0 8 Pvertex PbeginContour 2 2 Pvertex 6 2 Pvertex 6 6 Pvertex 2 6 Pvertex PendContour 2 4 Pget",
            Ok(r"[-16777216,null]"),
        ),
        (
            r"@E PbeginShape 0 0 Pvertex 8 0 Pvertex 8 8 Pvertex 0 8 Pvertex PbeginContour 2 2 Pvertex 6 2 Pvertex 6 6 Pvertex 2 6 Pvertex PendContour 1 4 Pget",
            Ok(r"[-16777216,null]"),
        ),
        (
            r"@E 'POINTS' PbeginShape 0 0 Pvertex 8 0 Pvertex PbeginContour 2 2 Pvertex PendContour PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
    ]);
}

#[test]
fn bezier_vertex() {
    check(&[
        (
            r"@F PbezierVertex",
            Err(r"Invalid number of parameters, expected one of [6, 9] but found 0"),
        ),
        (
            r"@F 1 2 3 4 5 6 PbezierVertex",
            Err(
                r"beginShape() or beginShape(POLYGON) must be used before bezierVertex() or quadraticVertex()",
            ),
        ),
        (
            r"@F 'POINTS' PbeginShape 1 1 Pvertex 1 2 3 4 5 6 PbezierVertex",
            Err(
                r"beginShape() or beginShape(POLYGON) must be used before bezierVertex() or quadraticVertex()",
            ),
        ),
        (
            r"@F PbeginShape 1 2 3 4 5 6 PbezierVertex",
            Err(r"vertex() must be used at least once before bezierVertex() or quadraticVertex()"),
        ),
        (
            r"@F PbeginShape 1 2 3 4 5 6 7 PbezierVertex",
            Err(r"Invalid number of parameters, expected one of [6, 9] but found 7"),
        ),
        (
            r"@F PbeginShape 1 2 3 4 5 PbezierVertex",
            Err(r"Invalid number of parameters, expected one of [6, 9] but found 5"),
        ),
        (
            r"@F PbeginShape 1 1 Pvertex 1 2 3 4 5 6 7 8 PbezierVertex",
            Err(r"Invalid number of parameters, expected one of [6, 9] but found 8"),
        ),
        (
            r"@F 'POINTS' PbeginShape 1 1 Pvertex PbezierVertex Ppixels",
            Err(r"Invalid number of parameters, expected one of [6, 9] but found 0"),
        ),
    ]);
}

#[test]
fn quadratic_vertex() {
    check(&[
        (
            r"@F 1 2 3 4 PquadraticVertex",
            Err(
                r"beginShape() or beginShape(POLYGON) must be used before bezierVertex() or quadraticVertex()",
            ),
        ),
        (
            r"@F 'POINTS' PbeginShape 1 1 Pvertex 1 2 3 4 PquadraticVertex",
            Err(
                r"beginShape() or beginShape(POLYGON) must be used before bezierVertex() or quadraticVertex()",
            ),
        ),
        (
            r"@F PbeginShape 1 2 3 4 PquadraticVertex",
            Err(r"vertex() must be used at least once before bezierVertex() or quadraticVertex()"),
        ),
        (
            r"@F PbeginShape 1 2 3 PquadraticVertex",
            Err(r"Invalid number of parameters, expected one of [4, 6] but found 3"),
        ),
        (
            r"@F PbeginShape 1 2 3 4 5 PquadraticVertex",
            Err(r"Invalid number of parameters, expected one of [4, 6] but found 5"),
        ),
    ]);
}

#[test]
fn curve_vertex() {
    check(&[
        (
            r"@F 1 2 PcurveVertex",
            Err(r"You must use beginShape() or beginShape(POLYGON) before curveVertex()"),
        ),
        (
            r"@F 'LINES' PbeginShape 1 2 PcurveVertex",
            Err(r"You must use beginShape() or beginShape(POLYGON) before curveVertex()"),
        ),
        (
            r"@F PbeginShape 1 2 PcurveVertex PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@F PbeginShape 1 PcurveVertex",
            Err(r"Invalid number of parameters, expected one of [2, 3] but found 1"),
        ),
        (
            r"@F PbeginShape 1 2 3 4 5 PcurveVertex",
            Err(r"Invalid number of parameters, expected one of [2, 3] but found 4"),
        ),
        (
            r"@F PbeginShape 1 2 3 PcurveVertex PendShape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
    ]);
}
