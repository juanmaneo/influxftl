// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Oracle-checked tests of `INTERPOLATOR.ND.MICROSPHERE` and `INTERPOLATOR.ND.SMICROSPHERE`: the points the microsphere
//! has samples for exactly, the seeded interpolations up to rounding, the snapshots and the errors.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts: the JSON stack (top
//! first) or the error message (in its JDK 11+ form).

use serde_json::Value as Json;

use crate::tests::test_stack;
use crate::value::Value;

const P2: &str =
    r"[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ]";
const P1: &str = r"[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ]";
const P3: &str = r"[ [ 0.0 0.0 0.0 ] [ 1.0 0.0 0.0 ] [ 0.0 1.0 0.0 ] [ 0.0 0.0 1.0 ] [ 1.0 1.0 1.0 ] ] [ 0.0 1.0 2.0 3.0 6.0 ]";
const P4: &str = r"[ [ 0.0 0.0 0.0 0.0 ] [ 1.0 0.0 0.0 0.0 ] [ 0.0 1.0 0.0 0.0 ] [ 0.0 0.0 1.0 0.0 ] [ 0.0 0.0 0.0 1.0 ] [ 1.0 1.0 1.0 1.0 ] ] [ 0.0 1.0 2.0 3.0 4.0 10.0 ]";
const GX: &str = r"NEWGTS 'x' RENAME 1 NaN NaN NaN 0.0 ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE 3 NaN NaN NaN 0.5 ADDVALUE";
const GY: &str = r"NEWGTS 'y' RENAME 1 NaN NaN NaN 0.0 ADDVALUE 2 NaN NaN NaN 0.0 ADDVALUE 3 NaN NaN NaN 0.5 ADDVALUE";
const GZ: &str = r"NEWGTS 'z' RENAME 1 NaN NaN NaN 0.0 ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE 3 NaN NaN NaN 1.5 ADDVALUE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@P2", P2)
        .replace("@P1", P1)
        .replace("@P3", P3)
        .replace("@P4", P4)
        .replace("@GX", GX)
        .replace("@GY", GY)
        .replace("@GZ", GZ)
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

/// Whether two JSON documents are the same, their numbers equal up to rounding errors.
fn close(actual: &Json, expected: &Json) -> bool {
    match (actual, expected) {
        (Json::Number(a), Json::Number(b)) => match (a.as_i64(), b.as_i64()) {
            (Some(a), Some(b)) => a == b,
            _ => {
                let (a, b) = (
                    a.as_f64().expect("a finite number"),
                    b.as_f64().expect("a finite number"),
                );
                (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
            }
        },
        (Json::Array(a), Json::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(a, b))
        }
        (Json::Object(a), Json::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(key, a)| b.get(key).is_some_and(|b| close(a, b)))
        }
        (a, b) => a == b,
    }
}

