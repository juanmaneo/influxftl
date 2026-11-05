// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the Geo Time Series functions.

use indexmap::IndexMap;

use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_stack, assert_top, exec, test_stack, top};
use crate::value::Value;

/// A `LONG` series `g{l=v}` with three datapoints, the second one located and elevated.
const SERIES: &str = "NEWGTS 'g' RENAME { 'l' 'v' } RELABEL \
    10 NaN NaN NaN 1 ADDVALUE 20 48.0 2.0 35 2 ADDVALUE 30 NaN NaN 5 3 ADDVALUE";

fn longs(values: &[i64]) -> Value {
    Value::list(values.iter().copied().map(Value::Long).collect())
}

fn doubles(values: &[f64]) -> Value {
    Value::list(values.iter().copied().map(Value::Double).collect())
}

fn string_map(entries: &[(&str, &str)]) -> Value {
    Value::map(
        entries
            .iter()
            .map(|(k, v)| (Value::from(*k), Value::from(*v)))
            .collect::<IndexMap<_, _>>(),
    )
}

/// Run `script` with a Rust-built series on the stack and return the top of the stack.
fn top_with(gts: GeoTimeSerie, script: &str) -> Value {
    let mut stack = test_stack();
    stack.push(Value::Gts(gts.shared()));
    stack.exec_checked(script).unwrap();
    stack.pop().unwrap()
}

#[test]
fn newgts_is_an_empty_series_named_with_the_empty_string() {
    assert_top("NEWGTS NAME", "");
    assert_top("NEWGTS TYPEOF", "GTS");
    assert_top("NEWGTS SIZE", 0);
    assert_top("NEWGTS LABELS", Value::map(IndexMap::new()));
    assert_top("NEWGTS ATTRIBUTES", Value::map(IndexMap::new()));
    assert_top("NEWGTS LASTACTIVITY", 0);
    assert_top("NEWGTS TOSTRING", "");
    assert_top(
        "NEWGTS ->JSON",
        "{\"c\":\"\",\"l\":{},\"a\":{},\"la\":0,\"v\":[]}",
    );
}

