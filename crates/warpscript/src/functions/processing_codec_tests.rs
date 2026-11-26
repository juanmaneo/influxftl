// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the Processing codecs: the codecs of the Processing API.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const G: &str = r"4 4 '2D' PGraphics";
const C: &str = r"2 2 '2D' PGraphics 128 Pbackground 1 1 -16777216 Pset";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@G", G).replace("@C", C)
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
fn codec_refusals() {
    check(&[
        (r"Pencode", Err(r"Empty stack.")),
        (r"{ } Pencode", Err(r"Empty stack.")),
        (
            r"@C { 'format' 'jpeg' 'quality' NaN } Pencode",
            Err(r"not a finite number: NaN"),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' 1e300 } Pencode",
            Err(r"not a finite number: +Infinity"),
        ),
        (
            r"@C { 'iTXt' [ { 'keyword' 'k' 'text' 'v' 'translatedKeyword' NULL } ] } Pencode",
            Err(r"NullPointerException"),
        ),
        (r"Pdecode", Err(r"Empty stack.")),
        (
            r"'data:image/png;base64,!!!' Pdecode",
            Err(r"Pdecode cannot read the image"),
        ),
        (
            r"'data:image/png;base64,AAAA' Pdecode",
            Err(r"Pdecode cannot read the image"),
        ),
    ]);
}