/// Check every case, reporting all the mismatches at once.
fn check(cases: &[(&str, Result<&str, &str>)]) {
    let mut failures = Vec::new();
    for (template, expected) in cases {
        let script = expand(template);
        let matches = match (expected, stack_json(&script)) {
            (Ok(json), Ok(actual)) => {
                let expected: Json = serde_json::from_str(json).expect("the expectation is JSON");
                let actual: Json = serde_json::from_str(&actual).expect("the result is JSON");
                close(&actual, &expected)
            }
            (Err(message), Err(actual)) => actual.contains(message),
            _ => false,
        };
        if !matches {
            failures.push(format!(
                "{template}\n  expected {expected:?}\n  actual   {:?}",
                stack_json(&script)
            ));
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
fn at_samples() {
    check(&[
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE [ 0.0 0.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"42 PRNG @P2 INTERPOLATOR.ND.SMICROSPHERE [ 0.0 0.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE [ 1.0 0.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 INTERPOLATOR.ND.SMICROSPHERE [ 1.0 0.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE [ 0.0 1.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 INTERPOLATOR.ND.SMICROSPHERE [ 0.0 1.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE [ 1.0 1.0 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"42 PRNG @P2 INTERPOLATOR.ND.SMICROSPHERE [ 1.0 1.0 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE [ 0.5 0.5 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"@P1 INTERPOLATOR.ND.MICROSPHERE [ 0.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"42 PRNG @P1 INTERPOLATOR.ND.SMICROSPHERE [ 0.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"@P1 INTERPOLATOR.ND.MICROSPHERE [ 1.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P1 INTERPOLATOR.ND.SMICROSPHERE [ 1.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"@P1 INTERPOLATOR.ND.MICROSPHERE [ 3.0 ] SWAP EVAL",
            Ok(r"[9.0]"),
        ),
        (
            r"42 PRNG @P1 INTERPOLATOR.ND.SMICROSPHERE [ 3.0 ] SWAP EVAL",
            Ok(r"[9.0]"),
        ),
        (
            r"@P3 INTERPOLATOR.ND.MICROSPHERE [ 0.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"42 PRNG @P3 INTERPOLATOR.ND.SMICROSPHERE [ 0.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"@P3 INTERPOLATOR.ND.MICROSPHERE [ 1.0 1.0 1.0 ] SWAP EVAL",
            Ok(r"[6.0]"),
        ),
        (
            r"42 PRNG @P3 INTERPOLATOR.ND.SMICROSPHERE [ 1.0 1.0 1.0 ] SWAP EVAL",
            Ok(r"[6.0]"),
        ),
        (
            r"@P3 INTERPOLATOR.ND.MICROSPHERE [ 0.0 0.0 1.0 ] SWAP EVAL",
            Ok(r"[3.0]"),
        ),
        (
            r"42 PRNG @P3 INTERPOLATOR.ND.SMICROSPHERE [ 0.0 0.0 1.0 ] SWAP EVAL",
            Ok(r"[3.0]"),
        ),
        (
            r"@P4 INTERPOLATOR.ND.MICROSPHERE [ 0.0 0.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"42 PRNG @P4 INTERPOLATOR.ND.SMICROSPHERE [ 0.0 0.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"@P4 INTERPOLATOR.ND.MICROSPHERE [ 1.0 1.0 1.0 1.0 ] SWAP EVAL",
            Ok(r"[10.0]"),
        ),
        (
            r"42 PRNG @P4 INTERPOLATOR.ND.SMICROSPHERE [ 1.0 1.0 1.0 1.0 ] SWAP EVAL",
            Ok(r"[10.0]"),
        ),
        (
            r"@P2 { 'no.interpolation.tolerance' 0.1 } INTERPOLATOR.ND.MICROSPHERE [ 0.05 0.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"@P2 { 'no.interpolation.tolerance' 0.1 } INTERPOLATOR.ND.MICROSPHERE [ 0.95 0.95 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"@P2 { 'no.interpolation.tolerance' 0.1 } INTERPOLATOR.ND.MICROSPHERE [ 0.5 0.45 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"@P2 { 'no.interpolation.tolerance' 0.1 } INTERPOLATOR.ND.MICROSPHERE [ 0.0 0.05 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"@P2 { 'elements' 1 } INTERPOLATOR.ND.MICROSPHERE [ 1.0 1.0 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"@P2 { 'elements' 2 } INTERPOLATOR.ND.MICROSPHERE [ 1.0 1.0 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"@P2 { 'elements' 5 } INTERPOLATOR.ND.MICROSPHERE [ 1.0 1.0 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"@P2 { 'elements' 50 } INTERPOLATOR.ND.MICROSPHERE [ 1.0 1.0 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"@P2 { 'elements' 0 } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"0 is smaller than, or equal to, the minimum (0)"),
        ),
        (
            r"@P2 { 'elements' -1 } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"-1 is smaller than, or equal to, the minimum (0)"),
        ),
    ]);
}

#[test]
fn seeded_0() {
    check(&[
        (
            r"1 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.2368933524020567]"),
        ),
        (
            r"1 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0603364489782703]"),
        ),
        (
            r"1 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0085717617328065]"),
        ),
        (
            r"1 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.293743835252072]"),
        ),
        (
            r"1 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.2368933524020567]"),
        ),
        (
            r"1 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"1 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0481334891694625]"),
        ),
        (
            r"1 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0126992575099871]"),
        ),
        (
            r"1 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"1 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[2.0]"),
        ),
        (
            r"1 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"1 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[0.698167263285822]"),
        ),
        (
            r"1 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[0.47547241181925254]"),
        ),
        (
            r"1 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"1 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"1 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"1 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"1 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"1 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"1 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL",
            Ok(r"[7.0]"),
        ),
        (
            r"1 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[0.5]"),
        ),
        (
            r"1 PRNG @P1 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[0.6]"),
        ),
        (
            r"1 PRNG @P1 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[0.5]"),
        ),
        (
            r"1 PRNG @P1 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[0.5]"),
        ),
        (
            r"1 PRNG @P1 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[0.5]"),
        ),
        (
            r"1 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[5.0]"),
        ),
        (
            r"1 PRNG @P1 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[5.8]"),
        ),
        (
            r"1 PRNG @P1 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[5.0]"),
        ),
        (
            r"1 PRNG @P1 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[5.0]"),
        ),
        (
            r"1 PRNG @P1 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[5.0]"),
        ),
        (
            r"1 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"1 PRNG @P1 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"1 PRNG @P1 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"1 PRNG @P1 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"1 PRNG @P1 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"1 PRNG @P3  INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[2.5871155809345265]"),
        ),
        (
            r"1 PRNG @P3 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[1.0011049421594336]"),
        ),
        (
            r"1 PRNG @P3 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[1.1004958089445107]"),
        ),
        (
            r"1 PRNG @P3 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[2.566229149370804]"),
        ),
        (
            r"1 PRNG @P3 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[2.5871155809345265]"),
        ),
        (
            r"1 PRNG @P3  INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[6.0]"),
        ),
        (
            r"1 PRNG @P3 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[1.769963349516707]"),
        ),
        (
            r"1 PRNG @P3 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[1.3013118271866437]"),
        ),
        (
            r"1 PRNG @P3 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[6.0]"),
        ),
        (
            r"1 PRNG @P3 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[6.0]"),
        ),
        (
            r"1 PRNG @P4  INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[5.7992871962590895]"),
        ),
        (
            r"1 PRNG @P4 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[4.826017445957166]"),
        ),
        (
            r"1 PRNG @P4 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[4.575882757845434]"),
        ),
        (
            r"1 PRNG @P4 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[5.7992871962590895]"),
        ),
        (
            r"1 PRNG @P4 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[5.7992871962590895]"),
        ),
        (
            r"42 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0284595132803735]"),
        ),
        (
            r"42 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0100741605218864]"),
        ),
        (
            r"42 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0203160602011436]"),
        ),
        (
            r"42 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"42 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
    ]);
}