#[test]
fn addvalue_takes_a_tick_a_location_an_elevation_and_a_value() {
    assert_top(&format!("{SERIES} TICKLIST"), longs(&[10, 20, 30]));
    assert_top(&format!("{SERIES} VALUES"), longs(&[1, 2, 3]));
    assert_stack(
        &format!("{SERIES} LOCATIONS"),
        &[
            doubles(&[f64::NAN, 47.999_999_988_824_13, f64::NAN]),
            doubles(&[f64::NAN, 1.999_999_964_609_742_2, f64::NAN]),
        ],
    );
    assert_top(
        &format!("{SERIES} ELEVATIONS"),
        Value::list(vec![
            Value::Double(f64::NAN),
            Value::Long(35),
            Value::Long(5),
        ]),
    );
    // A `NaN` latitude or longitude means no location; a `LONG` elevation is kept as is and a
    // `DOUBLE` one truncated.
    assert_top(
        "NEWGTS 1 NaN 2.0 NaN 'a' ADDVALUE 2 1.0 NaN 3.9 'b' ADDVALUE ELEVATIONS",
        Value::list(vec![Value::Double(f64::NAN), Value::Long(3)]),
    );
    assert_top(
        "NEWGTS 1 NaN 2.0 NaN 'a' ADDVALUE 2 1.0 NaN 3.9 'b' ADDVALUE LOCATIONS DROP",
        doubles(&[f64::NAN, f64::NAN]),
    );
    // Every scalar type is accepted; the first value fixes the type and the others convert.
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN true ADDVALUE \
         4 NaN NaN NaN '4e1' ADDVALUE 5 NaN NaN NaN 'abc' 'UTF-8' ->BYTES ADDVALUE VALUES",
        doubles(&[1.5, 2.0, 1.0, 40.0, 0.0]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 'abc' 'UTF-8' ->BYTES ADDVALUE 2 NaN NaN NaN false ADDVALUE VALUES",
        Value::list(vec![Value::from("abc"), Value::from("F")]),
    );
    // Counters are `Number`s for the argument check but not stored (`AtomicLong` is none of
    // the value types `GTSHelper.setValue` converts), which still fixes the type to BOOLEAN.
    assert_top(
        "NEWGTS 1 NaN NaN NaN COUNTER ADDVALUE 2 NaN NaN NaN 7 ADDVALUE VALUES",
        Value::list(vec![Value::Boolean(true)]),
    );
    // The series is modified in place and left on the stack.
    let mut stack = exec("NEWGTS 'g' STORE $g 1 NaN NaN NaN 1 ADDVALUE $g SIZE");
    assert_eq!(stack.pop().unwrap(), Value::Long(1));
    let returned = stack.pop().unwrap();
    stack.exec_checked("$g").unwrap();
    assert!(returned.same_object(&stack.pop().unwrap()));
    // Datapoints keep their insertion order, `SETVALUE` overwrites an existing tick.
    assert_top(
        "NEWGTS 2 NaN NaN NaN 'b' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'c' ADDVALUE \
         2 NaN NaN NaN 'd' SETVALUE VALUES",
        Value::list(vec![Value::from("d"), Value::from("a"), Value::from("c")]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' SETVALUE 2 NaN NaN NaN 'c' SETVALUE \
         TICKLIST",
        longs(&[1, 2]),
    );
}

#[test]
fn addvalue_accepts_a_datapoint_list() {
    assert_top(
        "NEWGTS [ 10 48.0 2.0 35 1 ] ADDVALUE [ 20 NaN 2.0 2 ] ADDVALUE [ 30 7 3 ] ADDVALUE \
         [ 40 4 ] ADDVALUE [ 5 ] ADDVALUE [ 60 NaN 6 ] SETVALUE TOSTRING",
        "10/47.99999998882413:1.9999999646097422/35 {}{} 1\r\n=20// 2\r\n=30//7 3\r\n=40// 4\r\n=0// 5\r\n=60// 6\r\n",
    );
    assert_top(
        "NEWGTS [ 1 'x' ] ADDVALUE [ 1 'y' ] SETVALUE [ 2.9 1.0 2.0 3.9 'z' ] ADDVALUE ELEVATIONS",
        Value::list(vec![Value::Double(f64::NAN), Value::Long(3)]),
    );
    assert_top(
        "NEWGTS [ 1 'x' ] ADDVALUE [ 1 'y' ] SETVALUE [ 2.9 1.0 2.0 3.9 'z' ] ADDVALUE TICKLIST",
        longs(&[1, 2]),
    );
    assert_fails(
        "NEWGTS [ ] ADDVALUE",
        "Expected a list of 1 to 5 elements, got 0",
    );
    assert_fails(
        "NEWGTS [ 1 2 3 4 5 6 ] ADDVALUE",
        "Expected a list of 1 to 5 elements, got 6",
    );
    assert_fails(
        "NEWGTS [ 'tick' 1 ] ADDVALUE",
        "class java.lang.String cannot be cast to class java.lang.Number",
    );
    assert_fails(
        "NEWGTS [ 1 NaN 'lon' 1 ] ADDVALUE",
        "class java.lang.String cannot be cast to class java.lang.Number",
    );
    assert_fails(
        "NEWGTS [ 1 true 1 ] ADDVALUE",
        "class java.lang.Boolean cannot be cast to class java.lang.Number",
    );
    // Every element but the value must be a number, whatever the arity.
    for list in [
        "[ 'x' 1.0 2.0 3 1 ]",
        "[ 1 'x' 2.0 3 1 ]",
        "[ 1 1.0 'x' 3 1 ]",
        "[ 1 1.0 2.0 'x' 1 ]",
        "[ 'x' 1.0 2.0 1 ]",
        "[ 1 1.0 'x' 1 ]",
        "[ 'x' 3 1 ]",
        "[ 1 'x' 1 ]",
        "[ 'x' 1 ]",
    ] {
        assert_fails(
            &format!("NEWGTS {list} ADDVALUE"),
            "cannot be cast to class java.lang.Number",
        );
    }
    assert_fails(
        "NEWGTS [ NEWGTS 1 ] ADDVALUE",
        "class io.warp10.continuum.gts.GeoTimeSerie cannot be cast to class java.lang.Number",
    );
}

#[test]
fn setvalue_finds_the_tick_by_binary_search_on_a_sorted_series() {
    const SORTED: &str = "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE \
        3 NaN NaN NaN 'c' ADDVALUE 4 NaN NaN NaN 'd' ADDVALUE 5 NaN NaN NaN 'e' ADDVALUE";
    assert_top(
        &format!(
            "{SORTED} 1 NaN NaN NaN 'A' SETVALUE 5 NaN NaN NaN 'E' SETVALUE 6 NaN NaN NaN 'f' SETVALUE VALUES"
        ),
        Value::list(
            ["A", "b", "c", "d", "E", "f"]
                .into_iter()
                .map(Value::from)
                .collect(),
        ),
    );
    assert_top(
        &format!("{SORTED} RSORT 1 NaN NaN NaN 'A' SETVALUE 4 NaN NaN NaN 'D' SETVALUE VALUES"),
        Value::list(
            ["e", "D", "c", "b", "A"]
                .into_iter()
                .map(Value::from)
                .collect(),
        ),
    );
}

#[test]
fn functions_fail_on_an_empty_stack() {
    let cases: &[(&str, &[&str])] = &[
        (
            "ADDVALUE",
            &[
                "",
                "1",
                "NaN 1",
                "NaN NaN 1",
                "NaN NaN NaN 1",
                "1 NaN NaN NaN 1",
                "[ 1 1 ]",
            ],
        ),
        ("SETVALUE", &[""]),
        ("REMOVETICK", &["", "1"]),
        ("NAME", &[""]),
        ("RENAME", &["", "'n'"]),
        ("LABELS", &[""]),
        ("RELABEL", &["", "{ }"]),
        ("ATTRIBUTES", &[""]),
        ("SETATTRIBUTES", &["", "{ }"]),
        ("LASTACTIVITY", &[""]),
        ("TICKS", &[""]),
        ("TICKLIST", &[""]),
        ("VALUES", &[""]),
        ("LOCATIONS", &[""]),
        ("ELEVATIONS", &[""]),
        ("SORT", &[""]),
        ("RSORT", &[""]),
        ("FIRSTTICK", &[""]),
        ("LASTTICK", &[""]),
        ("CLONEEMPTY", &[""]),
        ("ATINDEX", &["", "0"]),
        ("ATTICK", &["", "0"]),
        ("TICKINDEX", &[""]),
        ("BUCKETSPAN", &[""]),
        ("BUCKETCOUNT", &[""]),
        ("LASTBUCKET", &[""]),
    ];
    for (name, prefixes) in cases {
        for prefix in *prefixes {
            assert_fails(&format!("{prefix} {name}"), "Empty stack.");
        }
    }
}

#[test]
fn addvalue_checks_its_arguments() {
    assert_fails(
        "NEWGTS 1 NaN NaN NaN { } ADDVALUE",
        "ADDVALUE expects a LONG, DOUBLE, STRING, byte array or BOOLEAN value or a Geo Time Series or ENCODER.",
    );
    assert_fails(
        "NEWGTS 1 NaN NaN NaN NEWGTS SETVALUE",
        "SETVALUE encountered an error while serializing the Geo Time Series.",
    );
    assert_fails(
        "NEWGTS 1 NaN NaN 'e' 1 ADDVALUE",
        "ADDVALUE expects the elevation to be numeric or NaN.",
    );
    assert_fails(
        "NEWGTS 1 NaN 'lon' NaN 1 ADDVALUE",
        "ADDVALUE expects the longitude to be numeric or NaN.",
    );
    assert_fails(
        "NEWGTS 1 'lat' NaN NaN 1 ADDVALUE",
        "ADDVALUE expects the latitude to be numeric or NaN.",
    );
    assert_fails(
        "NEWGTS 't' NaN NaN NaN 1 ADDVALUE",
        "ADDVALUE expects the tick to be numeric.",
    );
    assert_fails(
        "'gts' 1 NaN NaN NaN 1 ADDVALUE",
        "ADDVALUE operates on a single Geo Time Series or GTS Encoder.",
    );
    assert_fails(
        "'gts' [ 1 1 ] SETVALUE",
        "SETVALUE operates on a single Geo Time Series or GTS Encoder.",
    );
}

#[test]
fn removetick_drops_the_datapoints_at_the_given_ticks() {
    assert_top(
        &format!("{SERIES} 20 REMOVETICK TICKLIST"),
        longs(&[10, 30]),
    );
    assert_top(
        &format!("{SERIES} [ 10 30 ] REMOVETICK TOSTRING"),
        "20/47.99999998882413:1.9999999646097422/35 g{l=v}{} 2\r\n",
    );
    assert_top(
        &format!("{SERIES} [ 10 20 ] ->SET REMOVETICK VALUES"),
        longs(&[3]),
    );
    assert_top(&format!("{SERIES} [ ] REMOVETICK SIZE"), 3);
    // The result is a new series; the original is untouched.
    assert_stack(
        &format!("{SERIES} 'g' STORE $g 10 REMOVETICK SIZE $g SIZE"),
        &[Value::Long(2), Value::Long(3)],
    );
    assert_fails(
        &format!("{SERIES} 'x' REMOVETICK"),
        "REMOVETICK expects a tick (LONG) or a collection thereof on top of the stack.",
    );
    assert_fails(
        &format!("{SERIES} [ 10 'x' ] REMOVETICK"),
        "REMOVETICK expects a tick (LONG) or a collection thereof on top of the stack.",
    );
    assert_fails(
        &format!("{SERIES} [ 1.5 ] ->SET REMOVETICK"),
        "REMOVETICK expects a tick (LONG) or a collection thereof on top of the stack.",
    );
    assert_fails(
        "'g' 10 REMOVETICK",
        "REMOVETICK operates on a Geo Time Series.",
    );
}

#[test]
fn name_rename_and_lastactivity() {
    assert_top("NEWGTS 'gts.name' RENAME NAME", "gts.name");
    assert_top("NEWGTS 'a' RENAME '+b' RENAME NAME", "ab");
    // A list of series is renamed in place and pushed back.
    assert_top(
        "[ NEWGTS NEWGTS ] 'n' RENAME <% NAME %> false LMAP",
        Value::list(vec![Value::from("n"), Value::from("n")]),
    );
    assert_fails("NEWGTS 1 RENAME", "RENAME expects a STRING name.");
    assert_fails(
        "[ NEWGTS 1 ] 'n' RENAME",
        "RENAME expects a GeoTimeSeries, a GTSEncoder or a list thereof under the new name.",
    );
    assert_fails(
        "1 NAME",
        "NAME expects a Geo Time Series or encoder instance on top of the stack.",
    );
    assert_fails(
        "1 LASTACTIVITY",
        "LASTACTIVITY expects a Geo Time Series or encoder instance on top of the stack.",
    );
    let mut gts = GeoTimeSerie::new();
    gts.metadata.last_activity = 1_700_000_000_000;
    assert_eq!(
        top_with(gts, "LASTACTIVITY"),
        Value::Long(1_700_000_000_000_000)
    );
}

#[test]
fn relabel_and_labels() {
    assert_top(
        "NEWGTS { 'a' '1' 'b' '2' } RELABEL { 'a' '' 'c' '3' 'd' NULL } RELABEL LABELS",
        string_map(&[("b", "2"), ("c", "3")]),
    );
    // A `null` key discards the existing labels first.
    assert_top(
        "NEWGTS { 'a' '1' } RELABEL { NULL NULL 'b' '2' } RELABEL LABELS",
        string_map(&[("b", "2")]),
    );
    assert_top(
        "[ NEWGTS NEWGTS ] { 'a' '1' } RELABEL <% LABELS %> false LMAP",
        Value::list(vec![string_map(&[("a", "1")]), string_map(&[("a", "1")])]),
    );
    // `LABELS` is a copy: modifying it does not touch the series.
    assert_top(
        "NEWGTS 'g' STORE $g { 'a' '1' } RELABEL LABELS 'x' 'b' PUT DROP $g LABELS SIZE",
        1,
    );
    assert_fails(
        "NEWGTS 1 RELABEL",
        "RELABEL expects a map of labels as parameter.",
    );
    assert_fails(
        "NEWGTS { 1 'v' } RELABEL",
        "RELABEL keys and values MUST be STRING or NULL",
    );
    assert_fails(
        "NEWGTS { 'k' 1 } RELABEL",
        "RELABEL keys and values MUST be STRING or NULL",
    );
    assert_fails(
        "[ 1 ] { 'k' 'v' } RELABEL",
        "RELABEL expects a Geo Time Series, a GTSEncoder or a list thereof under the labels parameter.",
    );
    assert_fails(
        "1 LABELS",
        "LABELS expects a Geo Time Series or encoder instance on top of the stack.",
    );
}

#[test]
fn setattributes_and_attributes() {
    assert_top(
        "NEWGTS { 'a' '1' 'b' '2' } SETATTRIBUTES { 'a' '' 'c' '3' 'b' NULL 'd' '4' } SETATTRIBUTES \
         ATTRIBUTES",
        string_map(&[("c", "3"), ("d", "4")]),
    );
    assert_top(
        "NEWGTS { 'a' '1' } SETATTRIBUTES { NULL 'ignored' 'b' '2' } SETATTRIBUTES ATTRIBUTES",
        string_map(&[("b", "2")]),
    );
    assert_top(
        "[ NEWGTS NEWGTS ] { 'a' '1' } SETATTRIBUTES <% ATTRIBUTES %> false LMAP",
        Value::list(vec![string_map(&[("a", "1")]), string_map(&[("a", "1")])]),
    );
    assert_fails(
        "NEWGTS 1 SETATTRIBUTES",
        "SETATTRIBUTES expects a map of attributes as parameter.",
    );
    assert_fails(
        "NEWGTS { 1 'v' } SETATTRIBUTES",
        "SETATTRIBUTES attribute key and value MUST be of type String.",
    );
    assert_fails(
        "NEWGTS { 'k' 1 } SETATTRIBUTES",
        "SETATTRIBUTES attribute key and value MUST be of type String.",
    );
    // An empty map is applied before the entries are checked, like Warp 10.
    assert_top("[ ] { 'k' 1 } SETATTRIBUTES SIZE", 0);
    assert_fails(
        "'g' { 'k' 'v' } SETATTRIBUTES",
        "SETATTRIBUTES expects a GeoTimeSeries, a GTSEncoder or a list thereof under the attribute map.",
    );
    assert_fails(
        "1 ATTRIBUTES",
        "ATTRIBUTES expects a Geo Time Series or encoder instance on top of the stack.",
    );
}

#[test]
fn ticks_are_the_sorted_distinct_ticks() {
    assert_top(
        "NEWGTS 3 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 1 ADDVALUE TICKS",
        longs(&[1, 3]),
    );
    assert_top(
        &format!("[ {SERIES} NEWGTS 5 NaN NaN NaN 1 ADDVALUE ] TICKS"),
        longs(&[5, 10, 20, 30]),
    );
    assert_top("[ ] TICKS", longs(&[]));
    assert_fails(
        "1 TICKS",
        "TICKS expects a Geo Time Series or a list thereof on top of the stack.",
    );
    assert_fails(
        "[ NEWGTS 1 ] TICKS",
        "TICKS expects a Geo Time Series or a list thereof on top of the stack.",
    );
    // `TICKLIST` and `VALUES` keep the storage order and work on lists.
    assert_top(
        "[ NEWGTS 3 NaN NaN NaN 'c' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE NEWGTS ] TICKLIST",
        Value::list(vec![longs(&[3, 1]), longs(&[])]),
    );
    assert_top(
        "[ NEWGTS 3 NaN NaN NaN 'c' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE ] VALUES",
        Value::list(vec![Value::list(vec![Value::from("c"), Value::from("a")])]),
    );
    assert_fails(
        "[ 1 ] TICKLIST",
        "TICKLIST expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
    );
    assert_fails(
        "'g' VALUES",
        "VALUES expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
    );
    assert_fails(
        "1 LOCATIONS",
        "LOCATIONS expects a Geo Time Series on top of the stack.",
    );
    assert_top(
        &format!("[ {SERIES} NEWGTS ] ELEVATIONS"),
        Value::list(vec![
            Value::list(vec![
                Value::Double(f64::NAN),
                Value::Long(35),
                Value::Long(5),
            ]),
            Value::list(vec![]),
        ]),
    );
    assert_fails(
        "1 ELEVATIONS",
        "ELEVATIONS can only operate on Geo Time Series instances.",
    );
    // `GTSStackFunction` pushes the offending argument back before failing.
    let mut stack = test_stack();
    let error = stack.exec_checked("[ NEWGTS 1 ] ELEVATIONS").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("ELEVATIONS can only operate on Geo Time Series instances.")
    );
    assert_eq!(stack.depth(), 1);
    assert!(matches!(stack.pop().unwrap(), Value::List(_)));
}

