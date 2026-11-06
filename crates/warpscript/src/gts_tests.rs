// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the Geo Time Series model.

use std::rc::Rc;

use crate::geoxp::to_geoxp_point;
use crate::gts::{
    GeoTimeSerie, NO_ELEVATION, NO_LOCATION, Type, Values, encode_value, java_binary_search,
};
use crate::value::Value;

/// A series with the given ticks and `LONG` values `v0, v1…` (as their index).
fn series(ticks: &[i64]) -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::new();
    for (i, tick) in ticks.iter().enumerate() {
        gts.set_value(
            *tick,
            NO_LOCATION,
            NO_ELEVATION,
            &Value::Long(i as i64),
            false,
        );
    }
    gts
}

fn longs(gts: &GeoTimeSerie) -> Vec<i64> {
    match &gts.values {
        Values::Long(v) => v.clone(),
        other => panic!("not a LONG series: {other:?}"),
    }
}

#[test]
fn first_value_fixes_the_type_and_later_ones_are_converted() {
    let mut gts = GeoTimeSerie::new();
    assert_eq!(gts.type_of(), Type::Undefined);
    assert_eq!(gts.value_at(0), Value::Null);
    // LONG series.
    gts.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Long(7), false);
    gts.set_value(2, NO_LOCATION, NO_ELEVATION, &Value::Double(2.9), false);
    gts.set_value(3, NO_LOCATION, NO_ELEVATION, &Value::Boolean(true), false);
    gts.set_value(4, NO_LOCATION, NO_ELEVATION, &Value::from("12"), false);
    gts.set_value(5, NO_LOCATION, NO_ELEVATION, &Value::from("3.7"), false);
    gts.set_value(6, NO_LOCATION, NO_ELEVATION, &Value::from("abc"), false);
    gts.set_value(7, NO_LOCATION, NO_ELEVATION, &Value::Null, false);
    gts.set_value(8, NO_LOCATION, NO_ELEVATION, &Value::list(vec![]), false);
    assert_eq!(gts.type_of(), Type::Long);
    assert_eq!(longs(&gts), vec![7, 2, 1, 12, 3, 0]);
    assert_eq!(gts.ticks, vec![1, 2, 3, 4, 5, 6]);
    // DOUBLE series.
    let mut gts = GeoTimeSerie::new();
    gts.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Double(1.5), false);
    gts.set_value(2, NO_LOCATION, NO_ELEVATION, &Value::Long(2), false);
    gts.set_value(3, NO_LOCATION, NO_ELEVATION, &Value::Boolean(false), false);
    gts.set_value(4, NO_LOCATION, NO_ELEVATION, &Value::from(" 4e1 "), false);
    gts.set_value(5, NO_LOCATION, NO_ELEVATION, &Value::from("x"), false);
    assert_eq!(gts.type_of(), Type::Double);
    assert!(matches!(&gts.values, Values::Double(v) if *v == vec![1.5, 2.0, 0.0, 40.0, 0.0]));
    // BOOLEAN series.
    let mut gts = GeoTimeSerie::new();
    gts.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Boolean(true), false);
    gts.set_value(2, NO_LOCATION, NO_ELEVATION, &Value::Long(0), false);
    gts.set_value(
        3,
        NO_LOCATION,
        NO_ELEVATION,
        &Value::Double(f64::NAN),
        false,
    );
    gts.set_value(4, NO_LOCATION, NO_ELEVATION, &Value::from(""), false);
    gts.set_value(5, NO_LOCATION, NO_ELEVATION, &Value::from("F"), false);
    assert_eq!(gts.type_of(), Type::Boolean);
    assert!(
        matches!(&gts.values, Values::Boolean(v) if *v == vec![true, false, true, false, true])
    );
    // STRING series; byte arrays are decoded as ISO-8859-1.
    let mut gts = GeoTimeSerie::new();
    gts.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::from("a"), false);
    gts.set_value(2, NO_LOCATION, NO_ELEVATION, &Value::Long(2), false);
    gts.set_value(3, NO_LOCATION, NO_ELEVATION, &Value::Double(0.5), false);
    gts.set_value(4, NO_LOCATION, NO_ELEVATION, &Value::Boolean(true), false);
    gts.set_value(
        5,
        NO_LOCATION,
        NO_ELEVATION,
        &Value::Bytes(Rc::from([0xE9u8, 0x41])),
        false,
    );
    assert_eq!(gts.type_of(), Type::String);
    let texts: Vec<String> = (0..5).map(|i| gts.value_at(i).to_string()).collect();
    assert_eq!(texts, vec!["a", "2", "0.5", "T", "éA"]);
    // An unsupported first value still fixes the type to BOOLEAN, like Warp 10.
    let mut gts = GeoTimeSerie::new();
    gts.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::list(vec![]), false);
    assert_eq!(gts.type_of(), Type::Boolean);
    assert_eq!(gts.size(), 0);
}

