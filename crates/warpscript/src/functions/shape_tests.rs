// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Oracle-checked tests of `SHAPE`, `CHECKSHAPE`, `HULLSHAPE`, `RESHAPE` and `PERMUTE`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const T: &str = r"[ [ [ 1 2 3 4 ] [ 5 6 7 8 ] [ 9 10 11 12 ] ] [ [ 13 14 15 16 ] [ 17 18 19 20 ] [ 21 22 23 24 ] ] ]";
const M: &str = r"[ [ 1 2 3 ] [ 4 5 6 ] ]";
const J: &str = r"[ [ 1 2 ] [ 3 ] [ 4 [ 5 6 7 ] ] ]";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@T", T).replace("@M", M).replace("@J", J)
}

/// The stack left by `script`, top first, as the JSON array of the Warp 10 exec endpoint,
/// or the error message.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = test_stack();
    stack
        .exec_checked(&format!("{script} DEPTH ->LIST REVERSE ->JSON"))
        .map_err(|err| err.to_string())?;
    match stack.pop().unwrap() {
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
fn shapes() {
    check(&[
        (r"@T SHAPE", Ok(r"[[2,3,4]]")),
        (r"@M SHAPE", Ok(r"[[2,3]]")),
        (
            r"@J SHAPE",
            Err(
                r"SHAPE expects that the sizes of the nested lists are coherent together to form a tensor (or multidimensional array).",
            ),
        ),
        (r"[ ] SHAPE", Ok(r"[[0]]")),
        (r"[ [ ] ] SHAPE", Ok(r"[[1,0]]")),
        (r"[ [ ] [ ] ] SHAPE", Ok(r"[[2,0]]")),
        (
            r"[ 1 [ 2 ] ] SHAPE",
            Err(
                r"SHAPE expects that the sizes of the nested lists are coherent together to form a tensor (or multidimensional array).",
            ),
        ),
        (
            r"[ [ 2 ] 1 ] SHAPE",
            Err(
                r"SHAPE expects that the sizes of the nested lists are coherent together to form a tensor (or multidimensional array).",
            ),
        ),
        (r"[ 1 2 3 ] SHAPE", Ok(r"[[3]]")),
        (r"[ [ [ ] ] [ [ ] ] ] SHAPE", Ok(r"[[2,1,0]]")),
        (
            r"[ [ 1 ] [ 1 2 ] ] SHAPE",
            Err(
                r"SHAPE expects that the sizes of the nested lists are coherent together to form a tensor (or multidimensional array).",
            ),
        ),
        (r"1 SHAPE", Err(r"SHAPE expects a LIST.")),
        (r"SHAPE", Err(r"Empty stack.")),
        (
            r"[ [ 1 2 ] [ [ 1 ] 2 ] ] SHAPE",
            Err(
                r"SHAPE expects that the sizes of the nested lists are coherent together to form a tensor (or multidimensional array).",
            ),
        ),
        (
            r"[ [ [ 1 ] [ 2 3 ] ] [ 4 ] ] SHAPE",
            Err(
                r"SHAPE expects that the sizes of the nested lists are coherent together to form a tensor (or multidimensional array).",
            ),
        ),
        (r"@T CHECKSHAPE", Ok(r"[true]")),
        (r"@M CHECKSHAPE", Ok(r"[true]")),
        (r"@J CHECKSHAPE", Ok(r"[false]")),
        (r"[ ] CHECKSHAPE", Ok(r"[true]")),
        (r"[ [ ] ] CHECKSHAPE", Ok(r"[true]")),
        (r"[ [ ] [ ] ] CHECKSHAPE", Ok(r"[true]")),
        (r"[ 1 [ 2 ] ] CHECKSHAPE", Ok(r"[false]")),
        (r"[ [ 2 ] 1 ] CHECKSHAPE", Ok(r"[false]")),
        (r"[ 1 2 3 ] CHECKSHAPE", Ok(r"[true]")),
        (r"[ [ [ ] ] [ [ ] ] ] CHECKSHAPE", Ok(r"[true]")),
        (r"[ [ 1 ] [ 1 2 ] ] CHECKSHAPE", Ok(r"[false]")),
        (r"1 CHECKSHAPE", Err(r"CHECKSHAPE expects a LIST.")),
        (r"CHECKSHAPE", Err(r"Empty stack.")),
        (r"[ [ 1 2 ] [ [ 1 ] 2 ] ] CHECKSHAPE", Ok(r"[false]")),
        (r"[ [ [ 1 ] [ 2 3 ] ] [ 4 ] ] CHECKSHAPE", Ok(r"[false]")),
        (r"@T HULLSHAPE", Ok(r"[[2,3,4]]")),
        (r"@M HULLSHAPE", Ok(r"[[2,3]]")),
        (r"@J HULLSHAPE", Ok(r"[[3,2,3]]")),
        (r"[ ] HULLSHAPE", Ok(r"[[0]]")),
        (r"[ [ ] ] HULLSHAPE", Ok(r"[[1,0]]")),
        (r"[ [ ] [ ] ] HULLSHAPE", Ok(r"[[2,0]]")),
        (r"[ 1 [ 2 ] ] HULLSHAPE", Ok(r"[[2,1]]")),
        (r"[ [ 2 ] 1 ] HULLSHAPE", Ok(r"[[2,1]]")),
        (r"[ 1 2 3 ] HULLSHAPE", Ok(r"[[3]]")),
        (r"[ [ [ ] ] [ [ ] ] ] HULLSHAPE", Ok(r"[[2,1,0]]")),
        (r"[ [ 1 ] [ 1 2 ] ] HULLSHAPE", Ok(r"[[2,2]]")),
        (r"1 HULLSHAPE", Err(r"HULLSHAPE expects a LIST.")),
        (r"HULLSHAPE", Err(r"Empty stack.")),
        (r"[ [ 1 2 ] [ [ 1 ] 2 ] ] HULLSHAPE", Ok(r"[[2,2,1]]")),
        (r"[ [ [ 1 ] [ 2 3 ] ] [ 4 ] ] HULLSHAPE", Ok(r"[[2,2,2]]")),
        (r"@J T SHAPE", Ok(r"[[3,2]]")),
        (
            r"@J F SHAPE",
            Err(
                r"SHAPE expects that the sizes of the nested lists are coherent together to form a tensor (or multidimensional array).",
            ),
        ),
        (r"[ 1 [ 2 ] ] T SHAPE", Ok(r"[[2]]")),
        (r"T SHAPE", Err(r"Empty stack.")),
    ]);
}

#[test]
fn reshape() {
    check(&[
        (
            r"@T [ 4 6 ] RESHAPE",
            Ok(r"[[[1,2,3,4,5,6],[7,8,9,10,11,12],[13,14,15,16,17,18],[19,20,21,22,23,24]]]"),
        ),
        (
            r"@T [ 24 ] RESHAPE",
            Ok(r"[[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24]]"),
        ),
        (
            r"@T [ 2 2 2 3 ] RESHAPE",
            Ok(
                r"[[[[[1,2,3],[4,5,6]],[[7,8,9],[10,11,12]]],[[[13,14,15],[16,17,18]],[[19,20,21],[22,23,24]]]]]",
            ),
        ),
        (r"@M [ 3 2 ] RESHAPE", Ok(r"[[[1,2],[3,4],[5,6]]]")),
        (r"@M [ 6 1 ] RESHAPE", Ok(r"[[[1],[2],[3],[4],[5],[6]]]")),
        (r"@M [ 1 6 ] RESHAPE", Ok(r"[[[1,2,3,4,5,6]]]")),
        (
            r"@M [ 4 ] RESHAPE",
            Err(r"RESHAPE expected 4 elements, but found 6"),
        ),
        (
            r"@M [ 0 ] RESHAPE",
            Err(r"RESHAPE dimension cannot be negative or null."),
        ),
        (
            r"@M [ -2 -3 ] RESHAPE",
            Err(r"RESHAPE dimension cannot be negative or null."),
        ),
        (
            r"@M [ 'x' ] RESHAPE",
            Err(r"RESHAPE expects dimensions to be LONGs."),
        ),
        (
            r"@M 'x' RESHAPE",
            Err(r"RESHAPE expects a shape (a list of dimensions)."),
        ),
        (r"1 [ 1 ] RESHAPE", Err(r"RESHAPE operates on a list.")),
        (
            r"@M [ ] RESHAPE",
            Err(r"RESHAPE expected 1 elements, but found 6"),
        ),
        (
            r"[ ] [ ] RESHAPE",
            Err(r"RESHAPE expected 1 elements, but found 0"),
        ),
        (r"[ 1 ] [ ] RESHAPE", Ok(r"[[1]]")),
        (r"@J [ 7 ] RESHAPE", Ok(r"[[1,2,3,4,5,6,7]]")),
        (
            r"@J [ 7 1 ] RESHAPE",
            Ok(r"[[[1],[2],[3],[4],[5],[6],[7]]]"),
        ),
        (
            r"[ ] [ 1 ] RESHAPE",
            Err(r"RESHAPE expected 1 elements, but found 0"),
        ),
        (
            r"@M [ 2 4294967299 ] RESHAPE",
            Err(r"RESHAPE expected 8589934598 elements, but found 6"),
        ),
        (
            r"@M [ 2 4294967296 ] RESHAPE",
            Err(r"RESHAPE expected 8589934592 elements, but found 6"),
        ),
        (
            r"@M [ 3 2 ] RESHAPE DUP 0 GET 9 0 SET DROP",
            Ok(r"[[[9,2],[3,4],[5,6]]]"),
        ),
        (r"RESHAPE", Err(r"Empty stack.")),
        (r"[ 1 ] RESHAPE", Err(r"Empty stack.")),
        (
            r"1 RESHAPE",
            Err(r"RESHAPE expects a shape (a list of dimensions)."),
        ),
        (
            r"[ 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 ] [ 2 3 5 ] RESHAPE",
            Ok(
                r"[[[[1,2,3,4,5],[6,7,8,9,10],[11,12,13,14,15]],[[16,17,18,19,20],[21,22,23,24,25],[26,27,28,29,30]]]]",
            ),
        ),
        (
            r"@M [ 2 9223372036854775807 2 ] RESHAPE",
            Err(r"RESHAPE expected -4 elements, but found 6"),
        ),
        (
            r"@M 1 ->SET RESHAPE",
            Err(r"->SET operates on a LIST or SET."),
        ),
        (r"[ ] [ 4294967296 4294967296 ] RESHAPE", Err(r"/ by zero")),
        (
            r"[ ] [ 9223372036854775808 2 ] RESHAPE",
            Err(r#"For input string: "9223372036854775808""#),
        ),
        (
            r"[ ] [ 4611686018427387904 4 ] RESHAPE",
            Err(r"RESHAPE invalid shape, cannot create 4 elements from 0"),
        ),
        (
            r"@M [ 5 7378697624329859890 4294967295 ] RESHAPE",
            Err(r"Illegal Capacity: -6"),
        ),
        (
            r"[ 1 2 3 4 5 6 7 8 ] [ 3 3416063716081037768 4294967299 ] RESHAPE",
            Err(r"toIndex = 9"),
        ),
        (
            r"[ [ [ [ [ [ [ [ [ [ [ [ [ [ [ [ [ 1 ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] ] [ 1 ] RESHAPE",
            Err(r"FLATTEN cannot flatten lists nested more than 16 levels"),
        ),
    ]);
}

#[test]
fn permute() {
    check(&[
        (
            r"@T [ 0 1 2 ] PERMUTE",
            Ok(
                r"[[[[1,2,3,4],[5,6,7,8],[9,10,11,12]],[[13,14,15,16],[17,18,19,20],[21,22,23,24]]]]",
            ),
        ),
        (
            r"@T [ 2 1 0 ] PERMUTE",
            Ok(
                r"[[[[1,13],[5,17],[9,21]],[[2,14],[6,18],[10,22]],[[3,15],[7,19],[11,23]],[[4,16],[8,20],[12,24]]]]",
            ),
        ),
        (
            r"@T [ 1 0 2 ] PERMUTE",
            Ok(
                r"[[[[1,2,3,4],[13,14,15,16]],[[5,6,7,8],[17,18,19,20]],[[9,10,11,12],[21,22,23,24]]]]",
            ),
        ),
        (
            r"@T [ 2 0 1 ] PERMUTE",
            Ok(
                r"[[[[1,5,9],[13,17,21]],[[2,6,10],[14,18,22]],[[3,7,11],[15,19,23]],[[4,8,12],[16,20,24]]]]",
            ),
        ),
        (r"@M [ 1 0 ] PERMUTE", Ok(r"[[[1,4],[2,5],[3,6]]]")),
        (r"@M [ 0 1 ] PERMUTE", Ok(r"[[[1,2,3],[4,5,6]]]")),
        (
            r"@M [ 1 ] PERMUTE",
            Err(r"Index -1 out of bounds for length 1"),
        ),
        (r"@M [ 0 ] PERMUTE", Ok(r"[[[1,2,3],[4,5,6]]]")),
        (
            r"@M [ ] PERMUTE",
            Err(r"Index 0 out of bounds for length 0"),
        ),
        (
            r"@M [ 2 0 ] PERMUTE",
            Err(r"Index 2 out of bounds for length 2"),
        ),
        (r"@M [ -1 0 ] PERMUTE", Err(r"-1")),
        (
            r"@M [ 1 1 ] PERMUTE",
            Err(r"PERMUTE error: duplicate axis in permutation pattern."),
        ),
        (
            r"@M [ 1 'x' ] PERMUTE",
            Err(r"PERMUTE expects a LIST of LONG as second argument."),
        ),
        (
            r"@M 'x' PERMUTE",
            Err(r"PERMUTE expects a LIST of LONG as second argument."),
        ),
        (
            r"1 [ 0 ] PERMUTE",
            Err(r"PERMUTE expects a LIST as first argument."),
        ),
        (
            r"@J [ 1 0 ] PERMUTE",
            Err(
                r"PERMUTE expects that the sizes of the nested lists are coherent together to form a tensor (or multidimensional array).",
            ),
        ),
        (r"@J [ 1 0 ] T PERMUTE", Err(r"Index out of bound, 1 >= 1")),
        (r"@J [ 0 ] T PERMUTE", Ok(r"[[[1,2],[3],[4,[5,6,7]]]]")),
        (
            r"[ [ 1 2 ] [ 3 ] ] [ 1 0 ] T PERMUTE",
            Err(r"Index out of bound, 1 >= 1"),
        ),
        (r"[ 1 2 ] [ 0 ] PERMUTE", Ok(r"[[1,2]]")),
        (r"[ ] [ 0 ] PERMUTE", Ok(r"[[]]")),
        (r"[ [ ] [ ] ] [ 1 0 ] PERMUTE", Ok(r"[[]]")),
        (r"@M [ 1 0 ] F PERMUTE", Ok(r"[[[1,4],[2,5],[3,6]]]")),
        (r"@T [ 0 2 ] PERMUTE", Err(r"-1")),
        (r"@T [ 2 1 ] PERMUTE", Err(r"-1")),
        (r"@M [ 4294967297 0 ] PERMUTE", Err(r"-1")),
        (r"PERMUTE", Err(r"Empty stack.")),
        (r"[ 0 ] PERMUTE", Err(r"Empty stack.")),
        (r"T PERMUTE", Err(r"Empty stack.")),
        (
            r"@M [ 0 1 ] 1 PERMUTE",
            Err(r"PERMUTE expects a LIST of LONG as second argument."),
        ),
        (
            r"[ 1 [ 2 3 ] ] [ 0 1 ] T PERMUTE",
            Err(r"Index 1 out of bounds for length 1"),
        ),
        (
            r"[ [ 1 2 ] 3 ] [ 1 0 ] T PERMUTE",
            Err(r"Tried to get an element at a nested path that does not exist in the input list."),
        ),
        (
            r"@M [ 0 0 1 ] PERMUTE",
            Err(r"PERMUTE error: duplicate axis in permutation pattern."),
        ),
    ]);
}
