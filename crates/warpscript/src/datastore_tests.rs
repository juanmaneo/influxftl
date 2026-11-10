// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the datastore: the selectors, the time ranges and the datastore in memory.

use indexmap::IndexMap;

use crate::datastore::{
    Datastore, DeleteRequest, FetchRequest, MemoryDatastore, RangeNames, Selector, TimeRange,
    compute_time_range, get_timestamp,
};
use crate::error::datastore::{DatastoreError, TimeRangeError};
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Metadata, NO_ELEVATION, NO_LOCATION, Type};
use crate::value::Value;

/// A selector of a class and labels selectors.
fn selector(class: &str, labels: &[(&str, &str)]) -> Selector {
    Selector {
        class: class.to_owned(),
        labels: labels
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    }
}

/// A series with the points `(tick, value)`.
fn series(class: &str, labels: &[(&str, &str)], points: &[(i64, Value)]) -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::new();
    gts.metadata.name = Some(class.to_owned());
    gts.metadata.labels = labels
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    for (tick, value) in points {
        gts.set_value(*tick, NO_LOCATION, NO_ELEVATION, value, false);
    }
    gts
}

/// A request over a range of ticks, for every series.
fn request(start: i64, end: i64) -> FetchRequest {
    FetchRequest {
        selectors: vec![selector("~.*", &[])],
        start,
        end,
        count: None,
        value_type: None,
        boundary_pre: 0,
        boundary_post: 0,
        keep_empty: false,
        skip: 0,
        step: 1,
        timestep: None,
        series_skip: 0,
        series_count: i64::MAX,
    }
}

/// The ticks of the series a request fetches, series by series.
fn fetched_ticks(store: &MemoryDatastore, request: &FetchRequest) -> Vec<Vec<i64>> {
    store
        .fetch(request)
        .expect("the fetch should succeed")
        .iter()
        .map(|gts| gts.ticks.clone())
        .collect()
}

fn longs(range: std::ops::RangeInclusive<i64>) -> Vec<(i64, Value)> {
    range.map(|tick| (tick, Value::Long(tick * 10))).collect()
}

#[test]
fn a_selector_is_written_as_text() {
    let selector = selector("cpu", &[("host", "a"), ("dc", "~e.*"), ("zone", "=z")]);
    assert_eq!(selector.text(), "cpu{host=a,dc~e.*,zone=z}");
    assert_eq!(
        Selector {
            class: "a b".to_owned(),
            labels: IndexMap::new()
        }
        .text(),
        "a%20b{}"
    );
}

#[test]
fn a_selector_selects_metadata() {
    let metadata = Metadata {
        name: Some("cpu".to_owned()),
        labels: IndexMap::from([("host".to_owned(), "a".to_owned())]),
        ..Metadata::default()
    };
    assert!(
        selector("cpu", &[("host", "a")])
            .matches(&metadata)
            .expect("a selector")
    );
    assert!(
        selector("~c.*", &[("host", "~a|b")])
            .matches(&metadata)
            .expect("a selector")
    );
    assert!(
        !selector("cpu", &[("host", "b")])
            .matches(&metadata)
            .expect("a selector")
    );
    assert!(!selector("mem", &[]).matches(&metadata).expect("a selector"));
    assert!(
        selector("cpu", &[("host", "~(")])
            .matches(&metadata)
            .is_err()
    );
}

#[test]
fn a_datastore_finds_the_series_it_stores() {
    let store = MemoryDatastore::new();
    store
        .update(&[
            series("mem", &[("host", "b")], &longs(1..=2)),
            series("cpu", &[("host", "b")], &longs(1..=2)),
            series("cpu", &[("host", "a")], &longs(1..=2)),
        ])
        .expect("the update should succeed");
    let found = store
        .find(&[selector("cpu", &[])])
        .expect("the find should succeed");
    let labels: Vec<_> = found.iter().map(|m| m.labels["host"].clone()).collect();
    // Sorted by class then labels.
    assert_eq!(labels, ["a", "b"]);
    let all = store
        .find(&[selector("~.*", &[])])
        .expect("the find should succeed");
    let names: Vec<_> = all.iter().filter_map(|m| m.name.clone()).collect();
    assert_eq!(names, ["cpu", "cpu", "mem"]);
    // Two selectors select what either selects, once.
    let either = store
        .find(&[selector("cpu", &[("host", "a")]), selector("cpu", &[])])
        .expect("the find should succeed");
    assert_eq!(either.len(), 2);
    assert!(store.find(&[selector("cpu", &[("host", "~(")])]).is_err());
}

