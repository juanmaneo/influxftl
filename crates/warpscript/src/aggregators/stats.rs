// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The statistical aggregators: `rms`, `var` / `sd` (with their Welford variants) and
//! `mean.circular`, the last three built by stack functions consuming their parameter.

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Roles, double_value, latest};
use crate::error::{Error, Result};
use crate::format::java_double_to_string;
use crate::functions::Entry;
use crate::geoxp;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::stack::Stack;
use crate::value::Value;

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "bucketizer.rms",
        Entry::Object(|n| {
            Named::object(
                n,
                Rms {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "mapper.rms",
        Entry::Object(|n| {
            Named::object(
                n,
                Rms {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.rms",
        Entry::Object(|n| {
            Named::object(
                n,
                Rms {
                    ignore_nulls: false,
                },
            )
        }),
    ),
    (
        "reducer.rms.exclude-nulls",
        Entry::Object(|n| Named::object(n, Rms { ignore_nulls: true })),
    ),
    (
        "bucketizer.var",
        Entry::Builder(|s, n| variance_builder(s, n, false, false, false)),
    ),
    (
        "bucketizer.var.welford",
        Entry::Builder(|s, n| variance_builder(s, n, false, true, false)),
    ),
    (
        "mapper.var",
        Entry::Builder(|s, n| variance_builder(s, n, false, false, false)),
    ),
    (
        "mapper.var.welford",
        Entry::Builder(|s, n| variance_builder(s, n, false, true, false)),
    ),
    (
        "reducer.var",
        Entry::Builder(|s, n| variance_builder(s, n, false, false, false)),
    ),
    (
        "reducer.var.welford",
        Entry::Builder(|s, n| variance_builder(s, n, false, true, false)),
    ),
    (
        "reducer.var.forbid-nulls",
        Entry::Builder(|s, n| variance_builder(s, n, true, false, false)),
    ),
    (
        "reducer.var.welford.forbid-nulls",
        Entry::Builder(|s, n| variance_builder(s, n, true, true, false)),
    ),
    (
        "bucketizer.sd",
        Entry::Builder(|s, n| variance_builder(s, n, false, false, true)),
    ),
    (
        "bucketizer.sd.welford",
        Entry::Builder(|s, n| variance_builder(s, n, false, true, true)),
    ),
    (
        "mapper.sd",
        Entry::Builder(|s, n| variance_builder(s, n, false, false, true)),
    ),
    (
        "mapper.sd.welford",
        Entry::Builder(|s, n| variance_builder(s, n, false, true, true)),
    ),
    (
        "reducer.sd",
        Entry::Builder(|s, n| variance_builder(s, n, false, false, true)),
    ),
    (
        "reducer.sd.welford",
        Entry::Builder(|s, n| variance_builder(s, n, false, true, true)),
    ),
    (
        "reducer.sd.forbid-nulls",
        Entry::Builder(|s, n| variance_builder(s, n, true, false, true)),
    ),
    (
        "reducer.sd.welford.forbid-nulls",
        Entry::Builder(|s, n| variance_builder(s, n, true, true, true)),
    ),
    (
        "bucketizer.mean.circular",
        Entry::Builder(|s, n| circular_mean_builder(s, n, true)),
    ),
    (
        "mapper.mean.circular",
        Entry::Builder(|s, n| circular_mean_builder(s, n, true)),
    ),
    (
        "reducer.mean.circular",
        Entry::Builder(|s, n| circular_mean_builder(s, n, true)),
    ),
    (
        "reducer.mean.circular.exclude-nulls",
        Entry::Builder(|s, n| circular_mean_builder(s, n, false)),
    ),
];

/// `RMS`: the root mean square of the values, computed with doubles and cast back to a
/// `LONG` when the first value is one; location and elevation of the most recent datapoint.
pub struct Rms {
    /// Whether `null` values are skipped rather than producing no result.
    pub ignore_nulls: bool,
}

impl Aggregator for Rms {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.ticks.is_empty() {
            return Ok(Aggregated::none());
        }
        let (location, elevation) = latest(args);
        let mut long_type = None;
        let mut sqsum = 0.0f64;
        let mut nulls = 0usize;
        for value in args.values {
            if value.is_null() {
                nulls += 1;
                continue;
            }
            if long_type.is_none() {
                long_type = Some(match value {
                    Value::Long(_) => true,
                    Value::Double(_) => false,
                    // The RMS of strings or booleans has no meaning.
                    _ => return Ok(Aggregated::none()),
                });
            }
            sqsum += crate::jmath::pow(double_value(value)?, 2.0);
        }
        // Nulls poison the result unless ignored; all nulls would divide by zero.
        if (!self.ignore_nulls && nulls > 0) || nulls == args.values.len() {
            return Ok(Aggregated::none());
        }
        let rms = (sqsum / (args.values.len() - nulls) as f64).sqrt();
        let value = if long_type == Some(true) {
            Value::Long(rms as i64)
        } else {
            Value::Double(rms)
        };
        Ok(Aggregated {
            tick: args.tick,
            location,
            elevation,
            value,
        })
    }
}

/// `Variance` / `VarianceWelford`: the variance of the values (a `DOUBLE`), by the naive
/// `E[X²] - E[X]²` formula or Welford's online algorithm, with Bessel's correction on
/// request; location and elevation of the most recent datapoint.
pub struct Variance {
    /// Apply Bessel's correction (`n / (n - 1)`).
    pub use_bessel: bool,
    /// Whether a `null` value produces no result (otherwise nulls are skipped).
    pub forbid_nulls: bool,
    /// Use Welford's algorithm rather than the naive formula.
    pub welford: bool,
}

/// The `(location, elevation, variance)` of the window, `None` when there is no result.
fn variance_of(variance: &Variance, args: &AggregatorArgs) -> Result<Option<(i64, i64, f64)>> {
    if args.ticks.is_empty() {
        return Ok(None);
    }
    let mut location = NO_LOCATION;
    let mut elevation = NO_ELEVATION;
    let mut timestamp = i64::MIN;
    let mut nticks = 0usize;
    // Naive: the running sum and sum of squares, typed by the first value.
    let (mut sum, mut sumsq) = (0.0f64, 0.0f64);
    let mut typed = false;
    // Welford: the running mean and sum of squared differences.
    let (mut m, mut s) = (0.0f64, 0.0f64);
    for (i, value) in args.values.iter().enumerate() {
        if value.is_null() {
            if variance.forbid_nulls {
                return Ok(None);
            }
            continue;
        }
        nticks += 1;
        if args.ticks[i] > timestamp {
            location = args.locations[i];
            elevation = args.elevations[i];
            timestamp = args.ticks[i];
        }
        if variance.welford {
            // The variance of strings or booleans has no meaning.
            let Some(x) = value.as_f64() else {
                return Ok(None);
            };
            // Warp 10 divides by the index in the window, nulls included.
            if i == 0 {
                m = x;
                s = 0.0;
            } else {
                let mnew = m + (x - m) / (i + 1) as f64;
                s += (x - m) * (x - mnew);
                m = mnew;
            }
        } else if typed {
            let v = double_value(value)?;
            sum += v;
            sumsq += v * v;
        } else {
            // The variance of strings or booleans has no meaning.
            let Some(v) = value.as_f64() else {
                return Ok(None);
            };
            typed = true;
            sum = v;
            sumsq = sum * sum;
        }
    }
    let n = nticks as f64;
    let value = if variance.welford {
        if nticks > 1 && variance.use_bessel {
            s / (n - 1.0)
        } else {
            s / n
        }
    } else {
        let mut v = (sumsq / n) - (sum * sum) / (n * n);
        if nticks > 1 && variance.use_bessel {
            v = v * n / (n - 1.0);
        }
        v
    };
    Ok(Some((location, elevation, value)))
}

/// The result of `Variance` / `StandardDeviation` from the window's `(location, elevation,
/// value)`.
fn aggregated(result: Option<(i64, i64, f64)>) -> Aggregated {
    match result {
        Some((location, elevation, value)) => Aggregated {
            tick: 0,
            location,
            elevation,
            value: Value::Double(value),
        },
        None => Aggregated::none(),
    }
}

impl Aggregator for Variance {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        Ok(aggregated(variance_of(self, args)?))
    }

    fn parameters(&self) -> Option<String> {
        Some(self.use_bessel.to_string())
    }
}

/// `StandardDeviation`: the square root of the variance.
pub struct StandardDeviation {
    /// The variance it is the square root of.
    pub variance: Variance,
}

impl Aggregator for StandardDeviation {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let result = variance_of(&self.variance, args)?;
        Ok(aggregated(result.map(|(location, elevation, variance)| {
            (location, elevation, variance.sqrt())
        })))
    }

    fn parameters(&self) -> Option<String> {
        self.variance.parameters()
    }
}

/// `Variance.Builder` / `StandardDeviation.Builder`: `useBessel name` pushes the configured
/// aggregator.
fn variance_builder(
    stack: &mut Stack,
    name: &'static str,
    forbid_nulls: bool,
    welford: bool,
    standard_deviation: bool,
) -> Result<()> {
    let Value::Boolean(use_bessel) = stack.pop()? else {
        return Err(Error::new(format!(
            "{name} expects a boolean parameter to determine whether or not to apply Bessel's correction."
        )));
    };
    let variance = Variance {
        use_bessel,
        forbid_nulls,
        welford,
    };
    let object = if standard_deviation {
        Named::object(name, StandardDeviation { variance })
    } else {
        Named::object(name, variance)
    };
    stack.push(Value::Function(object));
    Ok(())
}

/// `CircularMean`: the mean of circular quantities of the given period (the angle of the
/// mean of the unit vectors), at the mean location and elevation.
pub struct CircularMean {
    /// The period of the quantity (`360` for degrees).
    pub period: f64,
    /// Whether a `null` value produces no result (otherwise nulls are skipped).
    pub forbid_nulls: bool,
}

impl Aggregator for CircularMean {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.ticks.is_empty() {
            return Ok(Aggregated::none());
        }
        let (mut sinsum, mut cossum) = (0.0f64, 0.0f64);
        let mut typed = false;
        let (mut latitudes, mut longitudes, mut locationcount) = (0i64, 0i64, 0i64);
        let (mut elev, mut elevationcount) = (0i64, 0i64);
        let mut nticks = 0usize;
        for (i, value) in args.values.iter().enumerate() {
            if value.is_null() {
                if self.forbid_nulls {
                    return Ok(Aggregated::none());
                }
                continue;
            }
            nticks += 1;
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
            let v = if typed {
                double_value(value)?
            } else {
                // The mean of strings or booleans has no meaning.
                let Some(v) = value.as_f64() else {
                    return Ok(Aggregated::none());
                };
                typed = true;
                v
            };
            let angle = std::f64::consts::PI * 2.0 * (v / self.period);
            sinsum += angle.sin();
            cossum += angle.cos();
        }
        let mut circularmean = if nticks == 0 {
            f64::NAN
        } else {
            sinsum.atan2(cossum)
        };
        if !circularmean.is_nan() {
            circularmean = circularmean * self.period / (2.0 * std::f64::consts::PI);
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
        Ok(Aggregated {
            tick: 0,
            location,
            elevation,
            value: Value::Double(circularmean),
        })
    }

    fn parameters(&self) -> Option<String> {
        Some(java_double_to_string(self.period))
    }
}

/// `CircularMean.Builder`: `period name` pushes the configured aggregator.
fn circular_mean_builder(stack: &mut Stack, name: &'static str, forbid_nulls: bool) -> Result<()> {
    let top = stack.pop()?;
    let period = top.as_f64().filter(|p| p.is_finite() && *p > 0.0);
    let Some(period) = period else {
        return Err(Error::new(format!(
            "{name} expects a finite, strictly positive, numeric 'period' parameter."
        )));
    };
    stack.push(Value::Function(Named::object(
        name,
        CircularMean {
            period,
            forbid_nulls,
        },
    )));
    Ok(())
}
