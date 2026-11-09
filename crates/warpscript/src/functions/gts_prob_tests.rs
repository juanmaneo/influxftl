// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `PROB`, `CPROB` and `PROBABILITY`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const S: &str = r"NEWGTS 's' RENAME { 'k' 'v' } RELABEL 1 0 0 NaN 'a' ADDVALUE 2 NaN NaN 7 'b' ADDVALUE 3 NaN NaN NaN 'a' ADDVALUE 4 NaN NaN NaN 'c' ADDVALUE 5 NaN NaN NaN 'a' ADDVALUE";
const L: &str = r"NEWGTS 'l' RENAME 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 1 ADDVALUE 4 NaN NaN NaN 1 ADDVALUE 5 NaN NaN NaN 3 ADDVALUE";
const D: &str = r"NEWGTS 'd' RENAME 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN NaN ADDVALUE 4 NaN NaN NaN -0.0 ADDVALUE 5 NaN NaN NaN 0.0 ADDVALUE 6 NaN NaN NaN 1.0 ADDVALUE";
const B: &str = r"NEWGTS 'b' RENAME 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE 3 NaN NaN NaN true ADDVALUE";
const BK: &str = r"[ NEWGTS 'k' RENAME 10 NaN NaN NaN 1 ADDVALUE 20 NaN NaN NaN 2 ADDVALUE 50 NaN NaN NaN 1 ADDVALUE bucketizer.last 50 10 0 ] BUCKETIZE 0 GET";
const C: &str = r"NEWGTS 'c' RENAME 1 NaN NaN NaN 'a,b' ADDVALUE 2 NaN NaN NaN 'a,c' ADDVALUE 3 NaN NaN NaN 'd,b' ADDVALUE 4 NaN NaN NaN 'a,b' ADDVALUE 5 NaN NaN NaN 'd,e' ADDVALUE 6 NaN NaN NaN 'a,b' ADDVALUE 7 NaN NaN NaN 'x,y,z' ADDVALUE 8 NaN NaN NaN 'x,y,z' ADDVALUE 9 NaN NaN NaN 'x,q' ADDVALUE";
const C2: &str = r"NEWGTS 'c' RENAME 5 NaN NaN NaN 'g1|g2|e1' ADDVALUE 1 NaN NaN NaN 'g1|g2|e2' ADDVALUE 3 NaN NaN NaN 'g1|e1' ADDVALUE 2 NaN NaN NaN 'g1|g2|e1' ADDVALUE 4 NaN NaN NaN 'g3|e9' ADDVALUE";
const E: &str = r"NEWGTS 'e' RENAME";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@BK", BK)
        .replace("@C2", C2)
        .replace("@S", S)
        .replace("@L", L)
        .replace("@D", D)
        .replace("@B", B)
        .replace("@C", C)
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
fn prob() {
    check(&[
        (
            r"@S PROB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,0.6],[2,7,0.2],[3,0.6],[4,0.2],[5,0.6]]}]"#,
            ),
        ),
        (
            r"@L PROB",
            Ok(r#"[{"c":"l","l":{},"a":{},"la":0,"v":[[1,0.6],[2,0.2],[3,0.6],[4,0.6],[5,0.2]]}]"#),
        ),
        (
            r"@D PROB",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[1,0.3333333333333333],[2,0.3333333333333333],[3,0.3333333333333333],[4,0.16666666666666666],[5,0.16666666666666666],[6,0.3333333333333333]]}]"#,
            ),
        ),
        (
            r"@B PROB",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[1,0.6666666666666666],[2,0.3333333333333333],[3,0.6666666666666666]]}]"#,
            ),
        ),
        (
            r"@BK PROB",
            Ok(
                r#"[{"c":"k","l":{},"a":{},"la":0,"v":[[50,0.6666666666666666],[20,0.3333333333333333],[10,0.6666666666666666]]}]"#,
            ),
        ),
        (r"@E PROB", Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#)),
        (
            r"[ @S @L ] PROB",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,0.6],[2,7,0.2],[3,0.6],[4,0.2],[5,0.6]]},{"c":"l","l":{},"a":{},"la":0,"v":[[1,0.6],[2,0.2],[3,0.6],[4,0.6],[5,0.2]]}]]"#,
            ),
        ),
        (
            r"[ @S 1 ] PROB",
            Err(r"PROB can only operate on Geo Time Series instances."),
        ),
        (
            r"1 PROB",
            Err(r"PROB can only operate on Geo Time Series instances."),
        ),
        (r"PROB", Err(r"Empty stack.")),
        (
            r"@S { 'x' 'y' } SETATTRIBUTES PROB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{"x":"y"},"la":0,"v":[[1,0.0,0.0,0.6],[2,7,0.2],[3,0.6],[4,0.2],[5,0.6]]}]"#,
            ),
        ),
        (r"@BK PROB DUP BUCKETSPAN SWAP LASTBUCKET", Ok(r"[50,10]")),
    ]);
}