#[test]
fn the_most_recent_datapoint_comes_first() {
    let store = MemoryDatastore::new();
    store
        .update(&[series("cpu", &[], &longs(1..=5))])
        .expect("the update should succeed");
    let fetched = store
        .fetch(&request(2, 4))
        .expect("the fetch should succeed");
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].ticks, [4, 3, 2]);
    assert_eq!(fetched[0].value_at(0), Value::Long(40));
    assert_eq!(fetched[0].metadata.name.as_deref(), Some("cpu"));
    // The range includes its bounds, and may hold nothing.
    assert_eq!(fetched_ticks(&store, &request(5, 5)), [[5]]);
    assert_eq!(
        fetched_ticks(&store, &request(6, 9)),
        Vec::<Vec<i64>>::new()
    );
}

#[test]
fn a_datastore_counts_from_the_end() {
    let store = MemoryDatastore::new();
    store
        .update(&[series("cpu", &[], &longs(1..=5))])
        .expect("the update should succeed");
    let counted = |count| FetchRequest {
        count: Some(count),
        ..request(i64::MIN, 4)
    };
    assert_eq!(fetched_ticks(&store, &counted(2)), [[4, 3]]);
    assert_eq!(fetched_ticks(&store, &counted(99)), [[4, 3, 2, 1]]);
    assert_eq!(fetched_ticks(&store, &counted(0)), Vec::<Vec<i64>>::new());
    assert_eq!(fetched_ticks(&store, &counted(-3)), Vec::<Vec<i64>>::new());
}

#[test]
fn a_datastore_fetches_the_boundaries_around_a_range() {
    let store = MemoryDatastore::new();
    store
        .update(&[series("cpu", &[], &longs(1..=9))])
        .expect("the update should succeed");
    let bounded = FetchRequest {
        boundary_pre: 2,
        boundary_post: 1,
        ..request(4, 6)
    };
    assert_eq!(fetched_ticks(&store, &bounded), [[7, 6, 5, 4, 3, 2]]);
    let pre_only = FetchRequest {
        boundary_pre: 99,
        ..request(8, 9)
    };
    assert_eq!(
        fetched_ticks(&store, &pre_only),
        [[9, 8, 7, 6, 5, 4, 3, 2, 1]]
    );
    // A negative boundary is none.
    let negative = FetchRequest {
        boundary_pre: -1,
        boundary_post: -1,
        ..request(4, 6)
    };
    assert_eq!(fetched_ticks(&store, &negative), [[6, 5, 4]]);
}

#[test]
fn a_datastore_skips_steps_and_spaces_the_datapoints() {
    let store = MemoryDatastore::new();
    store
        .update(&[series("cpu", &[], &longs(1..=10))])
        .expect("the update should succeed");
    let skipped = FetchRequest {
        skip: 2,
        ..request(1, 10)
    };
    assert_eq!(fetched_ticks(&store, &skipped), [[8, 7, 6, 5, 4, 3, 2, 1]]);
    let stepped = FetchRequest {
        step: 3,
        ..request(1, 10)
    };
    assert_eq!(fetched_ticks(&store, &stepped), [[10, 7, 4, 1]]);
    let spaced = FetchRequest {
        timestep: Some(4),
        ..request(1, 10)
    };
    assert_eq!(fetched_ticks(&store, &spaced), [[10, 6, 2]]);
    let all = FetchRequest {
        skip: 1,
        step: 2,
        timestep: Some(3),
        count: Some(2),
        ..request(1, 10)
    };
    assert_eq!(fetched_ticks(&store, &all), [[9, 5]]);
}

