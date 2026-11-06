// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the time transforms of Geo Time Series. The expected values were checked against
//! Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use indexmap::IndexMap;

use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_stack, assert_top, exec, test_stack};
use crate::value::Value;

/// A `LONG` series `g{l=v}` with three datapoints, the second one located and elevated.
const SERIES: &str = "NEWGTS 'g' RENAME { 'l' 'v' } RELABEL \
    10 NaN NaN NaN 1 ADDVALUE 20 48.0 2.0 35 2 ADDVALUE 30 NaN NaN 5 3 ADDVALUE";

/// A `LONG` series with two gaps of at least 5 time units.
const GAPS: &str = "NEWGTS 'g' RENAME { 'l' 'v' } RELABEL { 'a' 'b' } SETATTRIBUTES \
    1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 10 NaN NaN NaN 3 ADDVALUE \
    20 NaN NaN NaN 4 ADDVALUE 21 NaN NaN NaN 5 ADDVALUE";

/// A `DOUBLE` series `g{l=v,x=1}{at=1}`.
const DOUBLES: &str = "NEWGTS 'g' RENAME { 'l' 'v' 'x' '1' } RELABEL { 'at' '1' } SETATTRIBUTES \
    40 NaN NaN NaN 4.5 ADDVALUE 5 NaN NaN NaN 0.5 ADDVALUE";

fn longs(values: &[i64]) -> Value {
    Value::list(values.iter().copied().map(Value::Long).collect())
}

fn strings(values: &[&str]) -> Value {
    Value::list(values.iter().copied().map(Value::from).collect())
}

fn string_map(entries: &[(&str, &str)]) -> Value {
    Value::map(
        entries
            .iter()
            .map(|(k, v)| (Value::from(*k), Value::from(*v)))
            .collect::<IndexMap<_, _>>(),
    )
}

/// `[ SERIES bucketizer.sum 100 10 0 ] BUCKETIZE 0 GET` as Warp 10 builds it: the datapoints
/// in reverse order, 10 buckets of 10 ending at 100.
fn bucketized() -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::with_buckets(100, 10, 10);
    gts.metadata.name = Some("g".into());
    gts.metadata.labels.insert("l".into(), "v".into());
    for (tick, value) in [(30, 3), (20, 2), (10, 1)] {
        gts.set_value(tick, NO_LOCATION, NO_ELEVATION, &Value::Long(value), false);
    }
    gts
}

/// Run `script` with `gts` stored under `$b` and return the stack contents, bottom first.
fn run_with(gts: GeoTimeSerie, script: &str) -> Vec<Value> {
    let mut stack = test_stack();
    stack.push(Value::Gts(gts.shared()));
    stack.exec_checked("'b' STORE").unwrap();
    stack.exec_checked(script).unwrap();
    stack.elements().to_vec()
}

#[test]
fn timeshift_offsets_ticks_and_last_bucket() {
    assert_stack(
        &format!("{SERIES} 5 TIMESHIFT DUP LASTBUCKET SWAP TICKLIST"),
        &[Value::Long(0), longs(&[15, 25, 35])],
    );
    assert_eq!(
        run_with(bucketized(), "$b 5 TIMESHIFT DUP LASTBUCKET SWAP TICKLIST"),
        vec![Value::Long(105), longs(&[35, 25, 15])]
    );
    // A list is shifted series by series; the originals are untouched.
    assert_stack(
        &format!("{SERIES} 'g' STORE [ $g $g ] 1 TIMESHIFT <% TICKLIST %> false LMAP $g TICKLIST"),
        &[
            Value::list(vec![longs(&[11, 21, 31]), longs(&[11, 21, 31])]),
            longs(&[10, 20, 30]),
        ],
    );
    assert_top(
        &format!("{SERIES} 9223372036854775807 TIMESHIFT TICKLIST"),
        longs(&[i64::MIN + 9, i64::MIN + 19, i64::MIN + 29]),
    );
    assert_fails(
        &format!("{SERIES} 'x' TIMESHIFT"),
        "TIMESHIFT expects a long offset as parameter.",
    );
    assert_fails(
        "[ 1 ] 1 TIMESHIFT",
        "TIMESHIFT expects a Geo Time Series or a list thereof under the offset parameter.",
    );
}

#[test]
fn timescale_multiplies_ticks() {
    assert_top(
        &format!("{SERIES} 2 TIMESCALE TICKLIST"),
        longs(&[20, 40, 60]),
    );
    assert_top(
        &format!("{SERIES} 0.5 TIMESCALE TICKLIST"),
        longs(&[5, 10, 15]),
    );
    assert_top(
        &format!("{SERIES} 0.33 TIMESCALE TICKLIST"),
        longs(&[3, 6, 9]),
    );
    assert_top(
        &format!("{SERIES} NaN TIMESCALE TICKLIST"),
        longs(&[0, 0, 0]),
    );
    // A negative scale reverses the order of a sorted series.
    assert_stack(
        &format!("{SERIES} -1 TIMESCALE DUP TICKLIST SWAP FIRSTTICK"),
        &[longs(&[-10, -20, -30]), Value::Long(-30)],
    );
    assert_top(
        &format!("{SERIES} -1 TIMESCALE 25 NaN NaN NaN 9 ADDVALUE SORT TICKLIST"),
        longs(&[-30, -20, -10, 25]),
    );
    assert_top(
        "NEWGTS 3 NaN NaN NaN 1 ADDVALUE 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE \
         -2 TIMESCALE SORT TICKLIST",
        longs(&[-6, -4, -2]),
    );
    let mut stack = test_stack();
    stack.push(Value::Gts(bucketized().shared()));
    let error = stack.exec_checked("2 TIMESCALE").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Cannot apply timescale on a bucketized GTS. Unbucketize it first."),
        "{error}"
    );
    assert_fails(
        &format!("{SERIES} 'x' TIMESCALE"),
        "TIMESCALE expects a time scale on top of the stack.",
    );
    assert_fails(
        "[ 1 ] 1 TIMESCALE",
        "TIMESCALE expects a Geo Time Series or a list thereof under the scale parameter.",
    );
}

