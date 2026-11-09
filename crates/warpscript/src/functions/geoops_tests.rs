// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of the `GEOSHAPE` operations: `GEO.UNION`, `GEO.INTERSECTION`, `GEO.DIFFERENCE`, `GEO.INTERSECTS`, `GEO.WITHIN`,
//! `GEO.OPTIMIZE`, `GEO.NORMALIZE`, `GEOSHIFT` and `GEO.REGEXP`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const A: &str = r"[ 'b9ad' ] ->GEOSHAPE";
const C: &str = r"[ 'b9ae' ] ->GEOSHAPE";
const P: &str = r"[ 'b9' ] ->GEOSHAPE";
const PA: &str = r"[ 'b9a' ] ->GEOSHAPE";
const K: &str = r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' ] ->GEOSHAPE";
const K15: &str = r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' ] ->GEOSHAPE";
const M: &str = r"[ 'b9ad' 'b8' 'f' 'a1' ] ->GEOSHAPE";
const D: &str = r"[ 'b9ad1ee0' 'b9ad1ee1' 'f01234567' ] ->GEOSHAPE";
const E: &str = r"[ ] ->GEOSHAPE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@K15", K15)
        .replace("@PA", PA)
        .replace("@A", A)
        .replace("@C", C)
        .replace("@P", P)
        .replace("@K", K)
        .replace("@M", M)
        .replace("@D", D)
        .replace("@E", E)
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
fn union_1() {
    check(&[
        (r"@A @A GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A @C GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ad","b9ae"]]"#)),
        (r"@A @P GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@A @PA GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@A @K GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@A @K15 GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@A @M GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A @D GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f01234567","b9ad"]]"#),
        ),
        (r"@A @E GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@C @A GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ad","b9ae"]]"#)),
        (r"@C @C GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C @P GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@C @PA GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@C @K GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@C @K15 GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@C @M GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad","b9ae"]]"#),
        ),
        (
            r"@C @D GEO.UNION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567","b9ae"]]"#),
        ),
        (r"@C @E GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@P @A GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P @C GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P @P GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P @PA GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P @K GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P @K15 GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (
            r"@P @M GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9"]]"#),
        ),
        (
            r"@P @D GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f01234567","b9"]]"#),
        ),
        (r"@P @E GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@PA @A GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@PA @C GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@PA @P GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@PA @PA GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@PA @K GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@PA @K15 GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@PA @M GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9a"]]"#),
        ),
        (
            r"@PA @D GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f01234567","b9a"]]"#),
        ),
        (r"@PA @E GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K @A GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K @C GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K @P GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@K @PA GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K @K GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K @K15 GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K @M GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9a"]]"#),
        ),
        (
            r"@K @D GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f01234567","b9a"]]"#),
        ),
        (r"@K @E GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K15 @A GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @C GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (r"@K15 @P GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@K15 @PA GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K15 @K GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K15 @K15 GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @M GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["f","a1","b8","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @D GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["f01234567","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @E GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@M @A GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M @C GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad","b9ae"]]"#),
        ),
        (
            r"@M @P GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9"]]"#),
        ),
        (
            r"@M @PA GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9a"]]"#),
        ),
        (
            r"@M @K GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9a"]]"#),
        ),
        (
            r"@M @K15 GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["f","a1","b8","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@M @M GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M @D GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M @E GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@D @A GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f01234567","b9ad"]]"#),
        ),
        (
            r"@D @C GEO.UNION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567","b9ae"]]"#),
        ),
        (
            r"@D @P GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f01234567","b9"]]"#),
        ),
        (
            r"@D @PA GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f01234567","b9a"]]"#),
        ),
        (
            r"@D @K GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f01234567","b9a"]]"#),
        ),
        (
            r"@D @K15 GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["f01234567","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@D @M GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@D @D GEO.UNION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D @E GEO.UNION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (r"@E @A GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@E @C GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@E @P GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@E @PA GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@E @K GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@E @K15 GEO.UNION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@E @M GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@E @D GEO.UNION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (r"@E @E GEO.UNION T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"[ @A @C ] GEO.UNION T GEOSHAPE->",
            Ok(r#"[["b9ad","b9ae"]]"#),
        ),
        (r"[ @A @P @C ] GEO.UNION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"[ @A ] GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"[ ] GEO.UNION T GEOSHAPE->", Ok(r"[[]]")),
        (r"[ @K @A ] GEO.UNION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"[ @K15 @C @P @M @D ] GEO.UNION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9"]]"#),
        ),
        (
            r"[ @A 1 ] GEO.UNION",
            Err(
                r"GEO.UNION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"[ 1 @A ] GEO.UNION",
            Err(
                r"GEO.UNION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"[ 1 ] GEO.UNION",
            Err(
                r"GEO.UNION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"[ @A @C 'a' ] GEO.UNION",
            Err(
                r"GEO.UNION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"@A 1 GEO.UNION",
            Err(
                r"GEO.UNION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"1 @A GEO.UNION",
            Err(
                r"GEO.UNION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (r"1 GEO.UNION", Err(r"Empty stack.")),
        (r"'a' GEO.UNION", Err(r"Empty stack.")),
        (r"GEO.UNION", Err(r"Empty stack.")),
        (r"@A GEO.UNION", Err(r"Empty stack.")),
        (
            r"[ NULL ] GEO.UNION",
            Err(
                r"GEO.UNION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (r"NULL GEO.UNION", Err(r"Empty stack.")),
        (r"@A @C GEO.UNION @A @C GEO.UNION ==", Ok(r"[true]")),
    ]);
}

#[test]
fn union_2() {
    check(&[
        (r"{ 'a' @A } GEO.UNION", Err(r"Empty stack.")),
        (r"@A @A GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"[ @A @A ] GEO.UNION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A @A GEO.UNION SIZE", Ok(r"[1]")),
        (r"@M @M GEO.UNION SIZE", Ok(r"[4]")),
        (r"@D @D GEO.UNION SIZE", Ok(r"[3]")),
        (r"@A @C GEO.UNION TYPEOF", Ok(r#"["GEOSHAPE"]"#)),
    ]);
}

#[test]
fn intersection_1() {
    check(&[
        (r"@A @A GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A @C GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@A @P GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A @PA GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A @K GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@A @K15 GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (r"@A @M GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@A @D GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (r"@A @E GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@C @A GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@C @C GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C @P GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C @PA GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C @K GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (
            r"@C @K15 GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ae"]]"#),
        ),
        (r"@C @M GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@C @D GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@C @E GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@P @A GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@P @C GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@P @P GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P @PA GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@P @K GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@P @K15 GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (r"@P @M GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@P @D GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (r"@P @E GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@PA @A GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@PA @C GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@PA @P GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@PA @PA GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@PA @K GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@PA @K15 GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (r"@PA @M GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@PA @D GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (r"@PA @E GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@K @A GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@K @C GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (
            r"@K @P GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K @PA GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K @K GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K @K15 GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (r"@K @M GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@K @D GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (r"@K @E GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@K15 @A GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@K15 @C GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ae"]]"#),
        ),
        (
            r"@K15 @P GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @PA GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @K GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @K15 GEO.INTERSECTION T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @M GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@K15 @D GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (r"@K15 @E GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@M @A GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@M @C GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@M @P GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@M @PA GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@M @K GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@M @K15 GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M @M GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M @D GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (r"@M @E GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@D @A GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (r"@D @C GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@D @P GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (
            r"@D @PA GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (
            r"@D @K GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (
            r"@D @K15 GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1"]]"#),
        ),
        (
            r"@D @M GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D @D GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (r"@D @E GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @A GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @C GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @P GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @PA GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @K GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @K15 GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @M GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @D GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @E GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"[ @A @C ] GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"[ @A @P @C ] GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (r"[ @A ] GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"[ ] GEO.INTERSECTION T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"[ @K @A ] GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"[ @K15 @C @P @M @D ] GEO.INTERSECTION T GEOSHAPE->",
            Ok(r"[[]]"),
        ),
        (
            r"[ @A 1 ] GEO.INTERSECTION",
            Err(
                r"GEO.INTERSECTION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"[ 1 @A ] GEO.INTERSECTION",
            Err(
                r"GEO.INTERSECTION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"[ 1 ] GEO.INTERSECTION",
            Err(
                r"GEO.INTERSECTION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"[ @A @C 'a' ] GEO.INTERSECTION",
            Err(
                r"GEO.INTERSECTION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"@A 1 GEO.INTERSECTION",
            Err(
                r"GEO.INTERSECTION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (
            r"1 @A GEO.INTERSECTION",
            Err(
                r"GEO.INTERSECTION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (r"1 GEO.INTERSECTION", Err(r"Empty stack.")),
        (r"'a' GEO.INTERSECTION", Err(r"Empty stack.")),
        (r"GEO.INTERSECTION", Err(r"Empty stack.")),
        (r"@A GEO.INTERSECTION", Err(r"Empty stack.")),
        (
            r"[ NULL ] GEO.INTERSECTION",
            Err(
                r"GEO.INTERSECTION expects two GeoShape instances as the top 2 elements of the stack or a list of GeoShape instances.",
            ),
        ),
        (r"NULL GEO.INTERSECTION", Err(r"Empty stack.")),
        (
            r"@A @C GEO.INTERSECTION @A @C GEO.INTERSECTION ==",
            Ok(r"[true]"),
        ),
    ]);
}

#[test]
fn intersection_2() {
    check(&[
        (r"{ 'a' @A } GEO.INTERSECTION", Err(r"Empty stack.")),
        (r"@A @A GEO.INTERSECTION T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"[ @A @A ] GEO.INTERSECTION T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (r"@A @A GEO.INTERSECTION SIZE", Ok(r"[1]")),
        (r"@M @M GEO.INTERSECTION SIZE", Ok(r"[4]")),
        (r"@D @D GEO.INTERSECTION SIZE", Ok(r"[3]")),
        (r"@A @C GEO.INTERSECTION TYPEOF", Ok(r#"["GEOSHAPE"]"#)),
    ]);
}

#[test]
fn difference() {
    check(&[
        (r"@A @A GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@A @C GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A @P GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@A @PA GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@A @K GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@A @K15 GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@A @M GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@A @D GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (r"@A @E GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@C @A GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C @C GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@C @P GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@C @PA GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@C @K GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@C @K15 GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@C @M GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C @D GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C @E GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (
            r"@P @A GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@P @C GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9af"]]"#,
            ),
        ),
        (r"@P @P GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@P @PA GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f"]]"#,
            ),
        ),
        (
            r"@P @K GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f"]]"#,
            ),
        ),
        (
            r"@P @K15 GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9af"]]"#,
            ),
        ),
        (
            r"@P @M GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@P @D GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (r"@P @E GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (
            r"@PA @A GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@PA @C GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9af"]]"#,
            ),
        ),
        (r"@PA @P GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@PA @PA GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@PA @K GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@PA @K15 GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9af"]]"#)),
        (
            r"@PA @M GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@PA @D GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (r"@PA @E GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K @A GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K @C GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9af"]]"#,
            ),
        ),
        (r"@K @P GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@K @PA GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@K @K GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@K @K15 GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9af"]]"#)),
        (
            r"@K @M GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K @D GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (r"@K @E GEO.DIFFERENCE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K15 @A GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @C GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad"]]"#,
            ),
        ),
        (r"@K15 @P GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@K15 @PA GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@K15 @K GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@K15 @K15 GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@K15 @M GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @D GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (
            r"@K15 @E GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@M @A GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8"]]"#),
        ),
        (
            r"@M @C GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M @P GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8"]]"#),
        ),
        (
            r"@M @PA GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8"]]"#),
        ),
        (
            r"@M @K GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8"]]"#),
        ),
        (
            r"@M @K15 GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8"]]"#),
        ),
        (r"@M @M GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@M @D GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","f0123450","f0123451","f0123452","f0123453","f0123454","f0123455","f0123457","f0123458","f0123459","f012345a","f012345b","f012345c","f012345d","f012345e","f012345f","f01234560","f01234561","f01234562","f01234563","f01234564","f01234565","f01234566","f01234568","f01234569","f0123456a","f0123456b","f0123456c","f0123456d","f0123456e","f0123456f","a1","b8","f1","f2","f3","f4","f5","f6","f7","f8","f9","fa","fb","fc","fd","fe","ff","f00","f02","f03","f04","f05","f06","f07","f08","f09","f0a","f0b","f0c","f0d","f0e","f0f","f010","f011","f013","f014","f015","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","f0120","f0121","f0122","f0124","f0125","f0126","f0127","f0128","f0129","f012a","f012b","f012c","f012d","f012e","f012f","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","f01230","f01231","f01232","f01233","f01235","f01236","f01237","f01238","f01239","f0123a","f0123b","f0123c","f0123d","f0123e","f0123f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef","f012340","f012341","f012342","f012343","f012344","f012346","f012347","f012348","f012349","f01234a","f01234b","f01234c","f01234d","f01234e","f01234f"]]"#,
            ),
        ),
        (
            r"@M @E GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@D @A GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f01234567"]]"#),
        ),
        (
            r"@D @C GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D @P GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f01234567"]]"#),
        ),
        (
            r"@D @PA GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f01234567"]]"#),
        ),
        (
            r"@D @K GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f01234567"]]"#),
        ),
        (
            r"@D @K15 GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["f01234567"]]"#),
        ),
        (r"@D @M GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@D @D GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@D @E GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (r"@E @A GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @C GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @P GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @PA GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @K GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @K15 GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @M GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @D GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E @E GEO.DIFFERENCE T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@P [ 'b9ad1e' ] ->GEOSHAPE GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f"]]"#,
            ),
        ),
        (
            r"@P [ 'b9ad1ee0' ] ->GEOSHAPE GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (
            r"@P [ 'b9ad1ee0c5' ] ->GEOSHAPE GEO.DIFFERENCE SIZE",
            Ok(r"[120]"),
        ),
        (
            r"[ 'b9ad1ee0c5' ] ->GEOSHAPE @P GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r"[[]]"),
        ),
        (
            r"@K [ 'b9a0' 'b9a1' ] ->GEOSHAPE GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"[ 'b' ] ->GEOSHAPE [ 'b9ad' ] ->GEOSHAPE GEO.DIFFERENCE SIZE",
            Ok(r"[45]"),
        ),
        (
            r"[ 'b' ] ->GEOSHAPE [ 'b9ad1ee0' ] ->GEOSHAPE GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b0","b1","b2","b3","b4","b5","b6","b7","b8","ba","bb","bc","bd","be","bf","b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (
            r"@M [ 'b9ad1e' 'f1' 'a' ] ->GEOSHAPE GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b8","f0","f2","f3","f4","f5","f6","f7","f8","f9","fa","fb","fc","fd","fe","ff","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f"]]"#,
            ),
        ),
        (
            r"@D [ 'b9ad1ee0' ] ->GEOSHAPE GEO.DIFFERENCE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"[ 'b9ad' ] ->GEOSHAPE [ 'b9ad1ee0' ] ->GEOSHAPE GEO.DIFFERENCE T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (
            r"@A 1 GEO.DIFFERENCE",
            Err(
                r"GEO.DIFFERENCE expects two GeoShape instances as the top 2 elements of the stack.",
            ),
        ),
        (
            r"1 @A GEO.DIFFERENCE",
            Err(
                r"GEO.DIFFERENCE expects two GeoShape instances as the top 2 elements of the stack.",
            ),
        ),
        (r"1 GEO.DIFFERENCE", Err(r"Empty stack.")),
        (r"GEO.DIFFERENCE", Err(r"Empty stack.")),
        (r"@A GEO.DIFFERENCE", Err(r"Empty stack.")),
        (r"[ @A @C ] GEO.DIFFERENCE", Err(r"Empty stack.")),
        (
            r"@A NULL GEO.DIFFERENCE",
            Err(
                r"GEO.DIFFERENCE expects two GeoShape instances as the top 2 elements of the stack.",
            ),
        ),
        (
            r"NULL @A GEO.DIFFERENCE",
            Err(
                r"GEO.DIFFERENCE expects two GeoShape instances as the top 2 elements of the stack.",
            ),
        ),
    ]);
}

#[test]
fn intersects_1() {
    check(&[
        (r"@A @A GEO.INTERSECTS", Ok(r"[true]")),
        (r"@A @C GEO.INTERSECTS", Ok(r"[false]")),
        (r"@A @P GEO.INTERSECTS", Ok(r"[true]")),
        (r"@A @PA GEO.INTERSECTS", Ok(r"[true]")),
        (r"@A @K GEO.INTERSECTS", Ok(r"[true]")),
        (r"@A @K15 GEO.INTERSECTS", Ok(r"[true]")),
        (r"@A @M GEO.INTERSECTS", Ok(r"[true]")),
        (r"@A @D GEO.INTERSECTS", Ok(r"[true]")),
        (r"@A @E GEO.INTERSECTS", Ok(r"[false]")),
        (r"@C @A GEO.INTERSECTS", Ok(r"[false]")),
        (r"@C @C GEO.INTERSECTS", Ok(r"[true]")),
        (r"@C @P GEO.INTERSECTS", Ok(r"[true]")),
        (r"@C @PA GEO.INTERSECTS", Ok(r"[true]")),
        (r"@C @K GEO.INTERSECTS", Ok(r"[true]")),
        (r"@C @K15 GEO.INTERSECTS", Ok(r"[true]")),
        (r"@C @M GEO.INTERSECTS", Ok(r"[false]")),
        (r"@C @D GEO.INTERSECTS", Ok(r"[false]")),
        (r"@C @E GEO.INTERSECTS", Ok(r"[false]")),
        (r"@P @A GEO.INTERSECTS", Ok(r"[true]")),
        (r"@P @C GEO.INTERSECTS", Ok(r"[true]")),
        (r"@P @P GEO.INTERSECTS", Ok(r"[true]")),
        (r"@P @PA GEO.INTERSECTS", Ok(r"[true]")),
        (r"@P @K GEO.INTERSECTS", Ok(r"[true]")),
        (r"@P @K15 GEO.INTERSECTS", Ok(r"[true]")),
        (r"@P @M GEO.INTERSECTS", Ok(r"[true]")),
        (r"@P @D GEO.INTERSECTS", Ok(r"[true]")),
        (r"@P @E GEO.INTERSECTS", Ok(r"[false]")),
        (r"@PA @A GEO.INTERSECTS", Ok(r"[true]")),
        (r"@PA @C GEO.INTERSECTS", Ok(r"[true]")),
        (r"@PA @P GEO.INTERSECTS", Ok(r"[true]")),
        (r"@PA @PA GEO.INTERSECTS", Ok(r"[true]")),
        (r"@PA @K GEO.INTERSECTS", Ok(r"[true]")),
        (r"@PA @K15 GEO.INTERSECTS", Ok(r"[true]")),
        (r"@PA @M GEO.INTERSECTS", Ok(r"[true]")),
        (r"@PA @D GEO.INTERSECTS", Ok(r"[true]")),
        (r"@PA @E GEO.INTERSECTS", Ok(r"[false]")),
        (r"@K @A GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K @C GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K @P GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K @PA GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K @K GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K @K15 GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K @M GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K @D GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K @E GEO.INTERSECTS", Ok(r"[false]")),
        (r"@K15 @A GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K15 @C GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K15 @P GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K15 @PA GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K15 @K GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K15 @K15 GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K15 @M GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K15 @D GEO.INTERSECTS", Ok(r"[true]")),
        (r"@K15 @E GEO.INTERSECTS", Ok(r"[false]")),
        (r"@M @A GEO.INTERSECTS", Ok(r"[true]")),
        (r"@M @C GEO.INTERSECTS", Ok(r"[false]")),
        (r"@M @P GEO.INTERSECTS", Ok(r"[true]")),
        (r"@M @PA GEO.INTERSECTS", Ok(r"[true]")),
        (r"@M @K GEO.INTERSECTS", Ok(r"[true]")),
        (r"@M @K15 GEO.INTERSECTS", Ok(r"[true]")),
        (r"@M @M GEO.INTERSECTS", Ok(r"[true]")),
        (r"@M @D GEO.INTERSECTS", Ok(r"[true]")),
        (r"@M @E GEO.INTERSECTS", Ok(r"[false]")),
        (r"@D @A GEO.INTERSECTS", Ok(r"[true]")),
        (r"@D @C GEO.INTERSECTS", Ok(r"[false]")),
        (r"@D @P GEO.INTERSECTS", Ok(r"[true]")),
        (r"@D @PA GEO.INTERSECTS", Ok(r"[true]")),
        (r"@D @K GEO.INTERSECTS", Ok(r"[true]")),
        (r"@D @K15 GEO.INTERSECTS", Ok(r"[true]")),
        (r"@D @M GEO.INTERSECTS", Ok(r"[true]")),
        (r"@D @D GEO.INTERSECTS", Ok(r"[true]")),
        (r"@D @E GEO.INTERSECTS", Ok(r"[false]")),
        (r"@E @A GEO.INTERSECTS", Ok(r"[false]")),
        (r"@E @C GEO.INTERSECTS", Ok(r"[false]")),
        (r"@E @P GEO.INTERSECTS", Ok(r"[false]")),
        (r"@E @PA GEO.INTERSECTS", Ok(r"[false]")),
        (r"@E @K GEO.INTERSECTS", Ok(r"[false]")),
        (r"@E @K15 GEO.INTERSECTS", Ok(r"[false]")),
        (r"@E @M GEO.INTERSECTS", Ok(r"[false]")),
        (r"@E @D GEO.INTERSECTS", Ok(r"[false]")),
        (r"@E @E GEO.INTERSECTS", Ok(r"[false]")),
        (r"1 @A @C GEO.INTERSECTS", Ok(r"[false,1]")),
        (
            r"@A 1 GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS expects a geo shape on top of the stack."),
        ),
        (
            r"1 @A GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS can only operate on Geo Time Series instances."),
        ),
        (r"@A GEO.INTERSECTS", Err(r"Empty stack.")),
        (r"GEO.INTERSECTS", Err(r"Empty stack.")),
        (
            r"'a' GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS expects a geo shape on top of the stack."),
        ),
        (
            r"@A [ 1 ] GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS expects a geo shape on top of the stack."),
        ),
        (
            r"@A [ NEWGTS ] GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS expects a geo shape on top of the stack."),
        ),
        (
            r"@A [ ] GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS expects a geo shape on top of the stack."),
        ),
        (
            r"@A NULL GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS expects a geo shape on top of the stack."),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE @A GEO.INTERSECTS",
            Ok(r"[false]"),
        ),
        (
            r"@A NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE SWAP GEO.INTERSECTS",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE @A GEO.INTERSECTS",
            Ok(r"[false]"),
        ),
        (r"NEWGTS @A GEO.INTERSECTS", Ok(r"[false]")),
        (
            r"[ NEWGTS 1 -10.0 -10.0 NaN 1 ADDVALUE NEWGTS 2 48.85 2.35 NaN 1 ADDVALUE ] @P GEO.INTERSECTS",
            Ok(r"[[false,false]]"),
        ),
        (
            r"[ NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE NEWGTS 2 NaN NaN NaN 1 ADDVALUE ] @P GEO.INTERSECTS",
            Ok(r"[[false,false]]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE 2 -10.0 -10.0 NaN 2 ADDVALUE 3 NaN NaN NaN 3 ADDVALUE [ 'b' 'f' ] ->GEOSHAPE GEO.INTERSECTS",
            Ok(r"[false]"),
        ),
        (
            r"[ NEWGTS 1 ] [ 'b9ad' ] ->GEOSHAPE GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS can only operate on Geo Time Series instances."),
        ),
        (
            r"[ NEWGTS 'a' ] @A GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS can only operate on Geo Time Series instances."),
        ),
    ]);
}