#[test]
fn seeded_1() {
    check(&[
        (
            r"42 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[0.8244935891591715]"),
        ),
        (
            r"42 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[0.7796485449011679]"),
        ),
        (
            r"42 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[0.599249887765873]"),
        ),
        (
            r"42 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[0.8244935891591715]"),
        ),
        (
            r"42 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"42 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"42 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P1 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[0.9]"),
        ),
        (
            r"42 PRNG @P1 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[0.7333333333333333]"),
        ),
        (
            r"42 PRNG @P1 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P1 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"42 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[9.0]"),
        ),
        (
            r"42 PRNG @P1 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[8.2]"),
        ),
        (
            r"42 PRNG @P1 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[6.866666666666666]"),
        ),
        (
            r"42 PRNG @P1 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[9.0]"),
        ),
        (
            r"42 PRNG @P1 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[9.0]"),
        ),
        (
            r"42 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"42 PRNG @P1 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"42 PRNG @P1 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"42 PRNG @P1 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"42 PRNG @P1 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"42 PRNG @P3  INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[4.280863737625029]"),
        ),
        (
            r"42 PRNG @P3 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[2.7898659980607086]"),
        ),
        (
            r"42 PRNG @P3 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[1.1174700388498129]"),
        ),
        (
            r"42 PRNG @P3 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[4.487808385232013]"),
        ),
        (
            r"42 PRNG @P3 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[4.280863737625029]"),
        ),
        (
            r"42 PRNG @P3  INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[1.9077051315513585]"),
        ),
        (
            r"42 PRNG @P3 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[1.6560170382628097]"),
        ),
        (
            r"42 PRNG @P3 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[1.2486158520417538]"),
        ),
        (
            r"42 PRNG @P3 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[2.1297799110574935]"),
        ),
        (
            r"42 PRNG @P3 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[1.9077051315513585]"),
        ),
        (
            r"42 PRNG @P4  INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[7.888655022655493]"),
        ),
        (
            r"42 PRNG @P4 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[5.931456862431314]"),
        ),
        (
            r"42 PRNG @P4 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[5.053455351257716]"),
        ),
        (
            r"42 PRNG @P4 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[7.888655022655493]"),
        ),
        (
            r"42 PRNG @P4 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[7.888655022655493]"),
        ),
        (
            r"12345 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.026974307903277]"),
        ),
        (
            r"12345 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.003721345793303]"),
        ),
        (
            r"12345 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.3 0.7 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.092798942925864]"),
        ),
        (
            r"12345 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0095415607883647]"),
        ),
        (
            r"12345 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 2.0 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[0.8525844844776567]"),
        ),
        (
            r"12345 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[0.5787005359219428]"),
        ),
        (
            r"12345 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.25 0.25 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"12345 PRNG @P2 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"12345 PRNG @P2 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"12345 PRNG @P2 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"12345 PRNG @P2 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 3.0 3.0 ] SWAP EVAL",
            Ok(r"[7.0]"),
        ),
    ]);
}