#[test]
fn timemodulo_splits_by_quotient() {
    assert_top(
        &format!(
            "{SERIES} 25 'q' TIMEMODULO <% DUP LABELS SWAP DUP TICKLIST SWAP VALUES 3 ->LIST %> false LMAP"
        ),
        Value::list(vec![
            Value::list(vec![
                string_map(&[("l", "v"), ("q", "0")]),
                longs(&[10, 20]),
                longs(&[1, 2]),
            ]),
            Value::list(vec![
                string_map(&[("l", "v"), ("q", "1")]),
                longs(&[5]),
                longs(&[3]),
            ]),
        ]),
    );
    // Java's truncating division and remainder.
    assert_top(
        &format!(
            "{SERIES} -25 'q' TIMEMODULO <% DUP LABELS 'q' GET SWAP TICKLIST 2 ->LIST %> false LMAP"
        ),
        Value::list(vec![
            Value::list(vec![Value::from("-1"), longs(&[5])]),
            Value::list(vec![Value::from("0"), longs(&[10, 20])]),
        ]),
    );
    assert_top(
        &format!("[ {SERIES} NEWGTS ] 25 'q' TIMEMODULO <% SIZE %> false LMAP"),
        longs(&[2, 0]),
    );
    assert_top(
        &format!("{SERIES} 20 48.0 2.0 35 2 SETVALUE 25 'q' TIMEMODULO 0 GET ELEVATIONS"),
        Value::list(vec![Value::Double(f64::NAN), Value::Long(35)]),
    );
    assert_fails(&format!("{SERIES} 0 'q' TIMEMODULO"), "/ by zero");
    assert_fails(
        &format!("[ NEWGTS {SERIES} ] 0 'q' TIMEMODULO"),
        "/ by zero",
    );
    assert_top("NEWGTS 0 'q' TIMEMODULO SIZE", 0);
    assert_fails(
        &format!("{SERIES} 1 TIMEMODULO"),
        "TIMEMODULO expects a quotient label name on top of the stack.",
    );
    assert_fails(
        &format!("{SERIES} 'x' 'q' TIMEMODULO"),
        "TIMEMODULO expects a LONG modulo under the quotient label name.",
    );
    assert_fails(
        "1 25 'q' TIMEMODULO",
        "TIMEMODULO can only operate on Geo Time Series instances.",
    );
    assert_fails(
        "[ NEWGTS 1 ] 25 'q' TIMEMODULO",
        "TIMEMODULO can only operate on Geo Time Series instances.",
    );
}