#[test]
fn intersects_2() {
    check(&[
        (
            r"[ 'a' ] @A GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS can only operate on Geo Time Series instances."),
        ),
        (
            r"'a' @A GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS can only operate on Geo Time Series instances."),
        ),
        (
            r"1 @A GEO.INTERSECTS",
            Err(r"GEO.INTERSECTS can only operate on Geo Time Series instances."),
        ),
        (r"@A @A GEO.INTERSECTS", Ok(r"[true]")),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE [ 48.85 2.35 ->GEOHASH ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE [ 48.85 2.35 ->GEOHASH 0 5 SUBSTRING ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 48.86 2.36 NaN 1 ADDVALUE [ 48.86 2.36 ->GEOHASH ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 48.86 2.36 NaN 1 ADDVALUE [ 48.86 2.36 ->GEOHASH 0 5 SUBSTRING ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 0.0 0.0 NaN 1 ADDVALUE [ 0.0 0.0 ->GEOHASH ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 0.0 0.0 NaN 1 ADDVALUE [ 0.0 0.0 ->GEOHASH 0 5 SUBSTRING ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 -33.9 151.2 NaN 1 ADDVALUE [ -33.9 151.2 ->GEOHASH ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 -33.9 151.2 NaN 1 ADDVALUE [ -33.9 151.2 ->GEOHASH 0 5 SUBSTRING ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 89.99 -179.99 NaN 1 ADDVALUE [ 89.99 -179.99 ->GEOHASH ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 89.99 -179.99 NaN 1 ADDVALUE [ 89.99 -179.99 ->GEOHASH 0 5 SUBSTRING ] GEOHASH-> GEO.INTERSECTS",
            Ok(r"[true]"),
        ),
    ]);
}

#[test]
fn within() {
    check(&[
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE @A GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE 2 -10.0 -10.0 NaN 2 ADDVALUE @A GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 48.85 2.35 NaN 2 ADDVALUE @A GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE @A GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (r"NEWGTS @A GEO.WITHIN", Ok(r"[false]")),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE @P GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE 2 -10.0 -10.0 NaN 2 ADDVALUE @P GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 48.85 2.35 NaN 2 ADDVALUE @P GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE @P GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (r"NEWGTS @P GEO.WITHIN", Ok(r"[false]")),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE @K GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE 2 -10.0 -10.0 NaN 2 ADDVALUE @K GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 48.85 2.35 NaN 2 ADDVALUE @K GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE @K GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (r"NEWGTS @K GEO.WITHIN", Ok(r"[false]")),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE @M GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE 2 -10.0 -10.0 NaN 2 ADDVALUE @M GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 48.85 2.35 NaN 2 ADDVALUE @M GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE @M GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (r"NEWGTS @M GEO.WITHIN", Ok(r"[false]")),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE @D GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE 2 -10.0 -10.0 NaN 2 ADDVALUE @D GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 48.85 2.35 NaN 2 ADDVALUE @D GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE @D GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (r"NEWGTS @D GEO.WITHIN", Ok(r"[false]")),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE @E GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE 2 -10.0 -10.0 NaN 2 ADDVALUE @E GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 48.85 2.35 NaN 2 ADDVALUE @E GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 1 ADDVALUE @E GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (r"NEWGTS @E GEO.WITHIN", Ok(r"[false]")),
        (
            r"[ NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE NEWGTS 2 -10.0 -10.0 NaN 1 ADDVALUE ] @P GEO.WITHIN",
            Ok(r"[[false,false]]"),
        ),
        (r"[ ] @P GEO.WITHIN", Ok(r"[[]]")),
        (r"@P GEO.WITHIN", Err(r"Empty stack.")),
        (r"GEO.WITHIN", Err(r"Empty stack.")),
        (
            r"NEWGTS 1 ADDVALUE 1 GEO.WITHIN",
            Err(r"ADDVALUE expects the elevation to be numeric or NaN."),
        ),
        (
            r"NEWGTS 1 GEO.WITHIN",
            Err(r"GEO.WITHIN expects a geo shape on top of the stack."),
        ),
        (
            r"[ NEWGTS 1 ] @P GEO.WITHIN",
            Err(r"GEO.WITHIN can only operate on Geo Time Series instances."),
        ),
        (
            r"[ NEWGTS 'a' ] @P GEO.WITHIN",
            Err(r"GEO.WITHIN can only operate on Geo Time Series instances."),
        ),
        (
            r"'a' @P GEO.WITHIN",
            Err(r"GEO.WITHIN can only operate on Geo Time Series instances."),
        ),
        (
            r"@P @P GEO.WITHIN",
            Err(r"GEO.WITHIN can only operate on Geo Time Series instances."),
        ),
        (
            r"NEWGTS @P NEWGTS GEO.WITHIN",
            Err(r"GEO.WITHIN expects a geo shape on top of the stack."),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 48.85 2.35 NaN 3 ADDVALUE @P GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 90.0 180.0 NaN 1 ADDVALUE @K GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 -90.0 -180.0 NaN 1 ADDVALUE [ '0' ] ->GEOSHAPE GEO.WITHIN",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 0.0 0.0 NaN 1 ADDVALUE [ 'c' '9' '6' '3' ] ->GEOSHAPE GEO.WITHIN",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE [ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE GEO.WITHIN",
            Ok(r"[false]"),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE 48.85 2.35 ->HHCODE 'x' SPLIT DROP DROP [ ] ->GEOSHAPE GEO.WITHIN",
            Err(r"Empty stack."),
        ),
        (
            r"NEWGTS 1 48.85 2.35 NaN 1 ADDVALUE [ 48.85 2.35 ->GEOHASH ] GEOHASH-> GEO.WITHIN",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 48.86 2.36 NaN 1 ADDVALUE [ 48.86 2.36 ->GEOHASH ] GEOHASH-> GEO.WITHIN",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 0.0 0.0 NaN 1 ADDVALUE [ 0.0 0.0 ->GEOHASH ] GEOHASH-> GEO.WITHIN",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 -33.9 151.2 NaN 1 ADDVALUE [ -33.9 151.2 ->GEOHASH ] GEOHASH-> GEO.WITHIN",
            Ok(r"[true]"),
        ),
        (
            r"NEWGTS 1 89.99 -179.99 NaN 1 ADDVALUE [ 89.99 -179.99 ->GEOHASH ] GEOHASH-> GEO.WITHIN",
            Ok(r"[true]"),
        ),
    ]);
}

#[test]
fn optimize() {
    check(&[
        (r"@A 0 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 2 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b"]]"#)),
        (r"@A 4 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@A 6 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@A 8 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 30 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@C 0 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 2 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b"]]"#)),
        (r"@C 4 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@C 6 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@C 8 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 30 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@P 0 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 2 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b"]]"#)),
        (r"@P 4 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 6 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 8 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 30 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@PA 0 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@PA 2 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b"]]"#)),
        (r"@PA 4 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@PA 6 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@PA 8 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@PA 30 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 0 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 2 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b"]]"#)),
        (r"@K 4 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@K 6 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 8 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 30 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K15 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (r"@K15 2 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b"]]"#)),
        (r"@K15 4 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@K15 6 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K15 8 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 30 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@M 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (r"@M 2 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["a","b","f"]]"#)),
        (
            r"@M 4 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9"]]"#),
        ),
        (
            r"@M 6 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9a"]]"#),
        ),
        (
            r"@M 8 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 30 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@D 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (r"@D 2 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b","f"]]"#)),
        (r"@D 4 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9","f0"]]"#)),
        (r"@D 6 GEO.OPTIMIZE T GEOSHAPE->", Ok(r#"[["b9a","f01"]]"#)),
        (
            r"@D 8 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad","f012"]]"#),
        ),
        (
            r"@D 30 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (r"@E 0 GEO.OPTIMIZE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E 2 GEO.OPTIMIZE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E 4 GEO.OPTIMIZE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E 6 GEO.OPTIMIZE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E 8 GEO.OPTIMIZE T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E 30 GEO.OPTIMIZE T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' 'b9ad1ee0' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9a"]]"#),
        ),
        (
            r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' 'b9ad1ee0' ] ->GEOSHAPE 4 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"[ 'b9ad1ee0' 'b9ad1ee1' 'b9ad1ee2' ] ->GEOSHAPE 6 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9a"]]"#),
        ),
        (
            r"[ 'b9ad1ee0' 'b9ad1ee1' 'b9ad1ee2' ] ->GEOSHAPE 4 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"[ 'b9ad1ee0' 'b9ad1ee1' 'b9ad1ee2' ] ->GEOSHAPE 8 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"[ 'b9ad1ee0' 'b9ad1ef0' 'b9ae1ef0' ] ->GEOSHAPE 4 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"[ 'b9ad1ee0' 'b9ad1ef0' 'b9ae1ef0' ] ->GEOSHAPE 2 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b"]]"#),
        ),
        (
            r"[ 'b9ad1ee0' 'b9ad1ef0' 'b9ae1ef0' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ef0","b9ae1ef0"]]"#),
        ),
        (
            r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9b0' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9a","b9b0"]]"#),
        ),
        (
            r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' 'b9b0' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af","b9b0"]]"#,
            ),
        ),
        (
            r"[ 'a9a0' 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9a","a9a0"]]"#),
        ),
        (
            r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' 'b9b0' 'b9b1' 'b9b2' 'b9b3' 'b9b4' 'b9b5' 'b9b6' 'b9b7' 'b9b8' 'b9b9' 'b9ba' 'b9bb' 'b9bc' 'b9bd' 'b9be' 'b9bf' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9a","b9b"]]"#),
        ),
        (
            r"[ 'b9a00' 'b9a01' 'b9a02' 'b9a03' 'b9a04' 'b9a05' 'b9a06' 'b9a07' 'b9a08' 'b9a09' 'b9a0a' 'b9a0b' 'b9a0c' 'b9a0d' 'b9a0e' 'b9a0f' 'b9a10' 'b9a11' 'b9a12' 'b9a13' 'b9a14' 'b9a15' 'b9a16' 'b9a17' 'b9a18' 'b9a19' 'b9a1a' 'b9a1b' 'b9a1c' 'b9a1d' 'b9a1e' 'b9a1f' 'b9a20' 'b9a21' 'b9a22' 'b9a23' 'b9a24' 'b9a25' 'b9a26' 'b9a27' 'b9a28' 'b9a29' 'b9a2a' 'b9a2b' 'b9a2c' 'b9a2d' 'b9a2e' 'b9a2f' 'b9a30' 'b9a31' 'b9a32' 'b9a33' 'b9a34' 'b9a35' 'b9a36' 'b9a37' 'b9a38' 'b9a39' 'b9a3a' 'b9a3b' 'b9a3c' 'b9a3d' 'b9a3e' 'b9a3f' 'b9a40' 'b9a41' 'b9a42' 'b9a43' 'b9a44' 'b9a45' 'b9a46' 'b9a47' 'b9a48' 'b9a49' 'b9a4a' 'b9a4b' 'b9a4c' 'b9a4d' 'b9a4e' 'b9a4f' 'b9a50' 'b9a51' 'b9a52' 'b9a53' 'b9a54' 'b9a55' 'b9a56' 'b9a57' 'b9a58' 'b9a59' 'b9a5a' 'b9a5b' 'b9a5c' 'b9a5d' 'b9a5e' 'b9a5f' 'b9a60' 'b9a61' 'b9a62' 'b9a63' 'b9a64' 'b9a65' 'b9a66' 'b9a67' 'b9a68' 'b9a69' 'b9a6a' 'b9a6b' 'b9a6c' 'b9a6d' 'b9a6e' 'b9a6f' 'b9a70' 'b9a71' 'b9a72' 'b9a73' 'b9a74' 'b9a75' 'b9a76' 'b9a77' 'b9a78' 'b9a79' 'b9a7a' 'b9a7b' 'b9a7c' 'b9a7d' 'b9a7e' 'b9a7f' 'b9a80' 'b9a81' 'b9a82' 'b9a83' 'b9a84' 'b9a85' 'b9a86' 'b9a87' 'b9a88' 'b9a89' 'b9a8a' 'b9a8b' 'b9a8c' 'b9a8d' 'b9a8e' 'b9a8f' 'b9a90' 'b9a91' 'b9a92' 'b9a93' 'b9a94' 'b9a95' 'b9a96' 'b9a97' 'b9a98' 'b9a99' 'b9a9a' 'b9a9b' 'b9a9c' 'b9a9d' 'b9a9e' 'b9a9f' 'b9aa0' 'b9aa1' 'b9aa2' 'b9aa3' 'b9aa4' 'b9aa5' 'b9aa6' 'b9aa7' 'b9aa8' 'b9aa9' 'b9aaa' 'b9aab' 'b9aac' 'b9aad' 'b9aae' 'b9aaf' 'b9ab0' 'b9ab1' 'b9ab2' 'b9ab3' 'b9ab4' 'b9ab5' 'b9ab6' 'b9ab7' 'b9ab8' 'b9ab9' 'b9aba' 'b9abb' 'b9abc' 'b9abd' 'b9abe' 'b9abf' 'b9ac0' 'b9ac1' 'b9ac2' 'b9ac3' 'b9ac4' 'b9ac5' 'b9ac6' 'b9ac7' 'b9ac8' 'b9ac9' 'b9aca' 'b9acb' 'b9acc' 'b9acd' 'b9ace' 'b9acf' 'b9ad0' 'b9ad1' 'b9ad2' 'b9ad3' 'b9ad4' 'b9ad5' 'b9ad6' 'b9ad7' 'b9ad8' 'b9ad9' 'b9ada' 'b9adb' 'b9adc' 'b9add' 'b9ade' 'b9adf' 'b9ae0' 'b9ae1' 'b9ae2' 'b9ae3' 'b9ae4' 'b9ae5' 'b9ae6' 'b9ae7' 'b9ae8' 'b9ae9' 'b9aea' 'b9aeb' 'b9aec' 'b9aed' 'b9aee' 'b9aef' 'b9af0' 'b9af1' 'b9af2' 'b9af3' 'b9af4' 'b9af5' 'b9af6' 'b9af7' 'b9af8' 'b9af9' 'b9afa' 'b9afb' 'b9afc' 'b9afd' 'b9afe' 'b9aff' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9a"]]"#),
        ),
        (
            r"[ 'b9a00' 'b9a01' 'b9a02' 'b9a03' 'b9a04' 'b9a05' 'b9a06' 'b9a07' 'b9a08' 'b9a09' 'b9a0a' 'b9a0b' 'b9a0c' 'b9a0d' 'b9a0e' 'b9a0f' 'b9a10' 'b9a11' 'b9a12' 'b9a13' 'b9a14' 'b9a15' 'b9a16' 'b9a17' 'b9a18' 'b9a19' 'b9a1a' 'b9a1b' 'b9a1c' 'b9a1d' 'b9a1e' 'b9a1f' 'b9a20' 'b9a21' 'b9a22' 'b9a23' 'b9a24' 'b9a25' 'b9a26' 'b9a27' 'b9a28' 'b9a29' 'b9a2a' 'b9a2b' 'b9a2c' 'b9a2d' 'b9a2e' 'b9a2f' 'b9a30' 'b9a31' 'b9a32' 'b9a33' 'b9a34' 'b9a35' 'b9a36' 'b9a37' 'b9a38' 'b9a39' 'b9a3a' 'b9a3b' 'b9a3c' 'b9a3d' 'b9a3e' 'b9a3f' 'b9a40' 'b9a41' 'b9a42' 'b9a43' 'b9a44' 'b9a45' 'b9a46' 'b9a47' 'b9a48' 'b9a49' 'b9a4a' 'b9a4b' 'b9a4c' 'b9a4d' 'b9a4e' 'b9a4f' 'b9a50' 'b9a51' 'b9a52' 'b9a53' 'b9a54' 'b9a55' 'b9a56' 'b9a57' 'b9a58' 'b9a59' 'b9a5a' 'b9a5b' 'b9a5c' 'b9a5d' 'b9a5e' 'b9a5f' 'b9a60' 'b9a61' 'b9a62' 'b9a63' 'b9a64' 'b9a65' 'b9a66' 'b9a67' 'b9a68' 'b9a69' 'b9a6a' 'b9a6b' 'b9a6c' 'b9a6d' 'b9a6e' 'b9a6f' 'b9a70' 'b9a71' 'b9a72' 'b9a73' 'b9a74' 'b9a75' 'b9a76' 'b9a77' 'b9a78' 'b9a79' 'b9a7a' 'b9a7b' 'b9a7c' 'b9a7d' 'b9a7e' 'b9a7f' 'b9a80' 'b9a81' 'b9a82' 'b9a83' 'b9a84' 'b9a85' 'b9a86' 'b9a87' 'b9a88' 'b9a89' 'b9a8a' 'b9a8b' 'b9a8c' 'b9a8d' 'b9a8e' 'b9a8f' 'b9a90' 'b9a91' 'b9a92' 'b9a93' 'b9a94' 'b9a95' 'b9a96' 'b9a97' 'b9a98' 'b9a99' 'b9a9a' 'b9a9b' 'b9a9c' 'b9a9d' 'b9a9e' 'b9a9f' 'b9aa0' 'b9aa1' 'b9aa2' 'b9aa3' 'b9aa4' 'b9aa5' 'b9aa6' 'b9aa7' 'b9aa8' 'b9aa9' 'b9aaa' 'b9aab' 'b9aac' 'b9aad' 'b9aae' 'b9aaf' 'b9ab0' 'b9ab1' 'b9ab2' 'b9ab3' 'b9ab4' 'b9ab5' 'b9ab6' 'b9ab7' 'b9ab8' 'b9ab9' 'b9aba' 'b9abb' 'b9abc' 'b9abd' 'b9abe' 'b9abf' 'b9ac0' 'b9ac1' 'b9ac2' 'b9ac3' 'b9ac4' 'b9ac5' 'b9ac6' 'b9ac7' 'b9ac8' 'b9ac9' 'b9aca' 'b9acb' 'b9acc' 'b9acd' 'b9ace' 'b9acf' 'b9ad0' 'b9ad1' 'b9ad2' 'b9ad3' 'b9ad4' 'b9ad5' 'b9ad6' 'b9ad7' 'b9ad8' 'b9ad9' 'b9ada' 'b9adb' 'b9adc' 'b9add' 'b9ade' 'b9adf' 'b9ae0' 'b9ae1' 'b9ae2' 'b9ae3' 'b9ae4' 'b9ae5' 'b9ae6' 'b9ae7' 'b9ae8' 'b9ae9' 'b9aea' 'b9aeb' 'b9aec' 'b9aed' 'b9aee' 'b9aef' 'b9af0' 'b9af1' 'b9af2' 'b9af3' 'b9af4' 'b9af5' 'b9af6' 'b9af7' 'b9af8' 'b9af9' 'b9afa' 'b9afb' 'b9afc' 'b9afd' 'b9afe' 'b9aff' ] ->GEOSHAPE 4 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"[ 'b9' 'b9a' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9","b9a"]]"#),
        ),
        (
            r"[ 'b9' 'b9ad' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"[ 'b9' 'b9ad1e' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9","b9ad1e"]]"#),
        ),
        (
            r"[ 'b9ad' 'b9ad1e' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"[ 'b9ad' 'b9ad1ee0' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad"]]"#),
        ),
        (
            r"[ 'b9ad' 'b9ad1ee0' ] ->GEOSHAPE 4 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"[ 'b9ad' 'b9ad1ee0' ] ->GEOSHAPE 2 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b"]]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 4 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 0 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0c5a9e7"]]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 30 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0c5a9e7"]]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 2 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b"]]"#),
        ),
        (
            r"@A 1 GEO.OPTIMIZE",
            Err(
                r"GEO.OPTIMIZE expects a maximum resolution which is an even number between 2 and 30 or 0.",
            ),
        ),
        (
            r"@A 3 GEO.OPTIMIZE",
            Err(
                r"GEO.OPTIMIZE expects a maximum resolution which is an even number between 2 and 30 or 0.",
            ),
        ),
        (
            r"@A 31 GEO.OPTIMIZE",
            Err(
                r"GEO.OPTIMIZE expects a maximum resolution which is an even number between 2 and 30 or 0.",
            ),
        ),
        (
            r"@A 32 GEO.OPTIMIZE",
            Err(
                r"GEO.OPTIMIZE expects a maximum resolution which is an even number between 2 and 30 or 0.",
            ),
        ),
        (
            r"@A -2 GEO.OPTIMIZE",
            Err(
                r"GEO.OPTIMIZE expects a maximum resolution which is an even number between 2 and 30 or 0.",
            ),
        ),
        (
            r"@A -1 GEO.OPTIMIZE",
            Err(
                r"GEO.OPTIMIZE expects a maximum resolution which is an even number between 2 and 30 or 0.",
            ),
        ),
        (
            r"@A 4294967300 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"@A 4294967296 GEO.OPTIMIZE T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@A 4294967297 GEO.OPTIMIZE",
            Err(
                r"GEO.OPTIMIZE expects a maximum resolution which is an even number between 2 and 30 or 0.",
            ),
        ),
        (
            r"@A 'a' GEO.OPTIMIZE",
            Err(r"GEO.OPTIMIZE expects a maximum resolution on top of the stack."),
        ),
        (
            r"@A 4.0 GEO.OPTIMIZE",
            Err(r"GEO.OPTIMIZE expects a maximum resolution on top of the stack."),
        ),
        (
            r"@A GEO.OPTIMIZE",
            Err(r"GEO.OPTIMIZE expects a maximum resolution on top of the stack."),
        ),
        (r"4 GEO.OPTIMIZE", Err(r"Empty stack.")),
        (
            r"1 4 GEO.OPTIMIZE",
            Err(r"GEO.OPTIMIZE operates on a shape."),
        ),
        (
            r"'a' 4 GEO.OPTIMIZE",
            Err(r"GEO.OPTIMIZE operates on a shape."),
        ),
        (
            r"NULL 4 GEO.OPTIMIZE",
            Err(r"GEO.OPTIMIZE operates on a shape."),
        ),
        (
            r"[ @A ] 4 GEO.OPTIMIZE",
            Err(r"GEO.OPTIMIZE operates on a shape."),
        ),
        (r"GEO.OPTIMIZE", Err(r"Empty stack.")),
        (
            r"@A NULL GEO.OPTIMIZE",
            Err(r"GEO.OPTIMIZE expects a maximum resolution on top of the stack."),
        ),
        (
            r"@A T GEO.OPTIMIZE",
            Err(r"GEO.OPTIMIZE expects a maximum resolution on top of the stack."),
        ),
    ]);
}

#[test]
fn normalize() {
    check(&[
        (
            r"@A @A GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ad"],["b9ad"]]"#),
        ),
        (
            r"@A @C GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ad"],["b9ae"]]"#),
        ),
        (
            r"@A @P GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad"],["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@A @PA GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@A @K GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@A @K15 GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@A @M GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ad"],["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A @D GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"],["b9ad1ee0","b9ad1ee1","f01234567"]]"#,
            ),
        ),
        (
            r"@A @E GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ad"],[]]"#),
        ),
        (
            r"@C @A GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ae"],["b9ad"]]"#),
        ),
        (
            r"@C @C GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ae"],["b9ae"]]"#),
        ),
        (
            r"@C @P GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ae"],["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@C @PA GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ae"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@C @K GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ae"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@C @K15 GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ae"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@C @M GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ae"],["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@C @D GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ae"],["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@C @E GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ae"],[]]"#),
        ),
        (
            r"@P @A GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9ad"]]"#,
            ),
        ),
        (
            r"@P @C GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9ae"]]"#,
            ),
        ),
        (
            r"@P @P GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9"],["b9"]]"#),
        ),
        (
            r"@P @PA GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9a","b9b","b9c","b9d","b9e","b9f"],["b9a"]]"#,
            ),
        ),
        (
            r"@P @K GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@P @K15 GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@P @M GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["f","a1","b8","b9ad"]]"#,
            ),
        ),
        (
            r"@P @D GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"],["b9ad1ee0","b9ad1ee1","f01234567"]]"#,
            ),
        ),
        (
            r"@P @E GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9"],[]]"#),
        ),
        (
            r"@PA @A GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9ad"]]"#,
            ),
        ),
        (
            r"@PA @C GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9ae"]]"#,
            ),
        ),
        (
            r"@PA @P GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a"],["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9a","b9b","b9c","b9d","b9e","b9f"]]"#,
            ),
        ),
        (
            r"@PA @PA GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9a"],["b9a"]]"#),
        ),
        (
            r"@PA @K GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@PA @K15 GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@PA @M GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["f","a1","b8","b9ad"]]"#,
            ),
        ),
        (
            r"@PA @D GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"],["b9ad1ee0","b9ad1ee1","f01234567"]]"#,
            ),
        ),
        (
            r"@PA @E GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9a"],[]]"#),
        ),
        (
            r"@K @A GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9ad"]]"#,
            ),
        ),
        (
            r"@K @C GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9ae"]]"#,
            ),
        ),
        (
            r"@K @P GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K @PA GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K @K GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K @K15 GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K @M GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],["f","a1","b8","b9ad"]]"#,
            ),
        ),
        (
            r"@K @D GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"],["b9ad1ee0","b9ad1ee1","f01234567"]]"#,
            ),
        ),
        (
            r"@K @E GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"],[]]"#,
            ),
        ),
        (
            r"@K15 @A GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"],["b9ad"]]"#,
            ),
        ),
        (
            r"@K15 @C GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"],["b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @P GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"],["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K15 @PA GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K15 @K GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@K15 @K15 GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@K15 @M GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"],["f","a1","b8","b9ad"]]"#,
            ),
        ),
        (
            r"@K15 @D GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"],["b9ad1ee0","b9ad1ee1","f01234567"]]"#,
            ),
        ),
        (
            r"@K15 @E GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"],[]]"#,
            ),
        ),
        (
            r"@M @A GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"],["b9ad"]]"#),
        ),
        (
            r"@M @C GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"],["b9ae"]]"#),
        ),
        (
            r"@M @P GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["f","a1","b8","b9ad"],["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@M @PA GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["f","a1","b8","b9ad"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@M @K GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["f","a1","b8","b9ad"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@M @K15 GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["f","a1","b8","b9ad"],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@M @M GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"],["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M @D GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","f0123450","f0123451","f0123452","f0123453","f0123454","f0123455","f0123457","f0123458","f0123459","f012345a","f012345b","f012345c","f012345d","f012345e","f012345f","f01234560","f01234561","f01234562","f01234563","f01234564","f01234565","f01234566","f01234567","f01234568","f01234569","f0123456a","f0123456b","f0123456c","f0123456d","f0123456e","f0123456f","a1","b8","f1","f2","f3","f4","f5","f6","f7","f8","f9","fa","fb","fc","fd","fe","ff","f00","f02","f03","f04","f05","f06","f07","f08","f09","f0a","f0b","f0c","f0d","f0e","f0f","f010","f011","f013","f014","f015","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","f0120","f0121","f0122","f0124","f0125","f0126","f0127","f0128","f0129","f012a","f012b","f012c","f012d","f012e","f012f","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","f01230","f01231","f01232","f01233","f01235","f01236","f01237","f01238","f01239","f0123a","f0123b","f0123c","f0123d","f0123e","f0123f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef","f012340","f012341","f012342","f012343","f012344","f012346","f012347","f012348","f012349","f01234a","f01234b","f01234c","f01234d","f01234e","f01234f"],["b9ad1ee0","b9ad1ee1","f01234567"]]"#,
            ),
        ),
        (
            r"@M @E GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"],[]]"#),
        ),
        (
            r"@D @A GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","f01234567"],["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (
            r"@D @C GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"],["b9ae"]]"#),
        ),
        (
            r"@D @P GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","f01234567"],["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (
            r"@D @PA GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","f01234567"],["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (
            r"@D @K GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","f01234567"],["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (
            r"@D @K15 GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","f01234567"],["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef"]]"#,
            ),
        ),
        (
            r"@D @M GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[["b9ad1ee0","b9ad1ee1","f01234567"],["b9ad1ee0","b9ad1ee1","b9ad1ee2","b9ad1ee3","b9ad1ee4","b9ad1ee5","b9ad1ee6","b9ad1ee7","b9ad1ee8","b9ad1ee9","b9ad1eea","b9ad1eeb","b9ad1eec","b9ad1eed","b9ad1eee","b9ad1eef","f0123450","f0123451","f0123452","f0123453","f0123454","f0123455","f0123457","f0123458","f0123459","f012345a","f012345b","f012345c","f012345d","f012345e","f012345f","f01234560","f01234561","f01234562","f01234563","f01234564","f01234565","f01234566","f01234567","f01234568","f01234569","f0123456a","f0123456b","f0123456c","f0123456d","f0123456e","f0123456f","a1","b8","f1","f2","f3","f4","f5","f6","f7","f8","f9","fa","fb","fc","fd","fe","ff","f00","f02","f03","f04","f05","f06","f07","f08","f09","f0a","f0b","f0c","f0d","f0e","f0f","f010","f011","f013","f014","f015","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","f0120","f0121","f0122","f0124","f0125","f0126","f0127","f0128","f0129","f012a","f012b","f012c","f012d","f012e","f012f","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1f","f01230","f01231","f01232","f01233","f01235","f01236","f01237","f01238","f01239","f0123a","f0123b","f0123c","f0123d","f0123e","f0123f","b9ad1e0","b9ad1e1","b9ad1e2","b9ad1e3","b9ad1e4","b9ad1e5","b9ad1e6","b9ad1e7","b9ad1e8","b9ad1e9","b9ad1ea","b9ad1eb","b9ad1ec","b9ad1ed","b9ad1ef","f012340","f012341","f012342","f012343","f012344","f012346","f012347","f012348","f012349","f01234a","f01234b","f01234c","f01234d","f01234e","f01234f"]]"#,
            ),
        ),
        (
            r"@D @D GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"],["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D @E GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"],[]]"#),
        ),
        (
            r"@E @A GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[[],["b9ad"]]"#),
        ),
        (
            r"@E @C GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[[],["b9ae"]]"#),
        ),
        (
            r"@E @P GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[[],["b9"]]"#),
        ),
        (
            r"@E @PA GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[[],["b9a"]]"#),
        ),
        (
            r"@E @K GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[[],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae","b9af"]]"#,
            ),
        ),
        (
            r"@E @K15 GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(
                r#"[[],["b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ad","b9ae"]]"#,
            ),
        ),
        (
            r"@E @M GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[[],["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@E @D GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r#"[[],["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@E @E GEO.NORMALIZE T GEOSHAPE-> SWAP T GEOSHAPE->",
            Ok(r"[[],[]]"),
        ),
        (
            r"@P [ 'b9ad1e' ] ->GEOSHAPE GEO.NORMALIZE SWAP T GEOSHAPE-> SWAP SIZE",
            Ok(
                r#"[1,["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1e","b9ad1f"]]"#,
            ),
        ),
        (
            r"[ 'b9ad1e' ] ->GEOSHAPE @P GEO.NORMALIZE T GEOSHAPE-> SWAP SIZE",
            Ok(
                r#"[1,["b90","b91","b92","b93","b94","b95","b96","b97","b98","b99","b9b","b9c","b9d","b9e","b9f","b9a0","b9a1","b9a2","b9a3","b9a4","b9a5","b9a6","b9a7","b9a8","b9a9","b9aa","b9ab","b9ac","b9ae","b9af","b9ad0","b9ad2","b9ad3","b9ad4","b9ad5","b9ad6","b9ad7","b9ad8","b9ad9","b9ada","b9adb","b9adc","b9add","b9ade","b9adf","b9ad10","b9ad11","b9ad12","b9ad13","b9ad14","b9ad15","b9ad16","b9ad17","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1e","b9ad1f"]]"#,
            ),
        ),
        (
            r"@K [ 'b9a0' 'b9a1' ] ->GEOSHAPE GEO.NORMALIZE SWAP SIZE SWAP SIZE",
            Ok(r"[2,16]"),
        ),
        (r"@M @D GEO.NORMALIZE SWAP SIZE SWAP SIZE", Ok(r"[3,184]")),
        (
            r"[ 'b9ad' 'b9ad1e' ] ->GEOSHAPE [ 'b9ad1ee' 'b9ae' ] ->GEOSHAPE GEO.NORMALIZE SWAP SIZE SWAP SIZE",
            Ok(r"[2,46]"),
        ),
        (
            r"@P [ 'b9ad1ee0c5' ] ->GEOSHAPE GEO.NORMALIZE SWAP SIZE SWAP SIZE",
            Ok(r"[1,121]"),
        ),
        (
            r"@P [ 'b9ad1ee0c5' ] ->GEOSHAPE GEO.NORMALIZE SWAP T GEOSHAPE-> SWAP DEPTH ->LIST SIZE",
            Ok(r"[2]"),
        ),
        (
            r"@A 1 GEO.NORMALIZE",
            Err(r"GEO.NORMALIZE operates on two GEOSHAPE instances."),
        ),
        (
            r"1 @A GEO.NORMALIZE",
            Err(r"GEO.NORMALIZE operates on two GEOSHAPE instances."),
        ),
        (
            r"1 GEO.NORMALIZE",
            Err(r"GEO.NORMALIZE operates on two GEOSHAPE instances."),
        ),
        (r"GEO.NORMALIZE", Err(r"Empty stack.")),
        (r"@A GEO.NORMALIZE", Err(r"Empty stack.")),
        (
            r"[ @A @C ] GEO.NORMALIZE",
            Err(r"GEO.NORMALIZE operates on two GEOSHAPE instances."),
        ),
        (
            r"@A NULL GEO.NORMALIZE",
            Err(r"GEO.NORMALIZE operates on two GEOSHAPE instances."),
        ),
        (
            r"NULL @A GEO.NORMALIZE",
            Err(r"GEO.NORMALIZE operates on two GEOSHAPE instances."),
        ),
        (r"@A @C GEO.NORMALIZE DEPTH", Ok(r"[2,null,null]")),
        (r"@A @C GEO.NORMALIZE TYPEOF", Ok(r#"["GEOSHAPE",null]"#)),
    ]);
}