#[test]
fn a_datastore_fetches_the_values_of_a_type() {
    let store = MemoryDatastore::new();
    store
        .update(&[
            series("n", &[], &[(1, Value::Long(1)), (2, Value::Long(2))]),
            series("d", &[], &[(1, Value::Double(1.5))]),
            series("s", &[], &[(1, Value::from("a"))]),
            series("b", &[], &[(1, Value::Boolean(true))]),
        ])
        .expect("the update should succeed");
    for (value_type, class) in [
        (Type::Long, "n"),
        (Type::Double, "d"),
        (Type::String, "s"),
        (Type::Boolean, "b"),
    ] {
        let typed = FetchRequest {
            value_type: Some(value_type),
            ..request(0, 9)
        };
        let fetched = store.fetch(&typed).expect("the fetch should succeed");
        assert_eq!(fetched.len(), 1, "{value_type:?}");
        assert_eq!(fetched[0].metadata.name.as_deref(), Some(class));
    }
}

#[test]
fn a_datastore_keeps_empty_series_when_asked() {
    let store = MemoryDatastore::new();
    store
        .update(&[series("cpu", &[], &longs(1..=3))])
        .expect("the update should succeed");
    assert_eq!(
        fetched_ticks(&store, &request(7, 9)),
        Vec::<Vec<i64>>::new()
    );
    let kept = FetchRequest {
        keep_empty: true,
        ..request(7, 9)
    };
    assert_eq!(fetched_ticks(&store, &kept), [Vec::<i64>::new()]);
}

#[test]
fn a_datastore_skips_and_counts_series() {
    let store = MemoryDatastore::new();
    store
        .update(&[
            series("a", &[], &longs(1..=2)),
            series("b", &[], &longs(1..=2)),
            series("c", &[], &longs(1..=2)),
        ])
        .expect("the update should succeed");
    let names = |request: &FetchRequest| -> Vec<String> {
        store
            .fetch(request)
            .expect("the fetch should succeed")
            .iter()
            .filter_map(|gts| gts.metadata.name.clone())
            .collect()
    };
    assert_eq!(
        names(&FetchRequest {
            series_skip: 1,
            ..request(1, 2)
        }),
        ["b", "c"]
    );
    assert_eq!(
        names(&FetchRequest {
            series_count: 2,
            ..request(1, 2)
        }),
        ["a", "b"]
    );
    assert_eq!(
        names(&FetchRequest {
            series_skip: 1,
            series_count: 1,
            ..request(1, 2)
        }),
        ["b"]
    );
    assert_eq!(
        names(&FetchRequest {
            series_skip: -1,
            series_count: -1,
            ..request(1, 2)
        }),
        Vec::<String>::new()
    );
}

#[test]
fn locations_and_elevations_are_kept() {
    let store = MemoryDatastore::new();
    let mut gts = GeoTimeSerie::new();
    gts.metadata.name = Some("gps".to_owned());
    gts.set_value(
        1,
        crate::geoxp::to_geoxp_point(48.0, 2.0),
        120,
        &Value::Double(1.0),
        false,
    );
    store.update(&[gts]).expect("the update should succeed");
    let fetched = store
        .fetch(&request(0, 9))
        .expect("the fetch should succeed");
    assert_eq!(
        fetched[0].location_at(0),
        crate::geoxp::to_geoxp_point(48.0, 2.0)
    );
    assert_eq!(fetched[0].elevation_at(0), 120);
}

#[test]
fn an_update_replaces_the_datapoints_at_the_same_ticks() {
    let store = MemoryDatastore::new();
    store
        .update(&[series("cpu", &[("h", "a")], &longs(1..=3))])
        .expect("the update should succeed");
    store
        .update(&[series(
            "cpu",
            &[("h", "a")],
            &[(3, Value::Long(-1)), (4, Value::Long(-2))],
        )])
        .expect("the update should succeed");
    let fetched = store
        .fetch(&request(0, 9))
        .expect("the fetch should succeed");
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].ticks, [4, 3, 2, 1]);
    assert_eq!(fetched[0].value_at(1), Value::Long(-1));
    // A series that has no datapoint changes nothing.
    store
        .update(&[series("cpu", &[("h", "a")], &[])])
        .expect("the update should succeed");
}

