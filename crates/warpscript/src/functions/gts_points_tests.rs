// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the datapoint selection and ordering functions. The expected values were checked
//! against Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use indexmap::IndexMap;

use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_stack, assert_top, exec, test_stack, top};
use crate::value::Value;

/// A `LONG` series `g{l=v}` with three datapoints, the second one located and elevated.
const SERIES: &str = "NEWGTS 'g' RENAME { 'l' 'v' } RELABEL \
    10 NaN NaN NaN 1 ADDVALUE 20 48.0 2.0 35 2 ADDVALUE 30 NaN NaN 5 3 ADDVALUE";

/// A `STRING` series with duplicate ticks, out of order.
const DUPES: &str = "NEWGTS 'g' RENAME 3 NaN NaN NaN 'c' ADDVALUE 1 NaN NaN NaN 'a' ADDVALUE \
    3 NaN NaN NaN 'c2' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE 1 NaN NaN NaN 'a2' ADDVALUE \
    3 NaN NaN NaN 'c3' ADDVALUE";

/// A `LONG` series with runs of equal values.
const RUNS: &str = "NEWGTS 'g' RENAME 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE \
    3 NaN NaN NaN 1 ADDVALUE 4 NaN NaN NaN 2 ADDVALUE 5 NaN NaN NaN 2 ADDVALUE \
    6 NaN NaN NaN 1 ADDVALUE 7 NaN NaN NaN 1 ADDVALUE";

/// A `LONG` series with resets.
const RESETS: &str = "NEWGTS 'g' RENAME 1 NaN NaN NaN 5 ADDVALUE 2 NaN NaN NaN 7 ADDVALUE \
    3 NaN NaN NaN 2 ADDVALUE 4 NaN NaN NaN 3 ADDVALUE 5 NaN NaN NaN 1 ADDVALUE";

const THREE: &str = "[ SERIES 'b' RENAME SERIES 'a' RENAME { 'l' 'w' } RELABEL SERIES 'c' RENAME { 'l' 'a' } RELABEL ]";

const ATTRIBUTED: &str = "[ SERIES 'b' RENAME { 'x' '2' } SETATTRIBUTES SERIES 'a' RENAME { 'x' '1' } SETATTRIBUTES SERIES 'c' RENAME ]";

const DOUBLES: &str =
    "NEWGTS 1 NaN NaN NaN 2.5 ADDVALUE 2 NaN NaN NaN 1.5 ADDVALUE 3 NaN NaN NaN 3.0 ADDVALUE";

const STRINGS: &str =
    "NEWGTS 1 NaN NaN NaN 'b' ADDVALUE 2 NaN NaN NaN 'a' ADDVALUE 3 NaN NaN NaN 'c' ADDVALUE";

fn longs(values: &[i64]) -> Value {
    Value::list(values.iter().copied().map(Value::Long).collect())
}

fn doubles(values: &[f64]) -> Value {
    Value::list(values.iter().copied().map(Value::Double).collect())
}

fn strings(values: &[&str]) -> Value {
    Value::list(values.iter().copied().map(Value::from).collect())
}

fn booleans(values: &[bool]) -> Value {
    Value::list(values.iter().copied().map(Value::Boolean).collect())
}

fn string_map(entries: &[(&str, &str)]) -> Value {
    Value::map(
        entries
            .iter()
            .map(|(k, v)| (Value::from(*k), Value::from(*v)))
            .collect::<IndexMap<_, _>>(),
    )
}

/// `[ SERIES bucketizer.sum 100 10 0 ] BUCKETIZE 0 GET` as Warp 10 builds it (the location
/// and elevation of the lone datapoint of each bucket are kept).
fn bucketized() -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::with_buckets(100, 10, 10);
    gts.metadata.name = Some("g".into());
    gts.metadata.labels.insert("l".into(), "v".into());
    gts.set_value(30, NO_LOCATION, 5, &Value::Long(3), false);
    gts.set_value(
        20,
        crate::geoxp::to_geoxp_point(48.0, 2.0),
        35,
        &Value::Long(2),
        false,
    );
    gts.set_value(10, NO_LOCATION, NO_ELEVATION, &Value::Long(1), false);
    gts
}

/// Run `script` with `gts` stored under `$b` and return the stack contents, bottom first.
fn run_with(gts: GeoTimeSerie, script: &str) -> Vec<Value> {
    let mut stack = test_stack();
    stack.push(Value::Gts(gts.shared()));
    stack
        .exec_checked("'b' STORE")
        .expect("exec_checked should succeed");
    stack
        .exec_checked(script)
        .expect("exec_checked should succeed");
    stack.elements().to_vec()
}