#[test]
fn shift_1() {
    check(&[
        (r"@A 0.0 0.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@A 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@A -0.703125 -1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a6"]]"#),
        ),
        (r"@A 1.0 1.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ba"]]"#)),
        (r"@A -1.0 -1.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a6"]]"#)),
        (r"@A 0.35 0.7 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 45.0 90.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a307"]]"#)),
        (
            r"@A -90.0 180.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad"]]"#),
        ),
        (
            r"@A 180.0 360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (
            r"@A -180.0 -360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (r"@A 0.0 0.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@A 0.703125 1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@A -0.703125 -1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (r"@A 1.0 1.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A -1.0 -1.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 0.35 0.7 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 45.0 90.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a307"]]"#)),
        (
            r"@A -90.0 180.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad"]]"#),
        ),
        (
            r"@A 180.0 360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (
            r"@A -180.0 -360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (r"@A 0.0 0.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@A 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@A -0.703125 -1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (r"@A 1.0 1.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A -1.0 -1.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 0.35 0.7 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 45.0 90.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a307"]]"#)),
        (
            r"@A -90.0 180.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad"]]"#),
        ),
        (
            r"@A 180.0 360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (
            r"@A -180.0 -360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (r"@A 0.0 0.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@A 0.703125 1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@A -0.703125 -1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (r"@A 1.0 1.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A -1.0 -1.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 0.35 0.7 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 45.0 90.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a307"]]"#)),
        (
            r"@A -90.0 180.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad"]]"#),
        ),
        (
            r"@A 180.0 360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (
            r"@A -180.0 -360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (r"@A 0.0 0.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@A 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@A -0.703125 -1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a6"]]"#),
        ),
        (r"@A 1.0 1.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ba"]]"#)),
        (r"@A -1.0 -1.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a6"]]"#)),
        (r"@A 0.35 0.7 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 45.0 90.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a307"]]"#)),
        (
            r"@A -90.0 180.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad"]]"#),
        ),
        (
            r"@A 180.0 360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (
            r"@A -180.0 -360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307"]]"#),
        ),
        (
            r"@A 0.0 0.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A 0.703125 1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A -0.703125 -1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A 1.0 1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A -1.0 -1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A 0.35 0.7 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A 45.0 90.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A -90.0 180.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A 180.0 360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A -180.0 -360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (r"@C 0.0 0.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (
            r"@C 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb05"]]"#),
        ),
        (
            r"@C -0.703125 -1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a9"]]"#),
        ),
        (r"@C 1.0 1.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["bb05"]]"#)),
        (r"@C -1.0 -1.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a9"]]"#)),
        (r"@C 0.35 0.7 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 45.0 90.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a304"]]"#)),
        (
            r"@C -90.0 180.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ae"]]"#),
        ),
        (
            r"@C 180.0 360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
        (
            r"@C -180.0 -360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
        (r"@C 0.0 0.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (
            r"@C 0.703125 1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ae"]]"#),
        ),
        (
            r"@C -0.703125 -1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ae"]]"#),
        ),
        (r"@C 1.0 1.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C -1.0 -1.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 0.35 0.7 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 45.0 90.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a304"]]"#)),
        (
            r"@C -90.0 180.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ae"]]"#),
        ),
        (
            r"@C 180.0 360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
        (
            r"@C -180.0 -360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
        (r"@C 0.0 0.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (
            r"@C 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ae"]]"#),
        ),
        (
            r"@C -0.703125 -1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ae"]]"#),
        ),
        (r"@C 1.0 1.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C -1.0 -1.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 0.35 0.7 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 45.0 90.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a304"]]"#)),
        (
            r"@C -90.0 180.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ae"]]"#),
        ),
        (
            r"@C 180.0 360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
        (
            r"@C -180.0 -360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
        (r"@C 0.0 0.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (
            r"@C 0.703125 1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ae"]]"#),
        ),
        (
            r"@C -0.703125 -1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ae"]]"#),
        ),
        (r"@C 1.0 1.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C -1.0 -1.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 0.35 0.7 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 45.0 90.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a304"]]"#)),
        (
            r"@C -90.0 180.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ae"]]"#),
        ),
        (
            r"@C 180.0 360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
        (
            r"@C -180.0 -360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
    ]);
}

#[test]
fn shift_2() {
    check(&[
        (r"@C 0.0 0.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (
            r"@C 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb05"]]"#),
        ),
        (
            r"@C -0.703125 -1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a9"]]"#),
        ),
        (r"@C 1.0 1.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["bb05"]]"#)),
        (r"@C -1.0 -1.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a9"]]"#)),
        (r"@C 0.35 0.7 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ae"]]"#)),
        (r"@C 45.0 90.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a304"]]"#)),
        (
            r"@C -90.0 180.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ae"]]"#),
        ),
        (
            r"@C 180.0 360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
        (
            r"@C -180.0 -360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5304"]]"#),
        ),
        (
            r"@C 0.0 0.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@C 0.703125 1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@C -0.703125 -1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@C 1.0 1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@C -1.0 -1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@C 0.35 0.7 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@C 45.0 90.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@C -90.0 180.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@C 180.0 360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@C -180.0 -360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (r"@P 0.0 0.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (
            r"@P 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"@P -0.703125 -1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (r"@P 1.0 1.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P -1.0 -1.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 0.35 0.7 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 45.0 90.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a3"]]"#)),
        (r"@P -90.0 180.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["79"]]"#)),
        (r"@P 180.0 360.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["53"]]"#)),
        (
            r"@P -180.0 -360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["53"]]"#),
        ),
        (r"@P 0.0 0.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (
            r"@P 0.703125 1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"@P -0.703125 -1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (r"@P 1.0 1.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P -1.0 -1.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 0.35 0.7 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 45.0 90.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a3"]]"#)),
        (r"@P -90.0 180.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["79"]]"#)),
        (r"@P 180.0 360.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["53"]]"#)),
        (
            r"@P -180.0 -360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["53"]]"#),
        ),
        (r"@P 0.0 0.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (
            r"@P 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"@P -0.703125 -1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (r"@P 1.0 1.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P -1.0 -1.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 0.35 0.7 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9"]]"#)),
        (r"@P 45.0 90.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a3"]]"#)),
        (r"@P -90.0 180.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["79"]]"#)),
        (r"@P 180.0 360.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["53"]]"#)),
        (
            r"@P -180.0 -360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["53"]]"#),
        ),
        (
            r"@P 0.0 0.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 0.703125 1.40625 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -0.703125 -1.40625 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 1.0 1.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -1.0 -1.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 0.35 0.7 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 45.0 90.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -90.0 180.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 180.0 360.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -180.0 -360.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 0.0 0.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -0.703125 -1.40625 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 1.0 1.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -1.0 -1.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 0.35 0.7 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 45.0 90.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -90.0 180.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 180.0 360.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -180.0 -360.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 0.0 0.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 0.703125 1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -0.703125 -1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 1.0 1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -1.0 -1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 0.35 0.7 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 45.0 90.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -90.0 180.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P 180.0 360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@P -180.0 -360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"@M 0.0 0.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bb00","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@M -0.703125 -1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f0","f1","f2","f3","f4","f6","f8","f9","fc","a10","a11","a12","a13","a14","a16","a18","a19","a1c","b80","b81","b82","b83","b84","b86","b88","b89","b8c","f50","f51","f52","f53","f54","f56","f58","f59","f5a","f5b","f5c","f5e","f70","f71","f72","f73","f74","f76","f78","f79","f7a","f7b","f7c","f7e","fa0","fa1","fa2","fa3","fa4","fa5","fa6","fa7","fa8","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fd0","fd1","fd2","fd3","fd4","fd6","fd8","fd9","fda","fdb","fdc","fde","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff6","ff8","ff9","ffc","8aff","8baa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","a055","a057","a05d","a05f","a075","a077","a07d","a07f","a0d5","a0d7","a0dd","a0df","a0f5","a0f7","a0fd","a150","a151","a152","a153","a154","a156","a158","a159","a15a","a15b","a15c","a15e","a170","a171","a172","a173","a174","a176","a178","a179","a17a","a17b","a17c","a17e","a1a0","a1a1","a1a2","a1a3","a1a4","a1a5","a1a6","a1a7","a1a8","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1d0","a1d1","a1d2","a1d3","a1d4","a1d6","a1d8","a1d9","a1da","a1db","a1dc","a1de","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f6","a1f8","a1f9","a1fc","a7ff","ad55","ad57","ad5d","ad5f","ad75","ad77","ad7d","ad7f","add5","add7","addd","addf","adf5","adf7","adfd","b2aa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b850","b851","b852","b853","b854","b856","b858","b859","b85a","b85b","b85c","b85e","b870","b871","b872","b873","b874","b876","b878","b879","b87a","b87b","b87c","b87e","b8a0","b8a1","b8a2","b8a3","b8a4","b8a5","b8a6","b8a7","b8a8","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8d0","b8d1","b8d2","b8d3","b8d4","b8d6","b8d8","b8d9","b8da","b8db","b8dc","b8de","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f6","b8f8","b8f9","b8fc","b9a6","cfff","daaa","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","e555","e557","e55d","e55f","e575","e577","e57d","e57f","e5d5","e5d7","e5dd","e5df","e5f5","e5f7","e5fd","e5ff","e755","e757","e75d","e75f","e775","e777","e77d","e77f","e7d5","e7d7","e7dd","e7df","e7f5","e7f7","e7fd","e7ff","ed55","ed57","ed5d","ed5f","ed75","ed77","ed7d","ed7f","edd5","edd7","eddd","eddf","edf5","edf7","edfd","edff","ef55","ef57","ef5d","ef5f","ef75","ef77","ef7d","ef7f","efd5","efd7","efdd","efdf","eff5","eff7","effd","f550","f551","f552","f553","f554","f556","f558","f559","f55a","f55b","f55c","f55e","f570","f571","f572","f573","f574","f576","f578","f579","f57a","f57b","f57c","f57e","f5d0","f5d1","f5d2","f5d3","f5d4","f5d6","f5d8","f5d9","f5da","f5db","f5dc","f5de","f5f0","f5f1","f5f2","f5f3","f5f4","f5f6","f5f8","f5f9","f5fa","f5fb","f5fc","f5fe","f750","f751","f752","f753","f754","f756","f758","f759","f75a","f75b","f75c","f75e","f770","f771","f772","f773","f774","f776","f778","f779","f77a","f77b","f77c","f77e","f7d0","f7d1","f7d2","f7d3","f7d4","f7d6","f7d8","f7d9","f7da","f7db","f7dc","f7de","f7f0","f7f1","f7f2","f7f3","f7f4","f7f6","f7f8","f7f9","f7fa","f7fb","f7fc","f7fe","faa0","faa1","faa2","faa3","faa4","faa5","faa6","faa7","faa8","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fd50","fd51","fd52","fd53","fd54","fd56","fd58","fd59","fd5a","fd5b","fd5c","fd5e","fd70","fd71","fd72","fd73","fd74","fd76","fd78","fd79","fd7a","fd7b","fd7c","fd7e","fdd0","fdd1","fdd2","fdd3","fdd4","fdd6","fdd8","fdd9","fdda","fddb","fddc","fdde","fdf0","fdf1","fdf2","fdf3","fdf4","fdf6","fdf8","fdf9","fdfa","fdfb","fdfc","fdfe","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ff50","ff51","ff52","ff53","ff54","ff56","ff58","ff59","ff5a","ff5b","ff5c","ff5e","ff70","ff71","ff72","ff73","ff74","ff76","ff78","ff79","ff7a","ff7b","ff7c","ff7e","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd6","ffd8","ffd9","ffda","ffdb","ffdc","ffde","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff6","fff8","fff9","fffc"]]"#,
            ),
        ),
        (
            r"@M 1.0 1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bb00","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@M -1.0 -1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f0","f1","f2","f3","f4","f6","f8","f9","fc","a10","a11","a12","a13","a14","a16","a18","a19","a1c","b80","b81","b82","b83","b84","b86","b88","b89","b8c","f50","f51","f52","f53","f54","f56","f58","f59","f5a","f5b","f5c","f5e","f70","f71","f72","f73","f74","f76","f78","f79","f7a","f7b","f7c","f7e","fa0","fa1","fa2","fa3","fa4","fa5","fa6","fa7","fa8","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fd0","fd1","fd2","fd3","fd4","fd6","fd8","fd9","fda","fdb","fdc","fde","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff6","ff8","ff9","ffc","8aff","8baa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","a055","a057","a05d","a05f","a075","a077","a07d","a07f","a0d5","a0d7","a0dd","a0df","a0f5","a0f7","a0fd","a150","a151","a152","a153","a154","a156","a158","a159","a15a","a15b","a15c","a15e","a170","a171","a172","a173","a174","a176","a178","a179","a17a","a17b","a17c","a17e","a1a0","a1a1","a1a2","a1a3","a1a4","a1a5","a1a6","a1a7","a1a8","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1d0","a1d1","a1d2","a1d3","a1d4","a1d6","a1d8","a1d9","a1da","a1db","a1dc","a1de","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f6","a1f8","a1f9","a1fc","a7ff","ad55","ad57","ad5d","ad5f","ad75","ad77","ad7d","ad7f","add5","add7","addd","addf","adf5","adf7","adfd","b2aa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b850","b851","b852","b853","b854","b856","b858","b859","b85a","b85b","b85c","b85e","b870","b871","b872","b873","b874","b876","b878","b879","b87a","b87b","b87c","b87e","b8a0","b8a1","b8a2","b8a3","b8a4","b8a5","b8a6","b8a7","b8a8","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8d0","b8d1","b8d2","b8d3","b8d4","b8d6","b8d8","b8d9","b8da","b8db","b8dc","b8de","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f6","b8f8","b8f9","b8fc","b9a6","cfff","daaa","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","e555","e557","e55d","e55f","e575","e577","e57d","e57f","e5d5","e5d7","e5dd","e5df","e5f5","e5f7","e5fd","e5ff","e755","e757","e75d","e75f","e775","e777","e77d","e77f","e7d5","e7d7","e7dd","e7df","e7f5","e7f7","e7fd","e7ff","ed55","ed57","ed5d","ed5f","ed75","ed77","ed7d","ed7f","edd5","edd7","eddd","eddf","edf5","edf7","edfd","edff","ef55","ef57","ef5d","ef5f","ef75","ef77","ef7d","ef7f","efd5","efd7","efdd","efdf","eff5","eff7","effd","f550","f551","f552","f553","f554","f556","f558","f559","f55a","f55b","f55c","f55e","f570","f571","f572","f573","f574","f576","f578","f579","f57a","f57b","f57c","f57e","f5d0","f5d1","f5d2","f5d3","f5d4","f5d6","f5d8","f5d9","f5da","f5db","f5dc","f5de","f5f0","f5f1","f5f2","f5f3","f5f4","f5f6","f5f8","f5f9","f5fa","f5fb","f5fc","f5fe","f750","f751","f752","f753","f754","f756","f758","f759","f75a","f75b","f75c","f75e","f770","f771","f772","f773","f774","f776","f778","f779","f77a","f77b","f77c","f77e","f7d0","f7d1","f7d2","f7d3","f7d4","f7d6","f7d8","f7d9","f7da","f7db","f7dc","f7de","f7f0","f7f1","f7f2","f7f3","f7f4","f7f6","f7f8","f7f9","f7fa","f7fb","f7fc","f7fe","faa0","faa1","faa2","faa3","faa4","faa5","faa6","faa7","faa8","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fd50","fd51","fd52","fd53","fd54","fd56","fd58","fd59","fd5a","fd5b","fd5c","fd5e","fd70","fd71","fd72","fd73","fd74","fd76","fd78","fd79","fd7a","fd7b","fd7c","fd7e","fdd0","fdd1","fdd2","fdd3","fdd4","fdd6","fdd8","fdd9","fdda","fddb","fddc","fdde","fdf0","fdf1","fdf2","fdf3","fdf4","fdf6","fdf8","fdf9","fdfa","fdfb","fdfc","fdfe","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ff50","ff51","ff52","ff53","ff54","ff56","ff58","ff59","ff5a","ff5b","ff5c","ff5e","ff70","ff71","ff72","ff73","ff74","ff76","ff78","ff79","ff7a","ff7b","ff7c","ff7e","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd6","ffd8","ffd9","ffda","ffdb","ffdc","ffde","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff6","fff8","fff9","fffc"]]"#,
            ),
        ),
        (
            r"@M 0.35 0.7 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 45.0 90.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["e","a2","fb","a307"]]"#),
        ),
        (
            r"@M -90.0 180.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["3","61","78","79ad"]]"#),
        ),
        (
            r"@M 180.0 360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
        (
            r"@M -180.0 -360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
        (
            r"@M 0.0 0.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 0.703125 1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M -0.703125 -1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 1.0 1.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M -1.0 -1.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 0.35 0.7 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 45.0 90.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["e","a2","fb","a307"]]"#),
        ),
        (
            r"@M -90.0 180.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["3","61","78","79ad"]]"#),
        ),
        (
            r"@M 180.0 360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
        (
            r"@M -180.0 -360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
    ]);
}

#[test]
fn shift_3() {
    check(&[
        (
            r"@M 0.0 0.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M -0.703125 -1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 1.0 1.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M -1.0 -1.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 0.35 0.7 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 45.0 90.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["e","a2","fb","a307"]]"#),
        ),
        (
            r"@M -90.0 180.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["3","61","78","79ad"]]"#),
        ),
        (
            r"@M 180.0 360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
        (
            r"@M -180.0 -360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
        (
            r"@M 0.0 0.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 0.703125 1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M -0.703125 -1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 1.0 1.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M -1.0 -1.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 0.35 0.7 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 45.0 90.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["e","a2","fb","a307"]]"#),
        ),
        (
            r"@M -90.0 180.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["3","61","78","79ad"]]"#),
        ),
        (
            r"@M 180.0 360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
        (
            r"@M -180.0 -360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
        (
            r"@M 0.0 0.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bb00","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@M -0.703125 -1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f0","f1","f2","f3","f4","f6","f8","f9","fc","a10","a11","a12","a13","a14","a16","a18","a19","a1c","b80","b81","b82","b83","b84","b86","b88","b89","b8c","f50","f51","f52","f53","f54","f56","f58","f59","f5a","f5b","f5c","f5e","f70","f71","f72","f73","f74","f76","f78","f79","f7a","f7b","f7c","f7e","fa0","fa1","fa2","fa3","fa4","fa5","fa6","fa7","fa8","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fd0","fd1","fd2","fd3","fd4","fd6","fd8","fd9","fda","fdb","fdc","fde","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff6","ff8","ff9","ffc","8aff","8baa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","a055","a057","a05d","a05f","a075","a077","a07d","a07f","a0d5","a0d7","a0dd","a0df","a0f5","a0f7","a0fd","a150","a151","a152","a153","a154","a156","a158","a159","a15a","a15b","a15c","a15e","a170","a171","a172","a173","a174","a176","a178","a179","a17a","a17b","a17c","a17e","a1a0","a1a1","a1a2","a1a3","a1a4","a1a5","a1a6","a1a7","a1a8","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1d0","a1d1","a1d2","a1d3","a1d4","a1d6","a1d8","a1d9","a1da","a1db","a1dc","a1de","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f6","a1f8","a1f9","a1fc","a7ff","ad55","ad57","ad5d","ad5f","ad75","ad77","ad7d","ad7f","add5","add7","addd","addf","adf5","adf7","adfd","b2aa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b850","b851","b852","b853","b854","b856","b858","b859","b85a","b85b","b85c","b85e","b870","b871","b872","b873","b874","b876","b878","b879","b87a","b87b","b87c","b87e","b8a0","b8a1","b8a2","b8a3","b8a4","b8a5","b8a6","b8a7","b8a8","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8d0","b8d1","b8d2","b8d3","b8d4","b8d6","b8d8","b8d9","b8da","b8db","b8dc","b8de","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f6","b8f8","b8f9","b8fc","b9a6","cfff","daaa","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","e555","e557","e55d","e55f","e575","e577","e57d","e57f","e5d5","e5d7","e5dd","e5df","e5f5","e5f7","e5fd","e5ff","e755","e757","e75d","e75f","e775","e777","e77d","e77f","e7d5","e7d7","e7dd","e7df","e7f5","e7f7","e7fd","e7ff","ed55","ed57","ed5d","ed5f","ed75","ed77","ed7d","ed7f","edd5","edd7","eddd","eddf","edf5","edf7","edfd","edff","ef55","ef57","ef5d","ef5f","ef75","ef77","ef7d","ef7f","efd5","efd7","efdd","efdf","eff5","eff7","effd","f550","f551","f552","f553","f554","f556","f558","f559","f55a","f55b","f55c","f55e","f570","f571","f572","f573","f574","f576","f578","f579","f57a","f57b","f57c","f57e","f5d0","f5d1","f5d2","f5d3","f5d4","f5d6","f5d8","f5d9","f5da","f5db","f5dc","f5de","f5f0","f5f1","f5f2","f5f3","f5f4","f5f6","f5f8","f5f9","f5fa","f5fb","f5fc","f5fe","f750","f751","f752","f753","f754","f756","f758","f759","f75a","f75b","f75c","f75e","f770","f771","f772","f773","f774","f776","f778","f779","f77a","f77b","f77c","f77e","f7d0","f7d1","f7d2","f7d3","f7d4","f7d6","f7d8","f7d9","f7da","f7db","f7dc","f7de","f7f0","f7f1","f7f2","f7f3","f7f4","f7f6","f7f8","f7f9","f7fa","f7fb","f7fc","f7fe","faa0","faa1","faa2","faa3","faa4","faa5","faa6","faa7","faa8","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fd50","fd51","fd52","fd53","fd54","fd56","fd58","fd59","fd5a","fd5b","fd5c","fd5e","fd70","fd71","fd72","fd73","fd74","fd76","fd78","fd79","fd7a","fd7b","fd7c","fd7e","fdd0","fdd1","fdd2","fdd3","fdd4","fdd6","fdd8","fdd9","fdda","fddb","fddc","fdde","fdf0","fdf1","fdf2","fdf3","fdf4","fdf6","fdf8","fdf9","fdfa","fdfb","fdfc","fdfe","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ff50","ff51","ff52","ff53","ff54","ff56","ff58","ff59","ff5a","ff5b","ff5c","ff5e","ff70","ff71","ff72","ff73","ff74","ff76","ff78","ff79","ff7a","ff7b","ff7c","ff7e","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd6","ffd8","ffd9","ffda","ffdb","ffdc","ffde","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff6","fff8","fff9","fffc"]]"#,
            ),
        ),
        (
            r"@M 1.0 1.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bb00","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@M -1.0 -1.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f0","f1","f2","f3","f4","f6","f8","f9","fc","a10","a11","a12","a13","a14","a16","a18","a19","a1c","b80","b81","b82","b83","b84","b86","b88","b89","b8c","f50","f51","f52","f53","f54","f56","f58","f59","f5a","f5b","f5c","f5e","f70","f71","f72","f73","f74","f76","f78","f79","f7a","f7b","f7c","f7e","fa0","fa1","fa2","fa3","fa4","fa5","fa6","fa7","fa8","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fd0","fd1","fd2","fd3","fd4","fd6","fd8","fd9","fda","fdb","fdc","fde","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff6","ff8","ff9","ffc","8aff","8baa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","a055","a057","a05d","a05f","a075","a077","a07d","a07f","a0d5","a0d7","a0dd","a0df","a0f5","a0f7","a0fd","a150","a151","a152","a153","a154","a156","a158","a159","a15a","a15b","a15c","a15e","a170","a171","a172","a173","a174","a176","a178","a179","a17a","a17b","a17c","a17e","a1a0","a1a1","a1a2","a1a3","a1a4","a1a5","a1a6","a1a7","a1a8","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1d0","a1d1","a1d2","a1d3","a1d4","a1d6","a1d8","a1d9","a1da","a1db","a1dc","a1de","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f6","a1f8","a1f9","a1fc","a7ff","ad55","ad57","ad5d","ad5f","ad75","ad77","ad7d","ad7f","add5","add7","addd","addf","adf5","adf7","adfd","b2aa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b850","b851","b852","b853","b854","b856","b858","b859","b85a","b85b","b85c","b85e","b870","b871","b872","b873","b874","b876","b878","b879","b87a","b87b","b87c","b87e","b8a0","b8a1","b8a2","b8a3","b8a4","b8a5","b8a6","b8a7","b8a8","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8d0","b8d1","b8d2","b8d3","b8d4","b8d6","b8d8","b8d9","b8da","b8db","b8dc","b8de","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f6","b8f8","b8f9","b8fc","b9a6","cfff","daaa","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","e555","e557","e55d","e55f","e575","e577","e57d","e57f","e5d5","e5d7","e5dd","e5df","e5f5","e5f7","e5fd","e5ff","e755","e757","e75d","e75f","e775","e777","e77d","e77f","e7d5","e7d7","e7dd","e7df","e7f5","e7f7","e7fd","e7ff","ed55","ed57","ed5d","ed5f","ed75","ed77","ed7d","ed7f","edd5","edd7","eddd","eddf","edf5","edf7","edfd","edff","ef55","ef57","ef5d","ef5f","ef75","ef77","ef7d","ef7f","efd5","efd7","efdd","efdf","eff5","eff7","effd","f550","f551","f552","f553","f554","f556","f558","f559","f55a","f55b","f55c","f55e","f570","f571","f572","f573","f574","f576","f578","f579","f57a","f57b","f57c","f57e","f5d0","f5d1","f5d2","f5d3","f5d4","f5d6","f5d8","f5d9","f5da","f5db","f5dc","f5de","f5f0","f5f1","f5f2","f5f3","f5f4","f5f6","f5f8","f5f9","f5fa","f5fb","f5fc","f5fe","f750","f751","f752","f753","f754","f756","f758","f759","f75a","f75b","f75c","f75e","f770","f771","f772","f773","f774","f776","f778","f779","f77a","f77b","f77c","f77e","f7d0","f7d1","f7d2","f7d3","f7d4","f7d6","f7d8","f7d9","f7da","f7db","f7dc","f7de","f7f0","f7f1","f7f2","f7f3","f7f4","f7f6","f7f8","f7f9","f7fa","f7fb","f7fc","f7fe","faa0","faa1","faa2","faa3","faa4","faa5","faa6","faa7","faa8","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fd50","fd51","fd52","fd53","fd54","fd56","fd58","fd59","fd5a","fd5b","fd5c","fd5e","fd70","fd71","fd72","fd73","fd74","fd76","fd78","fd79","fd7a","fd7b","fd7c","fd7e","fdd0","fdd1","fdd2","fdd3","fdd4","fdd6","fdd8","fdd9","fdda","fddb","fddc","fdde","fdf0","fdf1","fdf2","fdf3","fdf4","fdf6","fdf8","fdf9","fdfa","fdfb","fdfc","fdfe","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ff50","ff51","ff52","ff53","ff54","ff56","ff58","ff59","ff5a","ff5b","ff5c","ff5e","ff70","ff71","ff72","ff73","ff74","ff76","ff78","ff79","ff7a","ff7b","ff7c","ff7e","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd6","ffd8","ffd9","ffda","ffdb","ffdc","ffde","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff6","fff8","fff9","fffc"]]"#,
            ),
        ),
        (
            r"@M 0.35 0.7 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@M 45.0 90.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["e","a2","fb","a307"]]"#),
        ),
        (
            r"@M -90.0 180.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["3","61","78","79ad"]]"#),
        ),
        (
            r"@M 180.0 360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
        (
            r"@M -180.0 -360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1","4b","52","5307"]]"#),
        ),
        (
            r"@M 0.0 0.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@M 0.703125 1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@M -0.703125 -1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@M 1.0 1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@M -1.0 -1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@M 0.35 0.7 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@M 45.0 90.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@M -90.0 180.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@M 180.0 360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@M -180.0 -360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@D 0.0 0.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba1ee0","b9ba1ee1","f01934567"]]"#),
        ),
        (
            r"@D -0.703125 -1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a61ee0","b9a61ee1","f00534567"]]"#),
        ),
        (
            r"@D 1.0 1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba89d4","b9ba89d5","f0198cb27"]]"#),
        ),
        (
            r"@D -1.0 -1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a4e704","b9a4e705","daafe7627"]]"#),
        ),
        (
            r"@D 0.35 0.7 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b9adde9fc","b9adde9fd","b9adde9fe","b9adde9ff","b9addeb54","b9addeb55","b9addeb56","b9addeb57","b9addeca8","b9addeca9","b9addecaa","b9addecab","b9addecac","b9addecad","b9addecae","b9addecaf","b9addecb8","b9addecb9","b9addecba","b9addecbb","b9addee00","b9addee01","b9addee02","b9addee03","b9addee04","b9addee05","b9addee06","b9addee07","b9addee10","b9addee11","b9addee12","b9addee13","f012f454b"]]"#,
            ),
        ),
        (
            r"@D 45.0 90.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a307b44a","a307b44b","eab89efcd"]]"#),
        ),
        (
            r"@D -90.0 180.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad1ee0","79ad1ee1","301234567"]]"#),
        ),
        (
            r"@D 180.0 360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D -180.0 -360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D 0.0 0.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 0.703125 1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D -0.703125 -1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 1.0 1.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D -1.0 -1.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 0.35 0.7 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 45.0 90.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a307b44a","a307b44b","eab89efcd"]]"#),
        ),
        (
            r"@D -90.0 180.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad1ee0","79ad1ee1","301234567"]]"#),
        ),
        (
            r"@D 180.0 360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D -180.0 -360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D 0.0 0.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D -0.703125 -1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 1.0 1.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D -1.0 -1.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 0.35 0.7 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 45.0 90.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a307b44a","a307b44b","eab89efcd"]]"#),
        ),
        (
            r"@D -90.0 180.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad1ee0","79ad1ee1","301234567"]]"#),
        ),
        (
            r"@D 180.0 360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D -180.0 -360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D 0.0 0.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 0.703125 1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D -0.703125 -1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 1.0 1.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D -1.0 -1.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 0.35 0.7 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 45.0 90.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a307b44a","a307b44b","eab89efcd"]]"#),
        ),
        (
            r"@D -90.0 180.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad1ee0","79ad1ee1","301234567"]]"#),
        ),
        (
            r"@D 180.0 360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D -180.0 -360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D 0.0 0.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba1ee0","b9ba1ee1","f01934567"]]"#),
        ),
        (
            r"@D -0.703125 -1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a61ee0","b9a61ee1","f00534567"]]"#),
        ),
        (
            r"@D 1.0 1.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba1ee0","b9ba1ee1","f01934567"]]"#),
        ),
        (
            r"@D -1.0 -1.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a61ee0","b9a61ee1","f00534567"]]"#),
        ),
        (
            r"@D 0.35 0.7 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 45.0 90.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a307b44a","a307b44b","eab89efcd"]]"#),
        ),
        (
            r"@D -90.0 180.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad1ee0","79ad1ee1","301234567"]]"#),
        ),
        (
            r"@D 180.0 360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D -180.0 -360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D 0.0 0.0 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0","b9ad1ee1","f01234567"]]"#),
        ),
        (
            r"@D 0.703125 1.40625 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba1ee0","b9ba1ee1","f01934567"]]"#),
        ),
        (
            r"@D -0.703125 -1.40625 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a61ee0","b9a61ee1","f00534567"]]"#),
        ),
        (
            r"@D 1.0 1.0 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba8ee0","b9ba8ee1","f019a4567"]]"#),
        ),
        (
            r"@D -1.0 -1.0 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a4cee0","b9a4cee1","daafe4567"]]"#),
        ),
        (
            r"@D 0.35 0.7 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9addee0","b9addee1","f012f4567"]]"#),
        ),
        (
            r"@D 45.0 90.0 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a307b44a","a307b44b","eab89efcd"]]"#),
        ),
        (
            r"@D -90.0 180.0 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79ad1ee0","79ad1ee1","301234567"]]"#),
        ),
        (
            r"@D 180.0 360.0 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
        (
            r"@D -180.0 -360.0 10 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["5307b44a","5307b44b","1ab89efcd"]]"#),
        ),
    ]);
}

