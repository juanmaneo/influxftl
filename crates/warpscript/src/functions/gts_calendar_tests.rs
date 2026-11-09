// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Oracle-checked tests of `BUCKETIZE.CALENDAR`, `UNBUCKETIZE.CALENDAR` and `UNBUCKETIZE`.
//! The expectations are the answers of Warp 10 3.5.0 to the same scripts:
//! the JSON stack (top first) or the error message (in its JDK 11+ form).

use crate::tests::test_stack;
use crate::value::Value;

const S: &str = r"NEWGTS 's' RENAME { 'k' 'v' } RELABEL 1 0 0 NaN 1 ADDVALUE 3 NaN NaN 10 4 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 7 NaN NaN NaN 3 ADDVALUE 5 NaN NaN NaN 9 ADDVALUE 6 NaN NaN NaN 8.5 ADDVALUE 9 NaN NaN NaN 1 ADDVALUE 10 NaN NaN NaN 0 ADDVALUE";
const M: &str = r"NEWGTS 'm' RENAME 0 NaN NaN NaN 0 ADDVALUE 432000000000 NaN NaN NaN 5 ADDVALUE 1728000000000 NaN NaN NaN 20 ADDVALUE 2678400000000 NaN NaN NaN 31 ADDVALUE 3456000000000 NaN NaN NaN 40 ADDVALUE 5011200000000 NaN NaN NaN 58 ADDVALUE 5097600000000 NaN NaN NaN 59 ADDVALUE 5184000000000 NaN NaN NaN 60 ADDVALUE 7776000000000 NaN NaN NaN 90 ADDVALUE 8640000000000 NaN NaN NaN 100 ADDVALUE 10368000000000 NaN NaN NaN 120 ADDVALUE 1296000000000 48.0 -4.5 100 15 ADDVALUE";
const P: &str = r"NEWGTS 'p' RENAME 1679787000000000 NaN NaN NaN 0 ADDVALUE 1679788800000000 NaN NaN NaN 1 ADDVALUE 1679790600000000 NaN NaN NaN 2 ADDVALUE 1679792400000000 NaN NaN NaN 3 ADDVALUE 1679794200000000 NaN NaN NaN 4 ADDVALUE 1679796000000000 NaN NaN NaN 5 ADDVALUE 1679797800000000 NaN NaN NaN 6 ADDVALUE 1679799600000000 NaN NaN NaN 7 ADDVALUE 1679801400000000 NaN NaN NaN 8 ADDVALUE 1679803200000000 NaN NaN NaN 9 ADDVALUE 1679805000000000 NaN NaN NaN 10 ADDVALUE 1679806800000000 NaN NaN NaN 11 ADDVALUE";
const N: &str = r"NEWGTS 'n' RENAME -86400000000 NaN NaN NaN 1.5 ADDVALUE -172800000000 NaN NaN NaN 2.5 ADDVALUE -2592000000000 NaN NaN NaN 30.5 ADDVALUE -2678400000000 NaN NaN NaN 31.5 ADDVALUE -2764800000000 NaN NaN NaN 32.5 ADDVALUE -6048000000000 NaN NaN NaN 70.5 ADDVALUE";
const E: &str = r"NEWGTS 'e' RENAME";
const B: &str = r"NEWGTS 'b' RENAME true 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE";
const U: &str = r"PT0.000002S";
const ATTRS: &str = r"{ '.bucketduration' 'PT1S' '.bucketoffset' '0' '.buckettimezone' 'UTC' }";