#[test]
fn timeclip_keeps_a_time_frame() {
    // `end duration`: the frame is `[end - duration + 1, end]`.
    assert_top(
        &format!("{SERIES} 30 15 TIMECLIP TICKLIST"),
        longs(&[20, 30]),
    );
    // ISO 8601 bounds, in any combination.
    assert_top(
        &format!(
            "{SERIES} '1970-01-01T00:00:00.000030Z' '1970-01-01T00:00:00.000020Z' TIMECLIP TICKLIST"
        ),
        longs(&[20, 30]),
    );
    assert_top(
        &format!("{SERIES} 30 '1970-01-01T00:00:00.000020Z' TIMECLIP TICKLIST"),
        longs(&[20, 30]),
    );
    assert_top(
        &format!("{SERIES} '1970-01-01T00:00:00.000030Z' 20 TIMECLIP TICKLIST"),
        longs(&[20, 30]),
    );
    // A sorted series is sorted ascending in place; the clip keeps name and labels.
    assert_stack(
        &format!("{SERIES} 'g' STORE $g RSORT DROP $g 30 100 TIMECLIP TICKLIST $g TICKLIST"),
        &[longs(&[10, 20, 30]), longs(&[10, 20, 30])],
    );
    assert_stack(
        &format!("{SERIES} 20 5 TIMECLIP DUP NAME SWAP DUP LABELS SWAP TICKLIST"),
        &[Value::from("g"), string_map(&[("l", "v")]), longs(&[20])],
    );
    assert_top(&format!("{SERIES} 5 100 TIMECLIP SIZE"), 0);
    assert_top(&format!("{SERIES} 100 5 TIMECLIP SIZE"), 0);
    assert_top(&format!("{SERIES} 15 1 TIMECLIP SIZE"), 0);
    assert_top(&format!("{SERIES} 25 5 TIMECLIP SIZE"), 0);
    assert_top(&format!("{SERIES} 10 -5 TIMECLIP SIZE"), 0);
    assert_top(
        &format!("{SERIES} 20 48.0 2.0 35 2 ADDVALUE 20 1 TIMECLIP SIZE"),
        2,
    );
    assert_top(
        &format!("{SERIES} 10 NaN NaN NaN 1 ADDVALUE SORT 10 1 TIMECLIP SIZE"),
        2,
    );
    // Duplicates of the bounds are all kept, wherever the binary searches land.
    assert_top(
        "NEWGTS 5 NaN NaN NaN 'x' ADDVALUE 10 NaN NaN NaN 'a' ADDVALUE 10 NaN NaN NaN 'b' ADDVALUE \
         10 NaN NaN NaN 'c' ADDVALUE 10 NaN NaN NaN 'd' ADDVALUE 20 NaN NaN NaN 'e' ADDVALUE \
         10 1 TIMECLIP VALUES",
        strings(&["a", "b", "c", "d"]),
    );
    assert_top(
        &format!("{DOUBLES} SORT 40 36 TIMECLIP VALUES"),
        Value::list(vec![Value::Double(0.5), Value::Double(4.5)]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE 1 1 TIMECLIP VALUES",
        Value::list(vec![Value::Boolean(true)]),
    );
    // A sorted series without datapoint and a flagged-sorted empty one.
    assert_top("NEWGTS SORT 10 1 TIMECLIP SIZE", 0);
    assert_top("NEWGTS 10 1 TIMECLIP SIZE", 0);
    // An unsorted series is scanned in storage order and keeps its bucketization.
    assert_top(
        "NEWGTS 30 NaN NaN NaN 3 ADDVALUE 10 NaN NaN NaN 1 ADDVALUE 20 NaN NaN NaN 2 ADDVALUE \
         25 20 TIMECLIP TICKLIST",
        longs(&[10, 20]),
    );
    assert_eq!(
        run_with(
            bucketized(),
            "$b 25 NaN NaN NaN 9 ADDVALUE 30 100 TIMECLIP DUP TICKLIST SWAP BUCKETCOUNT"
        ),
        vec![longs(&[30, 20, 10, 25]), Value::Long(10)]
    );
    assert_eq!(
        run_with(
            bucketized(),
            "$b 30 100 TIMECLIP DUP TICKLIST SWAP BUCKETCOUNT"
        ),
        vec![longs(&[10, 20, 30]), Value::Long(0)]
    );
    assert_top(
        &format!("[ {SERIES} NEWGTS ] 30 15 TIMECLIP <% SIZE %> false LMAP"),
        longs(&[2, 0]),
    );
    assert_fails(
        &format!("{SERIES} 20 'x' TIMECLIP"),
        "Can only parse an ISO8601 timestamp (YYYY-MM-DDThh:mm:ss.sssssssss) with a specified time zone. (Text 'x' could not be parsed at index 0)",
    );
    assert_fails(
        &format!("{SERIES} 'x' 20 TIMECLIP"),
        "Text 'x' could not be parsed at index 0",
    );
    assert_fails(
        &format!("{SERIES} 1.5 TIMECLIP"),
        "TIMECLIP expects either an ISO8601 timestamp as the origin timestamp or a duration.",
    );
    assert_fails(
        &format!("{SERIES} 1.5 10 TIMECLIP"),
        "TIMECLIP expects either an ISO8601 timestamp or a delta since Unix Epoch as 'now' parameter.",
    );
    assert_fails(
        "1 30 10 TIMECLIP",
        "TIMECLIP expects a Geo Time Series, a GTSEncoder or a list thereof under the timeframe definition.",
    );
}

#[test]
fn clip_applies_several_frames() {
    assert_top(
        &format!("{SERIES} [ [ 25 5 ] [ 15 100 ] [ 100 200 ] ] CLIP <% TICKLIST %> false LMAP"),
        Value::list(vec![longs(&[10, 20]), longs(&[20, 30]), longs(&[])]),
    );
    // Reversed pairs are reordered in place; doubles are truncated.
    assert_top(
        &format!("[ [ 25 5 ] ] 'p' STORE {SERIES} $p CLIP DROP $p"),
        Value::list(vec![longs(&[5, 25])]),
    );
    assert_top(
        &format!("{SERIES} [ [ 1.5 20.7 ] ] CLIP <% TICKLIST %> false LMAP"),
        Value::list(vec![longs(&[10, 20])]),
    );
    assert_top(
        &format!("[ {SERIES} {SERIES} ] [ [ 1 10 ] ] CLIP <% 0 GET SIZE %> false LMAP"),
        longs(&[1, 1]),
    );
    assert_fails(
        &format!("{SERIES} [ [ 'a' 2 ] ] CLIP"),
        "CLIP expects the limits to be numeric.",
    );
    assert_fails(
        &format!("{SERIES} [ [ 1 'b' ] ] CLIP"),
        "CLIP expects the limits to be numeric.",
    );
    for limits in ["[ [ 1 2 3 ] ]", "[ 1 ]", "1"] {
        assert_fails(
            &format!("{SERIES} {limits} CLIP"),
            "CLIP expects a list of limit pairs on top of the stack.",
        );
    }
    assert_fails(
        "[ 1 ] [ ] CLIP",
        "CLIP expects a GeoTimeSeries, a GTSEncoder or a list thereof under the list of limit.",
    );
}

#[test]
fn timesplit_cuts_at_quiet_periods() {
    assert_top(
        &format!(
            "{GAPS} 5 1 's' TIMESPLIT <% DUP LABELS 's' GET SWAP DUP ATTRIBUTES SWAP TICKLIST 3 ->LIST %> false LMAP"
        ),
        Value::list(vec![
            Value::list(vec![
                Value::from("1"),
                string_map(&[("a", "b")]),
                longs(&[1, 2]),
            ]),
            Value::list(vec![
                Value::from("2"),
                string_map(&[("a", "b")]),
                longs(&[10]),
            ]),
            Value::list(vec![
                Value::from("3"),
                string_map(&[("a", "b")]),
                longs(&[20, 21]),
            ]),
        ]),
    );
    // Pieces too small are dropped but still numbered.
    assert_top(
        &format!(
            "{GAPS} 5 2 's' TIMESPLIT <% DUP LABELS 's' GET SWAP TICKLIST 2 ->LIST %> false LMAP"
        ),
        Value::list(vec![
            Value::list(vec![Value::from("1"), longs(&[1, 2])]),
            Value::list(vec![Value::from("3"), longs(&[20, 21])]),
        ]),
    );
    assert_top(&format!("{GAPS} 5 -1 's' TIMESPLIT SIZE"), 3);
    assert_top(&format!("{GAPS} 5 3 's' TIMESPLIT SIZE"), 0);
    assert_top(&format!("{GAPS} 0 1 's' TIMESPLIT SIZE"), 5);
    assert_top(&format!("{GAPS} 5 4294967297 's' TIMESPLIT SIZE"), 3);
    // An existing label or an empty series: a single clone.
    assert_top(
        &format!("{GAPS} 5 1 'l' TIMESPLIT <% TICKLIST %> false LMAP"),
        Value::list(vec![longs(&[1, 2, 10, 20, 21])]),
    );
    assert_top("NEWGTS 5 1 's' TIMESPLIT SIZE", 1);
    let mut stack = exec(&format!("{GAPS} 'g' STORE $g 5 1 'l' TIMESPLIT 0 GET $g"));
    let original = stack.pop().unwrap();
    assert!(!stack.pop().unwrap().same_object(&original));
    // The input is sorted in place.
    assert_top(
        &format!("{GAPS} 'g' STORE $g RSORT DROP $g 5 1 's' TIMESPLIT DROP $g TICKLIST"),
        longs(&[1, 2, 10, 20, 21]),
    );
    assert_top(
        &format!("[ {GAPS} NEWGTS ] 5 1 's' TIMESPLIT <% SIZE %> false LMAP"),
        longs(&[3, 1]),
    );
    assert_eq!(
        run_with(bucketized(), "$b 5 1 's' TIMESPLIT 0 GET BUCKETCOUNT"),
        vec![Value::Long(10)]
    );
    assert_fails(
        &format!("{GAPS} 5 1 1 TIMESPLIT"),
        "TIMESPLIT expects a label on top of the stack.",
    );
    assert_fails(
        &format!("{GAPS} 5 'x' 's' TIMESPLIT"),
        "TIMESPLIT expects a minimum number of values under the label.",
    );
    assert_fails(
        &format!("{GAPS} 'x' 1 's' TIMESPLIT"),
        "TIMESPLIT expects a quiet period under the minimum number of values.",
    );
    assert_fails(
        "1 5 1 's' TIMESPLIT",
        "TIMESPLIT expects a Geo Time Series or a list thereof under the label, minimum values and the quiet period parameters.",
    );
}

/// The `[ label ticklist ]` pairs of the chunks `script` produces.
fn chunks(script: &str) -> Value {
    crate::tests::top(&format!(
        "{script} <% DUP LABELS 'c' GET SWAP TICKLIST 2 ->LIST %> false LMAP"
    ))
}

fn chunk(label: &str, ticks: &[i64]) -> Value {
    Value::list(vec![Value::from(label), longs(ticks)])
}

#[test]
fn chunk_cuts_fixed_width_chunks() {
    // `lastchunk chunkwidth overlap chunkcount label keepempty CHUNK`
    assert_eq!(
        chunks(&format!("{SERIES} 0 10 0 0 'c' false CHUNK")),
        Value::list(vec![
            chunk("10", &[10]),
            chunk("20", &[20]),
            chunk("30", &[30])
        ])
    );
    assert_eq!(
        chunks(&format!("{SERIES} 35 10 0 0 'c' false CHUNK")),
        Value::list(vec![
            chunk("15", &[10]),
            chunk("25", &[20]),
            chunk("35", &[30])
        ])
    );
    assert_eq!(
        chunks(&format!("{SERIES} 50 10 0 0 'c' true CHUNK")),
        Value::list(vec![
            chunk("10", &[10]),
            chunk("20", &[20]),
            chunk("30", &[30]),
            chunk("40", &[]),
            chunk("50", &[]),
        ])
    );
    // Without empty chunks, the leading empty chunks are deducted from the count.
    assert_top(&format!("{SERIES} 50 10 0 2 'c' false CHUNK SIZE"), 0);
    assert_eq!(
        chunks(&format!("{SERIES} 50 10 0 3 'c' false CHUNK")),
        Value::list(vec![chunk("30", &[30])])
    );
    assert_eq!(
        chunks(&format!("{SERIES} 50 10 0 3 'c' true CHUNK")),
        Value::list(vec![chunk("30", &[30]), chunk("40", &[]), chunk("50", &[])])
    );
    // An overlap copies the neighbouring datapoints into each chunk.
    assert_eq!(
        chunks(&format!("{SERIES} 0 10 3 0 'c' false CHUNK")),
        Value::list(vec![
            chunk("10", &[10]),
            chunk("20", &[20, 10]),
            chunk("30", &[30, 20]),
        ])
    );
    assert_eq!(
        chunks(&format!("{SERIES} 0 10 3 2 'c' true CHUNK")),
        Value::list(vec![chunk("20", &[20, 10]), chunk("30", &[30, 20])])
    );
    assert_eq!(
        chunks(&format!("{SERIES} 0 25 5 0 'c' false CHUNK")),
        Value::list(vec![chunk("25", &[20, 10, 30]), chunk("50", &[30])])
    );
    assert_eq!(
        chunks(&format!("{SERIES} 0 10 10 0 'c' false CHUNK")),
        Value::list(vec![
            chunk("10", &[10, 20]),
            chunk("20", &[20, 10, 30]),
            chunk("30", &[30, 20]),
        ])
    );
    // The unaligned last chunk moves to the next boundary; the input is sorted in reverse.
    assert_eq!(
        chunks(&format!("{SERIES} -7 10 0 0 'c' false CHUNK")),
        Value::list(vec![])
    );
    assert_eq!(
        chunks("NEWGTS 15 NaN NaN NaN 1 ADDVALUE 0 10 0 0 'c' false CHUNK"),
        Value::list(vec![chunk("20", &[15])])
    );
    assert_top(
        &format!("{SERIES} 'g' STORE $g 0 10 0 0 'c' false CHUNK DROP $g TICKLIST"),
        longs(&[30, 20, 10]),
    );
    // Empty series.
    assert_top("NEWGTS 0 10 0 0 'c' false CHUNK SIZE", 0);
    assert_top(
        "NEWGTS 100 10 0 3 'c' true CHUNK <% LABELS 'c' GET %> false LMAP",
        strings(&["80", "90", "100"]),
    );
    assert_top("NEWGTS 100 10 0 3 'c' false CHUNK SIZE", 0);
    // Metadata is copied into every chunk.
    assert_top(
        &format!(
            "{SERIES} {{ 'a' '1' }} SETATTRIBUTES 0 10 0 0 'c' false CHUNK 0 GET DUP NAME SWAP DUP LABELS SWAP ATTRIBUTES 3 ->LIST"
        ),
        Value::list(vec![
            Value::from("g"),
            string_map(&[("l", "v"), ("c", "10")]),
            string_map(&[("a", "1")]),
        ]),
    );
    assert_top(
        &format!("[ {SERIES} NEWGTS ] 0 10 0 0 'c' false CHUNK <% SIZE %> false LMAP"),
        longs(&[3, 0]),
    );
    assert_fails(
        &format!("{SERIES} 0 10 11 0 'c' false CHUNK"),
        "Overlap cannot exceed chunk width.",
    );
    assert_fails(
        &format!("{SERIES} 0 -10 0 0 'c' false CHUNK"),
        "Overlap cannot exceed chunk width.",
    );
    assert_fails(
        &format!("{SERIES} {{ 'c' 'x' }} RELABEL 0 10 0 0 'c' false CHUNK"),
        "Cannot operate on Geo Time Series which already have a label named 'c'",
    );
    assert_fails(&format!("{SERIES} 0 0 0 0 'c' false CHUNK"), "/ by zero");
    assert_fails(
        "1 0 10 0 0 'c' false CHUNK",
        "CHUNK expects a Geo Time Series, a GTSEncoder or a list thereof under the lastchunk parameter.",
    );
    for (script, message) in [
        (
            "0 10 0 0 'c' 1 CHUNK",
            "CHUNK expects on top of the stack a boolean indicating whether or not to keep empty chunks.",
        ),
        (
            "0 10 0 0 1 true CHUNK",
            "CHUNK expects the name of the 'chunk' label below the top of the stack.",
        ),
        (
            "0 10 0 'x' 'c' true CHUNK",
            "CHUNK expects a number of chunks under the 'chunk' label.",
        ),
        (
            "0 10 'x' 0 'c' true CHUNK",
            "CHUNK expects an overlap below the number of chunks.",
        ),
        (
            "0 'x' 0 0 'c' true CHUNK",
            "CHUNK expects a chunk width on top of the end timestamp of the most recent chunk.",
        ),
        (
            "'x' 10 0 0 'c' true CHUNK",
            "CHUNK expects the end timestamp of the most recent chunk under the chunk width.",
        ),
    ] {
        assert_fails(&format!("{SERIES} {script}"), message);
    }
}

#[test]
fn chunk_of_bucketized_series() {
    // Chunks of a bucketized series are bucketized when the span divides the width.
    assert_eq!(
        run_with(
            bucketized(),
            "$b 0 20 0 0 'c' true CHUNK <% DUP LABELS 'c' GET SWAP DUP TICKLIST SWAP DUP BUCKETCOUNT SWAP LASTBUCKET 4 ->LIST %> false LMAP"
        ),
        vec![Value::list(vec![
            Value::list(vec![
                Value::from("20"),
                longs(&[20, 10]),
                Value::Long(2),
                Value::Long(20)
            ]),
            Value::list(vec![
                Value::from("40"),
                longs(&[30]),
                Value::Long(2),
                Value::Long(40)
            ]),
            Value::list(vec![
                Value::from("60"),
                longs(&[]),
                Value::Long(2),
                Value::Long(60)
            ]),
            Value::list(vec![
                Value::from("80"),
                longs(&[]),
                Value::Long(2),
                Value::Long(80)
            ]),
            Value::list(vec![
                Value::from("100"),
                longs(&[]),
                Value::Long(2),
                Value::Long(100)
            ]),
        ])]
    );
    assert_eq!(
        run_with(
            bucketized(),
            "$b 0 20 0 0 'c' false CHUNK <% DUP LABELS 'c' GET SWAP TICKLIST 2 ->LIST %> false LMAP"
        ),
        vec![Value::list(vec![
            chunk("20", &[20, 10]),
            chunk("40", &[30])
        ])]
    );
    assert_eq!(
        run_with(
            bucketized(),
            "$b 0 15 0 0 'c' false CHUNK <% DUP LABELS 'c' GET SWAP DUP TICKLIST SWAP BUCKETSPAN 3 ->LIST %> false LMAP"
        ),
        vec![Value::list(vec![
            Value::list(vec![Value::from("10"), longs(&[10]), Value::Long(0)]),
            Value::list(vec![Value::from("25"), longs(&[20]), Value::Long(0)]),
            Value::list(vec![Value::from("40"), longs(&[30]), Value::Long(0)]),
        ])]
    );
    // Chunks outside the buckets are empty.
    assert_eq!(
        run_with(
            bucketized(),
            "$b 120 20 0 8 'c' true CHUNK <% LABELS 'c' GET %> false LMAP"
        ),
        vec![strings(&["-20", "0", "20", "40", "60", "80", "100", "120"])]
    );
    assert_eq!(
        run_with(
            bucketized(),
            "$b 120 20 0 8 'c' false CHUNK <% LABELS 'c' GET %> false LMAP"
        ),
        vec![strings(&["20", "40"])]
    );
    assert_eq!(
        run_with(
            bucketized(),
            "$b 120 20 5 8 'c' false CHUNK <% DUP LABELS 'c' GET SWAP TICKLIST 2 ->LIST %> false LMAP"
        ),
        vec![Value::list(vec![
            chunk("20", &[20, 10]),
            chunk("40", &[30, 20])
        ])]
    );
    let mut stack = test_stack();
    stack.push(Value::Gts(bucketized().shared()));
    let error = stack.exec_checked("0 5 0 0 'c' false CHUNK").unwrap_err();
    assert!(
        error.to_string().contains(
            "Cannot operate on Geo Time Series with a bucketspan greater than the chunk width."
        ),
        "{error}"
    );
}

#[test]
fn crop_keeps_the_bucket_boundaries() {
    let mut gts = GeoTimeSerie::with_buckets(100, 10, 10);
    gts.metadata.name = Some("g".into());
    gts.metadata.labels.insert("l".into(), "v".into());
    for (tick, value) in [(80, 7), (30, 3), (20, 2), (10, 1)] {
        gts.set_value(tick, NO_LOCATION, NO_ELEVATION, &Value::Long(value), false);
    }
    assert_eq!(
        run_with(
            gts.clone(),
            "$b 80 NaN NaN NaN 8 ADDVALUE 100 NaN NaN NaN 10 ADDVALUE 75 NaN NaN NaN 7 ADDVALUE \
             110 NaN NaN NaN 11 ADDVALUE CROP DUP BUCKETCOUNT SWAP DUP BUCKETSPAN SWAP DUP LASTBUCKET \
             SWAP DUP TICKLIST SWAP DUP NAME SWAP LABELS"
        ),
        vec![
            Value::Long(10),
            Value::Long(10),
            Value::Long(100),
            longs(&[10, 20, 30, 80, 80, 100]),
            Value::from("g"),
            string_map(&[("l", "v")]),
        ]
    );
    assert_eq!(
        run_with(
            gts.clone(),
            "$b 115 NaN NaN NaN 8 ADDVALUE CROP DUP BUCKETCOUNT SWAP TICKLIST"
        ),
        vec![Value::Long(8), longs(&[10, 20, 30, 80])]
    );
    // The input is sorted in place; a non bucketized series is cloned.
    assert_eq!(
        run_with(gts.clone(), "$b CROP DROP $b TICKLIST"),
        vec![longs(&[10, 20, 30, 80])]
    );
    assert_stack(
        &format!("{SERIES} CROP DUP TICKLIST SWAP BUCKETCOUNT"),
        &[longs(&[10, 20, 30]), Value::Long(0)],
    );
    let mut stack = exec(&format!("{SERIES} 'g' STORE $g CROP $g"));
    let original = stack.pop().unwrap();
    assert!(!stack.pop().unwrap().same_object(&original));
    // Lists may nest lists of series; other elements are ignored.
    assert_top(
        &format!("{SERIES} 'g' STORE [ $g [ $g ] 1 'x' ] CROP SIZE"),
        2,
    );
    let mut stack = test_stack();
    stack.push(Value::Gts(gts.shared()));
    stack
        .exec_checked("'b' STORE [ $b ] CROP 0 GET BUCKETCOUNT")
        .unwrap();
    assert_eq!(stack.pop().unwrap(), Value::Long(8));
    // No datapoint on a bucket boundary: Warp 10 dies on its empty result.
    let mut off = GeoTimeSerie::with_buckets(100, 10, 10);
    off.set_value(75, NO_LOCATION, NO_ELEVATION, &Value::Long(1), false);
    for script in ["CROP", "[ ] SWAP +! CROP"] {
        let mut stack = test_stack();
        stack.push(Value::Gts(off.clone().shared()));
        let error = stack.exec_checked(script).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Cannot load from long array because \"cropped.ticks\" is null"),
            "{error}"
        );
    }
    assert_fails(
        "[ [ 1 ] ] CROP",
        "CROP expects a LIST or a Geo Time Series as input.",
    );
    assert_fails(
        "1 CROP",
        "CROP expects a LIST or a Geo Time Series as input.",
    );
}

