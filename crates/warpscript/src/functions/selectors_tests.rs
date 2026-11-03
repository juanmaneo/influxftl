// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `PARSESELECTOR`, `TOSELECTOR`, `filter.byselector`, `filter.bymetadata` and
//! `filter.latencies`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const G1: &str = r"NEWGTS 'a' RENAME { 'x' '1' 'y' '2' } RELABEL { 'z' '3' } SETATTRIBUTES";
const G2: &str = r"NEWGTS 'ab' RENAME { 'x' '2' } RELABEL { 'y' '2' } SETATTRIBUTES";
const G3: &str = r"NEWGTS '' RENAME";
const G4: &str = r"NEWGTS 'a' RENAME { 'z' '9' } RELABEL";
const G5: &str = r"NEWGTS NEWGTS +";
const G6: &str = r"NEWGTS 'a b' RENAME { 'x' 'é' } RELABEL";
const ALL: &str = r"[ @G1 @G2 @G3 @G4 @G5 @G6 ]";
const NAMES: &str = r"<% DROP DUP NAME SWAP LABELS 2 ->LIST %> LMAP";
const U: &str = r"NEWGTS 'u' RENAME { 'k' 'v' } RELABEL 10 NaN NaN NaN 5 ADDVALUE 20 NaN NaN NaN 7 ADDVALUE 25 NaN NaN NaN 6 ADDVALUE 40 NaN NaN NaN 9 ADDVALUE";
const D1: &str = r"NEWGTS 'd' RENAME 15 NaN NaN NaN 5 ADDVALUE 30 NaN NaN NaN 7 ADDVALUE 18 NaN NaN NaN 5 ADDVALUE 41 NaN NaN NaN 9 ADDVALUE";
const D2: &str = r"NEWGTS 'e' RENAME 12 NaN NaN NaN 5 ADDVALUE 26 NaN NaN NaN 6 ADDVALUE 200 NaN NaN NaN 9 ADDVALUE 22 NaN NaN NaN 7.5 ADDVALUE";
const OPTS: &str = r"[ 'downlink.matches' 'downlinks.bitset' 'uplink.latency.min' 'uplink.latency.max' 'downlink.latency.min' 'downlink.latency.max' 'downlinks.totalmatches' 'downlinks.withmatches' 'x' 3 ]";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@NAMES", NAMES)
        .replace("@OPTS", OPTS)
        .replace("@ALL", ALL)
        .replace("@G1", G1)
        .replace("@G2", G2)
        .replace("@G3", G3)
        .replace("@G4", G4)
        .replace("@G5", G5)
        .replace("@G6", G6)
        .replace("@D1", D1)
        .replace("@D2", D2)
        .replace("@U", U)
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
fn parseselector() {
    check(&[
        (r"'a{x=1}' PARSESELECTOR", Ok(r#"[{"x":"=1"},"a"]"#)),
        (
            r"'a%20b{x=1,y~z.*, ,k%3D=v}' PARSESELECTOR",
            Ok(r#"[{"x":"=1","y":"~z.*","k":"="},"a b"]"#),
        ),
        (
            r"'a{x}' PARSESELECTOR",
            Err(r"PARSESELECTOR could not parse input. (java.text.ParseException: x (x))"),
        ),
        (
            r"'a' PARSESELECTOR",
            Err(r"PARSESELECTOR could not parse input. ( invalid syntax for selector.)"),
        ),
        (
            r"'{}' PARSESELECTOR",
            Err(r"PARSESELECTOR could not parse input. ( invalid syntax for selector.)"),
        ),
        (
            r"'a{x=1=2,y}' PARSESELECTOR",
            Err(r"PARSESELECTOR could not parse input. (java.text.ParseException: y (y))"),
        ),
        (
            r"'a{x=%25zz}' PARSESELECTOR",
            Err(
                r#"URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: "z" = 122"#,
            ),
        ),
        (
            r"'a%25zz{}' PARSESELECTOR",
            Err(
                r#"URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: "z" = 122"#,
            ),
        ),
        (r"'a{x=1}}' PARSESELECTOR", Ok(r#"[{"x":"=1}"},"a"]"#)),
        (
            r"'a{b{c}' PARSESELECTOR",
            Err(r"PARSESELECTOR could not parse input. (java.text.ParseException: b{c (b{c))"),
        ),
        (r"'a{}' PARSESELECTOR", Ok(r#"[{},"a"]"#)),
        (r"'~a.*{}' PARSESELECTOR", Ok(r#"[{},"~a.*"]"#)),
        (
            r"'a{x=1,x=2,y=3}' PARSESELECTOR",
            Ok(r#"[{"x":"=2","y":"=3"},"a"]"#),
        ),
        (r"'a{x~a=b}' PARSESELECTOR", Ok(r#"[{"x~a":"=b"},"a"]"#)),
        (r"'a{ x = 1 }' PARSESELECTOR", Ok(r#"[{"x ":"= 1"},"a"]"#)),
        (
            r"'a{x=1+2%252B}' PARSESELECTOR",
            Ok(r#"[{"x":"=1+2+"},"a"]"#),
        ),
        (
            r"1 PARSESELECTOR",
            Err(r"PARSESELECTOR expects a String on top of the stack."),
        ),
        (r"'a{=1}' PARSESELECTOR", Ok(r#"[{"":"=1"},"a"]"#)),
        (r"'a{x=}' PARSESELECTOR", Ok(r#"[{"x":"="},"a"]"#)),
        (r"'a{x~}' PARSESELECTOR", Ok(r#"[{"x":"~"},"a"]"#)),
        (
            r"'a{%0A}' PARSESELECTOR",
            Err(r"PARSESELECTOR could not parse input. ( invalid syntax for selector.)"),
        ),
        (r"'%0A{}' PARSESELECTOR", Ok(r#"[{},"\n"]"#)),
        (
            r"'a{x=%25}' PARSESELECTOR",
            Err(r"URLDecoder: Incomplete trailing escape (%) pattern"),
        ),
        (r"'a{x=1,}' PARSESELECTOR", Ok(r#"[{"x":"=1"},"a"]"#)),
        (
            r"'a}{' PARSESELECTOR",
            Err(r"PARSESELECTOR could not parse input. ( invalid syntax for selector.)"),
        ),
        (
            r"'a{' PARSESELECTOR",
            Err(r"PARSESELECTOR could not parse input. ( invalid syntax for selector.)"),
        ),
        (
            r"'a{x=1' PARSESELECTOR",
            Err(r"PARSESELECTOR could not parse input. ( invalid syntax for selector.)"),
        ),
        (r"PARSESELECTOR", Err(r"Empty stack.")),
        (
            r"'a{x=%25C3%25A9}' PARSESELECTOR",
            Ok(r#"[{"x":"=\u00E9"},"a"]"#),
        ),
        (
            r"'a{%25zz=1}' PARSESELECTOR",
            Err(
                r#"URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: "z" = 122"#,
            ),
        ),
    ]);
}

#[test]
fn toselector() {
    check(&[
        (r"@G1 TOSELECTOR", Ok(r#"["a{x=1,y=2}"]"#)),
        (r"NEWGTS TOSELECTOR", Ok(r#"["{}"]"#)),
        (
            r"NEWGTS '=a b' RENAME { 'k,' 'v}' 'b' 'é' } RELABEL TOSELECTOR",
            Ok(r#"["=%3Da%20b{b=%C3%A9,k%2C=v%7D}"]"#),
        ),
        (r"NEWGTS '~x' RENAME TOSELECTOR", Ok(r#"["=%7Ex{}"]"#)),
        (r"[ @G1 NEWGTS ] TOSELECTOR", Ok(r#"[["a{x=1,y=2}","{}"]]"#)),
        (
            r"1 TOSELECTOR",
            Err(
                r"TOSELECTOR expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
            ),
        ),
        (
            r"[ @G1 1 ] TOSELECTOR",
            Err(
                r"TOSELECTOR expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
            ),
        ),
        (r"[ ] TOSELECTOR", Ok(r"[[]]")),
        (r"@G5 TOSELECTOR", Ok(r#"["{}"]"#)),
        (
            r"NEWGTS 'n' RENAME { 'b' '1' 'a' '2' 'B' '3' } RELABEL TOSELECTOR",
            Ok(r#"["n{B=3,a=2,b=1}"]"#),
        ),
        (
            r"@G1 TOSELECTOR PARSESELECTOR",
            Ok(r#"[{"x":"=1","y":"=2"},"a"]"#),
        ),
        (r"@G6 TOSELECTOR", Ok(r#"["a%20b{x=%C3%A9}"]"#)),
        (r"TOSELECTOR", Err(r"Empty stack.")),
        (
            r"[ [ @G1 ] ] TOSELECTOR",
            Err(
                r"TOSELECTOR expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
            ),
        ),
    ]);
}

#[test]
fn byselector() {
    check(&[
        (
            r"[ @ALL [] '~.*{}' filter.byselector ] FILTER @NAMES",
            Ok(
                r#"[[["a",{"x":"1","y":"2"}],["ab",{"x":"2"}],["",{}],["a",{"z":"9"}],["a b",{"x":"\u00E9"}]]]"#,
            ),
        ),
        (
            r"[ @ALL [] '={}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["",{}]]]"#),
        ),
        (
            r"[ @ALL [] 'a{}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}],["a",{"z":"9"}]]]"#),
        ),
        (
            r"[ @ALL [] '=a{}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}],["a",{"z":"9"}]]]"#),
        ),
        (
            r"[ @ALL [] '~a.*{}' filter.byselector ] FILTER @NAMES",
            Ok(
                r#"[[["a",{"x":"1","y":"2"}],["ab",{"x":"2"}],["a",{"z":"9"}],["a b",{"x":"\u00E9"}]]]"#,
            ),
        ),
        (
            r"[ @ALL [] '~a.*{x=1}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{x~[12]}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}],["ab",{"x":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{z=3}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{z=3}{}' filter.byselector ] FILTER @NAMES",
            Ok(r"[[]]"),
        ),
        (
            r"[ @ALL [] '~.*{}{z=3}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{y=2}{z=3}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{y=}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["",{}],["a",{"z":"9"}],["a b",{"x":"\u00E9"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{z=}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["ab",{"x":"2"}],["",{}],["a b",{"x":"\u00E9"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{z=}{}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}],["ab",{"x":"2"}],["",{}],["a b",{"x":"\u00E9"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{}{z=}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["ab",{"x":"2"}],["",{}],["a",{"z":"9"}],["a b",{"x":"\u00E9"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{x~}' filter.byselector ] FILTER @NAMES",
            Ok(r"[[]]"),
        ),
        (
            r"[ @ALL [] 'a{x=1,y=2}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{x=1}{y=2}' filter.byselector ] FILTER @NAMES",
            Ok(r"[[]]"),
        ),
        (
            r"[ @ALL [] '~{}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["",{}]]]"#),
        ),
        (
            r"[ @ALL [] '={x=2}{y=2}' filter.byselector ] FILTER @NAMES",
            Ok(r"[[]]"),
        ),
        (
            r"[ @ALL [] 'a%20b{}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a b",{"x":"\u00E9"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{w=}' filter.byselector ] FILTER @NAMES",
            Ok(
                r#"[[["a",{"x":"1","y":"2"}],["ab",{"x":"2"}],["",{}],["a",{"z":"9"}],["a b",{"x":"\u00E9"}]]]"#,
            ),
        ),
        (
            r"[ @ALL [] '~.*{}{}' filter.byselector ] FILTER @NAMES",
            Ok(
                r#"[[["a",{"x":"1","y":"2"}],["ab",{"x":"2"}],["",{}],["a",{"z":"9"}],["a b",{"x":"\u00E9"}]]]"#,
            ),
        ),
        (
            r"[ @ALL [] '~.*{y=2}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}],["ab",{"x":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{x=%C3%A9}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a b",{"x":"\u00E9"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{x=1,w=}{}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{}{z=3,w=}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{x=}{z=}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["",{}],["a",{"z":"9"}]]]"#),
        ),
        (
            r"[ @ALL [] '~.*{y=}{y=2}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["ab",{"x":"2"}]]]"#),
        ),
        (
            r"[ @ALL [] '~a{x=1}' filter.byselector ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}]]]"#),
        ),
        (
            r"'a' filter.byselector",
            Err(r"invalid syntax for selector."),
        ),
        (
            r"'a{' filter.byselector",
            Err(r"invalid syntax for selector."),
        ),
        (
            r"'a{}x' filter.byselector",
            Err(r"invalid syntax for selector."),
        ),
        (
            r"'a{}{' filter.byselector",
            Err(r"invalid syntax for selector."),
        ),
        (
            r"'a{}{}}' filter.byselector",
            Err(r"invalid syntax for selector."),
        ),
        (
            r"'a{}{}{}' filter.byselector",
            Err(r"invalid syntax for selector."),
        ),
        (
            r"'{}' filter.byselector",
            Err(r"invalid syntax for selector."),
        ),
        (
            r"'~[{}' filter.byselector",
            Err(r"Parsing error at position 1: Invalid character class"),
        ),
        (
            r"'a{x~[}' filter.byselector",
            Err(r"Parsing error at position 1: Invalid character class"),
        ),
        (r"'a{x}' filter.byselector", Err(r"x")),
        (r"'a{}{x}' filter.byselector", Err(r"x")),
        (
            r"1 filter.byselector",
            Err(r"filter.byselector expects a STRING selector"),
        ),
        (
            r"'a%25zz{}' filter.byselector",
            Err(
                r#"URLDecoder: Illegal hex characters in escape (%) pattern - not a hexadecimal digit: "z" = 122"#,
            ),
        ),
        (
            r"'a{x=1}' filter.byselector SNAPSHOT",
            Ok(r#"["'a%7Bx%3D1%7D' filter.byselector "]"#),
        ),
        (r"'a{x=1}' filter.byselector TYPEOF", Ok(r#"["FILTER"]"#)),
        (
            r"'a{}{x~[}' filter.byselector",
            Err(r"Parsing error at position 1: Invalid character class"),
        ),
        (
            r"'a{x={y}' filter.byselector",
            Ok(r#"["'a%7Bx%3D%7By%7D' filter.byselector"]"#),
        ),
        (
            r"'a{x=1}{y={}' filter.byselector SNAPSHOT",
            Ok(r#"["'a%7Bx%3D1%7D%7By%3D%7B%7D' filter.byselector "]"#),
        ),
        (r"filter.byselector", Err(r"Empty stack.")),
        (
            r"[ @ALL @ALL [] '~a.*{}' filter.byselector ] FILTER @NAMES",
            Ok(
                r#"[[["a",{"x":"1","y":"2"}],["ab",{"x":"2"}],["a",{"z":"9"}],["a b",{"x":"\u00E9"}],["a",{"x":"1","y":"2"}],["ab",{"x":"2"}],["a",{"z":"9"}],["a b",{"x":"\u00E9"}]]]"#,
            ),
        ),
    ]);
}

#[test]
fn bymetadata() {
    check(&[
        (
            r"[ @ALL [] [ @G1 @G4 ] filter.bymetadata ] FILTER @NAMES",
            Ok(r#"[[["a",{"x":"1","y":"2"}]]]"#),
        ),
        (
            r"[ @G1 ] filter.bymetadata SNAPSHOT",
            Ok(
                r#"["[ 'a%7Bx%3D1%2Cy%3D2%7D' PARSESELECTOR NEWGTS SWAP RELABEL SWAP RENAME ] filter.bymetadata "]"#,
            ),
        ),
        (
            r"[ @G1 @G1 ] filter.bymetadata SNAPSHOT",
            Ok(
                r#"["[ 'a%7Bx%3D1%2Cy%3D2%7D' PARSESELECTOR NEWGTS SWAP RELABEL SWAP RENAME ] filter.bymetadata "]"#,
            ),
        ),
        (
            r"[ ] filter.bymetadata SNAPSHOT",
            Ok(r#"["[ ] filter.bymetadata "]"#),
        ),
        (
            r"1 filter.bymetadata",
            Err(r"filter.bymetadata expects a list of GTS on top of the stack."),
        ),
        (
            r"[ 1 ] filter.bymetadata",
            Err(r"filter.bymetadata expects a list of GTS on top of the stack."),
        ),
        (
            r"[ @ALL [] [ @G1 { 'z' '4' } SETATTRIBUTES ] filter.bymetadata ] FILTER SIZE",
            Ok(r"[0]"),
        ),
        (
            r"[ @ALL [] [ @G5 @G3 ] filter.bymetadata ] FILTER @NAMES",
            Ok(r"[[]]"),
        ),
        (r"[ @ALL [] [ ] filter.bymetadata ] FILTER SIZE", Ok(r"[0]")),
        (
            r"[ @G6 ] filter.bymetadata SNAPSHOT",
            Ok(
                r#"["[ 'a%2520b%7Bx%3D%25C3%25A9%7D' PARSESELECTOR NEWGTS SWAP RELABEL SWAP RENAME ] filter.bymetadata "]"#,
            ),
        ),
        (
            r"[ NEWGTS '~x' RENAME ] filter.bymetadata SNAPSHOT",
            Ok(
                r#"["[ '%3D%257Ex%7B%7D' PARSESELECTOR NEWGTS SWAP RELABEL SWAP RENAME ] filter.bymetadata "]"#,
            ),
        ),
        (r"[ @G1 ] filter.bymetadata TYPEOF", Ok(r#"["FILTER"]"#)),
        (r"filter.bymetadata", Err(r"Empty stack.")),
    ]);
}

#[test]
fn latencies() {
    check(&[
        (
            r"[ [ @U ] [ @D1 ] [] 0 100 @OPTS filter.latencies ] FILTER",
            Ok(
                r#"[[{"c":"u:uplink.latency.min","l":{"k":"v"},"a":{},"la":0,"v":[[10,5],[25,-1],[20,10],[40,1]]},{"c":"u:uplink.latency.max","l":{"k":"v"},"a":{},"la":0,"v":[[10,8],[25,-1],[20,10],[40,1]]},{"c":"d:downlink.latency.min","l":{},"a":{},"la":0,"v":[[10,5],[25,-1],[20,10],[40,1]]},{"c":"d:downlink.latency.max","l":{},"a":{},"la":0,"v":[[10,8],[25,-1],[20,10],[40,1]]},{"c":"d:downlink.matches","l":{},"a":{},"la":0,"v":[[10,2],[25,0],[20,1],[40,1]]},{"c":"u:downlinks.bitset","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[25,0],[20,1],[40,1]]},{"c":"u:downlinks.totalmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[25,0],[20,1],[40,1]]},{"c":"u:downlinks.withmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[25,0],[20,1],[40,1]]}]]"#,
            ),
        ),
        (
            r"[ [ @U ] [ @D1 @D2 ] [] 0 100 @OPTS filter.latencies ] FILTER",
            Ok(
                r#"[[{"c":"u:uplink.latency.min","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,1]]},{"c":"u:uplink.latency.max","l":{"k":"v"},"a":{},"la":0,"v":[[10,8],[25,1],[20,10],[40,1]]},{"c":"d:downlink.latency.min","l":{},"a":{},"la":0,"v":[[10,5],[25,-1],[20,10],[40,1]]},{"c":"e:downlink.latency.min","l":{},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,-1]]},{"c":"d:downlink.latency.max","l":{},"a":{},"la":0,"v":[[10,8],[25,-1],[20,10],[40,1]]},{"c":"e:downlink.latency.max","l":{},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,-1]]},{"c":"d:downlink.matches","l":{},"a":{},"la":0,"v":[[10,2],[25,0],[20,1],[40,1]]},{"c":"e:downlink.matches","l":{},"a":{},"la":0,"v":[[10,1],[25,1],[20,1],[40,0]]},{"c":"u:downlinks.bitset","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[25,2],[20,3],[40,1]]},{"c":"u:downlinks.totalmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[25,1],[20,2],[40,1]]},{"c":"u:downlinks.withmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,1]]}]]"#,
            ),
        ),
        (
            r"[ [ @U ] [ @D1 @D2 ] [] 4 100 @OPTS filter.latencies ] FILTER",
            Ok(
                r#"[[{"c":"u:uplink.latency.min","l":{"k":"v"},"a":{},"la":0,"v":[[10,5],[25,-1],[20,10],[40,-1]]},{"c":"u:uplink.latency.max","l":{"k":"v"},"a":{},"la":0,"v":[[10,8],[25,-1],[20,10],[40,-1]]},{"c":"d:downlink.latency.min","l":{},"a":{},"la":0,"v":[[10,5],[25,-1],[20,10],[40,-1]]},{"c":"e:downlink.latency.min","l":{},"a":{},"la":0,"v":[[10,-1],[25,-1],[20,-1],[40,-1]]},{"c":"d:downlink.latency.max","l":{},"a":{},"la":0,"v":[[10,8],[25,-1],[20,10],[40,-1]]},{"c":"e:downlink.latency.max","l":{},"a":{},"la":0,"v":[[10,-1],[25,-1],[20,-1],[40,-1]]},{"c":"d:downlink.matches","l":{},"a":{},"la":0,"v":[[10,2],[25,0],[20,1],[40,0]]},{"c":"e:downlink.matches","l":{},"a":{},"la":0,"v":[[10,0],[25,0],[20,0],[40,0]]},{"c":"u:downlinks.bitset","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[25,0],[20,1],[40,0]]},{"c":"u:downlinks.totalmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[25,0],[20,1],[40,0]]},{"c":"u:downlinks.withmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,1],[25,0],[20,1],[40,0]]}]]"#,
            ),
        ),
        (
            r"[ [ @U ] [ @D1 @D2 ] [] 0 5 @OPTS filter.latencies ] FILTER",
            Ok(
                r#"[[{"c":"u:uplink.latency.min","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,1]]},{"c":"u:uplink.latency.max","l":{"k":"v"},"a":{},"la":0,"v":[[10,5],[25,1],[20,2],[40,1]]},{"c":"d:downlink.latency.min","l":{},"a":{},"la":0,"v":[[10,5],[25,-1],[20,-1],[40,1]]},{"c":"e:downlink.latency.min","l":{},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,-1]]},{"c":"d:downlink.latency.max","l":{},"a":{},"la":0,"v":[[10,5],[25,-1],[20,-1],[40,1]]},{"c":"e:downlink.latency.max","l":{},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,-1]]},{"c":"d:downlink.matches","l":{},"a":{},"la":0,"v":[[10,1],[25,0],[20,0],[40,1]]},{"c":"e:downlink.matches","l":{},"a":{},"la":0,"v":[[10,1],[25,1],[20,1],[40,0]]},{"c":"u:downlinks.bitset","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[25,2],[20,2],[40,1]]},{"c":"u:downlinks.totalmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[25,1],[20,1],[40,1]]},{"c":"u:downlinks.withmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[25,1],[20,1],[40,1]]}]]"#,
            ),
        ),
        (
            r"[ [ @U ] [ @D2 ] [ @D1 ] [] -5 100 @OPTS filter.latencies ] FILTER",
            Ok(
                r#"[[{"c":"u:uplink.latency.min","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,1]]},{"c":"u:uplink.latency.max","l":{"k":"v"},"a":{},"la":0,"v":[[10,8],[25,1],[20,10],[40,1]]},{"c":"e:downlink.latency.min","l":{},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,-1]]},{"c":"d:downlink.latency.min","l":{},"a":{},"la":0,"v":[[10,5],[25,-1],[20,10],[40,1]]},{"c":"e:downlink.latency.max","l":{},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,-1]]},{"c":"d:downlink.latency.max","l":{},"a":{},"la":0,"v":[[10,8],[25,-1],[20,10],[40,1]]},{"c":"e:downlink.matches","l":{},"a":{},"la":0,"v":[[10,1],[25,1],[20,1],[40,0]]},{"c":"d:downlink.matches","l":{},"a":{},"la":0,"v":[[10,2],[25,0],[20,1],[40,1]]},{"c":"u:downlinks.bitset","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[25,1],[20,3],[40,2]]},{"c":"u:downlinks.totalmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,3],[25,1],[20,2],[40,1]]},{"c":"u:downlinks.withmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,2],[25,1],[20,2],[40,1]]}]]"#,
            ),
        ),
        (
            r"[ [ ] [ @D1 ] [] 0 100 [ 'downlink.matches' ] filter.latencies ] FILTER",
            Ok(r"[[]]"),
        ),
        (
            r"[ [ @U @U ] [ @D1 ] [] 0 100 [ 'downlink.matches' ] filter.latencies ] FILTER",
            Ok(r"[[]]"),
        ),
        (
            r"[ [ @U ] [ ] [] 0 100 [ 'downlink.matches' 'uplink.latency.min' ] filter.latencies ] FILTER",
            Ok(
                r#"[[{"c":"u:uplink.latency.min","l":{"k":"v"},"a":{},"la":0,"v":[[10,-1],[25,-1],[20,-1],[40,-1]]}]]"#,
            ),
        ),
        (
            r"[ [ @U ] [] 0 100 @OPTS filter.latencies ] FILTER",
            Ok(
                r#"[[{"c":"u:uplink.latency.min","l":{"k":"v"},"a":{},"la":0,"v":[[10,-1],[25,-1],[20,-1],[40,-1]]},{"c":"u:uplink.latency.max","l":{"k":"v"},"a":{},"la":0,"v":[[10,-1],[25,-1],[20,-1],[40,-1]]},{"c":"u:downlinks.bitset","l":{"k":"v"},"a":{},"la":0,"v":[[10,0],[25,0],[20,0],[40,0]]},{"c":"u:downlinks.totalmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,0],[25,0],[20,0],[40,0]]},{"c":"u:downlinks.withmatches","l":{"k":"v"},"a":{},"la":0,"v":[[10,0],[25,0],[20,0],[40,0]]}]]"#,
            ),
        ),
        (
            r"@U 'u' STORE [ [ $u ] [ @D1 ] [] 0 100 [ 'downlink.matches' ] filter.latencies ] FILTER DROP $u",
            Ok(r#"[{"c":"u","l":{"k":"v"},"a":{},"la":0,"v":[[10,5],[25,6],[20,7],[40,9]]}]"#),
        ),
        (
            r"[ [ NEWGTS 'u' RENAME 1 NaN NaN NaN 'a' ADDVALUE ] [ @D1 ] [] 0 100 [ ] filter.latencies ] FILTER",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"[ [ @U ] [ NEWGTS 'd' RENAME 1 NaN NaN NaN true ADDVALUE ] [] 0 100 [ ] filter.latencies ] FILTER",
            Err(r"class java.lang.Boolean cannot be cast to class java.lang.Number"),
        ),
        (
            r"[ [ @U ] [ NEWGTS 1 NaN NaN NaN 5 ADDVALUE ] [] 0 100 [ 'downlink.matches' ] filter.latencies ] FILTER",
            Ok(
                r#"[[{"c":":downlink.matches","l":{},"a":{},"la":0,"v":[[10,0],[25,0],[20,0],[40,0]]}]]"#,
            ),
        ),
        (
            r"[ [ NEWGTS ] [ @D1 ] [] 0 100 [ 'uplink.latency.max' ] filter.latencies ] FILTER",
            Ok(r#"[[{"c":":uplink.latency.max","l":{},"a":{},"la":0,"v":[]}]]"#),
        ),
        (
            r"[ [ @G5 ] [ @D1 ] [] 0 100 [ 'uplink.latency.max' ] filter.latencies ] FILTER",
            Ok(r#"[[{"c":"null:uplink.latency.max","l":{},"a":{},"la":0,"v":[]}]]"#),
        ),
        (
            r"1 1000 [ 'downlink.matches' 3 [ 1 ] ] filter.latencies SNAPSHOT",
            Ok(r#"["1 1000 [ 'downlink.matches' '3' '%5B1%5D' ] filter.latencies "]"#),
        ),
        (r"1 1000 [ ] filter.latencies TYPEOF", Ok(r#"["FILTER"]"#)),
        (
            r"1 2 'x' filter.latencies",
            Err(r"filter.latencies expects a list of options on top of the stack."),
        ),
        (
            r"1 2.0 [ ] filter.latencies",
            Err(r"filter.latencies expects a maximum latency under the list of options."),
        ),
        (
            r"1.0 2 [ ] filter.latencies",
            Err(r"filter.latencies expects a minimum latency under the list of options."),
        ),
        (r"[ ] filter.latencies", Err(r"Empty stack.")),
        (r"2 [ ] filter.latencies", Err(r"Empty stack.")),
        (r"filter.latencies", Err(r"Empty stack.")),
    ]);
}