/// The script of a template, its `@NAME` placeholders replaced by the series above.
fn expand(template: &str) -> String {
    template
        .replace("@ATTRS", ATTRS)
        .replace("@S", S)
        .replace("@M", M)
        .replace("@P", P)
        .replace("@N", N)
        .replace("@E", E)
        .replace("@B", B)
        .replace("@U", U)
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
fn bucketize_calendar_errors() {
    check(&[
        (
            r"1 BUCKETIZE.CALENDAR",
            Err(r"BUCKETIZE.CALENDAR expects a list as input."),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' ] BUCKETIZE.CALENDAR",
            Err(r"BUCKETIZE.CALENDAR needs a list of at least 5 parameters as input."),
        ),
        (
            r"[ 1 bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects a Geo Time Series or a list of Geo Time Series as first parameter.",
            ),
        ),
        (
            r"[ [ @S 1 ] bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"BUCKETIZE.CALENDAR expects a list of Geo Time Series as first parameter."),
        ),
        (
            r"[ @S 'x' 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects a bucketizer function or a macro as fourth to last parameter.",
            ),
        ),
        (
            r"[ @S mapper.add 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"Invalid parameter for mapper.add"),
        ),
        (
            r"[ @S bucketizer.last 10.0 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects lastbucket, bucketduration, bucketcount (and optionally timezone) as last parameters.",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects lastbucket, bucketduration, bucketcount (and optionally timezone) as last parameters.",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 0.0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects lastbucket, bucketduration, bucketcount (and optionally timezone) as last parameters.",
            ),
        ),
        (
            r"[ @S bucketizer.last 0 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"BUCKETIZE.CALENDAR does not allow lastbucket to be 0. It must be specified."),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' -1 ] BUCKETIZE.CALENDAR",
            Err(r"BUCKETIZE.CALENDAR expects a positive bucketcount."),
        ),
        (
            r"[ @S @ATTRS SETATTRIBUTES bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects GTS for which the attributes .bucketduration, .bucketoffset and .buckettimezone are not set. If an input GTS is supposed to be already duration-bucketized, duration-unbucketize it first before applying a new duration-bucketization.",
            ),
        ),
        (
            r"[ @S { '.bucketoffset' '0' } SETATTRIBUTES bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects GTS for which the attributes .bucketduration, .bucketoffset and .buckettimezone are not set. If an input GTS is supposed to be already duration-bucketized, duration-unbucketize it first before applying a new duration-bucketization.",
            ),
        ),
        (
            r"[ @S { '.buckettimezone' 'UTC' } SETATTRIBUTES bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects GTS for which the attributes .bucketduration, .bucketoffset and .buckettimezone are not set. If an input GTS is supposed to be already duration-bucketized, duration-unbucketize it first before applying a new duration-bucketization.",
            ),
        ),
        (
            r"[ @S NULL 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"BUCKETIZE.CALENDAR expects a non null bucketizer."),
        ),
        (
            r"[ @S bucketizer.last 10 'T1S' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects that the bucketduration is in ISO8601 duration format.",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 'Pxx' 0 ] BUCKETIZE.CALENDAR",
            Err(r"/ by zero"),
        ),
        (
            r"[ @S bucketizer.last 10 'PT1.2.3S' 0 ] BUCKETIZE.CALENDAR",
            Err(r"BUCKETIZE.CALENDAR encountered an exception. (Invalid ISO8601 duration)"),
        ),
        (
            r"[ @S bucketizer.last 10 'PT1.xS' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r#"BUCKETIZE.CALENDAR encountered an exception. (Parsing of sub second precision part of duration has failed. tried to parse: x (For input string: "0.x"))"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 'PT-1S' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects the bucketduration parameter to be a positive ISO8601 duration.",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 'PT0S' 0 ] BUCKETIZE.CALENDAR",
            Err(r"/ by zero"),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 0 'Foo/Bar' ] BUCKETIZE.CALENDAR",
            Err(r"The datetime zone id 'Foo/Bar' is not recognised"),
        ),
        (
            r"[ @M bucketizer.last 8640000000000 'P1MT-2592000S' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Period is negative from timestamp 8640000000000. Can not add a negative period. Period is -2 times P1MT-2592000S plus 0 time unit(s).)",
            ),
        ),
        (
            r"[ @M bucketizer.last -8640000000000 'P1MT-2592000S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1MT-2592000S",".bucketoffset":"86399999999"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 864000000000 'P1MT-2592000S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1MT-2592000S",".bucketoffset":"86399999999"},"la":0,"v":[[3,0],[15,5]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 3456000000000 'P1MT-2592000S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1MT-2592000S",".bucketoffset":"86399999999"},"la":0,"v":[[4,0],[16,5],[39,47.99999998882413,-4.500000067055225,100,15],[51,20],[75,31],[93,40]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 3456000000000 'P1MT-2592000,5S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1MT-2592000,5S",".bucketoffset":"86352999999"},"la":0,"v":[[4,0],[16,5],[39,47.99999998882413,-4.500000067055225,100,15],[51,20],[75,31],[93,40]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 3456000000000 'P1MT-2592000S' 0 'Europe/Paris' ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"Europe/Paris",".bucketduration":"P1MT-2592000S",".bucketoffset":"86399999999"},"la":0,"v":[[4,0],[16,5],[39,47.99999998882413,-4.500000067055225,100,15],[51,20],[75,31],[93,40]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 3456000000000 'P1DT-86400.5S' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 6912001 times P1DT-86400S plus 500000 time unit(s).",
            ),
        ),
        (
            r"[ @M bucketizer.last -3456000000000 'P1DT-86400.5S' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is -6912000 times P1DT-86400S plus 500000 time unit(s).",
            ),
        ),
        (
            r"[ @M bucketizer.last 3456000000000 'P1Y2M3W4DT5H6M7,250S' 0 ] BUCKETIZE.CALENDAR 0 GET ATTRIBUTES",
            Ok(
                r#"[{".buckettimezone":"UTC",".bucketduration":"P1Y2M3W4DT5H6M7,250S",".bucketoffset":"35355967249999"}]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 1000 'P1DT-86399.999S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1DT-86399.999S",".bucketoffset":"999"},"la":0,"v":[[0,0]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 1000 'P1DT-86399,5S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1DT-86399,5S",".bucketoffset":"498999"},"la":0,"v":[[0,0]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 1000 'P1DT-86400,5S' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects the bucketduration parameter to be a positive ISO8601 duration.",
            ),
        ),
        (
            r"[ @M bucketizer.last 1000 'P1DT-86401S' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects the bucketduration parameter to be a positive ISO8601 duration.",
            ),
        ),
        (r"BUCKETIZE.CALENDAR", Err(r"Empty stack.")),
        (
            r"[ @S bucketizer.last 10 '' 0 ] BUCKETIZE.CALENDAR",
            Err(r"Index 0 out of bounds for length 0"),
        ),
        (
            r"[ @M bucketizer.last 36720000000000 'P1MT-2592000S' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Period is negative from timestamp 36720000000000. Can not add a negative period. Period is -1 times P1MT-2592000S plus 0 time unit(s).)",
            ),
        ),
        (
            r"[ @M bucketizer.last 36720000000000 'P1MT-2592000S' 0 'Europe/Paris' ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Period is negative from timestamp 36720000000000. Can not add a negative period. Period is -1 times P1MT-2592000S plus 0 time unit(s).)",
            ),
        ),
        (
            r"[ @M bucketizer.last 34560000000000 'P1MT-2592000S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1MT-2592000S",".bucketoffset":"86399999999"},"la":0,"v":[[3,0],[15,5],[37,47.99999998882413,-4.500000067055225,100,15],[49,20],[75,31],[97,40],[140,58],[142,59],[144,60],[212,90],[236,100],[279,120]]}]]"#,
            ),
        ),
    ]);
}