#[test]
fn dedup_keeps_the_last_datapoint_of_each_tick() {
    assert_stack(
        &format!("{DUPES} DEDUP DUP TICKLIST SWAP VALUES"),
        &[longs(&[2, 1, 3]), strings(&["b", "a2", "c3"])],
    );
    // The result is a copy, not flagged as sorted (so `SORT` really sorts it).
    assert_top(&format!("{DUPES} 'g' STORE $g DEDUP DROP $g SIZE"), 6);
    assert_top(
        &format!("{DUPES} DEDUP SORT 3 NaN NaN NaN 'x' SETVALUE VALUES"),
        strings(&["a2", "b", "x"]),
    );
    assert_top(&format!("{SERIES} DEDUP TICKLIST"), longs(&[10, 20, 30]));
    assert_top("NEWGTS DEDUP SIZE", 0);
    assert_top("NEWGTS 1 NaN NaN NaN 1 ADDVALUE DEDUP SIZE", 1);
    assert_top(
        &format!("[ {DUPES} [ {DUPES} ] ] DEDUP <% TYPEOF %> false LMAP"),
        strings(&["GTS", "LIST"]),
    );
    assert_top(&format!("[ [ {DUPES} ] ] DEDUP 0 GET 0 GET SIZE"), 3);
    for script in ["[ 1 ] DEDUP", "1 DEDUP", "[ [ 1 ] ] DEDUP"] {
        assert_fails(
            script,
            "DEDUP expects a Geo Time Series instance or a list thereof under an optional macro or mapper.",
        );
    }
}

#[test]
fn valuededup_keeps_one_datapoint_per_value() {
    const ALTERNATING: &str = "NEWGTS 1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE \
        3 NaN NaN NaN 2 ADDVALUE 4 NaN NaN NaN 1 ADDVALUE 5 NaN NaN NaN 2 ADDVALUE";
    assert_stack(
        &format!("{ALTERNATING} true VALUEDEDUP DUP TICKLIST SWAP VALUES"),
        &[longs(&[2, 1]), longs(&[1, 2])],
    );
    assert_stack(
        &format!("{ALTERNATING} false VALUEDEDUP DUP TICKLIST SWAP VALUES"),
        &[longs(&[4, 5]), longs(&[1, 2])],
    );
    // The input is value-sorted in place.
    assert_stack(
        &format!("{DUPES} 'g' STORE $g true VALUEDEDUP DUP TICKLIST SWAP VALUES $g VALUES"),
        &[
            longs(&[1, 1, 2, 3, 3, 3]),
            strings(&["a", "a2", "b", "c", "c2", "c3"]),
            strings(&["a", "a2", "b", "c", "c2", "c3"]),
        ],
    );
    assert_top("NEWGTS true VALUEDEDUP SIZE", 0);
    assert_top(
        &format!("[ {ALTERNATING} NEWGTS ] true VALUEDEDUP <% SIZE %> false LMAP"),
        longs(&[2, 0]),
    );
    assert_fails(
        &format!("{SERIES} 'x' VALUEDEDUP"),
        "VALUEDEDUP expects a boolean on top of the stack.",
    );
    assert_fails(
        "1 true VALUEDEDUP",
        "VALUEDEDUP can only operate on Geo Time Series instances.",
    );
}

#[test]
fn compact_drops_repeated_datapoints() {
    assert_stack(
        &format!("{RUNS} COMPACT DUP TICKLIST SWAP VALUES"),
        &[longs(&[1, 4, 6, 7]), longs(&[1, 2, 1, 1])],
    );
    assert_stack(
        &format!("{RUNS} RANGECOMPACT DUP TICKLIST SWAP VALUES"),
        &[longs(&[1, 3, 4, 5, 6, 7]), longs(&[1, 1, 2, 2, 1, 1])],
    );
    assert_top(
        &format!("{RUNS} 8 NaN NaN NaN 1 ADDVALUE RANGECOMPACT TICKLIST"),
        longs(&[1, 3, 4, 5, 6, 8]),
    );
    assert_top(
        &format!("{RUNS} 8 NaN NaN NaN 1 ADDVALUE COMPACT TICKLIST"),
        longs(&[1, 4, 6, 8]),
    );
    // Up to two datapoints: the sorted copy.
    assert_top(
        "NEWGTS 2 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 1 ADDVALUE COMPACT TICKLIST",
        longs(&[1, 2]),
    );
    assert_top(
        "NEWGTS 3 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE COMPACT TICKLIST",
        longs(&[1, 3]),
    );
    // Location and elevation take part in the comparison; `NaN`s are equal values.
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 1.0 2.0 NaN 1 ADDVALUE 3 1.0 2.0 NaN 1 ADDVALUE \
         4 1.0 2.0 7 1 ADDVALUE 5 1.0 2.0 7 1 ADDVALUE COMPACT TICKLIST",
        longs(&[1, 2, 4, 5]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN NaN ADDVALUE 2 NaN NaN NaN NaN ADDVALUE 3 NaN NaN NaN 1.0 ADDVALUE COMPACT TICKLIST",
        longs(&[1, 3]),
    );
    assert_top(&format!("{RUNS} 'g' STORE $g COMPACT DROP $g SIZE"), 7);
    assert_top(
        &format!("[ {RUNS} NEWGTS ] COMPACT <% SIZE %> false LMAP"),
        longs(&[4, 0]),
    );
    assert_fails(
        "[ 1 ] COMPACT",
        "COMPACT can only operate on Geo Time Series instances.",
    );
    assert_fails(
        "1 RANGECOMPACT",
        "RANGECOMPACT can only operate on Geo Time Series instances.",
    );
}

