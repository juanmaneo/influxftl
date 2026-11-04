// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Oracle-checked tests of `DEDUP` with a macro or a mapper (`GTSHelper.map` in its
//! deduplicating mode).
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const D: &str = r"NEWGTS 'd' RENAME { 'k' 'v' } RELABEL 3 NaN NaN NaN 'c' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE 1 NaN NaN NaN 'a2' ADDVALUE 3 NaN NaN NaN 'c2' ADDVALUE 3 NaN NaN NaN 'c3' ADDVALUE";
const N: &str = r"NEWGTS 'n' RENAME 3 NaN NaN NaN 3 ADDVALUE 1 1.0 2.0 NaN 1 ADDVALUE 2 NaN NaN 5 2 ADDVALUE 1 NaN NaN NaN 10 ADDVALUE 3 NaN NaN NaN 30 ADDVALUE 3 NaN NaN NaN 300 ADDVALUE";
const B: &str = r"[ NEWGTS 'b' RENAME 10 NaN NaN NaN 1 ADDVALUE 20 NaN NaN NaN 2 ADDVALUE 20 NaN NaN NaN 3 ADDVALUE 40 NaN NaN NaN 4 ADDVALUE bucketizer.last 40 10 0 ] BUCKETIZE 0 GET";
const E: &str = r"NEWGTS 'e' RENAME";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@D", D)
        .replace("@N", N)
        .replace("@B", B)
        .replace("@E", E)
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
fn dedup_with_mappers() {
    check(&[
        (
            r"@N mapper.sum DEDUP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,11],[2,5,2],[3,333]]}]"#,
            ),
        ),
        (
            r"@N mapper.max DEDUP",
            Ok(r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,10],[2,5,2],[3,300]]}]"#),
        ),
        (
            r"@N mapper.first DEDUP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,1],[2,5,2],[3,300]]}]"#,
            ),
        ),
        (
            r"@N mapper.last DEDUP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,1],[2,5,2],[3,300]]}]"#,
            ),
        ),
        (
            r"@N mapper.count DEDUP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,2],[2,5,1],[3,3]]}]"#,
            ),
        ),
        (
            r"@N mapper.mean DEDUP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,5.5],[2,5,2.0],[3,111.0]]}]"#,
            ),
        ),
        (
            r"@N mapper.tick DEDUP",
            Err(r"mapper.tick can only be applied to a single value."),
        ),
        (
            r"@N 2 mapper.add DEDUP",
            Err(r"mapper.add can only be applied to a single value."),
        ),
        (
            r"@D mapper.first DEDUP",
            Ok(r#"[{"c":"d","l":{"k":"v"},"a":{},"la":0,"v":[[1,"a"],[2,"b"],[3,"c3"]]}]"#),
        ),
        (
            r"@D mapper.last DEDUP",
            Ok(r#"[{"c":"d","l":{"k":"v"},"a":{},"la":0,"v":[[1,"a"],[2,"b"],[3,"c3"]]}]"#),
        ),
        (
            r"@D '-' mapper.join DEDUP",
            Ok(r#"[{"c":"d","l":{"k":"v"},"a":{},"la":0,"v":[[1,"a-a2"],[2,"b"],[3,"c3-c2-c"]]}]"#),
        ),
        (
            r"@D mapper.count DEDUP",
            Ok(r#"[{"c":"d","l":{"k":"v"},"a":{},"la":0,"v":[[1,2],[2,1],[3,3]]}]"#),
        ),
        (
            r"@N mapper.delta DEDUP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,0],[2,5,0],[3,0]]}]"#,
            ),
        ),
        (
            r"@N mapper.product DEDUP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,10],[2,5,2],[3,27000]]}]"#,
            ),
        ),
        (
            r"@B mapper.sum DEDUP",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,1],[20,2],[40,4]]}]"#),
        ),
        (
            r"@E mapper.sum DEDUP",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"[ @N [ @D ] ] mapper.count DEDUP",
            Ok(
                r#"[[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,2],[2,5,1],[3,3]]},[{"c":"d","l":{"k":"v"},"a":{},"la":0,"v":[[1,2],[2,1],[3,3]]}]]]"#,
            ),
        ),
        (
            r"@N bucketizer.sum DEDUP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,11],[2,5,2],[3,333]]}]"#,
            ),
        ),
        (
            r"@N reducer.sum DEDUP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,11],[2,5,2],[3,333]]}]"#,
            ),
        ),
        (
            r"@N 'x' DEDUP",
            Err(
                r"DEDUP expects a Geo Time Series instance or a list thereof under an optional macro or mapper.",
            ),
        ),
        (
            r"@N mapper.sum DEDUP DROP @N",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[3,3],[1,0.9999999823048711,1.9999999646097422,1],[2,5,2],[1,10],[3,30],[3,300]]}]"#,
            ),
        ),
        (
            r"@N DUP mapper.sum DEDUP DROP",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,1],[1,10],[2,5,2],[3,300],[3,30],[3,3]]}]"#,
            ),
        ),
        (r"mapper.sum DEDUP", Err(r"Empty stack.")),
        (
            r"1 mapper.sum DEDUP",
            Err(
                r"DEDUP expects a Geo Time Series instance or a list thereof under an optional macro or mapper.",
            ),
        ),
        (
            r"[ @N 1 ] mapper.sum DEDUP",
            Err(
                r"DEDUP expects a Geo Time Series instance or a list thereof under an optional macro or mapper.",
            ),
        ),
        (
            r"@N <% %> MACROMAPPER DEDUP",
            Err(r"Expected a list of 1 to 5 elements, got 8"),
        ),
        (
            r"@N <% DUP 0 GET SWAP 7 GET SIZE [ SWAP NaN NaN NaN SWAP ] %> MACROMAPPER DEDUP",
            Ok(r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,NaN],[2,NaN],[3,NaN]]},3,2,1]"#),
        ),
    ]);
}

