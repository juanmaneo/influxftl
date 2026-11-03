// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The aggregators of change and motion over a window: `mapper.delta`, `mapper.rate`,
//! `mapper.highest` / `mapper.lowest` (by elevation), `mapper.hdist` / `mapper.hspeed`
//! (loxodromic distance), `mapper.vdist` / `mapper.vspeed` (elevation) and
//! `mapper.truecourse`. Registered as mappers, the Java classes also implement the
//! bucketizer and reducer interfaces.

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Num, Roles};
use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::functions::time::TIME_UNITS_PER_S;
use crate::geoxp;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::jmath;
use crate::value::{Value, double_to_long};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("mapper.delta", Entry::Object(|n| motion(n, Kind::Delta))),
    ("mapper.rate", Entry::Object(|n| motion(n, Kind::Rate))),
    (
        "mapper.highest",
        Entry::Object(|n| motion(n, Kind::Highest)),
    ),
    ("mapper.lowest", Entry::Object(|n| motion(n, Kind::Lowest))),
    ("mapper.hdist", Entry::Object(|n| motion(n, Kind::HDist))),
    ("mapper.hspeed", Entry::Object(|n| motion(n, Kind::HSpeed))),
    ("mapper.vdist", Entry::Object(|n| motion(n, Kind::VDist))),
    ("mapper.vspeed", Entry::Object(|n| motion(n, Kind::VSpeed))),
    (
        "mapper.truecourse",
        Entry::Object(|n| motion(n, Kind::TrueCourse)),
    ),
];

/// Elevation units per meter (`Constants.ELEVATION_UNITS_PER_M`).
const ELEVATION_UNITS_PER_M: f64 = 1000.0;

/// What a [`Motion`] aggregator computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `Delta`: the last value minus the first (by tick), at the delta of their ticks.
    Delta,
    /// `Rate`: the delta of the values per second.
    Rate,
    /// `Highest`: the datapoint of highest elevation (the latest of equal ones).
    Highest,
    /// `Lowest`: the datapoint of lowest elevation (the earliest of equal ones).
    Lowest,
    /// `HDist`: the loxodromic distance along the located datapoints, in meters.
    HDist,
    /// `HSpeed`: that distance per second over the span of the window.
    HSpeed,
    /// `VDist`: the sum of the elevation changes, in meters.
    VDist,
    /// `VSpeed`: the elevation change per second between the first and last datapoints.
    VSpeed,
    /// `TrueCourse`: the initial course from the first to the last datapoint, in degrees.
    TrueCourse,
}

/// A change or motion aggregator.
pub struct Motion {
    /// What the aggregator computes.
    pub kind: Kind,
}

/// The [`Motion`] aggregator `kind` registered as `name`.
fn motion(name: &'static str, kind: Kind) -> std::rc::Rc<dyn crate::value::FunctionObject> {
    Named::object(name, Motion { kind })
}

/// `GTSHelper.getFirstLastTicks`: the indices of the first lowest and first highest ticks,
/// `None` when no tick is below `Long.MAX_VALUE` (resp. above `Long.MIN_VALUE`), where Java
/// keeps `-1`.
fn first_last(ticks: &[i64]) -> (Option<usize>, Option<usize>) {
    let (mut first, mut last) = (None, None);
    let (mut first_tick, mut last_tick) = (i64::MAX, i64::MIN);
    for (i, &tick) in ticks.iter().enumerate() {
        if tick < first_tick {
            first = Some(i);
            first_tick = tick;
        }
        if tick > last_tick {
            last = Some(i);
            last_tick = tick;
        }
    }
    (first, last)
}

/// An index from [`first_last`], the `ArrayIndexOutOfBoundsException` of Java's `-1`.
fn index(index: Option<usize>, length: usize) -> Result<usize> {
    index.ok_or_else(|| Error::index_out_of_bounds(-1, length))
}

/// `(Number) value`: the `NullPointerException` of a `null`, the `ClassCastException` of a
/// non numeric value.
fn number(value: &Value) -> Result<Num> {
    match value {
        Value::Null => Err(Error::null_pointer()),
        other => Num::of(other).ok_or_else(|| super::class_cast(other)),
    }
}

/// `((Number) value).longValue()`.
fn long_of(value: Num) -> i64 {
    match value {
        Num::Long(l) => l,
        Num::Double(d) => double_to_long(d),
    }
}

/// The location and elevation of the first datapoint at the tick of the result.
fn at_tick(args: &AggregatorArgs) -> (i64, i64) {
    args.ticks
        .iter()
        .position(|&tick| tick == args.tick)
        .map_or((NO_LOCATION, NO_ELEVATION), |i| {
            (args.locations[i], args.elevations[i])
        })
}

/// The loxodromic distance along the located datapoints, in the order of the window.
fn horizontal_distance(locations: &[i64]) -> f64 {
    let mut last = NO_LOCATION;
    let mut total = 0.0;
    for &location in locations {
        if location == NO_LOCATION {
            continue;
        }
        if last != NO_LOCATION {
            total += geoxp::loxodromic_distance(last, location);
        }
        last = location;
    }
    total
}

/// A tick delta in seconds, as Java computes it (`delta / (double) TIME_UNITS_PER_S`).
fn seconds(from: i64, to: i64) -> f64 {
    to.wrapping_sub(from) as f64 / TIME_UNITS_PER_S as f64
}