#[test]
fn bucketize_calendar() {
    check(&[
        (
            r"[ @S bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,2],[2,10,4],[3,8],[4,3],[5,0]]}]]"#,
            ),
        ),
        (
            r"[ @S bucketizer.first 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,0.0,0.0,1],[2,10,4],[3,9],[4,3],[5,1]]}]]"#,
            ),
        ),
        (
            r"[ @S bucketizer.sum 9 'PT0.000003S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000003S",".bucketoffset":"2"},"la":0,"v":[[1,10,7],[2,17],[3,4]]}]]"#,
            ),
        ),
        (
            r"[ @S bucketizer.count 8 'PT0.000003S' 2 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000003S",".bucketoffset":"0"},"la":0,"v":[[2,2]]}]]"#,
            ),
        ),
        (
            r"[ @S bucketizer.mean 100 'PT0.000003S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000003S",".bucketoffset":"1"},"la":0,"v":[[0,0.0,0.0,1.0],[1,10,3.0],[2,6.666666666666667],[3,0.5]]}]]"#,
            ),
        ),
        (
            r"[ @S bucketizer.mean 100 'PT0.000003S' 0 ] BUCKETIZE.CALENDAR 0 GET DUP BUCKETSPAN SWAP DUP BUCKETCOUNT SWAP LASTBUCKET",
            Ok(r"[33,34,1]"),
        ),
        (
            r"[ @S bucketizer.last -5 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"0"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 0 ] DUP BUCKETIZE.CALENDAR SWAP SIZE",
            Ok(
                r#"[5,[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,2],[2,10,4],[3,8],[4,3],[5,0]]}]]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 3 'Europe/Paris' ] DUP BUCKETIZE.CALENDAR SWAP SIZE",
            Ok(
                r#"[5,[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"Europe/Paris",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[4,3],[5,0]]}]]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 'UTC' ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR expects a bucketizer function or a macro as fourth to last parameter.",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 0 '+02:00' ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"+02:00",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,2],[2,10,4],[3,8],[4,3],[5,0]]}]]"#,
            ),
        ),
        (
            r"[ @S @S bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,2],[2,10,4],[3,8],[4,3],[5,0]]},{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,2],[2,10,4],[3,8],[4,3],[5,0]]}]]"#,
            ),
        ),
        (
            r"[ [ @S @E ] @M bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,2],[2,10,4],[3,8],[4,3],[5,0]]},{"c":"e","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[]},{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[0,0]]}]]"#,
            ),
        ),
        (
            r"[ [ ] bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(r"[[]]"),
        ),
        (
            r"[ @E bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"e","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @B bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"ADDVALUE operates on a single Geo Time Series or GTS Encoder."),
        ),
        (
            r"[ @B bucketizer.sum 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"ADDVALUE operates on a single Geo Time Series or GTS Encoder."),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR DROP @S",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[2,2],[7,3],[5,9],[6,8],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"[ @M bucketizer.sum 10368000000000 'P1M' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1M",".bucketoffset":"2678399999999"},"la":0,"v":[[0,0],[1,71],[2,157],[3,150],[4,220]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.sum 10368000000005 'P1M' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1M",".bucketoffset":"2678399999994"},"la":0,"v":[[0,0],[1,71],[2,157],[3,150],[4,220]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.sum 17280000000000 'P1M' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1M",".bucketoffset":"1036799999999"},"la":0,"v":[[0,47.99999998882413,-4.500000067055225,100,20],[1,91],[2,177],[3,190],[4,120]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.sum 17280000000000 'P1M' 2 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1M",".bucketoffset":"1036799999999"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 5097600000000 'P1M' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1M",".bucketoffset":"2678399999999"},"la":0,"v":[[0,0],[1,31],[2,59]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.last 5097600000000 'P1M' 0 ] BUCKETIZE.CALENDAR 0 GET UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"m","l":{},"a":{},"la":0,"v":[[0,0],[2678400000000,31],[5097600000000,59]]}]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 10368000000000 'P1W' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1W",".bucketoffset":"518399999999"},"la":0,"v":[[0,1],[1,1],[2,47.99999998882413,-4.500000067055225,100,1],[3,1],[5,1],[6,1],[9,3],[13,1],[15,1],[17,1]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 10368000000000 'P2W' 0 'America/New_York' ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"America/New_York",".bucketduration":"P2W",".bucketoffset":"518399999999"},"la":0,"v":[[0,2],[1,2],[2,1],[3,1],[4,3],[6,1],[7,1],[8,1]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 10627200000000 'P1D' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1D",".bucketoffset":"86399999999"},"la":0,"v":[[0,1],[5,1],[15,47.99999998882413,-4.500000067055225,100,1],[20,1],[31,1],[40,1],[58,1],[59,1],[60,1],[90,1],[100,1],[120,1]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 10627200000000 'P1Y' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1Y",".bucketoffset":"20908799999999"},"la":0,"v":[[0,12]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 69120000000000 'P1Y' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1Y",".bucketoffset":"25574399999999"},"la":0,"v":[[0,9],[1,3]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 69120000000000 'P1Y' 0 ] BUCKETIZE.CALENDAR 0 GET UNBUCKETIZE.CALENDAR",
            Ok(r#"[{"c":"m","l":{},"a":{},"la":0,"v":[[5961600000000,9],[37497600000000,3]]}]"#),
        ),
        (
            r"[ @M bucketizer.count 86400000000000000 'P100Y' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P100Y",".bucketoffset":"1959465599999999"},"la":0,"v":[[0,12]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 10627200000000 'P1MT12H' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1MT12H",".bucketoffset":"2635199999999"},"la":0,"v":[[0,1],[1,4],[2,4],[3,1],[4,2]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 10627200000000 'PT36H' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"PT36H",".bucketoffset":"129599999999"},"la":0,"v":[[0,1],[4,1],[10,47.99999998882413,-4.500000067055225,100,1],[14,1],[21,1],[27,1],[39,1],[40,2],[60,1],[67,1],[80,1]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 10627200000000 'PT3600.5S' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"PT3600.5S",".bucketoffset":"1475999999"},"la":0,"v":[[0,1],[120,1],[360,47.99999998882413,-4.500000067055225,100,1],[480,1],[744,1],[960,1],[1392,1],[1416,1],[1440,1],[2160,1],[2400,1],[2880,1]]}]]"#,
            ),
        ),
        (
            r"[ @M bucketizer.count 8640000000000000 'PT36H' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"PT36H",".bucketoffset":"43199999999"},"la":0,"v":[[0,1],[3,1],[10,47.99999998882413,-4.500000067055225,100,1],[13,1],[20,1],[26,1],[38,1],[39,1],[40,1],[60,1],[66,1],[80,1]]}]]"#,
            ),
        ),
        (
            r"[ @P bucketizer.sum 1679806800000000 'PT1H' 0 'Europe/Paris' ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"p","l":{},"a":{".buckettimezone":"Europe/Paris",".bucketduration":"PT1H",".bucketoffset":"3599999999"},"la":0,"v":[[466608,1],[466609,5],[466610,9],[466611,13],[466612,17],[466613,21]]}]]"#,
            ),
        ),
        (
            r"[ @P bucketizer.sum 1679806800000000 'P1D' 0 'Europe/Paris' ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"p","l":{},"a":{".buckettimezone":"Europe/Paris",".bucketduration":"P1D",".bucketoffset":"68399999999"},"la":0,"v":[[19442,66]]}]]"#,
            ),
        ),
        (
            r"[ @P bucketizer.sum 1679806800000000 'P1D' 0 'Europe/Paris' ] BUCKETIZE.CALENDAR 0 GET UNBUCKETIZE.CALENDAR",
            Ok(r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[1679806800000000,66]]}]"#),
        ),
        (
            r"[ @P bucketizer.sum 1679806800000000 'PT2H' 0 'Europe/Paris' ] BUCKETIZE.CALENDAR 0 GET UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"p","l":{},"a":{},"la":0,"v":[[1679792400000000,6],[1679799600000000,22],[1679806800000000,38]]}]"#,
            ),
        ),
        (
            r"[ @P bucketizer.count 1679806800000000 'P1D' 0 'Asia/Kolkata' ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"p","l":{},"a":{".buckettimezone":"Asia/Kolkata",".bucketduration":"P1D",".bucketoffset":"68399999999"},"la":0,"v":[[19442,12]]}]]"#,
            ),
        ),
        (
            r"[ @N bucketizer.sum -86400000000 'P1M' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"n","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1M",".bucketoffset":"86399999999"},"la":0,"v":[[-3,70.5],[-2,32.5],[-1,66.0]]}]]"#,
            ),
        ),
        (
            r"[ @N bucketizer.sum -86400000000 'P1M' 0 ] BUCKETIZE.CALENDAR 0 GET UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"n","l":{},"a":{},"la":0,"v":[[-5356800000000,70.5],[-2764800000000,32.5],[-86400000000,66.0]]}]"#,
            ),
        ),
        (
            r"[ @N bucketizer.sum -86400000001 'P1D' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"n","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1D",".bucketoffset":"0"},"la":0,"v":[[-70,70.5],[-32,32.5],[-31,31.5],[-30,30.5],[-2,2.5]]}]]"#,
            ),
        ),
        (
            r"[ @N bucketizer.sum -86400000000000 'P1D' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"n","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1D",".bucketoffset":"86399999999"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @N bucketizer.sum 432000000000 'P7D' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"n","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P7D",".bucketoffset":"172799999999"},"la":0,"v":[[-10,70.5],[-5,94.5],[-1,2.5],[0,1.5]]}]]"#,
            ),
        ),
        (
            r"[ @N bucketizer.sum -3456000000000 'PT240H' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"n","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"PT240H",".bucketoffset":"863999999999"},"la":0,"v":[[-7,70.5]]}]]"#,
            ),
        ),
        (
            r"[ @N bucketizer.sum -86400000000 'P2D' 3 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"n","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P2D",".bucketoffset":"86399999999"},"la":0,"v":[[-1,4.0]]}]]"#,
            ),
        ),
    ]);
}