#[test]
fn onlybuckets_removes_datapoints_off_the_buckets() {
    assert_eq!(
        run_with(
            bucketized(),
            "$b 25 NaN NaN NaN 9 ADDVALUE 40 NaN NaN NaN 4 ADDVALUE 35 NaN NaN NaN 8 ADDVALUE \
             50 NaN NaN NaN 5 ADDVALUE ONLYBUCKETS DUP TICKLIST SWAP VALUES"
        ),
        vec![longs(&[30, 20, 10, 50, 40]), longs(&[3, 2, 1, 5, 4])]
    );
    assert_eq!(
        run_with(
            bucketized(),
            "$b 25 NaN NaN NaN 9 ADDVALUE 35 NaN NaN NaN 8 ADDVALUE ONLYBUCKETS TICKLIST"
        ),
        vec![longs(&[30, 20, 10])]
    );
    // Beyond the last bucket or before the first one is off the buckets too.
    assert_eq!(
        run_with(
            bucketized(),
            "$b 105 NaN NaN NaN 9 ADDVALUE 110 NaN NaN NaN 8 ADDVALUE 0 NaN NaN NaN 7 ADDVALUE ONLYBUCKETS TICKLIST"
        ),
        vec![longs(&[30, 20, 10])]
    );
    // The sort flags of the input are kept.
    assert_eq!(
        run_with(
            bucketized(),
            "$b 25 NaN NaN NaN 9 ADDVALUE ONLYBUCKETS 20 NaN NaN NaN 7 SETVALUE VALUES"
        ),
        vec![longs(&[3, 7, 1])]
    );
    assert_eq!(
        run_with(bucketized(), "[ $b $b ] ONLYBUCKETS <% SIZE %> false LMAP"),
        vec![longs(&[3, 3])]
    );
    assert_top(
        &format!("{SERIES} ONLYBUCKETS TICKLIST"),
        longs(&[10, 20, 30]),
    );
    assert_fails(
        "1 ONLYBUCKETS",
        "ONLYBUCKETS can only operate on Geo Time Series instances.",
    );
}

#[test]
fn empty_and_nonempty_filter_lists() {
    assert_top(
        &format!(
            "[ {SERIES} NEWGTS 'e' RENAME [ NEWGTS 'f' RENAME {SERIES} ] 7 ] EMPTY <% NAME %> false LMAP"
        ),
        strings(&["e", "f"]),
    );
    assert_top(
        &format!(
            "[ {SERIES} NEWGTS 'e' RENAME [ NEWGTS 'f' RENAME {SERIES} 'h' RENAME ] 7 ] NONEMPTY <% NAME %> false LMAP"
        ),
        strings(&["g", "h"]),
    );
    // The same series objects are returned.
    let mut stack = exec(&format!("{SERIES} 'g' STORE [ $g ] NONEMPTY 0 GET $g"));
    let original = stack.pop().expect("pop should succeed");
    assert!(
        stack
            .pop()
            .expect("pop should succeed")
            .same_object(&original)
    );
    assert_fails("1 EMPTY", "EMPTY operates on a list of Geo Time Series.");
    assert_fails(
        "1 NONEMPTY",
        "NONEMPTY operates on a list of Geo Time Series.",
    );
    assert_fails(
        "[ [ 1 ] ] EMPTY",
        "EMPTY expects a list of Geo Time Series.",
    );
    assert_fails(
        "[ [ 1 ] ] NONEMPTY",
        "NONEMPTY expects a list of Geo Time Series.",
    );
}

