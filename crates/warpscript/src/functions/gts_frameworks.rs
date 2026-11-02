// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Geo Time Series frameworks: `BUCKETIZE` (`MAP`, `REDUCE`, `APPLY` and `FILTER` to
//! come).

use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::gts::{Datapoint, datapoint_from_list};
use crate::aggregators::{Aggregator, AggregatorArgs, aggregator_of};
use crate::error::{Error, Result};
use crate::geoxp;
use crate::gts::{GeoTimeSerie, Gts, NO_ELEVATION, NO_LOCATION};
use crate::macros::Macro;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[("BUCKETIZE", Entry::Stack(bucketize))];

/// `MACROMAPPER.stackToObjects`: the `tick lat lon elevation value` a macro leaves on the
/// stack, as a datapoint.
fn datapoint_from_stack(stack: &mut Stack, value: Value) -> Result<Datapoint> {
    let elevation = stack.pop()?;
    let longitude = stack.pop()?;
    let latitude = stack.pop()?;
    let rtick = stack.pop()?;
    let (Value::Double(longitude), Value::Double(latitude)) = (&longitude, &latitude) else {
        return Err(Error::new(
            "Macro MUST return a latitude and a longitude which are of type DOUBLE.",
        ));
    };
    let location = if latitude.is_nan() || longitude.is_nan() {
        NO_LOCATION
    } else {
        geoxp::to_geoxp_point(*latitude, *longitude)
    };
    let elevation = match elevation {
        Value::Long(l) => l,
        Value::Double(d) if d.is_nan() => NO_ELEVATION,
        Value::Double(_) => {
            return Err(Error::new(
                "Macro MUST return an elevations which is NaN if it is of type DOUBLE.",
            ));
        }
        _ => {
            return Err(Error::new(
                "Macro MUST return an elevation which is either NaN or LONG.",
            ));
        }
    };
    let Value::Long(tick) = rtick else {
        return Err(Error::new("Macro MUST return a tick of type LONG."));
    };
    Ok(Datapoint {
        tick,
        location,
        elevation,
        value,
    })
}

/// The bucketizer of `BUCKETIZE`: an aggregator or a macro; `None` only sets the
/// bucketization parameters of the series.
enum Bucketizer<'a> {
    Aggregator(&'a dyn Aggregator),
    Macro(Rc<Macro>),
}