#[test]
fn sort_and_rsort_order_the_datapoints_in_place() {
    const UNSORTED: &str = "NEWGTS 3 NaN NaN NaN 'c' ADDVALUE 1 1.0 2.0 7 'a' ADDVALUE \
        2 NaN NaN NaN 'b' ADDVALUE 1 NaN NaN NaN 'a2' ADDVALUE";
    // Warp 10's quicksort is not stable: `a2` lands before `a` (verified with Java).
    assert_top(
        &format!("{UNSORTED} SORT VALUES"),
        Value::list(vec![
            Value::from("a2"),
            Value::from("a"),
            Value::from("b"),
            Value::from("c"),
        ]),
    );
    assert_top(&format!("{UNSORTED} RSORT TICKLIST"), longs(&[3, 2, 1, 1]));
    assert_top(
        &format!("{UNSORTED} RSORT ELEVATIONS"),
        Value::list(vec![
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Long(7),
        ]),
    );
    assert_stack(
        &format!("{UNSORTED} 'g' STORE $g SORT DROP $g TICKLIST"),
        &[longs(&[1, 1, 2, 3])],
    );
    assert_top(
        &format!("[ {UNSORTED} {UNSORTED} RSORT ] SORT TICKLIST"),
        Value::list(vec![longs(&[1, 1, 2, 3]), longs(&[1, 1, 2, 3])]),
    );
    assert_fails(
        "1 SORT",
        "SORT can only operate on Geo Time Series instances.",
    );
    assert_fails(
        "1 SORT",
        "SORT can only operate on Geo Time Series instances.",
    );
    assert_fails(
        "[ 1 ] RSORT",
        "RSORT can only operate on Geo Time Series instances.",
    );
    // Every value type is sorted and reversed (an untyped series has nothing to reverse).
    for (values, sorted) in [
        ("1 2 3", longs(&[1, 2, 3])),
        ("1.5 2.5 3.5", doubles(&[1.5, 2.5, 3.5])),
        (
            "true false true",
            Value::list(vec![
                Value::Boolean(true),
                Value::Boolean(false),
                Value::Boolean(true),
            ]),
        ),
        (
            "'a' 'b' 'c'",
            Value::list(vec![Value::from("a"), Value::from("b"), Value::from("c")]),
        ),
    ] {
        let [v1, v2, v3]: [&str; 3] = values.split(' ').collect::<Vec<_>>().try_into().unwrap();
        let script = format!(
            "NEWGTS 3 NaN NaN NaN {v3} ADDVALUE 1 NaN NaN NaN {v1} ADDVALUE 2 NaN NaN NaN {v2} ADDVALUE"
        );
        assert_top(&format!("{script} SORT VALUES"), sorted.clone());
        let Value::List(list) = sorted else {
            panic!("not a list");
        };
        let mut reversed = list.borrow().clone();
        reversed.reverse();
        assert_top(
            &format!("{script} SORT RSORT VALUES"),
            Value::list(reversed),
        );
    }
    assert_top("NEWGTS SORT RSORT SORT SIZE", 0);
    // A typed series left empty by an ignored value has nothing to sort either.
    assert_top("NEWGTS 1 NaN NaN NaN COUNTER ADDVALUE SORT RSORT SIZE", 0);
}

