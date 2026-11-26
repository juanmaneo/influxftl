// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the Processing functions: the transform functions of the Processing API.
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
fn transform() {
    check(&[
        (r"@G PpushMatrix PpopMatrix", Ok(r"[null]")),
        (
            r"@G PpopMatrix",
            Err(r"missing a pushMatrix() to go with that popMatrix()"),
        ),
        (
            r"@G PpushMatrix PpushMatrix PpopMatrix PpopMatrix",
            Ok(r"[null]"),
        ),
        (r"@G PresetMatrix", Ok(r"[null]")),
        (r"@G 1 Protate", Ok(r"[null]")),
        (r"@G 1.5 Protate", Ok(r"[null]")),
        (
            r"@G 'x' Protate",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G Protate",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (
            r"@G 1 2 Protate",
            Err(r"Invalid number of parameters, expected one of [1] but found 2"),
        ),
        (r"@G 1 ProtateX", Ok(r"[null]")),
        (r"@G 1 ProtateY", Ok(r"[null]")),
        (r"@G 1 ProtateZ", Ok(r"[null]")),
        (
            r"@G 'x' ProtateY",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G 1 2 Pscale", Ok(r"[null]")),
        (r"@G 2 Pscale", Ok(r"[null]")),
        (r"@G 1 2 3 Pscale", Ok(r"[null]")),
        (
            r"@G 1 2 3 4 Pscale",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3] but found 4"),
        ),
        (
            r"@G 'x' Pscale",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G Pscale",
            Err(r"Invalid number of parameters, expected one of [1, 2, 3] but found 0"),
        ),
        (r"@G 1 PshearX", Ok(r"[null]")),
        (r"@G 1 PshearY", Ok(r"[null]")),
        (
            r"@G 'x' PshearX",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G PshearY",
            Err(r"Invalid number of parameters, expected one of [1] but found 0"),
        ),
        (r"@G 1 2 Ptranslate", Ok(r"[null]")),
        (r"@G 1 2 3 Ptranslate", Ok(r"[null]")),
        (
            r"@G 1 Ptranslate",
            Err(r"Invalid number of parameters, expected one of [2, 3] but found 1"),
        ),
        (
            r"@G 'x' 2 Ptranslate",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 1 2 3 4 Ptranslate",
            Err(r"Invalid number of parameters, expected one of [2, 3] but found 4"),
        ),
        (r"@G 1 2 Ptranslate 3 4 Ptranslate", Ok(r"[null]")),
        (r"@G 1 2 Ptranslate PresetMatrix", Ok(r"[null]")),
        (
            r"@G PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix",
            Ok(r"[null]"),
        ),
        (
            r"@G PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix PpushMatrix",
            Err(r"pushMatrix() cannot use push more than 32 times"),
        ),
    ]);
}

#[test]
fn refusals() {
    check(&[
        (
            r"@G 1 PpushMatrix",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
        (
            r"@G 1 PpopMatrix",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
        (
            r"@G 1 PresetMatrix",
            Err(r"Invalid number of parameters, expected one of [0] but found 1"),
        ),
        (
            r"@G 'x' 1 Pscale",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'x' 1 2 Pscale",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@G 'x' 1 2 Ptranslate",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (r"@G 3e38 Pscale 3e38 Pscale", Err(r"not a finite number")),
        (
            r"@G 3e38 0 Ptranslate 3e38 0 Ptranslate",
            Err(r"not a finite number"),
        ),
        (r"@G 3e38 Pscale 1 PshearX", Err(r"not a finite number")),
        (
            r"@G 3e38 Pscale 0.7853982 PshearX 0.7853982 Protate",
            Err(r"not a finite number"),
        ),
    ]);
}
