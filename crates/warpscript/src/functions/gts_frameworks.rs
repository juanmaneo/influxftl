// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Geo Time Series frameworks `BUCKETIZE` and `MAP` (`REDUCE` is in `gts_reduce`;
//! `APPLY` and `FILTER` to come), and the application of aggregators shared by them.

use std::rc::Rc;

use indexmap::IndexMap;

use super::Entry;
use super::args::fail;
use super::gts::{Datapoint, datapoint_from_list};
use crate::aggregators::{Aggregator, AggregatorArgs, Window, aggregator_of};
use crate::error::{Error, Result};
use crate::geoxp;
use crate::gts::{GeoTimeSerie, Gts, NO_ELEVATION, NO_LOCATION};
use crate::macros::Macro;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("BUCKETIZE", Entry::Stack(bucketize)),
    ("MAP", Entry::Stack(map)),
];

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
    // A copy, so that no borrow is held while macros (which may reach the series) run.
    let source = gts.borrow().clone();
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
                    window: Some(Window {
                        prewindow: 0,
                        postwindow: bucketspan.wrapping_neg(),
                        start: bucket_start,
                        stop: bucket_end,
                        tick_index: None,
                    }),
                };
                // `(Object[]) applyOnSubLists(…)`: a map result cannot be cast.
                let Mapped::Single(MappedDatapoint { datapoint, .. }) =
                    aggregate(stack, *aggregator, &args)?
                else {
                    return Err(Error::class_cast(
                        "java.util.LinkedHashMap",
                        "[Ljava.lang.Object;",
                    ));
                };
                if !datapoint.value.is_null() {
                    bucketized.set_value(
                        bucket_end,
                        datapoint.location,
                        datapoint.elevation,
                        &datapoint.value,
                        false,
                    );
                }
            }
        }
    }
    Ok(bucketized.shared())
}

/// The parameters of `GTSHelper.map`.
struct MapParams {
    prewindow: i64,
    postwindow: i64,
    /// The number of applications (`0`: every tick), as an absolute value.
    occurrences: i64,
    reversed: bool,
    step: i32,
    override_tick: bool,
    /// The ticks to compute a result for instead of the series' own.
    output_ticks: Option<Vec<i64>>,
}

/// A datapoint produced by a mapper, with whether its tick is the `Integer` zero of a
/// single-element list result (which the tick override cannot cast to a `long`).
pub(super) struct MappedDatapoint {
    pub(super) datapoint: Datapoint,
    pub(super) integer_tick: bool,
}

impl MappedDatapoint {
    /// `MACROMAPPER.listToObjects`.
    fn from_list(list: &[Value]) -> Result<Self> {
        Ok(MappedDatapoint {
            datapoint: datapoint_from_list(list)?,
            integer_tick: list.len() == 1,
        })
    }

    /// The tick the datapoint is set at: its own when the tick override is on, else `tick`.
    pub(super) fn tick(&self, override_tick: bool, tick: i64) -> Result<i64> {
        if !override_tick {
            return Ok(tick);
        }
        if self.integer_tick {
            return Err(Error::class_cast("java.lang.Integer", "java.lang.Long"));
        }
        Ok(self.datapoint.tick)
    }
}

/// The result of one application of a mapper: a datapoint for the mapped series, or one per
/// named output series.
pub(super) enum Mapped {
    Single(MappedDatapoint),
    Multiple(Vec<(String, MappedDatapoint)>),
}

/// `MACROMAPPER`: the result a macro mapper leaves on the stack: `[ tick lat lon elev value ]`
/// (and its shorter forms), a map of such lists keyed by output series name, or the five
/// values `tick lat lon elevation value`.
fn macro_result(stack: &mut Stack) -> Result<Mapped> {
    match stack.pop()? {
        Value::List(list) => Ok(Mapped::Single(MappedDatapoint::from_list(&list.borrow())?)),
        Value::Map(map) => {
            let mut results = Vec::with_capacity(map.borrow().len());
            for (key, value) in map.borrow().iter() {
                let Value::List(list) = value else {
                    return Err(Error::class_cast(value.java_class(), "java.util.List"));
                };
                results.push((key.to_string(), MappedDatapoint::from_list(&list.borrow())?));
            }
            Ok(Mapped::Multiple(results))
        }
        value => Ok(Mapped::Single(MappedDatapoint {
            datapoint: datapoint_from_stack(stack, value)?,
            integer_tick: false,
        })),
    }
}