#[test]
fn shift_4() {
    check(&[
        (r"@K 0.0 0.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b9a3","b9a6","b9a7","b9a9","b9ab","b9ac","b9ad","b9ae","b9af","b9b2","b9b8","b9ba","bb01","bb04","bb05","bb10"]]"#,
            ),
        ),
        (
            r"@K -0.703125 -1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b8df","b8f5","b8f7","b8fd","b98a","b98b","b98e","b9a0","b9a1","b9a2","b9a3","b9a4","b9a6","b9a8","b9a9","b9ac"]]"#,
            ),
        ),
        (
            r"@K 1.0 1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b9a3","b9a6","b9a7","b9a9","b9ab","b9ac","b9ad","b9ae","b9af","b9b2","b9b8","b9ba","bb01","bb04","bb05","bb10"]]"#,
            ),
        ),
        (
            r"@K -1.0 -1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b8df","b8f5","b8f7","b8fd","b98a","b98b","b98e","b9a0","b9a1","b9a2","b9a3","b9a4","b9a6","b9a8","b9a9","b9ac"]]"#,
            ),
        ),
        (r"@K 0.35 0.7 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 45.0 90.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a30"]]"#)),
        (
            r"@K -90.0 180.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79a"]]"#),
        ),
        (
            r"@K 180.0 360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (
            r"@K -180.0 -360.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (r"@K 0.0 0.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K 0.703125 1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a"]]"#),
        ),
        (
            r"@K -0.703125 -1.40625 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a"]]"#),
        ),
        (r"@K 1.0 1.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K -1.0 -1.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 0.35 0.7 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 45.0 90.0 2 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a30"]]"#)),
        (
            r"@K -90.0 180.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79a"]]"#),
        ),
        (
            r"@K 180.0 360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (
            r"@K -180.0 -360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (r"@K 0.0 0.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a"]]"#),
        ),
        (
            r"@K -0.703125 -1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a"]]"#),
        ),
        (r"@K 1.0 1.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K -1.0 -1.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 0.35 0.7 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 45.0 90.0 4 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a30"]]"#)),
        (
            r"@K -90.0 180.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79a"]]"#),
        ),
        (
            r"@K 180.0 360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (
            r"@K -180.0 -360.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (r"@K 0.0 0.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K 0.703125 1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a"]]"#),
        ),
        (
            r"@K -0.703125 -1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a"]]"#),
        ),
        (r"@K 1.0 1.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K -1.0 -1.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 0.35 0.7 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 45.0 90.0 6 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a30"]]"#)),
        (
            r"@K -90.0 180.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79a"]]"#),
        ),
        (
            r"@K 180.0 360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (
            r"@K -180.0 -360.0 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (r"@K 0.0 0.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (
            r"@K 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b9a3","b9a6","b9a7","b9a9","b9ab","b9ac","b9ad","b9ae","b9af","b9b2","b9b8","b9ba","bb01","bb04","bb05","bb10"]]"#,
            ),
        ),
        (
            r"@K -0.703125 -1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b8df","b8f5","b8f7","b8fd","b98a","b98b","b98e","b9a0","b9a1","b9a2","b9a3","b9a4","b9a6","b9a8","b9a9","b9ac"]]"#,
            ),
        ),
        (
            r"@K 1.0 1.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b9a3","b9a6","b9a7","b9a9","b9ab","b9ac","b9ad","b9ae","b9af","b9b2","b9b8","b9ba","bb01","bb04","bb05","bb10"]]"#,
            ),
        ),
        (
            r"@K -1.0 -1.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b8df","b8f5","b8f7","b8fd","b98a","b98b","b98e","b9a0","b9a1","b9a2","b9a3","b9a4","b9a6","b9a8","b9a9","b9ac"]]"#,
            ),
        ),
        (r"@K 0.35 0.7 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9a"]]"#)),
        (r"@K 45.0 90.0 8 GEOSHIFT T GEOSHAPE->", Ok(r#"[["a30"]]"#)),
        (
            r"@K -90.0 180.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["79a"]]"#),
        ),
        (
            r"@K 180.0 360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (
            r"@K -180.0 -360.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["530"]]"#),
        ),
        (
            r"@K 0.0 0.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@K 0.703125 1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@K -0.703125 -1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@K 1.0 1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@K -1.0 -1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@K 0.35 0.7 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@K 45.0 90.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@K -90.0 180.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@K 180.0 360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@K -180.0 -360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (r"@E 0.0 0.0 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@E -0.703125 -1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r"[[]]"),
        ),
        (r"@E 1.0 1.0 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E -1.0 -1.0 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E 0.35 0.7 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E 45.0 90.0 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E -90.0 180.0 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E 180.0 360.0 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (r"@E -180.0 -360.0 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@E 0.0 0.0 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.703125 1.40625 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -0.703125 -1.40625 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 1.0 1.0 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -1.0 -1.0 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.35 0.7 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 45.0 90.0 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -90.0 180.0 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 180.0 360.0 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -180.0 -360.0 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.0 0.0 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -0.703125 -1.40625 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 1.0 1.0 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -1.0 -1.0 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.35 0.7 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 45.0 90.0 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -90.0 180.0 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 180.0 360.0 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -180.0 -360.0 4 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.0 0.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.703125 1.40625 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -0.703125 -1.40625 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 1.0 1.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -1.0 -1.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.35 0.7 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 45.0 90.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -90.0 180.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 180.0 360.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -180.0 -360.0 6 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
    ]);
}