#[test]
fn a_series_needs_a_name_to_be_stored() {
    let store = MemoryDatastore::new();
    assert_eq!(
        store
            .update(&[series("", &[], &longs(1..=2))])
            .expect_err("no name"),
        DatastoreError::Unnamed.into()
    );
    let mut unnamed = GeoTimeSerie::new();
    unnamed.set_value(1, NO_LOCATION, NO_ELEVATION, &Value::Long(1), false);
    assert_eq!(
        store.update(&[unnamed]).expect_err("no name"),
        DatastoreError::Unnamed.into()
    );
    assert_eq!(
        store
            .update_metadata(&[Metadata::default()], false)
            .expect_err("no name"),
        DatastoreError::Unnamed.into()
    );
    assert_eq!(
        store
            .find(&[selector("~.*", &[])])
            .expect("the find should succeed"),
        Vec::<Metadata>::new()
    );
}

#[test]
fn attributes_are_replaced_or_changed() {
    let store = MemoryDatastore::new();
    let attributes = |pairs: &[(&str, &str)]| Metadata {
        name: Some("cpu".to_owned()),
        labels: IndexMap::from([("h".to_owned(), "a".to_owned())]),
        attributes: pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        ..Metadata::default()
    };
    let current = || {
        store
            .find(&[selector("cpu", &[])])
            .expect("the find should succeed")[0]
            .attributes
            .clone()
    };
    // A series that does not exist is created.
    store
        .update_metadata(&[attributes(&[("a", "1"), ("b", "2")])], false)
        .expect("the update");
    assert_eq!(
        current(),
        IndexMap::from([
            ("a".to_owned(), "1".to_owned()),
            ("b".to_owned(), "2".to_owned())
        ])
    );
    // A change keeps the others, and an empty value removes.
    store
        .update_metadata(&[attributes(&[("a", ""), ("c", "3")])], true)
        .expect("the update");
    assert_eq!(
        current(),
        IndexMap::from([
            ("b".to_owned(), "2".to_owned()),
            ("c".to_owned(), "3".to_owned())
        ])
    );
    // Without delta the attributes replace the others.
    store
        .update_metadata(&[attributes(&[("d", "4")])], false)
        .expect("the update");
    assert_eq!(
        current(),
        IndexMap::from([("d".to_owned(), "4".to_owned())])
    );
    // The datapoints and the attributes of a series live together.
    store
        .update(&[series("cpu", &[("h", "a")], &longs(1..=2))])
        .expect("the update should succeed");
    assert_eq!(current().len(), 1);
}

#[test]
fn a_datastore_deletes_a_range_or_everything() {
    let store = MemoryDatastore::new();
    store
        .update(&[
            series("cpu", &[], &longs(1..=6)),
            series("mem", &[], &longs(1..=6)),
        ])
        .expect("the update should succeed");
    let deleted = store
        .delete(&DeleteRequest {
            selectors: vec![selector("cpu", &[])],
            range: Some((2, 4)),
        })
        .expect("the delete should succeed");
    assert_eq!(deleted.len(), 1);
    assert_eq!(
        fetched_ticks(&store, &request(0, 9)),
        [vec![6, 5, 1], vec![6, 5, 4, 3, 2, 1]]
    );
    let deleted = store
        .delete(&DeleteRequest {
            selectors: vec![selector("mem", &[])],
            range: None,
        })
        .expect("the delete should succeed");
    assert_eq!(deleted.len(), 1);
    assert_eq!(
        store
            .find(&[selector("~.*", &[])])
            .expect("the find should succeed")
            .len(),
        1
    );
    assert!(
        store
            .delete(&DeleteRequest {
                selectors: vec![selector("cpu", &[("h", "~(")])],
                range: None
            })
            .is_err()
    );
}