#[test]
fn commonticks_keeps_the_shared_ticks() {
    assert_top(
        &format!("[ {SERIES} ] COMMONTICKS 0 GET TICKLIST"),
        longs(&[10, 20, 30]),
    );
    assert_top(
        &format!(
            "[ {SERIES} 5 TIMESHIFT {SERIES} 10 TIMESHIFT {SERIES} ] COMMONTICKS <% TICKLIST %> false LMAP"
        ),
        Value::list(vec![longs(&[]), longs(&[]), longs(&[])]),
    );
    // Duplicates of a common tick are all kept; the inputs are sorted in place.
    assert_top(
        &format!(
            "[ {SERIES} {SERIES} 10 NaN NaN NaN 7 ADDVALUE 25 NaN NaN NaN 8 ADDVALUE ] COMMONTICKS <% DUP TICKLIST SWAP VALUES 2 ->LIST %> false LMAP"
        ),
        Value::list(vec![
            Value::list(vec![longs(&[10, 20, 30]), longs(&[1, 2, 3])]),
            Value::list(vec![longs(&[10, 10, 20, 30]), longs(&[7, 1, 2, 3])]),
        ]),
    );
    assert_top(
        &format!(
            "[ {SERIES} 10 NaN NaN NaN 7 ADDVALUE {SERIES} ] COMMONTICKS <% TICKLIST %> false LMAP"
        ),
        Value::list(vec![longs(&[10, 10, 20, 30]), longs(&[10, 20, 30])]),
    );
    assert_top(
        &format!(
            "[ {SERIES} {SERIES} 20 REMOVETICK 15 NaN NaN NaN 0 ADDVALUE 25 NaN NaN NaN 0 ADDVALUE ] COMMONTICKS <% TICKLIST %> false LMAP"
        ),
        Value::list(vec![longs(&[10, 30]), longs(&[10, 30])]),
    );
    assert_top(
        &format!(
            "[ {SERIES} 20 REMOVETICK 25 NaN NaN NaN 0 ADDVALUE 35 NaN NaN NaN 0 ADDVALUE {SERIES} ] COMMONTICKS <% TICKLIST %> false LMAP"
        ),
        Value::list(vec![longs(&[10, 30]), longs(&[10, 30])]),
    );
    assert_top(
        &format!("[ {SERIES} 'g' STORE $g RSORT DROP $g $g ] COMMONTICKS DROP $g TICKLIST"),
        longs(&[10, 20, 30]),
    );
    // A series entirely before the leader is exhausted while catching up.
    assert_top(
        &format!("[ {SERIES} {SERIES} -9 TIMESHIFT ] COMMONTICKS <% SIZE %> false LMAP"),
        longs(&[0, 0]),
    );
    assert_top(
        &format!("[ {SERIES} NEWGTS ] COMMONTICKS <% SIZE %> false LMAP"),
        longs(&[0, 0]),
    );
    assert_top(
        &format!("[ NEWGTS {SERIES} ] COMMONTICKS <% SIZE %> false LMAP"),
        longs(&[0, 0]),
    );
    assert_fails("[ ] COMMONTICKS", "Index -1 out of bounds for length 0");
    assert_fails(
        &format!("[ {SERIES} 1 ] COMMONTICKS"),
        "COMMONTICKS operates on a list of GTS.",
    );
    assert_fails("1 COMMONTICKS", "COMMONTICKS operates on a list of GTS.");
}