#[test]
fn locations_and_elevations_appear_on_demand() {
    let mut gts = GeoTimeSerie::new();
    gts.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Long(1), false);
    assert!(gts.locations.is_none() && gts.elevations.is_none());
    assert_eq!(gts.location_at(0), NO_LOCATION);
    assert_eq!(gts.elevation_at(0), NO_ELEVATION);
    let paris = to_geoxp_point(48.8566, 2.3522);
    gts.set_value(2, paris, 35, &Value::Long(2), false);
    assert_eq!(gts.locations.as_deref(), Some(&[NO_LOCATION, paris][..]));
    assert_eq!(gts.elevations.as_deref(), Some(&[NO_ELEVATION, 35][..]));
    gts.set_value(3, NO_LOCATION, NO_ELEVATION, &Value::Long(3), false);
    assert_eq!(gts.location_at(2), NO_LOCATION);
    assert_eq!(gts.elevation_at(9), NO_ELEVATION);
    assert_eq!(gts.tick_at(9), i64::MIN);
}

#[test]
fn sorted_flag_follows_the_appended_ticks() {
    // A fresh series is not flagged as sorted (`sorted = false` in `GeoTimeSerie`).
    let mut gts = series(&[]);
    assert!(!gts.sorted && !gts.reversed);
    gts = series(&[5]);
    assert!(gts.sorted && !gts.reversed);
    gts = series(&[5, 3]);
    assert!(gts.sorted && gts.reversed);
    gts = series(&[5, 3, 3, 1]);
    assert!(gts.sorted && gts.reversed);
    gts = series(&[5, 3, 4]);
    assert!(!gts.sorted);
    gts = series(&[1, 2, 2, 3]);
    assert!(gts.sorted && !gts.reversed);
    gts = series(&[1, 2, 1]);
    assert!(!gts.sorted);
    // Once unsorted, stays unsorted.
    gts.set_value(9, NO_LOCATION, NO_ELEVATION, &Value::Long(9), false);
    assert!(!gts.sorted);
}

#[test]
fn quicksort_permutes_like_warp10() {
    // Expected orders of the values computed with `GTSHelper.quicksort`.
    let cases: [(&[i64], bool, &[i64]); 6] = [
        (&[3, 1, 2, 1], false, &[3, 1, 2, 0]),
        (&[3, 1, 2, 1], true, &[0, 2, 3, 1]),
        (&[5, 5, 5, 1, 5, 2, 5], false, &[3, 5, 4, 6, 1, 2, 0]),
        (&[1, 2, 3, 4, 5, 6, 7, 8], true, &[7, 6, 5, 4, 3, 2, 1, 0]),
        (&[2, 2, 1, 1, 3, 3, 2, 1], false, &[3, 7, 2, 6, 0, 1, 4, 5]),
        (&[7], false, &[0]),
    ];
    for (ticks, reversed, expected) in cases {
        let mut gts = series(ticks);
        // Force the quicksort even when the series is already sorted.
        gts.sorted = false;
        gts.sort(reversed);
        assert_eq!(
            longs(&gts),
            expected.to_vec(),
            "{ticks:?} reversed={reversed}"
        );
        assert!(gts.sorted && gts.reversed == reversed);
        let mut sorted_ticks = ticks.to_vec();
        sorted_ticks.sort_unstable();
        if reversed {
            sorted_ticks.reverse();
        }
        assert_eq!(gts.ticks, sorted_ticks);
    }
    // Sorting an empty series.
    let mut gts = series(&[]);
    gts.sorted = false;
    gts.sort(false);
    assert!(gts.sorted);
}