#[test]
fn cprob() {
    check(&[
        (
            r"@S NULL CPROB",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,0.6],[2,7,0.2],[3,0.6],[4,0.2],[5,0.6]]}]"#,
            ),
        ),
        (
            r"@L NULL CPROB",
            Ok(r#"[{"c":"l","l":{},"a":{},"la":0,"v":[[1,0.6],[2,0.2],[3,0.6],[4,0.6],[5,0.2]]}]"#),
        ),
        (
            r"@D NULL CPROB",
            Ok(
                r#"[{"c":"d","l":{},"a":{},"la":0,"v":[[1,0.3333333333333333],[2,0.3333333333333333],[3,0.3333333333333333],[4,0.16666666666666666],[5,0.16666666666666666],[6,0.3333333333333333]]}]"#,
            ),
        ),
        (
            r"@B NULL CPROB",
            Ok(
                r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[1,0.6666666666666666],[2,0.3333333333333333],[3,0.6666666666666666]]}]"#,
            ),
        ),
        (
            r"@BK NULL CPROB",
            Ok(
                r#"[{"c":"k","l":{},"a":{},"la":0,"v":[[50,0.6666666666666666],[20,0.3333333333333333],[10,0.6666666666666666]]}]"#,
            ),
        ),
        (
            r"@E NULL CPROB",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@C ',' CPROB",
            Ok(
                r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[1,0.75],[4,0.75],[6,0.75],[2,0.25],[3,0.5],[5,0.5],[9,1.0],[7,1.0],[8,1.0]]}]"#,
            ),
        ),
        (
            r"@C2 '|' CPROB",
            Ok(
                r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[3,1.0],[2,0.6666666666666666],[5,0.6666666666666666],[1,0.3333333333333333],[4,1.0]]}]"#,
            ),
        ),
        (
            r"@C2 '|' CPROB DROP @C2",
            Ok(
                r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[5,"g1|g2|e1"],[1,"g1|g2|e2"],[3,"g1|e1"],[2,"g1|g2|e1"],[4,"g3|e9"]]}]"#,
            ),
        ),
        (
            r"@C '' CPROB",
            Ok(
                r#"[{"c":"c","l":{},"a":{},"la":0,"v":[[1,1.0],[4,1.0],[6,1.0],[2,1.0],[3,1.0],[5,1.0],[9,1.0],[7,1.0],[8,1.0]]}]"#,
            ),
        ),
        (
            r"@C 'b' CPROB",
            Err(r"Separator not found, unable to isolate given events."),
        ),
        (
            r"@C ';' CPROB",
            Err(r"Separator not found, unable to isolate given events."),
        ),
        (
            r"@L ',' CPROB",
            Err(r"Can only compute conditional probabilities for String Geo Time Series."),
        ),
        (
            r"@E ',' CPROB",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (r"@C ',' CPROB DUP BUCKETSPAN SWAP LASTBUCKET", Ok(r"[0,0]")),
        (
            r"NEWGTS 1 NaN NaN NaN 'a, b' ADDVALUE ',' CPROB",
            Err(r"NullPointerException"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 'a,b ' ADDVALUE ',' CPROB",
            Err(r"NullPointerException"),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 'a,,b' ADDVALUE 2 NaN NaN NaN 'a,,c' ADDVALUE ',,' CPROB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[1,0.5],[2,0.5]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 'é,ü' ADDVALUE 2 NaN NaN NaN 'é,ä' ADDVALUE ',' CPROB",
            Ok(r#"[{"c":"","l":{},"a":{},"la":0,"v":[[2,0.5],[1,0.5]]}]"#),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 'a,b' ADDVALUE 2 NaN NaN NaN 'ab' ADDVALUE ',' CPROB",
            Err(r"Separator not found, unable to isolate given events."),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 'ab' ADDVALUE 2 NaN NaN NaN 'b,a' ADDVALUE ',' CPROB",
            Err(r"Separator not found, unable to isolate given events."),
        ),
        (
            r"[ @C @C2 ] ',' CPROB",
            Err(r"Separator not found, unable to isolate given events."),
        ),
        (
            r"[ @C 1 ] ',' CPROB",
            Err(r"CPROB can only operate on Geo Time Series instances."),
        ),
        (
            r"1 ',' CPROB",
            Err(r"CPROB can only operate on Geo Time Series instances."),
        ),
        (
            r"@C 1 CPROB",
            Err(r"CPROB expects null or a String separator on top of the stack."),
        ),
        (r"CPROB", Err(r"Empty stack.")),
        (
            r"@C CPROB",
            Err(r"CPROB expects null or a String separator on top of the stack."),
        ),
    ]);
}