#[test]
fn bucketize_calendar_macros() {
    check(&[
        (
            r"[ @S <% DUP TICKS SWAP VALUES 2 ->LIST %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"class java.util.ArrayList cannot be cast to class java.lang.Number"),
        ),
        (
            r"[ @S <% DUP SIZE SWAP DROP %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"Empty stack."),
        ),
        (
            r"[ @S <% DROP 0 NaN NaN NaN 42 %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,42],[2,42],[3,42],[4,42],[5,42]]}]]"#,
            ),
        ),
        (
            r"[ @S <% DROP 0 1.0 2.0 7 'x' %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,0.9999999823048711,1.9999999646097422,7,"x"],[2,0.9999999823048711,1.9999999646097422,7,"x"],[3,0.9999999823048711,1.9999999646097422,7,"x"],[4,0.9999999823048711,1.9999999646097422,7,"x"],[5,0.9999999823048711,1.9999999646097422,7,"x"]]}]]"#,
            ),
        ),
        (
            r"[ @S <% DROP [ 0 NaN NaN NaN NULL ] %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @S <% DROP [ 0 NaN NaN NaN 3 ] %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,3],[2,3],[3,3],[4,3],[5,3]]}]]"#,
            ),
        ),
        (
            r"[ @S <% DROP [ 3 ] %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,3],[2,3],[3,3],[4,3],[5,3]]}]]"#,
            ),
        ),
        (
            r"[ @S <% DROP [ ] %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Expected a list of 1 to 5 elements, got 0)",
            ),
        ),
        (
            r"[ @S <% DROP 0 NaN NaN NaN { 'a' 1 } %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @S <% DROP 0 1 2 3 4 %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Macro MUST return a latitude and a longitude which are of type DOUBLE.)",
            ),
        ),
        (
            r"[ @S <% DROP 0 NaN NaN 1.5 4 %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Macro MUST return an elevations which is NaN if it is of type DOUBLE.)",
            ),
        ),
        (
            r"[ @S <% DROP 0 NaN NaN 'x' 4 %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Macro MUST return an elevation which is either NaN or LONG.)",
            ),
        ),
        (
            r"[ @S <% DROP 'x' NaN NaN NaN 4 %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Macro MUST return a tick of type LONG.)",
            ),
        ),
        (
            r"[ @S <% DROP %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"Empty stack."),
        ),
        (
            r"[ @S <% DROP 'boom' MSGFAIL %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Exception at 'MSGFAIL' in section '[TOP]' (MSGFAIL boom))",
            ),
        ),
        (
            r"[ @S <% DROP [ 'x' 1 ] %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"class java.lang.String cannot be cast to class java.lang.Number"),
        ),
        (
            r"[ @S <% DROP 1 0 GET %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Exception at 'GET' in section '[TOP]' (GET failed. (Invalid OBJECT to GET on.)))",
            ),
        ),
        (
            r"[ @S <% 'a' STORE [ 0 NaN NaN NaN $a SIZE ] $a 'k' STORE %> 10 '@U' 0 ] BUCKETIZE.CALENDAR $k",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2],[1,0.0,0.0,1]]},[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,2],[2,1],[3,2],[4,1],[5,2]]}]]"#,
            ),
        ),
        (
            r"[ @S <% 1 NaN NaN NaN 1 ADDVALUE [ 0 1 ] %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,1],[2,1],[3,1],[4,1],[5,1]]}],{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2],[1,0.0,0.0,1],[1,1]]},{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2],[1,0.0,0.0,1],[1,1]]},{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2],[1,0.0,0.0,1],[1,1]]},{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2],[1,0.0,0.0,1],[1,1]]},{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2],[1,0.0,0.0,1],[1,1]]}]"#,
            ),
        ),
        (
            r"[ @S <% DUP NAME SWAP LABELS 'k' GET + [ SWAP 0 SWAP ] %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,"sv"],[2,"sv"],[3,"sv"],[4,"sv"],[5,"sv"]]}]]"#,
            ),
        ),
        (
            r"[ @S <% DROP RETURN %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"Empty stack."),
        ),
        (
            r"[ @S <% 7 %> MACROBUCKETIZER 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"Empty stack."),
        ),
        (
            r"[ @S <% DUP 7 GET SWAP 3 GET + %> MACROBUCKETIZER 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @S <% 0 GET [ SWAP NaN NaN NaN 4 ] %> MACROBUCKETIZER 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,4],[2,4],[3,4],[4,4],[5,4]]}]]"#,
            ),
        ),
        (
            r"[ @S <% DROP { 'a' [ 0 1 ] } %> MACROBUCKETIZER 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(r"class java.util.LinkedHashMap cannot be cast to class [Ljava.lang.Object;"),
        ),
        (
            r"[ @S <% DUP 1 GET SWAP 2 GET ->JSON + [ SWAP ] %> MACROBUCKETIZER 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @M <% [ SWAP 4 GET 0 GET ] %> MACROBUCKETIZER 2592000000000 'P1M' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1M",".bucketoffset":"86399999999"},"la":0,"v":[[0,NaN]]}]]"#,
            ),
        ),
        (
            r"[ @M <% [ SWAP 6 GET ] %> MACROBUCKETIZER 2592000000000 'P1M' 0 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"m","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"P1M",".bucketoffset":"86399999999"},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ @S <% FIRSTTICK 3 < <% 'boom' MSGFAIL %> IFT [ 0 1 ] %> 10 '@U' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Exception at 'IFT' in section '[TOP]' (Exception at 'MSGFAIL' in section '[TOP]' (MSGFAIL boom)))",
            ),
        ),
        (
            r"[ @S <% FIRSTTICK 3 < <% 'boom' MSGFAIL %> IFT [ 0 1 ] %> 10 '@U' 2 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[5,1]]}]]"#,
            ),
        ),
    ]);
}