/// The arguments of an aggregator as the list `MacroMapperWrapper.apply` pushes for its
/// macro: `[ tick [ names ] [ labels ] [ ticks ] [ lats ] [ lons ] [ elevations ] [ values ] ]`,
/// a missing location or elevation being `NaN`.
fn macro_arguments(args: &AggregatorArgs) -> Value {
    let (lats, lons): (Vec<Value>, Vec<Value>) = args
        .locations
        .iter()
        .map(|&location| {
            let (lat, lon) = if location == NO_LOCATION {
                (f64::NAN, f64::NAN)
            } else {
                geoxp::from_geoxp_point(location)
            };
            (Value::Double(lat), Value::Double(lon))
        })
        .unzip();
    let elevations = args
        .elevations
        .iter()
        .map(|&elevation| {
            if elevation == NO_ELEVATION {
                Value::Double(f64::NAN)
            } else {
                Value::Long(elevation)
            }
        })
        .collect();
    let labels = args
        .labels
        .iter()
        .map(|labels| {
            Value::map(
                labels
                    .iter()
                    .map(|(k, v)| (Value::from(k.as_str()), Value::from(v.as_str())))
                    .collect(),
            )
        })
        .collect();
    Value::list(vec![
        Value::Long(args.tick),
        Value::list(args.names.iter().map(|n| Value::from(n.as_str())).collect()),
        Value::list(labels),
        Value::list(args.ticks.iter().map(|&t| Value::Long(t)).collect()),
        Value::list(lats),
        Value::list(lons),
        Value::list(elevations),
        Value::list(args.values.to_vec()),
    ])
}

/// Apply `aggregator` to `args`. A `MACROMAPPER` wrapper runs its macro on the stack with the
/// arguments as lists and collects its output like a macro mapper (a list, a map of lists or
/// five values); the caller must hold no borrow of a series the macro could reach.
pub(super) fn aggregate(
    stack: &mut Stack,
    aggregator: &dyn Aggregator,
    args: &AggregatorArgs,
) -> Result<Mapped> {
    let Some(wrapped) = aggregator.wrapped_macro().cloned() else {
        let aggregated = aggregator.apply(args)?;
        return Ok(Mapped::Single(MappedDatapoint {
            datapoint: Datapoint {
                tick: aggregated.tick,
                location: aggregated.location,
                elevation: aggregated.elevation,
                value: aggregated.value,
            },
            integer_tick: false,
        }));
    };
    stack.push(macro_arguments(args));
    stack.exec_macro(&wrapped)?;
    macro_result(stack)
}

/// The sliding window of `GTSHelper.map` at one output tick: the `[start, stop]` interval.
struct Interval {
    start: i64,
    stop: i64,
}

/// The index bookkeeping of `GTSHelper.map` over the ticks of a non bucketized series.
struct Cursor {
    /// The input ticks, sorted in the mapping direction.
    ticks: Vec<i64>,
    /// With output ticks, forward: the index of the first input tick at or after the current
    /// output tick (`ticks.len()` when there is none).
    right: usize,
    /// With output ticks, reversed: the index of the first input tick at or before the current
    /// output tick (`ticks.len()` when there is none).
    left: usize,
}

impl Cursor {
    /// The tick at `index` when it is a valid index, `None` otherwise (`i64` arithmetic like
    /// Java's `idx + prewindow < ticks.length` tests).
    fn at(&self, index: i64) -> Option<i64> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.ticks.get(i))
            .copied()
    }
}

