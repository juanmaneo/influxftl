// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Error paths of `SMARTPARSE`: the numbers its groups capture that Java's
//! `Double.parseDouble` / `Long.parseLong` reject, and the other ways a call can fail.

use crate::tests::test_stack;

/// A series with the single datapoint `b=2.5`.
const GTS: &str = "NEWGTS 'in' RENAME 10 NaN NaN NaN 'b=2.5' ADDVALUE";

/// The message of the error `script` fails with.
fn error_of(script: &str) -> String {
    let mut stack = test_stack();
    stack
        .exec_checked(script)
        .expect_err("exec_checked should fail")
        .to_string()
}

#[test]
fn rejected_numbers() {
    let nfe_abc = "For input string: \"abc\"";
    let cases = [
        // A STRING text.
        ("'abc' '(?<VDv>[a-z]+)' SMARTPARSE", nfe_abc),
        ("'abc' '(?<VLv>[a-z]+)' SMARTPARSE", nfe_abc),
        // The datapoint of a series.
        (
            "NEWGTS 'in' RENAME 10 NaN NaN NaN 'abc' ADDVALUE '(?<VLv>[a-z]+)' SMARTPARSE",
            nfe_abc,
        ),
        // Location and elevation.
        ("'abc 1' '(?<lat>[a-z]+) (?<lon>1)' SMARTPARSE", nfe_abc),
        ("'1 abc' '(?<lat>1) (?<lon>[a-z]+)' SMARTPARSE", nfe_abc),
        ("'abc' '(?<elev>[a-z]+)' SMARTPARSE", nfe_abc),
        // Timestamps, in every unit.
        ("'abc' '(?<Ts>[a-z]+)' SMARTPARSE", nfe_abc),
        ("'abc' '(?<Tms>[a-z]+)' SMARTPARSE", nfe_abc),
        ("'abc' '(?<Tus>[a-z]+)' SMARTPARSE", nfe_abc),
        ("'abc' '(?<Tns>[a-z]+)' SMARTPARSE", nfe_abc),
    ];
    for (script, expected) in cases {
        let message = error_of(script);
        assert!(
            message.contains(expected),
            "{script}\n  expected {expected:?}\n  actual   {message:?}"
        );
    }
    let message = error_of(&format!(
        "{GTS} '(?<Lname>[a-z]+)=(?<VLvalue>[0-9.]+)' SMARTPARSE"
    ));
    assert!(message.contains("For input string: \"2.5\""), "{message}");
}

#[test]
fn regexp_must_be_a_string() {
    let message = error_of("'abc' 1 SMARTPARSE");
    assert!(message.contains("expects a STRING regexp."), "{message}");
}

#[test]
fn matching_can_fail_at_runtime() {
    // Catastrophic backtracking, which fancy-regex bounds with an error where Java would
    // keep searching.
    let text = "ab".repeat(100);
    let mut stack = test_stack();
    let result = stack.exec_checked(&format!("'{text}' '(?<Lk>(a|b|ab)*)(?!x)c' SMARTPARSE"));
    assert!(result.is_err());
}
