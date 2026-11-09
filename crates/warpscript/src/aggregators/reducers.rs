// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `reducer.argmin` / `reducer.argmax` (`Argminmax`: the values of a label of the series
//! holding the extremum) and `reducer.shannonentropy.0` / `.1` (`ShannonEntropy`: the
//! normalized entropy of the values). Registered as reducers, the Java classes also implement
//! the mapper and bucketizer interfaces.

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Num, Roles};
use crate::error::{Error, Result};
use crate::format::url_encode;
use crate::functions::Entry;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::Value;

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "reducer.argmin",
        Entry::Builder(|s, n| argminmax_builder(s, n, true)),
    ),
    (
        "reducer.argmax",
        Entry::Builder(|s, n| argminmax_builder(s, n, false)),
    ),
    (
        "reducer.shannonentropy.0",
        Entry::Object(|n| {
            Named::object(
                n,
                ShannonEntropy {
                    name: n,
                    invert: false,
                },
            )
        }),
    ),
    (
        "reducer.shannonentropy.1",
        Entry::Object(|n| {
            Named::object(
                n,
                ShannonEntropy {
                    name: n,
                    invert: true,
                },
            )
        }),
    ),
];

/// A result at the tick of the window, without location nor elevation.
fn at_tick(args: &AggregatorArgs, value: Value) -> Aggregated {
    Aggregated {
        tick: args.tick,
        location: NO_LOCATION,
        elevation: NO_ELEVATION,
        value,
    }
}

/// `Argminmax`: the comma separated, URL encoded values of `label` of the series holding the
/// minimum (maximum) value, in the order of the series, at most `count` of them (all when
/// `count` is not positive). `LONG` values compare as `long`s until a `DOUBLE` is met.
pub struct Argminmax {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// The label to report.
    pub label: String,
    /// The most values to report.
    pub count: i32,
    /// `argmin` rather than `argmax`.
    pub argmin: bool,
}

impl Argminmax {
    /// The indices of the extrema among the values (`null`s skipped).
    #[allow(clippy::float_cmp)]
    fn extrema(&self, args: &AggregatorArgs) -> Result<Vec<usize>> {
        let mut found: Vec<usize> = Vec::new();
        let mut is_double = false;
        let mut lmax = if self.argmin { i64::MAX } else { i64::MIN };
        let mut dmax = if self.argmin {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
        for (i, value) in args.values.iter().enumerate() {
            if value.is_null() {
                continue;
            }
            // `labels[i]`: `BUCKETIZE` provides a single label map.
            let labels = args
                .labels
                .get(i)
                .ok_or_else(|| Error::index_out_of_bounds(i as i64, args.labels.len()))?;
            if !labels.contains_key(&self.label) {
                return Err(Error::usage(
                    self.name,
                    format!("expects all labels to contain label '{}'.", self.label),
                ));
            }
            let Some(number) = Num::of(value) else {
                return Err(Error::usage(
                    self.name,
                    "can only operate on numerical values.",
                ));
            };
            let number = match number {
                Num::Long(l) if is_double => Num::Double(l as f64),
                Num::Double(d) if !is_double => {
                    // Only once an extremum was found.
                    if !found.is_empty() {
                        dmax = lmax as f64;
                    }
                    is_double = true;
                    Num::Double(d)
                }
                number => number,
            };
            // As Java computes it, `==` then `>` (inverted for `argmin`).
            let (equal, beyond) = match number {
                Num::Long(l) => (l == lmax, (l > lmax) ^ self.argmin),
                Num::Double(d) => (d == dmax, (d > dmax) ^ self.argmin),
            };
            if equal {
                found.push(i);
            } else if beyond {
                found.clear();
                found.push(i);
                match number {
                    Num::Long(l) => lmax = l,
                    Num::Double(d) => dmax = d,
                }
            }
        }
        Ok(found)
    }
}

impl Aggregator for Argminmax {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let found = self.extrema(args)?;
        let limit = usize::try_from(self.count)
            .ok()
            .filter(|&count| count > 0)
            .unwrap_or(usize::MAX);
        let reported: Vec<String> = found
            .iter()
            .take(limit)
            .map(|&i| url_encode(args.labels[i].get(&self.label).map_or("", String::as_str)))
            .collect();
        Ok(at_tick(args, Value::string(reported.join(","))))
    }

    fn parameters(&self) -> Option<String> {
        Some(format!(
            "{} {}",
            value_snapshot(&Value::string(&self.label)).trim_end(),
            self.count
        ))
    }
}

/// `Argminmax.Builder`: `'label' count reducer.argmax` (the count an `int`).
fn argminmax_builder(stack: &mut Stack, name: &'static str, argmin: bool) -> Result<()> {
    let Value::Long(count) = stack.pop()? else {
        return Err(Error::usage(
            name,
            "expects an integer number of maxima to report (use 0 to report them all).",
        ));
    };
    let Value::String(label) = stack.pop()? else {
        return Err(Error::usage(
            name,
            "expects the name of the label to report on the second level of the stack.",
        ));
    };
    stack.push(Value::Function(Named::object(
        name,
        Argminmax {
            name,
            label: label.to_string(),
            count: count as i32,
            argmin,
        },
    )));
    Ok(())
}

/// `ShannonEntropy`: the Shannon entropy of the non-null `LONG` values taken as frequencies,
/// normalized by the logarithm of their count; a single value gives `0.0` (`1.0` for the
/// inverted form, `reducer.shannonentropy.1`).
pub struct ShannonEntropy {
    /// The registered name, for the error messages.
    pub name: &'static str,
    /// Whether a single value gives 1.
    pub invert: bool,
}

impl Aggregator for ShannonEntropy {
    fn roles(&self) -> Roles {
        Roles::ALL
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let mut longs = Vec::with_capacity(args.values.len());
        let mut sum = 0.0;
        for value in args.values {
            match value {
                Value::Null => {}
                Value::Long(l) => {
                    longs.push(*l);
                    sum += *l as f64;
                }
                _ => {
                    return Err(Error::usage(self.name, "can only operate on long values."));
                }
            }
        }
        let n = longs.len() as i32;
        let entropy = if n == 1 {
            if self.invert { 1.0 } else { 0.0 }
        } else {
            let mut h = 0.0;
            for &x in longs.iter().filter(|&&x| x != 0) {
                let p = x as f64 / sum;
                h -= p * p.ln();
            }
            h / f64::from(n).ln()
        };
        Ok(at_tick(args, Value::Double(entropy)))
    }
}
