// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the Processing functions: the style settings of the Processing functions.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const G: &str = r"4 4 '2D' PGraphics";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@G", G)
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
fn style() {
    check(&[
        (r"@G 2 PstrokeWeight", Ok(r"[null]")),
        (r"@G 0 PstrokeWeight", Ok(r"[null]")),
        (r"@G 0.5 PstrokeWeight", Ok(r"[null]")),
        (r"@G -1 PstrokeWeight", Err(r"negative width")),
        (
            r"@G 'x' PstrokeWeight",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G NULL PstrokeWeight", Err(r"NullPointerException")),
        (
            r"@G PstrokeWeight",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G 1 2 PstrokeWeight",
            Err(r"Invalid number of parameters, expected one of [1] but found 2"),
        ),
        (r"@G 'SQUARE' PstrokeCap", Ok(r"[null]")),
        (r"@G 'PROJECT' PstrokeCap", Ok(r"[null]")),
        (r"@G 'ROUND' PstrokeCap", Ok(r"[null]")),
        (
            r"@G 'square' PstrokeCap",
            Err(r"PstrokeCap: invalid mode, should be 'SQUARE', 'PROJECT' or 'ROUND'."),
        ),
        (
            r"@G 1 PstrokeCap",
            Err(r"PstrokeCap: invalid mode, should be 'SQUARE', 'PROJECT' or 'ROUND'."),
        ),
        (r"@G NULL PstrokeCap", Err(r"NullPointerException")),
        (
            r"@G PstrokeCap",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (r"@G 'MITER' PstrokeJoin", Ok(r"[null]")),
        (r"@G 'BEVEL' PstrokeJoin", Ok(r"[null]")),
        (r"@G 'ROUND' PstrokeJoin", Ok(r"[null]")),
        (
            r"@G 'SQUARE' PstrokeJoin",
            Err(r"PstrokeJoin: invalid mode, should be 'MITER', 'BEVEL' or 'ROUND'."),
        ),
        (r"@G 'CENTER' PellipseMode", Ok(r"[null]")),
        (r"@G 'RADIUS' PellipseMode", Ok(r"[null]")),
        (r"@G 'CORNER' PellipseMode", Ok(r"[null]")),
        (r"@G 'CORNERS' PellipseMode", Ok(r"[null]")),
        (
            r"@G 'LEFT' PellipseMode",
            Err(
                r"PellipseMode: invalid mode, should be 'CENTER', 'RADIUS', 'CORNER' or 'CORNERS'.",
            ),
        ),
        (r"@G 'CENTER' PrectMode", Ok(r"[null]")),
        (r"@G 'RADIUS' PrectMode", Ok(r"[null]")),
        (r"@G 'CORNER' PrectMode", Ok(r"[null]")),
        (r"@G 'CORNERS' PrectMode", Ok(r"[null]")),
        (
            r"@G 'BOTTOM' PrectMode",
            Err(r"PrectMode: invalid mode, should be 'CENTER', 'RADIUS', 'CORNER' or 'CORNERS'."),
        ),
        (r"@G 'CENTER' PimageMode", Ok(r"[null]")),
        (r"@G 'CORNER' PimageMode", Ok(r"[null]")),
        (r"@G 'CORNERS' PimageMode", Ok(r"[null]")),
        (
            r"@G 'RADIUS' PimageMode",
            Err(r"PimageMode: invalid mode, should be 'CENTER', 'CORNER' or 'CORNERS'."),
        ),
        (r"@G 'CENTER' PshapeMode", Ok(r"[null]")),
        (r"@G 'CORNER' PshapeMode", Ok(r"[null]")),
        (r"@G 'CORNERS' PshapeMode", Ok(r"[null]")),
        (
            r"@G 'RADIUS' PshapeMode",
            Err(r"PshapeMode: invalid mode, should be 'CENTER', 'CORNER' or 'CORNERS'."),
        ),
        (r"@G 20 PbezierDetail", Ok(r"[null]")),
        (r"@G 0 PbezierDetail", Ok(r"[null]")),
        (r"@G -1 PbezierDetail", Ok(r"[null]")),
        (r"@G 2.7 PbezierDetail", Ok(r"[null]")),
        (
            r"@G 'x' PbezierDetail",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G PbezierDetail",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (r"@G 20 PcurveDetail", Ok(r"[null]")),
        (r"@G 0 PcurveDetail", Ok(r"[null]")),
        (r"@G -1 PcurveDetail", Ok(r"[null]")),
        (r"@G 2.7 PcurveDetail", Ok(r"[null]")),
        (
            r"@G 'x' PcurveDetail",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G 0.5 PcurveTightness", Ok(r"[null]")),
        (r"@G 0 PcurveTightness", Ok(r"[null]")),
        (
            r"@G 'x' PcurveTightness",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 1 2 PcurveTightness",
            Err(r"Invalid number of parameters, expected one of [1] but found 2"),
        ),
        (r"@G 30 PsphereDetail", Ok(r"[null]")),
        (r"@G 3 4 PsphereDetail", Ok(r"[null]")),
        (r"@G 0 0 PsphereDetail", Ok(r"[null]")),
        (r"@G 1.5 PsphereDetail", Ok(r"[null]")),
        (
            r"@G 'x' PsphereDetail",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 1 2 3 PsphereDetail",
            Err(r"Invalid number of parameters, expected one of [1, 2] but found 3"),
        ),
        (
            r"@G PsphereDetail",
            Err(r"Invalid number of parameters, expected one of [1, 2] but found 0"),
        ),
        (
            r"@G 3 'x' PsphereDetail",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G PstrokeJoin",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G PellipseMode",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G PrectMode",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G PimageMode",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G PshapeMode",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G PcurveDetail",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
    ]);
}
