// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the Processing shapes read from SVG (`PloadShape`, `Pshape`), drawn in a 2D context.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const G: &str = r"4 4 '2D' PGraphics 0 Pbackground";
const TEN: &str = r"10 10 '2D' PGraphics 0 Pbackground";
const INNER: &str = r##"'<svg width="4" height="4"><rect x="1" y="1" width="2" height="2" fill="#ffffff"/></svg>'"##;
const SQUARE: &str = r##"'<svg width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="#ffffff"/></svg>'"##;
const CIRCLE: &str =
    r##"'<svg width="4" height="4"><circle cx="2" cy="2" r="2" fill="#ffffff"/></svg>'"##;
const PATHSQ: &str =
    r##"'<svg width="4" height="4"><path d="M0 0 L4 0 L4 4 L0 4 Z" fill="#ffffff"/></svg>'"##;
const POLY: &str =
    r##"'<svg width="4" height="4"><polygon points="0,0 4,0 4,4 0,4" fill="#ffffff"/></svg>'"##;
const GROUP: &str = r#"'<svg width="4" height="4"><g fill="white"><rect x="0" y="0" width="2" height="4"/></g></svg>'"#;
const STROKED: &str = r##"'<svg width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="#ffffff" stroke="#ff0000"/></svg>'"##;
const NOHEIGHT: &str =
    r##"'<svg width="4"><rect x="0" y="0" width="4" height="4" fill="#ffffff"/></svg>'"##;
const NOWIDTH: &str =
    r##"'<svg width="4" height="4"><rect x="0" y="0" height="4" fill="#ffffff"/></svg>'"##;