#[test]
fn firsttick_and_lasttick() {
    assert_top(&format!("{SERIES} FIRSTTICK"), 10);
    assert_top(&format!("{SERIES} LASTTICK"), 30);
    // Unsorted series are scanned.
    assert_top(
        "NEWGTS 5 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE 9 NaN NaN NaN 1 ADDVALUE FIRSTTICK",
        2,
    );
    assert_top(
        "NEWGTS 5 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE 9 NaN NaN NaN 1 ADDVALUE LASTTICK",
        9,
    );
    assert_top(
        "NEWGTS 5 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE 9 NaN NaN NaN 1 ADDVALUE \
         RSORT FIRSTTICK",
        2,
    );
    // Over a list, the extreme of the series; an empty series spans everything.
    assert_top(
        &format!("[ {SERIES} NEWGTS 5 NaN NaN NaN 1 ADDVALUE ] FIRSTTICK"),
        5,
    );
    assert_top(
        &format!("[ NEWGTS 5 NaN NaN NaN 1 ADDVALUE {SERIES} ] LASTTICK"),
        30,
    );
    assert_top("NEWGTS FIRSTTICK", i64::MAX);
    assert_top("NEWGTS LASTTICK", i64::MIN);
    assert_top("[ ] FIRSTTICK", i64::MAX);
    assert_top("[ ] LASTTICK", i64::MIN);
    assert_fails(
        "1 FIRSTTICK",
        "FIRSTTICK expects a Geo Time Series or a list thereof on top of the stack.",
    );
    assert_fails(
        "[ NEWGTS 1 ] LASTTICK",
        "LASTTICK expects a Geo Time Series or a list thereof on top of the stack.",
    );
    // A bucketized series answers with its bucket boundaries.
    let mut gts = GeoTimeSerie::new();
    gts.bucketspan = 10;
    gts.bucketcount = 4;
    gts.lastbucket = 100;
    assert_eq!(top_with(gts.clone(), "FIRSTTICK"), Value::Long(70));
    assert_eq!(top_with(gts, "LASTTICK"), Value::Long(100));
}