#[test]
fn probability() {
    check(&[
        (
            r"{ 'a' 2 'b' 3 } PROBABILITY DUP TYPEOF SWAP 'a' SWAP EVAL",
            Ok(r#"[0.4,"FUNCTION"]"#),
        ),
        (r"{ 'a' 2 'b' 3 } PROBABILITY 'c' SWAP EVAL", Ok(r"[0.0]")),
        (
            r"{ 1 2 1.0 3 } PROBABILITY DUP 1 SWAP EVAL SWAP 1.0 SWAP EVAL",
            Ok(r"[0.6,0.4]"),
        ),
        (
            r"{ NULL 1 'a' 2 'b' 3 } PROBABILITY SNAPSHOT",
            Ok(r#"["{ NULL 0.16666666666666666 'a' 0.3333333333333333 'b' 0.5  } PROBABILITY "]"#),
        ),
        (
            r"{ NULL 1 'a' 2 'b' 3 } PROBABILITY NULL SWAP EVAL",
            Ok(r"[0.16666666666666666]"),
        ),
        (
            r"{ 'a' 2 } PROBABILITY ->JSON",
            Ok(r#"["\"{ 'a' 1.0  } PROBABILITY\""]"#),
        ),
        (r"{ } PROBABILITY SNAPSHOT", Ok(r#"["{  } PROBABILITY "]"#)),
        (r"{ } PROBABILITY 'a' SWAP EVAL", Ok(r"[0.0]")),
        (r"{ 'a' 0 'b' 0 } PROBABILITY 'a' SWAP EVAL", Ok(r"[NaN]")),
        (
            r"{ 'a' 2.5 'b' 2.5 } PROBABILITY 'b' SWAP EVAL",
            Ok(r"[0.5]"),
        ),
        (
            r"{ 'a' 'x' } PROBABILITY",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"{ 'a' 1 'b' NULL } PROBABILITY",
            Err(r"NullPointerException"),
        ),
        (
            r"{ 'a' 1 'b' true } PROBABILITY",
            Err(r"class java.lang.Boolean cannot be cast to class java.lang.Number"),
        ),
        (
            r"[ 1 ] PROBABILITY",
            Err(r"PROBABILITY expects a value histogram on top of the stack."),
        ),
        (r"PROBABILITY", Err(r"Empty stack.")),
        (r"{ 'a' 2 } PROBABILITY EVAL", Err(r"Empty stack.")),
        (r"@S VALUEHISTOGRAM PROBABILITY 'a' SWAP EVAL", Ok(r"[0.6]")),
        (
            r"@BK VALUEHISTOGRAM PROBABILITY NULL SWAP EVAL",
            Ok(r"[0.4]"),
        ),
        (
            r"{ 'a' 2 } PROBABILITY 'p' STORE <% 'a' @p %> 'm' STORE $m SNAPSHOT",
            Ok(r#"["<% 'a' 'p' RUN %> "]"#),
        ),
        (
            r"{ 'a' 1 } PROBABILITY <% %> SWAP 'f' STORE 'a' $f EVAL",
            Ok(r#"[1.0,"<% %>"]"#),
        ),
        (
            r"[ { 'a' 1 } PROBABILITY ] ->MACRO 'm' STORE 'a' $m EVAL",
            Ok(r"[1.0]"),
        ),
        (
            r"[ { 'a' 1 } PROBABILITY ] ->MACRO EVAL",
            Err(r"Empty stack."),
        ),
        (
            r"[ { 'a' 1 } PROBABILITY ] ->MACRO SNAPSHOT",
            Ok(r#"["<% { 'a' 1.0  } PROBABILITY EVAL %> "]"#),
        ),
    ]);
}