#[test]
fn dedup_with_macros() {
    check(&[
        (
            r"@N <% DUP LASTTICK SWAP VALUES [ SWAP NaN NaN NaN SWAP ] %> DEDUP",
            Err(r"class java.util.ArrayList cannot be cast to class java.lang.Number"),
        ),
        (
            r"@N <% DUP LASTTICK SWAP SIZE [ SWAP NaN NaN NaN SWAP ] %> DEDUP",
            Ok(r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,NaN],[2,NaN],[3,NaN]]},3,2,1]"#),
        ),
        (
            r"@D <% DUP LASTTICK SWAP VALUES 0 GET [ SWAP NaN NaN NaN SWAP ] %> DEDUP",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"@N <% DROP { 'x' [ 0 1 ] 'a' [ 0 2 ] } %> DEDUP",
            Ok(
                r#"[[{"c":"a","l":{},"a":{},"la":0,"v":[[1,2],[2,2],[3,2]]},{"c":"x","l":{},"a":{},"la":0,"v":[[1,1],[2,1],[3,1]]}]]"#,
            ),
        ),
        (
            r"@N <% DUP LASTTICK SWAP SIZE 0 == <% NULL %> <% 1 %> IFTE [ SWAP NaN NaN NaN SWAP ] %> DEDUP",
            Ok(r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,NaN],[2,NaN],[3,NaN]]},3,2,1]"#),
        ),
        (
            r"@N <% LASTTICK NaN NaN NaN 4 %> DEDUP",
            Ok(r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[1,4],[2,4],[3,4]]}]"#),
        ),
        (r"@N <% DROP 'boom' MSGFAIL %> DEDUP", Err(r"MSGFAIL boom")),
        (r"@N <% DROP %> DEDUP", Err(r"Empty stack.")),
        (
            r"@B <% SIZE [ SWAP ] %> DEDUP",
            Ok(r#"[{"c":"b","l":{},"a":{},"la":0,"v":[[10,1],[20,1],[30,0],[40,1]]}]"#),
        ),
        (
            r"@E <% SIZE [ SWAP ] %> DEDUP",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"[ @N @D ] <% SIZE [ SWAP ] %> DEDUP",
            Ok(
                r#"[[{"c":"n","l":{},"a":{},"la":0,"v":[[1,2],[2,1],[3,3]]},{"c":"d","l":{"k":"v"},"a":{},"la":0,"v":[[1,2],[2,1],[3,3]]}]]"#,
            ),
        ),
        (r"<% %> DEDUP", Err(r"Empty stack.")),
        (r"DEDUP", Err(r"Empty stack.")),
    ]);
}