#[test]
fn sorting_a_sorted_series_reverses_or_does_nothing() {
    let paris = to_geoxp_point(48.8566, 2.3522);
    let mut gts = GeoTimeSerie::new();
    gts.set_value(1, paris, 10, &Value::from("a"), false);
    gts.set_value(2, NO_LOCATION, 20, &Value::from("b"), false);
    gts.set_value(3, NO_LOCATION, NO_ELEVATION, &Value::from("c"), false);
    gts.sort(true);
    assert_eq!(gts.ticks, vec![3, 2, 1]);
    assert_eq!(gts.location_at(2), paris);
    assert_eq!(gts.elevations.as_deref(), Some(&[NO_ELEVATION, 20, 10][..]));
    assert_eq!(gts.value_at(0), Value::from("c"));
    // Already sorted in that direction: untouched.
    gts.sort(true);
    assert_eq!(gts.ticks, vec![3, 2, 1]);
    gts.sort(false);
    assert_eq!(gts.ticks, vec![1, 2, 3]);
    // Every value type reverses.
    for value in [Value::Long(1), Value::Double(1.0), Value::Boolean(true)] {
        let mut gts = GeoTimeSerie::new();
        gts.set_value(1, NO_LOCATION, NO_ELEVATION, &value, false);
        gts.set_value(2, NO_LOCATION, NO_ELEVATION, &Value::Null, false);
        gts.set_value(2, NO_LOCATION, NO_ELEVATION, &Value::Boolean(false), false);
        gts.sort(true);
        assert_eq!(gts.ticks, vec![2, 1]);
        assert_eq!(gts.value_at(1), value);
    }
    let mut empty = GeoTimeSerie::new();
    empty.sort(true);
    assert!(empty.reversed);
}

#[test]
fn overwriting_finds_the_first_datapoint_at_the_tick() {
    // Sorted: binary search (ascending and descending).
    let mut gts = series(&[1, 2, 2, 3]);
    gts.set_value(2, NO_LOCATION, 5, &Value::Long(20), true);
    assert_eq!(longs(&gts), vec![0, 20, 2, 3]);
    assert_eq!(
        gts.elevations.as_deref(),
        Some(&[NO_ELEVATION, 5, NO_ELEVATION, NO_ELEVATION][..])
    );
    gts.set_value(7, NO_LOCATION, NO_ELEVATION, &Value::Long(70), true);
    assert_eq!(gts.ticks, vec![1, 2, 2, 3, 7]);
    let mut gts = series(&[3, 2, 2, 1]);
    gts.set_value(2, NO_LOCATION, NO_ELEVATION, &Value::Long(20), true);
    assert_eq!(longs(&gts), vec![0, 20, 2, 3]);
    gts.set_value(0, NO_LOCATION, NO_ELEVATION, &Value::Long(9), true);
    assert_eq!(gts.ticks, vec![3, 2, 2, 1, 0]);
    // Unsorted: scan.
    let mut gts = series(&[3, 1, 2, 1]);
    gts.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Long(10), true);
    assert_eq!(longs(&gts), vec![0, 10, 2, 3]);
    gts.set_value(5, NO_LOCATION, NO_ELEVATION, &Value::Long(50), true);
    assert_eq!(gts.ticks, vec![3, 1, 2, 1, 5]);
    // The type is fixed by the first value even when overwriting.
    let mut gts = GeoTimeSerie::new();
    gts.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Double(1.0), true);
    assert_eq!(gts.type_of(), Type::Double);
}

#[test]
fn first_and_last_ticks() {
    let empty = series(&[]);
    assert_eq!(empty.first_tick(), i64::MAX);
    assert_eq!(empty.last_tick(), i64::MIN);
    let sorted = series(&[1, 5, 9]);
    assert_eq!((sorted.first_tick(), sorted.last_tick()), (1, 9));
    let reversed = series(&[9, 5, 1]);
    assert_eq!((reversed.first_tick(), reversed.last_tick()), (1, 9));
    let unsorted = series(&[5, 9, 1]);
    assert_eq!((unsorted.first_tick(), unsorted.last_tick()), (1, 9));
    let mut bucketized = series(&[5, 9, 1]);
    bucketized.bucketspan = 10;
    bucketized.bucketcount = 4;
    bucketized.lastbucket = 100;
    assert!(bucketized.is_bucketized());
    assert_eq!((bucketized.first_tick(), bucketized.last_tick()), (70, 100));
}

#[test]
fn clone_empty_keeps_metadata_and_buckets() {
    let mut gts = series(&[1, 2]);
    gts.metadata.name = Some("name".into());
    gts.metadata.labels.insert("a".into(), "b".into());
    gts.bucketspan = 10;
    gts.bucketcount = 2;
    gts.lastbucket = 20;
    let empty = gts.clone_empty();
    assert_eq!(empty.size(), 0);
    assert_eq!(empty.type_of(), Type::Undefined);
    assert_eq!(empty.name(), Some("name"));
    assert_eq!(
        empty.metadata.labels.get("a").map(String::as_str),
        Some("b")
    );
    assert_eq!(
        (empty.bucketspan, empty.bucketcount, empty.lastbucket),
        (10, 2, 20)
    );
}