/// A series parameter of `BUCKETIZE`: a series or a list of series.
enum Input<'a> {
    Series(&'a Gts),
    List(&'a crate::value::List),
}

/// The bucketization parameters as `BUCKETIZE` receives them.
struct Buckets {
    lastbucket: i64,
    bucketspan: i64,
    bucketcount: i32,
}

/// `GTSHelper.bucketize`: resolve the bucketization parameters (`0` meaning "derive from the
/// series"), then aggregate the datapoints of each bucket (the series is sorted in place).
fn bucketize_gts(
    stack: &mut Stack,
    gts: &Gts,
    params: &Buckets,
    bucketizer: Option<&Bucketizer>,
    maxbuckets: i64,
) -> Result<Gts> {
    let (lasttick, firsttick) = {
        let gts = gts.borrow();
        (gts.last_tick(), gts.first_tick())
    };
    let Buckets {
        mut lastbucket,
        mut bucketspan,
        mut bucketcount,
    } = *params;
    let zero_lastbucket = lastbucket == 0;
    let zero_bucketcount = bucketcount == 0;
    if lastbucket == 0 {
        lastbucket = lasttick;
    }
    if bucketspan == 0 || bucketspan == -1 {
        if bucketcount == 0 {
            return Err(Error::new(
                "One of bucketspan or bucketcount must be different from zero.",
            ));
        }
        if lastbucket >= firsttick {
            let delta;
            if bucketspan == 0 {
                delta = lastbucket.wrapping_sub(firsttick).wrapping_add(1);
                bucketspan = delta.wrapping_div(i64::from(bucketcount));
            } else {
                delta = lastbucket.wrapping_sub(firsttick);
                bucketspan = if bucketcount == 1 {
                    delta
                } else {
                    delta.wrapping_div(i64::from(bucketcount) - 1)
                };
            }
            if bucketspan == 0 || delta.wrapping_rem(bucketspan) != 0 {
                bucketspan += 1;
            }
        }
    }
    if bucketspan < 0 {
        bucketspan = 0;
    }
    if bucketcount == 0 && lastbucket >= firsttick {
        let delta = lastbucket.wrapping_sub(firsttick);
        if delta < bucketspan {
            bucketcount = 1;
        } else {
            if bucketspan == 0 {
                return Err(Error::new("/ by zero"));
            }
            bucketcount = 1i32.wrapping_add((delta / bucketspan) as i32);
        }
    }
    if zero_lastbucket && zero_bucketcount && lastbucket.wrapping_rem(bucketspan) != 0 {
        lastbucket = lastbucket
            .wrapping_sub(lastbucket.wrapping_rem(bucketspan))
            .wrapping_add(bucketspan);
        if lastbucket.wrapping_sub(i64::from(bucketcount).wrapping_mul(bucketspan)) >= firsttick {
            bucketcount = bucketcount.wrapping_add(1);
        }
    }
    if bucketcount < 0 || i64::from(bucketcount) > maxbuckets {
        return Err(Error::new(format!(
            "Bucket count ({bucketcount}) would exceed maximum value of {maxbuckets}. Consider raising the limit or using capabilities."
        )));
    }
    if bucketspan == 0 {
        return Err(Error::new(
            "Undefined bucket span, check your GTS timestamps.",
        ));
    }
    let bucketcount = i64::from(bucketcount);
    let Some(bucketizer) = bucketizer else {
        let mut gts_mut = gts.borrow_mut();
        gts_mut.lastbucket = lastbucket;
        gts_mut.bucketcount = bucketcount;
        gts_mut.bucketspan = bucketspan;
        return Ok(gts.clone());
    };
    let mut bucketized = GeoTimeSerie::with_buckets(lastbucket, bucketcount, bucketspan);
    bucketized.metadata = gts.borrow().metadata.clone();
    let n = gts.borrow().size();
    let first_bucket_start = lastbucket
        .wrapping_sub(bucketspan.wrapping_mul(bucketcount))
        .wrapping_add(1);
    if n == 0 || firsttick > lastbucket || lasttick < first_bucket_start {
        return Ok(bucketized.shared());
    }
    gts.borrow_mut().sort(false);
    let source = gts.borrow();
    // The index of the most recent datapoint within the buckets.
    let mut i: i64 = if lastbucket >= lasttick {
        n as i64 - 1
    } else {
        // The series starts before `lastbucket`, so the search cannot land before it.
        let found = crate::gts::java_binary_search_range(&source.ticks, 0, n as i64, lastbucket);
        if found < 0 {
            -found - 2
        } else {
            let mut last = found;
            while (last + 1) < n as i64 && source.ticks[(last + 1) as usize] == lastbucket {
                last += 1;
            }
            last
        }
    };
    let names = [bucketized.name().unwrap_or_default().to_owned()];
    let labels = [bucketized.metadata.labels.clone()];
    let oldest = lastbucket.wrapping_sub(bucketspan.wrapping_mul(bucketcount));
    while i >= 0 && source.ticks[i as usize] > oldest {
        let end_position = i as usize;
        let tick = source.ticks[end_position];
        let bucket_end = tick.wrapping_add(lastbucket.wrapping_sub(tick).wrapping_rem(bucketspan));
        let bucket_start = bucket_end.wrapping_sub(bucketspan);
        while i >= 0 && source.ticks[i as usize] > bucket_start {
            i -= 1;
        }
        let start_position = (i + 1) as usize;
        let range = start_position..=end_position;
        match bucketizer {
            Bucketizer::Macro(m) => {
                // The datapoints of the bucket as a series sharing the result's metadata.
                let mut subgts = GeoTimeSerie::new();
                subgts.metadata = bucketized.metadata.clone();
                for j in range {
                    subgts.copy_datapoint(&source, j);
                }
                stack.push(Value::Gts(subgts.shared()));
                stack.exec_macro(m)?;
                // A `null` result, alone or last in a list, is left on the stack like Warp 10.
                let result = stack.pop()?;
                let datapoint = match &result {
                    Value::List(list) => {
                        if list.borrow().last().is_some_and(|last| !last.is_null()) {
                            Some(datapoint_from_list(&list.borrow())?)
                        } else {
                            stack.push(result.clone());
                            None
                        }
                    }
                    Value::Null => {
                        stack.push(Value::Null);
                        None
                    }
                    _ => Some(datapoint_from_stack(stack, result.clone())?),
                };
                if let Some(datapoint) = datapoint {
                    bucketized.set_value(
                        bucket_end,
                        datapoint.location,
                        datapoint.elevation,
                        &datapoint.value,
                        false,
                    );
                }
            }
            Bucketizer::Aggregator(aggregator) => {
                let locations: Vec<i64> = range.clone().map(|j| source.location_at(j)).collect();
                let elevations: Vec<i64> = range.clone().map(|j| source.elevation_at(j)).collect();
                let values: Vec<Value> = range.clone().map(|j| source.value_at(j)).collect();
                let args = AggregatorArgs {
                    tick: bucket_end,
                    names: &names,
                    labels: &labels,
                    ticks: &source.ticks[start_position..=end_position],
                    locations: &locations,
                    elevations: &elevations,
                    values: &values,
                    window: Some([0, bucketspan.wrapping_neg(), bucket_start, bucket_end]),
                };
                let aggregated = aggregator.apply(&args)?;
                if !aggregated.value.is_null() {
                    bucketized.set_value(
                        bucket_end,
                        aggregated.location,
                        aggregated.elevation,
                        &aggregated.value,
                        false,
                    );
                }
            }
        }
    }
    Ok(bucketized.shared())
}

/// `BUCKETIZE`: `[ series… bucketizer lastbucket bucketspan bucketcount ] BUCKETIZE`.
fn bucketize(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BUCKETIZE";
    let Value::List(params) = stack.pop()? else {
        return Err(fail(NAME, "expects a list as input."));
    };
    let params = params.borrow().clone();
    if params.len() < 5 {
        return Err(fail(
            NAME,
            "needs a list of at least 5 parameters as input.",
        ));
    }
    let n = params.len();
    let mut inputs = Vec::with_capacity(n - 4);
    for param in &params[..n - 4] {
        match param {
            Value::Gts(gts) => inputs.push(Input::Series(gts)),
            Value::List(list) => inputs.push(Input::List(list)),
            _ => {
                return Err(fail(
                    NAME,
                    "expects a list of Geo Time Series as first parameter.",
                ));
            }
        }
    }
    let bucketizer = match (&params[n - 4], aggregator_of(&params[n - 4])) {
        (Value::Null, _) => None,
        (Value::Macro(m), _) => Some(Bucketizer::Macro(Rc::clone(m))),
        (_, Some(aggregator)) if aggregator.roles().bucketizer => {
            Some(Bucketizer::Aggregator(aggregator))
        }
        _ => {
            return Err(fail(
                NAME,
                "expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
            ));
        }
    };
    let (Value::Long(lastbucket), Value::Long(bucketspan), Value::Long(bucketcount)) =
        (&params[n - 3], &params[n - 2], &params[n - 1])
    else {
        return Err(fail(
            NAME,
            "expects lastbucket, bucketspan and bucketcount as last 3 parameters.",
        ));
    };
    let buckets = Buckets {
        lastbucket: *lastbucket,
        bucketspan: *bucketspan,
        bucketcount: *bucketcount as i32,
    };
    let mut series: Vec<Gts> = Vec::new();
    for input in inputs {
        match input {
            Input::Series(gts) => series.push(gts.clone()),
            Input::List(list) => {
                for element in list.borrow().iter() {
                    let Value::Gts(gts) = element else {
                        return Err(fail(
                            NAME,
                            "expects a list of Geo Time Series as first parameter.",
                        ));
                    };
                    series.push(gts.clone());
                }
            }
        }
    }
    let maxbuckets = stack.limits.max_buckets;
    let mut bucketized = Vec::with_capacity(series.len());
    for gts in &series {
        bucketized.push(Value::Gts(bucketize_gts(
            stack,
            gts,
            &buckets,
            bucketizer.as_ref(),
            maxbuckets,
        )?));
    }
    stack.push(Value::list(bucketized));
    Ok(())
}