#[test]
fn valuesort_orders_by_value_then_tick() {
    const MIXED: &str = "NEWGTS 3 NaN NaN NaN 2 ADDVALUE 1 NaN NaN NaN 2 ADDVALUE \
        2 NaN NaN NaN 1 ADDVALUE 4 NaN NaN NaN 3 ADDVALUE";
    assert_stack(
        &format!("{MIXED} VALUESORT DUP TICKLIST SWAP VALUES"),
        &[longs(&[2, 1, 3, 4]), longs(&[1, 2, 2, 3])],
    );
    assert_stack(
        &format!("{MIXED} RVALUESORT DUP TICKLIST SWAP VALUES"),
        &[longs(&[4, 3, 1, 2]), longs(&[3, 2, 2, 1])],
    );
    // `NaN`s compare with nothing and land where Warp 10's quicksort leaves them.
    assert_stack(
        "NEWGTS 1 NaN NaN NaN NaN ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE 3 NaN NaN NaN NaN ADDVALUE \
         4 NaN NaN NaN 0.5 ADDVALUE VALUESORT DUP TICKLIST SWAP VALUES",
        &[
            longs(&[3, 4, 1, 2]),
            doubles(&[f64::NAN, 0.5, f64::NAN, 1.0]),
        ],
    );
    assert_stack(
        "NEWGTS 1 NaN NaN NaN 'b' ADDVALUE 2 NaN NaN NaN 'a' ADDVALUE 3 NaN NaN NaN 'b' ADDVALUE \
         VALUESORT DUP TICKLIST SWAP VALUES",
        &[longs(&[2, 1, 3]), strings(&["a", "b", "b"])],
    );
    // Booleans: the `false` datapoints then the `true` ones.
    assert_stack(
        "NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE 3 NaN NaN NaN true ADDVALUE \
         VALUESORT DUP TICKLIST SWAP VALUES",
        &[longs(&[2, 1, 3]), booleans(&[false, true, true])],
    );
    assert_stack(
        "NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE 3 NaN NaN NaN true ADDVALUE \
         RVALUESORT DUP TICKLIST SWAP VALUES",
        &[longs(&[3, 1, 2]), booleans(&[true, true, false])],
    );
    assert_stack(
        "NEWGTS 3 NaN NaN NaN true ADDVALUE 1 NaN NaN NaN false ADDVALUE 2 NaN NaN NaN true ADDVALUE \
         5 NaN NaN NaN false ADDVALUE 4 NaN NaN NaN false ADDVALUE VALUESORT DUP TICKLIST SWAP VALUES",
        &[
            longs(&[1, 4, 5, 2, 3]),
            booleans(&[false, false, false, true, true]),
        ],
    );
    // An unsorted half is tick-sorted but read through its stale `reversed` flag, as in
    // Warp 10 (which additionally reads past the end of its arrays there).
    assert_stack(
        "NEWGTS 3 NaN NaN NaN true ADDVALUE 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN true ADDVALUE \
         VALUESORT TICKLIST",
        &[longs(&[3, 2, 1])],
    );
    // In place; the series is no longer flagged as sorted.
    assert_top(
        "NEWGTS 1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE 'g' STORE $g VALUESORT DROP $g TICKLIST",
        longs(&[2, 1]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE 3 NaN NaN NaN 1 ADDVALUE VALUESORT \
         1 NaN NaN NaN 9 SETVALUE VALUES",
        longs(&[1, 1, 9]),
    );
    assert_top("NEWGTS VALUESORT SIZE", 0);
    assert_top(&format!("{SERIES} 0 SHRINK VALUESORT SIZE"), 0);
    assert_top("NEWGTS 1 NaN NaN NaN COUNTER ADDVALUE VALUESORT SIZE", 0);
    assert_top(
        &format!("[ {MIXED} {MIXED} ] RVALUESORT <% TICKLIST %> false LMAP"),
        Value::list(vec![longs(&[4, 3, 1, 2]), longs(&[4, 3, 1, 2])]),
    );
    assert_fails(
        "1 VALUESORT",
        "VALUESORT can only operate on Geo Time Series instances.",
    );
    assert_fails(
        "1 RVALUESORT",
        "RVALUESORT can only operate on Geo Time Series instances.",
    );
}

#[test]
fn lastsort_orders_by_most_recent_value() {
    assert_top(
        &format!(
            "[ {SERIES} 'b' RENAME 5 TIMESHIFT {SERIES} 'a' RENAME NEWGTS 'z' RENAME \
             {SERIES} 'c' RENAME 1 NaN NaN NaN 9 ADDVALUE {SERIES} 'd' RENAME 40 NaN NaN NaN 3 ADDVALUE ] \
             LASTSORT <% NAME %> false LMAP"
        ),
        strings(&["d", "b", "a", "c", "z"]),
    );
    // Compared series are reverse sorted in place; the list is left on the stack.
    assert_stack(
        &format!(
            "[ {SERIES} 'b' RENAME 5 TIMESHIFT {SERIES} 'a' RENAME ] LASTSORT DUP SIZE SWAP 0 GET TICKLIST"
        ),
        &[Value::Long(2), longs(&[35, 25, 15])],
    );
    assert_top(
        "[ NEWGTS 'b' RENAME NEWGTS 'a' RENAME NEWGTS ] LASTSORT <% NAME %> false LMAP",
        strings(&["", "a", "b"]),
    );
    // Mixed types compare as doubles or as strings.
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 2.5 ADDVALUE 'a' RENAME NEWGTS 1 NaN NaN NaN 2 ADDVALUE 'b' RENAME \
         NEWGTS 1 NaN NaN NaN 'x' ADDVALUE 'c' RENAME NEWGTS 1 NaN NaN NaN true ADDVALUE 'd' RENAME \
         NEWGTS 1 NaN NaN NaN 10 ADDVALUE 'e' RENAME ] LASTSORT <% NAME %> false LMAP",
        strings(&["b", "a", "e", "d", "c"]),
    );
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 2 ADDVALUE 'a' RENAME NEWGTS 1 NaN NaN NaN 2.5 ADDVALUE 'b' RENAME ] \
         LASTSORT <% NAME %> false LMAP",
        strings(&["a", "b"]),
    );
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 'a' RENAME NEWGTS 1 NaN NaN NaN 0.5 ADDVALUE 'b' RENAME \
         NEWGTS 1 NaN NaN NaN 'y' ADDVALUE 'c' RENAME NEWGTS 1 NaN NaN NaN 'x' ADDVALUE 'd' RENAME ] \
         LASTSORT <% NAME %> false LMAP",
        strings(&["b", "a", "d", "c"]),
    );
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN false ADDVALUE 'a' RENAME NEWGTS 1 NaN NaN NaN true ADDVALUE 'b' RENAME \
         NEWGTS 2 NaN NaN NaN false ADDVALUE 'c' RENAME ] LASTSORT <% NAME %> false LMAP",
        strings(&["c", "a", "b"]),
    );
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 'a' RENAME NEWGTS ] LASTSORT <% NAME %> false LMAP",
        strings(&["a", ""]),
    );
    assert_fails(
        "[ 1 ] LASTSORT",
        "LASTSORT operates on a list of Geo Time Series.",
    );
    assert_fails(
        "1 LASTSORT",
        "LASTSORT operates on a list of Geo Time Series.",
    );
}

