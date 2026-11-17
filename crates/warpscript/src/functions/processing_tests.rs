// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the Processing functions: structure and maths of the Processing functions.
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
fn structure() {
    check(&[
        (r"@G PpushStyle PpopStyle", Ok(r"[null]")),
        (
            r"@G PpopStyle",
            Err(r"Too many popStyle() without enough pushStyle()"),
        ),
        (
            r"@G PpushStyle PpushStyle PpopStyle PpopStyle",
            Ok(r"[null]"),
        ),
        (
            r"@G PpushStyle PpopStyle PpopStyle",
            Err(r"Too many popStyle() without enough pushStyle()"),
        ),
        (r"@G 'ADD' PblendMode", Ok(r"[null]")),
        (r"@G 'REPLACE' PblendMode", Ok(r"[null]")),
        (
            r"@G 'bogus' PblendMode",
            Err(
                r"PblendMode: invalid mode, should be 'BLEND', 'ADD', 'SUBTRACT', 'DARKEST', 'LIGHTEST', 'DIFFERENCE', 'EXCLUSION', 'MULTIPLY', 'SCREEN', or 'REPLACE'.",
            ),
        ),
        (
            r"@G 1 PblendMode",
            Err(
                r"PblendMode: invalid mode, should be 'BLEND', 'ADD', 'SUBTRACT', 'DARKEST', 'LIGHTEST', 'DIFFERENCE', 'EXCLUSION', 'MULTIPLY', 'SCREEN', or 'REPLACE'.",
            ),
        ),
        (r"@G NULL PblendMode", Err(r"NullPointerException")),
        (
            r"@G PblendMode",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G 'ADD' 'SUBTRACT' PblendMode",
            Err(r"Invalid number of parameters, expected one of [1] but found 2"),
        ),
        (r"@G 0 0 10 10 Pclip", Ok(r"[null]")),
        (r"@G 0 0 10 10 Pclip 2 2 20 20 Pclip", Ok(r"[null]")),
        (r"@G 0 0 10 10 Pclip PnoClip", Ok(r"[null]")),
        (r"@G PnoClip", Ok(r"[null]")),
        (
            r"@G 0 0 10 Pclip",
            Err(r"Invalid number of parameters, expected one of [4] but found 3"),
        ),
        (
            r"@G 0 0 10 'x' Pclip",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G 0 0 10 NULL Pclip", Err(r"NullPointerException")),
        (
            r"@G 1 PnoClip",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
        (r"4 3 '2D' PGraphics", Ok(r"[null]")),
        (r"4 3 '2D2' PGraphics", Ok(r"[null]")),
        (r"4 3 '2D-1' PGraphics", Ok(r"[null]")),
        (r"4 3 '2D4' PGraphics", Ok(r"[null]")),
        (r"4 3 '2Dx' PGraphics", Err(r#"For input string: "x""#)),
        (
            r"4 3 'x' PGraphics",
            Err(r"PGraphics expects a type ('2D' or '3D') on top of the stack."),
        ),
        (
            r"4 3 2 PGraphics",
            Err(r"PGraphics expects a type ('2D' or '3D') on top of the stack."),
        ),
        (
            r"4 PGraphics",
            Err(r"PGraphics expects a type ('2D' or '3D') on top of the stack."),
        ),
        (r"'2D' PGraphics", Err(r"Empty stack.")),
        (r"3 '2D' PGraphics", Err(r"Empty stack.")),
        (
            r"0 10 '2D' PGraphics",
            Err(r"Width (0) and height (10) cannot be <= 0"),
        ),
        (
            r"-1 -1 '2D' PGraphics",
            Err(r"Width (-1) and height (-1) cannot be <= 0"),
        ),
        (
            r"4 3 NULL PGraphics",
            Err(r"PGraphics expects a type ('2D' or '3D') on top of the stack."),
        ),
        (
            r"4 'x' '2D' PGraphics",
            Err(r"PGraphics expects a height in pixels below the type."),
        ),
        (
            r"4.5 3 '2D' PGraphics",
            Err(r"PGraphics expects a width in pixels below the height."),
        ),
    ]);
}

#[test]
fn maths() {
    check(&[
        (r"@G 0 0 10 10 Pdist", Ok(r"[14.142135620117188,null]")),
        (r"@G 1 2 3 4 5 6 Pdist", Ok(r"[0.0,null]")),
        (r"@G 1.5 2.5 1 2 Pdist", Ok(r"[0.7071067690849304,null]")),
        (
            r"@G 0 0 Pdist",
            Err(r"Invalid number of parameters, expected one of [4, 6] but found 2"),
        ),
        (
            r"@G 0 0 0 Pdist",
            Err(r"Invalid number of parameters, expected one of [4, 6] but found 3"),
        ),
        (
            r"@G 'a' 0 0 0 Pdist",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 1 2 3 4 5 Pdist",
            Err(r"Invalid number of parameters, expected one of [4, 6] but found 5"),
        ),
        (r"@G 0 0 0 NULL Pdist", Err(r"NullPointerException")),
        (r"@G 1 2 Pmag", Ok(r"[2.2360680103302,null]")),
        (r"@G 1 2 2 Pmag", Ok(r"[3.0,null]")),
        (r"@G 3 4 Pmag", Ok(r"[5.0,null]")),
        (
            r"@G 1 Pmag",
            Err(r"Invalid number of parameters, expected one of [2, 3] but found 1"),
        ),
        (
            r"@G 1 2 3 4 Pmag",
            Err(r"Invalid number of parameters, expected one of [2, 3] but found 4"),
        ),
        (
            r"@G 'x' 1 Pmag",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G 3 0 10 Pconstrain", Ok(r"[3,null]")),
        (r"@G 3.5 0 10 Pconstrain", Ok(r"[3.5,null]")),
        (r"@G -3 0 10 Pconstrain", Ok(r"[0,null]")),
        (r"@G 30 0 10 Pconstrain", Ok(r"[10,null]")),
        (r"@G 30.5 0 10 Pconstrain", Ok(r"[10.0,null]")),
        (r"@G 3 0.5 10 Pconstrain", Ok(r"[3,null]")),
        (
            r"@G 1 2 Pconstrain",
            Err(r"Invalid number of parameters, expected one of [3] but found 2"),
        ),
        (
            r"@G 'x' 0 10 Pconstrain",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G 5 0 10 Pconstrain", Ok(r"[5,null]")),
        (r"@G 0 10 0.5 Plerp", Ok(r"[5.0,null]")),
        (r"@G 3 0 10 Plerp", Ok(r"[-27.0,null]")),
        (
            r"@G 0 10 'x' Plerp",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 0 10 Plerp",
            Err(r"Invalid number of parameters, expected one of [3] but found 2"),
        ),
        (r"@G 0 10 0.25 Plerp", Ok(r"[2.5,null]")),
        (
            r"@G 0 10 1 2 Plerp",
            Err(r"Invalid number of parameters, expected one of [3] but found 4"),
        ),
        (r"@G 15 10 20 Pnorm", Ok(r"[0.5,null]")),
        (r"@G 10 10 20 Pnorm", Ok(r"[0.0,null]")),
        (
            r"@G 0 1 2 3 Pnorm",
            Err(r"Invalid number of parameters, expected one of [3] but found 4"),
        ),
        (
            r"@G 15 10 20 'x' Pnorm",
            Err(r"Invalid number of parameters, expected one of [3] but found 4"),
        ),
        (r"@G 5 0 10 0 20 Pmap", Ok(r"[10.0,null]")),
        (r"@G 1 1 5 0 10 Pmap", Ok(r"[0.0,null]")),
        (
            r"@G 5 0 10 0 Pmap",
            Err(r"Invalid number of parameters, expected one of [5] but found 4"),
        ),
        (
            r"@G 5 0 10 0 20 'x' Pmap",
            Err(r"Invalid number of parameters, expected one of [5] but found 6"),
        ),
        (r"@G 'a' 'b' 'c' 'd' 'e' 'f' Pdist", Ok(r"[0.0,null]")),
        (
            r"0 0 10 10 Pdist",
            Err(r"class java.lang.Long cannot be cast to class processing.core.PGraphics"),
        ),
        (
            r"Pdist",
            Err(r"Invalid number of parameters, expected one of [4, 6] but found 0"),
        ),
        (
            r"0 0 0 0 0 Pdist",
            Err(r"Invalid number of parameters, expected one of [4, 6] but found 5"),
        ),
        (
            r"1 2 3 4 5 6 7 8 Pdist",
            Err(r"Invalid number of parameters, expected one of [4, 6] but found 7"),
        ),
        (
            r"'x' Pmag",
            Err(r"Invalid number of parameters, expected one of [2, 3] but found 1"),
        ),
        (
            r"NULL Pmag",
            Err(r"Invalid number of parameters, expected one of [2, 3] but found 1"),
        ),
    ]);
}
