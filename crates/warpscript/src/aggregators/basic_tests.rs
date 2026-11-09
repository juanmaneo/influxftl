// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the elementary aggregators on windows `BUCKETIZE` cannot produce: `null` values
//! and mixed value types (the windows of reducers).

use indexmap::IndexMap;

use super::basic::{And, Count, First, Last, Max, Mean, Min, Or, Sum};
use super::{Aggregated, Aggregator, AggregatorArgs, Roles};
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::value::Value;

/// Apply `aggregator` to datapoints at ticks `1, 2, …` with the given values.
fn apply(aggregator: &dyn Aggregator, values: &[Value]) -> crate::error::Result<Aggregated> {
    let ticks: Vec<i64> = (1..=values.len() as i64).collect();
    apply_at(aggregator, &ticks, values)
}

/// Apply `aggregator` to datapoints at the given ticks (elevation `10 * index`).
fn apply_at(
    aggregator: &dyn Aggregator,
    ticks: &[i64],
    values: &[Value],
) -> crate::error::Result<Aggregated> {
    let locations = vec![NO_LOCATION; values.len()];
    let elevations: Vec<i64> = (0..values.len() as i64).map(|i| i * 10).collect();
    let names = ["g".to_owned()];
    let labels = [IndexMap::new()];
    aggregator.apply(&AggregatorArgs {
        tick: 100,
        names: &names,
        labels: &labels,
        ticks,
        locations: &locations,
        elevations: &elevations,
        values,
        window: None,
    })
}

fn value(aggregator: &dyn Aggregator, values: &[Value]) -> Value {
    apply(aggregator, values)
        .expect("apply should succeed")
        .value
}

#[test]
fn nulls_are_ignored_or_poison_the_result() {
    let with_null = [Value::Long(1), Value::Null, Value::Long(3)];
    assert_eq!(
        value(&Sum { ignore_nulls: true }, &with_null),
        Value::Long(4)
    );
    assert_eq!(
        value(
            &Sum {
                ignore_nulls: false
            },
            &with_null
        ),
        Value::Null
    );
    assert_eq!(
        value(&Mean { ignore_nulls: true }, &with_null),
        Value::Double(2.0)
    );
    assert_eq!(
        apply(
            &Mean {
                ignore_nulls: false
            },
            &with_null
        )
        .expect("the value should succeed"),
        Aggregated::none()
    );
    assert_eq!(
        value(&Min { ignore_nulls: true }, &with_null),
        Value::Long(1)
    );
    assert_eq!(
        value(
            &Min {
                ignore_nulls: false
            },
            &with_null
        ),
        Value::Null
    );
    assert_eq!(
        value(&Max { ignore_nulls: true }, &with_null),
        Value::Long(3)
    );
    assert_eq!(
        value(
            &Max {
                ignore_nulls: false
            },
            &with_null
        ),
        Value::Null
    );
    assert_eq!(
        value(&Count { omit_nulls: true }, &with_null),
        Value::Long(2)
    );
    assert_eq!(
        value(&Count { omit_nulls: false }, &with_null),
        Value::Long(3)
    );
    let booleans = [Value::Boolean(true), Value::Null, Value::Boolean(true)];
    assert_eq!(
        value(&And { ignore_nulls: true }, &booleans),
        Value::Boolean(true)
    );
    assert_eq!(
        apply(
            &And {
                ignore_nulls: false
            },
            &booleans
        )
        .expect("the value should succeed"),
        Aggregated::none()
    );
    assert_eq!(
        value(&Or { ignore_nulls: true }, &booleans),
        Value::Boolean(true)
    );
    assert_eq!(
        apply(
            &Or {
                ignore_nulls: false
            },
            &booleans
        )
        .expect("the value should succeed"),
        Aggregated::none()
    );
    // The location and elevation come from the most recent non null datapoint.
    let aggregated = apply(
        &And { ignore_nulls: true },
        &[Value::Boolean(true), Value::Null],
    )
    .expect("the value should succeed");
    assert_eq!((aggregated.tick, aggregated.elevation), (100, 0));
    // Only nulls: nothing to sum, a `null` first / last, an unreadable extremum.
    let nulls = [Value::Null, Value::Null];
    assert_eq!(value(&Sum { ignore_nulls: true }, &nulls), Value::Null);
    assert_eq!(value(&Mean { ignore_nulls: true }, &nulls), Value::Null);
    assert_eq!(value(&First, &nulls), Value::Null);
    assert_eq!(value(&Last, &nulls), Value::Null);
    assert_eq!(
        apply(&Min { ignore_nulls: true }, &nulls)
            .expect_err("the value should fail")
            .message(),
        "Index -1 out of bounds for length 2"
    );
    // Empty windows.
    for aggregator in [
        &Sum { ignore_nulls: true } as &dyn Aggregator,
        &Mean { ignore_nulls: true },
        &Min { ignore_nulls: true },
        &Max { ignore_nulls: true },
        &First,
        &Last,
        &Count { omit_nulls: true },
        &And { ignore_nulls: true },
        &Or { ignore_nulls: true },
    ] {
        assert_eq!(
            apply(aggregator, &[]).expect("apply should succeed"),
            Aggregated::none()
        );
        assert_eq!(aggregator.roles(), Roles::ALL);
    }
}

