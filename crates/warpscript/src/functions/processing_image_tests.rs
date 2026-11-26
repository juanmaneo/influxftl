// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the Processing image functions: the image functions of the Processing API.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const G: &str = r"4 4 '2D' PGraphics";
const I: &str = r"2 2 '2D' PGraphics 255 Pbackground PtoImage";
const M: &str = r"2 2 '2D' PGraphics 128 Pbackground PtoImage";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@G", G).replace("@I", I).replace("@M", M)
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
fn image() {
    check(&[
        (r"PtoImage", Err(r"Empty stack.")),
        (
            r"@G @I 1 1 Pimage Ppixels",
            Ok(r"[[0,0,0,0,0,-1,-1,0,0,-1,-1,0,0,0,0,0],null]"),
        ),
        (
            r"@G @I 1 1 2 2 Pimage Ppixels",
            Ok(r"[[0,0,0,0,0,-1,-1,0,0,-1,-1,0,0,0,0,0],null]"),
        ),
        (
            r"@G 'CENTER' PimageMode @I 1 1 Pimage Ppixels",
            Ok(r"[[-1,-1,0,0,-1,-1,0,0,0,0,0,0,0,0,0,0],null]"),
        ),
        (
            r"@G 'CENTER' PimageMode @I 2 2 2 2 Pimage Ppixels",
            Ok(r"[[0,0,0,0,0,-1,-1,0,0,-1,-1,0,0,0,0,0],null]"),
        ),
        (
            r"@G 'CENTER' PimageMode @I 2 2 Pimage Ppixels",
            Ok(r"[[0,0,0,0,0,-1,-1,0,0,-1,-1,0,0,0,0,0],null]"),
        ),
        (
            r"@G 'CORNERS' PimageMode @I 1 1 3 3 Pimage Ppixels",
            Ok(r"[[0,0,0,0,0,-1,-1,0,0,-1,-1,0,0,0,0,0],null]"),
        ),
        (
            r"@G 'CORNERS' PimageMode @I 1 1 Pimage Ppixels",
            Ok(r"[[0,0,0,0,0,-1,-1,0,0,-1,-1,0,0,0,0,0],null]"),
        ),
        (
            r"@G @I 1 1 0 0 Pimage Ppixels",
            Ok(r"[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],null]"),
        ),
        (
            r"@G @I 1 1 -1 1 Pimage Ppixels",
            Ok(r"[[0,0,0,0,-1,0,0,0,0,0,0,0,0,0,0,0],null]"),
        ),
        (
            r"@G @I 1.5 1.5 Pimage Ppixels",
            Ok(r"[[0,0,0,0,0,-1,-1,0,0,-1,-1,0,0,0,0,0],null]"),
        ),
        (
            r"@G 'RADIUS' PimageMode @I 2 2 Pimage Ppixels",
            Err(r"PimageMode: invalid mode, should be 'CENTER', 'CORNER' or 'CORNERS'."),
        ),
        (
            r"@G 5 1 1 Pimage Ppixels",
            Err(r"Pimage expects an image as parameter deepest."),
        ),
        (
            r"@G 'x' Pimage",
            Err(r"Invalid number of parameters, expected one of [3, 5] but found 1"),
        ),
        (
            r"@G Pimage",
            Err(r"Invalid number of parameters, expected one of [3, 5] but found 0"),
        ),
        (
            r"@G @I Pimage",
            Err(r"Invalid number of parameters, expected one of [3, 5] but found 1"),
        ),
        (
            r"@G @I 1 Pimage",
            Err(r"Invalid number of parameters, expected one of [3, 5] but found 2"),
        ),
        (
            r"@G @I 1 1 1 Pimage",
            Err(r"Invalid number of parameters, expected one of [3, 5] but found 4"),
        ),
        (
            r"@G @I 'x' 1 Pimage",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G @I NULL 1 Pimage", Err(r"NullPointerException")),
        (
            r"@G @I 1 1 'x' 2 Pimage",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G @I 1 1 NULL 2 Pimage", Err(r"NullPointerException")),
        (
            r"@G @G 1 1 Pimage",
            Err(r"Invalid number of parameters, expected one of [3, 5] but found 2"),
        ),
        (
            r"@G Pimage Ppixels",
            Err(r"Invalid number of parameters, expected one of [3, 5] but found 0"),
        ),
    ]);
}

#[test]
fn size() {
    check(&[
        (r"@G Psize", Ok(r"[4,4]")),
        (r"@I Psize", Ok(r"[2,2]")),
        (
            r"@G 5 Psize",
            Err(r"Psize expects a PIMAGE or a PGRAPHICS instance."),
        ),
        (
            r"NULL Psize",
            Err(r"Psize expects a PIMAGE or a PGRAPHICS instance."),
        ),
        (r"@G 2 2 Presize Psize", Ok(r"[2,2]")),
        (r"@G 0 2 Presize Psize", Ok(r"[2,2]")),
        (r"@G 2 0 Presize Psize", Ok(r"[2,2]")),
        (r"@G 3 2 Presize Psize", Ok(r"[2,3]")),
        (
            r"@G 0 0 Presize",
            Err(r"width or height must be > 0 for resize"),
        ),
        (
            r"@G -1 2 Presize",
            Err(r"Presize expects the height to be positive and less than 2147483647."),
        ),
        (
            r"@G 2 -1 Presize",
            Err(r"Presize expects the height to be positive and less than 2147483647."),
        ),
        (
            r"@G 'x' 2 Presize",
            Err(r"Presize expects a width in pixel."),
        ),
        (
            r"@G 2 'x' Presize",
            Err(r"Presize expects a height in pixel."),
        ),
        (
            r"@G 2 NULL Presize",
            Err(r"Presize expects a height in pixel."),
        ),
        (
            r"@G 2147483648 2 Presize",
            Err(r"Presize expects the height to be positive and less than 2147483647."),
        ),
        (r"@G 2 PresizeXX", Err(r"Unknown function 'PresizeXX'")),
        (r"@I 4 4 Presize Psize", Ok(r"[4,4]")),
        (r"@I 4 0 Presize Psize", Ok(r"[4,4]")),
        (r"@I 0 2 Presize Psize", Ok(r"[2,2]")),
        (r"@I 1 1 Presize Ppixels", Ok(r"[[-1]]")),
        (r"@G 1 1 Presize Ppixels", Ok(r"[[0],null]")),
        (r"@G 1 Presize", Err(r"Presize expects a width in pixel.")),
        (
            r"5 2 2 Presize",
            Err(r"Presize expects a PIMAGE or a PGRAPHICS instance."),
        ),
        (r"@G 2 2 Presize Ppixels", Ok(r"[[0,0,0,0],null]")),
        (r"Psize", Err(r"Empty stack.")),
        (r"Presize", Err(r"Empty stack.")),
        (r"5 Presize", Err(r"Empty stack.")),
        (r"4 5 Presize", Err(r"Empty stack.")),
        (
            r"@G 0 0 0 0 Pget 0 1 Presize",
            Err(r"Width (0) and height (0) cannot be <= 0"),
        ),
        (
            r"@G 0 0 0 0 Pget 2 0 Presize",
            Err(r"Width (0) and height (0) cannot be <= 0"),
        ),
        (
            r"@G 0 0 0 0 Pget 0 0 Presize",
            Err(r"width or height must be > 0 for resize"),
        ),
        (
            r"1 3 '2D' PGraphics 0 1 Presize",
            Err(r"Width (0) and height (1) cannot be <= 0"),
        ),
        (
            r"1 3 '2D' PGraphics PtoImage 0 1 Presize",
            Err(r"Width (0) and height (1) cannot be <= 0"),
        ),
        (
            r"3 1 '2D' PGraphics PtoImage 1 0 Presize",
            Err(r"Width (1) and height (0) cannot be <= 0"),
        ),
        (
            r"3 1 '2D' PGraphics 1 0 Presize",
            Err(r"Width (1) and height (0) cannot be <= 0"),
        ),
    ]);
}

#[test]
fn mask() {
    check(&[
        (r"Pmask", Err(r"Empty stack.")),
        (
            r"@I @M Pmask Ppixels",
            Ok(r"[[-2130706433,-2130706433,-2130706433,-2130706433]]"),
        ),
        (r"@I @I Pmask Ppixels", Ok(r"[[-1,-1,-1,-1]]")),
        (
            r"@M @I Pmask Ppixels",
            Ok(r"[[-8355712,-8355712,-8355712,-8355712]]"),
        ),
        (
            r"@I @G Pmask",
            Err(r"Pmask expects the two PIMAGEs to be of the same size."),
        ),
        (
            r"@I @G PtoImage Pmask",
            Err(r"Pmask expects the two PIMAGEs to be of the same size."),
        ),
        (
            r"@G @I Pmask",
            Err(r"Pmask expects the two PIMAGEs to be of the same size."),
        ),
        (
            r"@I 2 2 '2D' PGraphics 128 Pbackground Pmask Ppixels",
            Ok(r"[[-2130706433,-2130706433,-2130706433,-2130706433]]"),
        ),
        (
            r"@I 1 4 '2D' PGraphics 128 Pbackground Pmask Ppixels",
            Ok(r"[[-2130706433,-2130706433,-2130706433,-2130706433]]"),
        ),
        (
            r"@I 3 2 '2D' PGraphics 128 Pbackground Pmask",
            Err(r"Pmask expects the two PIMAGEs to be of the same size."),
        ),
        (
            r"@G 255 Pbackground 4 4 '2D' PGraphics 128 Pbackground Pmask Ppixels",
            Ok(
                r"[[-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433,-2130706433],null]",
            ),
        ),
        (
            r"@G @G Pmask Ppixels",
            Ok(r"[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],null]"),
        ),
        (r"@I Pmask", Err(r"Empty stack.")),
        (r"'x' @I Pmask", Err(r"Pmask expects two PIMAGEs.")),
        (r"@I 'x' Pmask", Err(r"Pmask expects two PIMAGEs.")),
        (r"@I NULL Pmask", Err(r"Pmask expects two PIMAGEs.")),
        (r"@I @M Pmask Psize", Ok(r"[2,2]")),
    ]);
}

#[test]
fn pixels() {
    check(&[
        (r"Ppixels", Err(r"Empty stack.")),
        (
            r"@G Ppixels",
            Ok(r"[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],null]"),
        ),
        (r"@I Ppixels", Ok(r"[[-1,-1,-1,-1]]")),
        (
            r"@G 255 Pbackground Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"5 Ppixels",
            Err(r"Ppixels can only be applied to PGraphics or PImage instances."),
        ),
        (
            r"NULL Ppixels",
            Err(r"Ppixels can only be applied to PGraphics or PImage instances."),
        ),
        (r"@I [ 0 1 2 3 ] PupdatePixels Ppixels", Ok(r"[[0,1,2,3]]")),
        (
            r"@I [ 0 1 2 ] PupdatePixels",
            Err(r"PupdatePixels expected array of 4 pixels, found 3"),
        ),
        (
            r"@I [ 0 1 'x' 3 ] PupdatePixels",
            Err(r"PupdatePixels expected an array of LONG."),
        ),
        (
            r"@I 'x' PupdatePixels",
            Err(r"Invalid number of parameters, expected one of [1] but found 2"),
        ),
        (
            r"@I PupdatePixels",
            Err(r"class processing.core.PImage cannot be cast to class processing.core.PGraphics"),
        ),
        (
            r"@G [ 1 2 3 ] PupdatePixels",
            Err(r"PupdatePixels expected an array of 16 pixels, found 3"),
        ),
        (
            r"@G 'x' PupdatePixels",
            Err(r"PupdatePixels expects an array of pixels on top of the stack."),
        ),
        (
            r"@G PupdatePixels",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G [ 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 ] PupdatePixels Ppixels",
            Ok(r"[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],null]"),
        ),
        (
            r"@G 255 Pbackground [ 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 ] PupdatePixels Ppixels",
            Ok(r"[[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],null]"),
        ),
        (
            r"@G PtoImage Ppixels",
            Ok(r"[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]]"),
        ),
        (
            r"@I PtoImage",
            Err(r"PtoImage operates on a PGraphics instance."),
        ),
        (
            r"5 PtoImage",
            Err(r"PtoImage operates on a PGraphics instance."),
        ),
        (r"@G PtoImage Psize", Ok(r"[4,4]")),
    ]);
}

#[test]
fn blend() {
    check(&[
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'BLEND' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'ADD' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-14671840,-14671840,-15724528,-15724528,-14671840,-14671840],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'SUBTRACT' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-16777216,-16777216,-15724528,-15724528,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'DARKEST' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'LIGHTEST' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'DIFFERENCE' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-16777216,-16777216,-15724528,-15724528,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'EXCLUSION' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-14803426,-14803426,-15724528,-15724528,-14803426,-14803426],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'MULTIPLY' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-16711423,-16711423,-15724528,-15724528,-16711423,-16711423],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'SCREEN' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-14737633,-14737633,-15724528,-15724528,-14737633,-14737633],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'OVERLAY' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-16645630,-16645630,-15724528,-15724528,-16645630,-16645630],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'HARD_LIGHT' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-16645630,-16645630,-15724528,-15724528,-16645630,-16645630],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'SOFT_LIGHT' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-16645630,-16645630,-15724528,-15724528,-16645630,-16645630],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'DODGE' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15658735,-15658735,-15724528,-15724528,-15658735,-15658735],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'BURN' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-16777216,-16777216,-15724528,-15724528,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'REPLACE' Pblend",
            Err(
                r"Pblend: invalid mode, should be 'BLEND', 'ADD', 'SUBTRACT', 'DARKEST', 'LIGHTEST', 'DIFFERENCE', 'EXCLUSION', 'MULTIPLY', 'SCREEN', 'OVERLAY', 'HARD_LIGHT', 'SOFT_LIGHT', 'DODGE' or 'BURN'.",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 'bogus' Pblend",
            Err(
                r"Pblend: invalid mode, should be 'BLEND', 'ADD', 'SUBTRACT', 'DARKEST', 'LIGHTEST', 'DIFFERENCE', 'EXCLUSION', 'MULTIPLY', 'SCREEN', 'OVERLAY', 'HARD_LIGHT', 'SOFT_LIGHT', 'DODGE' or 'BURN'.",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 NULL Pblend",
            Err(
                r"Pblend: invalid mode, should be 'BLEND', 'ADD', 'SUBTRACT', 'DARKEST', 'LIGHTEST', 'DIFFERENCE', 'EXCLUSION', 'MULTIPLY', 'SCREEN', 'OVERLAY', 'HARD_LIGHT', 'SOFT_LIGHT', 'DODGE' or 'BURN'.",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 3 Pblend",
            Err(r"class java.lang.Long cannot be cast to class java.lang.String"),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 'ADD' Pblend",
            Err(r"Invalid number of parameters, expected one of [9, 10] but found 7"),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 2 2 'ADD' Pblend",
            Err(r"Invalid number of parameters, expected one of [9, 10] but found 11"),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 1 1 2 2 'ADD' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-14671840,-14671840,-15724528,-15724528,-14671840,-14671840,-15724528,-15724528,-15724528,-15724528,-15724528],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 1 1 4 4 'MULTIPLY' Pblend Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-16711423,-16711423,-16711423,-15724528,-16711423,-16711423,-16711423,-15724528,-16711423,-16711423,-16711423],null]",
            ),
        ),
        (
            r"@G @I 0 0 2 2 1 1 2 2 'ADD' Pblend Ppixels",
            Ok(r"[[0,0,0,0,0,-1,-1,0,0,-1,-1,0,0,0,0,0],null]"),
        ),
        (
            r"@G @I 0 0 2 2 0 0 4 4 'BLEND' Pblend Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 5 0 0 2 2 1 1 2 2 'ADD' Pblend",
            Err(r"class java.lang.Long cannot be cast to class processing.core.PImage"),
        ),
        (
            r"@G NULL 0 0 2 2 1 1 2 2 'ADD' Pblend",
            Err(r"NullPointerException"),
        ),
        (
            r"@G 16 Pbackground Pblend",
            Err(r"Invalid number of parameters, expected one of [9, 10] but found 0"),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 'x' Pblend",
            Err(r"Invalid number of parameters, expected one of [9, 10] but found 6"),
        ),
        (
            r"@G 'x' 0 1 1 0 0 1 1 'BLEND' Pblend",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 2 2 '2D' PGraphics 255 Pbackground PtoImage 'x' 0 0 1 1 0 0 1 'BLEND' Pblend",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
    ]);
}

