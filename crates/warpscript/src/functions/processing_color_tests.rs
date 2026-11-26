// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the Processing functions: the colour functions of the Processing API.
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
fn argument_refusals() {
    const CAST: &str = "class java.lang.String cannot be cast to class java.lang.Number";
    check(&[
        (r"@G NaN Pfill", Err(r"not a finite number: NaN")),
        (r"@G NaN 1 Pfill", Err(r"not a finite number: NaN")),
        (r"@G 1 'x' Pfill", Err(CAST)),
        (r"@G 1.5 'x' Pfill", Err(CAST)),
        (r"@G 'x' 1 Pfill", Ok(r"[null]")),
        (r"@G 'x' 1 2 Pfill", Err(CAST)),
        (r"@G 1 2 'x' Pfill", Err(CAST)),
        (r"@G 'x' 1 2 3 Pfill", Err(CAST)),
        (r"@G 1 'x' 2 3 Pfill", Err(CAST)),
        (r"@G 1 2 'x' 3 Pfill", Err(CAST)),
        (r"@G 1 2 3 'x' Pfill", Err(CAST)),
        (r"@G 'x' 1 2 Pstroke", Err(CAST)),
        (r"@G 'x' 1 2 3 Pstroke", Err(CAST)),
        (
            r"@G Pstroke",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 0"),
        ),
        (
            r"@G 1 Pclear",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
        (
            r"@G 1 PnoFill",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
        (
            r"@G 1 PnoStroke",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
        (
            r"@G 1 PnoTint",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
        (r"@G 1 'x' Ptint", Err(CAST)),
        (r"@G 'x' 1 Ptint", Err(CAST)),
        (r"@G 1.5 'x' Ptint", Err(CAST)),
        (r"@G 'x' 2 3 Ptint", Err(CAST)),
        (r"@G 1 2 'x' Ptint", Err(CAST)),
        (r"@G 'x' 2 3 4 Ptint", Err(CAST)),
        (r"@G 1 'x' 3 4 Ptint", Err(CAST)),
        (r"@G 1 2 'x' 4 Ptint", Err(CAST)),
        (r"@G 1 2 3 'x' Ptint", Err(CAST)),
        (r"@G 1 'x' Pcolor", Err(CAST)),
        (r"@G 'x' 2 3 4 Pcolor", Err(CAST)),
        (r"@G 1 'x' 3 4 Pcolor", Err(CAST)),
        (r"@G 1 2 'x' 4 Pcolor", Err(CAST)),
        (r"@G 'x' 1 0.5 PlerpColor", Err(CAST)),
        (r"@G 1 'x' 0.5 PlerpColor", Err(CAST)),
        (r"@G 'RGB' 'x' 1 2 PcolorMode", Err(CAST)),
        (
            r"@G 'RGB' 0 1 1 PcolorMode",
            Err(r"a colour maximum must be a finite positive number."),
        ),
        (r"@G 'RGB' 'x' 1 1 1 PcolorMode", Err(CAST)),
        (
            r"@G 'RGB' 1 1 1 0 PcolorMode",
            Err(r"a colour maximum must be a finite positive number."),
        ),
        (
            r"@G 2 2 '2D' PGraphics 255 Pbackground PtoImage Pbackground",
            Ok(r"[null]"),
        ),
    ]);
}

#[test]
fn colour() {
    check(&[
        (r"@G 128 Pcolor", Ok(r"[-8355712,null]")),
        (r"@G 255 Pcolor", Ok(r"[-1,null]")),
        (r"@G 300 Pcolor", Ok(r"[300,null]")),
        (r"@G 0 Pcolor", Ok(r"[-16777216,null]")),
        (r"@G -1 Pcolor", Ok(r"[-1,null]")),
        (r"@G 0x80102030 Pcolor", Ok(r"[-2146426832,null]")),
        (r"@G 0xff102030 Pcolor", Ok(r"[-15720400,null]")),
        (r"@G 1.5 Pcolor", Ok(r"[-16711423,null]")),
        (r"@G 128 0.5 Pcolor", Ok(r"[8421504,null]")),
        (r"@G 0xff102030 127 Pcolor", Ok(r"[2130706432,null]")),
        (r"@G 0x80102030 127 Pcolor", Ok(r"[2130706432,null]")),
        (r"@G 0x00102030 127 Pcolor", Ok(r"[2147483647,null]")),
        (r"@G 100 127 Pcolor", Ok(r"[2137285732,null]")),
        (r"@G 300 127 Pcolor", Ok(r"[2147483647,null]")),
        (r"@G -1 127 Pcolor", Ok(r"[2130706432,null]")),
        (r"@G 0xff102030 255 Pcolor", Ok(r"[-16777216,null]")),
        (r"@G 0xff102030 0 Pcolor", Ok(r"[0,null]")),
        (r"@G 128 255 Pcolor", Ok(r"[-8355712,null]")),
        (r"@G 128 0 Pcolor", Ok(r"[8421504,null]")),
        (
            r"@G 128 'x' Pcolor",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G 0 0 0 Pcolor", Ok(r"[-16777216,null]")),
        (r"@G 255 0 0 Pcolor", Ok(r"[-65536,null]")),
        (r"@G 0 255 0 0.5 Pcolor", Ok(r"[65280,null]")),
        (
            r"@G 'x' 0 0 Pcolor",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 0 'x' 0 Pcolor",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 0 0 'x' Pcolor",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 0 0 0 'x' Pcolor",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 1 2 3 4 5 Pcolor",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 5"),
        ),
        (
            r"@G Pcolor",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 0"),
        ),
        (r"@G NULL Pcolor", Err(r"NullPointerException")),
        (r"@G 16 32 48 Pcolor Pred", Ok(r"[16.0,null]")),
        (r"@G 16 32 48 Pcolor Pgreen", Ok(r"[32.0,null]")),
        (r"@G 16 32 48 Pcolor Pblue", Ok(r"[48.0,null]")),
        (r"@G 16 32 48 0.5 Pcolor Palpha", Ok(r"[0.0,null]")),
        (r"@G 16 32 48 Pcolor Phue", Ok(r"[148.75,null]")),
        (r"@G 16 32 48 Pcolor Psaturation", Ok(r"[170.0,null]")),
        (r"@G 16 32 48 Pcolor Pbrightness", Ok(r"[48.0,null]")),
        (r"@G 0x80102030 Pcolor Palpha", Ok(r"[128.0,null]")),
        (
            r"@G 'x' Pred",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G NULL Pred", Err(r"NullPointerException")),
        (
            r"@G Pred",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G 1 2 Pred",
            Err(r"Invalid number of parameters, expected one of [1] but found 2"),
        ),
        (
            r"@G 'HSB' PcolorMode 0 0 100 Pcolor",
            Ok(r"[-10197916,null]"),
        ),
        (
            r"@G 'HSB' 100 PcolorMode 50 50 50 Pcolor",
            Ok(r"[-12615809,null]"),
        ),
        (
            r"@G 'HSB' 100 PcolorMode 120 100 100 Pcolor",
            Ok(r"[-65536,null]"),
        ),
        (
            r"@G 'HSB' 255 PcolorMode 120 255 255 Pcolor",
            Ok(r"[-16711726,null]"),
        ),
        (
            r"@G 'HSB' 100 PcolorMode 50 50 50 Phue",
            Err(r"Invalid number of parameters, expected one of [1] but found 2"),
        ),
        (
            r"@G 'HSB' 100 PcolorMode 0 0 100 Pcolor Pbrightness",
            Ok(r"[100.0,null]"),
        ),
        (
            r"@G 'HSB' 100 PcolorMode 0 0 100 Pcolor Phue",
            Ok(r"[0.0,null]"),
        ),
        (
            r"@G 'RGB' 100 PcolorMode 50 50 50 Pcolor",
            Ok(r"[-8421505,null]"),
        ),
        (
            r"@G 'RGB' 100 PcolorMode 50 50 50 Pcolor Pred",
            Ok(r"[49.80392074584961,null]"),
        ),
        (
            r"@G 'RGB' 100 PcolorMode 50 50 50 Pcolor Palpha",
            Ok(r"[100.0,null]"),
        ),
        (
            r"@G 'RGB' 100 PcolorMode 50 50 50 Pcolor Pbrightness",
            Ok(r"[49.80392074584961,null]"),
        ),
        (
            r"@G 'RGB' 100 200 300 PcolorMode 50 50 50 Pcolor Pgreen",
            Ok(r"[49.411766052246094,null]"),
        ),
        (
            r"@G 'RGB' 100 200 300 400 PcolorMode 50 50 50 50 Pcolor Palpha",
            Ok(r"[48.6274528503418,null]"),
        ),
        (
            r"@G 'RGB' 255 PcolorMode 128 Pcolor",
            Ok(r"[-8355712,null]"),
        ),
        (r"@G 'RGB' PcolorMode 128 Pcolor", Ok(r"[-8355712,null]")),
        (r"@G 'RGB' PcolorMode", Ok(r"[null]")),
        (r"@G 'HSB' PcolorMode", Ok(r"[null]")),
        (
            r"@G 'XYZ' PcolorMode",
            Err(r"PcolorMode: invalid mode, should be one of 'RGB' or 'HSB'"),
        ),
        (
            r"@G 0 PcolorMode",
            Err(r"PcolorMode: invalid mode, should be one of 'RGB' or 'HSB'"),
        ),
        (r"@G NULL PcolorMode", Err(r"NullPointerException")),
        (
            r"@G 'RGB' 'x' PcolorMode",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'RGB' 1 2 PcolorMode",
            Err(r"Invalid number of parameters, expected one of [1, 2, 4, 5] but found 3"),
        ),
        (
            r"@G 'RGB' 1 2 3 4 5 PcolorMode",
            Err(r"Invalid number of parameters, expected one of [1, 2, 4, 5] but found 6"),
        ),
        (
            r"@G 'RGB' 100 PcolorMode 'x' Pcolor",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 16 32 48 Pcolor 16 32 48 Pcolor PlerpColor",
            Err(r"Invalid number of parameters, expected one of [3] but found 1"),
        ),
        (r"@G 0 255 0.5 PlerpColor", Ok(r"[128,null]")),
        (
            r"@G 0xff0000ff 0xffff0000 0.5 PlerpColor",
            Ok(r"[-8388480,null]"),
        ),
        (r"@G 0 255 0 PlerpColor", Ok(r"[0,null]")),
        (r"@G 0 255 1 PlerpColor", Ok(r"[255,null]")),
        (
            r"@G 0 255 'x' PlerpColor",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 0 255 PlerpColor",
            Err(r"Invalid number of parameters, expected one of [3] but found 2"),
        ),
        (
            r"@G 0 'x' 0.5 PlerpColor",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'HSB' PcolorMode 0xff0000ff 0xffff0000 0.5 PlerpColor",
            Ok(r"[-16711936,null]"),
        ),
        (
            r"@G 'HSB' 100 PcolorMode 0 100 0.5 PlerpColor",
            Ok(r"[1651225,null]"),
        ),
        (r"@G 128 Pfill", Ok(r"[null]")),
        (r"@G 'x' Pfill", Ok(r"[null]")),
        (r"@G NULL Pfill", Ok(r"[null]")),
        (r"@G 128 0.5 Pfill", Ok(r"[null]")),
        (r"@G 0.5 Pfill", Ok(r"[null]")),
        (r"@G 0.5 0.5 Pfill", Ok(r"[null]")),
        (r"@G 'x' 0.5 Pfill", Ok(r"[null]")),
        (r"@G 10 20 30 Pfill", Ok(r"[null]")),
        (r"@G 10 20 30 40 Pfill", Ok(r"[null]")),
        (
            r"@G 10 'x' 30 Pfill",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 1 2 3 4 5 Pfill",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 5"),
        ),
        (
            r"@G Pfill",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 0"),
        ),
        (r"@G PnoFill", Ok(r"[null]")),
        (r"@G 128 Pstroke", Ok(r"[null]")),
        (r"@G 'x' Pstroke", Ok(r"[null]")),
        (r"@G 1 2 Pstroke", Ok(r"[null]")),
        (r"@G 10 20 30 Pstroke", Ok(r"[null]")),
        (r"@G PnoStroke", Ok(r"[null]")),
        (r"@G 128 Pbackground", Ok(r"[null]")),
        (r"@G 0.5 Pbackground", Ok(r"[null]")),
        (r"@G 'x' Pbackground", Ok(r"[null]")),
        (r"@G 1 2 Pbackground", Ok(r"[null]")),
        (r"@G 10 20 30 Pbackground", Ok(r"[null]")),
        (r"@G 10 20 30 40 Pbackground", Ok(r"[null]")),
        (
            r"@G 10 'x' 30 Pbackground",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G Pbackground",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 0"),
        ),
        (r"@G Pclear", Ok(r"[null]")),
        (r"@G 128 Ptint", Ok(r"[null]")),
        (r"@G 0.5 Ptint", Ok(r"[null]")),
        (
            r"@G 'x' Ptint",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G NULL Ptint", Err(r"NullPointerException")),
        (r"@G 128 0.5 Ptint", Ok(r"[null]")),
        (r"@G 0.5 0.5 Ptint", Ok(r"[null]")),
        (r"@G 10 20 30 Ptint", Ok(r"[null]")),
        (r"@G 10 20 30 40 Ptint", Ok(r"[null]")),
        (
            r"@G 10 'x' 30 Ptint",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 1 2 3 4 5 Ptint",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 5"),
        ),
        (
            r"@G Ptint",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3, 4] but found 0"),
        ),
        (r"@G PnoTint", Ok(r"[null]")),
    ]);
}
