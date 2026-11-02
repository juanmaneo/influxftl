// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the statistics and value-based transforms. The expected values were checked
//! against Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use indexmap::IndexMap;

use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_stack, assert_top, test_stack};
use crate::value::Value;

/// A `LONG` series `g{l=v}` with three datapoints, the second one located and elevated.
const SERIES: &str = "NEWGTS 'g' RENAME { 'l' 'v' } RELABEL \
    10 NaN NaN NaN 1 ADDVALUE 20 48.0 2.0 35 2 ADDVALUE 30 NaN NaN 5 3 ADDVALUE";

/// A `DOUBLE` series.
const DOUBLES: &str = "NEWGTS 'g' RENAME 1 NaN NaN NaN 2.5 ADDVALUE 2 NaN NaN NaN 1.5 ADDVALUE \
    3 NaN NaN NaN 4.0 ADDVALUE 4 NaN NaN NaN 1.0 ADDVALUE";

/// A `LONG` series with a geometric progression.
const POWERS: &str = "NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 4 ADDVALUE \
    4 NaN NaN NaN 8 ADDVALUE 5 NaN NaN NaN 16 ADDVALUE";

fn longs(values: &[i64]) -> Value {
    Value::list(values.iter().copied().map(Value::Long).collect())
}

fn doubles(values: &[f64]) -> Value {
    Value::list(values.iter().copied().map(Value::Double).collect())
}

fn strings(values: &[&str]) -> Value {
    Value::list(values.iter().copied().map(Value::from).collect())
}