#[test]
fn copy() {
    check(&[
        (
            r"@G Pcopy Ppixels",
            Ok(r"[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],null]"),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 2 2 Pcopy Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528],null]",
            ),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 1 1 4 4 Pcopy Ppixels",
            Ok(
                r"[[-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528,-15724528],null]",
            ),
        ),
        (
            r"@G @I 0 0 2 2 0 0 4 4 Pcopy Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G @I 0 0 2 2 0 0 2 2 Pcopy Ppixels",
            Ok(r"[[-1,-1,0,0,-1,-1,0,0,0,0,0,0,0,0,0,0],null]"),
        ),
        (
            r"@G 16 Pbackground 0 0 2 2 2 2 Pcopy",
            Err(r"Invalid number of parameters, expected one of [0, 8, 9] but found 6"),
        ),
        (
            r"@G @I Pcopy",
            Err(r"Invalid number of parameters, expected one of [0, 8, 9] but found 1"),
        ),
        (
            r"@G 5 0 0 2 2 0 0 2 2 Pcopy",
            Err(r"class java.lang.Long cannot be cast to class processing.core.PImage"),
        ),
        (
            r"@G NULL 0 0 2 2 0 0 2 2 Pcopy",
            Err(r"NullPointerException"),
        ),
        (
            r"@G 1 2 3 4 5 6 7 8 9 Pcopy",
            Err(r"class java.lang.Long cannot be cast to class processing.core.PImage"),
        ),
        (
            r"@G 'x' 0 1 1 0 0 1 1 Pcopy",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 2 2 '2D' PGraphics 255 Pbackground PtoImage 'x' 0 0 1 1 0 0 1 Pcopy",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
    ]);
}