#[test]
fn bucketize_calendar_limits() {
    check(&[
        (
            r"100000 MAXBUCKETS [ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 200001 NaN NaN NaN 1 ADDVALUE bucketizer.sum 200001 'PT0.000001S' 0 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR encountered an exception. (Bucket count (200001) is exceeding maximum value of 100000. Consider raising the limit or using capabilities.)",
            ),
        ),
        (
            r"100000 MAXBUCKETS [ NEWGTS 1 NaN NaN NaN 1 ADDVALUE bucketizer.sum 100 'PT0.000001S' 10000000 ] BUCKETIZE.CALENDAR",
            Err(
                r"BUCKETIZE.CALENDAR error: bucket count (10000000) would exceed maximum value of 100000. Consider raising the limit or using capabilities.",
            ),
        ),
        (
            r"100000 MAXBUCKETS [ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 200001 NaN NaN NaN 1 ADDVALUE bucketizer.sum 200001 'PT0.000001S' 3 ] BUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"","l":{},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000001S",".bucketoffset":"0"},"la":0,"v":[[200001,1]]}]]"#,
            ),
        ),
        (
            r"[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE bucketizer.sum 100 'PT0.000001S' 3000000000 ] BUCKETIZE.CALENDAR",
            Err(r"integer overflow"),
        ),
    ]);
}