#[test]
fn cloneempty_keeps_the_metadata_only() {
    assert_top(
        &format!("{SERIES} {{ 'a' '1' }} SETATTRIBUTES CLONEEMPTY TOSTRING"),
        "",
    );
    assert_stack(
        &format!("{SERIES} CLONEEMPTY DUP NAME SWAP DUP LABELS SWAP SIZE"),
        &[Value::from("g"), string_map(&[("l", "v")]), Value::Long(0)],
    );
    assert_top(
        &format!("[ {SERIES} NEWGTS ] CLONEEMPTY <% SIZE %> false LMAP"),
        longs(&[0, 0]),
    );
    assert_fails(
        "[ 'g' ] CLONEEMPTY",
        "CLONEEMPTY expects a GeoTimeSeries, a GTSEncoder or a list thereof on top of the stack.",
    );
    let mut gts = GeoTimeSerie::new();
    gts.bucketspan = 10;
    gts.bucketcount = 4;
    gts.lastbucket = 100;
    gts.set_value(100, NO_LOCATION, NO_ELEVATION, &Value::Long(1), false);
    assert_eq!(top_with(gts, "CLONEEMPTY BUCKETCOUNT"), Value::Long(4));
}

#[test]
fn atindex_and_attick_extract_a_datapoint() {
    assert_top(
        &format!("{SERIES} 1 ATINDEX"),
        Value::list(vec![
            Value::Long(20),
            Value::Double(47.999_999_988_824_13),
            Value::Double(1.999_999_964_609_742_2),
            Value::Long(35),
            Value::Long(2),
        ]),
    );
    assert_top(
        &format!("{SERIES} -3 ATINDEX"),
        Value::list(vec![
            Value::Long(10),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Long(1),
        ]),
    );
    assert_top(
        &format!("{SERIES} 2 ATINDEX"),
        Value::list(vec![
            Value::Long(30),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Long(5),
            Value::Long(3),
        ]),
    );
    assert_top(
        &format!("[ {SERIES} {SERIES} ] 2.7 ATINDEX <% 0 GET %> false LMAP"),
        longs(&[30, 30]),
    );
    assert_fails(&format!("{SERIES} 3 ATINDEX"), "Index out of bound, 3 >= 3");
    assert_fails(
        &format!("{SERIES} -4 ATINDEX"),
        "Index out of bound, -4 < -3",
    );
    assert_fails(
        &format!("{SERIES} 'x' ATINDEX"),
        "ATINDEX expects an index on top of the stack.",
    );
    assert_fails(
        "[ 'g' ] 0 ATINDEX",
        "ATINDEX expects a Geo Time Series instance or a list thereof under the index.",
    );
    // `ATTICK` sorts the series and answers `NaN`s and `null` for an absent tick.
    assert_top(
        "NEWGTS 3 NaN NaN NaN 'c' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE 1 ATTICK",
        Value::list(vec![
            Value::Long(1),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::from("a"),
        ]),
    );
    assert_top(
        "NEWGTS 3 NaN NaN NaN 'c' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE 2 ATTICK",
        Value::list(vec![
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Double(f64::NAN),
            Value::Null,
        ]),
    );
    assert_top(
        "NEWGTS 'g' STORE $g 3 NaN NaN NaN 'c' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE 1 ATTICK DROP \
         $g TICKLIST",
        longs(&[1, 3]),
    );
    assert_top("NEWGTS 1 ATTICK 4 GET", Value::Null);
    assert_top(
        &format!("[ {SERIES} NEWGTS ] 20.5 ATTICK <% 4 GET %> false LMAP"),
        Value::list(vec![Value::Long(2), Value::Null]),
    );
    assert_fails(&format!("{SERIES} 'x' ATTICK"), "ATTICK expects a tick.");
    assert_fails(
        "'g' 1 ATTICK",
        "ATTICK can only operate on Geo Time Series instances.",
    );
}

