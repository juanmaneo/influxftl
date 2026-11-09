// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `SMARTPARSE`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const S: &str =
    r"NEWGTS 'in' RENAME 10 NaN NaN NaN 'a=1 b=2.5' ADDVALUE 20 NaN NaN NaN 'a=3' ADDVALUE";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template.replace("@S", S)
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
fn smartparse() {
    check(&[
        (
            r"'a=1 b=2.5 a=3' '(?<Lname>[a-z]+)=(?<VDvalue>[0-9.]+)' SMARTPARSE",
            Ok(
                r#"[[{"c":"value","l":{"name":"b"},"a":{},"la":0,"v":[[0,2.5]]},{"c":"value","l":{"name":"a"},"a":{},"la":0,"v":[[0,1.0],[0,3.0]]}]]"#,
            ),
        ),
        (
            r"@S '(?<Lname>[a-z]+)=(?<VDvalue>[0-9.]+)' SMARTPARSE",
            Ok(
                r#"[[{"c":"value","l":{"name":"b"},"a":{},"la":0,"v":[[10,2.5]]},{"c":"value","l":{"name":"a"},"a":{},"la":0,"v":[[10,1.0],[20,3.0]]}]]"#,
            ),
        ),
        (
            r"[ @S @S ] '(?<Lname>[a-z]+)=(?<VDvalue>[0-9.]+)' SMARTPARSE",
            Ok(
                r#"[[[{"c":"value","l":{"name":"b"},"a":{},"la":0,"v":[[10,2.5]]},{"c":"value","l":{"name":"a"},"a":{},"la":0,"v":[[10,1.0],[20,3.0]]}],[{"c":"value","l":{"name":"b"},"a":{},"la":0,"v":[[10,2.5]]},{"c":"value","l":{"name":"a"},"a":{},"la":0,"v":[[10,1.0],[20,3.0]]}]]]"#,
            ),
        ),
        (
            r"'x=1' '(?<VLv>[a-z])=(?<VSs>[0-9])' SMARTPARSE",
            Err(r#"For input string: "x""#),
        ),
        (
            r"'x=1' '(?<VLv>[0-9])' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,1]]}]]"#),
        ),
        (
            r"'x=true y=TRUE z=no' '(?<Lk>[a-z])=(?<VBflag>[A-Za-z]+)' SMARTPARSE",
            Ok(
                r#"[[{"c":"flag","l":{"k":"x"},"a":{},"la":0,"v":[[0,true]]},{"c":"flag","l":{"k":"y"},"a":{},"la":0,"v":[[0,true]]},{"c":"flag","l":{"k":"z"},"a":{},"la":0,"v":[[0,false]]}]]"#,
            ),
        ),
        (
            r"'1000 5' '(?<Ts>[0-9]+) (?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[1000000000,5]]}]]"#),
        ),
        (
            r"'1000 5' '(?<Tms>[0-9]+) (?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[1000000,5]]}]]"#),
        ),
        (
            r"'1000 5' '(?<Tus>[0-9]+) (?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[1000,5]]}]]"#),
        ),
        (
            r"'1000 5' '(?<Tns>[0-9]+) (?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[1,5]]}]]"#),
        ),
        (
            r"'1000 5' '(?<Tx>[0-9]+) (?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,5]]}]]"#),
        ),
        (
            r"'1.5 5' '(?<Ts>[0-9.]+) (?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[1500000,5]]}]]"#),
        ),
        (
            r"'1000 5 7' '(?<Ts>[0-9]+ )?(?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[1000000000,5],[1000000000,7]]}]]"#),
        ),
        (
            r"'48.5 -4.25 12.6 5' '(?<lat>[0-9.-]+) (?<lon>[0-9.-]+) (?<elev>[0-9.]+) (?<VLv>[0-9]+)' SMARTPARSE",
            Ok(
                r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,48.49999995902181,-4.250000081956387,13,5]]}]]"#,
            ),
        ),
        (
            r"'48.5 5' '(?<lat>[0-9.-]+) (?<lon>[a-z]+)? ?(?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,5]]}]]"#),
        ),
        (
            r"'12.5 5' '(?<elev>[0-9.]+) (?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,13,5]]}]]"#),
        ),
        (
            r"'x 5' '(?<elev>[a-z]+)? ?(?<VLv>[0-9]+)' SMARTPARSE",
            Err(r#"For input string: "x""#),
        ),
        (
            r"'a 5' '(?<lat>[a-z]) (?<lon>[0-9])' SMARTPARSE",
            Err(r#"For input string: "a""#),
        ),
        (
            r"'a 5' '(?<lat>[a-z]) (?<lon>[0-9]) (?<VLv>[0-9])' SMARTPARSE",
            Ok(r"[[]]"),
        ),
        (
            r"'x 5' '(?<lat>[0-9]) (?<lon>[0-9]) (?<VLv>[0-9])|(?<elev>[a-z]) (?<VLw>[0-9])' SMARTPARSE",
            Err(r#"For input string: "x""#),
        ),
        (
            r"'ab' '(?<VLv>[a-z]+)' SMARTPARSE",
            Err(r#"For input string: "ab""#),
        ),
        (
            r"'ab' '(?<VDv>[a-z]+)' SMARTPARSE",
            Err(r#"For input string: "ab""#),
        ),
        (
            r"'ab' '(?<Ts>[a-z]+)(?<VLv>.)?' SMARTPARSE",
            Err(r#"For input string: "ab""#),
        ),
        (
            r"'ab' '(?<V>[a-z]+)' SMARTPARSE",
            Err(r"Index 1 out of bounds for length 1"),
        ),
        (r"'ab' '(?<Vx>[a-z]+)' SMARTPARSE", Ok(r"[[]]")),
        (
            r"'ab' '(?<other>[a-z]+)(?<VSv>.)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,"b"]]}]]"#),
        ),
        (
            r"'ab' '(?<=a)(?<VSv>b)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,"b"]]}]]"#),
        ),
        (
            r"'ab' '(?<!x)(?<VSv>b)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,"b"]]}]]"#),
        ),
        (
            r"'ab' '(' SMARTPARSE",
            Err(r"Parsing error at position 1: Opening parenthesis without closing parenthesis"),
        ),
        (
            r"'ab' 1 SMARTPARSE",
            Err(r"SMARTPARSE expects a STRING regexp."),
        ),
        (
            r"1 'x' SMARTPARSE",
            Err(r"SMARTPARSE can only operate on Geo Time Series instances."),
        ),
        (r"SMARTPARSE", Err(r"Invalid level.")),
        (r"'x' SMARTPARSE", Err(r"Invalid level.")),
        (
            r"[ 1 ] 'x' SMARTPARSE",
            Err(r"SMARTPARSE can only operate on Geo Time Series instances."),
        ),
        (
            r"[ @S 1 ] 'x' SMARTPARSE",
            Err(r"SMARTPARSE can only operate on Geo Time Series instances."),
        ),
        (
            r"'x=1 y=2 x=3' '(?<Lk>[a-z])=(?<VLv>[0-9])' SMARTPARSE",
            Ok(
                r#"[[{"c":"v","l":{"k":"x"},"a":{},"la":0,"v":[[0,1],[0,3]]},{"c":"v","l":{"k":"y"},"a":{},"la":0,"v":[[0,2]]}]]"#,
            ),
        ),
        (
            r"'a,b,1 a,c,2 b,b,3' '(?<La>[a-z]),(?<Lb>[a-z]),(?<VLv>[0-9])' SMARTPARSE",
            Ok(
                r#"[[{"c":"v","l":{"a":"a","b":"b"},"a":{},"la":0,"v":[[0,1]]},{"c":"v","l":{"a":"a","b":"c"},"a":{},"la":0,"v":[[0,2]]},{"c":"v","l":{"a":"b","b":"b"},"a":{},"la":0,"v":[[0,3]]}]]"#,
            ),
        ),
        (
            r"'1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18' '(?<Lk>[0-9]+)' SMARTPARSE DROP '1 2 3' '(?<VLv>[0-9]+)' SMARTPARSE",
            Ok(r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,1],[0,2],[0,3]]}]]"#),
        ),
        (
            r"'a1 b2 c3 d4 e5 f6 g7 h8 i9 j10 k11 l12 m13' '(?<VLv>[0-9]+)' SMARTPARSE",
            Ok(
                r#"[[{"c":"v","l":{},"a":{},"la":0,"v":[[0,1],[0,2],[0,3],[0,4],[0,5],[0,6],[0,7],[0,8],[0,9],[0,10],[0,11],[0,12],[0,13]]}]]"#,
            ),
        ),
        (
            r"'aa=1 bb=2 cc=3 dd=4 ee=5 ff=6 gg=7 hh=8 ii=9 jj=10 kk=11 ll=12 mm=13' '(?<VLname>[0-9]+)(?<Lx>.)?' SMARTPARSE",
            Ok(
                r#"[[{"c":"name","l":{},"a":{},"la":0,"v":[[0,13]]},{"c":"name","l":{"x":" "},"a":{},"la":0,"v":[[0,1],[0,2],[0,3],[0,4],[0,5],[0,6],[0,7],[0,8],[0,9],[0,10],[0,11],[0,12]]}]]"#,
            ),
        ),
        (
            r"'k=1 v=2' '(?<Lk>k)=(?<VLa>[0-9])|v=(?<VLb>[0-9])' SMARTPARSE",
            Ok(
                r#"[[{"c":"a","l":{"k":"k"},"a":{},"la":0,"v":[[0,1]]},{"c":"b","l":{},"a":{},"la":0,"v":[[0,2]]}]]"#,
            ),
        ),
    ]);
}
