// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `THRESHOLDTEST`, `ZSCORETEST`, `GRUBBSTEST` and `ESDTEST`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const N: &str = r"NEWGTS 'n' RENAME 1 NaN NaN NaN 10.1122 ADDVALUE 2 NaN NaN NaN 10.9547 ADDVALUE 3 NaN NaN NaN 11.296 ADDVALUE 4 NaN NaN NaN 10.7553 ADDVALUE 5 NaN NaN NaN 10.1998 ADDVALUE 6 NaN NaN NaN 10.1215 ADDVALUE 7 NaN NaN NaN 9.7926 ADDVALUE 8 NaN NaN NaN 9.5727 ADDVALUE 9 NaN NaN NaN 8.6577 ADDVALUE 10 NaN NaN NaN 8.6532 ADDVALUE 11 NaN NaN NaN 9.5062 ADDVALUE 12 NaN NaN NaN 9.8628 ADDVALUE 13 NaN NaN NaN 10.6753 ADDVALUE 14 NaN NaN NaN 10.9517 ADDVALUE 15 NaN NaN NaN 10.9075 ADDVALUE 16 NaN NaN NaN 10.9071 ADDVALUE 17 NaN NaN NaN 11.5696 ADDVALUE 18 NaN NaN NaN 10.3255 ADDVALUE 19 NaN NaN NaN 9.6894 ADDVALUE 20 NaN NaN NaN 9.1145 ADDVALUE 21 NaN NaN NaN 9.1489 ADDVALUE 22 NaN NaN NaN 9.0436 ADDVALUE 23 NaN NaN NaN 9.0211 ADDVALUE 24 NaN NaN NaN 9.4448 ADDVALUE 25 NaN NaN NaN 10.0156 ADDVALUE 26 NaN NaN NaN 10.6279 ADDVALUE 27 NaN NaN NaN 11.1328 ADDVALUE 28 NaN NaN NaN 11.0536 ADDVALUE 29 NaN NaN NaN 10.398 ADDVALUE 30 NaN NaN NaN 10.8608 ADDVALUE 31 NaN NaN NaN 9.7806 ADDVALUE 32 NaN NaN NaN 9.6607 ADDVALUE 33 NaN NaN NaN 8.8461 ADDVALUE 34 NaN NaN NaN 9.0392 ADDVALUE 35 NaN NaN NaN 8.7908 ADDVALUE 36 NaN NaN NaN 9.3132 ADDVALUE 37 NaN NaN NaN 10.6476 ADDVALUE 38 NaN NaN NaN 10.1353 ADDVALUE 39 NaN NaN NaN 10.8582 ADDVALUE 40 NaN NaN NaN 10.5159 ADDVALUE";
const O: &str = r"NEWGTS 'o' RENAME 1 NaN NaN NaN 10.1122 ADDVALUE 2 NaN NaN NaN 10.9547 ADDVALUE 3 NaN NaN NaN 11.296 ADDVALUE 4 NaN NaN NaN 10.7553 ADDVALUE 5 NaN NaN NaN 10.1998 ADDVALUE 6 NaN NaN NaN 25.0 ADDVALUE 7 NaN NaN NaN 9.7926 ADDVALUE 8 NaN NaN NaN 9.5727 ADDVALUE 9 NaN NaN NaN 8.6577 ADDVALUE 10 NaN NaN NaN 8.6532 ADDVALUE 11 NaN NaN NaN 9.5062 ADDVALUE 12 NaN NaN NaN 9.8628 ADDVALUE 13 NaN NaN NaN 10.6753 ADDVALUE 14 NaN NaN NaN 10.9517 ADDVALUE 15 NaN NaN NaN 10.9075 ADDVALUE 16 NaN NaN NaN 10.9071 ADDVALUE 17 NaN NaN NaN 11.5696 ADDVALUE 18 NaN NaN NaN -8.0 ADDVALUE 19 NaN NaN NaN 9.6894 ADDVALUE 20 NaN NaN NaN 9.1145 ADDVALUE 21 NaN NaN NaN 9.1489 ADDVALUE 22 NaN NaN NaN 9.0436 ADDVALUE 23 NaN NaN NaN 9.0211 ADDVALUE 24 NaN NaN NaN 9.4448 ADDVALUE 25 NaN NaN NaN 10.0156 ADDVALUE 26 NaN NaN NaN 10.6279 ADDVALUE 27 NaN NaN NaN 11.1328 ADDVALUE 28 NaN NaN NaN 11.0536 ADDVALUE 29 NaN NaN NaN 10.398 ADDVALUE 30 NaN NaN NaN 10.8608 ADDVALUE 31 NaN NaN NaN 18.5 ADDVALUE 32 NaN NaN NaN 9.6607 ADDVALUE 33 NaN NaN NaN 8.8461 ADDVALUE 34 NaN NaN NaN 9.0392 ADDVALUE 35 NaN NaN NaN 8.7908 ADDVALUE 36 NaN NaN NaN 9.3132 ADDVALUE 37 NaN NaN NaN 10.6476 ADDVALUE 38 NaN NaN NaN 10.1353 ADDVALUE 39 NaN NaN NaN 10.8582 ADDVALUE 40 NaN NaN NaN 10.5159 ADDVALUE";
const S3: &str = r"NEWGTS 's3' RENAME 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 1.5 ADDVALUE 3 NaN NaN NaN 9.0 ADDVALUE";
const S4: &str = r"NEWGTS 's4' RENAME 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 1.5 ADDVALUE 3 NaN NaN NaN 1.2 ADDVALUE 4 NaN NaN NaN 9.0 ADDVALUE";
const S5: &str = r"NEWGTS 's5' RENAME 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 1.5 ADDVALUE 3 NaN NaN NaN 1.2 ADDVALUE 4 NaN NaN NaN 9.0 ADDVALUE 5 NaN NaN NaN 1.1 ADDVALUE";
const C: &str = r"NEWGTS 'c' RENAME 1 NaN NaN NaN 2.0 ADDVALUE 2 NaN NaN NaN 2.0 ADDVALUE 3 NaN NaN NaN 2.0 ADDVALUE 4 NaN NaN NaN 2.0 ADDVALUE";
const M: &str = r"NEWGTS 'm' RENAME 1 NaN NaN NaN 2.0 ADDVALUE 2 NaN NaN NaN 2.0 ADDVALUE 3 NaN NaN NaN 2.0 ADDVALUE 4 NaN NaN NaN 7.0 ADDVALUE 5 NaN NaN NaN 2.0 ADDVALUE";
const L: &str = r"NEWGTS 'l' RENAME 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 30 ADDVALUE";
const NAN: &str = r"NEWGTS 'nan' RENAME 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN 1.2 ADDVALUE 4 NaN NaN NaN 9.0 ADDVALUE 5 NaN NaN NaN 1.1 ADDVALUE";
const BIG: &str = r"NEWGTS 'big' RENAME 1 NaN NaN NaN -0.2881 ADDVALUE 2 NaN NaN NaN 0.6625 ADDVALUE 3 NaN NaN NaN -0.0927 ADDVALUE 4 NaN NaN NaN 0.3702 ADDVALUE 5 NaN NaN NaN 0.1033 ADDVALUE 6 NaN NaN NaN -1.0061 ADDVALUE 7 NaN NaN NaN 0.5637 ADDVALUE 8 NaN NaN NaN -0.812 ADDVALUE 9 NaN NaN NaN 1.8718 ADDVALUE 10 NaN NaN NaN -0.5052 ADDVALUE 11 NaN NaN NaN 0.6861 ADDVALUE 12 NaN NaN NaN 0.0023 ADDVALUE 13 NaN NaN NaN 0.9524 ADDVALUE 14 NaN NaN NaN -0.6022 ADDVALUE 15 NaN NaN NaN 0.9989 ADDVALUE 16 NaN NaN NaN -0.1239 ADDVALUE 17 NaN NaN NaN 1.2633 ADDVALUE 18 NaN NaN NaN 0.6242 ADDVALUE 19 NaN NaN NaN 0.1413 ADDVALUE 20 NaN NaN NaN -0.7803 ADDVALUE 21 NaN NaN NaN 0.7678 ADDVALUE 22 NaN NaN NaN 0.4682 ADDVALUE 23 NaN NaN NaN 1.6419 ADDVALUE 24 NaN NaN NaN -0.377 ADDVALUE 25 NaN NaN NaN 0.5548 ADDVALUE 26 NaN NaN NaN 0.5079 ADDVALUE 27 NaN NaN NaN 0.283 ADDVALUE 28 NaN NaN NaN 0.2084 ADDVALUE 29 NaN NaN NaN 0.1821 ADDVALUE 30 NaN NaN NaN -0.5984 ADDVALUE 31 NaN NaN NaN -1.0145 ADDVALUE 32 NaN NaN NaN -0.3965 ADDVALUE 33 NaN NaN NaN 0.5908 ADDVALUE 34 NaN NaN NaN 1.5112 ADDVALUE 35 NaN NaN NaN 0.9771 ADDVALUE 36 NaN NaN NaN 1.0533 ADDVALUE 37 NaN NaN NaN 0.7773 ADDVALUE 38 NaN NaN NaN 0.6985 ADDVALUE 39 NaN NaN NaN 0.1834 ADDVALUE 40 NaN NaN NaN 0.7715 ADDVALUE 41 NaN NaN NaN 1.7737 ADDVALUE 42 NaN NaN NaN -0.3276 ADDVALUE 43 NaN NaN NaN -0.6938 ADDVALUE 44 NaN NaN NaN 1.9601 ADDVALUE 45 NaN NaN NaN 0.2447 ADDVALUE 46 NaN NaN NaN 0.971 ADDVALUE 47 NaN NaN NaN 0.8739 ADDVALUE 48 NaN NaN NaN -1.1357 ADDVALUE 49 NaN NaN NaN 2.4335 ADDVALUE 50 NaN NaN NaN 1.7758 ADDVALUE 51 NaN NaN NaN 0.1769 ADDVALUE 52 NaN NaN NaN 0.7525 ADDVALUE 53 NaN NaN NaN 0.1269 ADDVALUE 54 NaN NaN NaN -0.8842 ADDVALUE 55 NaN NaN NaN -0.1121 ADDVALUE 56 NaN NaN NaN 0.374 ADDVALUE 57 NaN NaN NaN 1.1158 ADDVALUE 58 NaN NaN NaN 0.7093 ADDVALUE 59 NaN NaN NaN 0.0595 ADDVALUE 60 NaN NaN NaN 1.3548 ADDVALUE 61 NaN NaN NaN -0.7607 ADDVALUE 62 NaN NaN NaN 0.7929 ADDVALUE 63 NaN NaN NaN 1.1122 ADDVALUE 64 NaN NaN NaN -0.292 ADDVALUE 65 NaN NaN NaN -1.7906 ADDVALUE 66 NaN NaN NaN -0.9063 ADDVALUE 67 NaN NaN NaN 0.237 ADDVALUE 68 NaN NaN NaN 0.5278 ADDVALUE 69 NaN NaN NaN 1.5482 ADDVALUE 70 NaN NaN NaN -1.0042 ADDVALUE 71 NaN NaN NaN 0.002 ADDVALUE 72 NaN NaN NaN 0.6488 ADDVALUE 73 NaN NaN NaN -0.1577 ADDVALUE 74 NaN NaN NaN -2.3697 ADDVALUE 75 NaN NaN NaN 0.8065 ADDVALUE 76 NaN NaN NaN 2.3123 ADDVALUE 77 NaN NaN NaN 1.0043 ADDVALUE 78 NaN NaN NaN -0.9174 ADDVALUE 79 NaN NaN NaN -0.4124 ADDVALUE 80 NaN NaN NaN 0.2218 ADDVALUE 81 NaN NaN NaN 2.489 ADDVALUE 82 NaN NaN NaN 0.6174 ADDVALUE 83 NaN NaN NaN 0.1136 ADDVALUE 84 NaN NaN NaN 1.5575 ADDVALUE 85 NaN NaN NaN -0.0817 ADDVALUE 86 NaN NaN NaN 1.8614 ADDVALUE 87 NaN NaN NaN -0.685 ADDVALUE 88 NaN NaN NaN -0.4748 ADDVALUE 89 NaN NaN NaN 0.7209 ADDVALUE 90 NaN NaN NaN 1.4244 ADDVALUE 91 NaN NaN NaN 0.6539 ADDVALUE 92 NaN NaN NaN 0.3016 ADDVALUE 93 NaN NaN NaN -1.8216 ADDVALUE 94 NaN NaN NaN -0.7129 ADDVALUE 95 NaN NaN NaN -0.6106 ADDVALUE 96 NaN NaN NaN -0.5336 ADDVALUE 97 NaN NaN NaN 0.5864 ADDVALUE 98 NaN NaN NaN -0.3352 ADDVALUE 99 NaN NaN NaN 0.7877 ADDVALUE 100 NaN NaN NaN 0.0823 ADDVALUE 101 NaN NaN NaN -0.3328 ADDVALUE 102 NaN NaN NaN 0.1182 ADDVALUE 103 NaN NaN NaN 0.4288 ADDVALUE 104 NaN NaN NaN 0.8581 ADDVALUE 105 NaN NaN NaN -0.4775 ADDVALUE 106 NaN NaN NaN -0.2327 ADDVALUE 107 NaN NaN NaN -1.3636 ADDVALUE 108 NaN NaN NaN 1.6038 ADDVALUE 109 NaN NaN NaN 1.4518 ADDVALUE 110 NaN NaN NaN -0.1788 ADDVALUE 111 NaN NaN NaN -0.4784 ADDVALUE 112 NaN NaN NaN -1.2359 ADDVALUE 113 NaN NaN NaN 0.1699 ADDVALUE 114 NaN NaN NaN 0.2063 ADDVALUE 115 NaN NaN NaN 2.1817 ADDVALUE 116 NaN NaN NaN 0.2463 ADDVALUE 117 NaN NaN NaN -0.7779 ADDVALUE 118 NaN NaN NaN -2.4446 ADDVALUE 119 NaN NaN NaN 1.4094 ADDVALUE 120 NaN NaN NaN 0.1894 ADDVALUE 200 NaN NaN NaN 15.0 ADDVALUE 201 NaN NaN NaN -12.0 ADDVALUE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@NAN", NAN)
        .replace("@BIG", BIG)
        .replace("@S3", S3)
        .replace("@S4", S4)
        .replace("@S5", S5)
        .replace("@N", N)
        .replace("@O", O)
        .replace("@C", C)
        .replace("@M", M)
        .replace("@L", L)
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
fn thresholdtest() {
    check(&[
        (r"@O 15.0 THRESHOLDTEST", Ok(r"[[6,31]]")),
        (
            r"@O -100.0 THRESHOLDTEST",
            Ok(
                r"[[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40]]",
            ),
        ),
        (r"@O 1e9 THRESHOLDTEST", Ok(r"[[]]")),
        (r"[ @O @N ] 12.0 THRESHOLDTEST", Ok(r"[[[6,31],[]]]")),
        (r"@L 2.0 THRESHOLDTEST", Err(r"GTS must be of type Double.")),
        (
            r"@O 15 THRESHOLDTEST",
            Err(r"THRESHOLDTEST expects a threshold (a DOUBLE) on top of the stack."),
        ),
        (
            r"@O THRESHOLDTEST",
            Err(r"THRESHOLDTEST expects a threshold (a DOUBLE) on top of the stack."),
        ),
        (r"THRESHOLDTEST", Err(r"Empty stack.")),
        (
            r"NEWGTS 1.0 THRESHOLDTEST",
            Err(r"GTS must be of type Double."),
        ),
        (
            r"[ @O 1 ] 1.0 THRESHOLDTEST",
            Err(r"THRESHOLDTEST can only operate on Geo Time Series instances."),
        ),
        (r"@NAN 1.0 THRESHOLDTEST", Ok(r"[[1,3,4,5]]")),
        (r"@O NaN THRESHOLDTEST", Ok(r"[[]]")),
        (
            r"@S5 1000 1 TIMECLIP 1.0 THRESHOLDTEST",
            Err(r"GTS must be of type Double."),
        ),
        (
            r"@S5 SORT 1000 1 TIMECLIP true ZSCORETEST",
            Err(r"GTS must be of type Double."),
        ),
        (
            r"@S5 SORT 1000 1 TIMECLIP 1 NaN NaN NaN 1 ADDVALUE TYPEOF",
            Ok(r#"["GTS"]"#),
        ),
        (
            r"@S5 SORT 1000 1 TIMECLIP 1 NaN NaN NaN 1 ADDVALUE VALUES",
            Ok(r"[[1]]"),
        ),
    ]);
}

#[test]
fn tests() {
    check(&[
        (r"@O false 3.0 ZSCORETEST", Ok(r"[[6,18]]")),
        (r"@N false 3.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@S3 false 3.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@S5 false 3.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@C false 3.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@M false 3.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@NAN false 3.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@O true 3.0 ZSCORETEST", Ok(r"[[6,18,31]]")),
        (r"@N true 3.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@S3 true 3.0 ZSCORETEST", Ok(r"[[3]]")),
        (r"@S5 true 3.0 ZSCORETEST", Ok(r"[[4]]")),
        (r"@C true 3.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@M true 3.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@NAN true 3.0 ZSCORETEST", Ok(r"[[4]]")),
        (r"@O false ZSCORETEST", Ok(r"[[6,18]]")),
        (r"@N false ZSCORETEST", Ok(r"[[]]")),
        (r"@S3 false ZSCORETEST", Ok(r"[[]]")),
        (r"@S5 false ZSCORETEST", Ok(r"[[]]")),
        (r"@C false ZSCORETEST", Ok(r"[[]]")),
        (r"@M false ZSCORETEST", Ok(r"[[]]")),
        (r"@NAN false ZSCORETEST", Ok(r"[[]]")),
        (r"@O true ZSCORETEST", Ok(r"[[6,18,31]]")),
        (r"@N true ZSCORETEST", Ok(r"[[]]")),
        (r"@S3 true ZSCORETEST", Ok(r"[[3]]")),
        (r"@S5 true ZSCORETEST", Ok(r"[[4]]")),
        (r"@C true ZSCORETEST", Ok(r"[[]]")),
        (r"@M true ZSCORETEST", Ok(r"[[]]")),
        (r"@NAN true ZSCORETEST", Ok(r"[[4]]")),
        (r"@O false 1.0 ZSCORETEST", Ok(r"[[6,18,31]]")),
        (
            r"@N false 1.0 ZSCORETEST",
            Ok(r"[[2,3,9,10,14,15,16,17,20,21,22,23,27,28,33,34,35]]"),
        ),
        (r"@S3 false 1.0 ZSCORETEST", Ok(r"[[3]]")),
        (r"@S5 false 1.0 ZSCORETEST", Ok(r"[[4]]")),
        (r"@C false 1.0 ZSCORETEST", Ok(r"[[]]")),
        (r"@M false 1.0 ZSCORETEST", Ok(r"[[4]]")),
        (r"@NAN false 1.0 ZSCORETEST", Ok(r"[[]]")),
        (
            r"@O true 0.5 ZSCORETEST",
            Ok(r"[[2,3,4,6,9,10,11,14,15,16,17,18,20,21,22,23,24,27,28,30,31,33,34,35,36,39]]"),
        ),
        (
            r"@N true 0.5 ZSCORETEST",
            Ok(r"[[2,3,4,8,9,10,11,13,14,15,16,17,20,21,22,23,24,27,28,30,33,34,35,36,37,39]]"),
        ),
        (r"@S3 true 0.5 ZSCORETEST", Ok(r"[[1,3]]")),
        (r"@S5 true 0.5 ZSCORETEST", Ok(r"[[1,2,4]]")),
        (r"@C true 0.5 ZSCORETEST", Ok(r"[[]]")),
        (r"@M true 0.5 ZSCORETEST", Ok(r"[[4]]")),
        (r"@NAN true 0.5 ZSCORETEST", Ok(r"[[1,4]]")),
        (r"@O false NaN ZSCORETEST", Ok(r"[[]]")),
        (r"@N false NaN ZSCORETEST", Ok(r"[[]]")),
        (r"@S3 false NaN ZSCORETEST", Ok(r"[[]]")),
        (r"@S5 false NaN ZSCORETEST", Ok(r"[[]]")),
        (r"@C false NaN ZSCORETEST", Ok(r"[[]]")),
        (r"@M false NaN ZSCORETEST", Ok(r"[[]]")),
        (r"@NAN false NaN ZSCORETEST", Ok(r"[[]]")),
        (r"@L false ZSCORETEST", Err(r"GTS must be of type Double.")),
        (
            r"@O 3.0 ZSCORETEST",
            Err(
                r"ZSCORETEST expects a flag (a BOOLEAN) that indicates wether to use modified z-score below the THRESHOLD level.",
            ),
        ),
        (
            r"@O 'x' ZSCORETEST",
            Err(
                r"ZSCORETEST expects a threshold level (a DOUBLE) or a flag (a BOOLEAN) on top of the stack.",
            ),
        ),
        (
            r"@O 1 ZSCORETEST",
            Err(
                r"ZSCORETEST expects a threshold level (a DOUBLE) or a flag (a BOOLEAN) on top of the stack.",
            ),
        ),
        (r"ZSCORETEST", Err(r"Empty stack.")),
        (r"[ @O @N ] true ZSCORETEST", Ok(r"[[[6,18,31],[]]]")),
        (r"@O false 0.05 GRUBBSTEST", Ok(r"[[18]]")),
        (r"@N false 0.05 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S3 false 0.05 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S4 false 0.05 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@S5 false 0.05 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@C false 0.05 GRUBBSTEST", Ok(r"[[]]")),
        (r"@M false 0.05 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@NAN false 0.05 GRUBBSTEST", Ok(r"[[]]")),
        (r"@BIG false 0.05 GRUBBSTEST", Ok(r"[[200]]")),
        (r"@O true 0.05 GRUBBSTEST", Ok(r"[[18]]")),
        (r"@N true 0.05 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S3 true 0.05 GRUBBSTEST", Ok(r"[[3]]")),
        (r"@S4 true 0.05 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@S5 true 0.05 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@C true 0.05 GRUBBSTEST", Ok(r"[[]]")),
        (r"@M true 0.05 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@NAN true 0.05 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@BIG true 0.05 GRUBBSTEST", Ok(r"[[200]]")),
        (r"@O false GRUBBSTEST", Ok(r"[[18]]")),
        (r"@N false GRUBBSTEST", Ok(r"[[]]")),
        (r"@S3 false GRUBBSTEST", Ok(r"[[]]")),
        (r"@S4 false GRUBBSTEST", Ok(r"[[4]]")),
        (r"@S5 false GRUBBSTEST", Ok(r"[[4]]")),
        (r"@C false GRUBBSTEST", Ok(r"[[]]")),
        (r"@M false GRUBBSTEST", Ok(r"[[4]]")),
        (r"@NAN false GRUBBSTEST", Ok(r"[[]]")),
        (r"@BIG false GRUBBSTEST", Ok(r"[[200]]")),
        (r"@O true GRUBBSTEST", Ok(r"[[18]]")),
        (r"@N true GRUBBSTEST", Ok(r"[[]]")),
        (r"@S3 true GRUBBSTEST", Ok(r"[[3]]")),
        (r"@S4 true GRUBBSTEST", Ok(r"[[4]]")),
        (r"@S5 true GRUBBSTEST", Ok(r"[[4]]")),
        (r"@C true GRUBBSTEST", Ok(r"[[]]")),
        (r"@M true GRUBBSTEST", Ok(r"[[4]]")),
        (r"@NAN true GRUBBSTEST", Ok(r"[[4]]")),
        (r"@BIG true GRUBBSTEST", Ok(r"[[200]]")),
        (r"@O false 0.5 GRUBBSTEST", Ok(r"[[18]]")),
        (r"@N false 0.5 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S3 false 0.5 GRUBBSTEST", Ok(r"[[3]]")),
        (r"@S4 false 0.5 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@S5 false 0.5 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@C false 0.5 GRUBBSTEST", Ok(r"[[]]")),
        (r"@M false 0.5 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@NAN false 0.5 GRUBBSTEST", Ok(r"[[]]")),
        (r"@BIG false 0.5 GRUBBSTEST", Ok(r"[[200]]")),
        (r"@O true 0.001 GRUBBSTEST", Ok(r"[[18]]")),
        (r"@N true 0.001 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S3 true 0.001 GRUBBSTEST", Ok(r"[[3]]")),
        (r"@S4 true 0.001 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@S5 true 0.001 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@C true 0.001 GRUBBSTEST", Ok(r"[[]]")),
        (r"@M true 0.001 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@NAN true 0.001 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@BIG true 0.001 GRUBBSTEST", Ok(r"[[200]]")),
        (r"@O false 0.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@N false 0.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S3 false 0.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S4 false 0.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S5 false 0.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@C false 0.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@M false 0.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@NAN false 0.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@BIG false 0.0 GRUBBSTEST", Ok(r"[[]]")),
        (
            r"@O false -0.5 GRUBBSTEST",
            Err(r"-0.006 out of [0, 1] range"),
        ),
        (
            r"@N false -0.5 GRUBBSTEST",
            Err(r"-0.006 out of [0, 1] range"),
        ),
        (
            r"@S3 false -0.5 GRUBBSTEST",
            Err(r"-0.083 out of [0, 1] range"),
        ),
        (
            r"@S4 false -0.5 GRUBBSTEST",
            Err(r"-0.062 out of [0, 1] range"),
        ),
        (
            r"@S5 false -0.5 GRUBBSTEST",
            Err(r"-0.05 out of [0, 1] range"),
        ),
        (r"@C false -0.5 GRUBBSTEST", Ok(r"[[]]")),
        (
            r"@M false -0.5 GRUBBSTEST",
            Err(r"-0.05 out of [0, 1] range"),
        ),
        (
            r"@NAN false -0.5 GRUBBSTEST",
            Err(r"-0.05 out of [0, 1] range"),
        ),
        (
            r"@BIG false -0.5 GRUBBSTEST",
            Err(r"-0.002 out of [0, 1] range"),
        ),
        (r"@O false 20.0 GRUBBSTEST", Ok(r"[[18]]")),
        (r"@N false 20.0 GRUBBSTEST", Ok(r"[[17]]")),
        (
            r"@S3 false 20.0 GRUBBSTEST",
            Err(r"3.333 out of [0, 1] range"),
        ),
        (
            r"@S4 false 20.0 GRUBBSTEST",
            Err(r"2.5 out of [0, 1] range"),
        ),
        (r"@S5 false 20.0 GRUBBSTEST", Err(r"2 out of [0, 1] range")),
        (r"@C false 20.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@M false 20.0 GRUBBSTEST", Err(r"2 out of [0, 1] range")),
        (r"@NAN false 20.0 GRUBBSTEST", Err(r"2 out of [0, 1] range")),
        (r"@BIG false 20.0 GRUBBSTEST", Ok(r"[[200]]")),
        (
            r"@O false NaN GRUBBSTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (
            r"@N false NaN GRUBBSTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (
            r"@S3 false NaN GRUBBSTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [-1, 1], values: [NaN, NaN]",
            ),
        ),
        (
            r"@S4 false NaN GRUBBSTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [-1, 1], values: [NaN, NaN]",
            ),
        ),
        (
            r"@S5 false NaN GRUBBSTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (r"@C false NaN GRUBBSTEST", Ok(r"[[]]")),
        (
            r"@M false NaN GRUBBSTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (
            r"@NAN false NaN GRUBBSTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (
            r"@BIG false NaN GRUBBSTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (r"@O false 1.0 GRUBBSTEST", Ok(r"[[18]]")),
        (r"@N false 1.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S3 false 1.0 GRUBBSTEST", Ok(r"[[3]]")),
        (r"@S4 false 1.0 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@S5 false 1.0 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@C false 1.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@M false 1.0 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@NAN false 1.0 GRUBBSTEST", Ok(r"[[]]")),
        (r"@BIG false 1.0 GRUBBSTEST", Ok(r"[[200]]")),
        (r"@O false 0.99999999 GRUBBSTEST", Ok(r"[[18]]")),
        (r"@N false 0.99999999 GRUBBSTEST", Ok(r"[[]]")),
        (r"@S3 false 0.99999999 GRUBBSTEST", Ok(r"[[3]]")),
        (r"@S4 false 0.99999999 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@S5 false 0.99999999 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@C false 0.99999999 GRUBBSTEST", Ok(r"[[]]")),
        (r"@M false 0.99999999 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@NAN false 0.99999999 GRUBBSTEST", Ok(r"[[]]")),
        (r"@BIG false 0.99999999 GRUBBSTEST", Ok(r"[[200]]")),
        (r"@S3 false 3.0 GRUBBSTEST", Ok(r"[[3]]")),
        (r"@S3 false 3.6 GRUBBSTEST", Ok(r"[[3]]")),
        (r"@S3 1 false 2.4 ESDTEST", Ok(r"[[3]]")),
        (r"@S3 1 false 1.5 ESDTEST", Ok(r"[[3]]")),
        (r"@S3 false 4.5 GRUBBSTEST", Ok(r"[[3]]")),
        (r"@S4 false 4.0 GRUBBSTEST", Ok(r"[[4]]")),
        (r"@S4 1 false 1.0 ESDTEST", Ok(r"[[4]]")),
        (
            r"NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 2.0 ADDVALUE false GRUBBSTEST",
            Ok(r"[[]]"),
        ),
        (r"@L false GRUBBSTEST", Err(r"GTS must be of type Double.")),
        (
            r"@O 0.05 GRUBBSTEST",
            Err(
                r"GRUBBSTEST expects a flag (a BOOLEAN) that indicates wether to use modified z-score below the significance level.",
            ),
        ),
        (
            r"@O 'x' GRUBBSTEST",
            Err(
                r"GRUBBSTEST expects a significance level (a DOUBLE) or a flag (a BOOLEAN) on top of the stack.",
            ),
        ),
        (r"GRUBBSTEST", Err(r"Empty stack.")),
        (r"[ @O @N @S3 ] false GRUBBSTEST", Ok(r"[[[18],[],[]]]")),
        (r"@O 5 false 0.05 ESDTEST", Ok(r"[[18,6,31]]")),
        (r"@N 5 false 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@S3 5 false 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@S5 5 false 0.05 ESDTEST", Ok(r"[[4]]")),
        (r"@C 5 false 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@M 5 false 0.05 ESDTEST", Ok(r"[[4]]")),
        (r"@NAN 5 false 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@BIG 5 false 0.05 ESDTEST", Ok(r"[[200,201]]")),
        (r"@O 5 true 0.05 ESDTEST", Ok(r"[[18,6,31]]")),
        (r"@N 5 true 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@S3 5 true 0.05 ESDTEST", Ok(r"[[3]]")),
        (r"@S5 5 true 0.05 ESDTEST", Ok(r"[[4,2]]")),
        (r"@C 5 true 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@M 5 true 0.05 ESDTEST", Ok(r"[[4]]")),
        (r"@NAN 5 true 0.05 ESDTEST", Ok(r"[[4]]")),
        (r"@BIG 5 true 0.05 ESDTEST", Ok(r"[[200,201]]")),
        (r"@O 5 false ESDTEST", Ok(r"[[18,6,31]]")),
        (r"@N 5 false ESDTEST", Ok(r"[[]]")),
        (r"@S3 5 false ESDTEST", Ok(r"[[]]")),
        (r"@S5 5 false ESDTEST", Ok(r"[[4]]")),
        (r"@C 5 false ESDTEST", Ok(r"[[]]")),
        (r"@M 5 false ESDTEST", Ok(r"[[4]]")),
        (r"@NAN 5 false ESDTEST", Ok(r"[[]]")),
        (r"@BIG 5 false ESDTEST", Ok(r"[[200,201]]")),
        (r"@O 5 true ESDTEST", Ok(r"[[18,6,31]]")),
        (r"@N 5 true ESDTEST", Ok(r"[[]]")),
        (r"@S3 5 true ESDTEST", Ok(r"[[3]]")),
        (r"@S5 5 true ESDTEST", Ok(r"[[4,2]]")),
        (r"@C 5 true ESDTEST", Ok(r"[[]]")),
        (r"@M 5 true ESDTEST", Ok(r"[[4]]")),
        (r"@NAN 5 true ESDTEST", Ok(r"[[4]]")),
        (r"@BIG 5 true ESDTEST", Ok(r"[[200,201]]")),
        (r"@O 1 false ESDTEST", Ok(r"[[18]]")),
        (r"@N 1 false ESDTEST", Ok(r"[[]]")),
        (r"@S3 1 false ESDTEST", Ok(r"[[]]")),
        (r"@S5 1 false ESDTEST", Ok(r"[[4]]")),
        (r"@C 1 false ESDTEST", Ok(r"[[]]")),
        (r"@M 1 false ESDTEST", Ok(r"[[4]]")),
        (r"@NAN 1 false ESDTEST", Ok(r"[[]]")),
        (r"@BIG 1 false ESDTEST", Ok(r"[[200]]")),
        (r"@O 0 false ESDTEST", Ok(r"[[]]")),
        (r"@N 0 false ESDTEST", Ok(r"[[]]")),
        (r"@S3 0 false ESDTEST", Ok(r"[[]]")),
        (r"@S5 0 false ESDTEST", Ok(r"[[]]")),
        (r"@C 0 false ESDTEST", Ok(r"[[]]")),
        (r"@M 0 false ESDTEST", Ok(r"[[]]")),
        (r"@NAN 0 false ESDTEST", Ok(r"[[]]")),
        (r"@BIG 0 false ESDTEST", Ok(r"[[]]")),
        (r"@O -3 false ESDTEST", Ok(r"[[]]")),
        (r"@N -3 false ESDTEST", Ok(r"[[]]")),
        (r"@S3 -3 false ESDTEST", Ok(r"[[]]")),
        (r"@S5 -3 false ESDTEST", Ok(r"[[]]")),
        (r"@C -3 false ESDTEST", Ok(r"[[]]")),
        (r"@M -3 false ESDTEST", Ok(r"[[]]")),
        (r"@NAN -3 false ESDTEST", Ok(r"[[]]")),
        (r"@BIG -3 false ESDTEST", Ok(r"[[]]")),
        (r"@O 10 true 0.2 ESDTEST", Ok(r"[[18,6,31]]")),
        (r"@N 10 true 0.2 ESDTEST", Ok(r"[[]]")),
        (r"@S3 10 true 0.2 ESDTEST", Ok(r"[[3]]")),
        (r"@S5 10 true 0.2 ESDTEST", Ok(r"[[4,2]]")),
        (r"@C 10 true 0.2 ESDTEST", Ok(r"[[]]")),
        (r"@M 10 true 0.2 ESDTEST", Ok(r"[[4]]")),
        (r"@NAN 10 true 0.2 ESDTEST", Ok(r"[[4]]")),
        (r"@BIG 10 true 0.2 ESDTEST", Ok(r"[[200,201]]")),
        (r"@O 3 false 0.0 ESDTEST", Ok(r"[[]]")),
        (r"@N 3 false 0.0 ESDTEST", Ok(r"[[]]")),
        (r"@S3 3 false 0.0 ESDTEST", Ok(r"[[]]")),
        (r"@S5 3 false 0.0 ESDTEST", Ok(r"[[]]")),
        (r"@C 3 false 0.0 ESDTEST", Ok(r"[[]]")),
        (r"@M 3 false 0.0 ESDTEST", Ok(r"[[]]")),
        (r"@NAN 3 false 0.0 ESDTEST", Ok(r"[[]]")),
        (r"@BIG 3 false 0.0 ESDTEST", Ok(r"[[]]")),
        (
            r"@O 3 false NaN ESDTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (
            r"@N 3 false NaN ESDTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (
            r"@S3 3 false NaN ESDTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [-1, 1], values: [NaN, NaN]",
            ),
        ),
        (
            r"@S5 3 false NaN ESDTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (r"@C 3 false NaN ESDTEST", Ok(r"[[]]")),
        (
            r"@M 3 false NaN ESDTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (
            r"@NAN 3 false NaN ESDTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (
            r"@BIG 3 false NaN ESDTEST",
            Err(
                r"function values at endpoints do not have different signs, endpoints: [NaN, NaN], values: [NaN, NaN]",
            ),
        ),
        (r"@O 3 false 30.0 ESDTEST", Ok(r"[[18,6,31]]")),
        (r"@N 3 false 30.0 ESDTEST", Ok(r"[[17,10,9]]")),
        (r"@S3 3 false 30.0 ESDTEST", Err(r"-4 out of [0, 1] range")),
        (r"@S5 3 false 30.0 ESDTEST", Err(r"-2 out of [0, 1] range")),
        (r"@C 3 false 30.0 ESDTEST", Ok(r"[[]]")),
        (r"@M 3 false 30.0 ESDTEST", Err(r"-2 out of [0, 1] range")),
        (r"@NAN 3 false 30.0 ESDTEST", Err(r"-2 out of [0, 1] range")),
        (r"@BIG 3 false 30.0 ESDTEST", Ok(r"[[200,201,118]]")),
        (
            r"@O 3 false -1.0 ESDTEST",
            Err(r"1.012 out of [0, 1] range"),
        ),
        (
            r"@N 3 false -1.0 ESDTEST",
            Err(r"1.012 out of [0, 1] range"),
        ),
        (
            r"@S3 3 false -1.0 ESDTEST",
            Err(r"1.167 out of [0, 1] range"),
        ),
        (r"@S5 3 false -1.0 ESDTEST", Err(r"1.1 out of [0, 1] range")),
        (r"@C 3 false -1.0 ESDTEST", Ok(r"[[]]")),
        (r"@M 3 false -1.0 ESDTEST", Err(r"1.1 out of [0, 1] range")),
        (
            r"@NAN 3 false -1.0 ESDTEST",
            Err(r"1.1 out of [0, 1] range"),
        ),
        (
            r"@BIG 3 false -1.0 ESDTEST",
            Err(r"1.004 out of [0, 1] range"),
        ),
        (
            r"@O 50 false 0.05 ESDTEST",
            Ok(
                r"[[18,6,31,17,10,9,35,33,23,34,22,20,21,36,24,11,8,32,19,7,12,25,1,38,5,3,29,40,27,28,26,37,13,4,39,30,2,14]]",
            ),
        ),
        (
            r"@N 50 false 0.05 ESDTEST",
            Ok(
                r"[[17,10,9,35,33,3,23,34,22,20,21,36,24,11,8,32,19,31,7,12,25,1,6,38,5,18,29,40,27,28,26,37,13,4,39,30,2,14]]",
            ),
        ),
        (r"@S3 50 false 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@S5 50 false 0.05 ESDTEST", Ok(r"[[4]]")),
        (r"@C 50 false 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@M 50 false 0.05 ESDTEST", Ok(r"[[4]]")),
        (r"@NAN 50 false 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@BIG 50 false 0.05 ESDTEST", Ok(r"[[200,201]]")),
        (
            r"@O 38 true 0.05 ESDTEST",
            Ok(
                r"[[18,6,31,10,9,17,35,33,23,34,22,20,21,36,24,11,8,32,19,7,12,25,1,38,5,29,3,40,26,37,13,27,4,28,39,2,30,14]]",
            ),
        ),
        (
            r"@N 38 true 0.05 ESDTEST",
            Ok(
                r"[[10,9,17,35,33,3,23,34,22,20,21,36,24,11,8,32,19,31,7,12,25,1,6,38,5,18,29,40,26,37,13,27,4,28,39,2,30,14]]",
            ),
        ),
        (r"@S3 38 true 0.05 ESDTEST", Ok(r"[[3]]")),
        (r"@S5 38 true 0.05 ESDTEST", Ok(r"[[4,2]]")),
        (r"@C 38 true 0.05 ESDTEST", Ok(r"[[]]")),
        (r"@M 38 true 0.05 ESDTEST", Ok(r"[[4]]")),
        (r"@NAN 38 true 0.05 ESDTEST", Ok(r"[[4]]")),
        (r"@BIG 38 true 0.05 ESDTEST", Ok(r"[[200,201]]")),
        (r"@L 2 false ESDTEST", Err(r"GTS must be of type Double.")),
        (
            r"@O false 0.05 ESDTEST",
            Err(
                r"ESDTEST expects an upper bound of the number of outliers (a LONG) below the flag.",
            ),
        ),
        (
            r"@O 2.0 false ESDTEST",
            Err(
                r"ESDTEST expects an upper bound of the number of outliers (a LONG) below the flag.",
            ),
        ),
        (
            r"@O 2 0.05 ESDTEST",
            Err(
                r"ESDTEST expects a flag (a BOOLEAN) that indicates wether to use modified z-score below the SIGNIFICANCE level.",
            ),
        ),
        (
            r"@O 2 'x' ESDTEST",
            Err(
                r"ESDTEST expects a significance level (a DOUBLE)  or a flag (a BOOLEAN) on top of the stack.",
            ),
        ),
        (r"ESDTEST", Err(r"Empty stack.")),
        (r"true ESDTEST", Err(r"Empty stack.")),
        (r"1.0 ZSCORETEST", Err(r"Empty stack.")),
        (r"[ @O @N ] 3 true ESDTEST", Ok(r"[[[18,6,31],[]]]")),
        (r"@O 4294967298 false ESDTEST", Ok(r"[[18,6]]")),
    ]);
}

#[test]
fn zscore_of_nothing() {
    // A `DOUBLE` series without values (not reachable from WarpScript).
    let mut gts = crate::gts::GeoTimeSerie::new();
    gts.values = crate::gts::Values::Double(Vec::new());
    let err = super::outliers::zscore(&gts, true, 1.0).expect_err("zscore should fail");
    assert_eq!(err.to_string(), "Index 0 out of bounds for length 0");
    assert_eq!(
        super::outliers::zscore(&gts, false, 1.0).expect("zscore should succeed"),
        Vec::<i64>::new()
    );
}