#[test]
fn shift_5() {
    check(&[
        (
            r"@E 0.0 0.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -0.703125 -1.40625 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 1.0 1.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -1.0 -1.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.35 0.7 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 45.0 90.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -90.0 180.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 180.0 360.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -180.0 -360.0 8 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.0 0.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.703125 1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -0.703125 -1.40625 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 1.0 1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -1.0 -1.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 0.35 0.7 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 45.0 90.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -90.0 180.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E 180.0 360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@E -180.0 -360.0 10 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (
            r"@A 0.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M 0.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","bb00","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 0.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M 0.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b884","b885","b890","b891","b894","b895","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba87","ba8d","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa5","baa7","baad","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","da86","da8c","da8e","daa4","daa6","daac","daae","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f004","f006","f00c","f00e","f024","f026","f02c","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa87","fa8d","fa8f","faa5","faa7","faad","faaf"]]"#,
            ),
        ),
        (
            r"@A 0.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M 0.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","828","82a","880","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc4","cc5","cc6","cc7","ccc","ccd","cce","ccf","ce4","ce5","ce6","ce7","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","d79","d7b","d7c","d7d","d7e","d7f","dc0","dc2","dc8","dca","dd1","dd4","dd5","de0","de2","de8","dea","e44","e45","e46","e47","e4c","e4d","e4e","e4f","e64","e65","e70","e71","e74","e75","ec9","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","a632","a638","a63a","a690","a692","a698","a69a","a6b0","a6b2","a6b8","a6ba","ac10","ac12","ac18","ac1a","ac30","ac32","ac38","ac3a","ac90","ac92","ac98","ac9a","acb0","acb2","acb8","acba","ae10","ae12","ae18","ae1a","ae30","ae32","ae38","ae3a","ae90","ae92","ae98","ae9a","aeb0","aeb2","aeb8","aeba","c633","c636","c637","c639","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c691","c693","c694","c695","c696","c697","c699","c69b","c69c","c69d","c69e","c69f","c6b1","c6b3","c6b4","c6b5","c6b6","c6b7","c6b9","c6bb","c6bc","c6bd","c6be","c6bf","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","cc11","cc13","cc14","cc15","cc16","cc17","cc19","cc1b","cc1c","cc1d","cc1e","cc1f","cc31","cc33","cc34","cc35","cc36","cc37","cc39","cc3b","cc3c","cc3d","cc3e","cc3f","cc91","cc93","cc94","cc95","cc96","cc97","cc99","cc9b","cc9c","cc9d","cc9e","cc9f","ccb1","ccb3","ccb4","ccb5","ccb6","ccb7","ccb9","ccbb","ccbc","ccbd","ccbe","ccbf","ce11","ce13","ce14","ce15","ce16","ce17","ce19","ce1b","ce1c","ce1d","ce1e","ce1f","ce31","ce33","ce34","ce35","ce36","ce37","ce39","ce3b","ce3c","ce3d","ce3e","ce3f","ce91","ce93","ce94","ce95","ce96","ce97","ce99","ce9b","ce9c","ce9d","ce9e","ce9f","ceb1","ceb3","ceb4","ceb5","ceb6","ceb7","ceb9","cebb","cebc","cebd","cebe","cebf","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","e411","e413","e414","e415","e416","e417","e419","e41b","e41c","e41d","e41e","e41f","e431","e433","e434","e435","e436","e437","e439","e43b","e43c","e43d","e43e","e43f","e491","e493","e494","e495","e496","e497","e499","e49b","e49c","e49d","e49e","e49f","e4b1","e4b3","e4b4","e4b5","e4b6","e4b7","e4b9","e4bb","e4bc","e4bd","e4be","e4bf","e611","e613","e614","e615","e616","e617","e619","e61b","e61c","e61d","e61e","e61f","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec36","ec37","ec38","ec39","ec3a","ec3b","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ec9a","ecb0","ecb2","ecb8","ecba","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee10","ee12","ee18","ee1a","ee30","ee31","ee32","ee34","ee35","ee38","ee3a","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ee90","ee92","ee98","ee9a","eeb0","eeb2","eeb8","eeba","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625"]]"#,
            ),
        ),
        (
            r"@A 2.35 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M 2.35 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b2ab","b2ae","b801","b803","b804","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","baae","baaf","bb00","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 2.35 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M 2.35 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fac","fad","fae","faf","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b82e","b82f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b890","b891","b894","b895","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba93","ba96","ba97","ba99","ba9b","ba9c","ba9d","ba9e","ba9f","bab1","bab3","bab4","bab5","bab6","bab7","bab9","babb","babc","babd","babe","babf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","da86","da87","da8c","da8d","da8e","da8f","da92","da98","da9a","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dab0","dab2","dab8","daba","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f010","f012","f018","f01a","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f030","f032","f038","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa93","fa94","fa95","fa96","fa97","fa99","fa9b","fa9c","fa9d","fa9e","fa9f","fab1","fab3","fab4","fab5","fab6","fab7","fab9","fabb","fabc","fabd","fabe","fabf"]]"#,
            ),
        ),
        (
            r"@A 2.35 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M 2.35 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","828","82a","880","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc4","cc5","cc6","cc7","ccc","ccd","cce","ccf","ce4","ce5","ce6","ce7","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","d79","d7b","d7c","d7d","d7e","d7f","dc0","dc2","dc8","dca","dd1","dd4","dd5","de0","de2","de8","dea","e44","e45","e46","e47","e4c","e4d","e4e","e4f","e64","e65","e70","e71","e74","e75","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e38","8e39","8e3a","8e3b","8e90","8e91","8e92","8e93","8e98","8e99","8e9a","8e9b","8eb0","8eb1","8eb2","8eb3","8eb8","8eb9","8eba","8ebb","a410","a411","a412","a413","a418","a419","a41a","a41b","a430","a431","a632","a633","a636","a638","a639","a63a","a63b","a63c","a63e","a690","a691","a692","a693","a694","a696","a698","a699","a69a","a69b","a69c","a69e","a6b0","a6b1","a6b2","a6b3","a6b4","a6b6","a6b8","a6b9","a6ba","a6bb","a6bc","a6be","ac10","ac11","ac12","ac13","ac14","ac16","ac18","ac19","ac1a","ac1b","ac1c","ac1e","ac30","ac31","ac32","ac33","ac34","ac36","ac38","ac39","ac3a","ac3b","ac3c","ac3e","ac90","ac91","ac92","ac93","ac94","ac96","ac98","ac99","ac9a","ac9b","ac9c","ac9e","acb0","acb1","acb2","acb3","acb4","acb6","acb8","acb9","acba","acbb","acbc","acbe","ae10","ae11","ae12","ae13","ae14","ae16","ae18","ae19","ae1a","ae1b","ae1c","ae1e","ae30","ae31","ae32","ae33","ae34","ae36","ae38","ae39","ae3a","ae3b","ae3c","ae3e","ae90","ae91","ae92","ae93","ae94","ae96","ae98","ae99","ae9a","ae9b","ae9c","ae9e","aeb0","aeb1","aeb2","aeb3","aeb4","aeb6","aeb8","aeb9","aeba","aebb","aebc","aebe","c637","c63d","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c695","c697","c69d","c69f","c6b5","c6b7","c6bd","c6bf","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","cc15","cc17","cc1d","cc1f","cc35","cc37","cc3d","cc3f","cc95","cc97","cc9d","cc9f","ccb5","ccb7","ccbd","ccbf","ce15","ce17","ce1d","ce1f","ce35","ce37","ce3d","ce3f","ce95","ce97","ce9d","ce9f","ceb5","ceb7","cebd","cebf","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","e415","e417","e41d","e41f","e435","e437","e43d","e43f","e495","e497","e49d","e49f","e4b5","e4b7","e4bd","e4bf","e615","e617","e61d","e61f","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec36","ec37","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ec94","ec95","ec96","ec97","ec9a","ec9b","ec9c","ec9d","ec9e","ec9f","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee30","ee31","ee32","ee33","ee34","ee35","ee36","ee38","ee39","ee3a","ee3b","ee3c","ee3e","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ee90","ee91","ee92","ee93","ee94","ee96","ee98","ee99","ee9a","ee9b","ee9c","ee9e","eeb0","eeb1","eeb2","eeb3","eeb4","eeb6","eeb8","eeb9","eeba","eebb","eebc","eebe","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625"]]"#,
            ),
        ),
        (
            r"@A 90.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 90.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b801","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 90.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 90.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","dab","dae","daf","dba","dbb","dbe","dbf","dea","deb","dee","def","dfa","dfb","dfe","dff","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","8a82","8a83","8a88","8a89","8a8a","8a8b","8aa0","8aa1","8aa2","8aa3","8aa8","8aa9","8aaa","8aab","a000","a001","a002","a003","a008","a009","a00a","a00b","a020","a021","a022","a023","a028","a029","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","da86","da87","da8c","da8d","da8e","da8f","da92","da93","da96","da97","da98","da99","da9a","da9b","da9c","da9d","da9e","da9f","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dac2","dac3","dac6","dac7","dac8","dac9","daca","dacb","dacc","dacd","dace","dacf","dad2","dad3","dad6","dad7","dad8","dad9","dada","dadb","dadc","dadd","dade","dadf","db82","db83","db86","db87","db88","db89","db8a","db8b","db8c","db8d","db8e","db8f","db92","db93","db96","db97","db98","db99","db9a","db9b","db9c","db9d","db9e","db9f","dbc2","dbc3","dbc6","dbc7","dbc8","dbc9","dbca","dbcb","dbcc","dbcd","dbce","dbcf","dbd2","dbd3","dbd6","dbd7","dbd8","dbd9","dbda","dbdb","dbdc","dbdd","dbde","dbdf","de82","de83","de86","de87","de88","de89","de8a","de8b","de8c","de8d","de8e","de8f","de92","de93","de96","de97","de98","de99","de9a","de9b","de9c","de9d","de9e","de9f","dec2","dec3","dec6","dec7","dec8","dec9","deca","decb","decc","decd","dece","decf","ded2","ded3","ded6","ded7","ded8","ded9","deda","dedb","dedc","dedd","dede","dedf","df82","df83","df86","df87","df88","df89","df8a","df8b","df8c","df8d","df8e","df8f","df92","df93","df96","df97","df98","df99","df9a","df9b","df9c","df9d","df9e","df9f","dfc2","dfc3","dfc6","dfc7","dfc8","dfc9","dfca","dfcb","dfcc","dfcd","dfce","dfcf","dfd2","dfd3","dfd6","dfd7","dfd8","dfd9","dfda","dfdb","dfdc","dfdd","dfde","dfdf","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa94","fa95","fac0","fac1","fac4","fac5","fad0","fad1","fad4","fad5","fb80","fb81","fb84","fb85","fb90","fb91","fb94","fb95","fbc0","fbc1","fbc4","fbc5","fbd0","fbd1","fbd4","fbd5","fe80","fe81","fe84","fe85","fe90","fe91","fe94","fe95","fec0","fec1","fec4","fec5","fed0","fed1","fed4","fed5","ff80","ff81","ff84","ff85","ff90","ff91","ff94","ff95","ffc0","ffc1","ffc4","ffc5","ffd0","ffd1","ffd4","ffd5"]]"#,
            ),
        ),
        (
            r"@A 90.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 90.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["ad","af","b8","b9","ba","bb","ef","fa","fb","828","82a","880","8e9","8eb","8ec","8ed","8ee","8ef","8f8","8fa","a41","a44","a45","a50","a69","a6b","a6c","a6d","a6e","a6f","a78","a79","a7a","a7b","a7c","a7d","a7e","a7f","ac1","ac3","ac4","ac5","ac6","ac7","ac9","acb","acc","acd","ace","acf","ae1","ae3","ae4","ae5","ae6","ae7","ae9","aeb","aec","aed","aee","aef","b28","b29","b2a","b2b","b2c","b2d","b2e","b2f","b38","b39","b3a","b3b","b3c","b3d","b3e","b3f","b68","b6a","bc0","bc2","bc8","bca","be0","be2","be8","bea","d79","d7b","d7c","d7d","d7e","d7f","dd1","dd4","dd5","ecb","ece","ecf","eda","edb","ede","edf","ee1","ee3","ee4","ee5","ee6","ee7","ee9","eeb","eec","eed","eee","eef","f8a","f8b","f8e","f8f","f9a","f9b","f9e","f9f","fca","fe0","fe2","fe8","fea","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e36","8e37","8e38","8e39","8e3a","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a430","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a632","a633","a636","a637","a638","a639","a63a","a63b","a63c","a63d","a63e","a63f","a662","a663","a666","a667","a668","a669","a66a","a66b","a66c","a66d","a66e","a66f","a672","a673","a676","a677","a678","a679","a67a","a67b","a67c","a67d","a67e","a67f","a722","a723","a726","a727","a728","a729","a72a","a72b","a72c","a72d","a72e","a72f","a732","a733","a736","a737","a738","a739","a73a","a73b","a73c","a73d","a73e","a73f","a762","a763","a766","a767","a768","a769","a76a","a76b","a76c","a76d","a76e","a76f","a772","a773","a776","a777","a778","a779","a77a","a77b","a77c","a77d","a77e","a77f","b222","b223","b226","b227","b228","b229","b22a","b22b","b22c","b22d","b22e","b22f","b232","b233","b236","b237","b238","b239","b23a","b23b","b23c","b23d","b23e","b23f","b262","b263","b266","b267","b268","b269","b26a","b26b","b26c","b26d","b26e","b26f","b272","b273","b276","b277","b278","b279","b27a","b27b","b27c","b27d","b27e","b27f","b322","b323","b326","b327","b328","b329","b32a","b32b","b32c","b32d","b32e","b32f","b332","b333","b336","b337","b338","b339","b33a","b33b","b33c","b33d","b33e","b33f","b362","b363","b366","b367","b368","b369","b36a","b36b","b36c","b36d","b36e","b36f","b372","b373","b376","b377","b378","b379","b37a","b37b","b37c","b37d","b37e","b37f","b622","b623","b626","b627","b628","b629","b62a","b62b","b62c","b62d","b62e","b62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","ec9a","ec9b","ec9e","ec9f","ecca","eccb","ecce","eccf","ecda","ecdb","ecde","ecdf","ed8a","ed8b","ed8e","ed8f","ed9a","ed9b","ed9e","ed9f","edca","edcb","edce","edcf","edda","eddb","edde","eddf","f88a","f88b","f88e","f88f","f89a","f89b","f89e","f89f","f8ca","f8cb","f8ce","f8cf","f8da","f8db","f8de","f8df","f98a","f98b","f98e","f98f","f99a","f99b","f99e","f99f","f9ca","f9cb","f9ce","f9cf","f9da","f9db","f9de","f9df","fc8a","fc8b","fc8e","fc8f"]]"#,
            ),
        ),
        (
            r"@A 100.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 100.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b801","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 100.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 100.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","dab","dae","daf","dba","dbb","dbe","dbf","dea","deb","dee","def","dfa","dfb","dfe","dff","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","8a82","8a83","8a88","8a89","8a8a","8a8b","8aa0","8aa1","8aa2","8aa3","8aa8","8aa9","8aaa","8aab","a000","a001","a002","a003","a008","a009","a00a","a00b","a020","a021","a022","a023","a028","a029","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","da86","da87","da8c","da8d","da8e","da8f","da92","da93","da96","da97","da98","da99","da9a","da9b","da9c","da9d","da9e","da9f","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dac2","dac3","dac6","dac7","dac8","dac9","daca","dacb","dacc","dacd","dace","dacf","dad2","dad3","dad6","dad7","dad8","dad9","dada","dadb","dadc","dadd","dade","dadf","db82","db83","db86","db87","db88","db89","db8a","db8b","db8c","db8d","db8e","db8f","db92","db93","db96","db97","db98","db99","db9a","db9b","db9c","db9d","db9e","db9f","dbc2","dbc3","dbc6","dbc7","dbc8","dbc9","dbca","dbcb","dbcc","dbcd","dbce","dbcf","dbd2","dbd3","dbd6","dbd7","dbd8","dbd9","dbda","dbdb","dbdc","dbdd","dbde","dbdf","de82","de83","de86","de87","de88","de89","de8a","de8b","de8c","de8d","de8e","de8f","de92","de93","de96","de97","de98","de99","de9a","de9b","de9c","de9d","de9e","de9f","dec2","dec3","dec6","dec7","dec8","dec9","deca","decb","decc","decd","dece","decf","ded2","ded3","ded6","ded7","ded8","ded9","deda","dedb","dedc","dedd","dede","dedf","df82","df83","df86","df87","df88","df89","df8a","df8b","df8c","df8d","df8e","df8f","df92","df93","df96","df97","df98","df99","df9a","df9b","df9c","df9d","df9e","df9f","dfc2","dfc3","dfc6","dfc7","dfc8","dfc9","dfca","dfcb","dfcc","dfcd","dfce","dfcf","dfd2","dfd3","dfd6","dfd7","dfd8","dfd9","dfda","dfdb","dfdc","dfdd","dfde","dfdf","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa94","fa95","fac0","fac1","fac4","fac5","fad0","fad1","fad4","fad5","fb80","fb81","fb84","fb85","fb90","fb91","fb94","fb95","fbc0","fbc1","fbc4","fbc5","fbd0","fbd1","fbd4","fbd5","fe80","fe81","fe84","fe85","fe90","fe91","fe94","fe95","fec0","fec1","fec4","fec5","fed0","fed1","fed4","fed5","ff80","ff81","ff84","ff85","ff90","ff91","ff94","ff95","ffc0","ffc1","ffc4","ffc5","ffd0","ffd1","ffd4","ffd5"]]"#,
            ),
        ),
        (
            r"@A 100.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 100.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["ad","af","b8","b9","ba","bb","ef","fa","fb","828","82a","880","8e9","8eb","8ec","8ed","8ee","8ef","8f8","8fa","a41","a44","a45","a50","a69","a6b","a6c","a6d","a6e","a6f","a78","a79","a7a","a7b","a7c","a7d","a7e","a7f","ac1","ac3","ac4","ac5","ac6","ac7","ac9","acb","acc","acd","ace","acf","ae1","ae3","ae4","ae5","ae6","ae7","ae9","aeb","aec","aed","aee","aef","b28","b29","b2a","b2b","b2c","b2d","b2e","b2f","b38","b39","b3a","b3b","b3c","b3d","b3e","b3f","b68","b6a","bc0","bc2","bc8","bca","be0","be2","be8","bea","d79","d7b","d7c","d7d","d7e","d7f","dd1","dd4","dd5","ecb","ece","ecf","eda","edb","ede","edf","ee1","ee3","ee4","ee5","ee6","ee7","ee9","eeb","eec","eed","eee","eef","f8a","f8b","f8e","f8f","f9a","f9b","f9e","f9f","fca","fe0","fe2","fe8","fea","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e36","8e37","8e38","8e39","8e3a","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a430","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a632","a633","a636","a637","a638","a639","a63a","a63b","a63c","a63d","a63e","a63f","a662","a663","a666","a667","a668","a669","a66a","a66b","a66c","a66d","a66e","a66f","a672","a673","a676","a677","a678","a679","a67a","a67b","a67c","a67d","a67e","a67f","a722","a723","a726","a727","a728","a729","a72a","a72b","a72c","a72d","a72e","a72f","a732","a733","a736","a737","a738","a739","a73a","a73b","a73c","a73d","a73e","a73f","a762","a763","a766","a767","a768","a769","a76a","a76b","a76c","a76d","a76e","a76f","a772","a773","a776","a777","a778","a779","a77a","a77b","a77c","a77d","a77e","a77f","b222","b223","b226","b227","b228","b229","b22a","b22b","b22c","b22d","b22e","b22f","b232","b233","b236","b237","b238","b239","b23a","b23b","b23c","b23d","b23e","b23f","b262","b263","b266","b267","b268","b269","b26a","b26b","b26c","b26d","b26e","b26f","b272","b273","b276","b277","b278","b279","b27a","b27b","b27c","b27d","b27e","b27f","b322","b323","b326","b327","b328","b329","b32a","b32b","b32c","b32d","b32e","b32f","b332","b333","b336","b337","b338","b339","b33a","b33b","b33c","b33d","b33e","b33f","b362","b363","b366","b367","b368","b369","b36a","b36b","b36c","b36d","b36e","b36f","b372","b373","b376","b377","b378","b379","b37a","b37b","b37c","b37d","b37e","b37f","b622","b623","b626","b627","b628","b629","b62a","b62b","b62c","b62d","b62e","b62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","ec9a","ec9b","ec9e","ec9f","ecca","eccb","ecce","eccf","ecda","ecdb","ecde","ecdf","ed8a","ed8b","ed8e","ed8f","ed9a","ed9b","ed9e","ed9f","edca","edcb","edce","edcf","edda","eddb","edde","eddf","f88a","f88b","f88e","f88f","f89a","f89b","f89e","f89f","f8ca","f8cb","f8ce","f8cf","f8da","f8db","f8de","f8df","f98a","f98b","f98e","f98f","f99a","f99b","f99e","f99f","f9ca","f9cb","f9ce","f9cf","f9da","f9db","f9de","f9df","fc8a","fc8b","fc8e","fc8f"]]"#,
            ),
        ),
        (
            r"@A 170.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 170.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b801","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 170.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 170.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","dab","dae","daf","dba","dbb","dbe","dbf","dea","deb","dee","def","dfa","dfb","dfe","dff","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","8a82","8a83","8a88","8a89","8a8a","8a8b","8aa0","8aa1","8aa2","8aa3","8aa8","8aa9","8aaa","8aab","a000","a001","a002","a003","a008","a009","a00a","a00b","a020","a021","a022","a023","a028","a029","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","da86","da87","da8c","da8d","da8e","da8f","da92","da93","da96","da97","da98","da99","da9a","da9b","da9c","da9d","da9e","da9f","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dac2","dac3","dac6","dac7","dac8","dac9","daca","dacb","dacc","dacd","dace","dacf","dad2","dad3","dad6","dad7","dad8","dad9","dada","dadb","dadc","dadd","dade","dadf","db82","db83","db86","db87","db88","db89","db8a","db8b","db8c","db8d","db8e","db8f","db92","db93","db96","db97","db98","db99","db9a","db9b","db9c","db9d","db9e","db9f","dbc2","dbc3","dbc6","dbc7","dbc8","dbc9","dbca","dbcb","dbcc","dbcd","dbce","dbcf","dbd2","dbd3","dbd6","dbd7","dbd8","dbd9","dbda","dbdb","dbdc","dbdd","dbde","dbdf","de82","de83","de86","de87","de88","de89","de8a","de8b","de8c","de8d","de8e","de8f","de92","de93","de96","de97","de98","de99","de9a","de9b","de9c","de9d","de9e","de9f","dec2","dec3","dec6","dec7","dec8","dec9","deca","decb","decc","decd","dece","decf","ded2","ded3","ded6","ded7","ded8","ded9","deda","dedb","dedc","dedd","dede","dedf","df82","df83","df86","df87","df88","df89","df8a","df8b","df8c","df8d","df8e","df8f","df92","df93","df96","df97","df98","df99","df9a","df9b","df9c","df9d","df9e","df9f","dfc2","dfc3","dfc6","dfc7","dfc8","dfc9","dfca","dfcb","dfcc","dfcd","dfce","dfcf","dfd2","dfd3","dfd6","dfd7","dfd8","dfd9","dfda","dfdb","dfdc","dfdd","dfde","dfdf","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa94","fa95","fac0","fac1","fac4","fac5","fad0","fad1","fad4","fad5","fb80","fb81","fb84","fb85","fb90","fb91","fb94","fb95","fbc0","fbc1","fbc4","fbc5","fbd0","fbd1","fbd4","fbd5","fe80","fe81","fe84","fe85","fe90","fe91","fe94","fe95","fec0","fec1","fec4","fec5","fed0","fed1","fed4","fed5","ff80","ff81","ff84","ff85","ff90","ff91","ff94","ff95","ffc0","ffc1","ffc4","ffc5","ffd0","ffd1","ffd4","ffd5"]]"#,
            ),
        ),
        (
            r"@A 170.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 170.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["ad","af","b8","b9","ba","bb","ef","fa","fb","828","82a","880","8e9","8eb","8ec","8ed","8ee","8ef","8f8","8fa","a41","a44","a45","a50","a69","a6b","a6c","a6d","a6e","a6f","a78","a79","a7a","a7b","a7c","a7d","a7e","a7f","ac1","ac3","ac4","ac5","ac6","ac7","ac9","acb","acc","acd","ace","acf","ae1","ae3","ae4","ae5","ae6","ae7","ae9","aeb","aec","aed","aee","aef","b28","b29","b2a","b2b","b2c","b2d","b2e","b2f","b38","b39","b3a","b3b","b3c","b3d","b3e","b3f","b68","b6a","bc0","bc2","bc8","bca","be0","be2","be8","bea","d79","d7b","d7c","d7d","d7e","d7f","dd1","dd4","dd5","ecb","ece","ecf","eda","edb","ede","edf","ee1","ee3","ee4","ee5","ee6","ee7","ee9","eeb","eec","eed","eee","eef","f8a","f8b","f8e","f8f","f9a","f9b","f9e","f9f","fca","fe0","fe2","fe8","fea","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e36","8e37","8e38","8e39","8e3a","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a430","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a632","a633","a636","a637","a638","a639","a63a","a63b","a63c","a63d","a63e","a63f","a662","a663","a666","a667","a668","a669","a66a","a66b","a66c","a66d","a66e","a66f","a672","a673","a676","a677","a678","a679","a67a","a67b","a67c","a67d","a67e","a67f","a722","a723","a726","a727","a728","a729","a72a","a72b","a72c","a72d","a72e","a72f","a732","a733","a736","a737","a738","a739","a73a","a73b","a73c","a73d","a73e","a73f","a762","a763","a766","a767","a768","a769","a76a","a76b","a76c","a76d","a76e","a76f","a772","a773","a776","a777","a778","a779","a77a","a77b","a77c","a77d","a77e","a77f","b222","b223","b226","b227","b228","b229","b22a","b22b","b22c","b22d","b22e","b22f","b232","b233","b236","b237","b238","b239","b23a","b23b","b23c","b23d","b23e","b23f","b262","b263","b266","b267","b268","b269","b26a","b26b","b26c","b26d","b26e","b26f","b272","b273","b276","b277","b278","b279","b27a","b27b","b27c","b27d","b27e","b27f","b322","b323","b326","b327","b328","b329","b32a","b32b","b32c","b32d","b32e","b32f","b332","b333","b336","b337","b338","b339","b33a","b33b","b33c","b33d","b33e","b33f","b362","b363","b366","b367","b368","b369","b36a","b36b","b36c","b36d","b36e","b36f","b372","b373","b376","b377","b378","b379","b37a","b37b","b37c","b37d","b37e","b37f","b622","b623","b626","b627","b628","b629","b62a","b62b","b62c","b62d","b62e","b62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","ec9a","ec9b","ec9e","ec9f","ecca","eccb","ecce","eccf","ecda","ecdb","ecde","ecdf","ed8a","ed8b","ed8e","ed8f","ed9a","ed9b","ed9e","ed9f","edca","edcb","edce","edcf","edda","eddb","edde","eddf","f88a","f88b","f88e","f88f","f89a","f89b","f89e","f89f","f8ca","f8cb","f8ce","f8cf","f8da","f8db","f8de","f8df","f98a","f98b","f98e","f98f","f99a","f99b","f99e","f99f","f9ca","f9cb","f9ce","f9cf","f9da","f9db","f9de","f9df","fc8a","fc8b","fc8e","fc8f"]]"#,
            ),
        ),
        (
            r"@A -100.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M -100.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","bb00","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A -100.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M -100.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b884","b885","b890","b891","b894","b895","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A -100.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M -100.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","ec9","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec36","ec37","ec38","ec39","ec3a","ec3b","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee30","ee31","ee34","ee35","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
        (
            r"@A -179.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M -179.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b803","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","ba01","ba04","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A -179.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M -179.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A -179.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M -179.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","8ec","8ed","8ee","8ef","8f8","8fa","a44","a45","a50","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","8e36","8e37","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8e94","8e95","8e96","8e97","8e9c","8e9d","8e9e","8e9f","8eb4","8eb5","8eb6","8eb7","8ebc","8ebd","8ebe","8ebf","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a414","a415","a416","a417","a41c","a41d","a41e","a41f","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec38","ec39","ec3a","ec3b","ec90","ec91","ec92","ec93","ec98","ec99","ec9a","ec9b","ecb0","ecb1","ecb2","ecb3","ecb8","ecb9","ecba","ecbb","ee10","ee11","ee12","ee13","ee18","ee19","ee1a","ee1b","ee30","ee31","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
        (
            r"@A 179.5 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 179.5 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b801","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 179.5 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 179.5 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","dab","dae","daf","dba","dbb","dbe","dbf","dea","deb","dee","def","dfa","dfb","dfe","dff","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","8a82","8a83","8a88","8a89","8a8a","8a8b","8aa0","8aa1","8aa2","8aa3","8aa8","8aa9","8aaa","8aab","a000","a001","a002","a003","a008","a009","a00a","a00b","a020","a021","a022","a023","a028","a029","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","da86","da87","da8c","da8d","da8e","da8f","da92","da93","da96","da97","da98","da99","da9a","da9b","da9c","da9d","da9e","da9f","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dac2","dac3","dac6","dac7","dac8","dac9","daca","dacb","dacc","dacd","dace","dacf","dad2","dad3","dad6","dad7","dad8","dad9","dada","dadb","dadc","dadd","dade","dadf","db82","db83","db86","db87","db88","db89","db8a","db8b","db8c","db8d","db8e","db8f","db92","db93","db96","db97","db98","db99","db9a","db9b","db9c","db9d","db9e","db9f","dbc2","dbc3","dbc6","dbc7","dbc8","dbc9","dbca","dbcb","dbcc","dbcd","dbce","dbcf","dbd2","dbd3","dbd6","dbd7","dbd8","dbd9","dbda","dbdb","dbdc","dbdd","dbde","dbdf","de82","de83","de86","de87","de88","de89","de8a","de8b","de8c","de8d","de8e","de8f","de92","de93","de96","de97","de98","de99","de9a","de9b","de9c","de9d","de9e","de9f","dec2","dec3","dec6","dec7","dec8","dec9","deca","decb","decc","decd","dece","decf","ded2","ded3","ded6","ded7","ded8","ded9","deda","dedb","dedc","dedd","dede","dedf","df82","df83","df86","df87","df88","df89","df8a","df8b","df8c","df8d","df8e","df8f","df92","df93","df96","df97","df98","df99","df9a","df9b","df9c","df9d","df9e","df9f","dfc2","dfc3","dfc6","dfc7","dfc8","dfc9","dfca","dfcb","dfcc","dfcd","dfce","dfcf","dfd2","dfd3","dfd6","dfd7","dfd8","dfd9","dfda","dfdb","dfdc","dfdd","dfde","dfdf","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa94","fa95","fac0","fac1","fac4","fac5","fad0","fad1","fad4","fad5","fb80","fb81","fb84","fb85","fb90","fb91","fb94","fb95","fbc0","fbc1","fbc4","fbc5","fbd0","fbd1","fbd4","fbd5","fe80","fe81","fe84","fe85","fe90","fe91","fe94","fe95","fec0","fec1","fec4","fec5","fed0","fed1","fed4","fed5","ff80","ff81","ff84","ff85","ff90","ff91","ff94","ff95","ffc0","ffc1","ffc4","ffc5","ffd0","ffd1","ffd4","ffd5"]]"#,
            ),
        ),
        (
            r"@A 179.5 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 179.5 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["ad","af","b8","b9","ba","bb","ef","fa","fb","828","82a","880","8e9","8eb","8ec","8ed","8ee","8ef","8f8","8fa","a41","a44","a45","a50","a69","a6b","a6c","a6d","a6e","a6f","a78","a79","a7a","a7b","a7c","a7d","a7e","a7f","ac1","ac3","ac4","ac5","ac6","ac7","ac9","acb","acc","acd","ace","acf","ae1","ae3","ae4","ae5","ae6","ae7","ae9","aeb","aec","aed","aee","aef","b28","b29","b2a","b2b","b2c","b2d","b2e","b2f","b38","b39","b3a","b3b","b3c","b3d","b3e","b3f","b68","b6a","bc0","bc2","bc8","bca","be0","be2","be8","bea","d79","d7b","d7c","d7d","d7e","d7f","dd1","dd4","dd5","ecb","ece","ecf","eda","edb","ede","edf","ee1","ee3","ee4","ee5","ee6","ee7","ee9","eeb","eec","eed","eee","eef","f8a","f8b","f8e","f8f","f9a","f9b","f9e","f9f","fca","fe0","fe2","fe8","fea","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e36","8e37","8e38","8e39","8e3a","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a430","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a632","a633","a636","a637","a638","a639","a63a","a63b","a63c","a63d","a63e","a63f","a662","a663","a666","a667","a668","a669","a66a","a66b","a66c","a66d","a66e","a66f","a672","a673","a676","a677","a678","a679","a67a","a67b","a67c","a67d","a67e","a67f","a722","a723","a726","a727","a728","a729","a72a","a72b","a72c","a72d","a72e","a72f","a732","a733","a736","a737","a738","a739","a73a","a73b","a73c","a73d","a73e","a73f","a762","a763","a766","a767","a768","a769","a76a","a76b","a76c","a76d","a76e","a76f","a772","a773","a776","a777","a778","a779","a77a","a77b","a77c","a77d","a77e","a77f","b222","b223","b226","b227","b228","b229","b22a","b22b","b22c","b22d","b22e","b22f","b232","b233","b236","b237","b238","b239","b23a","b23b","b23c","b23d","b23e","b23f","b262","b263","b266","b267","b268","b269","b26a","b26b","b26c","b26d","b26e","b26f","b272","b273","b276","b277","b278","b279","b27a","b27b","b27c","b27d","b27e","b27f","b322","b323","b326","b327","b328","b329","b32a","b32b","b32c","b32d","b32e","b32f","b332","b333","b336","b337","b338","b339","b33a","b33b","b33c","b33d","b33e","b33f","b362","b363","b366","b367","b368","b369","b36a","b36b","b36c","b36d","b36e","b36f","b372","b373","b376","b377","b378","b379","b37a","b37b","b37c","b37d","b37e","b37f","b622","b623","b626","b627","b628","b629","b62a","b62b","b62c","b62d","b62e","b62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","ec9a","ec9b","ec9e","ec9f","ecca","eccb","ecce","eccf","ecda","ecdb","ecde","ecdf","ed8a","ed8b","ed8e","ed8f","ed9a","ed9b","ed9e","ed9f","edca","edcb","edce","edcf","edda","eddb","edde","eddf","f88a","f88b","f88e","f88f","f89a","f89b","f89e","f89f","f8ca","f8cb","f8ce","f8cf","f8da","f8db","f8de","f8df","f98a","f98b","f98e","f98f","f99a","f99b","f99e","f99f","f9ca","f9cb","f9ce","f9cf","f9da","f9db","f9de","f9df","fc8a","fc8b","fc8e","fc8f"]]"#,
            ),
        ),
        (
            r"@A 180.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 180.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","ba01","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 180.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 180.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b28c","b28e","b2a4","b2a6","b2ac","b2ae","b804","b806","b80c","b80e","b824","b826","b82c","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b887","b88d","b88f","b8a5","b8a7","b8ad","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba05","ba07","ba0d","ba0f","ba25","ba27","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 180.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 180.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","8ec","8ed","8ee","8ef","8f8","8fa","a44","a45","a50","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","8e33","8e36","8e37","8e39","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8e91","8e93","8e94","8e95","8e96","8e97","8e99","8e9b","8e9c","8e9d","8e9e","8e9f","8eb1","8eb3","8eb4","8eb5","8eb6","8eb7","8eb9","8ebb","8ebc","8ebd","8ebe","8ebf","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a411","a413","a414","a415","a416","a417","a419","a41b","a41c","a41d","a41e","a41f","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec38","ec3a","ec90","ec92","ec98","ec9a","ecb0","ecb2","ecb8","ecba","ee10","ee12","ee18","ee1a","ee30","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
        (
            r"@A 360.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M 360.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","bb00","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 360.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M 360.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b884","b885","b890","b891","b894","b895","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba87","ba8d","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa5","baa7","baad","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","da86","da8c","da8e","daa4","daa6","daac","daae","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f004","f006","f00c","f00e","f024","f026","f02c","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa87","fa8d","fa8f","faa5","faa7","faad","faaf"]]"#,
            ),
        ),
        (
            r"@A 360.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M 360.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","828","82a","880","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc4","cc5","cc6","cc7","ccc","ccd","cce","ccf","ce4","ce5","ce6","ce7","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","d79","d7b","d7c","d7d","d7e","d7f","dc0","dc2","dc8","dca","dd1","dd4","dd5","de0","de2","de8","dea","e44","e45","e46","e47","e4c","e4d","e4e","e4f","e64","e65","e70","e71","e74","e75","ec9","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","a632","a638","a63a","a690","a692","a698","a69a","a6b0","a6b2","a6b8","a6ba","ac10","ac12","ac18","ac1a","ac30","ac32","ac38","ac3a","ac90","ac92","ac98","ac9a","acb0","acb2","acb8","acba","ae10","ae12","ae18","ae1a","ae30","ae32","ae38","ae3a","ae90","ae92","ae98","ae9a","aeb0","aeb2","aeb8","aeba","c633","c636","c637","c639","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c691","c693","c694","c695","c696","c697","c699","c69b","c69c","c69d","c69e","c69f","c6b1","c6b3","c6b4","c6b5","c6b6","c6b7","c6b9","c6bb","c6bc","c6bd","c6be","c6bf","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","cc11","cc13","cc14","cc15","cc16","cc17","cc19","cc1b","cc1c","cc1d","cc1e","cc1f","cc31","cc33","cc34","cc35","cc36","cc37","cc39","cc3b","cc3c","cc3d","cc3e","cc3f","cc91","cc93","cc94","cc95","cc96","cc97","cc99","cc9b","cc9c","cc9d","cc9e","cc9f","ccb1","ccb3","ccb4","ccb5","ccb6","ccb7","ccb9","ccbb","ccbc","ccbd","ccbe","ccbf","ce11","ce13","ce14","ce15","ce16","ce17","ce19","ce1b","ce1c","ce1d","ce1e","ce1f","ce31","ce33","ce34","ce35","ce36","ce37","ce39","ce3b","ce3c","ce3d","ce3e","ce3f","ce91","ce93","ce94","ce95","ce96","ce97","ce99","ce9b","ce9c","ce9d","ce9e","ce9f","ceb1","ceb3","ceb4","ceb5","ceb6","ceb7","ceb9","cebb","cebc","cebd","cebe","cebf","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","e411","e413","e414","e415","e416","e417","e419","e41b","e41c","e41d","e41e","e41f","e431","e433","e434","e435","e436","e437","e439","e43b","e43c","e43d","e43e","e43f","e491","e493","e494","e495","e496","e497","e499","e49b","e49c","e49d","e49e","e49f","e4b1","e4b3","e4b4","e4b5","e4b6","e4b7","e4b9","e4bb","e4bc","e4bd","e4be","e4bf","e611","e613","e614","e615","e616","e617","e619","e61b","e61c","e61d","e61e","e61f","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec36","ec37","ec38","ec39","ec3a","ec3b","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ec9a","ecb0","ecb2","ecb8","ecba","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee10","ee12","ee18","ee1a","ee30","ee31","ee32","ee34","ee35","ee38","ee3a","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ee90","ee92","ee98","ee9a","eeb0","eeb2","eeb8","eeba","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625"]]"#,
            ),
        ),
        (
            r"@A -540.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M -540.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","ba01","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A -540.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M -540.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b28c","b28e","b2a4","b2a6","b2ac","b2ae","b804","b806","b80c","b80e","b824","b826","b82c","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b887","b88d","b88f","b8a5","b8a7","b8ad","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba05","ba07","ba0d","ba0f","ba25","ba27","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A -540.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M -540.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","8ec","8ed","8ee","8ef","8f8","8fa","a44","a45","a50","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","8e33","8e36","8e37","8e39","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8e91","8e93","8e94","8e95","8e96","8e97","8e99","8e9b","8e9c","8e9d","8e9e","8e9f","8eb1","8eb3","8eb4","8eb5","8eb6","8eb7","8eb9","8ebb","8ebc","8ebd","8ebe","8ebf","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a411","a413","a414","a415","a416","a417","a419","a41b","a41c","a41d","a41e","a41f","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec38","ec3a","ec90","ec92","ec98","ec9a","ecb0","ecb2","ecb8","ecba","ee10","ee12","ee18","ee1a","ee30","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
        (
            r"@A 1000.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M 1000.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","bb00","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 1000.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M 1000.0 -5.0 3.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b884","b885","b890","b891","b894","b895","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 1000.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M 1000.0 30.0 -40.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","ec9","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec36","ec37","ec38","ec39","ec3a","ec3b","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee30","ee31","ee34","ee35","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
        (
            r"@A 0.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 0.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 0.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 0.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 0.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed07"]]"#),
        ),
        (
            r"@M 0.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["a6","ac","ae","c7","cd","cf","d2","d3","d7","d8","d9","da","db","e5","ec","ee","f0","f1","ed07"]]"#,
            ),
        ),
        (
            r"@A 2.35 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 2.35 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
    ]);
}