#[test]
fn metasort_orders_by_metadata() {
    assert_top(
        &format!(
            "[ {SERIES} 'b' RENAME {SERIES} 'a' RENAME {{ 'l' 'w' }} RELABEL \
             {SERIES} 'a' RENAME {{ 'k' 'z' }} RELABEL {SERIES} 'a' RENAME {{ 'a' 'z' }} SETATTRIBUTES \
             {SERIES} 'a' RENAME {{ NULL NULL }} RELABEL ] [ ] METASORT \
             <% DUP NAME SWAP DUP LABELS SWAP ATTRIBUTES 3 ->LIST %> false LMAP"
        ),
        Value::list(vec![
            Value::list(vec![Value::from("a"), string_map(&[]), string_map(&[])]),
            Value::list(vec![
                Value::from("a"),
                string_map(&[("l", "v"), ("k", "z")]),
                string_map(&[]),
            ]),
            Value::list(vec![
                Value::from("a"),
                string_map(&[("l", "v")]),
                string_map(&[("a", "z")]),
            ]),
            Value::list(vec![
                Value::from("a"),
                string_map(&[("l", "w")]),
                string_map(&[]),
            ]),
            Value::list(vec![
                Value::from("b"),
                string_map(&[("l", "v")]),
                string_map(&[]),
            ]),
        ]),
    );
    // Attributes, then their number, break the ties (the series are told apart by size).
    assert_top(
        &format!(
            "[ {SERIES} {{ 'a' '2' }} SETATTRIBUTES 1 SHRINK {SERIES} {{ 'a' '1' 'b' '1' }} SETATTRIBUTES 2 SHRINK \
             {SERIES} {{ 'a' '1' }} SETATTRIBUTES 3 SHRINK {SERIES} {{ 'c' '1' }} SETATTRIBUTES 0 SHRINK \
             {SERIES} 40 NaN NaN NaN 4 ADDVALUE ] [ ] METASORT <% SIZE %> false LMAP"
        ),
        longs(&[4, 3, 2, 1, 0]),
    );
    // More labels sort after the same labels.
    assert_top(
        &format!(
            "[ {SERIES} {{ 'm' '1' }} RELABEL 1 SHRINK {SERIES} 2 SHRINK ] [ ] METASORT <% SIZE %> false LMAP"
        ),
        longs(&[2, 1]),
    );
    // Series without a name sort first; the list is left on the stack.
    assert_stack(
        &format!(
            "[ {SERIES} [ ] MERGE 'u' RENAME [ ] MERGE ] [ ] METASORT DUP SIZE SWAP <% NAME %> false LMAP"
        ),
        &[
            Value::Long(3),
            Value::list(vec![Value::Null, Value::from("g"), Value::from("u")]),
        ],
    );
    assert_top(
        &format!("[ {SERIES} [ ] MERGE ] [ ] METASORT <% NAME %> false LMAP"),
        Value::list(vec![Value::Null, Value::from("g")]),
    );
    assert_top(
        &format!("[ [ ] MERGE {SERIES} ] [ ] METASORT <% NAME %> false LMAP"),
        Value::list(vec![Value::Null, Value::from("g")]),
    );
    assert_top(
        "[ [ ] MERGE { 'a' '1' } RELABEL [ ] MERGE ] [ ] METASORT <% LABELS SIZE %> false LMAP",
        longs(&[0, 1]),
    );
    // Along fields: labels, the name for `NULL`, attributes on request.
    let three = THREE.replace("SERIES", SERIES);
    assert_top(
        &format!("{three} [ 'l' ] METASORT <% NAME %> false LMAP"),
        strings(&["c", "b", "a"]),
    );
    assert_top(
        &format!("{three} [ NULL 'l' ] METASORT <% NAME %> false LMAP"),
        strings(&["a", "b", "c"]),
    );
    assert_top(
        &format!("{three} [ 'l' NULL ] METASORT <% NAME %> false LMAP"),
        strings(&["c", "b", "a"]),
    );
    assert_top(
        &format!("{three} [ 'missing' NULL ] METASORT <% NAME %> false LMAP"),
        strings(&["a", "b", "c"]),
    );
    assert_top(
        &format!(
            "[ {SERIES} 'b' RENAME {SERIES} 'a' RENAME ] [ 'l' NULL ] METASORT <% NAME %> false LMAP"
        ),
        strings(&["a", "b"]),
    );
    assert_top(
        &format!(
            "[ {SERIES} {{ 'x' '1' }} RELABEL 'a' RENAME {SERIES} 'b' RENAME ] [ 'x' ] METASORT <% NAME %> false LMAP"
        ),
        strings(&["b", "a"]),
    );
    assert_top(
        &format!(
            "[ {SERIES} 'b' RENAME {SERIES} {{ 'x' '1' }} RELABEL 'a' RENAME ] [ 'x' ] METASORT <% NAME %> false LMAP"
        ),
        strings(&["b", "a"]),
    );
    let attributed = ATTRIBUTED.replace("SERIES", SERIES);
    assert_top(
        &format!("{attributed} [ 'x' ] true METASORT <% NAME %> false LMAP"),
        strings(&["c", "a", "b"]),
    );
    assert_top(
        &format!("{attributed} [ 'x' ] false METASORT <% NAME %> false LMAP"),
        strings(&["b", "a", "c"]),
    );
    assert_top(
        &format!("{attributed} [ 'x' ] METASORT <% NAME %> false LMAP"),
        strings(&["b", "a", "c"]),
    );
    assert_fails(
        &format!("[ {SERIES} ] [ 1 ] METASORT"),
        "METASORT expects a list of fields (strings).",
    );
    assert_fails(
        &format!("[ {SERIES} ] 1 METASORT"),
        "METASORT expects a list of fields to consider for sorting on top of the stack.",
    );
    assert_fails(
        "[ 1 ] [ ] METASORT",
        "METASORT operates on a list of Geo Time Series.",
    );
    assert_fails(
        "1 [ ] METASORT",
        "METASORT operates on a list of Geo Time Series.",
    );
}

