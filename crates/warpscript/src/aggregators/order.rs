// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The order statistics aggregators: `median`, `mad` (median absolute deviation) and
//! `percentile` (built by a stack function consuming the rank and the Hyndman & Fan type).

use std::cmp::Ordering;

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Num, Roles, double_value};
use crate::error::{Error, Result};
use crate::format::java_double_to_string;
use crate::functions::Entry;
use crate::functions::functional::java_double_compare;
use crate::geoxp;
use crate::gts::{NO_ELEVATION, NO_LOCATION, java_binary_search};
use crate::jmath;
use crate::stack::Stack;
use crate::value::Value;

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "bucketizer.median",
        Entry::Object(|n| {
            Named::object(
                n,
                Median {
                    name: n,
                    forbid_nulls: false,
                },
            )
        }),
    ),
    (
        "mapper.median",
        Entry::Object(|n| {
            Named::object(
                n,
                Median {
                    name: n,
                    forbid_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.median",
        Entry::Object(|n| {
            Named::object(
                n,
                Median {
                    name: n,
                    forbid_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.median.forbid-nulls",
        Entry::Object(|n| {
            Named::object(
                n,
                Median {
                    name: n,
                    forbid_nulls: true,
                },
            )
        }),
    ),
    ("bucketizer.mad", Entry::Object(|n| Named::object(n, Mad))),
    ("mapper.mad", Entry::Object(|n| Named::object(n, Mad))),
    ("reducer.mad", Entry::Object(|n| Named::object(n, Mad))),
    (
        "bucketizer.percentile",
        Entry::Builder(|s, n| percentile_builder(s, n, false)),
    ),
    (
        "mapper.percentile",
        Entry::Builder(|s, n| percentile_builder(s, n, false)),
    ),
    (
        "reducer.percentile",
        Entry::Builder(|s, n| percentile_builder(s, n, false)),
    ),
    (
        "reducer.percentile.forbid-nulls",
        Entry::Builder(|s, n| percentile_builder(s, n, true)),
    ),
];

/// The indices of `values` sorted by increasing value with the `null`s last (the comparator
/// `Median` and `Percentile` share), failing with `<name> can only operate on numeric Geo
/// Time Series.` when two non-null values would be compared and one is not a number.
fn sorted_indices(values: &[Value], name: &str) -> Result<Vec<usize>> {
    let non_null: Vec<usize> = (0..values.len())
        .filter(|&i| !values[i].is_null())
        .collect();
    if non_null.len() < 2 {
        // No two non-null values are ever compared: the one there may be comes first, the
        // nulls keep their order.
        let mut indices = non_null;
        indices.extend((0..values.len()).filter(|&i| values[i].is_null()));
        return Ok(indices);
    }
    let keys: Vec<Option<Num>> = values.iter().map(Num::of).collect();
    if non_null.iter().any(|&i| keys[i].is_none()) {
        return Err(Error::new(format!(
            "{name} can only operate on numeric Geo Time Series."
        )));
    }
    let mut indices: Vec<usize> = (0..values.len()).collect();
    indices.sort_by(|&a, &b| match (keys[a], keys[b]) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => x.compare(y),
    });
    Ok(indices)
}

/// The value at `i` as a double, once the sort established it is numeric.
fn as_double(values: &[Value], i: usize) -> f64 {
    Num::of(&values[i]).map(Num::as_f64).unwrap_or_default()
}

/// `Median`: the median of the values (a `DOUBLE`) with the location and elevation of the
/// median datapoint, or the centroid / mean of the two central ones when the count is even.
pub struct Median {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// Whether a `null` value is an error (otherwise nulls are ignored).
    pub forbid_nulls: bool,
}

impl Aggregator for Median {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let values = args.values;
        let nulls = values.iter().filter(|v| v.is_null()).count();
        if nulls != 0 && self.forbid_nulls {
            return Err(Error::new(format!(
                "{} cannot compute median of null values.",
                self.name
            )));
        }
        let indices = sorted_indices(values, self.name)?;
        let n = values.len() - nulls;
        let mut location = NO_LOCATION;
        let mut elevation = NO_ELEVATION;
        let median = if n == 1 {
            let i = indices[0];
            location = args.locations[i];
            elevation = args.elevations[i];
            double_value(&values[i])?
        } else if n.is_multiple_of(2) {
            if n == 0 {
                // `indices[-1]` in Warp 10.
                return Err(Error::new(format!(
                    "Index -1 out of bounds for length {}",
                    values.len()
                )));
            }
            let low = indices[n / 2 - 1];
            let high = indices[n / 2];
            // The centroid of the two locations, the mean of the two elevations, or the one
            // there is.
            if args.elevations[low] != NO_ELEVATION && args.elevations[high] != NO_ELEVATION {
                elevation = args.elevations[low].wrapping_add(args.elevations[high]) / 2;
            } else if args.elevations[low] != NO_ELEVATION {
                elevation = args.elevations[low];
            } else if args.elevations[high] != NO_ELEVATION {
                elevation = args.elevations[high];
            }
            if args.locations[low] != NO_LOCATION && args.locations[high] != NO_LOCATION {
                let (lat_low, lon_low) = geoxp::xy_from_geoxp_point(args.locations[low]);
                let (lat_high, lon_high) = geoxp::xy_from_geoxp_point(args.locations[high]);
                location = geoxp::to_geoxp_point_xy(
                    lat_low.wrapping_add(lat_high) / 2,
                    lon_low.wrapping_add(lon_high) / 2,
                );
            } else if args.locations[low] != NO_LOCATION {
                location = args.locations[low];
            } else if args.locations[high] != NO_LOCATION {
                location = args.locations[high];
            }
            // `(a + b) / 2` as Java computes it.
            #[allow(clippy::manual_midpoint)]
            let median = (as_double(values, low) + as_double(values, high)) / 2.0;
            median
        } else {
            let i = indices[n / 2];
            location = args.locations[i];
            elevation = args.elevations[i];
            as_double(values, i)
        };
        Ok(Aggregated {
            tick: args.tick,
            location,
            elevation,
            value: Value::Double(median),
        })
    }
}

/// The values of a window sorted by their natural Java ordering (`Arrays.sort(Object[])`):
/// all of one class, since comparing two classes fails.
enum Sorted<'a> {
    /// No value.
    Empty,
    /// `LONG`s.
    Longs(Vec<i64>),
    /// `DOUBLE`s, in `Double.compareTo` order.
    Doubles(Vec<f64>),
    /// `STRING`s, in UTF-16 order.
    Strings(Vec<&'a str>),
    /// `BOOLEAN`s, `false` first.
    Booleans(Vec<bool>),
}

/// `Arrays.sort(values)` on the natural ordering, failing like Java's `ClassCastException`
/// when the values are not all of the same class (the message names the class of the first
/// value and the first class differing from it, which is what `ComparableTimSort` compares).
fn natural_sort(values: &[Value]) -> Result<Sorted<'_>> {
    let Some(first) = values.first() else {
        return Ok(Sorted::Empty);
    };
    if let Some(other) = values.iter().find(|v| v.java_class() != first.java_class()) {
        return Err(Error::new(format!(
            "class {} cannot be cast to class {}",
            first.java_class(),
            other.java_class()
        )));
    }
    Ok(match first {
        Value::Long(_) => {
            let mut longs: Vec<i64> = values.iter().filter_map(Value::as_i64).collect();
            longs.sort_unstable();
            Sorted::Longs(longs)
        }
        Value::Double(_) => {
            let mut doubles: Vec<f64> = values.iter().filter_map(Value::as_f64).collect();
            doubles.sort_by(|a, b| java_double_compare(*a, *b));
            Sorted::Doubles(doubles)
        }
        Value::String(_) => {
            let mut strings: Vec<&str> = values.iter().filter_map(Value::as_str).collect();
            strings.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            Sorted::Strings(strings)
        }
        Value::Boolean(_) => {
            let mut booleans: Vec<bool> = values.iter().filter_map(Value::as_bool).collect();
            booleans.sort_unstable();
            Sorted::Booleans(booleans)
        }
        // Series hold no other type; such a window would not even be `Comparable`.
        other => {
            return Err(Error::new(format!(
                "class {} cannot be cast to class java.lang.Comparable",
                other.java_class()
            )));
        }
    })
}

/// The median `MAD` takes of sorted `LONG`s: the first value when it equals the last, the
/// (truncated) mean of the two central values or the central one.
fn median_of_longs(sorted: &[i64]) -> i64 {
    let n = sorted.len();
    if sorted[0] == sorted[n - 1] {
        sorted[0]
    } else if n.is_multiple_of(2) {
        sorted[n / 2 - 1].wrapping_add(sorted[n / 2]) / 2
    } else {
        sorted[n / 2]
    }
}

/// The median `MAD` takes of sorted `DOUBLE`s (`Double.equals` on the extrema), the central
/// pair averaged as `(a + b) / 2` like Java.
#[allow(clippy::manual_midpoint)]
fn median_of_doubles(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if java_double_compare(sorted[0], sorted[n - 1]) == Ordering::Equal {
        sorted[0]
    } else if n.is_multiple_of(2) {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    }
}

/// Java's `(double) median` on a `STRING` or `BOOLEAN` median: the `ClassCastException`.
fn double_cast(class: &str) -> Error {
    Error::new(format!(
        "class {class} cannot be cast to class java.lang.Double"
    ))
}

/// The median of non numeric values: Warp 10 selects it like for numbers (identical
/// extrema, the central value of an odd count) and then fails to cast it to a double, or
/// fails right away on an even count of distinct values.
fn non_numeric_median<T: PartialEq>(sorted: &[T], class: &str) -> Error {
    let n = sorted.len();
    if sorted[0] == sorted[n - 1] || n % 2 == 1 {
        double_cast(class)
    } else {
        Error::new("Unable to compute median on an even number of non numeric values.")
    }
}

/// The median of a sorted array of locations or elevations once the `absent` marker is
/// removed the way `MAD` does: `Arrays.binarySearch` finds one occurrence of the marker and
/// the occurrences from there to the next real value are dropped — the ones before it
/// (and all of them when no real value follows) stay in the computation.
fn marker_median(sorted: &mut [i64], absent: i64, middle: impl Fn(i64, i64) -> i64) -> i64 {
    let n = sorted.len();
    if sorted[0] == sorted[n - 1] {
        return sorted[0];
    }
    let mut len = n;
    if let Some(idx) = java_binary_search(sorted, absent) {
        let mut i = idx + 1;
        while i < n && sorted[i] == absent {
            i += 1;
        }
        if i < n {
            sorted.copy_within(i.., idx);
            len -= i - idx;
        }
    }
    if len.is_multiple_of(2) {
        middle(sorted[len / 2 - 1], sorted[len / 2])
    } else {
        sorted[len / 2]
    }
}

/// `MAD`: the median absolute deviation of the values, at the median location and elevation.
pub struct Mad;

impl Aggregator for Mad {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.locations.is_empty() {
            // `locations[0]` in Warp 10.
            return Err(Error::new("Index 0 out of bounds for length 0"));
        }
        let mut locations = args.locations.to_vec();
        locations.sort_unstable();
        // `(a + b) / 2` as Java computes it.
        #[allow(clippy::manual_midpoint)]
        let location = marker_median(&mut locations, NO_LOCATION, |low, high| {
            let (lat_high, lon_high) = geoxp::from_geoxp_point(high);
            let (lat_low, lon_low) = geoxp::from_geoxp_point(low);
            geoxp::to_geoxp_point((lat_high + lat_low) / 2.0, (lon_high + lon_low) / 2.0)
        });
        let mut elevations = args.elevations.to_vec();
        elevations.sort_unstable();
        let elevation = marker_median(&mut elevations, NO_ELEVATION, |low, high| {
            high.wrapping_add(low) / 2
        });
        let values: Vec<Value> = args
            .values
            .iter()
            .filter(|v| !v.is_null())
            .cloned()
            .collect();
        let value = match natural_sort(&values)? {
            Sorted::Empty => Value::Null,
            Sorted::Longs(longs) => {
                let median = median_of_longs(&longs);
                let mut residuals: Vec<i64> = longs
                    .iter()
                    .map(|v| jmath::abs_long(median.wrapping_sub(*v)))
                    .collect();
                residuals.sort_unstable();
                Value::Long(median_of_longs(&residuals))
            }
            Sorted::Doubles(doubles) => {
                let median = median_of_doubles(&doubles);
                let mut residuals: Vec<f64> = doubles.iter().map(|v| (median - v).abs()).collect();
                residuals.sort_by(|a, b| java_double_compare(*a, *b));
                Value::Double(median_of_doubles(&residuals))
            }
            Sorted::Strings(strings) => {
                return Err(non_numeric_median(&strings, "java.lang.String"));
            }
            Sorted::Booleans(booleans) => {
                return Err(non_numeric_median(&booleans, "java.lang.Boolean"));
            }
        };
        Ok(Aggregated {
            tick: args.tick,
            location,
            elevation,
            value,
        })
    }
}

/// `Percentile`: the percentile of the values for one of the nine Hyndman & Fan sample
/// quantile definitions; types 1 and 3 select a datapoint (keeping its type, tick, location
/// and elevation), the others interpolate between two.
pub struct Percentile {
    /// The registered name, for the error messages.
    name: &'static str,
    /// The rank, in `[0, 100]`.
    rank: f64,
    /// The Hyndman & Fan type, `1..=9`.
    kind: u8,
    /// Whether a `null` value is an error (otherwise nulls are ignored).
    forbid_nulls: bool,
}

/// Doubles closer than this are equal (`Percentile.EPSILON`).
const EPSILON: f64 = 0.000_001;

impl Percentile {
    /// The percentile aggregator `name` of rank `percentile` (clamped to `[0, 100]`, a `NaN`
    /// staying `NaN` like `Math.min` / `Math.max`) and Hyndman & Fan type `kind`.
    pub fn new(name: &'static str, percentile: f64, kind: u8, forbid_nulls: bool) -> Self {
        Percentile {
            name,
            rank: jmath::min(100.0, jmath::max(0.0, percentile)),
            kind,
            forbid_nulls,
        }
    }

    /// The datapoint at sorted rank `j` as the result, with its value as is.
    fn at(args: &AggregatorArgs, indices: &[usize], j: usize) -> Aggregated {
        let i = indices[j];
        Aggregated {
            tick: args.ticks[i],
            location: args.locations[i],
            elevation: args.elevations[i],
            value: args.values[i].clone(),
        }
    }

    /// The datapoint at sorted rank `j` as the result, with its value as a double.
    fn double_at(args: &AggregatorArgs, indices: &[usize], j: usize) -> Result<Aggregated> {
        let mut result = Self::at(args, indices, j);
        result.value = Value::Double(double_value(&result.value)?);
        Ok(result)
    }

    /// The interpolation at `gamma` between the datapoints of sorted ranks `j` and `j + 1`
    /// (`n` non-null values), or the closest datapoint when `j` or `gamma` leave no room.
    #[allow(clippy::float_cmp)]
    fn interpolate(
        args: &AggregatorArgs,
        indices: &[usize],
        n: usize,
        j: i64,
        gamma: f64,
    ) -> Result<Aggregated> {
        if j < 0 {
            return Self::double_at(args, indices, 0);
        }
        let j = j as usize;
        if j >= n - 1 {
            return Self::double_at(args, indices, n - 1);
        }
        if gamma.abs() < EPSILON {
            return Self::double_at(args, indices, j);
        }
        if (gamma - 1.0).abs() < EPSILON {
            return Self::double_at(args, indices, j + 1);
        }
        let (a, b) = (indices[j], indices[j + 1]);
        let tick =
            jmath::round((1.0 - gamma) * args.ticks[a] as f64 + gamma * args.ticks[b] as f64);
        let location = match (args.locations[a], args.locations[b]) {
            (NO_LOCATION, location) | (location, NO_LOCATION) => location,
            (low, high) => {
                let (lat_low, lon_low) = geoxp::from_geoxp_point(low);
                let (lat_high, lon_high) = geoxp::from_geoxp_point(high);
                geoxp::to_geoxp_point(
                    (1.0 - gamma) * lat_low + gamma * lat_high,
                    (1.0 - gamma) * lon_low + gamma * lon_high,
                )
            }
        };
        let elevation = match (args.elevations[a], args.elevations[b]) {
            (NO_ELEVATION, elevation) | (elevation, NO_ELEVATION) => elevation,
            (low, high) => jmath::round((1.0 - gamma) * low as f64 + gamma * high as f64),
        };
        let value = (1.0 - gamma) * as_double(args.values, a) + gamma * as_double(args.values, b);
        Ok(Aggregated {
            tick,
            location,
            elevation,
            value: Value::Double(value),
        })
    }
}

impl Aggregator for Percentile {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.ticks.is_empty() {
            return Ok(Aggregated::none());
        }
        let values = args.values;
        let nulls = values.iter().filter(|v| v.is_null()).count();
        if nulls != 0 && self.forbid_nulls {
            return Err(Error::new(format!(
                "{} cannot compute percentile of null values.",
                self.name
            )));
        }
        if nulls == values.len() {
            return Ok(Aggregated {
                tick: 0,
                ..Aggregated::none()
            });
        }
        // The comparator's `RuntimeException` is wrapped in a `WarpScriptException`.
        let indices = sorted_indices(values, self.name).map_err(|err| {
            Error::with_cause(
                format!("java.lang.RuntimeException: {}", err.message()),
                err,
            )
        })?;
        let n = values.len() - nulls;
        let p = self.rank / 100.0;
        let pn = p * n as f64;
        match self.kind {
            1 => {
                // `ceil(pn)` is the 1-indexed rank, 0 selecting the lowest value too.
                let mut j = pn.ceil() as i64;
                if j > 0 {
                    j -= 1;
                }
                Ok(Self::at(args, indices.as_slice(), j as usize))
            }
            2 => {
                let j = pn.floor() as i64;
                let g = pn - j as f64;
                let gamma = if g.abs() < EPSILON { 0.5 } else { 1.0 };
                Self::interpolate(args, &indices, n, j - 1, gamma)
            }
            3 => {
                let mut j = (pn - 0.5).floor() as i64;
                let g = pn - 0.5 - j as f64;
                // Gamma is 0 or 1: X(j) or X(j + 1), 1-indexed, 0 selecting the lowest too.
                let gamma = i64::from(!(g.abs() < EPSILON && j % 2 == 0));
                if j + gamma > 0 {
                    j -= 1;
                }
                Ok(Self::at(args, indices.as_slice(), (j + gamma) as usize))
            }
            kind => {
                let m = match kind {
                    4 => 0.0,
                    5 => 0.5,
                    6 => p,
                    7 => 1.0 - p,
                    8 => (p + 1.0) / 3.0,
                    9 => p / 4.0 + 3.0 / 8.0,
                    _ => {
                        return Err(Error::new(format!("{} given invalid type.", self.name)));
                    }
                };
                let j = (pn + m).floor() as i64;
                let gamma = pn + m - j as f64;
                Self::interpolate(args, &indices, n, j - 1, gamma)
            }
        }
    }

    fn parameters(&self) -> Option<String> {
        Some(format!(
            "{} 'type{}'",
            java_double_to_string(self.rank),
            self.kind
        ))
    }
}

/// `Percentile.Builder`: `percentile [ 'typeX' ] name` pushes the configured aggregator.
fn percentile_builder(stack: &mut Stack, name: &'static str, forbid_nulls: bool) -> Result<()> {
    let mut top = stack.pop()?;
    let mut kind = 1u8;
    // The type is a string so as not to be confused with the percentile.
    if let Value::String(s) = &top {
        let lower = s.to_lowercase();
        let units: Vec<u16> = lower.encode_utf16().collect();
        if units.len() != 5 || !lower.starts_with("type") {
            return Err(Error::new(format!(
                "{name} expect percentile type to be 'typeX' where 0<X<10."
            )));
        }
        // `charAt(4) - '0'`
        let digit = i32::from(units[4]) - i32::from(b'0');
        if !(1..=9).contains(&digit) {
            return Err(Error::new(format!(
                "{name} expect percentile type to be 'typeX' where X is a integer and 0<X<10."
            )));
        }
        kind = digit as u8;
        top = stack.pop()?;
    }
    let Some(percentile) = top.as_f64() else {
        return Err(Error::new(format!("Invalid parameter for {name}")));
    };
    // A NaN passes (and stays NaN).
    if percentile < 0.0 || percentile > 100.0 {
        return Err(Error::new(format!(
            "Invalid percentile for {name}, MUST be between 0 and 100."
        )));
    }
    stack.push(Value::Function(Named::object(
        name,
        Percentile::new(name, percentile, kind, forbid_nulls),
    )));
    Ok(())
}