// -----------------------------------------------------------------------------------------
// The time range
// -----------------------------------------------------------------------------------------

const NAMES: RangeNames<'static> = RangeNames {
    start: "start",
    end: "end",
    timespan: "timespan",
    count: "count",
};
const NOW: i64 = 1_000_000;

fn long(value: i64) -> Value {
    Value::Long(value)
}

fn text(value: &str) -> Value {
    Value::from(value)
}

#[allow(clippy::needless_pass_by_value)]
fn range(
    start: Option<Value>,
    end: Option<Value>,
    timespan: Option<Value>,
    count: Option<Value>,
) -> Result<TimeRange> {
    compute_time_range(
        start.as_ref(),
        end.as_ref(),
        timespan.as_ref(),
        count.as_ref(),
        NAMES,
        NOW,
    )
}

fn ticks(start: i64, end: i64, timespan: Option<i64>) -> TimeRange {
    TimeRange {
        start,
        end,
        timespan,
    }
}

#[test]
fn a_timestamp_is_a_long_a_text_or_now() {
    let tick = |value: Option<Value>| get_timestamp(value.as_ref(), "end", NOW);
    assert_eq!(tick(None), Ok(None));
    assert_eq!(tick(Some(Value::Null)), Ok(None));
    assert_eq!(tick(Some(long(42))), Ok(Some(42)));
    assert_eq!(tick(Some(text("now"))), Ok(Some(NOW)));
    assert_eq!(tick(Some(text("123"))), Ok(Some(123)));
    assert_eq!(
        tick(Some(text("1970-01-01T00:00:01Z"))),
        Ok(Some(1_000_000))
    );
    let invalid: Error = TimeRangeError::InvalidFormat("end".to_owned()).into();
    assert_eq!(tick(Some(text("yesterday"))), Err(invalid.clone()));
    assert_eq!(tick(Some(Value::Double(1.0))), Err(invalid));
}

#[test]
fn a_range_has_its_bounds() {
    assert_eq!(
        range(Some(long(10)), Some(long(20)), None, None),
        Ok(ticks(10, 20, None))
    );
    // Swapped bounds are put right.
    assert_eq!(
        range(Some(long(20)), Some(long(10)), None, None),
        Ok(ticks(10, 20, None))
    );
    assert_eq!(
        range(Some(text("now")), Some(long(NOW + 5)), None, None),
        Ok(ticks(NOW, NOW + 5, None))
    );
}

#[test]
fn a_range_comes_from_a_bound_and_a_timespan() {
    assert_eq!(
        range(None, Some(long(100)), Some(long(10)), None),
        Ok(ticks(91, 100, Some(10)))
    );
    assert_eq!(
        range(Some(long(100)), None, Some(long(10)), None),
        Ok(ticks(100, 109, Some(10)))
    );
    assert_eq!(
        range(None, Some(long(100)), Some(text("50")), None),
        Ok(ticks(51, 100, Some(50)))
    );
    // A duration of days.
    assert_eq!(
        range(None, Some(long(i64::MAX)), Some(text("P1D")), None),
        Ok(ticks(
            i64::MAX - 86_400_000_000 + 1,
            i64::MAX,
            Some(86_400_000_000)
        ))
    );
    // A range that goes past the ends of time stops there.
    assert_eq!(
        range(None, Some(long(i64::MIN + 5)), Some(long(10)), None),
        Ok(ticks(i64::MIN, i64::MIN + 5, Some(10)))
    );
    assert_eq!(
        range(Some(long(i64::MAX - 5)), None, Some(long(10)), None),
        Ok(ticks(i64::MAX - 5, i64::MAX, Some(10)))
    );
    // A null timespan is none.
    assert_eq!(
        range(Some(long(1)), Some(long(2)), Some(Value::Null), None),
        Ok(ticks(1, 2, None))
    );
}