#[test]
fn seeded_2() {
    check(&[
        (
            r"12345 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P1 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[0.7]"),
        ),
        (
            r"12345 PRNG @P1 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[0.6]"),
        ),
        (
            r"12345 PRNG @P1 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P1 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 ] SWAP EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"12345 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[9.0]"),
        ),
        (
            r"12345 PRNG @P1 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[6.6]"),
        ),
        (
            r"12345 PRNG @P1 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[5.8]"),
        ),
        (
            r"12345 PRNG @P1 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[9.0]"),
        ),
        (
            r"12345 PRNG @P1 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 ] SWAP EVAL",
            Ok(r"[9.0]"),
        ),
        (
            r"12345 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"12345 PRNG @P1 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"12345 PRNG @P1 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"12345 PRNG @P1 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"12345 PRNG @P1 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ -1.0 ] SWAP EVAL",
            Ok(r"[0.0]"),
        ),
        (
            r"12345 PRNG @P3  INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[2.2923683844817275]"),
        ),
        (
            r"12345 PRNG @P3 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[2.3545925130118435]"),
        ),
        (
            r"12345 PRNG @P3 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[1.2198415336709858]"),
        ),
        (
            r"12345 PRNG @P3 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[3.1950405356250124]"),
        ),
        (
            r"12345 PRNG @P3 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.3 0.4 ] SWAP EVAL",
            Ok(r"[2.2923683844817275]"),
        ),
        (
            r"12345 PRNG @P3  INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[3.3850893017832435]"),
        ),
        (
            r"12345 PRNG @P3 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[1.8367355335622881]"),
        ),
        (
            r"12345 PRNG @P3 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[1.3259153296141344]"),
        ),
        (
            r"12345 PRNG @P3 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[3.727691896440879]"),
        ),
        (
            r"12345 PRNG @P3 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 2.0 0.0 0.0 ] SWAP EVAL",
            Ok(r"[3.3850893017832435]"),
        ),
        (
            r"12345 PRNG @P4  INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[7.184196254891774]"),
        ),
        (
            r"12345 PRNG @P4 { 'elements' 10 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[4.867150927159371]"),
        ),
        (
            r"12345 PRNG @P4 { 'elements' 30 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[4.318273035902612]"),
        ),
        (
            r"12345 PRNG @P4 { 'exponent' 1.5 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[7.184196254891774]"),
        ),
        (
            r"12345 PRNG @P4 { 'dark.threshold' 0.0 'background' 7.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.5 0.5 0.5 0.5 ] SWAP EVAL",
            Ok(r"[7.184196254891774]"),
        ),
    ]);
}