#[test]
fn merge_concatenates_series() {
    assert_stack(
        &format!(
            "[ {SERIES} {DOUBLES} ] MERGE DUP NAME SWAP DUP LABELS SWAP DUP ATTRIBUTES SWAP DUP TICKLIST SWAP VALUES"
        ),
        &[
            Value::from("g"),
            string_map(&[("l", "v")]),
            Value::map(IndexMap::new()),
            longs(&[10, 20, 30, 40, 5]),
            longs(&[1, 2, 3, 4, 0]),
        ],
    );
    // Nested lists are flattened, other elements ignored; the first series fixes the type.
    assert_stack(
        &format!("[ {DOUBLES} [ {SERIES} ] 7 'x' ] MERGE DUP LABELS SWAP VALUES"),
        &[
            string_map(&[("l", "v"), ("x", "1")]),
            Value::list(vec![
                Value::Double(4.5),
                Value::Double(0.5),
                Value::Double(1.0),
                Value::Double(2.0),
                Value::Double(3.0),
            ]),
        ],
    );
    assert_stack(
        "[ ] MERGE DUP NAME SWAP DUP LABELS SWAP SIZE",
        &[Value::Null, Value::map(IndexMap::new()), Value::Long(0)],
    );
    assert_top("[ ] MERGE TOSTRING", "");
    assert_top(
        &format!("{SERIES} 20 48.0 2.0 35 2 SETVALUE 'g' STORE [ $g $g ] MERGE ELEVATIONS"),
        Value::list(vec![
            Value::Double(f64::NAN),
            Value::Long(35),
            Value::Long(5),
            Value::Double(f64::NAN),
            Value::Long(35),
            Value::Long(5),
        ]),
    );
    // The result is a new series.
    assert_top(
        &format!("{SERIES} 'g' STORE [ $g ] MERGE 40 NaN NaN NaN 4 ADDVALUE DROP $g SIZE"),
        3,
    );
    assert_fails(
        "[ [ 1 ] ] MERGE",
        "MERGE expects a list of Geo Time Series or encoders as first parameter.",
    );
    assert_fails(
        "1 MERGE",
        "MERGE operates on a LIST containing GTS or ENCODERs.",
    );
    assert_fails(
        "1 true MERGE",
        "MERGE operates on a LIST containing GTS or ENCODERs.",
    );
    // The sorted merge produces a GTS Encoder.
    assert_top(
        &format!("[ {SERIES} ] false MERGE ->JSON"),
        "{\"c\":\"g\",\"l\":{\"l\":\"v\"},\"a\":{},\"la\":0,\"v\":[[10,1],[20,47.99999998882413,1.9999999646097422,35,2],[30,5,3]]}",
    );
    assert_top(&format!("[ {SERIES} ] false MERGE TYPEOF"), "GTSENCODER");
}