impl Motion {
    /// `Delta` and `Rate`.
    #[allow(clippy::float_cmp)]
    fn change(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let (first, last) = first_last(args.ticks);
        let length = args.ticks.len();
        let rate = self.kind == Kind::Rate;
        // `Rate` reads the ticks of both indices first, `Delta` the last value.
        if rate {
            index(first, length)?;
            index(last, length)?;
        }
        let last = index(last, length)?;
        let value = match &args.values[last] {
            Value::Long(l) => {
                let first = index(first, length)?;
                let delta = l.wrapping_sub(long_of(number(&args.values[first])?));
                (first, Num::Long(delta))
            }
            Value::Double(d) => {
                let first = index(first, length)?;
                (
                    first,
                    Num::Double(d - number(&args.values[first])?.as_f64()),
                )
            }
            _ => return Ok(Aggregated::none()),
        };
        let (first, delta) = value;
        let (first_tick, last_tick) = (args.ticks[first], args.ticks[last]);
        let value = match (rate, delta) {
            (false, Num::Long(l)) => Value::Long(l),
            (false, Num::Double(d)) => Value::Double(d),
            // As Java computes it: `0.0D == value`, so `-0.0` is returned as is.
            (true, delta) if delta.as_f64() == 0.0 || first_tick == last_tick => {
                Value::Double(delta.as_f64())
            }
            (true, delta) => Value::Double(delta.as_f64() / seconds(first_tick, last_tick)),
        };
        Ok(Aggregated {
            tick: last_tick.wrapping_sub(first_tick),
            location: args.locations[last],
            elevation: args.elevations[last],
            value,
        })
    }

    /// `Highest` and `Lowest`.
    fn extreme(&self, args: &AggregatorArgs) -> Aggregated {
        let highest = self.kind == Kind::Highest;
        let (mut timestamp, mut elevation) = if highest {
            (i64::MIN, i64::MIN)
        } else {
            (i64::MAX, i64::MAX)
        };
        let mut found = None;
        for i in 0..args.values.len() {
            let (e, t) = (args.elevations[i], args.ticks[i]);
            let better = if highest {
                e > elevation || e == elevation && t > timestamp
            } else {
                e < elevation || e == elevation && t < timestamp
            };
            if e != NO_ELEVATION && better {
                timestamp = t;
                elevation = e;
                found = Some(i);
            }
        }
        found.map_or_else(Aggregated::none, |i| Aggregated {
            tick: args.ticks[i],
            location: args.locations[i],
            elevation: args.elevations[i],
            value: args.values[i].clone(),
        })
    }

    /// `HDist`, `HSpeed`, `VDist`, `VSpeed` and `TrueCourse`, at the tick of the result.
    fn kinematics(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let ticks = args.ticks;
        let value = match self.kind {
            Kind::HDist => Value::Double(horizontal_distance(args.locations)),
            Kind::HSpeed => {
                // The distance follows the order of the window, the duration its span.
                let distance = horizontal_distance(args.locations);
                let (min, max) = ticks.iter().fold((i64::MAX, i64::MIN), |(min, max), &t| {
                    (min.min(t), max.max(t))
                });
                if min == max {
                    Value::Double(0.0)
                } else {
                    Value::Double(distance / seconds(min, max))
                }
            }
            Kind::VDist => {
                let mut last = NO_ELEVATION;
                let mut total = 0i64;
                for &elevation in args.elevations {
                    if elevation == NO_ELEVATION {
                        continue;
                    }
                    if last != NO_ELEVATION {
                        total = total.wrapping_add(last.wrapping_sub(elevation).wrapping_abs());
                    }
                    last = elevation;
                }
                Value::Double(total as f64 / ELEVATION_UNITS_PER_M)
            }
            Kind::VSpeed => {
                let (first, last) = first_last(ticks);
                let (first, last) = (index(first, ticks.len())?, index(last, ticks.len())?);
                let (from, to) = (args.elevations[first], args.elevations[last]);
                if from == NO_ELEVATION || to == NO_ELEVATION {
                    Value::Null
                } else {
                    let vspeed = to.wrapping_sub(from) as f64;
                    if vspeed == 0.0 {
                        Value::Double(0.0)
                    } else {
                        Value::Double(
                            vspeed / ELEVATION_UNITS_PER_M / seconds(ticks[first], ticks[last]),
                        )
                    }
                }
            }
            _ => {
                // The first and last datapoints of the window, which is not empty.
                let locations = args.locations;
                let (from, to) = (locations[0], locations[locations.len() - 1]);
                if from == NO_LOCATION || to == NO_LOCATION {
                    return Ok(Aggregated::none());
                }
                Value::Double(true_course(from, to))
            }
        };
        let (location, elevation) = at_tick(args);
        Ok(Aggregated {
            tick: args.tick,
            location,
            elevation,
            value,
        })
    }
}

/// `TrueCourse`: the initial course from `from` to `to` in degrees, `[0, 360)`. As in
/// Warp 10, the first term uses the cosine of the destination's longitude.
fn true_course(from: i64, to: i64) -> f64 {
    let (lat1, lon1) = geoxp::from_geoxp_point(from);
    let (lat2, lon2) = geoxp::from_geoxp_point(to);
    let (lat1, lon1) = (lat1.to_radians(), lon1.to_radians());
    let (lat2, lon2) = (lat2.to_radians(), lon2.to_radians());
    let y = (lon1 - lon2).sin() * lon2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * (lon1 - lon2).cos();
    let mut tc = jmath::ieee_remainder(y.atan2(x), 2.0 * std::f64::consts::PI);
    if tc < 0.0 {
        tc = tc + std::f64::consts::PI + std::f64::consts::PI;
    }
    tc.to_degrees()
}

impl Aggregator for Motion {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.ticks.is_empty() {
            return Ok(Aggregated::none());
        }
        match self.kind {
            Kind::Delta | Kind::Rate => self.change(args),
            Kind::Highest | Kind::Lowest => Ok(self.extreme(args)),
            _ => self.kinematics(args),
        }
    }
}
