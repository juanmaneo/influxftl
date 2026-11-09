// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Function builders: `POLYFUNC` (a polynomial, applicable to numbers, lists and series and
//! usable as a mapper) and `RANDPDF` / `SRANDPDF` (a random value drawn from a value
//! histogram).

use std::cell::RefCell;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::math::{ATTRIBUTE_SEEDED_PRNG, secure_double};
use crate::aggregators::{Aggregated, Aggregator, AggregatorArgs, Roles};
use crate::curvefit::polynomial_value;
use crate::encoder::EncodedValue;
use crate::error::math::MathError;
use crate::error::{Class, Error, Result};
use crate::gts::{GeoTimeSerie, NO_ELEVATION, NO_LOCATION, java_binary_search};
use crate::jrandom::JavaRandom;
use crate::macros::{Callable, StackFunction, StackObject};
use crate::snapshot::value_snapshot;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("POLYFUNC", Entry::Stack(polyfunc)),
    ("RANDPDF", Entry::Builder(randpdf)),
    ("SRANDPDF", Entry::Builder(randpdf)),
];

// -----------------------------------------------------------------------------------------
// POLYFUNC
// -----------------------------------------------------------------------------------------

/// The function `POLYFUNC` builds (`PolynomialFunction`).
#[derive(Debug)]
struct PolyFunc {
    /// The coefficients, constant term first, without trailing zeros (but at least one).
    coefficients: Vec<f64>,
}

impl PolyFunc {
    fn value(&self, x: f64) -> f64 {
        polynomial_value(&self.coefficients, x)
    }

    /// `GTSHelper.locationAtTick` / `elevationAtTick`: the location and elevation of the
    /// datapoint at `tick` (the series is sorted in place when it has any).
    fn geo_at_tick(gts: &mut GeoTimeSerie, tick: i64) -> (i64, i64) {
        let has_locations = gts.locations.is_some();
        let has_elevations = gts.elevations.is_some();
        if has_locations || has_elevations {
            gts.sort(false);
        }
        match java_binary_search(&gts.ticks, tick) {
            Some(idx) => (gts.location_at(idx), gts.elevation_at(idx)),
            None => (NO_LOCATION, NO_ELEVATION),
        }
    }
}

impl StackObject for PolyFunc {
    fn name(&self) -> &'static str {
        "POLYFUNC"
    }

    fn to_snapshot(&self) -> String {
        let coefficients = Value::list(
            self.coefficients
                .iter()
                .map(|&c| Value::Double(c))
                .collect(),
        );
        format!("{} POLYFUNC", value_snapshot(&coefficients))
    }

    fn type_name(&self) -> &'static str {
        "AGGREGATOR"
    }

    fn as_aggregator(&self) -> Option<&dyn Aggregator> {
        Some(self)
    }

    fn apply(&self, stack: &mut Stack) -> Result<()> {
        const NAME: &str = "POLYFUNC";
        let mut top = stack.pop()?;
        let mut use_bucket_id = false;
        if let Value::Boolean(b) = top {
            use_bucket_id = b;
            top = stack.pop()?;
        }
        let bucketized = matches!(&top, Value::Gts(gts) if gts.borrow().is_bucketized());
        if use_bucket_id && !bucketized {
            return Err(fail(
                NAME,
                "can only be applied to bucketized numerical Geo Time Series.",
            ));
        }
        let result = match top {
            Value::List(list) => {
                let mut values = Vec::with_capacity(list.borrow().len());
                for element in list.borrow().iter() {
                    let Some(x) = element.as_f64() else {
                        return Err(fail(NAME, "can only be applied to numerical values."));
                    };
                    values.push(Value::Double(self.value(x)));
                }
                Value::list(values)
            }
            Value::Gts(gts) => {
                let mut g = gts.borrow_mut();
                let mut out = GeoTimeSerie::new();
                if bucketized {
                    let n = g.bucketcount as i32;
                    let (lastbucket, bucketspan) = (g.lastbucket, g.bucketspan);
                    for i in 0..n {
                        let ts = if use_bucket_id {
                            i64::from(n - 1 - i)
                        } else {
                            lastbucket.wrapping_sub(i64::from(i).wrapping_mul(bucketspan))
                        };
                        let (location, elevation) = PolyFunc::geo_at_tick(&mut g, ts);
                        out.set_value(
                            ts,
                            location,
                            elevation,
                            &Value::Double(self.value(ts as f64)),
                            false,
                        );
                    }
                } else {
                    for i in 0..g.size() {
                        let ts = g.ticks[i];
                        out.set_value(
                            ts,
                            g.location_at(i),
                            g.elevation_at(i),
                            &Value::Double(self.value(ts as f64)),
                            false,
                        );
                    }
                }
                Value::Gts(out.shared())
            }
            Value::Encoder(encoder) => {
                let encoder = encoder.borrow();
                let mut decoder = encoder.decoder();
                let mut out = encoder.clone_empty();
                while decoder.next()? {
                    let ts = decoder.timestamp();
                    out.add_value(
                        ts,
                        decoder.location(),
                        decoder.elevation(),
                        &EncodedValue::Double(self.value(ts as f64)),
                    );
                }
                Value::Encoder(Rc::new(RefCell::new(out)))
            }
            other => match other.as_f64() {
                Some(x) => Value::Double(self.value(x)),
                None => {
                    return Err(fail(
                        NAME,
                        "can only be applied to numerical values which may appear in LISTs, GTS or ENCODERs.",
                    ));
                }
            },
        };
        stack.push(result);
        Ok(())
    }
}