#[test]
fn shift_6() {
    check(&[
        (
            r"@A 2.35 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 2.35 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 2.35 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed07"]]"#),
        ),
        (
            r"@M 2.35 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["a6","ac","ae","c7","cd","cf","d2","d3","d7","d8","d9","da","db","e5","ec","ee","f0","f1","ed07"]]"#,
            ),
        ),
        (
            r"@A 90.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 90.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 90.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 90.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 90.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["8fad"]]"#),
        ),
        (
            r"@M 90.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["8e","a6","a7","ac","ad","ae","af","b2","b3","b8","b9","ba","bb","d7","ee","ef","fa","fb","8fad"]]"#,
            ),
        ),
        (
            r"@A 100.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 100.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 100.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 100.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 100.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["8fad"]]"#),
        ),
        (
            r"@M 100.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["8e","a6","a7","ac","ad","ae","af","b2","b3","b8","b9","ba","bb","d7","ee","ef","fa","fb","8fad"]]"#,
            ),
        ),
        (
            r"@A 170.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 170.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 170.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 170.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 170.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["8fad"]]"#),
        ),
        (
            r"@M 170.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["8e","a6","a7","ac","ad","ae","af","b2","b3","b8","b9","ba","bb","d7","ee","ef","fa","fb","8fad"]]"#,
            ),
        ),
        (
            r"@A -100.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M -100.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A -100.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M -100.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A -100.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed07"]]"#),
        ),
        (
            r"@M -100.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["c6","c7","cc","cd","ce","cf","d2","d3","d8","d9","da","db","e4","e5","ec","f0","f1","ff","ed07"]]"#,
            ),
        ),
        (
            r"@A -179.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M -179.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A -179.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M -179.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A -179.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["8fad"]]"#),
        ),
        (
            r"@M -179.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["c6","c7","cc","cd","ce","cf","d2","d3","d8","d9","da","db","e4","e5","ec","f0","f1","ff","8fad"]]"#,
            ),
        ),
        (
            r"@A 179.5 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 179.5 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 179.5 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 179.5 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 179.5 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["8fad"]]"#),
        ),
        (
            r"@M 179.5 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["8e","a6","a7","ac","ad","ae","af","b2","b3","b8","b9","ba","bb","d7","ee","ef","fa","fb","8fad"]]"#,
            ),
        ),
        (
            r"@A 180.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 180.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 180.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 180.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 180.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["8fad"]]"#),
        ),
        (
            r"@M 180.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["c6","c7","cc","cd","ce","cf","d2","d3","d8","d9","da","db","e4","e5","ec","f0","f1","ff","8fad"]]"#,
            ),
        ),
        (
            r"@A 360.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 360.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 360.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 360.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 360.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed07"]]"#),
        ),
        (
            r"@M 360.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["a6","ac","ae","c7","cd","cf","d2","d3","d7","d8","d9","da","db","e5","ec","ee","f0","f1","ed07"]]"#,
            ),
        ),
        (
            r"@A -540.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M -540.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A -540.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M -540.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A -540.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["8fad"]]"#),
        ),
        (
            r"@M -540.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["c6","c7","cc","cd","ce","cf","d2","d3","d8","d9","da","db","e4","e5","ec","f0","f1","ff","8fad"]]"#,
            ),
        ),
        (
            r"@A 1000.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 1000.0 0.703125 1.40625 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 1000.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@M 1000.0 -5.0 3.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f","a1","b8","b9ad"]]"#),
        ),
        (
            r"@A 1000.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed07"]]"#),
        ),
        (
            r"@M 1000.0 30.0 -40.0 4 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["c6","c7","cc","cd","ce","cf","d2","d3","d8","d9","da","db","e4","e5","ec","f0","f1","ff","ed07"]]"#,
            ),
        ),
        (
            r"@A 0.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M 0.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","bb00","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 0.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M 0.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b884","b885","b890","b891","b894","b895","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba87","ba8d","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa5","baa7","baad","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","da86","da8c","da8e","daa4","daa6","daac","daae","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f004","f006","f00c","f00e","f024","f026","f02c","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa87","fa8d","fa8f","faa5","faa7","faad","faaf"]]"#,
            ),
        ),
        (
            r"@A 0.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M 0.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","828","82a","880","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc4","cc5","cc6","cc7","ccc","ccd","cce","ccf","ce4","ce5","ce6","ce7","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","d79","d7b","d7c","d7d","d7e","d7f","dc0","dc2","dc8","dca","dd1","dd4","dd5","de0","de2","de8","dea","e44","e45","e46","e47","e4c","e4d","e4e","e4f","e64","e65","e70","e71","e74","e75","ec9","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","a632","a638","a63a","a690","a692","a698","a69a","a6b0","a6b2","a6b8","a6ba","ac10","ac12","ac18","ac1a","ac30","ac32","ac38","ac3a","ac90","ac92","ac98","ac9a","acb0","acb2","acb8","acba","ae10","ae12","ae18","ae1a","ae30","ae32","ae38","ae3a","ae90","ae92","ae98","ae9a","aeb0","aeb2","aeb8","aeba","c633","c636","c637","c639","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c691","c693","c694","c695","c696","c697","c699","c69b","c69c","c69d","c69e","c69f","c6b1","c6b3","c6b4","c6b5","c6b6","c6b7","c6b9","c6bb","c6bc","c6bd","c6be","c6bf","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","cc11","cc13","cc14","cc15","cc16","cc17","cc19","cc1b","cc1c","cc1d","cc1e","cc1f","cc31","cc33","cc34","cc35","cc36","cc37","cc39","cc3b","cc3c","cc3d","cc3e","cc3f","cc91","cc93","cc94","cc95","cc96","cc97","cc99","cc9b","cc9c","cc9d","cc9e","cc9f","ccb1","ccb3","ccb4","ccb5","ccb6","ccb7","ccb9","ccbb","ccbc","ccbd","ccbe","ccbf","ce11","ce13","ce14","ce15","ce16","ce17","ce19","ce1b","ce1c","ce1d","ce1e","ce1f","ce31","ce33","ce34","ce35","ce36","ce37","ce39","ce3b","ce3c","ce3d","ce3e","ce3f","ce91","ce93","ce94","ce95","ce96","ce97","ce99","ce9b","ce9c","ce9d","ce9e","ce9f","ceb1","ceb3","ceb4","ceb5","ceb6","ceb7","ceb9","cebb","cebc","cebd","cebe","cebf","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","e411","e413","e414","e415","e416","e417","e419","e41b","e41c","e41d","e41e","e41f","e431","e433","e434","e435","e436","e437","e439","e43b","e43c","e43d","e43e","e43f","e491","e493","e494","e495","e496","e497","e499","e49b","e49c","e49d","e49e","e49f","e4b1","e4b3","e4b4","e4b5","e4b6","e4b7","e4b9","e4bb","e4bc","e4bd","e4be","e4bf","e611","e613","e614","e615","e616","e617","e619","e61b","e61c","e61d","e61e","e61f","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec36","ec37","ec38","ec39","ec3a","ec3b","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ec9a","ecb0","ecb2","ecb8","ecba","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee10","ee12","ee18","ee1a","ee30","ee31","ee32","ee34","ee35","ee38","ee3a","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ee90","ee92","ee98","ee9a","eeb0","eeb2","eeb8","eeba","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625"]]"#,
            ),
        ),
        (
            r"@A 2.35 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M 2.35 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b2ab","b2ae","b801","b803","b804","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","baae","baaf","bb00","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 2.35 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M 2.35 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fac","fad","fae","faf","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b82e","b82f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b890","b891","b894","b895","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba93","ba96","ba97","ba99","ba9b","ba9c","ba9d","ba9e","ba9f","bab1","bab3","bab4","bab5","bab6","bab7","bab9","babb","babc","babd","babe","babf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","da86","da87","da8c","da8d","da8e","da8f","da92","da98","da9a","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dab0","dab2","dab8","daba","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f010","f012","f018","f01a","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f030","f032","f038","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa93","fa94","fa95","fa96","fa97","fa99","fa9b","fa9c","fa9d","fa9e","fa9f","fab1","fab3","fab4","fab5","fab6","fab7","fab9","fabb","fabc","fabd","fabe","fabf"]]"#,
            ),
        ),
        (
            r"@A 2.35 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M 2.35 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","828","82a","880","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc4","cc5","cc6","cc7","ccc","ccd","cce","ccf","ce4","ce5","ce6","ce7","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","d79","d7b","d7c","d7d","d7e","d7f","dc0","dc2","dc8","dca","dd1","dd4","dd5","de0","de2","de8","dea","e44","e45","e46","e47","e4c","e4d","e4e","e4f","e64","e65","e70","e71","e74","e75","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e38","8e39","8e3a","8e3b","8e90","8e91","8e92","8e93","8e98","8e99","8e9a","8e9b","8eb0","8eb1","8eb2","8eb3","8eb8","8eb9","8eba","8ebb","a410","a411","a412","a413","a418","a419","a41a","a41b","a430","a431","a632","a633","a636","a638","a639","a63a","a63b","a63c","a63e","a690","a691","a692","a693","a694","a696","a698","a699","a69a","a69b","a69c","a69e","a6b0","a6b1","a6b2","a6b3","a6b4","a6b6","a6b8","a6b9","a6ba","a6bb","a6bc","a6be","ac10","ac11","ac12","ac13","ac14","ac16","ac18","ac19","ac1a","ac1b","ac1c","ac1e","ac30","ac31","ac32","ac33","ac34","ac36","ac38","ac39","ac3a","ac3b","ac3c","ac3e","ac90","ac91","ac92","ac93","ac94","ac96","ac98","ac99","ac9a","ac9b","ac9c","ac9e","acb0","acb1","acb2","acb3","acb4","acb6","acb8","acb9","acba","acbb","acbc","acbe","ae10","ae11","ae12","ae13","ae14","ae16","ae18","ae19","ae1a","ae1b","ae1c","ae1e","ae30","ae31","ae32","ae33","ae34","ae36","ae38","ae39","ae3a","ae3b","ae3c","ae3e","ae90","ae91","ae92","ae93","ae94","ae96","ae98","ae99","ae9a","ae9b","ae9c","ae9e","aeb0","aeb1","aeb2","aeb3","aeb4","aeb6","aeb8","aeb9","aeba","aebb","aebc","aebe","c637","c63d","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c695","c697","c69d","c69f","c6b5","c6b7","c6bd","c6bf","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","cc15","cc17","cc1d","cc1f","cc35","cc37","cc3d","cc3f","cc95","cc97","cc9d","cc9f","ccb5","ccb7","ccbd","ccbf","ce15","ce17","ce1d","ce1f","ce35","ce37","ce3d","ce3f","ce95","ce97","ce9d","ce9f","ceb5","ceb7","cebd","cebf","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","e415","e417","e41d","e41f","e435","e437","e43d","e43f","e495","e497","e49d","e49f","e4b5","e4b7","e4bd","e4bf","e615","e617","e61d","e61f","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec36","ec37","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ec94","ec95","ec96","ec97","ec9a","ec9b","ec9c","ec9d","ec9e","ec9f","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee30","ee31","ee32","ee33","ee34","ee35","ee36","ee38","ee39","ee3a","ee3b","ee3c","ee3e","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ee90","ee91","ee92","ee93","ee94","ee96","ee98","ee99","ee9a","ee9b","ee9c","ee9e","eeb0","eeb1","eeb2","eeb3","eeb4","eeb6","eeb8","eeb9","eeba","eebb","eebc","eebe","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625"]]"#,
            ),
        ),
        (
            r"@A 90.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 90.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b801","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 90.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 90.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","dab","dae","daf","dba","dbb","dbe","dbf","dea","deb","dee","def","dfa","dfb","dfe","dff","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","8a82","8a83","8a88","8a89","8a8a","8a8b","8aa0","8aa1","8aa2","8aa3","8aa8","8aa9","8aaa","8aab","a000","a001","a002","a003","a008","a009","a00a","a00b","a020","a021","a022","a023","a028","a029","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","da86","da87","da8c","da8d","da8e","da8f","da92","da93","da96","da97","da98","da99","da9a","da9b","da9c","da9d","da9e","da9f","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dac2","dac3","dac6","dac7","dac8","dac9","daca","dacb","dacc","dacd","dace","dacf","dad2","dad3","dad6","dad7","dad8","dad9","dada","dadb","dadc","dadd","dade","dadf","db82","db83","db86","db87","db88","db89","db8a","db8b","db8c","db8d","db8e","db8f","db92","db93","db96","db97","db98","db99","db9a","db9b","db9c","db9d","db9e","db9f","dbc2","dbc3","dbc6","dbc7","dbc8","dbc9","dbca","dbcb","dbcc","dbcd","dbce","dbcf","dbd2","dbd3","dbd6","dbd7","dbd8","dbd9","dbda","dbdb","dbdc","dbdd","dbde","dbdf","de82","de83","de86","de87","de88","de89","de8a","de8b","de8c","de8d","de8e","de8f","de92","de93","de96","de97","de98","de99","de9a","de9b","de9c","de9d","de9e","de9f","dec2","dec3","dec6","dec7","dec8","dec9","deca","decb","decc","decd","dece","decf","ded2","ded3","ded6","ded7","ded8","ded9","deda","dedb","dedc","dedd","dede","dedf","df82","df83","df86","df87","df88","df89","df8a","df8b","df8c","df8d","df8e","df8f","df92","df93","df96","df97","df98","df99","df9a","df9b","df9c","df9d","df9e","df9f","dfc2","dfc3","dfc6","dfc7","dfc8","dfc9","dfca","dfcb","dfcc","dfcd","dfce","dfcf","dfd2","dfd3","dfd6","dfd7","dfd8","dfd9","dfda","dfdb","dfdc","dfdd","dfde","dfdf","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa94","fa95","fac0","fac1","fac4","fac5","fad0","fad1","fad4","fad5","fb80","fb81","fb84","fb85","fb90","fb91","fb94","fb95","fbc0","fbc1","fbc4","fbc5","fbd0","fbd1","fbd4","fbd5","fe80","fe81","fe84","fe85","fe90","fe91","fe94","fe95","fec0","fec1","fec4","fec5","fed0","fed1","fed4","fed5","ff80","ff81","ff84","ff85","ff90","ff91","ff94","ff95","ffc0","ffc1","ffc4","ffc5","ffd0","ffd1","ffd4","ffd5"]]"#,
            ),
        ),
        (
            r"@A 90.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 90.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["ad","af","b8","b9","ba","bb","ef","fa","fb","828","82a","880","8e9","8eb","8ec","8ed","8ee","8ef","8f8","8fa","a41","a44","a45","a50","a69","a6b","a6c","a6d","a6e","a6f","a78","a79","a7a","a7b","a7c","a7d","a7e","a7f","ac1","ac3","ac4","ac5","ac6","ac7","ac9","acb","acc","acd","ace","acf","ae1","ae3","ae4","ae5","ae6","ae7","ae9","aeb","aec","aed","aee","aef","b28","b29","b2a","b2b","b2c","b2d","b2e","b2f","b38","b39","b3a","b3b","b3c","b3d","b3e","b3f","b68","b6a","bc0","bc2","bc8","bca","be0","be2","be8","bea","d79","d7b","d7c","d7d","d7e","d7f","dd1","dd4","dd5","ecb","ece","ecf","eda","edb","ede","edf","ee1","ee3","ee4","ee5","ee6","ee7","ee9","eeb","eec","eed","eee","eef","f8a","f8b","f8e","f8f","f9a","f9b","f9e","f9f","fca","fe0","fe2","fe8","fea","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e36","8e37","8e38","8e39","8e3a","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a430","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a632","a633","a636","a637","a638","a639","a63a","a63b","a63c","a63d","a63e","a63f","a662","a663","a666","a667","a668","a669","a66a","a66b","a66c","a66d","a66e","a66f","a672","a673","a676","a677","a678","a679","a67a","a67b","a67c","a67d","a67e","a67f","a722","a723","a726","a727","a728","a729","a72a","a72b","a72c","a72d","a72e","a72f","a732","a733","a736","a737","a738","a739","a73a","a73b","a73c","a73d","a73e","a73f","a762","a763","a766","a767","a768","a769","a76a","a76b","a76c","a76d","a76e","a76f","a772","a773","a776","a777","a778","a779","a77a","a77b","a77c","a77d","a77e","a77f","b222","b223","b226","b227","b228","b229","b22a","b22b","b22c","b22d","b22e","b22f","b232","b233","b236","b237","b238","b239","b23a","b23b","b23c","b23d","b23e","b23f","b262","b263","b266","b267","b268","b269","b26a","b26b","b26c","b26d","b26e","b26f","b272","b273","b276","b277","b278","b279","b27a","b27b","b27c","b27d","b27e","b27f","b322","b323","b326","b327","b328","b329","b32a","b32b","b32c","b32d","b32e","b32f","b332","b333","b336","b337","b338","b339","b33a","b33b","b33c","b33d","b33e","b33f","b362","b363","b366","b367","b368","b369","b36a","b36b","b36c","b36d","b36e","b36f","b372","b373","b376","b377","b378","b379","b37a","b37b","b37c","b37d","b37e","b37f","b622","b623","b626","b627","b628","b629","b62a","b62b","b62c","b62d","b62e","b62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","ec9a","ec9b","ec9e","ec9f","ecca","eccb","ecce","eccf","ecda","ecdb","ecde","ecdf","ed8a","ed8b","ed8e","ed8f","ed9a","ed9b","ed9e","ed9f","edca","edcb","edce","edcf","edda","eddb","edde","eddf","f88a","f88b","f88e","f88f","f89a","f89b","f89e","f89f","f8ca","f8cb","f8ce","f8cf","f8da","f8db","f8de","f8df","f98a","f98b","f98e","f98f","f99a","f99b","f99e","f99f","f9ca","f9cb","f9ce","f9cf","f9da","f9db","f9de","f9df","fc8a","fc8b","fc8e","fc8f"]]"#,
            ),
        ),
        (
            r"@A 100.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 100.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b801","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 100.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 100.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","dab","dae","daf","dba","dbb","dbe","dbf","dea","deb","dee","def","dfa","dfb","dfe","dff","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","8a82","8a83","8a88","8a89","8a8a","8a8b","8aa0","8aa1","8aa2","8aa3","8aa8","8aa9","8aaa","8aab","a000","a001","a002","a003","a008","a009","a00a","a00b","a020","a021","a022","a023","a028","a029","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","da86","da87","da8c","da8d","da8e","da8f","da92","da93","da96","da97","da98","da99","da9a","da9b","da9c","da9d","da9e","da9f","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dac2","dac3","dac6","dac7","dac8","dac9","daca","dacb","dacc","dacd","dace","dacf","dad2","dad3","dad6","dad7","dad8","dad9","dada","dadb","dadc","dadd","dade","dadf","db82","db83","db86","db87","db88","db89","db8a","db8b","db8c","db8d","db8e","db8f","db92","db93","db96","db97","db98","db99","db9a","db9b","db9c","db9d","db9e","db9f","dbc2","dbc3","dbc6","dbc7","dbc8","dbc9","dbca","dbcb","dbcc","dbcd","dbce","dbcf","dbd2","dbd3","dbd6","dbd7","dbd8","dbd9","dbda","dbdb","dbdc","dbdd","dbde","dbdf","de82","de83","de86","de87","de88","de89","de8a","de8b","de8c","de8d","de8e","de8f","de92","de93","de96","de97","de98","de99","de9a","de9b","de9c","de9d","de9e","de9f","dec2","dec3","dec6","dec7","dec8","dec9","deca","decb","decc","decd","dece","decf","ded2","ded3","ded6","ded7","ded8","ded9","deda","dedb","dedc","dedd","dede","dedf","df82","df83","df86","df87","df88","df89","df8a","df8b","df8c","df8d","df8e","df8f","df92","df93","df96","df97","df98","df99","df9a","df9b","df9c","df9d","df9e","df9f","dfc2","dfc3","dfc6","dfc7","dfc8","dfc9","dfca","dfcb","dfcc","dfcd","dfce","dfcf","dfd2","dfd3","dfd6","dfd7","dfd8","dfd9","dfda","dfdb","dfdc","dfdd","dfde","dfdf","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa94","fa95","fac0","fac1","fac4","fac5","fad0","fad1","fad4","fad5","fb80","fb81","fb84","fb85","fb90","fb91","fb94","fb95","fbc0","fbc1","fbc4","fbc5","fbd0","fbd1","fbd4","fbd5","fe80","fe81","fe84","fe85","fe90","fe91","fe94","fe95","fec0","fec1","fec4","fec5","fed0","fed1","fed4","fed5","ff80","ff81","ff84","ff85","ff90","ff91","ff94","ff95","ffc0","ffc1","ffc4","ffc5","ffd0","ffd1","ffd4","ffd5"]]"#,
            ),
        ),
        (
            r"@A 100.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 100.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["ad","af","b8","b9","ba","bb","ef","fa","fb","828","82a","880","8e9","8eb","8ec","8ed","8ee","8ef","8f8","8fa","a41","a44","a45","a50","a69","a6b","a6c","a6d","a6e","a6f","a78","a79","a7a","a7b","a7c","a7d","a7e","a7f","ac1","ac3","ac4","ac5","ac6","ac7","ac9","acb","acc","acd","ace","acf","ae1","ae3","ae4","ae5","ae6","ae7","ae9","aeb","aec","aed","aee","aef","b28","b29","b2a","b2b","b2c","b2d","b2e","b2f","b38","b39","b3a","b3b","b3c","b3d","b3e","b3f","b68","b6a","bc0","bc2","bc8","bca","be0","be2","be8","bea","d79","d7b","d7c","d7d","d7e","d7f","dd1","dd4","dd5","ecb","ece","ecf","eda","edb","ede","edf","ee1","ee3","ee4","ee5","ee6","ee7","ee9","eeb","eec","eed","eee","eef","f8a","f8b","f8e","f8f","f9a","f9b","f9e","f9f","fca","fe0","fe2","fe8","fea","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e36","8e37","8e38","8e39","8e3a","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a430","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a632","a633","a636","a637","a638","a639","a63a","a63b","a63c","a63d","a63e","a63f","a662","a663","a666","a667","a668","a669","a66a","a66b","a66c","a66d","a66e","a66f","a672","a673","a676","a677","a678","a679","a67a","a67b","a67c","a67d","a67e","a67f","a722","a723","a726","a727","a728","a729","a72a","a72b","a72c","a72d","a72e","a72f","a732","a733","a736","a737","a738","a739","a73a","a73b","a73c","a73d","a73e","a73f","a762","a763","a766","a767","a768","a769","a76a","a76b","a76c","a76d","a76e","a76f","a772","a773","a776","a777","a778","a779","a77a","a77b","a77c","a77d","a77e","a77f","b222","b223","b226","b227","b228","b229","b22a","b22b","b22c","b22d","b22e","b22f","b232","b233","b236","b237","b238","b239","b23a","b23b","b23c","b23d","b23e","b23f","b262","b263","b266","b267","b268","b269","b26a","b26b","b26c","b26d","b26e","b26f","b272","b273","b276","b277","b278","b279","b27a","b27b","b27c","b27d","b27e","b27f","b322","b323","b326","b327","b328","b329","b32a","b32b","b32c","b32d","b32e","b32f","b332","b333","b336","b337","b338","b339","b33a","b33b","b33c","b33d","b33e","b33f","b362","b363","b366","b367","b368","b369","b36a","b36b","b36c","b36d","b36e","b36f","b372","b373","b376","b377","b378","b379","b37a","b37b","b37c","b37d","b37e","b37f","b622","b623","b626","b627","b628","b629","b62a","b62b","b62c","b62d","b62e","b62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","ec9a","ec9b","ec9e","ec9f","ecca","eccb","ecce","eccf","ecda","ecdb","ecde","ecdf","ed8a","ed8b","ed8e","ed8f","ed9a","ed9b","ed9e","ed9f","edca","edcb","edce","edcf","edda","eddb","edde","eddf","f88a","f88b","f88e","f88f","f89a","f89b","f89e","f89f","f8ca","f8cb","f8ce","f8cf","f8da","f8db","f8de","f8df","f98a","f98b","f98e","f98f","f99a","f99b","f99e","f99f","f9ca","f9cb","f9ce","f9cf","f9da","f9db","f9de","f9df","fc8a","fc8b","fc8e","fc8f"]]"#,
            ),
        ),
        (
            r"@A 170.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 170.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b801","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 170.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 170.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","dab","dae","daf","dba","dbb","dbe","dbf","dea","deb","dee","def","dfa","dfb","dfe","dff","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","8a82","8a83","8a88","8a89","8a8a","8a8b","8aa0","8aa1","8aa2","8aa3","8aa8","8aa9","8aaa","8aab","a000","a001","a002","a003","a008","a009","a00a","a00b","a020","a021","a022","a023","a028","a029","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","da86","da87","da8c","da8d","da8e","da8f","da92","da93","da96","da97","da98","da99","da9a","da9b","da9c","da9d","da9e","da9f","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dac2","dac3","dac6","dac7","dac8","dac9","daca","dacb","dacc","dacd","dace","dacf","dad2","dad3","dad6","dad7","dad8","dad9","dada","dadb","dadc","dadd","dade","dadf","db82","db83","db86","db87","db88","db89","db8a","db8b","db8c","db8d","db8e","db8f","db92","db93","db96","db97","db98","db99","db9a","db9b","db9c","db9d","db9e","db9f","dbc2","dbc3","dbc6","dbc7","dbc8","dbc9","dbca","dbcb","dbcc","dbcd","dbce","dbcf","dbd2","dbd3","dbd6","dbd7","dbd8","dbd9","dbda","dbdb","dbdc","dbdd","dbde","dbdf","de82","de83","de86","de87","de88","de89","de8a","de8b","de8c","de8d","de8e","de8f","de92","de93","de96","de97","de98","de99","de9a","de9b","de9c","de9d","de9e","de9f","dec2","dec3","dec6","dec7","dec8","dec9","deca","decb","decc","decd","dece","decf","ded2","ded3","ded6","ded7","ded8","ded9","deda","dedb","dedc","dedd","dede","dedf","df82","df83","df86","df87","df88","df89","df8a","df8b","df8c","df8d","df8e","df8f","df92","df93","df96","df97","df98","df99","df9a","df9b","df9c","df9d","df9e","df9f","dfc2","dfc3","dfc6","dfc7","dfc8","dfc9","dfca","dfcb","dfcc","dfcd","dfce","dfcf","dfd2","dfd3","dfd6","dfd7","dfd8","dfd9","dfda","dfdb","dfdc","dfdd","dfde","dfdf","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa94","fa95","fac0","fac1","fac4","fac5","fad0","fad1","fad4","fad5","fb80","fb81","fb84","fb85","fb90","fb91","fb94","fb95","fbc0","fbc1","fbc4","fbc5","fbd0","fbd1","fbd4","fbd5","fe80","fe81","fe84","fe85","fe90","fe91","fe94","fe95","fec0","fec1","fec4","fec5","fed0","fed1","fed4","fed5","ff80","ff81","ff84","ff85","ff90","ff91","ff94","ff95","ffc0","ffc1","ffc4","ffc5","ffd0","ffd1","ffd4","ffd5"]]"#,
            ),
        ),
        (
            r"@A 170.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 170.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["ad","af","b8","b9","ba","bb","ef","fa","fb","828","82a","880","8e9","8eb","8ec","8ed","8ee","8ef","8f8","8fa","a41","a44","a45","a50","a69","a6b","a6c","a6d","a6e","a6f","a78","a79","a7a","a7b","a7c","a7d","a7e","a7f","ac1","ac3","ac4","ac5","ac6","ac7","ac9","acb","acc","acd","ace","acf","ae1","ae3","ae4","ae5","ae6","ae7","ae9","aeb","aec","aed","aee","aef","b28","b29","b2a","b2b","b2c","b2d","b2e","b2f","b38","b39","b3a","b3b","b3c","b3d","b3e","b3f","b68","b6a","bc0","bc2","bc8","bca","be0","be2","be8","bea","d79","d7b","d7c","d7d","d7e","d7f","dd1","dd4","dd5","ecb","ece","ecf","eda","edb","ede","edf","ee1","ee3","ee4","ee5","ee6","ee7","ee9","eeb","eec","eed","eee","eef","f8a","f8b","f8e","f8f","f9a","f9b","f9e","f9f","fca","fe0","fe2","fe8","fea","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e36","8e37","8e38","8e39","8e3a","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a430","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a632","a633","a636","a637","a638","a639","a63a","a63b","a63c","a63d","a63e","a63f","a662","a663","a666","a667","a668","a669","a66a","a66b","a66c","a66d","a66e","a66f","a672","a673","a676","a677","a678","a679","a67a","a67b","a67c","a67d","a67e","a67f","a722","a723","a726","a727","a728","a729","a72a","a72b","a72c","a72d","a72e","a72f","a732","a733","a736","a737","a738","a739","a73a","a73b","a73c","a73d","a73e","a73f","a762","a763","a766","a767","a768","a769","a76a","a76b","a76c","a76d","a76e","a76f","a772","a773","a776","a777","a778","a779","a77a","a77b","a77c","a77d","a77e","a77f","b222","b223","b226","b227","b228","b229","b22a","b22b","b22c","b22d","b22e","b22f","b232","b233","b236","b237","b238","b239","b23a","b23b","b23c","b23d","b23e","b23f","b262","b263","b266","b267","b268","b269","b26a","b26b","b26c","b26d","b26e","b26f","b272","b273","b276","b277","b278","b279","b27a","b27b","b27c","b27d","b27e","b27f","b322","b323","b326","b327","b328","b329","b32a","b32b","b32c","b32d","b32e","b32f","b332","b333","b336","b337","b338","b339","b33a","b33b","b33c","b33d","b33e","b33f","b362","b363","b366","b367","b368","b369","b36a","b36b","b36c","b36d","b36e","b36f","b372","b373","b376","b377","b378","b379","b37a","b37b","b37c","b37d","b37e","b37f","b622","b623","b626","b627","b628","b629","b62a","b62b","b62c","b62d","b62e","b62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","ec9a","ec9b","ec9e","ec9f","ecca","eccb","ecce","eccf","ecda","ecdb","ecde","ecdf","ed8a","ed8b","ed8e","ed8f","ed9a","ed9b","ed9e","ed9f","edca","edcb","edce","edcf","edda","eddb","edde","eddf","f88a","f88b","f88e","f88f","f89a","f89b","f89e","f89f","f8ca","f8cb","f8ce","f8cf","f8da","f8db","f8de","f8df","f98a","f98b","f98e","f98f","f99a","f99b","f99e","f99f","f9ca","f9cb","f9ce","f9cf","f9da","f9db","f9de","f9df","fc8a","fc8b","fc8e","fc8f"]]"#,
            ),
        ),
        (
            r"@A -100.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M -100.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","bb00","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A -100.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M -100.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b884","b885","b890","b891","b894","b895","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A -100.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M -100.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","ec9","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec36","ec37","ec38","ec39","ec3a","ec3b","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee30","ee31","ee34","ee35","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
    ]);
}