#[test]
fn unbucketize() {
    check(&[
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET UNBUCKETIZE DUP BUCKETSPAN SWAP DUP BUCKETCOUNT SWAP LASTBUCKET",
            Ok(r"[0,0,0]"),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE UNBUCKETIZE 0 GET LASTBUCKET",
            Ok(r"[0]"),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET DUP UNBUCKETIZE DROP BUCKETCOUNT",
            Ok(r"[0]"),
        ),
        (
            r"@S UNBUCKETIZE",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[3,10,4],[2,2],[7,3],[5,9],[6,8],[9,1],[10,0]]}]"#,
            ),
        ),
        (
            r"'x' UNBUCKETIZE",
            Err(r"UNBUCKETIZE can only operate on Geo Time Series instances."),
        ),
        (
            r"[ @S 'x' ] UNBUCKETIZE",
            Err(r"UNBUCKETIZE can only operate on Geo Time Series instances."),
        ),
        (r"UNBUCKETIZE", Err(r"Empty stack.")),
    ]);
}

#[test]
fn unbucketize_calendar() {
    check(&[
        (
            r"[ @S bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR 0 GET UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2],[4,10,4],[6,8],[8,3],[10,0]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2],[4,10,4],[6,8],[8,3],[10,0]]}]]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR 0 GET DUP UNBUCKETIZE.CALENDAR DROP",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{".buckettimezone":"UTC",".bucketduration":"PT0.000002S",".bucketoffset":"1"},"la":0,"v":[[1,2],[2,10,4],[3,8],[4,3],[5,0]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 0 ] BUCKETIZE.CALENDAR 0 GET UNBUCKETIZE.CALENDAR DUP BUCKETSPAN SWAP LASTBUCKET",
            Ok(r"[0,0]"),
        ),
        (
            r"[ @S bucketizer.last 10 '@U' 0 'Europe/Paris' ] BUCKETIZE.CALENDAR 0 GET UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2,2],[4,10,4],[6,8],[8,3],[10,0]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 'PT0.000003S' 0 ] BUCKETIZE.CALENDAR 0 GET UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1,0.0,0.0,1],[4,10,4],[7,3],[10,0]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 'PT0.000003S' 0 ] BUCKETIZE.CALENDAR 0 GET { 'x' 'y' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{"x":"y"},"la":0,"v":[[1,0.0,0.0,1],[4,10,4],[7,3],[10,0]]}]"#,
            ),
        ),
        (
            r"NEWGTS UNBUCKETIZE.CALENDAR",
            Err(
                r"UNBUCKETIZE.CALENDAR expects input GTS to be duration-bucketized. This information is stored in attributes.",
            ),
        ),
        (
            r"@E @ATTRS SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(r#"[{"c":"e","l":{},"a":{},"la":0,"v":[]}]"#),
        ),
        (
            r"@E @ATTRS SETATTRIBUTES { 'x' 'y' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(r#"[{"c":"e","l":{},"a":{"x":"y"},"la":0,"v":[]}]"#),
        ),
        (
            r"@S @ATTRS SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(r"UNBUCKETIZE.CALENDAR expects input GTS to be bucketized."),
        ),
        (
            r"@S { '.bucketduration' 'PT1S' '.bucketoffset' '0' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"UNBUCKETIZE.CALENDAR expects input GTS to be duration-bucketized. This information is stored in attributes.",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET @ATTRS SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[10999999,0],[9999999,1],[7999999,3],[6999999,8],[5999999,9],[3999999,10,4],[2999999,2],[1999999,0.0,0.0,1]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'Pxx' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 11 times PT0S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT1S' '.bucketoffset' 'x' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(r#"For input string: "x""#),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT1S' '.bucketoffset' '+5' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[10999994,0],[9999994,1],[7999994,3],[6999994,8],[5999994,9],[3999994,10,4],[2999994,2],[1999994,0.0,0.0,1]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT1S' '.bucketoffset' '0' '.buckettimezone' 'Foo' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(r"The datetime zone id 'Foo' is not recognised"),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT-1S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 11 times PT-1S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-2592000S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[345599999999,0],[259199999999,1],[172799999999,3],[172799999999,8],[86399999999,9],[-1,10,4],[-86400000001,2],[-86400000001,0.0,0.0,1]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-2592000S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times P1MT-2592000S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 1 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-2592000S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 2 times P1MT-2592000S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1D' '.bucketoffset' '7' '.buckettimezone' 'Europe/Paris' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[950399999992,0],[863999999992,1],[691199999992,3],[604799999992,8],[518399999992,9],[345599999992,10,4],[259199999992,2],[172799999992,0.0,0.0,1]]}]"#,
            ),
        ),
        (
            r"1 UNBUCKETIZE.CALENDAR",
            Err(r"UNBUCKETIZE.CALENDAR can only operate on Geo Time Series instances."),
        ),
        (
            r"[ 1 ] UNBUCKETIZE.CALENDAR",
            Err(r"UNBUCKETIZE.CALENDAR can only operate on Geo Time Series instances."),
        ),
        (r"UNBUCKETIZE.CALENDAR", Err(r"Empty stack.")),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT1.2.3S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(r"Invalid ISO8601 duration"),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT1.xS' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r#"Parsing of sub second precision part of duration has failed. tried to parse: x (For input string: "0.x")"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-2592000S' '.bucketoffset' '-5097600000000' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 5443199999999. Can not add a negative period. Period is -1 times P1MT-2592000S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-2592000S' '.bucketoffset' '-2592000000000' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[2937599999999,0],[2851199999999,1],[2764799999999,3],[2678399999999,8],[2678399999999,9],[2505599999999,10,4],[2505599999999,2],[2419199999999,0.0,0.0,1]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-719H-60M' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times P1MT-719H-60M plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-720H' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times P1MT-720H plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-43200M' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times P1MT-43200M plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-2592000,5S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times P1MT-2592000.500S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-2592000,005S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times P1MT-2592000.005S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1MT-2591999,5S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1999999,10,4],[-86398500001,2],[-86399000001,0.0,0.0,1]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1M-4W-2DT0,5S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[1999999,10,4],[-86398500001,2],[-86399000001,0.0,0.0,1]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1Y-12M' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times P1Y-12M plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1M-31DT0,5S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times P1M-31DT0.500S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1M-31DT1,25S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times P1M-31DT1.250S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT0,000S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"Period is negative from timestamp 0. Can not add a negative period. Period is 4 times PT0S plus 0 time unit(s).",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT1S' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Ok(
                r#"[{"c":"s","l":{"k":"v"},"a":{},"la":0,"v":[[3999999,10,4],[2999999,2],[1999999,0.0,0.0,1]]}]"#,
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT1S' '.bucketoffset' '' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(
                r"UNBUCKETIZE.CALENDAR expects input GTS to be duration-bucketized. This information is stored in attributes.",
            ),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT1S' '.bucketoffset' ' 1' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(r#"For input string: " 1""#),
        ),
        (
            r"[ @S bucketizer.last 3 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'PT1S' '.bucketoffset' '99999999999999999999' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
            Err(r#"For input string: "99999999999999999999""#),
        ),
    ]);
}

/// Additions overflowing the date arithmetic fail; Joda and `chrono` disagree on the limits
/// and messages (Warp 10: `Value cannot fit in an int: 11000000000`, `Value 1000001970 for
/// year must be in the range [-292275054,292278993]`).
#[test]
fn calendar_overflows() {
    for script in [
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE bucketizer.last 10 'P1000000000Y' 0 ] BUCKETIZE.CALENDAR",
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE bucketizer.last 10 1 0 ] BUCKETIZE 0 GET { '.bucketduration' 'P1000000000Y' '.bucketoffset' '0' '.buckettimezone' 'UTC' } SETATTRIBUTES UNBUCKETIZE.CALENDAR",
    ] {
        assert!(
            stack_json(script)
                .expect_err("stack_json should fail")
                .contains("overflow"),
            "{script}"
        );
    }
}