impl Aggregator for PolyFunc {
    fn roles(&self) -> Roles {
        Roles {
            bucketizer: false,
            mapper: true,
            reducer: false,
        }
    }

    /// The polynomial at the tick, with the location and elevation of the single datapoint.
    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let (location, elevation) = match args.values.len() {
            0 => (NO_LOCATION, NO_ELEVATION),
            1 => (args.locations[0], args.elevations[0]),
            _ => {
                return Err(Error::new(
                    "POLYFUNC can only be applied to a single value.",
                ));
            }
        };
        Ok(Aggregated {
            tick: args.tick,
            location,
            elevation,
            value: Value::Double(self.value(args.tick as f64)),
        })
    }
}

/// `POLYFUNC`: `[ coefficients ] POLYFUNC`, constant term first.
fn polyfunc(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "POLYFUNC";
    let Value::List(list) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a list of polynomial coefficients on top of the stack.",
        ));
    };
    let mut coefficients = Vec::with_capacity(list.borrow().len());
    for element in list.borrow().iter() {
        let Some(c) = element.as_f64() else {
            return Err(fail(
                NAME,
                "expects polynomial coefficients to be numerical.",
            ));
        };
        coefficients.push(c);
    }
    if coefficients.is_empty() {
        return Err(MathError::EmptyPolynomialCoefficients.into());
    }
    // `PolynomialFunction` drops the trailing zero coefficients.
    while coefficients.len() > 1 && coefficients[coefficients.len() - 1] == 0.0 {
        coefficients.pop();
    }
    stack.push(StackFunction::value(Callable::Object(Rc::new(PolyFunc {
        coefficients,
    }))));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// RANDPDF
// -----------------------------------------------------------------------------------------

/// The function `RANDPDF` / `SRANDPDF` builds: draws a value with the probabilities of a
/// histogram, from a secure random source or from the seeded PRNG (`PRNG`).
#[derive(Debug)]
struct RandPdf {
    name: &'static str,
    seeded: bool,
    /// The values in their natural order (`TreeMap`), with their probabilities.
    probabilities: Vec<(Value, f64)>,
    cumulative: Vec<f64>,
}

impl RandPdf {
    /// The value whose cumulative probability interval contains `rand`
    /// (`Arrays.binarySearch`, which the cumulative probabilities of negative or zero counts
    /// may defeat; `rand` is never `NaN` nor `-0.0`).
    fn pick(&self, rand: f64) -> Result<Value> {
        let mut low = 0i64;
        let mut high = self.cumulative.len() as i64 - 1;
        let mut pos = None;
        while pos.is_none() && low <= high {
            let mid = (low + high) >> 1;
            let mid_val = self.cumulative[mid as usize];
            if mid_val < rand {
                low = mid + 1;
            } else if mid_val > rand || mid_val.is_nan() {
                // A `NaN` sorts after every number (and the sums are never `-0.0`).
                high = mid - 1;
            } else {
                pos = Some(mid as usize);
            }
        }
        let pos = pos.unwrap_or(low as usize);
        let pos = if pos == self.cumulative.len() {
            pos.checked_sub(1)
                .ok_or_else(|| Error::index_out_of_bounds(-1, 0))?
        } else {
            pos
        };
        Ok(self.probabilities[pos].0.clone())
    }

    /// Draw a value, `entropy` feeding the unseeded form.
    fn draw(
        &self,
        stack: &mut Stack,
        entropy: fn() -> core::result::Result<u64, getrandom::Error>,
    ) -> Result<()> {
        let rand = if self.seeded {
            let Some(Value::Long(state)) = stack.attribute(ATTRIBUTE_SEEDED_PRNG) else {
                return Err(fail(self.name, "seeded PRNG was not initialized."));
            };
            let mut generator = JavaRandom::from_state(*state);
            let rand = generator.next_double();
            stack.set_attribute(ATTRIBUTE_SEEDED_PRNG, Some(Value::Long(generator.state())));
            rand
        } else {
            secure_double(entropy())?
        };
        let value = self.pick(rand)?;
        stack.push(value);
        Ok(())
    }
}