#[test]
fn fuse_assembles_chunks() {
    assert_stack(
        &format!(
            "[ {SERIES} {SERIES} 5 TIMESHIFT ] FUSE DUP NAME SWAP DUP LABELS SWAP DUP ATTRIBUTES SWAP DUP TICKLIST SWAP VALUES"
        ),
        &[
            Value::from("g"),
            string_map(&[("l", "v")]),
            Value::map(IndexMap::new()),
            longs(&[10, 20, 30, 15, 25, 35]),
            longs(&[1, 2, 3, 1, 2, 3]),
        ],
    );
    // The name is kept when all chunks agree; labels whose value differs are dropped.
    assert_top(
        &format!("[ {SERIES} {SERIES} 'h' RENAME ] FUSE NAME"),
        Value::Null,
    );
    assert_top(&format!("[ {SERIES} NEWGTS ] FUSE NAME"), Value::Null);
    assert_stack(
        &format!("[ NEWGTS {SERIES} ] FUSE DUP NAME SWAP LABELS"),
        &[Value::Null, Value::map(IndexMap::new())],
    );
    assert_stack(
        "[ NEWGTS NEWGTS ] FUSE DUP NAME SWAP SIZE",
        &[Value::from(""), Value::Long(0)],
    );
    assert_top(
        &format!(
            "[ {SERIES} {{ 'l' 'w' 'm' '1' }} RELABEL {SERIES} {{ 'm' '1' 'n' '2' }} RELABEL ] FUSE LABELS"
        ),
        string_map(&[("m", "1")]),
    );
    // Types must agree, empty chunks excepted.
    assert_fails(
        &format!("[ {SERIES} {DOUBLES} ] FUSE"),
        "Inconsistent types for chunks to fuse.",
    );
    assert_fails(
        &format!("[ {SERIES} NEWGTS 1 NaN NaN NaN 'x' ADDVALUE ] FUSE"),
        "Inconsistent types for chunks to fuse.",
    );
    assert_top(
        &format!("[ {SERIES} NEWGTS 1 NaN NaN NaN 'x' ADDVALUE 1 REMOVETICK ] FUSE SIZE"),
        3,
    );
    // A series typed by an ignored value counts as typed.
    assert_fails(
        &format!("[ NEWGTS 1 NaN NaN NaN COUNTER ADDVALUE {SERIES} ] FUSE"),
        "Inconsistent types for chunks to fuse.",
    );
    assert_stack(
        "[ ] FUSE DUP NAME SWAP SIZE",
        &[Value::Null, Value::Long(0)],
    );
    // Compatible bucketizations are merged, incompatible ones dropped.
    assert_eq!(
        run_with(
            bucketized(),
            "[ $b $b 50 TIMESHIFT ] FUSE DUP BUCKETCOUNT SWAP DUP BUCKETSPAN SWAP LASTBUCKET"
        ),
        vec![Value::Long(15), Value::Long(10), Value::Long(150)]
    );
    assert_eq!(
        run_with(
            bucketized(),
            "[ $b 50 TIMESHIFT $b ] FUSE DUP BUCKETCOUNT SWAP DUP BUCKETSPAN SWAP LASTBUCKET"
        ),
        vec![Value::Long(15), Value::Long(10), Value::Long(150)]
    );
    assert_eq!(
        run_with(bucketized(), "[ $b $b 55 TIMESHIFT ] FUSE BUCKETCOUNT"),
        vec![Value::Long(0)]
    );
    let mut other_span = bucketized();
    other_span.bucketspan = 5;
    other_span.bucketcount = 20;
    let mut stack = test_stack();
    stack.push(Value::Gts(bucketized().shared()));
    stack.push(Value::Gts(other_span.shared()));
    stack.exec_checked("2 ->LIST FUSE BUCKETCOUNT").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::Long(0));
    assert_eq!(
        run_with(bucketized(), &format!("[ $b {SERIES} ] FUSE BUCKETCOUNT")),
        vec![Value::Long(0)]
    );
    assert_fails("[ 1 ] FUSE", "FUSE expects a list of GTS as input.");
    assert_fails("1 FUSE", "FUSE expects a list of GTS as input.");
}