#[test]
fn integrate_accumulates_rates_per_second() {
    assert_stack(
        &format!("{SERIES} 1.5 INTEGRATE DUP TICKLIST SWAP VALUES"),
        &[longs(&[10, 20, 30]), doubles(&[1.5, 1.500_01, 1.500_03])],
    );
    assert_top(
        &format!("{SERIES} 30000000 TIMESCALE 1.5 INTEGRATE VALUES"),
        doubles(&[1.5, 301.5, 901.5]),
    );
    assert_top(
        &format!("{SERIES} 30000000 TIMESCALE 2 INTEGRATE ELEVATIONS"),
        Value::list(vec![
            Value::Double(f64::NAN),
            Value::Long(35),
            Value::Long(5),
        ]),
    );
    // The input is sorted in place; an empty series yields the initial value at
    // `Long.MIN_VALUE`.
    assert_stack(
        &format!("{SERIES} 'g' STORE $g RSORT DROP $g 0 INTEGRATE TICKLIST $g TICKLIST"),
        &[longs(&[10, 20, 30]), longs(&[10, 20, 30])],
    );
    assert_stack(
        "NEWGTS 1.0 INTEGRATE DUP TICKLIST SWAP VALUES",
        &[longs(&[i64::MIN]), doubles(&[1.0])],
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 1 INTEGRATE VALUES",
        doubles(&[1.0]),
    );
    assert_fails(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE 1 INTEGRATE",
        "class java.lang.String cannot be cast to class java.lang.Number",
    );
    assert_fails(
        "[ NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE ] 1 INTEGRATE",
        "class java.lang.Boolean cannot be cast to class java.lang.Number",
    );
    assert_top(
        &format!("[ {SERIES} NEWGTS ] 1 INTEGRATE <% SIZE %> false LMAP"),
        longs(&[3, 1]),
    );
    assert_fails(
        &format!("{SERIES} 'x' INTEGRATE"),
        "INTEGRATE expects a numeric initial value on top of the stack.",
    );
    assert_fails(
        "1 1 INTEGRATE",
        "INTEGRATE can only operate on Geo Time Series instances.",
    );
}