#[test]
fn mixed_types_follow_the_first_value() {
    // Sums are typed by their first value; non numeric values end the aggregation.
    assert_eq!(
        value(
            &Sum { ignore_nulls: true },
            &[Value::Long(1), Value::Double(2.7)]
        ),
        Value::Long(3)
    );
    assert_eq!(
        value(
            &Sum { ignore_nulls: true },
            &[Value::Double(1.5), Value::Long(2)]
        ),
        Value::Double(3.5)
    );
    assert_eq!(
        apply(
            &Sum { ignore_nulls: true },
            &[Value::from("a"), Value::Long(2)]
        )
        .expect("the value should succeed"),
        Aggregated::none()
    );
    assert_eq!(
        apply(
            &Sum { ignore_nulls: true },
            &[Value::Long(1), Value::from("a")]
        )
        .expect_err("the value should fail")
        .message(),
        "class java.lang.String cannot be cast to class java.lang.Number"
    );
    assert_eq!(
        apply(
            &Mean { ignore_nulls: true },
            &[Value::Double(1.0), Value::Boolean(true)]
        )
        .expect_err("the value should fail")
        .message(),
        "class java.lang.Boolean cannot be cast to class java.lang.Number"
    );
    // Extrema: numbers compare as the first value's type, strings by text, booleans by
    // `false < true`; a non numeric value after a number is an error.
    assert_eq!(
        value(
            &Min { ignore_nulls: true },
            &[Value::Long(2), Value::Double(1.5)]
        ),
        Value::Long(1)
    );
    assert_eq!(
        value(
            &Max { ignore_nulls: true },
            &[Value::Double(1.5), Value::Long(2)]
        ),
        Value::Double(2.0)
    );
    assert_eq!(
        apply(
            &Min { ignore_nulls: true },
            &[Value::Long(1), Value::from("x")]
        )
        .expect_err("the value should fail")
        .message(),
        "min cannot determine min of numeric and non numeric values."
    );
    assert_eq!(
        apply(
            &Max { ignore_nulls: true },
            &[Value::Double(1.0), Value::from("x")]
        )
        .expect_err("the value should fail")
        .message(),
        "max cannot compare non numeric values to numeric ones."
    );
    assert_eq!(
        apply(
            &Min { ignore_nulls: true },
            &[Value::Double(1.0), Value::Boolean(true)]
        )
        .expect_err("the value should fail")
        .message(),
        "min cannot determine min of numeric and non numeric values."
    );
    assert_eq!(
        value(
            &Min { ignore_nulls: true },
            &[Value::from("b"), Value::Long(10), Value::from("a")]
        ),
        Value::from("10")
    );
    assert_eq!(
        value(
            &Max { ignore_nulls: true },
            &[Value::from("b"), Value::Long(10), Value::from("c")]
        ),
        Value::from("c")
    );
    assert_eq!(
        value(
            &Min { ignore_nulls: true },
            &[Value::Boolean(true), Value::Boolean(true)]
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        value(
            &Max { ignore_nulls: true },
            &[Value::Boolean(false), Value::Boolean(false)]
        ),
        Value::Boolean(false)
    );
    // Among equal values the earliest tick wins, whatever the order of the window.
    let min = Min { ignore_nulls: true };
    let tie = apply_at(&min, &[2, 1], &[Value::from("a"), Value::from("a")])
        .expect("apply_at should succeed");
    assert_eq!((tie.tick, tie.elevation), (1, 10));
    let tie = apply_at(&min, &[2, 1], &[Value::Long(7), Value::Long(7)])
        .expect("apply_at should succeed");
    assert_eq!((tie.tick, tie.elevation), (1, 10));
    let tie = apply_at(
        &Max { ignore_nulls: true },
        &[2, 1],
        &[Value::Double(7.0), Value::Double(7.0)],
    )
    .expect("the value should succeed");
    assert_eq!((tie.tick, tie.elevation), (1, 10));
    // A value of another kind is never retained.
    assert_eq!(
        apply(&min, &[Value::list(vec![])])
            .expect_err("apply should fail")
            .message(),
        "Index -1 out of bounds for length 1"
    );
    // `and` / `or` keep the location and elevation of the most recent datapoint.
    let folded = apply_at(
        &And { ignore_nulls: true },
        &[2, 1],
        &[Value::Boolean(true), Value::Boolean(true)],
    )
    .expect("the value should succeed");
    assert_eq!(folded.elevation, 0);
    // The `Aggregated` tuple is comparable and printable.
    let aggregated = apply(&Last, &[Value::Long(1)]).expect("apply should succeed");
    assert_eq!(
        aggregated,
        Aggregated {
            tick: 1,
            location: NO_LOCATION,
            elevation: 0,
            value: Value::Long(1)
        }
    );
    assert!(format!("{aggregated:?}").contains("tick: 1"));
    assert_eq!(NO_ELEVATION, i64::MIN);
}