/// `[ SERIES bucketizer.sum 100 10 0 ] BUCKETIZE 0 GET` as Warp 10 builds it.
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
fn standardize_centers_and_scales() {
    assert_top(
        &format!("{SERIES} STANDARDIZE VALUES"),
        doubles(&[-0.999_999_999_999_999_8, 0.0, 0.999_999_999_999_999_8]),
    );
    assert_top(
        &format!("{DOUBLES} STANDARDIZE VALUES"),
        doubles(&[
            0.188_982_236_504_613_6,
            -0.566_946_709_513_840_9,
            1.322_875_655_532_295_1,
            -0.944_911_182_523_068,
        ]),
    );
    // Metadata, location and elevation are kept; the input is untouched.
    assert_stack(
        &format!("{SERIES} 'g' STORE $g STANDARDIZE DUP ELEVATIONS SWAP BUCKETCOUNT $g VALUES"),
        &[
            Value::list(vec![
                Value::Double(f64::NAN),
                Value::Long(35),
                Value::Long(5),
            ]),
            Value::Long(0),
            longs(&[1, 2, 3]),
        ],
    );
    // A constant series has a null deviation and is only centered.
    assert_top(
        "NEWGTS 1 NaN NaN NaN 5 ADDVALUE 2 NaN NaN NaN 5 ADDVALUE STANDARDIZE VALUES",
        doubles(&[0.0, 0.0]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 5 ADDVALUE STANDARDIZE VALUES",
        doubles(&[0.0]),
    );
    // Non numeric or empty series are cloned.
    assert_stack(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE STANDARDIZE VALUES NEWGTS STANDARDIZE SIZE",
        &[strings(&["a"]), Value::Long(0)],
    );
    assert_top(&format!("{SERIES} 0 SHRINK STANDARDIZE SIZE"), 0);
    assert_top(
        &format!("[ {SERIES} NEWGTS ] STANDARDIZE <% SIZE %> false LMAP"),
        longs(&[3, 0]),
    );
    assert_fails(
        "1 STANDARDIZE",
        "STANDARDIZE can only operate on Geo Time Series instances.",
    );
}

#[test]
fn normalize_maps_onto_the_unit_interval() {
    assert_top(
        &format!("{SERIES} NORMALIZE VALUES"),
        doubles(&[0.0, 0.5, 1.0]),
    );
    assert_top(
        &format!("{DOUBLES} NORMALIZE VALUES"),
        doubles(&[0.5, 0.166_666_666_666_666_66, 1.0, 0.0]),
    );
    assert_top(
        &format!("{SERIES} ISONORMALIZE VALUES"),
        doubles(&[-0.5, 0.0, 0.5]),
    );
    assert_top(
        &format!("{DOUBLES} ISONORMALIZE VALUES"),
        doubles(&[
            0.083_333_333_333_333_33,
            -0.25,
            0.583_333_333_333_333_4,
            -0.416_666_666_666_666_7,
        ]),
    );
    // Constant series map onto `1.0`; `NaN`s do not count as extrema.
    assert_stack(
        "NEWGTS 1 NaN NaN NaN 5 ADDVALUE 2 NaN NaN NaN 5 ADDVALUE NORMALIZE VALUES \
         NEWGTS 1 NaN NaN NaN 5.5 ADDVALUE ISONORMALIZE VALUES",
        &[doubles(&[1.0, 1.0]), doubles(&[1.0])],
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN NaN ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE NORMALIZE VALUES",
        doubles(&[1.0, 1.0]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN NaN ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE 3 NaN NaN NaN 3.0 ADDVALUE ISONORMALIZE VALUES",
        doubles(&[f64::NAN, f64::NAN, f64::NAN]),
    );
    // The range of a `LONG` series is a long subtraction, which may wrap.
    assert_top(
        "NEWGTS 1 NaN NaN NaN -9223372036854775808 ADDVALUE 2 NaN NaN NaN 9223372036854775807 ADDVALUE NORMALIZE VALUES",
        doubles(&[-0.0, 1.0]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 3 ADDVALUE 3 NaN NaN NaN 2 ADDVALUE ISONORMALIZE VALUES",
        doubles(&[-0.5, 0.5, 0.0]),
    );
    assert_stack(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE NORMALIZE VALUES NEWGTS ISONORMALIZE SIZE",
        &[strings(&["a"]), Value::Long(0)],
    );
    assert_top(
        &format!("[ {SERIES} {DOUBLES} ] NORMALIZE <% VALUES %> false LMAP"),
        Value::list(vec![
            doubles(&[0.0, 0.5, 1.0]),
            doubles(&[0.5, 0.166_666_666_666_666_66, 1.0, 0.0]),
        ]),
    );
    assert_fails(
        "1 NORMALIZE",
        "NORMALIZE can only operate on Geo Time Series instances.",
    );
    assert_fails(
        "1 ISONORMALIZE",
        "ISONORMALIZE can only operate on Geo Time Series instances.",
    );
}

#[test]
fn zscore_by_mean_or_median() {
    assert_top(
        &format!("{DOUBLES} false ZSCORE VALUES"),
        doubles(&[
            0.188_982_236_504_613_6,
            -0.566_946_709_513_840_9,
            1.322_875_655_532_295_1,
            -0.944_911_182_523_068,
        ]),
    );
    assert_top(
        &format!("{DOUBLES} true ZSCORE VALUES"),
        doubles(&[
            0.449_666_666_666_666_6,
            -0.449_666_666_666_666_6,
            1.798_666_666_666_666_4,
            -0.899_333_333_333_333_2,
        ]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 2.0 ADDVALUE 3 NaN NaN NaN 3.0 ADDVALUE true ZSCORE VALUES",
        doubles(&[-0.6745, 0.0, 0.6745]),
    );
    // A null median absolute deviation falls back to the standard deviation.
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE 3 NaN NaN NaN 5.0 ADDVALUE true ZSCORE VALUES",
        doubles(&[0.0, 0.0, 1.732_050_807_568_877_4]),
    );
    // The input is untouched; location and elevation are kept.
    assert_stack(
        "NEWGTS 1 1.0 2.0 7 1.0 ADDVALUE 2 NaN NaN NaN 3.0 ADDVALUE 'g' STORE $g false ZSCORE ELEVATIONS $g VALUES",
        &[
            Value::list(vec![Value::Long(7), Value::Double(f64::NAN)]),
            doubles(&[1.0, 3.0]),
        ],
    );
    assert_fails(
        "NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE true ZSCORE",
        "Median Absolute Deviation is null",
    );
    assert_fails(
        "NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 2 NaN NaN NaN 1.0 ADDVALUE false ZSCORE",
        "Standard Deviation is null",
    );
    assert_fails(
        &format!("{SERIES} false ZSCORE"),
        "GTS must be of type Double.",
    );
    assert_fails(
        &format!("[ {DOUBLES} {SERIES} ] false ZSCORE"),
        "GTS must be of type Double.",
    );
    // A `DOUBLE` series without datapoint: the median cannot be read, the mean is `NaN`.
    assert_fails(
        "NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 0 SHRINK true ZSCORE",
        "Index 0 out of bounds for length 0",
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1.0 ADDVALUE 0 SHRINK false ZSCORE SIZE",
        0,
    );
    assert_fails(
        &format!("{DOUBLES} 'x' ZSCORE"),
        "ZSCORE expects a flag (a BOOLEAN) on top of the stack indicating whether to normalize by the mean (F), or by the median (T).",
    );
    assert_fails(
        "1 true ZSCORE",
        "ZSCORE can only operate on Geo Time Series instances.",
    );
}

#[test]
fn skewness_and_kurtosis() {
    assert_stack(
        &format!(
            "{SERIES} true SKEWNESS {SERIES} false SKEWNESS {DOUBLES} true SKEWNESS {DOUBLES} false KURTOSIS {SERIES} true KURTOSIS"
        ),
        &[
            Value::Double(0.0),
            Value::Double(0.0),
            Value::Double(0.323_969_548_293_623_2),
            Value::Double(1.761_904_761_904_762_3),
            Value::Double(0.666_666_666_666_666_1),
        ],
    );
    assert_top(&format!("{POWERS} false SKEWNESS"), 0.889_048_134_816_954_4);
    assert_top(&format!("{POWERS} true KURTOSIS"), 1.488_602_150_537_634_6);
    // A single value has a null deviation; squares of longs may overflow.
    assert_top("NEWGTS 1 NaN NaN NaN 5 ADDVALUE true SKEWNESS", f64::NAN);
    assert_top(
        "NEWGTS 1 NaN NaN NaN 3037000500 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE false SKEWNESS",
        f64::NAN,
    );
    assert_top(
        &format!("[ {SERIES} {DOUBLES} ] true SKEWNESS"),
        doubles(&[0.0, 0.323_969_548_293_623_2]),
    );
    assert_fails(
        "NEWGTS true SKEWNESS",
        "SKEWNESS can only compute skewness for non empty series.",
    );
    assert_fails(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE true KURTOSIS",
        "KURTOSIS can only compute kurtosis for numerical series.",
    );
    assert_fails(
        &format!("{SERIES} 1 SKEWNESS"),
        "SKEWNESS expects a boolean on top of the stack to determine if Bessel's correction should be applied or not.",
    );
    assert_fails(
        "1 true KURTOSIS",
        "KURTOSIS can only operate on Geo Time Series instances.",
    );
}

#[test]
fn mode_lists_the_most_frequent_values() {
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 2 ADDVALUE \
         4 NaN NaN NaN 1 ADDVALUE 5 NaN NaN NaN 3 ADDVALUE MODE",
        longs(&[1, 2]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1 ADDVALUE 2 NaN NaN NaN 2 ADDVALUE 3 NaN NaN NaN 2 ADDVALUE MODE",
        longs(&[2]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 3 ADDVALUE 2 NaN NaN NaN 3 ADDVALUE 3 NaN NaN NaN 2 ADDVALUE MODE",
        longs(&[3]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN true ADDVALUE 2 NaN NaN NaN false ADDVALUE 3 NaN NaN NaN false ADDVALUE MODE",
        Value::list(vec![Value::Boolean(false)]),
    );
    // Values are compared by value (Warp 10 compares the boxed objects by reference).
    assert_top(
        "NEWGTS 1 NaN NaN NaN 1.5 ADDVALUE 2 NaN NaN NaN 1.5 ADDVALUE 3 NaN NaN NaN 2.5 ADDVALUE MODE",
        doubles(&[1.5]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'a' ADDVALUE 3 NaN NaN NaN 'b' ADDVALUE MODE",
        strings(&["a"]),
    );
    // The series is value-sorted in place.
    assert_stack(
        "NEWGTS 'g' STORE $g 1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 1 ADDVALUE MODE $g VALUES",
        &[longs(&[1, 2]), longs(&[1, 2])],
    );
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE NEWGTS ] MODE",
        Value::list(vec![longs(&[1]), longs(&[])]),
    );
    assert_fails(
        "1 MODE",
        "MODE can only operate on Geo Time Series instances.",
    );
}

#[test]
fn valuesplit_groups_by_value() {
    assert_top(
        &format!(
            "{DOUBLES} 'v' VALUESPLIT <% DUP LABELS 'v' GET SWAP TICKLIST 2 ->LIST %> false LMAP"
        ),
        Value::list(vec![
            Value::list(vec![Value::from("1.0"), longs(&[4])]),
            Value::list(vec![Value::from("1.5"), longs(&[2])]),
            Value::list(vec![Value::from("2.5"), longs(&[1])]),
            Value::list(vec![Value::from("4.0"), longs(&[3])]),
        ]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 'b' ADDVALUE 2 NaN NaN NaN 'a' ADDVALUE 3 NaN NaN NaN 'b' ADDVALUE \
         'v' VALUESPLIT <% DUP LABELS 'v' GET SWAP TICKLIST 2 ->LIST %> false LMAP",
        Value::list(vec![
            Value::list(vec![Value::from("a"), longs(&[2])]),
            Value::list(vec![Value::from("b"), longs(&[1, 3])]),
        ]),
    );
    // Booleans: the `false` then the `true` datapoints, empty halves left out.
    assert_top(
        "NEWGTS 2 NaN NaN NaN true ADDVALUE 1 NaN NaN NaN false ADDVALUE 3 NaN NaN NaN true ADDVALUE \
         'v' VALUESPLIT <% DUP LABELS 'v' GET SWAP TICKLIST 2 ->LIST %> false LMAP",
        Value::list(vec![
            Value::list(vec![Value::from("false"), longs(&[1])]),
            Value::list(vec![Value::from("true"), longs(&[2, 3])]),
        ]),
    );
    assert_top(
        "NEWGTS 2 NaN NaN NaN true ADDVALUE 3 NaN NaN NaN true ADDVALUE 'v' VALUESPLIT \
         <% DUP LABELS 'v' GET SWAP TICKLIST 2 ->LIST %> false LMAP",
        Value::list(vec![Value::list(vec![Value::from("true"), longs(&[2, 3])])]),
    );
    assert_top(
        "NEWGTS 2 NaN NaN NaN false ADDVALUE 'v' VALUESPLIT <% DUP LABELS 'v' GET SWAP TICKLIST 2 ->LIST %> false LMAP",
        Value::list(vec![Value::list(vec![Value::from("false"), longs(&[2])])]),
    );
    assert_stack(
        "NEWGTS 'v' VALUESPLIT SIZE NEWGTS 1 NaN NaN NaN COUNTER ADDVALUE 'v' VALUESPLIT SIZE",
        &[Value::Long(0), Value::Long(0)],
    );
    // The input is value-sorted in place; an existing label is overwritten.
    assert_top(
        &format!("{DOUBLES} 'g' STORE $g 'v' VALUESPLIT DROP $g VALUES"),
        doubles(&[1.0, 1.5, 2.5, 4.0]),
    );
    assert_top(
        &format!("{SERIES} 'l' VALUESPLIT <% LABELS 'l' GET %> false LMAP"),
        strings(&["1", "2", "3"]),
    );
    assert_fails(
        &format!("{SERIES} 1 VALUESPLIT"),
        "VALUESPLIT expects a label name on top of the stack.",
    );
    assert_fails(
        "1 'v' VALUESPLIT",
        "VALUESPLIT can only operate on Geo Time Series instances.",
    );
}

#[test]
fn valuehistogram_counts_values() {
    assert_top(
        &format!("{SERIES} VALUEHISTOGRAM"),
        Value::map(IndexMap::from([
            (Value::Long(1), Value::Long(1)),
            (Value::Long(2), Value::Long(1)),
            (Value::Long(3), Value::Long(1)),
        ])),
    );
    assert_top(
        &format!("{DOUBLES} 2 NaN NaN NaN 1.5 ADDVALUE VALUEHISTOGRAM ->JSON"),
        "{\"2.5\":1,\"1.5\":2,\"4.0\":1,\"1.0\":1}",
    );
    // A bucketized series reports its empty buckets under `null`.
    assert_eq!(
        run_with(
            bucketized(),
            "$b VALUEHISTOGRAM ->JSON $b 25 NaN NaN NaN 9 ADDVALUE VALUEHISTOGRAM ->JSON"
        ),
        vec![
            Value::from("{\"3\":1,\"2\":1,\"1\":1,\"null\":7}"),
            Value::from("{\"3\":1,\"2\":1,\"1\":1,\"9\":1,\"null\":6}"),
        ]
    );
    assert_stack(
        &format!(
            "NEWGTS VALUEHISTOGRAM [ {SERIES} NEWGTS 1 NaN NaN NaN true ADDVALUE ] VALUEHISTOGRAM ->JSON"
        ),
        &[
            Value::map(IndexMap::new()),
            Value::from("[{\"1\":1,\"2\":1,\"3\":1},{\"true\":1}]"),
        ],
    );
    assert_fails(
        &format!("[ {SERIES} 1 ] VALUEHISTOGRAM"),
        "VALUEHISTOGRAM can only operate on Geo Time Series or GTS Encoder instances.",
    );
    assert_fails(
        "1 VALUEHISTOGRAM",
        "VALUEHISTOGRAM can only operate on Geo Time Series instances.",
    );
    // The offending argument is pushed back.
    let mut stack = test_stack();
    assert!(stack.exec_checked("1 VALUEHISTOGRAM").is_err());
    assert_eq!(stack.pop().unwrap(), Value::Long(1));
}

#[test]
fn quantize_ranks_values_between_bounds() {
    assert_top(
        &format!("{DOUBLES} [ 1.5 2.5 ] [ ] QUANTIZE VALUES"),
        longs(&[1, 0, 2, 0]),
    );
    assert_top(
        &format!("{DOUBLES} [ 1.5 2.5 ] [ 'low' 'mid' 'high' ] QUANTIZE VALUES"),
        strings(&["mid", "low", "high", "low"]),
    );
    assert_top(
        &format!("{SERIES} [ 1.5 2.5 ] [ ] QUANTIZE VALUES"),
        longs(&[0, 1, 2]),
    );
    assert_top(
        &format!("{SERIES} [ 1 2 ] [ 10 20 30 ] QUANTIZE VALUES"),
        longs(&[10, 20, 30]),
    );
    assert_top(
        &format!("{SERIES} [ ] [ ] QUANTIZE VALUES"),
        longs(&[0, 0, 0]),
    );
    assert_top(
        &format!("{SERIES} [ ] [ 'x' ] QUANTIZE VALUES"),
        strings(&["x", "x", "x"]),
    );
    // `Arrays.binarySearch` order: `-0.0` before `0.0`, `NaN` last.
    assert_top(
        "NEWGTS 1 NaN NaN NaN NaN ADDVALUE 2 NaN NaN NaN -0.0 ADDVALUE 3 NaN NaN NaN 0.0 ADDVALUE \
         [ 0.0 ] [ ] QUANTIZE VALUES",
        longs(&[1, 0, 0]),
    );
    assert_top(
        "NEWGTS 1 NaN NaN NaN 0.0 ADDVALUE 2 NaN NaN NaN 5.0 ADDVALUE 3 NaN NaN NaN 1.0 ADDVALUE \
         [ -0.0 1.0 2.0 3.0 ] [ ] QUANTIZE VALUES",
        longs(&[1, 4, 1]),
    );
    // Metadata, location and elevation are kept.
    assert_stack(
        &format!("{SERIES} 'g' STORE $g [ 1 ] [ ] QUANTIZE DUP LABELS SWAP ELEVATIONS"),
        &[
            Value::map(IndexMap::from([(Value::from("l"), Value::from("v"))])),
            Value::list(vec![
                Value::Double(f64::NAN),
                Value::Long(35),
                Value::Long(5),
            ]),
        ],
    );
    assert_fails(
        &format!("{SERIES} [ 1 2 ] [ 'a' 'b' ] QUANTIZE"),
        "QUANTIZE expected 3 values but got 2",
    );
    assert_fails(
        &format!("{SERIES} [ 1 'x' ] [ ] QUANTIZE"),
        "class java.lang.String cannot be cast to class java.lang.Number",
    );
    assert_fails(
        &format!("{SERIES} [ 1 NaN ] [ ] QUANTIZE"),
        "QUANTIZE expects the bounds to be finite.",
    );
    for bounds in ["[ 2 1 ]", "[ 1 1 ]"] {
        assert_fails(
            &format!("{SERIES} {bounds} [ ] QUANTIZE"),
            "QUANTIZE identified unordered or duplicate bounds.",
        );
    }
    assert_fails(
        &format!("{SERIES} 1 [ ] QUANTIZE"),
        "QUANTIZE expects a list of bounds under the top of the stack.",
    );
    assert_fails(
        &format!("{SERIES} [ ] 1 QUANTIZE"),
        "QUANTIZE expects a list of target values on top of the stack.",
    );
    assert_fails(
        "NEWGTS 1 NaN NaN NaN 'a' ADDVALUE [ ] [ ] QUANTIZE",
        "Can only quantify numeric Geo Time Series.",
    );
    assert_fails(
        &format!("[ {SERIES} NEWGTS ] [ 2 ] [ ] QUANTIZE"),
        "Can only quantify numeric Geo Time Series.",
    );
    assert_fails(
        "[ 1 ] [ ] [ ] QUANTIZE",
        "QUANTIZE expects a Geo Time Series instance or a list thereof.",
    );
    assert_top(
        &format!("[ {SERIES} {DOUBLES} ] [ 2 ] [ ] QUANTIZE <% VALUES %> false LMAP"),
        Value::list(vec![longs(&[0, 0, 1]), longs(&[1, 0, 1, 0])]),
    );
}

#[test]
fn locstrings_are_hexadecimal_hhcodes() {
    assert_top(
        &format!("{SERIES} LOCSTRINGS"),
        strings(&["", "e021347021347021", ""]),
    );
    // The series is sorted in place first.
    assert_stack(
        "NEWGTS 3 -45.0 -90.0 NaN 1 ADDVALUE 1 48.8566 2.3522 NaN 1 ADDVALUE 2 0 0 NaN 1 ADDVALUE \
         'g' STORE $g LOCSTRINGS $g TICKLIST",
        &[
            strings(&["e0236ef00d321db5", "c000000000000000", "3000000000000000"]),
            longs(&[1, 2, 3]),
        ],
    );
    assert_top("NEWGTS LOCSTRINGS", strings(&[]));
    assert_fails(
        "1 LOCSTRINGS",
        "LOCSTRINGS expects a Geo Time Series on top of the stack.",
    );
}

#[test]
fn functions_fail_on_an_empty_stack() {
    let cases: &[(&str, &[&str])] = &[
        ("STANDARDIZE", &[""]),
        ("NORMALIZE", &[""]),
        ("ISONORMALIZE", &[""]),
        ("ZSCORE", &["", "true"]),
        ("SKEWNESS", &["", "true"]),
        ("KURTOSIS", &["", "true"]),
        ("MODE", &[""]),
        ("VALUESPLIT", &["", "'v'"]),
        ("VALUEHISTOGRAM", &[""]),
        ("QUANTIZE", &["", "[ ]", "[ ] [ ]"]),
        ("LOCSTRINGS", &[""]),
    ];
    for (name, prefixes) in cases {
        for prefix in *prefixes {
            assert_fails(&format!("{prefix} {name}"), "Empty stack.");
        }
    }
}
