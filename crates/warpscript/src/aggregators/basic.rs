// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The elementary aggregators shared by the bucketizers, mappers and reducers: `sum`,
//! `mean`, `min`, `max`, `first`, `last`, `count`, `and` and `or`.

use std::cmp::Ordering;

use super::{
    Aggregated, Aggregator, AggregatorArgs, Named, Roles, double_value, latest, long_value,
};
use crate::error::aggregator::AggregatorError;
use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::functions::functional::java_double_compare;
use crate::geoxp;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::value::Value;

/// The registered aggregator names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "bucketizer.and",
        Entry::Object(|n| {
            Named::object(
                n,
                And {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "bucketizer.first",
        Entry::Object(|n| Named::object(n, First)),
    ),
    ("bucketizer.last", Entry::Object(|n| Named::object(n, Last))),
    (
        "bucketizer.min",
        Entry::Object(|n| Named::object(n, Min { ignore_nulls: true })),
    ),
    (
        "bucketizer.max",
        Entry::Object(|n| Named::object(n, Max { ignore_nulls: true })),
    ),
    (
        "bucketizer.mean",
        Entry::Object(|n| {
            Named::object(
                n,
                Mean {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "bucketizer.or",
        Entry::Object(|n| {
            Named::object(
                n,
                Or {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "bucketizer.sum",
        Entry::Object(|n| Named::object(n, Sum { ignore_nulls: true })),
    ),
    (
        "bucketizer.count",
        Entry::Object(|n| Named::object(n, Count { omit_nulls: false })),
    ),
    (
        "mapper.and",
        Entry::Object(|n| {
            Named::object(
                n,
                And {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "mapper.count",
        Entry::Object(|n| Named::object(n, Count { omit_nulls: false })),
    ),
    ("mapper.first", Entry::Object(|n| Named::object(n, First))),
    ("mapper.last", Entry::Object(|n| Named::object(n, Last))),
    (
        "mapper.min",
        Entry::Object(|n| Named::object(n, Min { ignore_nulls: true })),
    ),
    (
        "mapper.max",
        Entry::Object(|n| Named::object(n, Max { ignore_nulls: true })),
    ),
    (
        "mapper.mean",
        Entry::Object(|n| {
            Named::object(
                n,
                Mean {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "mapper.or",
        Entry::Object(|n| {
            Named::object(
                n,
                Or {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "mapper.sum",
        Entry::Object(|n| Named::object(n, Sum { ignore_nulls: true })),
    ),
    (
        "reducer.and",
        Entry::Object(|n| {
            Named::object(
                n,
                And {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.and.exclude-nulls",
        Entry::Object(|n| Named::object(n, And { ignore_nulls: true })),
    ),
    (
        "reducer.min",
        Entry::Object(|n| Named::object(n, Min { ignore_nulls: true })),
    ),
    (
        "reducer.min.forbid-nulls",
        Entry::Object(|n| {
            Named::object(
                n,
                Min {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.min.nonnull",
        Entry::Object(|n| {
            Named::object(
                n,
                Min {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.max",
        Entry::Object(|n| Named::object(n, Max { ignore_nulls: true })),
    ),
    (
        "reducer.max.forbid-nulls",
        Entry::Object(|n| {
            Named::object(
                n,
                Max {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.max.nonnull",
        Entry::Object(|n| {
            Named::object(
                n,
                Max {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.mean",
        Entry::Object(|n| {
            Named::object(
                n,
                Mean {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.mean.exclude-nulls",
        Entry::Object(|n| Named::object(n, Mean { ignore_nulls: true })),
    ),
    (
        "reducer.or",
        Entry::Object(|n| {
            Named::object(
                n,
                Or {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.or.exclude-nulls",
        Entry::Object(|n| Named::object(n, Or { ignore_nulls: true })),
    ),
    (
        "reducer.sum",
        Entry::Object(|n| Named::object(n, Sum { ignore_nulls: true })),
    ),
    (
        "reducer.sum.forbid-nulls",
        Entry::Object(|n| {
            Named::object(
                n,
                Sum {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.sum.nonnull",
        Entry::Object(|n| {
            Named::object(
                n,
                Sum {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.count",
        Entry::Object(|n| Named::object(n, Count { omit_nulls: false })),
    ),
    (
        "reducer.count.include-nulls",
        Entry::Object(|n| Named::object(n, Count { omit_nulls: false })),
    ),
    (
        "reducer.count.exclude-nulls",
        Entry::Object(|n| Named::object(n, Count { omit_nulls: true })),
    ),
    (
        "reducer.count.nonnull",
        Entry::Object(|n| Named::object(n, Count { omit_nulls: true })),
    ),
];

/// The type a running sum takes from its first value.
enum SumType {
    Undefined,
    Long(i64),
    Double(f64),
}

/// Add `value` to a running sum (`Sum`, `Mean`); `None` when a value is not numeric.
fn accumulate(sum: &mut SumType, value: &Value) -> Result<bool> {
    match sum {
        SumType::Long(l) => *l = l.wrapping_add(long_value(value)?),
        SumType::Double(d) => *d += double_value(value)?,
        SumType::Undefined => match value {
            Value::Long(l) => *sum = SumType::Long(*l),
            Value::Double(d) => *sum = SumType::Double(*d),
            _ => return Ok(false),
        },
    }
    Ok(true)
}

/// `Sum`: the sum of the values, typed by the first one.
pub struct Sum {
    /// Whether `null` values are skipped rather than making the sum `null`.
    pub ignore_nulls: bool,
}

impl Aggregator for Sum {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.ticks.is_empty() {
            return Ok(Aggregated::none());
        }
        let (location, elevation) = latest(args);
        let mut sum = SumType::Undefined;
        let mut has_nulls = false;
        for value in args.values {
            if value.is_null() {
                has_nulls = true;
                continue;
            }
            if !accumulate(&mut sum, value)? {
                return Ok(Aggregated::none());
            }
        }
        let value = match sum {
            _ if has_nulls && !self.ignore_nulls => Value::Null,
            SumType::Long(l) => Value::Long(l),
            SumType::Double(d) => Value::Double(d),
            SumType::Undefined => Value::Null,
        };
        Ok(Aggregated {
            tick: 0,
            location,
            elevation,
            value,
        })
    }
}

/// `Mean`: the mean of the values (always a `DOUBLE`), at the mean tick, location and
/// elevation.
pub struct Mean {
    /// Whether `null` values are skipped rather than producing no result.
    pub ignore_nulls: bool,
}

impl Aggregator for Mean {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.ticks.is_empty() {
            return Ok(Aggregated::none());
        }
        let mut sum = SumType::Undefined;
        let mut ticksum = 0i64;
        let (mut latitudes, mut longitudes, mut locationcount) = (0i64, 0i64, 0i64);
        let (mut elev, mut elevationcount) = (0i64, 0i64);
        let mut nulls = 0usize;
        for (i, value) in args.values.iter().enumerate() {
            if value.is_null() {
                nulls += 1;
                continue;
            }
            ticksum = ticksum.wrapping_add(args.ticks[i].wrapping_sub(args.ticks[0]));
            if args.locations[i] != NO_LOCATION {
                let (lat, lon) = geoxp::xy_from_geoxp_point(args.locations[i]);
                latitudes = latitudes.wrapping_add(lat);
                longitudes = longitudes.wrapping_add(lon);
                locationcount += 1;
            }
            if args.elevations[i] != NO_ELEVATION {
                elev = elev.wrapping_add(args.elevations[i]);
                elevationcount += 1;
            }
            if !accumulate(&mut sum, value)? {
                return Ok(Aggregated::none());
            }
        }
        let location = if locationcount > 0 {
            geoxp::to_geoxp_point_xy(latitudes / locationcount, longitudes / locationcount)
        } else {
            NO_LOCATION
        };
        let elevation = if elevationcount > 0 {
            elev / elevationcount
        } else {
            NO_ELEVATION
        };
        if !self.ignore_nulls && nulls > 0 {
            return Ok(Aggregated::none());
        }
        let count = (args.values.len() - nulls) as f64;
        let value = match sum {
            SumType::Long(l) => Value::Double(l as f64 / count),
            SumType::Double(d) => Value::Double(d / count),
            SumType::Undefined => Value::Null,
        };
        Ok(Aggregated {
            tick: args.ticks[0].wrapping_add(ticksum / args.ticks.len() as i64),
            location,
            elevation,
            value,
        })
    }
}

/// The running extremum of `Min` / `Max`, typed by the first value.
enum Extremum {
    Long(i64),
    Double(f64),
    String(String),
    Boolean(bool),
}

/// `Min` / `Max`: the smallest / largest value (the earliest one among equals), with its tick,
/// location and elevation; numbers compare as the type of the first value, strings by their
/// text and booleans as `false < true`.
struct Extreme {
    name: &'static str,
    ignore_nulls: bool,
    max: bool,
}

impl Extreme {
    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.ticks.is_empty() {
            return Ok(Aggregated::none());
        }
        let message = if self.max {
            "cannot compare non numeric values to numeric ones."
        } else {
            "cannot determine min of numeric and non numeric values."
        };
        let mismatch = || Error::usage(self.name, message.to_string());
        // `compareTo` outcome that makes `candidate` the new extremum.
        let better = if self.max {
            Ordering::Less
        } else {
            Ordering::Greater
        };
        let mut tick = i64::MAX;
        // The index of the extremum and the extremum itself.
        let mut best: Option<(usize, Extremum)> = None;
        let mut has_nulls = false;
        for (i, value) in args.values.iter().enumerate() {
            if value.is_null() {
                has_nulls = true;
                continue;
            }
            let retained = match &best {
                Some((_, Extremum::Long(current))) => {
                    let Some(candidate) = value.as_i64() else {
                        return Err(mismatch());
                    };
                    let comp = current.cmp(&candidate);
                    (comp == better || (comp == Ordering::Equal && args.ticks[i] < tick))
                        .then_some(Extremum::Long(candidate))
                }
                Some((_, Extremum::Double(current))) => {
                    let Some(candidate) = value.as_f64() else {
                        return Err(mismatch());
                    };
                    let comp = java_double_compare(*current, candidate);
                    (comp == better || (comp == Ordering::Equal && args.ticks[i] < tick))
                        .then_some(Extremum::Double(candidate))
                }
                Some((_, Extremum::String(current))) => {
                    let candidate = value.to_string();
                    let comp = current.encode_utf16().cmp(candidate.encode_utf16());
                    (comp == better || (comp == Ordering::Equal && args.ticks[i] < tick))
                        .then_some(Extremum::String(candidate))
                }
                Some((_, Extremum::Boolean(_))) => {
                    (*value == Value::Boolean(self.max)).then_some(Extremum::Boolean(self.max))
                }
                None => match value {
                    Value::Long(l) => Some(Extremum::Long(*l)),
                    Value::Double(d) => Some(Extremum::Double(*d)),
                    Value::String(s) => Some(Extremum::String(s.to_string())),
                    Value::Boolean(b) => Some(Extremum::Boolean(*b)),
                    _ => None,
                },
            };
            if let Some(extremum) = retained {
                tick = args.ticks[i];
                best = Some((i, extremum));
            }
        }
        // Warp 10 reads the arrays at index -1 when no value was retained.
        let Some((idx, extremum)) = best else {
            return Err(AggregatorError::NothingRetained(args.ticks.len()).into());
        };
        let value = if has_nulls && !self.ignore_nulls {
            Value::Null
        } else {
            match extremum {
                Extremum::Long(l) => Value::Long(l),
                Extremum::Double(d) => Value::Double(d),
                Extremum::String(s) => Value::from(s),
                Extremum::Boolean(b) => Value::Boolean(b),
            }
        };
        Ok(Aggregated {
            tick: args.ticks[idx],
            location: args.locations[idx],
            elevation: args.elevations[idx],
            value,
        })
    }
}

/// `Min`: the smallest value.
pub struct Min {
    /// Whether `null` values are skipped rather than making the result `null`.
    pub ignore_nulls: bool,
}

impl Aggregator for Min {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        Extreme {
            name: "min",
            ignore_nulls: self.ignore_nulls,
            max: false,
        }
        .apply(args)
    }
}

/// `Max`: the largest value.
pub struct Max {
    /// Whether `null` values are skipped rather than making the result `null`.
    pub ignore_nulls: bool,
}

impl Aggregator for Max {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        Extreme {
            name: "max",
            ignore_nulls: self.ignore_nulls,
            max: true,
        }
        .apply(args)
    }
}

/// `First` / `Last`: the datapoint with the smallest / largest tick.
fn extreme_tick(args: &AggregatorArgs, last: bool) -> Aggregated {
    if args.ticks.is_empty() {
        return Aggregated::none();
    }
    let mut idx = 0;
    for (i, &tick) in args.ticks.iter().enumerate() {
        let current = args.ticks[idx];
        if (last && tick > current) || (!last && tick < current) {
            idx = i;
        }
    }
    Aggregated {
        tick: args.ticks[idx],
        location: args.locations[idx],
        elevation: args.elevations[idx],
        value: args.values[idx].clone(),
    }
}

/// `First`: the earliest datapoint.
pub struct First;

impl Aggregator for First {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        Ok(extreme_tick(args, false))
    }
}

/// `Last`: the most recent datapoint.
pub struct Last;

impl Aggregator for Last {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        Ok(extreme_tick(args, true))
    }
}

/// `Count`: the number of values (non `null` ones on request).
pub struct Count {
    /// Whether `null` values are left out of the count.
    pub omit_nulls: bool,
}

impl Aggregator for Count {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.ticks.is_empty() {
            return Ok(Aggregated::none());
        }
        let (location, elevation) = latest(args);
        let count = args
            .values
            .iter()
            .filter(|value| !self.omit_nulls || !value.is_null())
            .count();
        Ok(Aggregated {
            tick: args.tick,
            location,
            elevation,
            value: Value::Long(count as i64),
        })
    }
}

/// `And` / `Or`: the conjunction / disjunction of the values (`true` only for the `BOOLEAN`
/// `true`), with the location and elevation of the most recent non `null` datapoint.
fn boolean_fold(args: &AggregatorArgs, ignore_nulls: bool, or: bool) -> Aggregated {
    if args.ticks.is_empty() {
        return Aggregated::none();
    }
    let mut result = !or;
    let mut location = NO_LOCATION;
    let mut elevation = NO_ELEVATION;
    let mut timestamp: Option<i64> = None;
    let mut nulls = 0;
    for (i, value) in args.values.iter().enumerate() {
        if value.is_null() {
            nulls += 1;
            continue;
        }
        let truth = *value == Value::Boolean(true);
        result = if or { result || truth } else { result && truth };
        if timestamp.is_none_or(|t| args.ticks[i] > t) {
            location = args.locations[i];
            elevation = args.elevations[i];
            timestamp = Some(args.ticks[i]);
        }
    }
    if !ignore_nulls && nulls > 0 {
        return Aggregated::none();
    }
    Aggregated {
        tick: args.tick,
        location,
        elevation,
        value: Value::Boolean(result),
    }
}

/// `And`: whether all the values are `true`.
pub struct And {
    /// Whether `null` values are skipped rather than producing no result.
    pub ignore_nulls: bool,
}

impl Aggregator for And {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        Ok(boolean_fold(args, self.ignore_nulls, false))
    }
}

/// `Or`: whether one of the values is `true`.
pub struct Or {
    /// Whether `null` values are skipped rather than producing no result.
    pub ignore_nulls: bool,
}

impl Aggregator for Or {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        Ok(boolean_fold(args, self.ignore_nulls, true))
    }
}