#[test]
fn background() {
    check(&[
        (
            r"42 PRNG @P2 { 'max.dark.fraction' 0.0 'background' 42.0 'elements' 3 } INTERPOLATOR.ND.SMICROSPHERE [ -5.0 -5.0 ] SWAP EVAL",
            Ok(r"[42.0]"),
        ),
        (
            r"42 PRNG @P2 { 'max.dark.fraction' 0.0 'background' 42.0 'elements' 3 } INTERPOLATOR.ND.SMICROSPHERE [ 5.0 5.0 ] SWAP EVAL",
            Ok(r"[42.0]"),
        ),
        (
            r"7 PRNG @P2 { 'max.dark.fraction' 0.0 'background' 42.0 'elements' 3 } INTERPOLATOR.ND.SMICROSPHERE [ -5.0 -5.0 ] SWAP EVAL",
            Ok(r"[42.0]"),
        ),
        (
            r"7 PRNG @P2 { 'max.dark.fraction' 0.2 'background' -1.0 'elements' 5 } INTERPOLATOR.ND.SMICROSPHERE [ -5.0 -5.0 ] SWAP EVAL",
            Ok(r"[-1.0]"),
        ),
        (
            r"9 PRNG @P2 { 'dark.threshold' 100.0 'background' 5.5 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.2 ] SWAP EVAL",
            Ok(r"[5.5]"),
        ),
        (
            r"9 PRNG @P2 { 'dark.threshold' 100.0 } INTERPOLATOR.ND.SMICROSPHERE [ 0.2 0.2 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
        (
            r"9 PRNG @P2 { 'max.dark.fraction' 1.0 } INTERPOLATOR.ND.SMICROSPHERE [ -50.0 -50.0 ] SWAP EVAL ISNaN",
            Ok(r"[true]"),
        ),
    ]);
}

#[test]
fn snapshots() {
    check(&[
        (
            r"@P2  INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P2  INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P2  INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P2  INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P2 { } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P2 { } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P2 { } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P2 { } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P2 { 'elements' 20 } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 20 'elements' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P2 { 'elements' 20 } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 20 'elements' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P2 { 'elements' 20 } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P2 { 'elements' 20 } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 20 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'elements' 20 } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 20 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'elements' 20 } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P2 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 20 'elements' PUT 3 'exponent' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P2 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 20 'elements' PUT 3 'exponent' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P2 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P2 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 20 'elements' PUT 3 'exponent' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 20 'elements' PUT 3 'exponent' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P2 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 0.01 'no.interpolation.tolerance' PUT 1.0 'background' PUT 20 'elements' PUT 0.1 'dark.threshold' PUT 0.2 'max.dark.fraction' PUT 3 'exponent' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P2 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 0.01 'no.interpolation.tolerance' PUT 1.0 'background' PUT 20 'elements' PUT 0.1 'dark.threshold' PUT 0.2 'max.dark.fraction' PUT 3 'exponent' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P2 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P2 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 0.01 'no.interpolation.tolerance' PUT 1.0 'background' PUT 20 'elements' PUT 0.1 'dark.threshold' PUT 0.2 'max.dark.fraction' PUT 3 'exponent' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 0.01 'no.interpolation.tolerance' PUT 1.0 'background' PUT 20 'elements' PUT 0.1 'dark.threshold' PUT 0.2 'max.dark.fraction' PUT 3 'exponent' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P2 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 'y' 'x' PUT 1 'a' PUT 2 'b' PUT 3 'c' PUT 4 'elements' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P2 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 'y' 'x' PUT 1 'a' PUT 2 'b' PUT 3 'c' PUT 4 'elements' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P2 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P2 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 'y' 'x' PUT 1 'a' PUT 2 'b' PUT 3 'c' PUT 4 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 'y' 'x' PUT 1 'a' PUT 2 'b' PUT 3 'c' PUT 4 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P2 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} true 'flag' PUT 'it%27%27s' 'text' PUT [] 1 +! 2 +! 'list' PUT {} 'v' 'k' PUT 'map' PUT 3 'elements' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P2 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} true 'flag' PUT 'it%27%27s' 'text' PUT [] 1 +! 2 +! 'list' PUT {} 'v' 'k' PUT 'map' PUT 3 'elements' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P2 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P2 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} true 'flag' PUT 'it%27%27s' 'text' PUT [] 1 +! 2 +! 'list' PUT {} 'v' 'k' PUT 'map' PUT 3 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} true 'flag' PUT 'it%27%27s' 'text' PUT [] 1 +! 2 +! 'list' PUT {} 'v' 'k' PUT 'map' PUT 3 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P2 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P1  INTERPOLATOR.ND.MICROSPHERE",
            Ok(r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] INTERPOLATOR.ND.MICROSPHERE"]"#),
        ),
        (
            r"@P1  INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] INTERPOLATOR.ND.MICROSPHERE "]"#),
        ),
        (
            r"@P1  INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE",
            Ok(r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] INTERPOLATOR.ND.SMICROSPHERE"]"#),
        ),
        (
            r"42 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] INTERPOLATOR.ND.SMICROSPHERE "]"#),
        ),
        (
            r"42 PRNG @P1  INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P1 { } INTERPOLATOR.ND.MICROSPHERE",
            Ok(r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} INTERPOLATOR.ND.MICROSPHERE"]"#),
        ),
        (
            r"@P1 { } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P1 { } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P1 { } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P1 { 'elements' 20 } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 20 'elements' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P1 { 'elements' 20 } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 20 'elements' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P1 { 'elements' 20 } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P1 { 'elements' 20 } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 20 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'elements' 20 } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 20 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'elements' 20 } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P1 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 20 'elements' PUT 3 'exponent' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P1 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 20 'elements' PUT 3 'exponent' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P1 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P1 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 20 'elements' PUT 3 'exponent' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 20 'elements' PUT 3 'exponent' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'elements' 20 'exponent' 3 } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P1 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 0.01 'no.interpolation.tolerance' PUT 1.0 'background' PUT 20 'elements' PUT 0.1 'dark.threshold' PUT 0.2 'max.dark.fraction' PUT 3 'exponent' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P1 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 0.01 'no.interpolation.tolerance' PUT 1.0 'background' PUT 20 'elements' PUT 0.1 'dark.threshold' PUT 0.2 'max.dark.fraction' PUT 3 'exponent' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P1 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P1 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 0.01 'no.interpolation.tolerance' PUT 1.0 'background' PUT 20 'elements' PUT 0.1 'dark.threshold' PUT 0.2 'max.dark.fraction' PUT 3 'exponent' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 0.01 'no.interpolation.tolerance' PUT 1.0 'background' PUT 20 'elements' PUT 0.1 'dark.threshold' PUT 0.2 'max.dark.fraction' PUT 3 'exponent' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'exponent' 3 'elements' 20 'background' 1.0 'dark.threshold' 0.1 'max.dark.fraction' 0.2 'no.interpolation.tolerance' 0.01 } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P1 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 'y' 'x' PUT 1 'a' PUT 2 'b' PUT 3 'c' PUT 4 'elements' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P1 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 'y' 'x' PUT 1 'a' PUT 2 'b' PUT 3 'c' PUT 4 'elements' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P1 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P1 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 'y' 'x' PUT 1 'a' PUT 2 'b' PUT 3 'c' PUT 4 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} 'y' 'x' PUT 1 'a' PUT 2 'b' PUT 3 'c' PUT 4 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'a' 1 'b' 2 'c' 3 'elements' 4 'x' 'y' } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P1 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.MICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} true 'flag' PUT 'it%27%27s' 'text' PUT [] 1 +! 2 +! 'list' PUT {} 'v' 'k' PUT 'map' PUT 3 'elements' PUT INTERPOLATOR.ND.MICROSPHERE"]"#,
            ),
        ),
        (
            r"@P1 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} true 'flag' PUT 'it%27%27s' 'text' PUT [] 1 +! 2 +! 'list' PUT {} 'v' 'k' PUT 'map' PUT 3 'elements' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P1 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"42 PRNG @P1 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.SMICROSPHERE",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} true 'flag' PUT 'it%27%27s' 'text' PUT [] 1 +! 2 +! 'list' PUT {} 'v' 'k' PUT 'map' PUT 3 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE"]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.SMICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 ] [ 1.0 ] [ 3.0 ] ] [ 0.0 1.0 9.0 ] {} true 'flag' PUT 'it%27%27s' 'text' PUT [] 1 +! 2 +! 'list' PUT {} 'v' 'k' PUT 'map' PUT 3 'elements' PUT INTERPOLATOR.ND.SMICROSPHERE "]"#,
            ),
        ),
        (
            r"42 PRNG @P1 { 'elements' 3 'list' [ 1 2 ] 'map' { 'k' 'v' } 'flag' T 'text' 'it''s' } INTERPOLATOR.ND.SMICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
    ]);
}