#[test]
fn tickindex_replaces_ticks_by_indexes() {
    assert_top(
        "NEWGTS 30 NaN NaN NaN 'c' ADDVALUE 10 NaN NaN NaN 'a' ADDVALUE TICKINDEX TICKLIST",
        longs(&[0, 1]),
    );
    assert_stack(
        &format!("{SERIES} 'g' STORE $g TICKINDEX TICKLIST $g TICKLIST"),
        &[longs(&[0, 1, 2]), longs(&[10, 20, 30])],
    );
    assert_top(
        &format!("[ {SERIES} ] TICKINDEX 0 GET VALUES"),
        longs(&[1, 2, 3]),
    );
    let mut gts = GeoTimeSerie::new();
    gts.bucketspan = 10;
    gts.bucketcount = 4;
    gts.lastbucket = 100;
    assert_eq!(top_with(gts, "TICKINDEX BUCKETSPAN"), Value::Long(0));
    let message = crate::tests::fail_msg("[ 1 ] TICKINDEX");
    assert!(
        message.contains("TICKINDEX caught an exception while converting encoder.")
            && message.contains(
                "TICKINDEX can only operate on Geo Time Series or GTS Encoder instances."
            ),
        "{message}"
    );
}

#[test]
fn bucket_parameters_are_zero_unless_bucketized() {
    assert_stack(
        &format!("{SERIES} DUP BUCKETSPAN SWAP DUP BUCKETCOUNT SWAP LASTBUCKET"),
        &[Value::Long(0), Value::Long(0), Value::Long(0)],
    );
    let mut gts = GeoTimeSerie::new();
    gts.bucketspan = 10;
    gts.bucketcount = 4;
    gts.lastbucket = 100;
    assert_eq!(top_with(gts.clone(), "BUCKETSPAN"), Value::Long(10));
    assert_eq!(top_with(gts.clone(), "BUCKETCOUNT"), Value::Long(4));
    assert_eq!(top_with(gts.clone(), "LASTBUCKET"), Value::Long(100));
    // All three parameters must be set for the series to count as bucketized.
    gts.lastbucket = 0;
    assert_eq!(top_with(gts, "BUCKETSPAN"), Value::Long(0));
    assert_fails(
        "1 BUCKETSPAN",
        "BUCKETSPAN expects a Geo Time Series on top of the stack.",
    );
    assert_fails(
        "1 BUCKETCOUNT",
        "BUCKETCOUNT expects a Geo Time Series on top of the stack.",
    );
    assert_fails(
        "1 LASTBUCKET",
        "LASTBUCKET expects a Geo Time Series on top of the stack.",
    );
}