/// The `[start, stop]` interval of the window around output tick `tick` (index `idx`), for a
/// bucketized series (`buckets` = `(bucketspan, bucketcount)`) or from the cursor over the
/// input ticks.
fn interval(
    params: &MapParams,
    tick: i64,
    idx: i64,
    buckets: Option<(i64, i64)>,
    cursor: &Cursor,
) -> Interval {
    let (pre, post) = (params.prewindow, params.postwindow);
    let has_output_ticks = params.output_ticks.is_some();
    let n = cursor.ticks.len();
    let start = if pre < 0 {
        tick.wrapping_add(pre)
    } else if pre == 0 {
        tick
    } else if let Some((bucketspan, bucketcount)) = buckets {
        if pre <= bucketcount {
            tick.wrapping_sub(pre.wrapping_mul(bucketspan))
        } else {
            i64::MIN
        }
    } else if !has_output_ticks {
        let index = if params.reversed {
            idx + pre
        } else {
            idx - pre
        };
        cursor.at(index).unwrap_or(i64::MIN)
    } else if params.reversed {
        // One more tick in the window when the output tick is an input tick.
        let left = cursor.left as i64;
        let index = if cursor.left < n && cursor.ticks[cursor.left] == tick {
            left + pre
        } else {
            left - 1 + pre
        };
        cursor.at(index).unwrap_or(i64::MIN)
    } else {
        cursor.at(cursor.right as i64 - pre).unwrap_or(i64::MIN)
    };
    let stop = if post < 0 {
        tick.wrapping_sub(post)
    } else if post == 0 {
        tick
    } else if let Some((bucketspan, bucketcount)) = buckets {
        if post <= bucketcount {
            tick.wrapping_add(post.wrapping_mul(bucketspan))
        } else {
            i64::MAX
        }
    } else if !has_output_ticks {
        let index = if params.reversed {
            idx - post
        } else {
            idx + post
        };
        cursor.at(index).unwrap_or(i64::MAX)
    } else if params.reversed {
        cursor.at(cursor.left as i64 - post).unwrap_or(i64::MAX)
    } else {
        let right = cursor.right as i64;
        let index = if cursor.right < n && cursor.ticks[cursor.right] == tick {
            right + post
        } else {
            right - 1 + post
        };
        cursor.at(index).unwrap_or(i64::MAX)
    };
    Interval { start, stop }
}

/// The reusable per-series buffers of the aggregator arguments (`names` and `labels` repeat
/// the series' for every datapoint of the window).
struct ArgBuffers {
    names: Vec<String>,
    labels: Vec<IndexMap<String, String>>,
    locations: Vec<i64>,
    elevations: Vec<i64>,
}

impl ArgBuffers {
    fn grow(&mut self, n: usize, name: &str, labels: &IndexMap<String, String>) {
        while self.names.len() < n {
            self.names.push(name.to_owned());
            self.labels.push(labels.clone());
        }
    }
}

