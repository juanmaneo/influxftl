// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Oracle-checked tests of `PARSE`.
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
fn parse() {
    check(&[
        (
            r"'1// a{} 42' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,42]]}]]"#),
        ),
        (
            r"'1// a{x=1,y=2} 42%0A2// a{x=1,y=2} 43%0A=3// 44' PARSE",
            Ok(r#"[[{"c":"a","l":{"x":"1","y":"2"},"a":{},"la":0,"v":[[1,42],[2,43],[3,44]]}]]"#),
        ),
        (
            r"'1/48.0:2.0/100 a{} %27hello%2520world%27%0A=2/10.0:20.0/ T' PARSE",
            Ok(
                r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,47.99999998882413,1.9999999646097422,100,"hello world"],[2,9.999999990686774,19.99999998137355,"T"]]}]]"#,
            ),
        ),
        (
            r"'1// a{} 1.5%0A2// b{} 2%0A3// a{} 3' PARSE",
            Ok(
                r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,1.5]]},{"c":"b","l":{},"a":{},"la":0,"v":[[2,2]]},{"c":"a","l":{},"a":{},"la":0,"v":[[3,3]]}]]"#,
            ),
        ),
        (
            r"'1// a{l=v}{at=x} 1%0A2// a{l=v}{at=y} 2' PARSE",
            Ok(r#"[[{"c":"a","l":{"l":"v"},"a":{"at":"y"},"la":0,"v":[[1,1],[2,2]]}]]"#),
        ),
        (
            r"'# comment%0A%0A  1// a{} 1  %0A' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,1]]}]]"#),
        ),
        (
            r"'1// a{} 1%0D%0A2// a{} 2%0D3// a{} 3' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,1],[2,2],[3,3]]}]]"#),
        ),
        (
            r"'1/123456/ a{} 1' PARSE",
            Ok(
                r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,-89.9999912828207,-179.99996714293957,1]]}]]"#,
            ),
        ),
        (
            r"'1// %253D{} 1' PARSE",
            Ok(r#"[[{"c":"=","l":{},"a":{},"la":0,"v":[[1,1]]}]]"#),
        ),
        (
            r"'1// a{} [ 1 2 ]' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,"x\u0004,\u0002.\u0002\u0000"]]}]]"#),
        ),
        (
            r"'1// a{} %27x%27%0A2// a{} 3' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,"x"],[2,"3"]]}]]"#),
        ),
        (
            r"'1// a{} b64:AAA=' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,"\u0000\u0000"]]}]]"#),
        ),
        (
            r"'1// a{} hex:00ff' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,"\u0000\u00FF"]]}]]"#),
        ),
        (
            r"'1//   a{} 1' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,1]]}]]"#),
        ),
        (
            r"'1// a{} HH:48.0:2.0' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,-2296496678784176095]]}]]"#),
        ),
        (
            r"'1// a{} 1%0A=2// 2%0A=3/1:1/ 3%0A=4//5 4' PARSE",
            Ok(
                r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,1],[2,2],[3,0.9999999823048711,0.9999999403953552,3],[4,5,4]]}]]"#,
            ),
        ),
        (
            r"'-5// a{} 1' PARSE",
            Ok(r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[-5,1]]}]]"#),
        ),
        (
            r"'=1// 42' PARSE",
            Err(r"PARSE could not parse at index 0 in '=1// 42' (Invalid continuation.)"),
        ),
        (
            r"'1 a{} 42' PARSE",
            Err(r"PARSE could not parse at index 0 in '1 a{} 42' (Missing timestamp separator.)"),
        ),
        (
            r"'x// a{} 42' PARSE",
            Err(r"PARSE could not parse at index 0 in 'x// a{} 42' (Invalid timestamp.)"),
        ),
        (
            r"'1/x/ a{} 42' PARSE",
            Err(r"PARSE could not parse at index 2 in '1/x/ a{} 42' (Invalid location: 'x'.)"),
        ),
        (
            r"'1/1:x/ a{} 42' PARSE",
            Err(r"PARSE could not parse at index 2 in '1/1:x/ a{} 42' (Invalid location: '1:x'.)"),
        ),
        (
            r"'1// a{}' PARSE",
            Err(r"PARSE could not parse at index 6 in '1// a{}' (Missing value.)"),
        ),
        (
            r"'1//x a{} 42' PARSE",
            Err(r"PARSE could not parse at index 3 in '1//x a{} 42' (Invalid elevation: 'x'.)"),
        ),
        (
            r"'1// ' PARSE",
            Err(r"PARSE could not parse at index 3 in '1//' (Missing GTS name, labels and value.)"),
        ),
        (
            r"'1// a{x=1 42' PARSE",
            Err(
                r"PARSE could not parse at index 11 in '1// a{x=1 42' (Missing end of labels '}'.)",
            ),
        ),
        (
            r"'1// a{x~1} 42' PARSE",
            Err(
                r"PARSE could not parse at index 6 in '1// a{x~1} 42' (Invalid label definition. (~1))",
            ),
        ),
        (
            r"'1// a{x} 42' PARSE",
            Err(
                r"PARSE could not parse at index 6 in '1// a{x} 42' (Invalid label definition. (x))",
            ),
        ),
        (
            r"'1// a{}{y=1 42' PARSE",
            Err(
                r"PARSE could not parse at index 13 in '1// a{}{y=1 42' (Missing end of attributes '}'.)",
            ),
        ),
        (
            r"'1// a{}{y~1} 42' PARSE",
            Err(
                r"PARSE could not parse at index 11 in '1// a{}{y~1} 42' (Invalid attribute definition. (~1))",
            ),
        ),
        (
            r"'1// a{} ' PARSE",
            Err(r"PARSE could not parse at index 6 in '1// a{}' (Missing value.)"),
        ),
        (
            r"'1// a{}{}' PARSE",
            Err(r"PARSE could not parse at index 8 in '1// a{}{}' (Missing value.)"),
        ),
        (
            r"'1// a{} %27x' PARSE",
            Err(
                r#"PARSE could not parse at index 8 in '1// a{} 'x' (Cannot parse value. (For input string: "'x"))"#,
            ),
        ),
        (
            r"'1// a{} [ 1' PARSE",
            Err(r"PARSE could not parse at index 8 in '1// a{} [ 1' (Missing closing bracket.)"),
        ),
        (
            r"'1// a{} %25zz' PARSE",
            Err(
                r#"PARSE could not parse at index 8 in '1// a{} %zz' (Cannot parse value. (For input string: "%zz"))"#,
            ),
        ),
        (
            r"'1// %25zz{} 1' PARSE",
            Err(
                r#"URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: "z" = 122"#,
            ),
        ),
        (
            r"'1// a{%25zz=1} 1' PARSE",
            Err(
                r#"URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: "z" = 122"#,
            ),
        ),
        (
            r"'1// 42' PARSE",
            Err(
                r"PARSE could not parse at index 4 in '1// 42' (Missing or invalid GTS name and labels.)",
            ),
        ),
        (
            r"'=1/1/ 4' PARSE",
            Err(r"PARSE could not parse at index 0 in '=1/1/ 4' (Invalid continuation.)"),
        ),
        (
            r"'1// a{} 1%0A=2// ' PARSE",
            Err(r"PARSE could not parse at index 4 in '=2//' (Missing value.)"),
        ),
        (
            r"'1// a{} 1%0A=2//x 3' PARSE",
            Err(r"PARSE could not parse at index 4 in '=2//x 3' (Invalid elevation: 'x'.)"),
        ),
        (
            r"'T// a{} 1' PARSE",
            Err(r"PARSE could not parse at index 0 in 'T// a{} 1' (Invalid timestamp.)"),
        ),
        (
            r"'1// a{} 1%0Ax' PARSE",
            Err(r"PARSE could not parse at index 0 in 'x' (Missing timestamp separator.)"),
        ),
        (
            r"'1/a{} 1' PARSE",
            Err(
                r"PARSE could not parse at index 2 in '1/a{} 1' (Missing location/elevation separator.)",
            ),
        ),
        (
            r"'1/x:1/ a{} 42' PARSE",
            Err(r"PARSE could not parse at index 2 in '1/x:1/ a{} 42' (Invalid location: 'x:1'.)"),
        ),
        (
            r"1 PARSE",
            Err(r"PARSE expects a String on top of the stack."),
        ),
        (r"PARSE", Err(r"Empty stack.")),
        (r"'' PARSE", Ok(r"[[]]")),
        (r"'%0A%0A' PARSE", Ok(r"[[]]")),
        (
            r"'T5// a{} 1' PARSE 0 GET TICKLIST 0 GET NOW - ABS 10000000 <",
            Ok(r"[true]"),
        ),
        (
            r"'// a{} 1' PARSE 0 GET TICKLIST 0 GET NOW - ABS 10000000 <",
            Ok(r"[true]"),
        ),
    ]);
}

#[test]
fn untrimmed_line_without_value() {
    // `PARSE` trims its lines; `GTSHelper.parse` itself checks for a missing value.
    let Err(crate::gtsparse::LineError::Parse(error)) =
        crate::gtsparse::parse_line(None, "1// ", 0)
    else {
        panic!("a line without value must fail");
    };
    assert_eq!((error.message.as_str(), error.offset), ("Missing value", 4));
}