#[test]
fn rename_relabel_and_attributes() {
    let mut gts = GeoTimeSerie::new();
    assert_eq!(gts.name(), None);
    gts.rename("+x");
    assert_eq!(gts.name(), Some("nullx"));
    assert!(gts.renamed);
    gts.rename("abc");
    gts.rename("+.def");
    assert_eq!(gts.name(), Some("abc.def"));
    gts.metadata
        .relabel(&[(Some("a"), Some("1")), (Some("b"), Some("2"))]);
    gts.metadata.relabel(&[
        (Some("a"), None),
        (Some("c"), Some("")),
        (Some("d"), Some("4")),
    ]);
    let labels: Vec<(&str, &str)> = gts
        .metadata
        .labels
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(labels, vec![("b", "2"), ("d", "4")]);
    // A null key discards the existing labels.
    gts.metadata
        .relabel(&[(None, None), (Some("e"), Some("5"))]);
    assert_eq!(gts.metadata.labels.len(), 1);
    assert_eq!(gts.metadata.labels.get("e").map(String::as_str), Some("5"));
    gts.metadata
        .update_attributes(&[(Some("k"), Some("v")), (Some("z"), None)]);
    assert_eq!(
        gts.metadata.attributes.get("k").map(String::as_str),
        Some("v")
    );
}

#[test]
fn text_forms() {
    let mut gts = GeoTimeSerie::new();
    assert_eq!(gts.to_string(), "{}\n");
    assert_eq!(gts.dump(), "");
    gts.metadata.name = Some("na me".into());
    gts.metadata.labels.insert("l=1".into(), "v 1".into());
    gts.metadata.labels.insert("l2".into(), "v2".into());
    gts.metadata.attributes.insert("at".into(), "tr".into());
    gts.set_value(1, to_geoxp_point(48.0, 2.0), 35, &Value::from("é'x"), false);
    gts.set_value(2, NO_LOCATION, NO_ELEVATION, &Value::from("y"), false);
    assert_eq!(
        gts.to_string(),
        "na%20me{l%3D1=v%201,l2=v2}\n=1/47.99999998882413:1.9999999646097422/35 '%C3%A9%27x'\n=2// 'y'\n"
    );
    assert_eq!(
        gts.dump(),
        "1/47.99999998882413:1.9999999646097422/35 na%20me{l%3D1=v%201,l2=v2}{at=tr} '%C3%A9%27x'\r\n=2// 'y'\r\n"
    );
    let mut out = String::new();
    encode_value(&mut out, &Value::Long(-3));
    encode_value(&mut out, &Value::Double(1.5));
    encode_value(&mut out, &Value::Boolean(true));
    encode_value(&mut out, &Value::Boolean(false));
    encode_value(&mut out, &Value::Bytes(Rc::from([0xFBu8, 0xFF, 0x3E])));
    encode_value(&mut out, &Value::Null);
    assert_eq!(out, "-31.5TFb64:-_8-");
    // Every value type is dumped.
    for (value, text) in [
        (Value::Long(3), "3"),
        (Value::Double(0.25), "0.25"),
        (Value::Boolean(true), "T"),
    ] {
        let mut gts = GeoTimeSerie::new();
        gts.set_value(1, NO_LOCATION, NO_ELEVATION, &value, false);
        assert_eq!(gts.dump(), format!("1// {{}}{{}} {text}\r\n"));
    }
}

#[test]
fn binary_search_is_javas() {
    assert_eq!(java_binary_search(&[], 1), None);
    assert_eq!(java_binary_search(&[1, 3, 5], 3), Some(1));
    assert_eq!(java_binary_search(&[1, 3, 5], 4), None);
    assert_eq!(java_binary_search(&[1, 3, 5], 0), None);
    assert_eq!(java_binary_search(&[1, 3, 5], 9), None);
    // Among equal ticks, the one Java's loop lands on.
    assert_eq!(java_binary_search(&[2, 2, 2, 2, 2], 2), Some(2));
    assert_eq!(java_binary_search(&[1, 2, 2, 2], 2), Some(1));
}

#[test]
fn fullsort_without_values() {
    // Ticks without values (not reachable from WarpScript) still sort by tick.
    let mut gts = GeoTimeSerie::new();
    gts.ticks = vec![3, 1, 2, 1];
    gts.fullsort(false);
    assert_eq!(gts.ticks, vec![1, 1, 2, 3]);
}