#[test]
fn get() {
    check(&[
        (r"@G 1 1 Pget", Ok(r"[0,null]")),
        (r"@G 255 Pbackground 1 1 Pget", Ok(r"[-1,null]")),
        (r"@G 255 Pbackground 9 9 Pget", Ok(r"[0,null]")),
        (r"@G 255 Pbackground -1 -1 Pget", Ok(r"[0,null]")),
        (
            r"@G 0 0 0x80102030 Pset 0 0 Pget",
            Ok(r"[-2146426832,null]"),
        ),
        (
            r"@G 255 Pbackground 0 0 4 4 Pget Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 255 Pbackground 1 1 2 2 Pget Ppixels",
            Ok(r"[[-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 255 Pbackground 3 3 2 2 Pget Ppixels",
            Ok(r"[[-1,0,0,0],null]"),
        ),
        (r"@G 0 0 -1 1 Pget", Err(r"-1")),
        (r"@G 0 0 1 -1 Pget", Err(r"-1")),
        (r"@G 0 0 0 0 Pget Psize", Ok(r"[0,0,null]")),
        (r"@G 255 Pbackground Pget Psize", Ok(r"[4,4,null]")),
        (
            r"@G 1 Pget",
            Err(r"Invalid number of parameters, expected one of [0, 2, 4] but found 1"),
        ),
        (
            r"@G 'x' 1 Pget",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G 1 NULL Pget", Err(r"NullPointerException")),
        (
            r"@G 'x' 0 1 1 Pget",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 1 2 3 Pget",
            Err(r"Invalid number of parameters, expected one of [0, 2, 4] but found 3"),
        ),
    ]);
}

#[test]
fn set() {
    check(&[
        (r"@G 1 1 255 Pset 1 1 Pget", Ok(r"[255,null]")),
        (r"@G 1 1 -1 Pset 1 1 Pget", Ok(r"[-1,null]")),
        (
            r"@G 9 9 255 Pset Ppixels",
            Ok(r"[[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],null]"),
        ),
        (
            r"@G @I 1 1 Pset Ppixels",
            Err(r"class processing.core.PImage cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G @I 3 3 Pset Ppixels",
            Err(r"class processing.core.PImage cannot be cast to class java.lang.Number"),
        ),
        (r"@G 1 1 1.5 Pset 1 1 Pget", Ok(r"[1,null]")),
        (
            r"@G 1 1 @I Pset Ppixels",
            Ok(r"[[0,0,0,0,0,-1,-1,0,0,-1,-1,0,0,0,0,0],null]"),
        ),
        (
            r"@G 1 1 'x' Pset",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G 1 1 NULL Pset", Err(r"NullPointerException")),
        (
            r"@G 'x' 1 255 Pset",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 1 Pset",
            Err(r"Invalid number of parameters, expected one of [3] but found 1"),
        ),
        (
            r"@G 1 1 2 3 Pset",
            Err(r"Invalid number of parameters, expected one of [3] but found 4"),
        ),
    ]);
}

#[test]
fn filter() {
    check(&[
        (
            r"@G 255 Pbackground 'GRAY' Pfilter Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 128 Pbackground 'THRESHOLD' Pfilter Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 128 Pbackground 'THRESHOLD' 0.3 Pfilter Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 128 Pbackground 'THRESHOLD' 1.5 Pfilter Ppixels",
            Ok(
                r"[[-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216,-16777216],null]",
            ),
        ),
        (
            r"@G 128 Pbackground 'THRESHOLD' -1 Pfilter Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 128 Pbackground 'THRESHOLD' 'x' Pfilter",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 128 Pbackground 'THRESHOLD' NULL Pfilter",
            Err(r"NullPointerException"),
        ),
        (
            r"@G 128 Pbackground 'OPAQUE' Pfilter Ppixels",
            Ok(
                r"[[-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712],null]",
            ),
        ),
        (
            r"@G 128 Pbackground 'INVERT' Pfilter Ppixels",
            Ok(
                r"[[-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505,-8421505],null]",
            ),
        ),
        (
            r"@G 200 Pbackground 'POSTERIZE' 4 Pfilter Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 200 Pbackground 'POSTERIZE' 2 Pfilter Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 200 Pbackground 'POSTERIZE' 255 Pfilter Ppixels",
            Ok(
                r"[[-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409,-3684409],null]",
            ),
        ),
        (
            r"@G 200 Pbackground 'POSTERIZE' 2.5 Pfilter Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 200 Pbackground 'POSTERIZE' Pfilter",
            Err(r"Use filter(POSTERIZE, int levels) instead of filter(POSTERIZE)"),
        ),
        (
            r"@G 200 Pbackground 'POSTERIZE' 1 Pfilter",
            Err(r"Levels must be between 2 and 255 for filter(POSTERIZE, levels)"),
        ),
        (
            r"@G 200 Pbackground 'POSTERIZE' 256 Pfilter",
            Err(r"Levels must be between 2 and 255 for filter(POSTERIZE, levels)"),
        ),
        (
            r"@G 128 Pbackground 'GRAY' 1 Pfilter",
            Err(r"Use filter(GRAY) instead of filter(GRAY, param)"),
        ),
        (
            r"@G 128 Pbackground 'INVERT' 1 Pfilter",
            Err(r"Use filter(INVERT) instead of filter(INVERT, param)"),
        ),
        (
            r"@G 128 Pbackground 'ERODE' Pfilter Ppixels",
            Ok(
                r"[[-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712],null]",
            ),
        ),
        (
            r"@G 128 Pbackground 'DILATE' Pfilter Ppixels",
            Ok(
                r"[[-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712,-8355712],null]",
            ),
        ),
        (
            r"@G 255 Pbackground 'BLUR' Pfilter Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 255 Pbackground 'BLUR' 1 Pfilter Ppixels",
            Ok(r"[[-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1,-1],null]"),
        ),
        (
            r"@G 128 Pbackground 'bogus' Pfilter",
            Err(r"Pfilter invalid mode."),
        ),
        (
            r"@G 128 Pbackground 5 Pfilter",
            Err(r"Pfilter invalid mode."),
        ),
        (
            r"@G 128 Pbackground NULL Pfilter",
            Err(r"NullPointerException"),
        ),
        (
            r"@G Pfilter",
            Err(r"Invalid number of parameters, expected one of [1, 2] but found 0"),
        ),
    ]);
}

#[test]
fn presize_over_max_pixels() {
    let mut stack = crate::stack::Stack::new();
    let error = stack
        .exec_checked("4 4 '2D' PGraphics 3000 3000 Presize")
        .expect_err("a 3000 by 3000 graphics is over the default MAXPIXELS limit");
    assert!(
        error.to_string().contains("Presize only allows PGRAPHICS or PIMAGE with a total number of pixels less than 1000000 requested size was 3000x3000 (9000000)."),
        "unexpected message: {error}"
    );
}