#[test]
fn a_range_needs_a_count_without_a_start() {
    assert_eq!(
        range(None, Some(long(100)), None, Some(long(5))),
        Ok(ticks(i64::MIN, 100, None))
    );
    assert_eq!(
        range(None, Some(long(100)), None, None),
        Err(TimeRangeError::CountMandatory {
            count: "count".to_owned(),
            start: "start".to_owned(),
            timespan: "timespan".to_owned()
        }
        .into())
    );
    assert!(range(None, Some(long(100)), None, Some(Value::Null)).is_err());
}

#[test]
fn a_range_that_cannot_be_computed_is_refused() {
    let name = |name: &str| name.to_owned();
    let cases: Vec<(Result<TimeRange>, TimeRangeError)> = vec![
        (
            range(None, None, None, None),
            TimeRangeError::MissingStartOrEnd {
                start: name("start"),
                end: name("end"),
            },
        ),
        (
            range(None, None, Some(long(5)), None),
            TimeRangeError::MissingStartOrEnd {
                start: name("start"),
                end: name("end"),
            },
        ),
        (
            range(Some(long(1)), Some(long(2)), Some(long(5)), None),
            TimeRangeError::TooManyBounds {
                start: name("start"),
                end: name("end"),
                timespan: name("timespan"),
            },
        ),
        (
            range(None, Some(long(5)), Some(text("")), None),
            TimeRangeError::Empty(name("timespan")),
        ),
        (
            range(None, Some(long(5)), Some(long(-1)), None),
            TimeRangeError::Negative(name("timespan")),
        ),
        (
            range(None, Some(long(5)), Some(Value::Double(1.0)), None),
            TimeRangeError::InvalidFormat(name("timespan")),
        ),
        (
            range(None, Some(long(i64::MAX)), Some(long(0)), None),
            TimeRangeError::EmptyBeforeMaxEnd {
                timespan: name("timespan"),
                end: name("end"),
            },
        ),
        (
            range(Some(long(i64::MIN)), None, Some(long(0)), None),
            TimeRangeError::EmptyAfterMinStart {
                timespan: name("timespan"),
                start: name("start"),
            },
        ),
        (
            range(Some(long(5)), None, None, None),
            TimeRangeError::MissingEnd {
                end: name("end"),
                start: name("start"),
                timespan: name("timespan"),
            },
        ),
    ];
    for (result, expected) in cases {
        assert_eq!(result, Err(expected.clone().into()), "{expected}");
    }
}

#[test]
fn a_timespan_that_is_not_one_says_why() {
    for timespan in ["abc", "P1M", "P1Y"] {
        let err = range(None, Some(long(5)), Some(text(timespan)), None).expect_err("no timespan");
        assert_eq!(
            err.message(),
            "Invalid format for parameter 'timespan'.",
            "{timespan}"
        );
        assert!(err.cause().is_some(), "{timespan}");
    }
}

#[test]
fn a_selector_that_cannot_be_matched_fails_the_fetch() {
    let store = MemoryDatastore::new();
    store
        .update(&[series(&"a".repeat(46), &[], &longs(1..=2))])
        .expect("the update should succeed");
    // The backtracking limit of the regular expression engine.
    let catastrophic = selector("~(a|a)*\\1b", &[]);
    assert!(store.find(std::slice::from_ref(&catastrophic)).is_err());
    let request = FetchRequest {
        selectors: vec![catastrophic],
        ..request(0, 9)
    };
    assert!(store.fetch(&request).is_err());
    assert!(
        store
            .fetch(&FetchRequest {
                selectors: vec![selector("cpu", &[("a", "~(")])],
                ..request.clone()
            })
            .is_err()
    );
}

#[test]
fn the_bounds_of_a_range_must_be_times() {
    let invalid = |start: Option<Value>, end: Option<Value>| range(start, end, None, None);
    assert!(invalid(Some(text("yesterday")), Some(long(1))).is_err());
    assert!(invalid(Some(long(1)), Some(text("yesterday"))).is_err());
}