/// `GTSHelper.map`: apply `mapper` to the sliding window around each tick of `gts` (or of the
/// output ticks), producing the mapped series — several when the mapper names its outputs.
/// The series is sorted in place.
fn map_gts(stack: &mut Stack, gts: &Gts, mapper: &Value, params: &MapParams) -> Result<Vec<Gts>> {
    let mut mapped = gts.borrow().clone();
    let is_bucketized = mapped.is_bucketized();
    if mapped.size() == 0 && !is_bucketized && params.output_ticks.is_none() {
        return Ok(vec![mapped.shared()]);
    }
    mapped.sort(params.reversed);
    let mut cursor = Cursor {
        ticks: if is_bucketized {
            Vec::new()
        } else {
            mapped.ticks.clone()
        },
        right: 0,
        left: 0,
    };
    mapped.clear();
    let buckets = is_bucketized.then_some((mapped.bucketspan, mapped.bucketcount));
    let nticks = match &params.output_ticks {
        Some(ticks) => ticks.len() as i64,
        None if is_bucketized => mapped.bucketcount,
        None => cursor.ticks.len() as i64,
    };
    let name = mapped.name().unwrap_or_default().to_owned();
    let labels = mapped.metadata.labels.clone();
    let mut buffers = ArgBuffers {
        names: Vec::new(),
        labels: Vec::new(),
        locations: Vec::new(),
        elevations: Vec::new(),
    };
    let mut occurrences = params.occurrences;
    let has_occurrences = occurrences != 0;
    let mut multiple: Vec<(String, GeoTimeSerie)> = Vec::new();
    let mut single_result = true;
    let mut last_tick = 0i64;
    let mut duplicates = 0i64;
    let mut idx = 0i64;
    while idx < nticks {
        if has_occurrences && occurrences == 0 {
            break;
        }
        let tick = match &params.output_ticks {
            None if is_bucketized => {
                let offset = if params.reversed {
                    idx
                } else {
                    mapped.bucketcount - 1 - idx
                };
                mapped
                    .lastbucket
                    .wrapping_sub(offset.wrapping_mul(mapped.bucketspan))
            }
            None => cursor.ticks[idx as usize],
            Some(ticks) => {
                let tick = ticks[idx as usize];
                if !is_bucketized {
                    if params.reversed {
                        while cursor.left < cursor.ticks.len() && cursor.ticks[cursor.left] > tick {
                            cursor.left += 1;
                        }
                    } else {
                        while cursor.right < cursor.ticks.len() && cursor.ticks[cursor.right] < tick
                        {
                            cursor.right += 1;
                        }
                    }
                }
                tick
            }
        };
        // Repeated input ticks map one datapoint each.
        if idx != 0 && last_tick == tick {
            duplicates += 1;
        } else {
            duplicates = 0;
        }
        last_tick = tick;
        let Interval { start, stop } = interval(params, tick, idx, buckets, &cursor);
        let result = match mapper {
            Value::Macro(m) => {
                // The window as a series sharing the result's metadata (a copy here).
                let mut subgts = gts.borrow_mut().subserie(start, stop);
                subgts.metadata = mapped.metadata.clone();
                stack.push(Value::Gts(subgts.shared()));
                stack.exec_macro(m)?;
                macro_result(stack)?
            }
            _ => {
                let Some(aggregator) = aggregator_of(mapper).filter(|a| a.roles().mapper) else {
                    return Err(Error::new("Expected a mapper function."));
                };
                let mut source = gts.borrow_mut();
                source.sort(false);
                let (first, end) = source.window_range(start, stop);
                let n = end - first;
                buffers.grow(n, &name, &labels);
                buffers.locations.clear();
                buffers.elevations.clear();
                buffers
                    .locations
                    .extend((first..end).map(|j| source.location_at(j)));
                buffers
                    .elevations
                    .extend((first..end).map(|j| source.elevation_at(j)));
                let values: Vec<Value> = (first..end).map(|j| source.value_at(j)).collect();
                let window_ticks = source.ticks[first..end].to_vec();
                // A macro wrapper may reach the series.
                drop(source);
                // The first (last when reversed) index of the current tick in the window,
                // shifted along the repeated input ticks.
                let mut tickidx = -1i64;
                for (j, window_tick) in window_ticks.iter().enumerate() {
                    if (tickidx == -1 || params.reversed) && *window_tick == tick {
                        tickidx = j as i64;
                    }
                }
                if params.reversed {
                    tickidx -= duplicates;
                } else {
                    tickidx += duplicates;
                }
                let args = AggregatorArgs {
                    tick,
                    names: &buffers.names[..n],
                    labels: &buffers.labels[..n],
                    ticks: &window_ticks,
                    locations: &buffers.locations,
                    elevations: &buffers.elevations,
                    values: &values,
                    window: Some(Window {
                        prewindow: params.prewindow,
                        postwindow: params.postwindow,
                        start,
                        stop,
                        tick_index: Some(tickidx),
                    }),
                };
                aggregate(stack, aggregator, &args)?
            }
        };
        match result {
            Mapped::Single(mapped_datapoint) => {
                let datapoint = &mapped_datapoint.datapoint;
                if !datapoint.value.is_null() {
                    let at = mapped_datapoint.tick(params.override_tick, tick)?;
                    mapped.set_value(
                        at,
                        datapoint.location,
                        datapoint.elevation,
                        &datapoint.value,
                        false,
                    );
                }
            }
            Mapped::Multiple(datapoints) => {
                single_result = false;
                for (key, mapped_datapoint) in datapoints {
                    let position = match multiple.iter().position(|(k, _)| *k == key) {
                        Some(position) => position,
                        None => {
                            let mut mgts = mapped.clone_empty();
                            if params.output_ticks.is_some() {
                                mgts.bucketcount = 0;
                                mgts.bucketspan = 0;
                                mgts.lastbucket = 0;
                            }
                            mgts.metadata.name = Some(key.clone());
                            multiple.push((key, mgts));
                            multiple.len() - 1
                        }
                    };
                    let datapoint = &mapped_datapoint.datapoint;
                    if !datapoint.value.is_null() {
                        let at = mapped_datapoint.tick(params.override_tick, tick)?;
                        multiple[position].1.set_value(
                            at,
                            datapoint.location,
                            datapoint.elevation,
                            &datapoint.value,
                            false,
                        );
                    }
                }
            }
        }
        idx += i64::from(params.step);
        occurrences = occurrences.wrapping_sub(1);
    }
    let mut results = Vec::new();
    if single_result {
        if params.output_ticks.is_some() {
            mapped.bucketcount = 0;
            mapped.bucketspan = 0;
            mapped.lastbucket = 0;
        }
        results.push(mapped.shared());
    }
    // Named outputs in `String.compareTo` order (a `TreeMap` in Warp 10).
    multiple.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
    results.extend(multiple.into_iter().map(|(_, mgts)| mgts.shared()));
    Ok(results)
}