impl StackObject for RandPdf {
    fn name(&self) -> &str {
        self.name
    }

    fn to_snapshot(&self) -> String {
        let mut out = String::from("{ ");
        for (value, p) in &self.probabilities {
            out.push_str(&value_snapshot(value));
            out.push_str(&value_snapshot(&Value::Double(*p)));
        }
        out.push_str(&format!(" }} {}", self.name));
        out
    }

    fn apply(&self, stack: &mut Stack) -> Result<()> {
        self.draw(stack, getrandom::u64)
    }
}

/// A key of a `TreeMap` in its natural order (`Comparable.compareTo`); keys of different
/// classes are never compared.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SortKey {
    Text(Vec<u16>),
    Long(i64),
    /// `Double.compareTo`: the total order of the bits.
    Double(i64),
    Boolean(bool),
}

impl SortKey {
    /// The sort key of a histogram value, which must be comparable.
    fn of(key: &Value) -> Result<SortKey> {
        Ok(match key {
            Value::Null => return Err(Error::null_pointer()),
            Value::String(s) => SortKey::Text(s.encode_utf16().collect()),
            Value::Long(l) => SortKey::Long(*l),
            Value::Double(d) => {
                let bits = d.to_bits() as i64;
                SortKey::Double(bits ^ ((((bits >> 63) as u64) >> 1) as i64))
            }
            Value::Boolean(b) => SortKey::Boolean(*b),
            other => {
                return Err(Error::class_cast(other.kind(), Class::Comparable));
            }
        })
    }
}

/// `RANDPDF` / `SRANDPDF`: `histogram RANDPDF`, a function drawing a value of the histogram
/// with its probability.
fn randpdf(stack: &mut Stack, name: &'static str) -> Result<()> {
    let Value::Map(histogram) = stack.pop()? else {
        return Err(fail(name, "expects a value histogram on top of the stack."));
    };
    let histogram = histogram.borrow();
    let mut total = 0.0;
    let mut counts = Vec::with_capacity(histogram.len());
    for value in histogram.values() {
        let count = match value {
            Value::Null => return Err(Error::null_pointer()),
            other => other
                .as_f64()
                .ok_or_else(|| Error::class_cast(other.kind(), Class::Number))?,
        };
        total += count;
        counts.push(count);
    }
    // The keys of a `TreeMap`: comparable, and all of the class of the first one.
    let mut entries: Vec<(SortKey, Value, f64)> = Vec::with_capacity(histogram.len());
    for (key, count) in histogram.keys().zip(counts) {
        let sort_key = SortKey::of(key)?;
        if let Some((_, first, _)) = entries.first()
            && std::mem::discriminant(first) != std::mem::discriminant(key)
        {
            return Err(Error::class_cast(first.kind(), key.kind()));
        }
        entries.push((sort_key, key.clone(), count / total));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let probabilities: Vec<(Value, f64)> = entries.into_iter().map(|(_, k, p)| (k, p)).collect();
    let mut cumulative = Vec::with_capacity(probabilities.len());
    let mut sum = 0.0;
    for (_, p) in &probabilities {
        sum += p;
        cumulative.push(sum);
    }
    stack.push(StackFunction::value(Callable::Object(Rc::new(RandPdf {
        name,
        seeded: name == "SRANDPDF",
        probabilities,
        cumulative,
    }))));
    Ok(())
}

/// An unseeded draw fed by `entropy` (to exercise the failure of the system entropy).
#[cfg(test)]
pub(super) fn unseeded_draw(
    stack: &mut Stack,
    entropy: fn() -> core::result::Result<u64, getrandom::Error>,
) -> Result<()> {
    RandPdf {
        name: "RANDPDF",
        seeded: false,
        probabilities: vec![(Value::from("a"), 1.0)],
        cumulative: vec![1.0],
    }
    .draw(stack, entropy)
}

/// The value drawn for `rand` from `cumulative` probabilities (to exercise an exact match).
#[cfg(test)]
pub(super) fn pick(cumulative: &[f64], rand: f64) -> Result<Value> {
    RandPdf {
        name: "RANDPDF",
        seeded: false,
        probabilities: (0..cumulative.len())
            .map(|i| (Value::Long(i as i64), 0.0))
            .collect(),
        cumulative: cumulative.to_vec(),
    }
    .pick(rand)
}