#[test]
fn series_as_values() {
    assert_top(&format!("{SERIES} SIZE"), 3);
    // `CLONE` is a deep copy, pushed above the original.
    assert_stack(
        &format!("{SERIES} CLONE 40 NaN NaN NaN 4 ADDVALUE SIZE SWAP SIZE"),
        &[Value::Long(4), Value::Long(3)],
    );
    assert_top(
        &format!("{SERIES} {{ 'a' '1' }} SETATTRIBUTES TOSTRING"),
        "10// g{l=v}{a=1} 1\r\n=20/47.99999998882413:1.9999999646097422/35 2\r\n=30//5 3\r\n",
    );
    assert_top(
        &format!("{SERIES} {{ 'a' '1' }} SETATTRIBUTES ->JSON"),
        "{\"c\":\"g\",\"l\":{\"l\":\"v\"},\"a\":{\"a\":\"1\"},\"la\":0,\"v\":[[10,1],[20,47.99999998882413,1.9999999646097422,35,2],[30,5,3]]}",
    );
    assert_top(
        &format!("[ {SERIES} ] ->JSON"),
        "[{\"c\":\"g\",\"l\":{\"l\":\"v\"},\"a\":{},\"la\":0,\"v\":[[10,1],[20,47.99999998882413,1.9999999646097422,35,2],[30,5,3]]}]",
    );
    // Series are hashed by identity (sets, map keys); `==` on series is a GTS operator
    // (not ported yet) rather than an identity test.
    assert_stack(
        "NEWGTS 'a' STORE [ $a $a NEWGTS ] ->SET SIZE { $a 1 } $a GET",
        &[Value::Long(2), Value::Long(1)],
    );
    // The snapshot rebuilds the series (in the readable form on request); the string form
    // of the value is its `toString`.
    let snapshot = "NEWGTS 'g' RENAME {} 'v' 'l' PUT RELABEL {} '1' 'a' PUT SETATTRIBUTES \
        10 NaN NaN NaN 1 ADDVALUE 20 47.99999998882413 1.9999999646097422 35 2 ADDVALUE \
        30 NaN NaN 5 3 ADDVALUE ";
    assert_top(
        &format!("{SERIES} {{ 'a' '1' }} SETATTRIBUTES SNAPSHOT"),
        snapshot,
    );
    assert_top(
        &format!("{snapshot} TOSTRING"),
        "10// g{l=v}{a=1} 1\r\n=20/47.99999998882413:1.9999999646097422/35 2\r\n=30//5 3\r\n",
    );
    assert_top(
        "NEWGTS 'g h' RENAME 1 NaN NaN NaN 'x y' ADDVALUE SNAPSHOT",
        "NEWGTS 'g%20h' RENAME {} RELABEL {} SETATTRIBUTES 1 NaN NaN NaN 'x%20y' ADDVALUE ",
    );
    assert_top(
        "NEWGTS 'g h' RENAME { 'l' 'v' } RELABEL 1 NaN NaN NaN 'x y' ADDVALUE SNAPSHOTREADABLE",
        "NEWGTS 'g h' RENAME {\n'l' 'v' \n} RELABEL {\n} SETATTRIBUTES 1 NaN NaN NaN 'x y' ADDVALUE ",
    );
    assert_top(
        "[ NEWGTS 'g' RENAME ] SNAPSHOT",
        "[] NEWGTS 'g' RENAME {} RELABEL {} SETATTRIBUTES +! ",
    );
    // The elements of a series count as one more nesting level, like the items of a list.
    let nest = |levels: usize| {
        let mut nested = Value::Gts(GeoTimeSerie::new().shared());
        for _ in 0..levels {
            nested = Value::list(vec![nested]);
        }
        nested
    };
    let mut stack = test_stack();
    stack.push(nest(14));
    stack.exec_checked("SNAPSHOT").unwrap();
    assert!(stack.pop().unwrap().to_string().contains("NEWGTS"));
    let mut stack = test_stack();
    stack.push(nest(15));
    let error = stack.exec_checked("SNAPSHOT").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Recursive data structures exceeded 16 levels."),
        "{error}"
    );
    // The JSON size bound applies inside a series.
    assert_fails(
        "20 MAXJSON NEWGTS 'g' RENAME ->JSON",
        "Resulting JSON is too big.",
    );
    assert_eq!(
        top(&format!("{SERIES} 'g' STORE $g")).to_string(),
        "g{l=v}\n=10// 1\n=20/47.99999998882413:1.9999999646097422/35 2\n=30//5 3\n"
    );
    assert_eq!(
        exec(&format!("{SERIES} TYPEOF")).pop().unwrap(),
        Value::from("GTS")
    );
}

