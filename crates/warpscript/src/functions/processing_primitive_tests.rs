// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.


//! Oracle-checked tests of the Processing primitive shapes: the primitive shapes of the Processing API.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const G: &str = r"4 4 '2D' PGraphics 0 Pbackground";
const F: &str = r"4 4 '2D' PGraphics 0 Pbackground 255 Pfill PnoStroke";
const S: &str = r"4 4 '2D' PGraphics 0 Pbackground 255 Pstroke PnoFill";
const B: &str = r"6 6 '2D' PGraphics 0 Pbackground 255 Pfill PnoStroke";
const T: &str = r"10 10 '2D' PGraphics 0 Pbackground 255 Pfill PnoStroke";
const P: &str = r"4 4 '2D' PGraphics 0 Pbackground 255 Pstroke";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@G", G)
        .replace("@F", F)
        .replace("@S", S)
        .replace("@B", B)
        .replace("@T", T)
        .replace("@P", P)
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
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn point() {
    check(&[
        (r"@P 1 1 Ppoint Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@P 1.5 1.5 Ppoint Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@P 1 1 3 Ppoint Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@P 1 1 'x' Ppoint Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@P 1 Ppoint", Err(r"Invalid number of parameters, expected one of [2, 3] but found 1")),
        (r"@P 1 'x' Ppoint", Err(r"class java.lang.String cannot be cast to class java.lang.Number")),
        (r"@P 1 NULL Ppoint", Err(r"NullPointerException")),
        (r"@P 2 2 3 PstrokeWeight 2 2 Ppoint Ppixels", Err(r"Invalid number of parameters, expected one of [1] but found 2")),
        (r"@P 0 PstrokeWeight 1 1 Ppoint Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@P 1 1 Ppoint 2 2 Ppoint Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@G 1 1 Ppoint", Ok(r"[null]")),
    ]);
}

#[test]
fn line() {
    check(&[
        (r"@P 1 1 5 1 Pline Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@P 1 3 6 3 Pline Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1],null]")),
        (r"@P 3 1 3 5 Pline Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-1],null]")),
        (r"@P 1 1 5 5 Pline 2 2 Pget", Ok(r"[-1,null]")),
        (r"@P 1 3 6 Pline", Err(r"Invalid number of parameters, expected one of [4, 6] but found 3")),
        (r"@P 1 3 6 3 'x' Pline", Err(r"Invalid number of parameters, expected one of [4, 6] but found 5")),
        (r"@P 1 'x' 6 3 Pline", Err(r"class java.lang.String cannot be cast to class java.lang.Number")),
        (r"@P 2 2 3 3 Pline Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-1],null]")),
        (r"@P 0 0 4 4 Pline Ppixels", Ok(r"[[-1,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-1,-16777216,-16777216,-16777216,-16777216,-1],null]")),
    ]);
}

#[test]
fn triangle() {
    check(&[
        (r"@T 0 0 8 0 0 8 Ptriangle 1 1 Pget", Ok(r"[-1,null]")),
        (r"@T 0 0 8 0 0 8 Ptriangle 9 9 Pget", Ok(r"[-16777216,null]")),
        (r"@T 0 0 8 0 0 8 Ptriangle 2 2 Pget", Ok(r"[-1,null]")),
        (r"@T 0 0 8 0 0 8 Ptriangle 7 7 Pget", Ok(r"[-16777216,null]")),
        (r"@T 0 0 8 0 0 8 Ptriangle", Ok(r"[null]")),
        (r"@T 0 0 8 0 0 Ptriangle", Err(r"Invalid number of parameters, expected one of [6] but found 5")),
        (r"@T 0 0 8 0 0 'x' Ptriangle", Err(r"class java.lang.String cannot be cast to class java.lang.Number")),
    ]);
}

#[test]
fn quad() {
    check(&[
        (r"@B 1 1 4 1 4 4 1 4 Pquad Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@B 1 1 4 1 4 4 1 4 Pquad", Ok(r"[null]")),
        (r"@B 1 1 4 1 4 4 1 Pquad", Err(r"Invalid number of parameters, expected one of [8] but found 7")),
        (r"@B 1 1 4 1 4 4 1 'x' Pquad", Err(r"class java.lang.String cannot be cast to class java.lang.Number")),
        (r"@B 1 4 4 4 4 1 1 1 Pquad Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
    ]);
}

#[test]
fn rect() {
    check(&[
        (r"@F 1 1 4 4 Prect Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]")),
        (r"@F 1 1 4 4 Prect", Ok(r"[null]")),
        (r"@F 'CORNERS' PrectMode 1 1 5 5 Prect Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]")),
        (r"@F 'CORNERS' PrectMode 5 5 1 1 Prect Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]")),
        (r"@F 'CENTER' PrectMode 3 3 4 4 Prect Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]")),
        (r"@F 'RADIUS' PrectMode 3 3 2 2 Prect Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]")),
        (r"@F 1 1 -2 -2 Prect Ppixels", Ok(r"[[-1,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@F 0 0 0 0 Prect Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@F 1 1 4 4 0 Prect Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-1,-1,-16777216,-1,-1,-1],null]")),
        (r"@F 1 1 4 4 1 Prect", Ok(r"[null]")),
        (r"@F 1 1 4 Prect", Err(r"Invalid number of parameters, expected one of [4, 5, 8] but found 3")),
        (r"@F 1 1 4 4 1 1 Prect", Err(r"Invalid number of parameters, expected one of [4, 5, 8] but found 6")),
        (r"@F 1 1 4 4 1 1 1 Prect", Err(r"Invalid number of parameters, expected one of [4, 5, 8] but found 7")),
        (r"@F 'x' 1 4 4 Prect", Err(r"class java.lang.String cannot be cast to class java.lang.Number")),
        (r"@F 1 1 6 6 3 Prect 4 1 Pget", Ok(r"[0,null]")),
        (r"@F 1 1 6 6 3 Prect 4 4 Pget", Ok(r"[0,null]")),
        (r"@F 1 1 6 6 3 Prect 0 0 Pget", Ok(r"[-16777216,null]")),
        (r"@S 1 1 4 4 Prect Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-1,-1,-1,-16777216,-1,-16777216,-16777216,-16777216,-1,-16777216,-16777216],null]")),
    ]);
}

