// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the datastore functions (`FETCH`, `FIND`, `UPDATE`, `META`, `DELETE`…), on a datastore in
//! memory. Warp 10 has no datastore a script can reach here (its clients are not exposed), so the
//! expectations are the behaviour the functions document, read from the Java sources.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::datastore::{Datastore, DeleteRequest, FetchRequest, MemoryDatastore, Selector};
use crate::encoder::{EncodedValue, GtsEncoder, wrapper_to_encoder};
use crate::error::datastore::DatastoreError;
use crate::gts::{GeoTimeSerie, Metadata, NO_ELEVATION, NO_LOCATION};
use crate::stack::Stack;
use crate::tests::test_stack;
use crate::thrift::GtsWrapper;
use crate::value::Value;

/// The series the tests find in the datastore.
const SETUP: &str = "\
NEWGTS 'cpu' RENAME { 'host' 'a' } RELABEL 1 NaN NaN NaN 10 ADDVALUE 2 NaN NaN NaN 20 ADDVALUE 3 NaN NaN NaN 30 ADDVALUE 'tok' UPDATE \
NEWGTS 'cpu' RENAME { 'host' 'b' } RELABEL 1 NaN NaN NaN 1.5 ADDVALUE 2 NaN NaN NaN 2.5 ADDVALUE 'tok' UPDATE \
NEWGTS 'mem' RENAME { 'host' 'a' 'dc' 'x' } RELABEL 1 NaN NaN NaN 'one' ADDVALUE 2 NaN NaN NaN 'two' ADDVALUE 'tok' UPDATE \
NEWGTS 'up' RENAME { 'host' 'a' } RELABEL 1 NaN NaN NaN T ADDVALUE 'tok' UPDATE ";

/// A stack on a datastore that holds [`SETUP`].
fn stack() -> Stack {
    let mut stack = test_stack();
    stack.set_datastore(Some(Arc::new(MemoryDatastore::new())));
    stack.exec_checked(SETUP).expect("the setup should succeed");
    stack
}