#[test]
fn reducers() {
    check(&[
        (
            r"42 PRNG @P2 INTERPOLATOR.ND.SMICROSPHERE 'f' STORE [ [ @GX @GY ] [] $f ] REDUCE",
            Ok(r#"[[{"c":"","l":{},"a":{},"la":0,"v":[[1,0.0],[2,1.0],[3,1.0]]}]]"#),
        ),
        (
            r"42 PRNG @P2 INTERPOLATOR.ND.SMICROSPHERE 'f' STORE [ [ @GX @GY ] [] $f ] REDUCE VALUES",
            Ok(r"[[[0.0,1.0,1.0]]]"),
        ),
        (
            r"42 PRNG @P2 INTERPOLATOR.ND.SMICROSPHERE 'f' STORE [ [ @GX @GY ] [] $f ] REDUCE 0 GET TICKLIST",
            Ok(r"[[1,2,3]]"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE 'f' STORE [ [ @GX @GY ] [] $f ] REDUCE 0 GET VALUES",
            Ok(r"[[0.0,1.0,1.0]]"),
        ),
        (
            r"42 PRNG @P3 INTERPOLATOR.ND.SMICROSPHERE 'f' STORE [ [ @GX @GY @GZ ] [] $f ] REDUCE 0 GET VALUES",
            Ok(r"[[0.0,4.315155375230548,4.169327050980188]]"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE 'f' STORE [ [ @GX @GY @GZ ] [] $f ] REDUCE",
            Err(r"2 != 3"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE 'f' STORE [ [ @GX ] [] $f ] REDUCE",
            Err(r"2 != 1"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE 'f' STORE [ [ @GX NEWGTS 'a' RENAME 1 NaN NaN NaN 'a' ADDVALUE ] [] $f ] REDUCE",
            Err(
                r"MICROSPHERE expects to operate on numeric GTS when used as reducer, instead got at least one non numeric element",
            ),
        ),
    ]);
}

#[test]
fn errors() {
    check(&[
        (
            r"NEWGTS INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects GTS wth values of type DOUBLE."),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE INTERPOLATOR.ND.MICROSPHERE",
            Err(r"Empty stack."),
        ),
        (
            r"1 [ 0.0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects a List of List as first argument."),
        ),
        (
            r"'a' [ 0.0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects a List of List as first argument."),
        ),
        (r"INTERPOLATOR.ND.MICROSPHERE", Err(r"Empty stack.")),
        (r"INTERPOLATOR.ND.SMICROSPHERE", Err(r"Empty stack.")),
        (
            r"1 INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE: wrong argument type"),
        ),
        (
            r"'a' INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE: wrong argument type"),
        ),
        (
            r"NULL INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE: wrong argument type"),
        ),
        (r"{ } INTERPOLATOR.ND.MICROSPHERE", Err(r"Empty stack.")),
        (r"[ ] [ ] INTERPOLATOR.ND.MICROSPHERE", Err(r"-1")),
        (r"[ ] INTERPOLATOR.ND.MICROSPHERE", Err(r"Empty stack.")),
        (r"[ 0.0 ] INTERPOLATOR.ND.MICROSPHERE", Err(r"Empty stack.")),
        (
            r"[ 0.0 1.0 ] [ 0.0 1.0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects a List of List as first argument."),
        ),
        (
            r"[ [ 0.0 ] ] [ 0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects values of type DOUBLE."),
        ),
        (
            r"[ [ 0.0 ] ] [ 0.0 1.0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE: incoherent input list sizes."),
        ),
        (
            r"[ [ 0.0 ] [ 1.0 2.0 ] ] [ 0.0 1.0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE: incoherent input list sizes."),
        ),
        (
            r"[ [ 0 ] ] [ 0.0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects values of type DOUBLE."),
        ),
        (
            r"[ [ 0.0 ] 1 ] [ 0.0 1.0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects a List of List as first argument."),
        ),
        (
            r"[ [ 0.0 ] ] [ 'a' ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects values of type DOUBLE."),
        ),
        (
            r"[ [ ] ] [ 0.0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"0 is smaller than, or equal to, the minimum (0)"),
        ),
        (
            r"[ [ ] [ ] ] [ 0.0 1.0 ] INTERPOLATOR.ND.MICROSPHERE",
            Err(r"0 is smaller than, or equal to, the minimum (0)"),
        ),
        (
            r"@P2 { 'elements' 'a' } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@P2 { 'elements' NULL } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"NullPointerException"),
        ),
        (
            r"@P2 { 'elements' 51 } INTERPOLATOR.ND.MICROSPHERE",
            Err(
                r"INTERPOLATOR.ND.MICROSPHERE: argument elements is above the limit interpolation.microsphere.max.elements",
            ),
        ),
        (
            r"@P2 { 'elements' 50 } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P2 { 'elements' 2.0 } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P2 { 'elements' 2.9 } INTERPOLATOR.ND.MICROSPHERE SNAPSHOT",
            Ok(
                r#"["[ [ 0.0 0.0 ] [ 1.0 0.0 ] [ 0.0 1.0 ] [ 1.0 1.0 ] [ 0.5 0.5 ] ] [ 0.0 1.0 1.0 2.0 1.0 ] {} 2.9 'elements' PUT INTERPOLATOR.ND.MICROSPHERE "]"#,
            ),
        ),
        (
            r"@P2 { 'max.dark.fraction' 'a' } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@P2 { 'max.dark.fraction' 1.5 } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"1.5 out of [0, 1] range"),
        ),
        (
            r"@P2 { 'max.dark.fraction' -0.5 } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"-0.5 out of [0, 1] range"),
        ),
        (
            r"@P2 { 'max.dark.fraction' NULL } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"NullPointerException"),
        ),
        (
            r"@P2 { 'dark.threshold' -1.0 } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"-1 is smaller than the minimum (0)"),
        ),
        (
            r"@P2 { 'dark.threshold' 'a' } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@P2 { 'dark.threshold' NULL } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"NullPointerException"),
        ),
        (
            r"@P2 { 'background' 'a' } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@P2 { 'background' NULL } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"NullPointerException"),
        ),
        (
            r"@P2 { 'exponent' -1.0 } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"-1 is smaller than the minimum (0)"),
        ),
        (
            r"@P2 { 'exponent' -1 } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"-1 is smaller than the minimum (0)"),
        ),
        (
            r"@P2 { 'exponent' -1.5 } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"-1.5 is smaller than the minimum (0)"),
        ),
        (
            r"@P2 { 'exponent' 'a' } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@P2 { 'exponent' NULL } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"NullPointerException"),
        ),
        (
            r"@P2 { 'no.interpolation.tolerance' 'a' } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@P2 { 'no.interpolation.tolerance' NULL } INTERPOLATOR.ND.MICROSPHERE",
            Err(r"NullPointerException"),
        ),
        (
            r"@P2 { 1 2 } INTERPOLATOR.ND.MICROSPHERE TYPEOF",
            Ok(r#"["AGGREGATOR"]"#),
        ),
        (
            r"@P2 INTERPOLATOR.ND.SMICROSPHERE",
            Err(r"INTERPOLATOR.ND.SMICROSPHERE seeded PRNG was not initialized."),
        ),
        (
            r"@P2 { 'elements' 0 } INTERPOLATOR.ND.SMICROSPHERE",
            Err(r"INTERPOLATOR.ND.SMICROSPHERE seeded PRNG was not initialized."),
        ),
        (
            r"7 PRNG @P2 { 'elements' 0 } INTERPOLATOR.ND.SMICROSPHERE",
            Err(r"0 is smaller than, or equal to, the minimum (0)"),
        ),
        (
            r"7 PRNG @P2 { 'exponent' -1 } INTERPOLATOR.ND.SMICROSPHERE",
            Err(r"-1 is smaller than the minimum (0)"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE [ 0.3 ] SWAP EVAL",
            Err(r"2 != 1"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE [ 0.3 0.4 0.5 ] SWAP EVAL",
            Err(r"2 != 3"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE [ ] SWAP EVAL",
            Err(r"2 != 0"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE [ 'a' 'b' ] SWAP EVAL",
            Err(
                r"MICROSPHERE expects a numeric LIST as argument, instead got at least one non numeric element",
            ),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE 1 SWAP EVAL",
            Err(r"MICROSPHERE expects a LIST"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE 'a' SWAP EVAL",
            Err(r"MICROSPHERE expects a LIST"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE NULL SWAP EVAL",
            Err(r"MICROSPHERE expects a LIST"),
        ),
        (
            r"@P2 INTERPOLATOR.ND.MICROSPHERE EVAL",
            Err(r"Empty stack."),
        ),
        (
            r"@GX @GY INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects a List of GTS as first argument."),
        ),
        (
            r"[ @GX @GY ] @GZ INTERPOLATOR.ND.MICROSPHERE",
            Err(
                r"class java.util.ArrayList cannot be cast to class io.warp10.continuum.gts.GeoTimeSerie",
            ),
        ),
        (
            r"[ @GX ] @GX INTERPOLATOR.ND.MICROSPHERE",
            Err(
                r"class java.util.ArrayList cannot be cast to class io.warp10.continuum.gts.GeoTimeSerie",
            ),
        ),
        (
            r"[ @GX @GY ] NEWGTS INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects GTS wth values of type DOUBLE."),
        ),
        (
            r"[ @GX @GY ] @GZ { 'elements' 5 } INTERPOLATOR.ND.MICROSPHERE",
            Err(
                r"class java.util.ArrayList cannot be cast to class io.warp10.continuum.gts.GeoTimeSerie",
            ),
        ),
        (
            r"[ @GX NEWGTS ] @GZ INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects GTS wth values of type DOUBLE."),
        ),
        (
            r"[ @GX 1 ] @GZ INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects a List of GTS as first argument."),
        ),
        (
            r"[ NEWGTS ] NEWGTS INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects GTS wth values of type DOUBLE."),
        ),
        (
            r"[ @GX ] @GX SNAPSHOT INTERPOLATOR.ND.SMICROSPHERE",
            Err(r"INTERPOLATOR.ND.SMICROSPHERE: wrong argument type"),
        ),
        (
            r"[ @GX ] NEWGTS 1 NaN NaN NaN 1 ADDVALUE INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE expects GTS wth values of type DOUBLE."),
        ),
        (
            r"[ @GX ] NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE: incoherent GTS sizes."),
        ),
        (
            r"[ @GX ] NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE: incoherent GTS sizes."),
        ),
        (
            r"[ @GX ] NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE 5 NaN NaN NaN 1.0 ADDVALUE INTERPOLATOR.ND.MICROSPHERE",
            Err(r"INTERPOLATOR.ND.MICROSPHERE: GTS are not synchronized."),
        ),
    ]);
}
