// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Oracle-checked tests of `IMMUTABLE` and of the functions refusing to modify its result.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const ONE: &str = r"1";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@ONE", ONE)
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
fn immutable() {
    check(&[
        (
            r"[ 1 ] IMMUTABLE 2 +!",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"( 1 ) IMMUTABLE 2 +!",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ 1 ] IMMUTABLE 0 2 PUT",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"{ 'a' 1 } IMMUTABLE 2 'b' PUT",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ 1 ] IMMUTABLE 5 2 PUT",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ 1 ] IMMUTABLE 2 'a' PUT",
            Err(r"PUT expects a key which is an integer when operating on a list."),
        ),
        (
            r"[ 3 1 ] IMMUTABLE LSORT",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ ] IMMUTABLE LSORT",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ 3 1 ] IMMUTABLE 0 REMOVE",
            Err(r"UnsupportedOperationException"),
        ),
        (r"[ 1 ] IMMUTABLE 5 REMOVE", Ok(r"[null,[1]]")),
        (
            r"( 1 ) IMMUTABLE 5 REMOVE",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"{ 'a' 1 } IMMUTABLE 'a' REMOVE",
            Err(r"UnsupportedOperationException"),
        ),
        (r"[ 3 1 ] IMMUTABLE [ 2 ] APPEND", Ok(r"[[3,1,2]]")),
        (r"( 1 ) IMMUTABLE ( 2 ) APPEND", Ok(r"[[1,2]]")),
        (
            r"{ 1 1 } IMMUTABLE { 2 2 } APPEND",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ 3 1 ] IMMUTABLE REVERSE",
            Err(r"UnsupportedOperationException"),
        ),
        (r"[ 3 ] IMMUTABLE REVERSE", Ok(r"[[3]]")),
        (r"[ 3 1 ] IMMUTABLE CLONEREVERSE", Ok(r"[[1,3]]")),
        (
            r"[ 3 1 ] IMMUTABLE SHUFFLE",
            Err(r"UnsupportedOperationException"),
        ),
        (r"[ 3 ] IMMUTABLE SHUFFLE", Ok(r"[[3]]")),
        (r"[ 3 1 ] IMMUTABLE TYPEOF", Ok(r#"["LIST"]"#)),
        (r"[ 3 1 ] IMMUTABLE CLONE 3 +!", Ok(r"[[3,1,3],[3,1]]")),
        (
            r"1 IMMUTABLE",
            Err(r"IMMUTABLE operates on a LIST, SET or MAP."),
        ),
        (r"IMMUTABLE", Err(r"Empty stack.")),
        (
            r"{ 'b' 1 'a' 2 } IMMUTABLE MSORT",
            Err(r"MSORT operates on a map created by {} or ->MAP."),
        ),
        (
            r"[ 3 1 ] IMMUTABLE <% %> SORTBY",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ ] IMMUTABLE <% %> SORTBY",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ 3 1 ] IMMUTABLE <% - %> SORTWITH",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"{ 'b' 1 'a' 2 } IMMUTABLE <% DROP %> SORTBY",
            Err(r"SORTBY operates on a list or a map created by {} or ->MAP."),
        ),
        (
            r"{ 'b' 1 'a' 2 } IMMUTABLE <% DROP DROP DROP DROP 0 %> SORTWITH",
            Err(r"SORTWITH operates on a list or a map created by {} or ->MAP."),
        ),
        (r"[ [ 1 ] ] IMMUTABLE 0 GET 2 +!", Ok(r"[[1,2]]")),
        (
            r"[ 1 ] IMMUTABLE [ 0 ] 3 SET",
            Err(r"Index out of bound, 3 >= 1"),
        ),
        (
            r"[ 1 ] IMMUTABLE 2 0 SET",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ 1 ] IMMUTABLE 2 5 SET",
            Err(r"Index out of bound, 5 >= 1"),
        ),
        (r"[ [ 1 ] ] IMMUTABLE 2 [ 0 0 ] SET", Ok(r"[[[2]]]")),
        (
            r"[ [ 1 ] IMMUTABLE ] 2 [ 0 0 ] SET",
            Err(r"UnsupportedOperationException"),
        ),
        (
            r"[ [ 1 ] IMMUTABLE ] 2 [ 0 5 ] SET",
            Err(r"UnsupportedOperationException"),
        ),
        (r"[ 1 ] IMMUTABLE DUP 0 GET", Ok(r"[1,[1]]")),
        (r"{ 'a' 1 } IMMUTABLE 'a' GET", Ok(r"[1]")),
        (r"( 1 2 ) IMMUTABLE SIZE", Ok(r"[2]")),
        (r"[ 1 2 ] IMMUTABLE ->JSON", Ok(r#"["[1,2]"]"#)),
        (r"[ 1 ] DUP IMMUTABLE SWAP 2 +! DROP", Ok(r"[[1]]")),
        (
            r"{ 'a' 1 } IMMUTABLE { 'b' 2 } APPEND",
            Err(r"UnsupportedOperationException"),
        ),
    ]);
}