/// The stack left by `script`, top first, as JSON, or the error.
fn stack_json(script: &str) -> Result<String, String> {
    let mut stack = stack();
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
    for (script, expected) in cases {
        match (expected, stack_json(script)) {
            (Ok(json), Ok(actual)) if actual == *json => {}
            (Err(message), Err(actual)) if actual.contains(message) => {}
            (expected, actual) => failures.push(format!(
                "{script}\n  expected {expected:?}\n  actual   {actual:?}"
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
fn fetch() {
    check(&[
        (
            r"[ 'tok' 'cpu' { 'host' 'a' } 3 5 ] FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[2,20],[1,10]]}]]"#),
        ),
        (r"[ 'tok' 'cpu' { } 3 5 ] FETCH SIZE", Ok(r"[2]")),
        (
            r"[ 'tok' 'cpu' { 'host' '~a|b' } 3 2 ] FETCH",
            Ok(
                r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[2,20]]},{"c":"cpu","l":{"host":"b"},"a":{},"la":0,"v":[[2,2.5]]}]]"#,
            ),
        ),
        (
            r"[ 'tok' '~.*' { } '1970-01-01T00:00:00.000002Z' '1970-01-01T00:00:00.000003Z' ] FETCH",
            Ok(
                r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[2,20]]},{"c":"cpu","l":{"host":"b"},"a":{},"la":0,"v":[[2,2.5]]},{"c":"mem","l":{"host":"a","dc":"x"},"a":{},"la":0,"v":[[2,"two"]]}]]"#,
            ),
        ),
        (
            r"[ 'tok' 'cpu' { 'host' 'a' } 'now' 'now' 1 ] FETCH",
            Err(r"FETCH expects a list with 5 elements."),
        ),
        (
            r"[ 'tok' 'cpu' { 'host' 'a' } 3 ] FETCH",
            Err(r"FETCH expects a list with 5 elements."),
        ),
        (
            r"[ 'tok' 'cpu' { 'host' 'a' } 'x' 5 ] FETCH",
            Err(
                r"FETCH expects 'start' and 'end' to be Strings or 'end' and 'timespan' to be Longs.",
            ),
        ),
        (
            r"[ 'tok' 'cpu' { 'host' 'a' } 3 5 ] FETCHLONG",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[2,20],[1,10]]}]]"#),
        ),
        (
            r"[ 'tok' 'cpu' { 'host' 'a' } 3 5 ] FETCHDOUBLE",
            Ok(r"[[]]"),
        ),
        (r"[ 'tok' 'mem' { } 3 5 ] FETCHDOUBLE", Ok(r"[[]]")),
        (
            r"[ 'tok' 'mem' { } 3 5 ] FETCHSTRING",
            Ok(
                r#"[[{"c":"mem","l":{"host":"a","dc":"x"},"a":{},"la":0,"v":[[2,"two"],[1,"one"]]}]]"#,
            ),
        ),
        (r"[ 'tok' 'mem' { } 3 5 ] FETCHBOOLEAN", Ok(r"[[]]")),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'count' 2 } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[2,20]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' -2 } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[2,20]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' -2 'count' 1 } FETCH",
            Err(r"FETCH cannot be given both 'count' and negative 'timespan'."),
        ),
        (
            r"{ 'token' 'tok' 'selectors' [ 'cpu{host=a}' 'mem{}' ] 'end' 3 'timespan' 5 } FETCH SIZE",
            Ok(r"[2]"),
        ),
        (
            r"{ 'token' 'tok' 'class' 'cpu' 'labels' { 'host' 'b' } 'start' 1 'end' 3 } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"b"},"a":{},"la":0,"v":[[2,2.5],[1,1.5]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'class' 'cpu' 'labels' { 'host' 'b' } 'start' 1 'timespan' 2 } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"b"},"a":{},"la":0,"v":[[2,2.5],[1,1.5]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'count' 3 'skip' 1 'step' 2 } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[2,20]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'count' 3 'timestep' 2 } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[1,10]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'start' 2 'end' 2 'boundary' 1 } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[2,20],[1,10]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'start' 2 'end' 2 'boundary.pre' 1 'boundary.post' 0 } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[2,20],[1,10]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=z}' 'end' 3 'count' 2 } FETCH",
            Ok(r"[[]]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=z}' 'end' 3 'count' 2 'keepempty' T } FETCH",
            Ok(r"[[]]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'gskip' 1 } FETCH SIZE",
            Ok(r"[1]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'gcount' 1 } FETCH SIZE",
            Ok(r"[1]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'type' 'long' } FETCH SIZE",
            Ok(r"[1]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'type' 'DOUBLE' } FETCH SIZE",
            Ok(r"[1]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'type' 'bad' } FETCH",
            Err(r"FETCH Invalid value for parameter 'type'."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'merge' F 'priority' [ 'a' ] 'sample' 1.0 } FETCH",
            Err(r"FETCH does not support the 'sample' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'priority' 1 } FETCH",
            Err(r"FETCH Invalid type for parameter 'priority', expected a LIST."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'sample' 0.5 } FETCH",
            Err(r"FETCH does not support the 'sample' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'sample' 2.0 } FETCH",
            Err(r"FETCH Parameter 'sample' must be in the range ( 0.0, 1.0 ]."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'sample' 1 } FETCH",
            Err(r"FETCH Invalid type for parameter 'sample'."),
        ),
        (
            r"{ 'selector' 'cpu{}' 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Missing 'token' parameter"),
        ),
        (
            r"{ 'token' 'tok' 'end' 3 'count' 2 } FETCH",
            Err(
                r"FETCH Missing 'metaset', 'gts', 'selector', 'selectors' or 'class' and 'labels' parameters.",
            ),
        ),
        (
            r"{ 'token' 'tok' 'selectors' 'x' 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Invalid parameter 'selectors'"),
        ),
        (
            r"{ 'token' 'tok' 'selectors' [ 'bad' ] 'end' 3 'count' 2 } FETCH",
            Err(r"PARSESELECTOR could not parse input. ( invalid syntax for selector.)"),
        ),
        (
            r"{ 'token' 'tok' 'class' 'cpu' 'labels' 1 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH label selector must be a map."),
        ),
        (
            r"{ 'token' 'tok' 'class' 'cpu' 'labels' { 1 2 } 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH keys and values of label selector must be STRINGs."),
        ),
        (
            r"{ 'token' 'tok' 'class' 1 'labels' { } 'end' 3 'count' 2 } FETCH",
            Ok(r"[[]]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'count' 2 } FETCH",
            Err(r"FETCH given invalid parameters. (Missing either 'start' or 'end' parameter.)"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 } FETCH",
            Err(
                r"FETCH given invalid parameters. (Invalid time range specification: 'count' is mandatory if 'start' and 'timespan' are not specified.)",
            ),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 'x' } FETCH",
            Err(r"class java.lang.String cannot be cast to class java.lang.Long"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'start' 1 'end' 3 'timespan' 5 } FETCH",
            Err(
                r"FETCH given invalid parameters. (Invalid time range specification: 'start', 'end' and 'timespan' cannot all be defined. Only 2 out of those 3 parameters should be defined.)",
            ),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'metaset' 'x' 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH does not support the 'metaset' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'gts' [ ] 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH does not support the 'gts' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'extra' [ ] 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH does not support the 'extra' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'typeattr' 'x' 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH does not support the 'typeattr' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'wtimestamp' T 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH does not support the 'wtimestamp' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'ttl' T 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH does not support the 'ttl' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'ttl' T 'wtimestamp' T 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Only one of 'wtimestamp' and 'ttl' can be set to true."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'showuuid' T 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH does not support the 'showuuid' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'encoders' T 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH does not support the 'encoders' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'active.after' 1 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH does not support the 'active.after' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'quiet.after' 'x' 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Invalid type for parameter 'quiet.after'."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'boundary' 'x' 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Invalid type for parameter 'boundary'."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'skip' -1 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Parameter 'skip' must be >= 0."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'step' 0 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Parameter 'step' must be >= 1."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'timestep' 0 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Parameter 'timestep' must be >= 1."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'gskip' -1 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Parameter 'gskip' must be >= 0."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'gcount' 'x' 'end' 3 'count' 2 } FETCH",
            Err(r"FETCH Invalid type for parameter 'gcount'."),
        ),
        (
            r"[ 'tok' 'cpu' { } 3 ] FETCH",
            Err(r"FETCH expects a list with 5 elements."),
        ),
        (
            r"[ 'tok' 'cpu' { } 3 5 ] FETCHX",
            Err(r"Unknown function 'FETCHX'"),
        ),
        (
            r"1 FETCH",
            Err(r"FETCH expects a map or a list as parameter."),
        ),
        (r"FETCH", Err(r"Empty stack.")),
        (
            r"'tok' FETCH",
            Err(r"FETCH expects a map or a list as parameter."),
        ),
    ]);
}

#[test]
fn find_and_delete() {
    check(&[
        (
            r"[ 'tok' 'cpu' { } ] FIND",
            Ok(
                r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[]},{"c":"cpu","l":{"host":"b"},"a":{},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"[ 'tok' 'cpu' { 'host' 'a' } ] FIND",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[]}]]"#),
        ),
        (r"[ 'tok' '~.*' { 'host' '~a' } ] FIND SIZE", Ok(r"[3]")),
        (r"[ 'tok' '~.*' { } T ] FIND SIZE", Ok(r"[4]")),
        (r"[ 'tok' '~.*' { } 'x' ] FIND SIZE", Ok(r"[4]")),
        (
            r"[ 'tok' '~.*' { } ] FINDSETS",
            Ok(r#"[{},{"host":["a","b"],"dc":["x"]},["cpu","mem","up"]]"#),
        ),
        (
            r"[ 'tok' 'cpu' { 'host' 'zzz' } ] FINDSETS",
            Ok(r"[{},{},[]]"),
        ),
        (r"'tok' 'cpu' { } FIND SIZE", Ok(r"[2]")),
        (
            r"'tok' '~.*' { } FINDSETS",
            Ok(r#"[{},{"host":["a","b"],"dc":["x"]},["cpu","mem","up"]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' } FIND SIZE",
            Ok(r"[2]"),
        ),
        (
            r"{ 'token' 'tok' 'class' 'cpu' 'labels' { 'host' 'b' } } FIND",
            Ok(r#"[[{"c":"cpu","l":{"host":"b"},"a":{},"la":0,"v":[]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' '~.*{}' 'gskip' 1 'gcount' 2 } FIND SIZE",
            Ok(r"[2]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' '~.*{}' 'gskip' 1 'gcount' 2 } FINDSETS",
            Ok(r#"[{},{"host":["a","b"],"dc":["x"]},["cpu","mem"]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'active.after' 1 } FIND",
            Err(r"FIND does not support the 'active.after' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'quiet.after' 1 } FIND",
            Err(r"FIND does not support the 'quiet.after' parameter."),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' 'gskip' -1 } FIND",
            Err(r"FIND Parameter 'gskip' must be >= 0."),
        ),
        (
            r"{ 'token' 'tok' 'selectors' [ 'cpu{}' 'mem{}' ] } FIND",
            Err(r"FIND does not support the 'selectors' parameter."),
        ),
        (r"{ 'token' 'tok' } FIND", Err(r"Empty stack.")),
        (
            r"{ 'token' 'tok' 'selector' 'bad' } FIND",
            Err(r"PARSESELECTOR could not parse input. ( invalid syntax for selector.)"),
        ),
        (
            r"[ 'tok' 'cpu' ] FIND",
            Err(r"FIND expects 3 or 4 parameters."),
        ),
        (
            r"[ 'tok' 'cpu' { } T 1 ] FIND",
            Err(r"FIND expects 3 or 4 parameters."),
        ),
        (
            r"[ 'tok' 'cpu' { } ] FINDSTATS",
            Ok(
                r#"[{"gts.estimate":2,"classes.estimate":1,"labelnames.estimate":1,"labelvalues.estimate":2,"per.label.value.estimate":{"host":2},"error.rate":0.0}]"#,
            ),
        ),
        (
            r"[ 'tok' '~.*' { } ] FINDSTATS",
            Ok(
                r#"[{"gts.estimate":4,"classes.estimate":3,"labelnames.estimate":2,"labelvalues.estimate":3,"per.label.value.estimate":{"host":2,"dc":1},"error.rate":0.0}]"#,
            ),
        ),
        (
            r"'tok' '~.*' { 'host' 'a' } FINDSTATS",
            Ok(
                r#"[{"gts.estimate":3,"classes.estimate":3,"labelnames.estimate":2,"labelvalues.estimate":2,"per.label.value.estimate":{"host":1,"dc":1},"error.rate":0.0}]"#,
            ),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{}' } FINDSTATS",
            Ok(
                r#"[{"gts.estimate":2,"classes.estimate":1,"labelnames.estimate":1,"labelvalues.estimate":2,"per.label.value.estimate":{"host":2},"error.rate":0.0}]"#,
            ),
        ),
        (
            r"[ 'tok' 'cpu' ] FINDSTATS",
            Err(r"FINDSTATS expects 3 parameters."),
        ),
        (
            r"[ 'tok' 'cpu' { } T ] FINDSTATS",
            Err(r"FINDSTATS expects 3 parameters."),
        ),
        (r"1 FIND", Err(r"FIND expects a list or map of parameters.")),
        (
            r"'x' FIND",
            Err(r"FIND expects a list or map of parameters."),
        ),
        (r"FIND", Err(r"Empty stack.")),
        (
            r"[ 'tok' 'cpu' 1 ] FIND",
            Err(r"FIND label selector must be a map."),
        ),
        (
            r"[ 'tok' 1 { } ] FIND",
            Err(r"FIND Class selector must be a string."),
        ),
        (
            r"[ 1 'cpu' { } ] FIND",
            Err(r"FIND Token must be a string."),
        ),
        (
            r"[ 'tok' 'cpu' { 1 2 } ] FIND",
            Err(r"FIND keys and values of label selector must be STRINGs."),
        ),
        (
            r"[ 'tok' 'cpu' { } ] FINDSTATS 'gts.estimate' GET",
            Ok(r"[2]"),
        ),
        (
            r"[ 'tok' 'cpu' { } ] FINDSTATS 'per.label.value.estimate' GET",
            Ok(r#"[{"host":2}]"#),
        ),
        (
            r"NEWGTS 'new' RENAME 1 NaN NaN NaN 7 ADDVALUE 'tok' UPDATE [ 'tok' 'new' { } 5 5 ] FETCH",
            Ok(r#"[[{"c":"new","l":{},"a":{},"la":0,"v":[[1,7]]}]]"#),
        ),
        (
            r"NEWGTS 'new' RENAME { 'k' 'v' } RELABEL 1 NaN NaN NaN 7 ADDVALUE 'tok' UPDATE [ 'tok' 'new' { 'k' 'v' } 5 5 ] FETCH",
            Ok(r#"[[{"c":"new","l":{"k":"v"},"a":{},"la":0,"v":[[1,7]]}]]"#),
        ),
        (
            r"[ NEWGTS 'n1' RENAME 1 NaN NaN NaN 7 ADDVALUE NEWGTS 'n2' RENAME 1 NaN NaN NaN 8 ADDVALUE ] 'tok' UPDATE [ 'tok' '~n.*' { } 5 5 ] FETCH SIZE",
            Ok(r"[2]"),
        ),
        (
            r"NEWGTS 'new' RENAME 'tok' UPDATE [ 'tok' 'new' { } 5 5 ] FETCH SIZE",
            Ok(r"[0]"),
        ),
        (r"[ ] 'tok' UPDATE", Ok(r"[]")),
        (
            r"NEWGTS 1 NaN NaN NaN 7 ADDVALUE 'tok' UPDATE",
            Err(r"UPDATE can only update Geo Time Series which have a non empty name."),
        ),
        (
            r"NEWGTS '' RENAME 1 NaN NaN NaN 7 ADDVALUE 'tok' UPDATE",
            Err(r"UPDATE can only update Geo Time Series which have a non empty name."),
        ),
        (
            r"NEWGTS 1 NaN NaN NaN 7 ADDVALUE 'tok' UPDATE",
            Err(r"UPDATE can only update Geo Time Series which have a non empty name."),
        ),
        (
            r"NEWGTS 'x' RENAME 1 NaN NaN NaN 7 ADDVALUE '' RENAME 'tok' UPDATE",
            Err(r"UPDATE can only update Geo Time Series which have a non empty name."),
        ),
        (
            r"NEWGTS 'x' 1 NaN NaN NaN 7 ADDVALUE 'tok' UPDATE",
            Err(r"ADDVALUE operates on a single Geo Time Series or GTS Encoder."),
        ),
        (
            r"1 'tok' UPDATE",
            Err(r"UPDATE can only operate on Geo Time Series, encoders or a list thereof."),
        ),
        (
            r"[ 1 ] 'tok' UPDATE",
            Err(r"UPDATE can only operate on Geo Time Series, encoders or a list thereof."),
        ),
        (
            r"NEWGTS 'x' RENAME 1 NaN NaN NaN 7 ADDVALUE 1 UPDATE",
            Err(r"UPDATE expects a token on top of the stack."),
        ),
        (r"UPDATE", Err(r"Empty stack.")),
        (r"'tok' UPDATE", Err(r"Empty stack.")),
        (
            r"NEWGTS 'cpu' RENAME { 'host' 'a' } RELABEL { 'unit' 'pct' } SETATTRIBUTES 'tok' META [ 'tok' 'cpu' { 'host' 'a' } ] FIND 0 GET ATTRIBUTES",
            Ok(r#"[{"unit":"pct"}]"#),
        ),
        (
            r"NEWGTS 'cpu' RENAME { 'host' 'a' } RELABEL { 'unit' 'pct' } SETATTRIBUTES 'tok' META NEWGTS 'cpu' RENAME { 'host' 'a' } RELABEL { 'zone' 'z' } SETATTRIBUTES 'tok' META [ 'tok' 'cpu' { 'host' 'a' } ] FIND 0 GET ATTRIBUTES",
            Ok(r#"[{"zone":"z"}]"#),
        ),
        (
            r"NEWGTS 'cpu' RENAME { 'host' 'a' } RELABEL { 'unit' 'pct' } SETATTRIBUTES 'tok' META NEWGTS 'cpu' RENAME { 'host' 'a' } RELABEL { 'zone' 'z' } SETATTRIBUTES 'tok' METADIFF [ 'tok' 'cpu' { 'host' 'a' } ] FIND 0 GET ATTRIBUTES",
            Ok(r#"[{"unit":"pct","zone":"z"}]"#),
        ),
        (
            r"NEWGTS 'cpu' RENAME { 'host' 'a' } RELABEL { 'unit' 'pct' } SETATTRIBUTES 'tok' META NEWGTS 'cpu' RENAME { 'host' 'a' } RELABEL { 'unit' '' } SETATTRIBUTES 'tok' METADIFF [ 'tok' 'cpu' { 'host' 'a' } ] FIND 0 GET ATTRIBUTES",
            Ok(r#"[{"unit":"pct"}]"#),
        ),
        (
            r"[ NEWGTS 'm1' RENAME { 'a' '1' } SETATTRIBUTES NEWGTS 'm2' RENAME { 'b' '2' } SETATTRIBUTES ] 'tok' META [ 'tok' '~m.*' { } ] FINDSETS",
            Ok(r#"[{"a":["1"],"b":["2"]},{"host":["a"],"dc":["x"]},["m1","m2","mem"]]"#),
        ),
        (r"[ ] 'tok' META", Ok(r"[]")),
        (
            r"NEWGTS 'tok' META",
            Err(
                r"META can only set attributes of Geo Time Series or GTS Encoders which have a non empty name.",
            ),
        ),
        (r"NEWGTS 'cpu' RENAME 1 META", Err(r"META expects a token.")),
        (
            r"1 'tok' META",
            Err(r"META can only operate on Geo Time Series, GTS Encoders or a list thereof"),
        ),
        (
            r"[ 1 ] 'tok' META",
            Err(r"META can only operate on Geo Time Series, GTS Encoders or a list thereof."),
        ),
        (
            r"NEWGTS 'cpu' 'tok' META",
            Err(r"META can only operate on Geo Time Series, GTS Encoders or a list thereof"),
        ),
        (
            r"NEWGTS 'cpu' RENAME 1 'tok' METADIFF",
            Err(r"METADIFF can only operate on Geo Time Series, GTS Encoders or a list thereof"),
        ),
        (r"'tok' META", Err(r"Empty stack.")),
        (r"META", Err(r"Empty stack.")),
        (
            r"'tok' '~.*{}' NULL NULL 0 DELETE",
            Err(
                r"DELETE expected at most 0 Geo Time Series to be deleted but 1 would have been deleted instead.",
            ),
        ),
        (
            r"'tok' '~.*{}' NULL NULL NULL DELETE",
            Ok(
                r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[]},{"c":"cpu","l":{"host":"b"},"a":{},"la":0,"v":[]},{"c":"mem","l":{"host":"a","dc":"x"},"a":{},"la":0,"v":[]},{"c":"up","l":{"host":"a"},"a":{},"la":0,"v":[]}]]"#,
            ),
        ),
        (
            r"'tok' 'cpu{host=a}' NULL NULL NULL DELETE",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[]}]]"#),
        ),
        (
            r"'tok' 'cpu{host=a}' NULL NULL 5 DELETE [ 'tok' 'cpu' { 'host' 'a' } 5 5 ] FETCH SIZE",
            Ok(r"[0,1]"),
        ),
        (
            r"'tok' 'cpu{host=a}' 2 3 5 DELETE [ 'tok' 'cpu' { 'host' 'a' } 5 5 ] FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[1,10]]}],1]"#),
        ),
        (
            r"'tok' 'cpu{host=a}' '1970-01-01T00:00:00.000001Z' '1970-01-01T00:00:00.000002Z' 5 DELETE [ 'tok' 'cpu' { 'host' 'a' } 5 5 ] FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30]]}],1]"#),
        ),
        (
            r"'tok' 'cpu{}' NULL NULL 1 DELETE",
            Err(
                r"DELETE expected at most 1 Geo Time Series to be deleted but 2 would have been deleted instead.",
            ),
        ),
        (
            r"'tok' 'cpu{}' NULL NULL 2 DELETE [ 'tok' 'cpu' { } ] FIND SIZE",
            Ok(r"[0,2]"),
        ),
        (r"'tok' 'cpu{host=zzz}' NULL NULL 5 DELETE", Ok(r"[0]")),
        (
            r"'tok' 'cpu{host=a}' 2 NULL 5 DELETE",
            Err(r"DELETE expects both start and end timestamps MUST be NULL if one of them is."),
        ),
        (
            r"'tok' 'cpu{host=a}' NULL 2 5 DELETE",
            Err(r"DELETE expects both start and end timestamps MUST be NULL if one of them is."),
        ),
        (
            r"'tok' 'cpu{host=a}' 1.0 2 5 DELETE",
            Err(r"DELETE expects the start timestamp to be a Long, a String or NULL."),
        ),
        (
            r"'tok' 'cpu{host=a}' 1 2.0 5 DELETE",
            Err(r"DELETE expects the end timestamp to be a Long, a String or NULL."),
        ),
        (
            r"'tok' 'cpu{host=a}' 1 2 'x' DELETE",
            Err(r"DELETE expects a LONG count on top of the stack or NULL to indicate a dry run."),
        ),
        (
            r"'tok' 1 NULL NULL 5 DELETE",
            Err(r"DELETE expects a Geo Time Series selector below the time parameters."),
        ),
        (
            r"1 'cpu{}' NULL NULL 5 DELETE",
            Err(r"DELETE expects a token below the selector."),
        ),
        (
            r"'tok' 'bad' NULL NULL 5 DELETE",
            Err(r"PARSESELECTOR could not parse input. ( invalid syntax for selector.)"),
        ),
        (
            r"'tok' 'cpu{}' 1 DELETE",
            Err(r"Invalid format for parameter 'end'."),
        ),
        (r"DELETE", Err(r"Empty stack.")),
    ]);
}

#[test]
fn encoders_and_time_ranges() {
    check(&[
        (
            r"NEWENCODER 'enc' RENAME { 'k' 'v' } RELABEL 1 NaN NaN NaN 5 ADDVALUE 2 NaN NaN NaN 6.5 ADDVALUE 'tok' UPDATE [ 'tok' 'enc' { } 5 5 ] FETCH",
            Ok(r#"[[{"c":"enc","l":{"k":"v"},"a":{},"la":0,"v":[[2,6.5],[1,5.0]]}]]"#),
        ),
        (
            r"NEWENCODER 'enc' RENAME 'tok' UPDATE [ 'tok' 'enc' { } 5 5 ] FETCH SIZE",
            Ok(r"[0]"),
        ),
        (
            r"NEWENCODER 1 NaN NaN NaN 5 ADDVALUE 'tok' UPDATE",
            Err(r"UPDATE can only update encoders which have a non empty name."),
        ),
        (
            r"[ NEWENCODER 'e1' RENAME 1 NaN NaN NaN 'a' ADDVALUE NEWGTS 'g1' RENAME 1 NaN NaN NaN T ADDVALUE ] 'tok' UPDATE [ 'tok' '~[eg]1' { } 5 5 ] FETCH",
            Ok(
                r#"[[{"c":"e1","l":{},"a":{},"la":0,"v":[[1,"a"]]},{"c":"g1","l":{},"a":{},"la":0,"v":[[1,true]]}]]"#,
            ),
        ),
        (
            r"NEWENCODER 'cpu' RENAME { 'host' 'a' } RELABEL { 'e' '1' } SETATTRIBUTES 'tok' META [ 'tok' 'cpu' { 'host' 'a' } ] FIND 0 GET ATTRIBUTES",
            Ok(r#"[{"e":"1"}]"#),
        ),
        (
            r"[ NEWENCODER 'enc' RENAME { 'k' 'v' } RELABEL { 'x' 'y' } SETATTRIBUTES NEWGTS 'g' RENAME { 'z' '1' } SETATTRIBUTES ] 'tok' META [ 'tok' '~.*' { 'k' '~.*' } ] FINDSETS",
            Ok(r#"[{"x":["y"]},{"k":["v"]},["enc"]]"#),
        ),
        (
            r"NEWENCODER 'cpu' RENAME 'tok' UPDATE NEWENCODER 'cpu' RENAME { 'host' 'a' } RELABEL 5 NaN NaN NaN 1 ADDVALUE 'tok' UPDATE [ 'tok' 'cpu' { 'host' 'a' } 9 9 ] FETCH SIZE",
            Ok(r"[1]"),
        ),
        (
            r"[ 'tok' 'cpu' { 'host' 'a' } 3 'P1D' ] FETCH SIZE",
            Err(
                r"FETCH expects 'start' and 'end' to be Strings or 'end' and 'timespan' to be Longs.",
            ),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' 'P1D' } FETCH SIZE",
            Ok(r"[1]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'start' 3 'timespan' 'P1D' } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' 'PT0S' } FETCH",
            Ok(r"[[]]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' '2' } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[2,20]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' 'x' } FETCH",
            Err(
                r#"FETCH given invalid parameters. (Invalid format for parameter 'timespan'. (For input string: "x"))"#,
            ),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' 'P1M' } FETCH",
            Err(
                r"FETCH given invalid parameters. (Invalid format for parameter 'timespan'. (no support for ambiguous durations containing years or months, please convert those to days.))",
            ),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' '' } FETCH",
            Err(r"FETCH given invalid parameters. (Parameter 'timespan' is empty.)"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' 1.5 } FETCH",
            Err(r"FETCH given invalid parameters. (Invalid format for parameter 'timespan'.)"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'timespan' -9223372036854775808 } FETCH SIZE",
            Ok(r"[1]"),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'start' 3 'end' 1 } FETCH",
            Ok(r#"[[{"c":"cpu","l":{"host":"a"},"a":{},"la":0,"v":[[3,30],[2,20],[1,10]]}]]"#),
        ),
        (
            r"{ 'token' 'tok' 'selector' 'cpu{host=a}' 'end' 3 'count' 1 'type' 'long' 'skip' 0 'step' 1 'timestep' 1 'boundary.pre' 0 'boundary.post' 0 'gskip' 0 'gcount' 5 'keepempty' F 'merge' T 'priority' [ ] } FETCH SIZE",
            Ok(r"[1]"),
        ),
        (
            r"NEWENCODER 'nul' RENAME 1 NaN NaN NaN NULL ADDVALUE 2 NaN NaN NaN 3 ADDVALUE 'tok' UPDATE [ 'tok' 'nul' { } 5 5 ] FETCH",
            Err(
                r"ADDVALUE expects a LONG, DOUBLE, STRING, byte array or BOOLEAN value or a Geo Time Series or ENCODER.",
            ),
        ),
        (
            r"NEWENCODER 'byt' RENAME 1 NaN NaN NaN 'AB' ->BYTES ADDVALUE 'tok' UPDATE [ 'tok' 'byt' { } 5 5 ] FETCH",
            Err(r"->BYTES operates on a String."),
        ),
        (
            r"NEWENCODER 'mix' RENAME 1 NaN NaN NaN 5 ADDVALUE 2 NaN NaN NaN 6.5 ADDVALUE 3 NaN NaN NaN 'x' ADDVALUE 4 NaN NaN NaN 7 ADDVALUE 'tok' UPDATE [ 'tok' 'mix' { } 5 5 ] FETCHDOUBLE",
            Ok(r#"[[{"c":"mix","l":{},"a":{},"la":0,"v":[[2,6.5]]}]]"#),
        ),
        (
            r"NEWENCODER 'mix' RENAME 1 NaN NaN NaN 5 ADDVALUE 2 NaN NaN NaN 6.5 ADDVALUE 3 NaN NaN NaN 'x' ADDVALUE 4 NaN NaN NaN 7 ADDVALUE 'tok' UPDATE [ 'tok' 'mix' { } 5 5 ] FETCHLONG",
            Ok(r#"[[{"c":"mix","l":{},"a":{},"la":0,"v":[[4,7],[1,5]]}]]"#),
        ),
    ]);
}

#[test]
fn the_functions_need_a_datastore() {
    for script in [
        "[ 'tok' 'cpu' { } 3 5 ] FETCH",
        "[ 'tok' 'cpu' { } ] FIND",
        "[ 'tok' 'cpu' { } ] FINDSETS",
        "[ 'tok' 'cpu' { } ] FINDSTATS",
        "NEWGTS 'x' RENAME 1 NaN NaN NaN 1 ADDVALUE 'tok' UPDATE",
        "NEWGTS 'x' RENAME 'tok' META",
        "NEWGTS 'x' RENAME 'tok' METADIFF",
        "'tok' 'cpu{}' NULL NULL 5 DELETE",
    ] {
        let mut stack = test_stack();
        let err = stack
            .exec_checked(script)
            .expect_err("no datastore")
            .root_cause()
            .to_string();
        assert!(
            err.ends_with("needs a datastore, and none is connected to this runtime."),
            "{script}: {err}"
        );
    }
}

#[test]
fn a_delete_marker_is_not_written() {
    let mut encoder = GtsEncoder::new(0);
    encoder.metadata.name = Some("marked".to_owned());
    for (tick, value) in [(1, EncodedValue::Null), (2, EncodedValue::Long(3))] {
        encoder.add_value(
            tick,
            crate::gts::NO_LOCATION,
            crate::gts::NO_ELEVATION,
            &value,
        );
    }
    let mut stack = stack();
    stack.push(Value::Encoder(Rc::new(RefCell::new(encoder))));
    stack
        .exec_checked("'tok' UPDATE [ 'tok' 'marked' { } 5 5 ] FETCH ->JSON")
        .expect("the update should succeed");
    let Ok(Value::String(json)) = stack.pop() else {
        panic!("a string should be left");
    };
    assert_eq!(
        &*json,
        r#"[{"c":"marked","l":{},"a":{},"la":0,"v":[[2,3]]}]"#
    );
}

#[test]
fn more_parameters() {
    check(&[
        (
            "{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'type' 'string' } FETCH SIZE",
            Ok("[0]"),
        ),
        (
            "{ 'token' 'tok' 'selector' 'mem{}' 'end' 3 'count' 2 'type' 'String' } FETCH SIZE",
            Ok("[1]"),
        ),
        (
            "{ 'token' 'tok' 'selector' 'up{}' 'end' 3 'count' 2 'type' 'BOOLEAN' } FETCH SIZE",
            Ok("[1]"),
        ),
        (
            "{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'boundary.pre' 'x' } FETCH",
            Err("Invalid type for parameter 'boundary.pre'."),
        ),
        (
            "{ 'token' 'tok' 'selector' 'cpu{}' 'end' 3 'count' 2 'boundary.post' 'x' } FETCH",
            Err("Invalid type for parameter 'boundary.post'."),
        ),
        (
            "{ 'token' 'tok' 'selector' 'cpu{host~(}' 'end' 3 'count' 2 } FETCH",
            Err("Opening parenthesis without closing parenthesis"),
        ),
        ("'cpu' { } FIND", Err("Empty stack.")),
        ("1 FINDSTATS", Err("FINDSTATS expects 3 parameters.")),
        (
            "{ 'token' 'tok' 'selector' 'cpu{}' 'gcount' -1 } FIND",
            Err("Parameter 'gcount' must be >= 0."),
        ),
        (
            "NEWENCODER 'b' RENAME 1 NaN NaN NaN T ADDVALUE 'tok' UPDATE [ 'tok' 'b' { } 5 5 ] FETCH SIZE",
            Ok("[1]"),
        ),
        (
            "[ 'tok' 'cpu' { 'host' 'a' } 3 5 ] FETCH 0 GET 'tok' UPDATE",
            Err("UPDATE can only update Geo Time Series which have been renamed."),
        ),
        ("NULL DELETE", Err("Empty stack.")),
        ("NULL NULL DELETE", Err("Empty stack.")),
        ("NULL NULL NULL DELETE", Err("Empty stack.")),
        ("'cpu{}' NULL NULL NULL DELETE", Err("Empty stack.")),
    ]);
}

/// A datastore that fails, which finds one series when asked to.
#[derive(Debug)]
struct Failing {
    finds: bool,
}

fn down(operation: &'static str) -> crate::error::Error {
    DatastoreError::Backend {
        operation,
        message: "down".to_owned(),
    }
    .into()
}

impl Datastore for Failing {
    fn find(&self, _: &[Selector]) -> crate::error::Result<Vec<Metadata>> {
        if self.finds {
            Ok(vec![Metadata {
                name: Some("cpu".to_owned()),
                ..Metadata::default()
            }])
        } else {
            Err(down("find"))
        }
    }

    fn fetch(&self, _: &FetchRequest) -> crate::error::Result<Vec<GeoTimeSerie>> {
        Err(down("fetch"))
    }

    fn update(&self, _: &[GeoTimeSerie]) -> crate::error::Result<()> {
        Err(down("update"))
    }

    fn update_metadata(&self, _: &[Metadata], _: bool) -> crate::error::Result<()> {
        Err(down("update_metadata"))
    }

    fn delete(&self, _: &DeleteRequest) -> crate::error::Result<Vec<Metadata>> {
        Err(down("delete"))
    }
}

#[test]
fn a_datastore_that_fails_fails_the_functions() {
    for (finds, script, operation) in [
        (false, "[ 'tok' 'cpu' { } 3 5 ] FETCH", "fetch"),
        (false, "[ 'tok' 'cpu' { } ] FIND", "find"),
        (false, "[ 'tok' 'cpu' { } ] FINDSETS", "find"),
        (false, "[ 'tok' 'cpu' { } ] FINDSTATS", "find"),
        (
            false,
            "NEWGTS 'x' RENAME 1 NaN NaN NaN 1 ADDVALUE 'tok' UPDATE",
            "update",
        ),
        (false, "NEWGTS 'x' RENAME 'tok' META", "update_metadata"),
        (false, "NEWGTS 'x' RENAME 'tok' METADIFF", "update_metadata"),
        (false, "'tok' 'cpu{}' NULL NULL NULL DELETE", "find"),
        (false, "'tok' 'cpu{}' NULL NULL 5 DELETE", "find"),
        (true, "'tok' 'cpu{}' NULL NULL 5 DELETE", "delete"),
    ] {
        let mut stack = test_stack();
        stack.set_datastore(Some(Arc::new(Failing { finds })));
        let err = stack
            .exec_checked(script)
            .expect_err("the datastore is down");
        assert_eq!(
            err.root_cause().to_string(),
            format!("{operation} failed on the datastore: down"),
            "{script}"
        );
    }
}

#[test]
fn an_encoder_that_cannot_be_read_is_not_written() {
    let mut content = GtsEncoder::new(0);
    content.add_value(1, NO_LOCATION, NO_ELEVATION, &EncodedValue::Long(1));
    let mut bytes = content.bytes().to_vec();
    bytes.extend_from_slice(&[0xff; 8]);
    let wrapper = GtsWrapper {
        metadata: Some(Metadata {
            name: Some("broken".to_owned()),
            ..Metadata::default()
        }),
        base: Some(0),
        encoded: Some(bytes),
        count: Some(3),
        ..GtsWrapper::default()
    };
    let encoder = wrapper_to_encoder(&wrapper).expect("an encoder");
    let mut stack = stack();
    stack.push(Value::Encoder(Rc::new(RefCell::new(encoder))));
    let err = stack
        .exec_checked("'tok' UPDATE")
        .expect_err("the content is garbage");
    assert!(!err.root_cause().to_string().is_empty(), "{err}");
}