const BADCOLOUR: &str = r##"'<svg width="4" height="4"><rect x="0" y="0" width="4" height="4" fill="#zzzzzz"/></svg>'"##;
const BADPATH: &str = r##"'<svg width="4" height="4"><path d="M0 0 Q4 4" fill="#ffffff"/></svg>'"##;
const NOTSVG: &str = r"'<html/>'";
const VIEWBOX: &str = r##"'<svg width="4" height="4" viewBox="0 0 8 8"><rect x="0" y="0" width="4" height="4" fill="#ffffff"/></svg>'"##;
const VIEWONLY: &str =
    r##"'<svg viewBox="0 0 8 8"><rect x="0" y="0" width="4" height="4" fill="#ffffff"/></svg>'"##;

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@BADCOLOUR", BADCOLOUR)
        .replace("@NOHEIGHT", NOHEIGHT)
        .replace("@VIEWONLY", VIEWONLY)
        .replace("@STROKED", STROKED)
        .replace("@NOWIDTH", NOWIDTH)
        .replace("@BADPATH", BADPATH)
        .replace("@VIEWBOX", VIEWBOX)
        .replace("@SQUARE", SQUARE)
        .replace("@CIRCLE", CIRCLE)
        .replace("@PATHSQ", PATHSQ)
        .replace("@NOTSVG", NOTSVG)
        .replace("@INNER", INNER)
        .replace("@GROUP", GROUP)
        .replace("@POLY", POLY)
        .replace("@TEN", TEN)
        .replace("@G", G)
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
fn place() {
    check(&[
        (
            r"@G @INNER PloadShape 0 0 Pshape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G @INNER PloadShape 1 0 Pshape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G @INNER PloadShape -1 -1 Pshape Ppixels",
            Ok(
                r"[[-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G @SQUARE PloadShape 2 2 Pshape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-16777216,-16777216,-1,-1],null]",
            ),
        ),
        (
            r"@G @SQUARE PloadShape 0 0 Pshape Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (r"@G @SQUARE PloadShape 0 0 Pshape", Ok(r"[null]")),
        (
            r"@G @SQUARE PloadShape Pshape Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G @SQUARE PloadShape 0 Pshape Ppixels",
            Err(r"Invalid number of parameters, expected one of [1, 3, 5] but found 2"),
        ),
        (
            r"@G @SQUARE PloadShape 0 0 0 Pshape Ppixels",
            Err(r"Invalid number of parameters, expected one of [1, 3, 5] but found 4"),
        ),
        (
            r"@G @SQUARE PloadShape 0 0 4 Pshape Ppixels",
            Err(r"Invalid number of parameters, expected one of [1, 3, 5] but found 4"),
        ),
        (
            r"@G 5 0 0 Pshape Ppixels",
            Err(r"class java.lang.Long cannot be cast to class processing.core.PShape"),
        ),
    ]);
}

#[test]
fn scale() {
    check(&[
        (
            r"@G @SQUARE PloadShape 0 0 2 2 Pshape Ppixels",
            Ok(
                r"[[-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G @SQUARE PloadShape 2 2 4 4 Pshape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-16777216,-16777216,-1,-1],null]",
            ),
        ),
        (
            r"@G @SQUARE PloadShape -2 -2 4 4 Pshape Ppixels",
            Ok(
                r"[[-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G @SQUARE PloadShape 0 0 1 1 Pshape Ppixels",
            Ok(
                r"[[-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G @SQUARE PloadShape 0 0 8 8 Pshape Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G @SQUARE PloadShape 0 0 2 8 Pshape Ppixels",
            Ok(
                r"[[-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G @NOHEIGHT PloadShape 0 0 Pshape Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G @NOHEIGHT PloadShape 0 0 1 1 Pshape Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G @VIEWBOX PloadShape 0 0 Pshape Ppixels",
            Ok(
                r"[[-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G @VIEWBOX PloadShape 2 2 4 4 Pshape Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-16777216,-16777216,-1,-1],null]",
            ),
        ),
        (
            r"@G @VIEWONLY PloadShape 0 0 2 2 Pshape Ppixels",
            Ok(
                r"[[-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
    ]);
}

#[test]
fn pieces() {
    check(&[
        (
            r"@TEN @CIRCLE PloadShape 0 0 Pshape 1 1 Pget",
            Ok(r"[-1,null]"),
        ),
        (
            r"@TEN @CIRCLE PloadShape 0 0 Pshape 5 5 Pget",
            Ok(r"[-16777216,null]"),
        ),
        (
            r"@TEN @CIRCLE PloadShape 0 0 Pshape 9 9 Pget",
            Ok(r"[-16777216,null]"),
        ),
        (
            r"@G @PATHSQ PloadShape 0 0 Pshape Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G @POLY PloadShape 0 0 Pshape Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G @GROUP PloadShape 0 0 Pshape Ppixels",
            Ok(
                r"[[-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216,-1,-1,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G @STROKED PloadShape 0 0 Pshape Ppixels",
            Ok(
                r"[[-65536,-65536,-65536,-65536,-65536,-1,-1,-1,-65536,-1,-1,-1,-65536,-1,-1,-1],null]",
            ),
        ),
    ]);
}

#[test]
fn refusals() {
    check(&[
        (
            r"@G @SQUARE PloadShape 'x' 0 Pshape Ppixels",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G @SQUARE PloadShape 0 0 'x' 4 Pshape Ppixels",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"PloadShape", Err(r"Empty stack.")),
    ]);
}

#[test]
fn load_errors() {
    check(&[
        (
            r"@G 'not svg' PloadShape",
            Err(r"PloadShape caught an exception while loading SVG."),
        ),
        (
            r"@G 5 PloadShape",
            Err(r"PloadShape caught an exception while loading SVG."),
        ),
        (
            r"@G @NOTSVG PloadShape",
            Err(r"PloadShape caught an exception while loading SVG."),
        ),
        (
            r"@G @BADCOLOUR PloadShape 0 0 Pshape Ppixels",
            Err(r"PloadShape caught an exception while loading SVG."),
        ),
        (
            r"@G @NOWIDTH PloadShape 0 0 Pshape Ppixels",
            Err(r"PloadShape caught an exception while loading SVG."),
        ),
        (
            r"@G @BADPATH PloadShape 0 0 Pshape Ppixels",
            Err(r"PloadShape caught an exception while loading SVG."),
        ),
        (
            r"@G 'x' PloadShape 0 0 Pshape Ppixels",
            Err(r"PloadShape caught an exception while loading SVG."),
        ),
    ]);
}