#[test]
fn resets_compensate_counter_resets() {
    assert_top(
        &format!("{RESETS} false RESETS VALUES"),
        longs(&[5, 7, 9, 10, 11]),
    );
    assert_top(
        &format!("{RESETS} true RESETS VALUES"),
        longs(&[5, 12, 7, 10, 8]),
    );
    assert_top(
        &format!(
            "{RESETS} 'g' STORE [ $g [ $g 1.5 TIMESCALE ] 7 ] false RESETS <% VALUES %> false LMAP"
        ),
        Value::list(vec![longs(&[5, 7, 9, 10, 11]), longs(&[5, 7, 9, 10, 11])]),
    );
    assert_stack(
        "NEWGTS 2 NaN NaN NaN 2.5 ADDVALUE 1 NaN NaN NaN 1.5 ADDVALUE 3 NaN NaN NaN 1.0 ADDVALUE \
         false RESETS DUP TICKLIST SWAP VALUES",
        &[longs(&[1, 2, 3]), doubles(&[1.5, 2.5, 3.5])],
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 2.0 ADDVALUE 3 NaN NaN NaN 1.0 ADDVALUE \
         true RESETS VALUES",
        doubles(&[1.0, 3.0, 2.0]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE false RESETS VALUES",
        strings(&["a", "b"]),
    );
    assert_top("NEWGTS false RESETS SIZE", 0);
    assert_top("NEWGTS 1 NaN NaN NaN COUNTER ADDVALUE false RESETS SIZE", 0);
    // A numeric series without datapoint: Warp 10 reads its first value anyway.
    assert_fails(
        &format!("{RESETS} 0 SHRINK false RESETS"),
        "Index 0 out of bounds for length 0",
    );
    assert_fails(
        "[ NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 0 SHRINK ] false RESETS",
        "Index 0 out of bounds for length 0",
    );
    assert_top(
        &format!("{RESETS} 'g' STORE $g false RESETS DROP $g VALUES"),
        longs(&[5, 7, 2, 3, 1]),
    );
    assert_fails(
        &format!("{RESETS} 1 RESETS"),
        "RESETS expects a boolean as parameter.",
    );
    assert_fails(
        "[ [ 1 ] ] false RESETS",
        "RESETS expects a list of Geo Time Series as first parameter.",
    );
    assert_fails(
        "1 false RESETS",
        "RESETS operates on a Geo Time Series or list thereof.",
    );
}

#[test]
fn monotonic_is_the_running_extremum() {
    assert_top(
        &format!("{RESETS} false MONOTONIC VALUES"),
        longs(&[5, 7, 7, 7, 7]),
    );
    assert_top(
        &format!("{RESETS} true MONOTONIC VALUES"),
        longs(&[5, 5, 2, 2, 1]),
    );
    assert_top(
        &format!("{DOUBLES} false MONOTONIC VALUES"),
        doubles(&[2.5, 2.5, 3.0]),
    );
    assert_top(
        &format!("{DOUBLES} true MONOTONIC VALUES"),
        doubles(&[2.5, 1.5, 1.5]),
    );
    assert_top(
        &format!("{STRINGS} false MONOTONIC VALUES"),
        strings(&["b", "b", "c"]),
    );
    assert_top(
        &format!("{STRINGS} true MONOTONIC VALUES"),
        strings(&["b", "a", "a"]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN false ADDVALUE 2 NaN NaN NaN true ADDVALUE 3 NaN NaN NaN false ADDVALUE \
         false MONOTONIC VALUES",
        booleans(&[false, true, true]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE 3 NaN NaN NaN true ADDVALUE \
         true MONOTONIC VALUES",
        booleans(&[true, false, false]),
    );
    // The input is sorted in place; location and elevation are kept.
    assert_stack(
        &format!("{SERIES} 'g' STORE $g RSORT DROP $g false MONOTONIC ELEVATIONS $g TICKLIST"),
        &[
            Value::list(vec![
                Value::Double(f64::NAN),
                Value::Long(35),
                Value::Long(5),
            ]),
            longs(&[10, 20, 30]),
        ],
    );
    assert_top(
        &format!("[ {RESETS} NEWGTS ] true MONOTONIC <% SIZE %> false LMAP"),
        longs(&[5, 0]),
    );
    assert_fails(
        &format!("{RESETS} 'x' MONOTONIC"),
        "MONOTONIC expects a boolean on top of the stack.",
    );
    assert_fails(
        "1 true MONOTONIC",
        "MONOTONIC can only operate on Geo Time Series instances.",
    );
}

#[test]
fn atbucket_reads_a_bucket() {
    assert_eq!(
        run_with(
            bucketized(),
            "$b 0 ATBUCKET $b 9 ATBUCKET $b 7 ATBUCKET $b -1 ATBUCKET $b 2.9 ATBUCKET"
        ),
        vec![
            Value::list(vec![
                Value::Long(10),
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Long(1),
            ]),
            Value::list(vec![
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Null,
            ]),
            Value::list(vec![
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Null,
            ]),
            Value::list(vec![
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Null,
            ]),
            Value::list(vec![
                Value::Long(30),
                Value::Double(f64::NAN),
                Value::Double(f64::NAN),
                Value::Long(5),
                Value::Long(3),
            ]),
        ]
    );
    // The series is sorted in place.
    assert_eq!(
        run_with(bucketized(), "$b 9 ATBUCKET DROP $b TICKLIST"),
        vec![longs(&[10, 20, 30])]
    );
    let mut stack = test_stack();
    stack.push(Value::Gts(bucketized().shared()));
    let error = stack
        .exec_checked("10 ATBUCKET")
        .expect_err("exec_checked should fail");
    assert!(
        error
            .to_string()
            .contains("ATBUCKET cannot retrieve bucket 10 of a GTS with 10 buckets."),
        "{error}"
    );
    let mut stack = test_stack();
    stack.push(Value::Gts(bucketized().shared()));
    let error = stack
        .exec_checked("'x' ATBUCKET")
        .expect_err("exec_checked should fail");
    assert!(
        error
            .to_string()
            .contains("ATBUCKET expects an index on top of the stack."),
        "{error}"
    );
    assert_fails(
        &format!("{SERIES} 0 ATBUCKET"),
        "ATBUCKET expects the Geo Time Series on top of the stack to be bucketized.",
    );
    assert_fails(
        "1 0 ATBUCKET",
        "ATBUCKET expects a Geo Time Series instance on top of the stack.",
    );
    assert_eq!(
        top("NEWGTS 1 NaN NaN NaN 1 ADDVALUE TICKINDEX TICKLIST"),
        longs(&[0])
    );
}

#[test]
fn functions_fail_on_an_empty_stack() {
    let cases: &[(&str, &[&str])] = &[
        ("DEDUP", &[""]),
        ("VALUEDEDUP", &["", "true"]),
        ("COMPACT", &[""]),
        ("RANGECOMPACT", &[""]),
        ("ONLYBUCKETS", &[""]),
        ("EMPTY", &[""]),
        ("NONEMPTY", &[""]),
        ("COMMONTICKS", &[""]),
        ("VALUESORT", &[""]),
        ("RVALUESORT", &[""]),
        ("LASTSORT", &[""]),
        ("METASORT", &["", "true", "[ ]"]),
        ("INTEGRATE", &["", "1"]),
        ("RESETS", &["", "true"]),
        ("MONOTONIC", &["", "true"]),
        ("ATBUCKET", &["", "0"]),
    ];
    for (name, prefixes) in cases {
        for prefix in *prefixes {
            assert_fails(&format!("{prefix} {name}"), "Empty stack.");
        }
    }
}