#[test]
fn shift_7() {
    check(&[
        (
            r"@A -179.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M -179.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b803","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","ba01","ba04","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A -179.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M -179.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A -179.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M -179.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","8ec","8ed","8ee","8ef","8f8","8fa","a44","a45","a50","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","8e36","8e37","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8e94","8e95","8e96","8e97","8e9c","8e9d","8e9e","8e9f","8eb4","8eb5","8eb6","8eb7","8ebc","8ebd","8ebe","8ebf","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a414","a415","a416","a417","a41c","a41d","a41e","a41f","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec38","ec39","ec3a","ec3b","ec90","ec91","ec92","ec93","ec98","ec99","ec9a","ec9b","ecb0","ecb1","ecb2","ecb3","ecb8","ecb9","ecba","ecbb","ee10","ee11","ee12","ee13","ee18","ee19","ee1a","ee1b","ee30","ee31","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
        (
            r"@A 179.5 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 179.5 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f03","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f12","f13","f16","f17","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f42","f43","f46","f47","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f52","f53","f56","f57","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","aaaa","b2ab","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b801","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","baab","baae","baaf","baba","babb","babe","babf","baea","baeb","baee","baef","bafa","bafb","bafe","baff","bbaa","bbab","bbae","bbaf","bbba","bbbb","bbbe","bbbf","bbea","bbeb","bbee","bbef","bbfa","bbfb","bbfe","bbff","beaa","beab","beae","beaf","beba","bebb","bebe","bebf","beea","beeb","beee","beef","befa","befb","befe","beff","bfaa","bfab","bfae","bfaf","bfba","bfbb","bfbe","bfbf","bfea","bfeb","bfee","bfef","bffa","bffb","bffe","bfff","eaaa","f003","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f012","f013","f016","f017","f018","f019","f01a","f01b","f01c","f01d","f01e","f01f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f042","f043","f046","f047","f048","f049","f04a","f04b","f04c","f04d","f04e","f04f","f052","f053","f056","f057","f058","f059","f05a","f05b","f05c","f05d","f05e","f05f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f102","f103","f106","f107","f108","f109","f10a","f10b","f10c","f10d","f10e","f10f","f112","f113","f116","f117","f118","f119","f11a","f11b","f11c","f11d","f11e","f11f","f142","f143","f146","f147","f148","f149","f14a","f14b","f14c","f14d","f14e","f14f","f152","f153","f156","f157","f158","f159","f15a","f15b","f15c","f15d","f15e","f15f","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f402","f403","f406","f407","f408","f409","f40a","f40b","f40c","f40d","f40e","f40f","f412","f413","f416","f417","f418","f419","f41a","f41b","f41c","f41d","f41e","f41f","f442","f443","f446","f447","f448","f449","f44a","f44b","f44c","f44d","f44e","f44f","f452","f453","f456","f457","f458","f459","f45a","f45b","f45c","f45d","f45e","f45f","f502","f503","f506","f507","f508","f509","f50a","f50b","f50c","f50d","f50e","f50f","f512","f513","f516","f517","f518","f519","f51a","f51b","f51c","f51d","f51e","f51f","f542","f543","f546","f547","f548","f549","f54a","f54b","f54c","f54d","f54e","f54f","f552","f553","f556","f557","f558","f559","f55a","f55b","f55c","f55d","f55e","f55f","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 179.5 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 179.5 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","dab","dae","daf","dba","dbb","dbe","dbf","dea","deb","dee","def","dfa","dfb","dfe","dff","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","8a82","8a83","8a88","8a89","8a8a","8a8b","8aa0","8aa1","8aa2","8aa3","8aa8","8aa9","8aaa","8aab","a000","a001","a002","a003","a008","a009","a00a","a00b","a020","a021","a022","a023","a028","a029","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b886","b887","b88c","b88d","b88e","b88f","b8a4","b8a5","b8a6","b8a7","b8ac","b8ad","b8ae","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba04","ba05","ba06","ba07","ba0c","ba0d","ba0e","ba0f","ba24","ba25","ba26","ba27","ba2c","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","da86","da87","da8c","da8d","da8e","da8f","da92","da93","da96","da97","da98","da99","da9a","da9b","da9c","da9d","da9e","da9f","daa4","daa5","daa6","daa7","daac","daad","daae","daaf","dac2","dac3","dac6","dac7","dac8","dac9","daca","dacb","dacc","dacd","dace","dacf","dad2","dad3","dad6","dad7","dad8","dad9","dada","dadb","dadc","dadd","dade","dadf","db82","db83","db86","db87","db88","db89","db8a","db8b","db8c","db8d","db8e","db8f","db92","db93","db96","db97","db98","db99","db9a","db9b","db9c","db9d","db9e","db9f","dbc2","dbc3","dbc6","dbc7","dbc8","dbc9","dbca","dbcb","dbcc","dbcd","dbce","dbcf","dbd2","dbd3","dbd6","dbd7","dbd8","dbd9","dbda","dbdb","dbdc","dbdd","dbde","dbdf","de82","de83","de86","de87","de88","de89","de8a","de8b","de8c","de8d","de8e","de8f","de92","de93","de96","de97","de98","de99","de9a","de9b","de9c","de9d","de9e","de9f","dec2","dec3","dec6","dec7","dec8","dec9","deca","decb","decc","decd","dece","decf","ded2","ded3","ded6","ded7","ded8","ded9","deda","dedb","dedc","dedd","dede","dedf","df82","df83","df86","df87","df88","df89","df8a","df8b","df8c","df8d","df8e","df8f","df92","df93","df96","df97","df98","df99","df9a","df9b","df9c","df9d","df9e","df9f","dfc2","dfc3","dfc6","dfc7","dfc8","dfc9","dfca","dfcb","dfcc","dfcd","dfce","dfcf","dfd2","dfd3","dfd6","dfd7","dfd8","dfd9","dfda","dfdb","dfdc","dfdd","dfde","dfdf","f004","f005","f006","f007","f00c","f00d","f00e","f00f","f024","f025","f026","f027","f02c","f02d","f02e","f02f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa90","fa91","fa94","fa95","fac0","fac1","fac4","fac5","fad0","fad1","fad4","fad5","fb80","fb81","fb84","fb85","fb90","fb91","fb94","fb95","fbc0","fbc1","fbc4","fbc5","fbd0","fbd1","fbd4","fbd5","fe80","fe81","fe84","fe85","fe90","fe91","fe94","fe95","fec0","fec1","fec4","fec5","fed0","fed1","fed4","fed5","ff80","ff81","ff84","ff85","ff90","ff91","ff94","ff95","ffc0","ffc1","ffc4","ffc5","ffd0","ffd1","ffd4","ffd5"]]"#,
            ),
        ),
        (
            r"@A 179.5 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 179.5 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["ad","af","b8","b9","ba","bb","ef","fa","fb","828","82a","880","8e9","8eb","8ec","8ed","8ee","8ef","8f8","8fa","a41","a44","a45","a50","a69","a6b","a6c","a6d","a6e","a6f","a78","a79","a7a","a7b","a7c","a7d","a7e","a7f","ac1","ac3","ac4","ac5","ac6","ac7","ac9","acb","acc","acd","ace","acf","ae1","ae3","ae4","ae5","ae6","ae7","ae9","aeb","aec","aed","aee","aef","b28","b29","b2a","b2b","b2c","b2d","b2e","b2f","b38","b39","b3a","b3b","b3c","b3d","b3e","b3f","b68","b6a","bc0","bc2","bc8","bca","be0","be2","be8","bea","d79","d7b","d7c","d7d","d7e","d7f","dd1","dd4","dd5","ecb","ece","ecf","eda","edb","ede","edf","ee1","ee3","ee4","ee5","ee6","ee7","ee9","eeb","eec","eed","eee","eef","f8a","f8b","f8e","f8f","f9a","f9b","f9e","f9f","fca","fe0","fe2","fe8","fea","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","8e32","8e33","8e36","8e37","8e38","8e39","8e3a","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a430","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a632","a633","a636","a637","a638","a639","a63a","a63b","a63c","a63d","a63e","a63f","a662","a663","a666","a667","a668","a669","a66a","a66b","a66c","a66d","a66e","a66f","a672","a673","a676","a677","a678","a679","a67a","a67b","a67c","a67d","a67e","a67f","a722","a723","a726","a727","a728","a729","a72a","a72b","a72c","a72d","a72e","a72f","a732","a733","a736","a737","a738","a739","a73a","a73b","a73c","a73d","a73e","a73f","a762","a763","a766","a767","a768","a769","a76a","a76b","a76c","a76d","a76e","a76f","a772","a773","a776","a777","a778","a779","a77a","a77b","a77c","a77d","a77e","a77f","b222","b223","b226","b227","b228","b229","b22a","b22b","b22c","b22d","b22e","b22f","b232","b233","b236","b237","b238","b239","b23a","b23b","b23c","b23d","b23e","b23f","b262","b263","b266","b267","b268","b269","b26a","b26b","b26c","b26d","b26e","b26f","b272","b273","b276","b277","b278","b279","b27a","b27b","b27c","b27d","b27e","b27f","b322","b323","b326","b327","b328","b329","b32a","b32b","b32c","b32d","b32e","b32f","b332","b333","b336","b337","b338","b339","b33a","b33b","b33c","b33d","b33e","b33f","b362","b363","b366","b367","b368","b369","b36a","b36b","b36c","b36d","b36e","b36f","b372","b373","b376","b377","b378","b379","b37a","b37b","b37c","b37d","b37e","b37f","b622","b623","b626","b627","b628","b629","b62a","b62b","b62c","b62d","b62e","b62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","ec9a","ec9b","ec9e","ec9f","ecca","eccb","ecce","eccf","ecda","ecdb","ecde","ecdf","ed8a","ed8b","ed8e","ed8f","ed9a","ed9b","ed9e","ed9f","edca","edcb","edce","edcf","edda","eddb","edde","eddf","f88a","f88b","f88e","f88f","f89a","f89b","f89e","f89f","f8ca","f8cb","f8ce","f8cf","f8da","f8db","f8de","f8df","f98a","f98b","f98e","f98f","f99a","f99b","f99e","f99f","f9ca","f9cb","f9ce","f9cf","f9da","f9db","f9de","f9df","fc8a","fc8b","fc8e","fc8f"]]"#,
            ),
        ),
        (
            r"@A 180.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M 180.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","ba01","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 180.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M 180.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b28c","b28e","b2a4","b2a6","b2ac","b2ae","b804","b806","b80c","b80e","b824","b826","b82c","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b887","b88d","b88f","b8a5","b8a7","b8ad","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba05","ba07","ba0d","ba0f","ba25","ba27","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 180.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M 180.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","8ec","8ed","8ee","8ef","8f8","8fa","a44","a45","a50","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","8e33","8e36","8e37","8e39","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8e91","8e93","8e94","8e95","8e96","8e97","8e99","8e9b","8e9c","8e9d","8e9e","8e9f","8eb1","8eb3","8eb4","8eb5","8eb6","8eb7","8eb9","8ebb","8ebc","8ebd","8ebe","8ebf","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a411","a413","a414","a415","a416","a417","a419","a41b","a41c","a41d","a41e","a41f","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec38","ec3a","ec90","ec92","ec98","ec9a","ecb0","ecb2","ecb8","ecba","ee10","ee12","ee18","ee1a","ee30","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
        (
            r"@A 360.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M 360.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a11","a13","a14","a15","a16","a17","a19","a1c","a1d","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","8bab","8bae","8baf","8bba","8bbb","8bbe","8bbf","8bea","8beb","8bee","8bef","8bfa","8bfb","8bfe","8bff","8eaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a101","a103","a104","a105","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ac","a1ad","a1b0","a1b1","a1b2","a1b3","a1b4","a1b5","a1b6","a1b7","a1b8","a1b9","a1bc","a1bd","a1e0","a1e1","a1e2","a1e3","a1e4","a1e5","a1e6","a1e7","a1e8","a1e9","a1ec","a1ed","a1f0","a1f1","a1f2","a1f3","a1f4","a1f5","a1f6","a1f7","a1f8","a1f9","a1fc","a1fd","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a400","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","baab","bb00","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faab","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 360.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M 360.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","a19","a1b","a1c","a1d","a1e","a1f","a31","a34","a35","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a12e","a12f","a13a","a13b","a13e","a13f","a16a","a16b","a16e","a16f","a17a","a17b","a17e","a17f","a184","a185","a186","a187","a18c","a18d","a18e","a18f","a1a4","a1a5","a1a6","a1a7","a1ac","a1ad","a1ae","a1af","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a304","a305","a306","a307","a30c","a30d","a30e","a30f","a324","a325","a326","a327","a32c","a32d","a330","a331","a332","a333","a334","a335","a336","a337","a338","a339","a33c","a33d","a360","a361","a362","a363","a364","a365","a366","a367","a368","a369","a36c","a36d","a370","a371","a372","a373","a374","a375","a376","a377","a378","a379","a37c","a37d","a42a","a42b","a480","a481","a482","a483","a488","a489","a48a","a48b","a4a0","a4a1","a4a2","a4a3","a4a8","a4a9","a4aa","a4ab","a600","a601","a602","a603","a608","a609","a60a","a60b","a620","a621","a622","a623","a628","a629","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b884","b885","b890","b891","b894","b895","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba87","ba8d","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa5","baa7","baad","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","da86","da8c","da8e","daa4","daa6","daac","daae","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f004","f006","f00c","f00e","f024","f026","f02c","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa87","fa8d","fa8f","faa5","faa7","faad","faaf"]]"#,
            ),
        ),
        (
            r"@A 360.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M 360.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","828","82a","880","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc4","cc5","cc6","cc7","ccc","ccd","cce","ccf","ce4","ce5","ce6","ce7","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","d79","d7b","d7c","d7d","d7e","d7f","dc0","dc2","dc8","dca","dd1","dd4","dd5","de0","de2","de8","dea","e44","e45","e46","e47","e4c","e4d","e4e","e4f","e64","e65","e70","e71","e74","e75","ec9","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","8222","8223","8226","8227","8228","8229","822a","822b","822c","822d","822e","822f","8820","8821","8824","8825","a632","a638","a63a","a690","a692","a698","a69a","a6b0","a6b2","a6b8","a6ba","ac10","ac12","ac18","ac1a","ac30","ac32","ac38","ac3a","ac90","ac92","ac98","ac9a","acb0","acb2","acb8","acba","ae10","ae12","ae18","ae1a","ae30","ae32","ae38","ae3a","ae90","ae92","ae98","ae9a","aeb0","aeb2","aeb8","aeba","c633","c636","c637","c639","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c691","c693","c694","c695","c696","c697","c699","c69b","c69c","c69d","c69e","c69f","c6b1","c6b3","c6b4","c6b5","c6b6","c6b7","c6b9","c6bb","c6bc","c6bd","c6be","c6bf","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","cc11","cc13","cc14","cc15","cc16","cc17","cc19","cc1b","cc1c","cc1d","cc1e","cc1f","cc31","cc33","cc34","cc35","cc36","cc37","cc39","cc3b","cc3c","cc3d","cc3e","cc3f","cc91","cc93","cc94","cc95","cc96","cc97","cc99","cc9b","cc9c","cc9d","cc9e","cc9f","ccb1","ccb3","ccb4","ccb5","ccb6","ccb7","ccb9","ccbb","ccbc","ccbd","ccbe","ccbf","ce11","ce13","ce14","ce15","ce16","ce17","ce19","ce1b","ce1c","ce1d","ce1e","ce1f","ce31","ce33","ce34","ce35","ce36","ce37","ce39","ce3b","ce3c","ce3d","ce3e","ce3f","ce91","ce93","ce94","ce95","ce96","ce97","ce99","ce9b","ce9c","ce9d","ce9e","ce9f","ceb1","ceb3","ceb4","ceb5","ceb6","ceb7","ceb9","cebb","cebc","cebd","cebe","cebf","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","d732","d733","d736","d737","d738","d739","d73a","d73b","d73c","d73d","d73e","d73f","d762","d763","d766","d767","d768","d769","d76a","d76b","d76c","d76d","d76e","d76f","d772","d773","d776","d777","d778","d779","d77a","d77b","d77c","d77d","d77e","d77f","dd30","dd31","dd34","dd35","dd60","dd61","dd64","dd65","dd70","dd71","dd74","dd75","e411","e413","e414","e415","e416","e417","e419","e41b","e41c","e41d","e41e","e41f","e431","e433","e434","e435","e436","e437","e439","e43b","e43c","e43d","e43e","e43f","e491","e493","e494","e495","e496","e497","e499","e49b","e49c","e49d","e49e","e49f","e4b1","e4b3","e4b4","e4b5","e4b6","e4b7","e4b9","e4bb","e4bc","e4bd","e4be","e4bf","e611","e613","e614","e615","e616","e617","e619","e61b","e61c","e61d","e61e","e61f","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec36","ec37","ec38","ec39","ec3a","ec3b","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ec9a","ecb0","ecb2","ecb8","ecba","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee10","ee12","ee18","ee1a","ee30","ee31","ee32","ee34","ee35","ee38","ee3a","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ee90","ee92","ee98","ee9a","eeb0","eeb2","eeb8","eeba","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625"]]"#,
            ),
        ),
        (
            r"@A -540.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9b2"]]"#),
        ),
        (
            r"@M -540.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b81","b83","b84","b85","b86","b87","b89","b8c","b8d","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b2ae","b2af","b2ba","b2bb","b2be","b2bf","b2ea","b2eb","b2ee","b2ef","b2fa","b2fb","b2fe","b2ff","b3aa","b803","b804","b805","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8b0","b8b1","b8b2","b8b3","b8b4","b8b5","b8b6","b8b7","b8b8","b8b9","b8bc","b8bd","b8e0","b8e1","b8e2","b8e3","b8e4","b8e5","b8e6","b8e7","b8e8","b8e9","b8ec","b8ed","b8f0","b8f1","b8f2","b8f3","b8f4","b8f5","b8f6","b8f7","b8f8","b8f9","b8fc","b8fd","b900","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9b2","ba01","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A -540.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb33"]]"#),
        ),
        (
            r"@M -540.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b89","b8b","b8c","b8d","b8e","b8f","ba1","ba4","ba5","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b28c","b28e","b2a4","b2a6","b2ac","b2ae","b804","b806","b80c","b80e","b824","b826","b82c","b82e","b82f","b83a","b83b","b83e","b83f","b86a","b86b","b86e","b86f","b87a","b87b","b87e","b87f","b884","b885","b887","b88d","b88f","b8a5","b8a7","b8ad","b8af","b92a","b92b","b980","b981","b982","b983","b988","b989","b98a","b98b","b9a0","b9a1","b9a2","b9a3","b9a8","b9a9","b9aa","b9ab","ba05","ba07","ba0d","ba0f","ba25","ba27","ba2d","ba30","ba31","ba32","ba33","ba34","ba35","ba36","ba37","ba38","ba39","ba3c","ba3d","ba60","ba61","ba62","ba63","ba64","ba65","ba66","ba67","ba68","ba69","ba6c","ba6d","ba70","ba71","ba72","ba73","ba74","ba75","ba76","ba77","ba78","ba79","ba7c","ba7d","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb00","bb01","bb02","bb03","bb08","bb09","bb0a","bb0b","bb20","bb21","bb22","bb23","bb28","bb29","bb33","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A -540.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["a51f"]]"#),
        ),
        (
            r"@M -540.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","8ec","8ed","8ee","8ef","8f8","8fa","a44","a45","a50","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","8e33","8e36","8e37","8e39","8e3b","8e3c","8e3d","8e3e","8e3f","8e62","8e63","8e66","8e67","8e68","8e69","8e6a","8e6b","8e6c","8e6d","8e6e","8e6f","8e72","8e73","8e76","8e77","8e78","8e79","8e7a","8e7b","8e7c","8e7d","8e7e","8e7f","8e91","8e93","8e94","8e95","8e96","8e97","8e99","8e9b","8e9c","8e9d","8e9e","8e9f","8eb1","8eb3","8eb4","8eb5","8eb6","8eb7","8eb9","8ebb","8ebc","8ebd","8ebe","8ebf","8f22","8f23","8f26","8f27","8f28","8f29","8f2a","8f2b","8f2c","8f2d","8f2e","8f2f","a411","a413","a414","a415","a416","a417","a419","a41b","a41c","a41d","a41e","a41f","a431","a434","a435","a460","a461","a464","a465","a470","a471","a474","a475","a51f","a520","a521","a524","a525","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec38","ec3a","ec90","ec92","ec98","ec9a","ecb0","ecb2","ecb8","ecba","ee10","ee12","ee18","ee1a","ee30","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
        (
            r"@A 1000.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@M 1000.0 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f1","f3","f4","f5","f6","f7","f9","fc","fd","a13","a16","a17","a19","a1b","a1c","a1d","a1e","a1f","b83","b86","b87","b89","b8b","b8c","b8d","b8e","b8f","f01","f03","f04","f05","f06","f07","f09","f0b","f0c","f0d","f0e","f0f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fac","fad","fb0","fb1","fb2","fb3","fb4","fb5","fb6","fb7","fb8","fb9","fbc","fbd","fe0","fe1","fe2","fe3","fe4","fe5","fe6","fe7","fe8","fe9","fec","fed","ff0","ff1","ff2","ff3","ff4","ff5","ff6","ff7","ff8","ff9","ffc","ffd","8aaa","a000","a002","a008","a00a","a020","a022","a028","a02a","a080","a082","a088","a08a","a0a0","a0a2","a0a8","a0aa","a103","a106","a107","a109","a10b","a10c","a10d","a10e","a10f","a112","a113","a116","a117","a118","a119","a11a","a11b","a11c","a11d","a11e","a11f","a121","a123","a124","a125","a126","a127","a129","a12b","a12c","a12d","a12e","a12f","a142","a143","a146","a147","a148","a149","a14a","a14b","a14c","a14d","a14e","a14f","a152","a153","a156","a157","a158","a159","a15a","a15b","a15c","a15d","a15e","a15f","a181","a183","a184","a185","a186","a187","a189","a18b","a18c","a18d","a18e","a18f","a1a1","a1a3","a1a4","a1a5","a1a6","a1a7","a1a9","a1ab","a1ac","a1ad","a1ae","a1af","a200","a202","a208","a20a","a220","a222","a228","a22a","a280","a282","a288","a28a","a2a0","a2a2","a2a8","a2aa","a301","a304","a305","a310","a311","a314","a315","a340","a341","a344","a345","a350","a351","a354","a355","a402","a408","a40a","a420","a422","a428","a42a","a480","a482","a488","a48a","a4a0","a4a2","a4a8","a4aa","a600","a800","a802","a808","a80a","a820","a822","a828","a82a","a880","a882","a888","a88a","a8a0","a8a2","a8a8","a8aa","aa00","aa02","aa08","aa0a","aa20","aa22","aa28","aa2a","aa80","aa82","aa88","aa8a","aaa0","aaa2","aaa8","b803","b806","b807","b809","b80b","b80c","b80d","b80e","b80f","b812","b813","b816","b817","b818","b819","b81a","b81b","b81c","b81d","b81e","b81f","b821","b823","b824","b825","b826","b827","b829","b82b","b82c","b82d","b82e","b82f","b842","b843","b846","b847","b848","b849","b84a","b84b","b84c","b84d","b84e","b84f","b852","b853","b856","b857","b858","b859","b85a","b85b","b85c","b85d","b85e","b85f","b881","b883","b884","b885","b886","b887","b889","b88b","b88c","b88d","b88e","b88f","b8a1","b8a3","b8a4","b8a5","b8a6","b8a7","b8a9","b8ab","b8ac","b8ad","b8ae","b8af","b902","b908","b90a","b920","b922","b928","b92a","b980","b982","b988","b98a","b9a0","b9a2","b9a8","b9aa","b9ba","ba01","ba04","ba05","ba10","ba11","ba14","ba15","ba40","ba41","ba44","ba45","ba50","ba51","ba54","ba55","bb00","daab","daae","daaf","daba","dabb","dabe","dabf","daea","daeb","daee","daef","dafa","dafb","dafe","daff","dbaa","dbab","dbae","dbaf","dbba","dbbb","dbbe","dbbf","dbea","dbeb","dbee","dbef","dbfa","dbfb","dbfe","dbff","deaa","deab","deae","deaf","deba","debb","debe","debf","deea","deeb","deee","deef","defa","defb","defe","deff","dfaa","dfab","dfae","dfaf","dfba","dfbb","dfbe","dfbf","dfea","dfeb","dfee","dfef","dffa","dffb","dffe","dfff","f001","f003","f004","f005","f006","f007","f009","f00b","f00c","f00d","f00e","f00f","f021","f023","f024","f025","f026","f027","f029","f02b","f02c","f02d","f02e","f02f","f081","f083","f084","f085","f086","f087","f089","f08b","f08c","f08d","f08e","f08f","f0a1","f0a3","f0a4","f0a5","f0a6","f0a7","f0a9","f0ab","f0ac","f0ad","f0ae","f0af","f201","f203","f204","f205","f206","f207","f209","f20b","f20c","f20d","f20e","f20f","f221","f223","f224","f225","f226","f227","f229","f22b","f22c","f22d","f22e","f22f","f281","f283","f284","f285","f286","f287","f289","f28b","f28c","f28d","f28e","f28f","f2a1","f2a3","f2a4","f2a5","f2a6","f2a7","f2a9","f2ab","f2ac","f2ad","f2ae","f2af","f801","f803","f804","f805","f806","f807","f809","f80b","f80c","f80d","f80e","f80f","f821","f823","f824","f825","f826","f827","f829","f82b","f82c","f82d","f82e","f82f","f881","f883","f884","f885","f886","f887","f889","f88b","f88c","f88d","f88e","f88f","f8a1","f8a3","f8a4","f8a5","f8a6","f8a7","f8a9","f8ab","f8ac","f8ad","f8ae","f8af","fa01","fa03","fa04","fa05","fa06","fa07","fa09","fa0b","fa0c","fa0d","fa0e","fa0f","fa21","fa23","fa24","fa25","fa26","fa27","fa29","fa2b","fa2c","fa2d","fa2e","fa2f","fa81","fa83","fa84","fa85","fa86","fa87","fa89","fa8b","fa8c","fa8d","fa8e","fa8f","faa1","faa3","faa4","faa5","faa6","faa7","faa9","faac","faad","fab0","fab1","fab2","fab3","fab4","fab5","fab6","fab7","fab8","fab9","fabc","fabd","fae0","fae1","fae2","fae3","fae4","fae5","fae6","fae7","fae8","fae9","faec","faed","faf0","faf1","faf2","faf3","faf4","faf5","faf6","faf7","faf8","faf9","fafc","fafd","fba0","fba1","fba2","fba3","fba4","fba5","fba6","fba7","fba8","fba9","fbac","fbad","fbb0","fbb1","fbb2","fbb3","fbb4","fbb5","fbb6","fbb7","fbb8","fbb9","fbbc","fbbd","fbe0","fbe1","fbe2","fbe3","fbe4","fbe5","fbe6","fbe7","fbe8","fbe9","fbec","fbed","fbf0","fbf1","fbf2","fbf3","fbf4","fbf5","fbf6","fbf7","fbf8","fbf9","fbfc","fbfd","fea0","fea1","fea2","fea3","fea4","fea5","fea6","fea7","fea8","fea9","feac","fead","feb0","feb1","feb2","feb3","feb4","feb5","feb6","feb7","feb8","feb9","febc","febd","fee0","fee1","fee2","fee3","fee4","fee5","fee6","fee7","fee8","fee9","feec","feed","fef0","fef1","fef2","fef3","fef4","fef5","fef6","fef7","fef8","fef9","fefc","fefd","ffa0","ffa1","ffa2","ffa3","ffa4","ffa5","ffa6","ffa7","ffa8","ffa9","ffac","ffad","ffb0","ffb1","ffb2","ffb3","ffb4","ffb5","ffb6","ffb7","ffb8","ffb9","ffbc","ffbd","ffe0","ffe1","ffe2","ffe3","ffe4","ffe5","ffe6","ffe7","ffe8","ffe9","ffec","ffed","fff0","fff1","fff2","fff3","fff4","fff5","fff6","fff7","fff8","fff9","fffc","fffd"]]"#,
            ),
        ),
        (
            r"@A 1000.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b93b"]]"#),
        ),
        (
            r"@M 1000.0 -5.0 3.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["f3","f6","f7","f9","fb","fc","fd","fe","ff","8bb","8be","8bf","a11","a13","a14","a15","a16","a17","b2b","b2e","b2f","b81","b83","b84","b85","b86","b87","bab","bae","baf","bba","bbb","bbe","bbf","bea","beb","bee","bef","bfa","bfb","bfe","bff","f09","f0b","f0c","f0d","f0e","f0f","f18","f19","f1a","f1b","f1c","f1d","f1e","f1f","f21","f23","f24","f25","f26","f27","f29","f2b","f2c","f2d","f2e","f2f","f48","f49","f4a","f4b","f4c","f4d","f4e","f4f","f58","f59","f5a","f5b","f5c","f5d","f5e","f5f","f81","f83","f84","f85","f86","f87","f89","f8b","f8c","f8d","f8e","f8f","fa1","fa3","fa4","fa5","fa6","fa7","fa9","fab","fac","fad","fae","faf","8b86","8b87","8b8c","8b8d","8b8e","8b8f","8b92","8b93","8b96","8b97","8b98","8b99","8b9a","8b9b","8b9c","8b9d","8b9e","8b9f","8ba4","8ba5","8ba6","8ba7","8bac","8bad","8bae","8baf","8bc2","8bc3","8bc6","8bc7","8bc8","8bc9","8bca","8bcb","8bcc","8bcd","8bce","8bcf","8bd2","8bd3","8bd6","8bd7","8bd8","8bd9","8bda","8bdb","8bdc","8bdd","8bde","8bdf","8e82","8e83","8e88","8e89","8e8a","8e8b","8ea0","8ea1","8ea2","8ea3","8ea8","8ea9","8eaa","8eab","a02a","a02b","a080","a081","a082","a083","a088","a089","a08a","a08b","a0a0","a0a1","a0a2","a0a3","a0a8","a0a9","a0aa","a0ab","a104","a105","a106","a107","a10c","a10d","a10e","a10f","a124","a125","a126","a127","a12c","a12d","a12e","a12f","a184","a185","a190","a191","a194","a195","a1c0","a1c1","a1c4","a1c5","a1d0","a1d1","a1d4","a1d5","a200","a201","a202","a203","a208","a209","a20a","a20b","a220","a221","a222","a223","a228","a229","a22a","a22b","a280","a281","a282","a283","a288","a289","a28a","a28b","a2a0","a2a1","a2a2","a2a3","a2a8","a2a9","a2aa","a2ab","a400","a401","a402","a403","a408","a409","a40a","a40b","a420","a421","a422","a423","a428","a429","a42a","a42b","a480","a481","a800","a801","a802","a803","a808","a809","a80a","a80b","a820","a821","a822","a823","a828","a829","a82a","a82b","a880","a881","a882","a883","a888","a889","a88a","a88b","a8a0","a8a1","a8a2","a8a3","a8a8","a8a9","a8aa","a8ab","aa00","aa01","aa02","aa03","aa08","aa09","aa0a","aa0b","aa20","aa21","aa22","aa23","aa28","aa29","aa2a","aa2b","aa80","aa81","aa82","aa83","aa88","aa89","aa8a","aa8b","aaa0","aaa1","aaa2","aaa3","aaa8","aaa9","aaaa","aaab","b286","b287","b28c","b28d","b28e","b28f","b292","b293","b296","b297","b298","b299","b29a","b29b","b29c","b29d","b29e","b29f","b2a4","b2a5","b2a6","b2a7","b2ac","b2ad","b2ae","b2af","b2c2","b2c3","b2c6","b2c7","b2c8","b2c9","b2ca","b2cb","b2cc","b2cd","b2ce","b2cf","b2d2","b2d3","b2d6","b2d7","b2d8","b2d9","b2da","b2db","b2dc","b2dd","b2de","b2df","b382","b383","b388","b389","b38a","b38b","b3a0","b3a1","b3a2","b3a3","b3a8","b3a9","b3aa","b3ab","b804","b805","b806","b807","b80c","b80d","b80e","b80f","b824","b825","b826","b827","b82c","b82d","b82e","b82f","b884","b885","b890","b891","b894","b895","b8c0","b8c1","b8c4","b8c5","b8d0","b8d1","b8d4","b8d5","b900","b901","b902","b903","b908","b909","b90a","b90b","b920","b921","b922","b923","b928","b929","b92a","b92b","b93b","b980","b981","ba86","ba87","ba8c","ba8d","ba8e","ba8f","ba92","ba93","ba96","ba97","ba98","ba99","ba9a","ba9b","ba9c","ba9d","ba9e","ba9f","baa4","baa5","baa6","baa7","baac","baad","baae","baaf","bac2","bac3","bac6","bac7","bac8","bac9","baca","bacb","bacc","bacd","bace","bacf","bad2","bad3","bad6","bad7","bad8","bad9","bada","badb","badc","badd","bade","badf","bb82","bb83","bb86","bb87","bb88","bb89","bb8a","bb8b","bb8c","bb8d","bb8e","bb8f","bb92","bb93","bb96","bb97","bb98","bb99","bb9a","bb9b","bb9c","bb9d","bb9e","bb9f","bbc2","bbc3","bbc6","bbc7","bbc8","bbc9","bbca","bbcb","bbcc","bbcd","bbce","bbcf","bbd2","bbd3","bbd6","bbd7","bbd8","bbd9","bbda","bbdb","bbdc","bbdd","bbde","bbdf","be82","be83","be86","be87","be88","be89","be8a","be8b","be8c","be8d","be8e","be8f","be92","be93","be96","be97","be98","be99","be9a","be9b","be9c","be9d","be9e","be9f","bec2","bec3","bec6","bec7","bec8","bec9","beca","becb","becc","becd","bece","becf","bed2","bed3","bed6","bed7","bed8","bed9","beda","bedb","bedc","bedd","bede","bedf","bf82","bf83","bf86","bf87","bf88","bf89","bf8a","bf8b","bf8c","bf8d","bf8e","bf8f","bf92","bf93","bf96","bf97","bf98","bf99","bf9a","bf9b","bf9c","bf9d","bf9e","bf9f","bfc2","bfc3","bfc6","bfc7","bfc8","bfc9","bfca","bfcb","bfcc","bfcd","bfce","bfcf","bfd2","bfd3","bfd6","bfd7","bfd8","bfd9","bfda","bfdb","bfdc","bfdd","bfde","bfdf","ea82","ea83","ea88","ea89","ea8a","ea8b","eaa0","eaa1","eaa2","eaa3","eaa8","eaa9","eaaa","eaab","f02e","f02f","f03a","f03b","f03e","f03f","f06a","f06b","f06e","f06f","f07a","f07b","f07e","f07f","f084","f085","f086","f087","f08c","f08d","f08e","f08f","f0a4","f0a5","f0a6","f0a7","f0ac","f0ad","f0ae","f0af","f12a","f12b","f12e","f12f","f13a","f13b","f13e","f13f","f16a","f16b","f16e","f16f","f17a","f17b","f17e","f17f","f204","f205","f206","f207","f20c","f20d","f20e","f20f","f224","f225","f226","f227","f22c","f22d","f22e","f22f","f284","f285","f286","f287","f28c","f28d","f28e","f28f","f2a4","f2a5","f2a6","f2a7","f2ac","f2ad","f2ae","f2af","f42a","f42b","f42e","f42f","f43a","f43b","f43e","f43f","f46a","f46b","f46e","f46f","f47a","f47b","f47e","f47f","f52a","f52b","f52e","f52f","f53a","f53b","f53e","f53f","f56a","f56b","f56e","f56f","f57a","f57b","f57e","f57f","f804","f805","f806","f807","f80c","f80d","f80e","f80f","f824","f825","f826","f827","f82c","f82d","f82e","f82f","f884","f885","f886","f887","f88c","f88d","f88e","f88f","f8a4","f8a5","f8a6","f8a7","f8ac","f8ad","f8ae","f8af","fa04","fa05","fa06","fa07","fa0c","fa0d","fa0e","fa0f","fa24","fa25","fa26","fa27","fa2c","fa2d","fa2e","fa2f","fa84","fa85","fa86","fa87","fa8c","fa8d","fa8e","fa8f","faa4","faa5","faa6","faa7","faac","faad","faae","faaf"]]"#,
            ),
        ),
        (
            r"@A 1000.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["ed3d"]]"#),
        ),
        (
            r"@M 1000.0 30.0 -40.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["cd","cf","d8","d9","da","db","e5","f0","f1","a8a","aa0","aa2","c69","c6b","c6c","c6d","c6e","c6f","c78","c79","c7a","c7b","c7c","c7d","c7e","c7f","cc1","cc3","cc4","cc5","cc6","cc7","cc9","ccb","ccc","ccd","cce","ccf","ce1","ce3","ce4","ce5","ce6","ce7","ce9","ceb","cec","ced","cee","cef","d28","d29","d2a","d2b","d2c","d2d","d2e","d2f","d38","d39","d3a","d3b","d3c","d3d","d3e","d3f","d68","d6a","dc0","dc2","dc8","dca","de0","de2","de8","dea","e41","e43","e44","e45","e46","e47","e49","e4b","e4c","e4d","e4e","e4f","e61","e64","e65","e70","e71","e74","e75","ec9","ecb","ecc","ecd","ece","ecf","ed8","eda","ee1","ee4","ee5","ef0","f20","f21","f24","f25","f30","f31","f34","f35","f40","f42","f48","f4a","f60","fdb","fde","fdf","ff1","ff3","ff4","ff5","ff6","ff7","a88a","a88b","a88e","a88f","aa80","aa81","aa82","aa83","aa84","aa85","aa86","aa87","aa88","aa89","aa8c","aa8d","c632","c633","c636","c637","c638","c639","c63a","c63b","c63c","c63d","c63e","c63f","c662","c663","c666","c667","c668","c669","c66a","c66b","c66c","c66d","c66e","c66f","c672","c673","c676","c677","c678","c679","c67a","c67b","c67c","c67d","c67e","c67f","c722","c723","c726","c727","c728","c729","c72a","c72b","c72c","c72d","c72e","c72f","c732","c733","c736","c737","c738","c739","c73a","c73b","c73c","c73d","c73e","c73f","c762","c763","c766","c767","c768","c769","c76a","c76b","c76c","c76d","c76e","c76f","c772","c773","c776","c777","c778","c779","c77a","c77b","c77c","c77d","c77e","c77f","d222","d223","d226","d227","d228","d229","d22a","d22b","d22c","d22d","d22e","d22f","d232","d233","d236","d237","d238","d239","d23a","d23b","d23c","d23d","d23e","d23f","d262","d263","d266","d267","d268","d269","d26a","d26b","d26c","d26d","d26e","d26f","d272","d273","d276","d277","d278","d279","d27a","d27b","d27c","d27d","d27e","d27f","d322","d323","d326","d327","d328","d329","d32a","d32b","d32c","d32d","d32e","d32f","d332","d333","d336","d337","d338","d339","d33a","d33b","d33c","d33d","d33e","d33f","d362","d363","d366","d367","d368","d369","d36a","d36b","d36c","d36d","d36e","d36f","d372","d373","d376","d377","d378","d379","d37a","d37b","d37c","d37d","d37e","d37f","d622","d623","d626","d627","d628","d629","d62a","d62b","d62c","d62d","d62e","d62f","e630","e631","e634","e635","e660","e661","e664","e665","e670","e671","e674","e675","e720","e721","e724","e725","e730","e731","e734","e735","e760","e761","e764","e765","e770","e771","e774","e775","ec32","ec33","ec36","ec37","ec38","ec39","ec3a","ec3b","ec3c","ec3d","ec3e","ec3f","ec62","ec63","ec66","ec67","ec68","ec69","ec6a","ec6b","ec6c","ec6d","ec6e","ec6f","ec72","ec73","ec76","ec77","ec78","ec79","ec7a","ec7b","ec7c","ec7d","ec7e","ec7f","ed22","ed23","ed26","ed27","ed28","ed29","ed2a","ed2b","ed2c","ed2d","ed2e","ed2f","ed3d","ee30","ee31","ee34","ee35","ee60","ee61","ee64","ee65","ee70","ee71","ee74","ee75","ef20","ef21","ef24","ef25","f220","f221","f224","f225","f230","f231","f234","f235","f260","f261","f264","f265","f270","f271","f274","f275","f320","f321","f324","f325","f330","f331","f334","f335","f360","f361","f364","f365","f370","f371","f374","f375","f620","f621","f624","f625","fd9a","fd9b","fd9e","fd9f","fdca","fdcb","fdce","fdcf","fdda","fddb","fdde","fddf","ff90","ff91","ff92","ff93","ff94","ff95","ff96","ff97","ff98","ff99","ff9c","ff9d","ffc0","ffc1","ffc2","ffc3","ffc4","ffc5","ffc6","ffc7","ffc8","ffc9","ffcc","ffcd","ffd0","ffd1","ffd2","ffd3","ffd4","ffd5","ffd6","ffd7","ffd8","ffd9","ffdc","ffdd"]]"#,
            ),
        ),
        (
            r"[ 'b9ad' 'b9ad1e' ] ->GEOSHAPE 0.1 0.1 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b9ad3","b9ad6","b9ad7","b9ad9","b9adb","b9adc","b9add","b9ade","b9adf","b9ad09","b9ad0b","b9ad0c","b9ad0d","b9ad0e","b9ad0f","b9ad18","b9ad19","b9ad1a","b9ad1b","b9ad1c","b9ad1d","b9ad1e","b9ad1f","b9ad21","b9ad23","b9ad24","b9ad25","b9ad26","b9ad27","b9ad29","b9ad2b","b9ad2c","b9ad2d","b9ad2e","b9ad2f","b9ad48","b9ad49","b9ad4a","b9ad4b","b9ad4c","b9ad4d","b9ad4e","b9ad4f","b9ad58","b9ad59","b9ad5a","b9ad5b","b9ad5c","b9ad5d","b9ad5e","b9ad5f","b9ad81","b9ad83","b9ad84","b9ad85","b9ad86","b9ad87","b9ad89","b9ad8b","b9ad8c","b9ad8d","b9ad8e","b9ad8f","b9ada1","b9ada3","b9ada4","b9ada5","b9ada6","b9ada7","b9ada9","b9adab","b9adac","b9adad","b9adae","b9adaf","b9af01","b9af03","b9af04","b9af05","b9af06","b9af07","b9af10","b9af11","b9af12","b9af13","b9af14","b9af15","b9af16","b9af17","b9af40","b9af41","b9af42","b9af43","b9af44","b9af45","b9af46","b9af47","b9af50","b9af51","b9af52","b9af53","b9af54","b9af55","b9af56","b9af57","b9b808","b9b80a","b9b820","b9b822","b9b828","b9b82a","b9b880","b9b882","b9b888","b9b88a","b9b8a0","b9b8a2","b9b8a8","b9b8aa","b9ba00","b9ba02"]]"#,
            ),
        ),
        (
            r"[ 'b9ad' 'b9ad1e' ] ->GEOSHAPE 5.0 5.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["bb37","bb371e"]]"#),
        ),
        (
            r"[ 'b9' 'b9ad1ee0' ] ->GEOSHAPE 5.0 5.0 6 GEOSHIFT SIZE",
            Ok(r"[17]"),
        ),
        (
            r"[ 'b9' 'b9ad' ] ->GEOSHAPE 45.0 90.0 8 GEOSHIFT SIZE",
            Ok(r"[1]"),
        ),
        (
            r"[ 'b9' 'b9ad' ] ->GEOSHAPE 0.0 0.0 8 GEOSHIFT SIZE",
            Ok(r"[1]"),
        ),
        (
            r"[ 'b9' 'b9ad' ] ->GEOSHAPE 0.0 0.0 8 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9"]]"#),
        ),
        (
            r"[ 'b9' ] ->GEOSHAPE 0.0 0.0 6 GEOSHIFT SIZE",
            Err(r"GEOSHIFT cannot use resolution finer than 4"),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 0.001 0.001 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0fca011"]]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 0.001 0.001 28 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0fca011"]]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 0.001 0.001 30 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT cannot use resolution finer than 28"),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 0.001 0.001 14 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0c5a9e7"]]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 0.001 0.001 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee0c5a9e7"]]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 0.001 0.001 32 GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE 100.0 0.001 0.001 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad1ee05e0299"]]"#),
        ),
        (
            r"[ 'f01234567' ] ->GEOSHAPE 100.0 10.0 10.0 14 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["f0ef6ac67"]]"#),
        ),
        (
            r"[ '0' ] ->GEOSHAPE -100.0 10.0 10.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["0"]]"#),
        ),
        (
            r"[ '0' '1' '4' '5' ] ->GEOSHAPE 10.0 90.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["0","1","4","5"]]"#),
        ),
        (
            r"[ '0' '1' '4' '5' ] ->GEOSHAPE 170.0 10.0 90.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["0","1","4","5"]]"#),
        ),
        (
            r"[ 'f' ] ->GEOSHAPE 90.0 180.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["d"]]"#),
        ),
        (
            r"[ 'f' ] ->GEOSHAPE 90.0 360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["9"]]"#),
        ),
        (
            r"[ 'f' ] ->GEOSHAPE 90.0 -360.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["9"]]"#),
        ),
        (
            r"[ 'f' ] ->GEOSHAPE 45.0 0.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b"]]"#),
        ),
        (
            r"[ 'f' ] ->GEOSHAPE -45.0 0.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["d"]]"#),
        ),
        (
            r"[ 'f' ] ->GEOSHAPE 135.0 0.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["3"]]"#),
        ),
        (
            r"[ 'f' ] ->GEOSHAPE 180.0 0.0 2 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["1"]]"#),
        ),
        (
            r"[ 'f' ] ->GEOSHAPE 0.0 45.0 100.0 -50.0 2 GEOSHIFT T GEOSHAPE->",
            Err(r"GEOSHIFT operates on a GEOSHAPE."),
        ),
        (
            r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' ] ->GEOSHAPE 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b9a3","b9a6","b9a7","b9a9","b9ab","b9ac","b9ad","b9ae","b9af","b9b2","b9b8","b9ba","bb01","bb04","bb05","bb10"]]"#,
            ),
        ),
        (
            r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' ] ->GEOSHAPE 0.703125 1.40625 6 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9a"]]"#),
        ),
        (
            r"[ 'b9a0' 'b9a1' 'b9a2' 'b9a3' 'b9a4' 'b9a5' 'b9a6' 'b9a7' 'b9a8' 'b9a9' 'b9aa' 'b9ab' 'b9ac' 'b9ad' 'b9ae' 'b9af' ] ->GEOSHAPE 0.703125 1.40625 8 GEOSHIFT T GEOSHAPE->",
            Ok(
                r#"[["b9a3","b9a6","b9a7","b9a9","b9ab","b9ac","b9ad","b9ae","b9af","b9b2","b9b8","b9ba","bb01","bb04","bb05","bb10"]]"#,
            ),
        ),
        (r"@A NaN 0.0 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A 0.0 NaN 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (r"@A NaN NaN 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ad"]]"#)),
        (
            r"@A NaN 1.0 1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@A 0.0 0.703125 1.40625 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@A 1.0 0.0 / 1.0 1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@A -1.0 0.0 / 1.0 1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (
            r"@A 0.0 0.0 / 1.0 1.0 0 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ba"]]"#),
        ),
        (r"@A 1 2 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9ba"]]"#)),
        (r"@A 100 1 2 0 GEOSHIFT T GEOSHAPE->", Ok(r#"[["b9b2"]]"#)),
        (r"@A 1 2 0 GEOSHIFT TYPEOF", Ok(r#"["GEOSHAPE"]"#)),
        (
            r"@A 1.0 0.0 / 1.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (r"@A 1.0 1.0 / 1.0 0 GEOSHIFT", Ok(r"[null]")),
        (
            r"@A 1.0 0.0 / 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (
            r"@A 0.0 1.0 0.0 / 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a longitude delta between -360.0 and 360.0."),
        ),
        (
            r"@A 0.0 -1.0 0.0 / 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a longitude delta between -360.0 and 360.0."),
        ),
        (
            r"@A -1.0 0.0 / 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (
            r"@A 0.0 0.0 1 GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 -1 GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 -2 GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 3 GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 31 GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 32 GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 'a' GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 2.0 GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 NULL GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 4294967298 GEOSHIFT T GEOSHAPE->",
            Ok(r#"[["b9ad"]]"#),
        ),
        (
            r"@A 0.0 0.0 4294967297 GEOSHIFT",
            Err(r"GEOSHIFT expects a resolution, even number between 0 and 30."),
        ),
        (
            r"@A 0.0 0.0 10 GEOSHIFT",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A 0.0 0.0 12 GEOSHIFT",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@A 0.0 0.0 30 GEOSHIFT",
            Err(r"GEOSHIFT cannot use resolution finer than 8"),
        ),
        (
            r"@E 0.0 0.0 2 GEOSHIFT",
            Err(r"GEOSHIFT cannot use resolution finer than 0"),
        ),
        (r"@E 0.0 0.0 0 GEOSHIFT T GEOSHAPE->", Ok(r"[[]]")),
        (
            r"@A 0.0 361.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a longitude delta between -360.0 and 360.0."),
        ),
        (
            r"@A 0.0 -361.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a longitude delta between -360.0 and 360.0."),
        ),
    ]);
}