/// The value of key `name` in the parameter map of `MAP`, with whether the key is present.
fn param<'a>(params: &'a IndexMap<Value, Value>, name: &str) -> (bool, Option<&'a Value>) {
    let value = params.get(&Value::from(name));
    (value.is_some(), value.filter(|v| !v.is_null()))
}

/// `MAP.applyWithParamsFromMap`: check the parameters, pop the series and map each of them.
fn map_with_params(stack: &mut Stack, params: &IndexMap<Value, Value>) -> Result<()> {
    const NAME: &str = "MAP";
    const OCCURRENCES_MESSAGE: &str =
        "expects the occurrences parameter to be a LONG or a sorted LIST of LONG.";
    if params.contains_key(&Value::from("occurences")) {
        return Err(fail(
            NAME,
            "was given a deprecated parameter with a typo: occurences. Use instead parameter occurrences",
        ));
    }
    if params.contains_key(&Value::from("ticks")) {
        return Err(fail(
            NAME,
            "was given a deprecated parameter: ticks. Consider using occurrences instead (behaviour is slightly different - see documentation)",
        ));
    }
    let (has_mapper, mapper) = param(params, "mapper");
    if !has_mapper {
        return Err(fail(NAME, "Missing 'mapper' parameter."));
    }
    let mapper = mapper.cloned().unwrap_or(Value::Null);
    let long_param = |name: &str| -> Result<i64> {
        match param(params, name) {
            (_, Some(Value::Long(l))) => Ok(*l),
            (false, _) => Ok(0),
            (true, _) => Err(fail(
                NAME,
                &format!("expects the {name} parameter to be a LONG."),
            )),
        }
    };
    let prewindow = long_param("pre")?;
    let postwindow = long_param("post")?;
    let mut occurrences = 0i64;
    let mut reversed = false;
    let mut output_ticks = None;
    match param(params, "occurrences") {
        (_, Some(Value::Long(l))) => {
            // So that the absolute value is positive.
            occurrences = if *l == i64::MIN { i64::MIN + 1 } else { *l };
            reversed = occurrences < 0;
        }
        (_, Some(Value::List(list))) => {
            let mut ticks = Vec::with_capacity(list.borrow().len());
            for tick in list.borrow().iter() {
                let Value::Long(tick) = tick else {
                    return Err(fail(NAME, OCCURRENCES_MESSAGE));
                };
                ticks.push(*tick);
            }
            // Increasing, or else decreasing.
            reversed = ticks.windows(2).any(|pair| pair[1] < pair[0]);
            if reversed && ticks.windows(2).any(|pair| pair[1] > pair[0]) {
                return Err(fail(NAME, OCCURRENCES_MESSAGE));
            }
            output_ticks = Some(ticks);
        }
        (false, _) => {}
        (true, _) => return Err(fail(NAME, OCCURRENCES_MESSAGE)),
    }
    let step = match param(params, "step") {
        // Capped to `Integer.MAX_VALUE`, which runs the mapper once.
        (_, Some(Value::Long(l))) => (*l).min(i64::from(i32::MAX)) as i32,
        (false, _) => 1,
        (true, _) => {
            return Err(fail(NAME, "expects the step parameter to be a LONG."));
        }
    };
    let override_tick = match param(params, "override") {
        (_, Some(Value::Boolean(b))) => *b,
        (false, _) => false,
        (true, _) => {
            return Err(fail(
                NAME,
                "expects the override parameter to be a BOOLEAN.",
            ));
        }
    };
    // Windows and occurrences are capped to `Integer.MAX_VALUE`, the most indices a series
    // may have.
    let cap = i64::from(i32::MAX);
    let map_params = MapParams {
        prewindow: prewindow.min(cap),
        postwindow: postwindow.min(cap),
        occurrences: occurrences.abs().min(cap),
        reversed,
        // `GTSHelper.map` makes sure the step is positive.
        step: step.max(1),
        override_tick,
        output_ticks,
    };
    // A series, a list of series or a list of lists of series.
    let top = stack.pop()?;
    let mut series: Vec<Gts> = Vec::new();
    let not_series = || fail(NAME, "operates on lists of Geo Time Series.");
    match &top {
        Value::List(list) => {
            for element in list.borrow().iter() {
                match element {
                    Value::List(inner) => {
                        for element in inner.borrow().iter() {
                            let Value::Gts(gts) = element else {
                                return Err(not_series());
                            };
                            series.push(Rc::clone(gts));
                        }
                    }
                    Value::Gts(gts) => series.push(Rc::clone(gts)),
                    _ => return Err(not_series()),
                }
            }
        }
        Value::Gts(gts) => series.push(Rc::clone(gts)),
        _ => return Err(not_series()),
    }
    let mut mapped = Vec::with_capacity(series.len());
    for gts in &series {
        let results = match map_gts(stack, gts, &mapper, &map_params) {
            Ok(results) => results,
            // Control signals (`STOP` in a macro mapper…) and Java runtime exceptions pass
            // through unwrapped.
            Err(err) if err.is_signal() || err.is_runtime() => return Err(err),
            Err(err) => {
                return Err(super::args::fail_with_cause(
                    NAME,
                    "was given invalid parameters.",
                    err,
                ));
            }
        };
        if results.len() < 2 {
            mapped.extend(results.into_iter().map(Value::Gts));
        } else {
            mapped.push(Value::list(results.into_iter().map(Value::Gts).collect()));
        }
    }
    stack.push(Value::list(mapped));
    Ok(())
}