#[test]
fn makegts() {
    let json = |script: &str| top(&format!("{script} MAKEGTS ->JSON")).to_string();
    let meta = r#""c":"","l":{},"a":{},"la":0"#;
    for (script, values) in [
        (
            "[ 1 2 3 ] [ 10 20 ] [ 1 ] [ 5 6 7 ] [ 'a' 'b' ]",
            r#"[[1,9.999999990686774,0.9999999403953552,5,"a"],[2,6,"b"],[3,7,"b"]]"#,
        ),
        ("[ ] [ ] [ ] [ ] [ 'a' 'b' ]", r#"[[0,"a"],[1,"b"]]"#),
        ("[ ] [ ] [ ] [ ] [ ]", "[]"),
        ("[ 3 ] [ ] [ ] [ ] [ 1 2 3 ]", "[[3,1],[4,2],[5,3]]"),
        (
            "[ 1.5 2 ] [ 1 ] [ 2 ] [ 2.7 ] [ 1 2.5 'x' NULL ]",
            "[[1,0.9999999823048711,1.9999999646097422,2,1],[2,2],[3,0]]",
        ),
        (
            "[ 1 ] [ NaN ] [ 2 ] [ ] [ 1 ]",
            "[[1,-90.0,1.9999999646097422,1]]",
        ),
        ("[ 1 ] [ 1 ] [ 2 ] [ ] [ NEWGTS ]", "[]"),
        (
            "[ MAXLONG ] [ ] [ ] [ ] [ 1 2 ]",
            "[[9223372036854775807,1],[-9223372036854775808,2]]",
        ),
        ("[ 1 ] [ ] [ ] [ NaN ] [ 1 ]", "[[1,0,1]]"),
    ] {
        assert_eq!(
            json(script),
            format!(r#"{{{meta},"v":{values}}}"#),
            "{script}"
        );
    }
    for (script, message) in [
        (
            "[ ] [ ] [ ] [ 1 ] [ ]",
            "Index -1 out of bounds for length 0",
        ),
        (
            "[ 1 ] [ ] [ ] [ ] [ ]",
            "Index -1 out of bounds for length 0",
        ),
        (
            "[ 'x' ] [ ] [ ] [ ] [ 1 ]",
            "class java.lang.String cannot be cast to class java.lang.Number",
        ),
        (
            "[ ] [ 'x' ] [ 1 ] [ ] [ 1 ]",
            "class java.lang.String cannot be cast to class java.lang.Number",
        ),
        (
            "[ ] [ 1 ] [ 'x' ] [ ] [ 1 ]",
            "class java.lang.String cannot be cast to class java.lang.Number",
        ),
        (
            "[ ] [ ] [ ] [ 'x' ] [ 1 ]",
            "class java.lang.String cannot be cast to class java.lang.Number",
        ),
        ("[ ] [ ] [ ] [ NULL ] [ 1 ]", "NullPointerException"),
        ("[ NULL ] [ ] [ ] [ ] [ 1 ]", "NullPointerException"),
        ("[ ] [ NULL ] [ 1 ] [ ] [ 1 ]", "NullPointerException"),
        ("[ ] [ 1 ] [ NULL ] [ ] [ 1 ]", "NullPointerException"),
        (
            "[ ] [ ] [ ] [ ] 42",
            "MAKEGTS expects a list of values on top of the stack.",
        ),
        (
            "[ ] [ ] [ ] 42 [ ]",
            "MAKEGTS expects a list of elevations below the list of values.",
        ),
        (
            "[ ] [ ] 42 [ ] [ ]",
            "MAKEGTS expects a list of longitudes below the list of elevations.",
        ),
        (
            "[ ] 42 [ ] [ ] [ ]",
            "MAKEGTS expects a list of latitudes below the list of longitudes.",
        ),
        (
            "42 [ ] [ ] [ ] [ ]",
            "MAKEGTS expects a list of ticks below the list of latitudes.",
        ),
        ("[ ] [ ] [ ] [ ]", "Empty stack."),
    ] {
        assert_fails(&format!("{script} MAKEGTS"), message);
    }
}