#[test]
fn shift_8() {
    check(&[
        (
            r"@A 0.0 360.5 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a longitude delta between -360.0 and 360.0."),
        ),
        (
            r"@A 0.0 'a' 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a longitude delta between -360.0 and 360.0."),
        ),
        (
            r"@A 0.0 NULL 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a longitude delta between -360.0 and 360.0."),
        ),
        (
            r"@A 0.0 T 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a longitude delta between -360.0 and 360.0."),
        ),
        (
            r"@A 181.0 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (
            r"@A -181.0 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (
            r"@A 180.5 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (
            r"@A 'a' 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (
            r"@A NULL 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (
            r"@A T 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (
            r"@A 1.0 0.0 / 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT expects a latitude delta between -180.0 and 180.0."),
        ),
        (
            r"@A 'a' 0.0 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT operates on a GEOSHAPE."),
        ),
        (
            r"@A T 0.0 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT operates on a GEOSHAPE."),
        ),
        (
            r"@A NULL 0.0 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT operates on a GEOSHAPE."),
        ),
        (
            r"@A [ ] 0.0 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT operates on a GEOSHAPE."),
        ),
        (r"1 0.0 0.0 0 GEOSHIFT", Err(r"Empty stack.")),
        (
            r"'a' 0.0 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT operates on a GEOSHAPE."),
        ),
        (r"0.0 0.0 0 GEOSHIFT", Err(r"Empty stack.")),
        (r"0.0 0 GEOSHIFT", Err(r"Empty stack.")),
        (r"0 GEOSHIFT", Err(r"Empty stack.")),
        (r"GEOSHIFT", Err(r"Empty stack.")),
        (
            r"[ @A ] 0.0 0.0 0 GEOSHIFT",
            Err(r"GEOSHIFT operates on a GEOSHAPE."),
        ),
        (r"@A 0.0 0.0 0 GEOSHIFT DEPTH", Ok(r"[1,null]")),
        (r"@A 0.0 0.0 0 GEOSHIFT SIZE", Ok(r"[1]")),
        (r"@A 0.0 0.0 0 GEOSHIFT 0.0 0.0 0 GEOSHIFT SIZE", Ok(r"[1]")),
    ]);
}

#[test]
fn regexp() {
    check(&[
        (r"@A GEO.REGEXP", Ok(r#"["b9a[d].*"]"#)),
        (r"@C GEO.REGEXP", Ok(r#"["b9a[e].*"]"#)),
        (r"@P GEO.REGEXP", Ok(r#"["b[9].*"]"#)),
        (r"@PA GEO.REGEXP", Ok(r#"["b9[a].*"]"#)),
        (r"@K GEO.REGEXP", Ok(r#"["b9a[0123456789abcdef].*"]"#)),
        (r"@K15 GEO.REGEXP", Ok(r#"["b9a[0123456789abcde].*"]"#)),
        (r"@M GEO.REGEXP", Ok(r#"["[f].*|a[1].*|b[8].*|b9a[d].*"]"#)),
        (r"@D GEO.REGEXP", Ok(r#"["b9ad1ee[01].*|f0123456[7].*"]"#)),
        (r"@E GEO.REGEXP", Ok(r#"["].*"]"#)),
        (
            r"[ 'b9ad1ee0' 'b9ad1ee1' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["b9ad1ee[01].*"]"#),
        ),
        (
            r"[ 'b9ad1ee0' 'b9ad1ef1' 'b9ae1ee0' 'b9ad' 'f' 'a' 'e9' 'e8' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["[af].*|e[89].*|b9a[d].*|b9ad1ee[0].*|b9ad1ef[1].*|b9ae1ee[0].*"]"#),
        ),
        (
            r"[ 'b9ad1ee0' 'b9ad1ee0c5a9e7' 'f01234567' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["b9ad1ee[0].*|f0123456[7].*|b9ad1ee0c5a9e[7].*"]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["b9ad1ee0c5a9e[7].*"]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e7' 'b9ad1ee0c5a9e8' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["b9ad1ee0c5a9e[78].*"]"#),
        ),
        (
            r"[ 'b9ad1ee0c5a9e' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["b9ad1ee0c5a9[e].*"]"#),
        ),
        (r"[ 'b' ] ->GEOSHAPE GEO.REGEXP", Ok(r#"["[b].*"]"#)),
        (
            r"[ 'b' 'a' 'b9' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["[ab].*|b[9].*"]"#),
        ),
        (
            r"[ 'b' 'b9' 'b9a' 'b9ad' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["[b].*|b[9].*|b9[a].*|b9a[d].*"]"#),
        ),
        (
            r"[ '1' '2' '3' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["[123].*"]"#),
        ),
        (r"[ '0' ] ->GEOSHAPE GEO.REGEXP", Ok(r#"["[0].*"]"#)),
        (r"[ 'f' ] ->GEOSHAPE GEO.REGEXP", Ok(r#"["[f].*"]"#)),
        (
            r"[ 'ff' 'fe' 'e1' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["e[1].*|f[ef].*"]"#),
        ),
        (
            r"[ 'b9ad' 'b9ad1ee0' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["b9a[d].*|b9ad1ee[0].*"]"#),
        ),
        (
            r"[ '1' '10' '11' '12' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["[1].*|1[012].*"]"#),
        ),
        (
            r"[ 'b9ad' 'b9ad1' 'b9ad2' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["b9a[d].*|b9ad[12].*"]"#),
        ),
        (
            r"[ 'b9a0' 'b9a1' 'b9b0' ] ->GEOSHAPE GEO.REGEXP",
            Ok(r#"["b9a[01].*|b9b[0].*"]"#),
        ),
        (r"@A GEO.REGEXP TYPEOF", Ok(r#"["STRING"]"#)),
        (r"@A GEO.REGEXP SIZE", Ok(r"[8]")),
        (
            r"1 GEO.REGEXP",
            Err(r"GEO.REGEXP expects a GeoShape instance on top of the stack."),
        ),
        (
            r"'a' GEO.REGEXP",
            Err(r"GEO.REGEXP expects a GeoShape instance on top of the stack."),
        ),
        (r"GEO.REGEXP", Err(r"Empty stack.")),
        (
            r"NULL GEO.REGEXP",
            Err(r"GEO.REGEXP expects a GeoShape instance on top of the stack."),
        ),
        (
            r"[ @A ] GEO.REGEXP",
            Err(r"GEO.REGEXP expects a GeoShape instance on top of the stack."),
        ),
        (r"@A GEO.REGEXP 'a' STORE $a", Ok(r#"["b9a[d].*"]"#)),
    ]);
}