/// `MAP`: `[ series… mapper prewindow postwindow occurrences [ step [ override ] ] ] MAP` or
/// `series { 'mapper' m 'pre' p 'post' p 'occurrences' o 'step' s 'override' b } MAP`.
fn map(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MAP";
    let top = stack.pop()?;
    let params = match &top {
        Value::Map(params) => {
            let params = params.borrow().clone();
            return map_with_params(stack, &params);
        }
        Value::List(params) => params,
        _ => {
            return Err(fail(
                NAME,
                "expects a list as input or a map of parameters on top of a list of GTS.",
            ));
        }
    };
    let nseries = params
        .borrow()
        .iter()
        .take_while(|p| matches!(p, Value::Gts(_) | Value::List(_)))
        .count();
    if nseries == 0 {
        return Err(fail(
            NAME,
            "expects Geo Time Series or lists thereof as first parameters.",
        ));
    }
    let is_mapper = |value: &Value| {
        matches!(value, Value::Macro(_)) || aggregator_of(value).is_some_and(|a| a.roles().mapper)
    };
    if params.borrow().get(nseries).is_none_or(|m| !is_mapper(m)) {
        return Err(fail(
            NAME,
            "expects a mapper function or a macro after Geo Time Series.",
        ));
    }
    // The missing windows and occurrences default to 0, in the list itself like Warp 10.
    while params.borrow().len() <= nseries + 3 {
        params.borrow_mut().push(Value::Long(0));
    }
    let params = params.borrow();
    if !matches!(params[nseries + 1], Value::Long(_))
        || !matches!(params[nseries + 2], Value::Long(_))
    {
        return Err(fail(
            NAME,
            "expects prewindow and postwindow as 2 parameters following the mapper function.",
        ));
    }
    if !matches!(params[nseries + 3], Value::Long(_) | Value::List(_)) {
        return Err(fail(
            NAME,
            "expects occurrences (LONG or LIST of LONG) as the parameter following the postwindow parameter.",
        ));
    }
    let mut step = 1i64;
    if let Some(value) = params.get(nseries + 4) {
        let Value::Long(l) = value else {
            return Err(fail(
                NAME,
                "expects a step parameter that is an integer number.",
            ));
        };
        // `intValue()`
        step = i64::from(*l as i32);
        if step <= 0 {
            return Err(fail(
                NAME,
                "expects a step parameter which is strictly positive.",
            ));
        }
    }
    let mut override_tick = false;
    if let Some(value) = params.get(nseries + 5) {
        let Value::Boolean(b) = value else {
            return Err(fail(
                NAME,
                "expects a boolean as 'override tick' parameter.",
            ));
        };
        override_tick = *b;
    }
    stack.push(Value::list(params[..nseries].to_vec()));
    let mut map_params = IndexMap::new();
    map_params.insert(Value::from("mapper"), params[nseries].clone());
    map_params.insert(Value::from("pre"), params[nseries + 1].clone());
    map_params.insert(Value::from("post"), params[nseries + 2].clone());
    map_params.insert(Value::from("occurrences"), params[nseries + 3].clone());
    map_params.insert(Value::from("step"), Value::Long(step));
    map_params.insert(Value::from("override"), Value::Boolean(override_tick));
    drop(params);
    map_with_params(stack, &map_params)
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