#[test]
fn codec() {
    check(&[
        (
            r"@C Pencode Pdecode Ppixels",
            Ok(r"[[-8355712,-8355712,-8355712,-16777216]]"),
        ),
        (r"@C Pencode Pdecode Psize", Ok(r"[2,2]")),
        (r"@G Pencode Pdecode Psize", Ok(r"[4,4]")),
        (
            r"@C { 'format' 'jpeg' } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' 0.5 } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' 50 } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' 1.0 } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' 'x' } Pencode",
            Err(
                r"Pencode failed. (Parameter 'quality' for format 'jpeg' must be a DOUBLE between 0.0 and 1.0, or a LONG between 0 and 100.)",
            ),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' 1.5 } Pencode",
            Err(r"Quality out-of-bounds!"),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' 150 } Pencode",
            Err(r"Quality out-of-bounds!"),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' -1 } Pencode",
            Err(r"Quality out-of-bounds!"),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' NULL } Pencode",
            Err(
                r"Pencode failed. (Parameter 'quality' for format 'jpeg' must be a DOUBLE between 0.0 and 1.0, or a LONG between 0 and 100.)",
            ),
        ),
        (
            r"@C { 'format' 'jpeg' 'quality' 'x' 'tEXt' 'x' } Pencode",
            Err(
                r"Pencode failed. (Parameter 'quality' for format 'jpeg' must be a DOUBLE between 0.0 and 1.0, or a LONG between 0 and 100.)",
            ),
        ),
        (
            r"@C { 'format' 'jpeg' 'tEXt' 'x' } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'format' 'gif' } Pencode",
            Err(r"Pencode failed. (Only formats 'png' and 'jpeg' are supported.)"),
        ),
        (
            r"@C { 'format' 5 } Pencode",
            Err(r"Pencode failed. (Only formats 'png' and 'jpeg' are supported.)"),
        ),
        (
            r"@C { 'format' NULL } Pencode",
            Err(r"Pencode failed. (Only formats 'png' and 'jpeg' are supported.)"),
        ),
        (
            r"@C { 'format' 'png' } Pencode",
            Err(r"Pencode failed. (Only 'tEXt', 'zTXt' and 'iTXt' chunks can be specified.)"),
        ),
        (
            r"@C { 'tEXt' [ { 'keyword' 'k' 'text' 'v' } ] } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'zTXt' [ { 'keyword' 'k' 'text' 'v' } ] } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'iTXt' [ { 'keyword' 'k' 'text' 'v' 'compressionFlag' true 'languageTag' 'en' 'translatedKeyword' 'kk' } ] } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'iTXt' [ { 'keyword' 'k' 'text' 'v' 'compressionFlag' false } ] } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'tEXt' [ { 'keyword' 'k' 'text' 'v' } { 'keyword' 'k2' 'text' 'v2' } ] } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'tEXt' 'x' } Pencode",
            Err(r"Pencode failed. (Chunk type must be associated with a list of chunks.)"),
        ),
        (
            r"@C { 'tEXt' [ 'x' ] } Pencode",
            Err(r"Pencode failed. (tEXt and zTXt chunks must be MAP instances.)"),
        ),
        (
            r"@C { 'tEXt' [ { 'keyword' 'k' } ] } Pencode",
            Err(
                r"Pencode failed. (tEXt and zTXt chunks MUST contains 'keyword' and 'text' entries of type STRING.)",
            ),
        ),
        (
            r"@C { 'tEXt' [ { 'keyword' 5 'text' 'v' } ] } Pencode",
            Err(
                r"Pencode failed. (tEXt and zTXt chunks MUST contains 'keyword' and 'text' entries of type STRING.)",
            ),
        ),
        (
            r"@C { 'zTXt' [ { 'keyword' 'k' 'text' 5 } ] } Pencode",
            Err(
                r"Pencode failed. (tEXt and zTXt chunks MUST contains 'keyword' and 'text' entries of type STRING.)",
            ),
        ),
        (
            r"@C { 'iTXt' 'x' } Pencode",
            Err(r"Pencode failed. (Chunk type must be associated with a list of chunks.)"),
        ),
        (
            r"@C { 'iTXt' [ 'x' ] } Pencode",
            Err(r"Pencode failed. (iTXt chunks must be MAP instances.)"),
        ),
        (
            r"@C { 'iTXt' [ { 'keyword' 'k' } ] } Pencode",
            Err(
                r"Pencode failed. (iTXt chunks MUST contains 'keyword' and 'text' entries of type STRING.)",
            ),
        ),
        (
            r"@C { 'iTXt' [ { 'keyword' 'k' 'text' 'v' 'languageTag' NULL } ] } Pencode",
            Err(r"NullPointerException"),
        ),
        (
            r"@C { 'bogus' [ ] } Pencode",
            Err(r"Pencode failed. (Only 'tEXt', 'zTXt' and 'iTXt' chunks can be specified.)"),
        ),
        (
            r"@C { 'format' 'jpeg' 'tEXt' 'x' 'iTXt' 'x' } Pencode Pdecode Psize",
            Ok(r"[2,2]"),
        ),
        (
            r"@C { 'tEXt' [ { 'keyword' 'k' 'text' 'v' } ] 'bogus' [ ] } Pencode",
            Err(r"Pencode failed. (Only 'tEXt', 'zTXt' and 'iTXt' chunks can be specified.)"),
        ),
        (
            r"@C 5 Pencode",
            Err(r"Pencode operates on a PGRAPHICS or PIMAGE instance."),
        ),
        (
            r"@C { } 5 Pencode",
            Err(r"Pencode operates on a PGRAPHICS or PIMAGE instance."),
        ),
        (
            r"@C Pencode Pdecode Pdecode",
            Err(r"Pdecode expects a base64 data URI or a byte array on top of the stack."),
        ),
        (
            r"'x' Pdecode",
            Err(r"Pdecode expects a base64 data URI or a byte array on top of the stack."),
        ),
        (
            r"NULL Pdecode",
            Err(r"Pdecode expects a base64 data URI or a byte array on top of the stack."),
        ),
        (
            r"'hello' Pdecode",
            Err(r"Pdecode expects a base64 data URI or a byte array on top of the stack."),
        ),
        (
            r"@C Pencode Pdecode Ppixels",
            Ok(r"[[-8355712,-8355712,-8355712,-16777216]]"),
        ),
        (
            r"@C 'x' Pencode",
            Err(r"Pencode operates on a PGRAPHICS or PIMAGE instance."),
        ),
    ]);
}