#[test]
fn ellipse() {
    check(&[
        (r"@T 1 1 8 8 Pellipse 5 5 Pget", Ok(r"[-16777216,null]")),
        (r"@T 1 1 8 8 Pellipse 0 0 Pget", Ok(r"[-1,null]")),
        (r"@T 1 1 8 8 Pellipse 9 9 Pget", Ok(r"[-16777216,null]")),
        (r"@T 'CENTER' PellipseMode 5 5 8 8 Pellipse 5 5 Pget", Ok(r"[-1,null]")),
        (r"@T 'RADIUS' PellipseMode 5 5 4 4 Pellipse 5 5 Pget", Ok(r"[-1,null]")),
        (r"@T 'RADIUS' PellipseMode 5 5 4 4 Pellipse 9 9 Pget", Ok(r"[-16777216,null]")),
        (r"@T 'CORNERS' PellipseMode 1 1 9 9 Pellipse 5 5 Pget", Ok(r"[-1,null]")),
        (r"@T 1 1 8 Pellipse", Err(r"Invalid number of parameters, expected one of [4] but found 3")),
        (r"@T 1 1 8 'x' Pellipse", Err(r"class java.lang.String cannot be cast to class java.lang.Number")),
    ]);
}

#[test]
fn arc() {
    check(&[
        (r"@T 1 1 8 8 0 1.5707963 Parc 6 6 Pget", Ok(r"[-16777216,null]")),
        (r"@T 1 1 8 8 0 1.5707963 Parc 2 2 Pget", Ok(r"[-1,null]")),
        (r"@T 1 1 8 8 0 1.5707963 'OPEN' Parc 6 6 Pget", Ok(r"[-16777216,null]")),
        (r"@T 1 1 8 8 0 1.5707963 'OPEN' Parc 2 2 Pget", Ok(r"[-16777216,null]")),
        (r"@T 1 1 8 8 0 1.5707963 'CHORD' Parc 6 6 Pget", Ok(r"[-16777216,null]")),
        (r"@T 1 1 8 8 0 1.5707963 'PIE' Parc 2 2 Pget", Ok(r"[-1,null]")),
        (r"@T 1 1 8 8 0 1.5707963 'PIE' Parc 6 6 Pget", Ok(r"[-16777216,null]")),
        (r"@T 1 1 8 8 0 1.5707963 'X' Parc", Err(r"Parc invalid mode, expected 'CHORD', 'PIE' or 'OPEN'.")),
        (r"@T 1 1 8 8 0 1.5707963 Parc", Ok(r"[null]")),
        (r"@T 1 1 8 8 0 Parc", Err(r"Invalid number of parameters, expected one of [6, 7] but found 5")),
        (r"@T 1 1 8 8 0 1.5707963 NULL Parc", Err(r"NullPointerException")),
        (r"@T 'CENTER' PellipseMode 5 5 8 8 0 1.5707963 Parc 6 6 Pget", Ok(r"[-1,null]")),
    ]);
}

#[test]
fn box_and_sphere() {
    check(&[
        (r"@G 1 Pbox Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@G 1 2 3 Pbox Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@G 1 Pbox", Ok(r"[null]")),
        (r"@G 'x' Pbox", Err(r"class java.lang.String cannot be cast to class java.lang.Number")),
        (r"@G 1 2 Pbox", Err(r"Invalid number of parameters, expected one of [1, 3] but found 2")),
        (r"@G 1 'x' 3 Pbox", Err(r"class java.lang.String cannot be cast to class java.lang.Number")),
        (r"@G 2 Psphere Ppixels", Ok(r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]")),
        (r"@G 2 Psphere", Ok(r"[null]")),
        (r"@G 'x' Psphere", Err(r"class java.lang.String cannot be cast to class java.lang.Number")),
        (r"@G Psphere", Err(r"Invalid number of parameters, expected one of [1] but found 0")),
    ]);
}