#[test]
fn shrink_keeps_the_first_or_last_datapoints() {
    assert_top(&format!("{SERIES} 2 SHRINK TICKLIST"), longs(&[10, 20]));
    assert_top(&format!("{SERIES} -2 SHRINK TICKLIST"), longs(&[30, 20]));
    assert_top(&format!("{SERIES} 0 SHRINK SIZE"), 0);
    assert_top(&format!("{SERIES} 3 SHRINK TICKLIST"), longs(&[10, 20, 30]));
    assert_top(
        &format!("{SERIES} 20 48.0 2.0 35 2 SETVALUE -2 SHRINK ELEVATIONS"),
        Value::list(vec![Value::Long(5), Value::Long(35)]),
    );
    assert_top(
        &format!("{DOUBLES} 1 SHRINK VALUES"),
        Value::list(vec![Value::Double(0.5)]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE 1 SHRINK VALUES",
        Value::list(vec![Value::Boolean(true)]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE -1 SHRINK VALUES",
        strings(&["b"]),
    );
    // Out of range sizes leave the series alone.
    for size in [
        "-9223372036854775808",
        "-2147483648",
        "-2147483647",
        "4294967297",
    ] {
        assert_top(&format!("{SERIES} {size} SHRINK SIZE"), 3);
    }
    // The series is shrunk in place, through nested lists.
    assert_top(
        &format!("{SERIES} 'g' STORE [ $g [ $g ] ] 1 SHRINK DROP $g TICKLIST"),
        longs(&[10]),
    );
    let mut stack = exec(&format!("{SERIES} 'g' STORE $g 1 SHRINK $g"));
    let original = stack.pop().unwrap();
    assert!(stack.pop().unwrap().same_object(&original));
    assert_eq!(
        crate::tests::top("[ [ ] [ [ ] ] ] -1 SHRINK").to_string(),
        "[[], [[]]]"
    );
    assert_fails(
        "[ 1 ] 1 SHRINK",
        "SHRINK can only handle GTS and list thereof.",
    );
    assert_fails("1 1 SHRINK", "SHRINK can only handle GTS and list thereof.");
    assert_fails(
        "[ ] 'a' SHRINK",
        "SHRINK expects a size on top of the stack.",
    );
}

#[test]
fn functions_fail_on_an_empty_stack() {
    let cases: &[(&str, &[&str])] = &[
        ("TIMESHIFT", &["", "1"]),
        ("TIMESCALE", &["", "1"]),
        ("TIMEMODULO", &["", "'q'", "1 'q'"]),
        ("TIMECLIP", &["", "1", "1 1"]),
        ("CLIP", &["", "[ ]"]),
        ("TIMESPLIT", &["", "'s'", "1 's'", "1 1 's'"]),
        (
            "CHUNK",
            &[
                "",
                "true",
                "'c' true",
                "0 'c' true",
                "0 0 'c' true",
                "10 0 0 'c' true",
                "0 10 0 0 'c' true",
            ],
        ),
        ("CROP", &[""]),
        ("MERGE", &["", "true"]),
        ("FUSE", &[""]),
        ("SHRINK", &["", "1"]),
    ];
    for (name, prefixes) in cases {
        for prefix in *prefixes {
            assert_fails(&format!("{prefix} {name}"), "Empty stack.");
        }
    }
}
