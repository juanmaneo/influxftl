// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Calendar bucketization: `BUCKETIZE.CALENDAR` (buckets of an ISO 8601 duration in a time
//! zone, numbered by their index since the epoch), `UNBUCKETIZE.CALENDAR` and `UNBUCKETIZE`.

use std::rc::Rc;

use super::Entry;
use super::args::{fail, fail_with_cause};
use super::gts::{datapoint_from_list, gts_or_list, gts_or_list_try};
use super::gts_frameworks::{Mapped, MappedDatapoint, aggregate, datapoint_from_stack};
use super::time::{Period, Zone, add_period, duration_to_period};
use crate::aggregators::{Aggregator, AggregatorArgs, Window, aggregator_of};
use crate::error::fault::{ArithmeticError, BoundsError, NumberFormatError};
use crate::error::{Class, Error, Result};
use crate::gts::{GeoTimeSerie, Gts};
use crate::macros::Macro;
use crate::stack::Stack;
use crate::value::{Value, ValueKind};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("BUCKETIZE.CALENDAR", Entry::Stack(bucketize_calendar)),
    ("UNBUCKETIZE.CALENDAR", Entry::Stack(unbucketize_calendar)),
    ("UNBUCKETIZE", Entry::Stack(unbucketize)),
];

/// `BUCKETIZECALENDAR.DURATION_ATTRIBUTE_KEY`.
const DURATION_ATTRIBUTE_KEY: &str = ".bucketduration";
/// `BUCKETIZECALENDAR.OFFSET_ATTRIBUTE_KEY`.
const OFFSET_ATTRIBUTE_KEY: &str = ".bucketoffset";
/// `BUCKETIZECALENDAR.TIMEZONE_ATTRIBUTE_KEY`.
const TIMEZONE_ATTRIBUTE_KEY: &str = ".buckettimezone";

/// A period and its sub-second offset (`ADDDURATION.ReadWritablePeriodWithSubSecondOffset`).
struct CalendarPeriod {
    period: Period,
    offset: i64,
}

/// Joda `Period.toString()` (`ISOPeriodFormat.standard()`): `PnYnMnWnDTnHnMnS`, the zero
/// fields left out (`PT0S` for the zero period), seconds and milliseconds printed together.
fn period_to_string(period: &Period) -> String {
    let mut out = String::from("P");
    for (value, unit) in [
        (period.years, 'Y'),
        (period.months, 'M'),
        (period.weeks, 'W'),
        (period.days, 'D'),
    ] {
        if value != 0 {
            out.push_str(&format!("{value}{unit}"));
        }
    }
    let seconds = period
        .seconds
        .wrapping_mul(1000)
        .wrapping_add(period.millis);
    if period.hours != 0 || period.minutes != 0 || seconds != 0 {
        out.push('T');
        for (value, unit) in [(period.hours, 'H'), (period.minutes, 'M')] {
            if value != 0 {
                out.push_str(&format!("{value}{unit}"));
            }
        }
        if seconds % 1000 != 0 {
            let sign = if seconds < 0 { "-" } else { "" };
            out.push_str(&format!(
                "{sign}{}.{:03}S",
                (seconds / 1000).abs(),
                (seconds % 1000).abs()
            ));
        } else if seconds != 0 {
            out.push_str(&format!("{}S", seconds / 1000));
        }
    }
    if out.len() == 1 {
        out.push_str("T0S");
    }
    out
}

/// `BUCKETIZECALENDAR.addNonNegativePeriod`: `origin` plus `n` times the period, which must
/// move away from `origin` in the direction of the sign of `n`.
fn add_non_negative_period(
    origin: i64,
    period: &CalendarPeriod,
    zone: Zone,
    n: i64,
) -> Result<i64> {
    let result = add_period(origin, &period.period, period.offset, zone, n)?;
    if n == 0 {
        return Ok(origin);
    }
    if (result > origin) ^ (n > 0) {
        return Err(Error::new(format!(
            "Period is negative from timestamp {origin}. Can not add a negative period. Period is {n} times {} plus {} time unit(s).",
            period_to_string(&period.period),
            period.offset
        )));
    }
    Ok(result)
}

/// `BUCKETIZECALENDAR.isDurationBucketized`: the three calendar attributes are set.
fn is_duration_bucketized(gts: &GeoTimeSerie) -> bool {
    let attributes = &gts.metadata.attributes;
    attributes.contains_key(DURATION_ATTRIBUTE_KEY)
        && attributes.contains_key(OFFSET_ATTRIBUTE_KEY)
        && attributes.contains_key(TIMEZONE_ATTRIBUTE_KEY)
}

/// `GTSHelper.unbucketize` and the removal of the calendar attributes.
fn clear_calendar(gts: &mut GeoTimeSerie) {
    gts.bucketcount = 0;
    gts.bucketspan = 0;
    gts.lastbucket = 0;
    for key in [
        DURATION_ATTRIBUTE_KEY,
        OFFSET_ATTRIBUTE_KEY,
        TIMEZONE_ATTRIBUTE_KEY,
    ] {
        gts.metadata.attributes.shift_remove(key);
    }
}

/// The bucketizer of `BUCKETIZE.CALENDAR`.
enum Bucketizer<'a> {
    Aggregator(&'a dyn Aggregator),
    Macro(Rc<Macro>),
}

/// The parameters of `BUCKETIZECALENDAR.durationBucketize`.
struct Calendar<'a> {
    period: CalendarPeriod,
    zone: Zone,
    bucketcount: i64,
    lastbucket: i64,
    lastbucket_index: i64,
    bucketizer: &'a Bucketizer<'a>,
    maxbuckets: i64,
}

/// `BUCKETIZECALENDAR.aggregateAndSet`: aggregate the datapoints of `subgts` (most recent
/// first) and set the result at `bucket_index` when its value is not `null`.
fn aggregate_and_set(
    stack: &mut Stack,
    bucketizer: &Bucketizer,
    subgts: &Gts,
    bucketized: &mut GeoTimeSerie,
    bucket_index: i64,
) -> Result<()> {
    let datapoint = match bucketizer {
        Bucketizer::Macro(m) => {
            stack.push(Value::Gts(Rc::clone(subgts)));
            stack.exec_macro(m)?;
            match stack.pop()? {
                Value::List(list) => datapoint_from_list(&list.borrow())?,
                value => datapoint_from_stack(stack, value)?,
            }
        }
        Bucketizer::Aggregator(aggregator) => {
            let sub = subgts.borrow().clone();
            let n = sub.size();
            let names = [sub.name().unwrap_or_default().to_owned()];
            let labels = [sub.metadata.labels.clone()];
            let locations: Vec<i64> = (0..n).map(|j| sub.location_at(j)).collect();
            let elevations: Vec<i64> = (0..n).map(|j| sub.elevation_at(j)).collect();
            let values: Vec<Value> = (0..n).map(|j| sub.value_at(j)).collect();
            let args = AggregatorArgs {
                tick: bucket_index,
                names: &names,
                labels: &labels,
                ticks: &sub.ticks,
                locations: &locations,
                elevations: &elevations,
                values: &values,
                window: Some(Window {
                    prewindow: 0,
                    postwindow: -1,
                    start: bucket_index,
                    stop: bucket_index,
                    tick_index: None,
                }),
            };
            // `(Object[]) aggregator.apply(…)`: a map result cannot be cast.
            let Mapped::Single(MappedDatapoint { datapoint, .. }) =
                aggregate(stack, *aggregator, &args)?
            else {
                return Err(Error::class_cast(ValueKind::Map, Class::ObjectArray));
            };
            datapoint
        }
    };
    if !datapoint.value.is_null() {
        bucketized.set_value(
            bucket_index,
            datapoint.location,
            datapoint.elevation,
            &datapoint.value,
            false,
        );
    }
    Ok(())
}

/// `BUCKETIZECALENDAR.durationBucketize`: walk the datapoints from the most recent one,
/// aggregating those of each calendar bucket; the result is indexed by bucket number (span
/// `1`, last bucket `lastbucket_index`). The input series is sorted in place.
fn duration_bucketize(stack: &mut Stack, gts: &Gts, params: &Calendar) -> Result<Gts> {
    // Warp 10 first computes a size hint with the span of one period from the epoch, which
    // cannot fail once `locate_lastbucket` succeeded.
    gts.borrow_mut().sort(false);
    // A copy, so that no borrow is held while macros (which may reach the series) run.
    let source = gts.borrow().clone();
    let mut bucketized = source.clone_empty();
    // One buffer series, emptied after each bucket like Warp 10's `shrinkTo(subgts, 0)`.
    let subgts = source.clone_empty().shared();
    let lastbucket_index = params.lastbucket_index;
    let mut bucket_start =
        add_non_negative_period(params.lastbucket, &params.period, params.zone, -1)?
            .wrapping_add(1);
    let mut bucket_index = lastbucket_index;
    for i in (0..source.size()).rev() {
        let tick = source.ticks[i];
        if tick < bucket_start && subgts.borrow().size() > 0 {
            aggregate_and_set(
                stack,
                params.bucketizer,
                &subgts,
                &mut bucketized,
                bucket_index,
            )?;
            let mut sub = subgts.borrow_mut();
            let n = sub.size();
            sub.keep_range(n, n);
        }
        while tick < bucket_start {
            bucket_index -= 1;
            bucket_start = add_non_negative_period(
                params.lastbucket,
                &params.period,
                params.zone,
                bucket_index - lastbucket_index - 1,
            )?
            .wrapping_add(1);
        }
        let count = lastbucket_index - bucket_index + 1;
        if params.bucketcount != 0 && count >= params.bucketcount {
            break;
        }
        if count > params.maxbuckets {
            return Err(Error::new(format!(
                "Bucket count ({count}) is exceeding maximum value of {}. Consider raising the limit or using capabilities.",
                params.maxbuckets
            )));
        }
        if tick <= params.lastbucket {
            subgts.borrow_mut().copy_datapoint(&source, i);
        }
    }
    if subgts.borrow().size() > 0 {
        aggregate_and_set(
            stack,
            params.bucketizer,
            &subgts,
            &mut bucketized,
            bucket_index,
        )?;
    }
    let bucketcount = if params.bucketcount == 0 {
        lastbucket_index - bucket_index + 1
    } else {
        params.bucketcount
    };
    // `Math.toIntExact`.
    let bucketcount = i32::try_from(bucketcount).map_err(|_| ArithmeticError::IntegerOverflow)?;
    bucketized.lastbucket = lastbucket_index;
    bucketized.bucketspan = 1;
    bucketized.bucketcount = i64::from(bucketcount);
    bucketized.sort(false);
    Ok(bucketized.shared())
}

/// The index since the epoch of the bucket ending at `lastbucket`, and the offset of the
/// buckets (the time between the end of that bucket and the next period boundary from the
/// epoch), found like `BUCKETIZE.CALENDAR` does: a hint from the average (positive) span of
/// a period, then steps.
fn locate_lastbucket(
    lastbucket: i64,
    period: &CalendarPeriod,
    average_span: i64,
) -> Result<(i64, i64)> {
    // The successive loops of Warp 10, as phases: land close to `lastbucket` with the hint,
    // jump back then forth by the estimated number of periods left, then step back and forth
    // one period at a time. `flag` is the end (exclusive) of the bucket at `index`.
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        Hint,
        JumpBack,
        JumpForth,
        StepBack,
        StepForth,
    }
    let hint = lastbucket / average_span;
    let mut phase = Phase::Hint;
    let mut flag = 0i64;
    let mut index = 0i64;
    loop {
        let after = flag > lastbucket;
        // The number of periods from the epoch to add, and the index of the bucket it ends.
        let (periods, next_index) = match phase {
            Phase::Hint if lastbucket > 0 => (hint.wrapping_add(1), hint),
            Phase::Hint => (hint, hint.wrapping_sub(1)),
            Phase::JumpBack | Phase::JumpForth => {
                let n = if phase == Phase::JumpBack {
                    (flag.wrapping_sub(lastbucket) / average_span)
                        .wrapping_sub(1)
                        .wrapping_neg()
                } else {
                    (lastbucket.wrapping_sub(flag) / average_span).wrapping_sub(1)
                };
                let jump = if phase == Phase::JumpBack {
                    after && n < -1
                } else {
                    !after && n > 1
                };
                if !jump {
                    phase = if phase == Phase::JumpBack {
                        Phase::JumpForth
                    } else {
                        Phase::StepBack
                    };
                    continue;
                }
                (index.wrapping_add(n).wrapping_add(1), index.wrapping_add(n))
            }
            Phase::StepBack if after => (index, index.wrapping_sub(1)),
            Phase::StepBack => {
                phase = Phase::StepForth;
                continue;
            }
            Phase::StepForth if !after => (index.wrapping_add(2), index.wrapping_add(1)),
            Phase::StepForth => break,
        };
        flag = add_non_negative_period(0, period, Zone::UTC, periods)?;
        index = next_index;
        if phase == Phase::Hint {
            phase = Phase::JumpBack;
        }
    }
    Ok((index, flag.wrapping_sub(lastbucket.wrapping_add(1))))
}

/// `BUCKETIZE.CALENDAR`:
/// `[ series… bucketizer lastbucket bucketduration bucketcount (timezone) ] BUCKETIZE.CALENDAR`.
fn bucketize_calendar(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "BUCKETIZE.CALENDAR";
    let Value::List(list) = stack.pop()? else {
        return Err(fail(NAME, "expects a list as input."));
    };
    if list.borrow().len() < 5 {
        return Err(fail(
            NAME,
            "needs a list of at least 5 parameters as input.",
        ));
    }
    // Warp 10 removes the time zone from the parameter list itself.
    let id = match list.borrow().last() {
        Some(Value::String(id)) => Some(Rc::clone(id)),
        _ => None,
    };
    let zone = match id {
        Some(id) => {
            list.borrow_mut().pop();
            Zone::for_id(Some(&id))?
        }
        None => Zone::UTC,
    };
    let params = list.borrow().clone();
    let n = params.len();
    let mut series: Vec<Gts> = Vec::new();
    for param in &params[..n - 4] {
        match param {
            Value::Gts(gts) => series.push(Rc::clone(gts)),
            Value::List(list) => {
                for element in list.borrow().iter() {
                    let Value::Gts(gts) = element else {
                        return Err(fail(
                            NAME,
                            "expects a list of Geo Time Series as first parameter.",
                        ));
                    };
                    series.push(Rc::clone(gts));
                }
            }
            _ => {
                return Err(fail(
                    NAME,
                    "expects a Geo Time Series or a list of Geo Time Series as first parameter.",
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
                "expects a bucketizer function or a macro as fourth to last parameter.",
            ));
        }
    };
    let (Value::Long(lastbucket), Value::String(duration), Value::Long(bucketcount)) =
        (&params[n - 3], &params[n - 2], &params[n - 1])
    else {
        return Err(fail(
            NAME,
            "expects lastbucket, bucketduration, bucketcount (and optionally timezone) as last parameters.",
        ));
    };
    let (lastbucket, bucketcount) = (*lastbucket, *bucketcount);
    if lastbucket == 0 {
        return Err(fail(
            NAME,
            "does not allow lastbucket to be 0. It must be specified.",
        ));
    }
    if bucketcount < 0 {
        return Err(fail(NAME, "expects a positive bucketcount."));
    }
    let maxbuckets = stack.limits.max_buckets;
    if bucketcount > maxbuckets {
        return Err(fail(
            NAME,
            &format!(
                "error: bucket count ({bucketcount}) would exceed maximum value of {maxbuckets}. Consider raising the limit or using capabilities."
            ),
        ));
    }
    if series.iter().any(|gts| {
        let gts = gts.borrow();
        let attributes = &gts.metadata.attributes;
        attributes.contains_key(DURATION_ATTRIBUTE_KEY)
            || attributes.contains_key(OFFSET_ATTRIBUTE_KEY)
            || attributes.contains_key(TIMEZONE_ATTRIBUTE_KEY)
    }) {
        return Err(fail(
            NAME,
            &format!(
                "expects GTS for which the attributes {DURATION_ATTRIBUTE_KEY}, {OFFSET_ATTRIBUTE_KEY} and {TIMEZONE_ATTRIBUTE_KEY} are not set. If an input GTS is supposed to be already duration-bucketized, duration-unbucketize it first before applying a new duration-bucketization."
            ),
        ));
    }
    let Some(bucketizer) = bucketizer else {
        return Err(fail(NAME, "expects a non null bucketizer."));
    };
    match duration.chars().next() {
        Some('P') => {}
        Some(_) => {
            return Err(fail(
                NAME,
                "expects that the bucketduration is in ISO8601 duration format.",
            ));
        }
        None => {
            return Err(BoundsError::StringIndex {
                index: 0,
                length: 0,
            }
            .into());
        }
    }
    let (period, offset) = duration_to_period(duration)
        .map_err(|err| fail_with_cause(NAME, "encountered an exception.", err))?;
    let period = CalendarPeriod { period, offset };
    // `toDurationFrom(EPOCH)`: the length of one period from the epoch, in UTC.
    let average_span = add_period(0, &period.period, 0, Zone::UTC, 1)?.wrapping_add(period.offset);
    if average_span < 0 {
        return Err(fail(
            NAME,
            "expects the bucketduration parameter to be a positive ISO8601 duration.",
        ));
    }
    if average_span == 0 {
        return Err(Error::division_by_zero());
    }
    let (lastbucket_index, bucketoffset) = locate_lastbucket(lastbucket, &period, average_span)?;
    let params = Calendar {
        period,
        zone,
        bucketcount,
        lastbucket,
        lastbucket_index,
        bucketizer: &bucketizer,
        maxbuckets,
    };
    let mut bucketized = Vec::with_capacity(series.len());
    for gts in &series {
        let result = duration_bucketize(stack, gts, &params).map_err(|err| {
            if err.is_signal() || err.is_runtime() {
                err
            } else {
                fail_with_cause(NAME, "encountered an exception.", err)
            }
        })?;
        {
            let mut b = result.borrow_mut();
            let attributes = &mut b.metadata.attributes;
            // Inserted in the iteration order of Warp 10's `HashMap` for these three keys.
            attributes.insert(TIMEZONE_ATTRIBUTE_KEY.to_owned(), zone.id());
            attributes.insert(DURATION_ATTRIBUTE_KEY.to_owned(), duration.to_string());
            attributes.insert(OFFSET_ATTRIBUTE_KEY.to_owned(), bucketoffset.to_string());
        }
        bucketized.push(Value::Gts(result));
    }
    stack.push(Value::list(bucketized));
    Ok(())
}

/// `Long.parseLong`.
fn parse_long(text: &str) -> Result<i64> {
    text.parse::<i64>()
        .map_err(|_| NumberFormatError::InvalidInput(text.to_owned()).into())
}

/// `UNBUCKETIZECALENDAR.gtsOp`: a calendar-bucketized series back on its timestamps (the end
/// of each bucket), without the bucketization nor the calendar attributes.
fn unbucketize_calendar_gts(gts: &GeoTimeSerie) -> Result<Value> {
    const NAME: &str = "UNBUCKETIZE.CALENDAR";
    if !is_duration_bucketized(gts) {
        return Err(fail(
            NAME,
            "expects input GTS to be duration-bucketized. This information is stored in attributes.",
        ));
    }
    let mut result = gts.clone_empty();
    if gts.size() == 0 {
        clear_calendar(&mut result);
        return Ok(Value::Gts(result.shared()));
    }
    if !gts.is_bucketized() {
        return Err(fail(NAME, "expects input GTS to be bucketized."));
    }
    let attribute = |key: &str| gts.metadata.attributes.get(key).map_or("", String::as_str);
    let (period, offset) = duration_to_period(attribute(DURATION_ATTRIBUTE_KEY))?;
    let period = CalendarPeriod { period, offset };
    let bucketoffset = parse_long(attribute(OFFSET_ATTRIBUTE_KEY))?;
    let lastbucket_index = gts.last_tick();
    let lastbucket =
        add_non_negative_period(0, &period, Zone::UTC, lastbucket_index + 1)? - 1 - bucketoffset;
    let zone = Zone::for_id(Some(attribute(TIMEZONE_ATTRIBUTE_KEY)))?;
    clear_calendar(&mut result);
    for i in 0..gts.size() {
        let tick =
            add_non_negative_period(lastbucket, &period, zone, gts.ticks[i] - lastbucket_index)?;
        result.set_value(
            tick,
            gts.location_at(i),
            gts.elevation_at(i),
            &gts.value_at(i),
            false,
        );
    }
    Ok(Value::Gts(result.shared()))
}

/// `UNBUCKETIZE.CALENDAR`.
fn unbucketize_calendar(stack: &mut Stack) -> Result<()> {
    gts_or_list_try(stack, "UNBUCKETIZE.CALENDAR", |gts| {
        unbucketize_calendar_gts(&gts.borrow())
    })
}

/// `UNBUCKETIZE`: clear the bucketization parameters of the series, in place.
fn unbucketize(stack: &mut Stack) -> Result<()> {
    gts_or_list(stack, "UNBUCKETIZE", |gts| {
        let mut g = gts.borrow_mut();
        g.bucketcount = 0;
        g.bucketspan = 0;
        g.lastbucket = 0;
        Value::Gts(Rc::clone(gts))
    })
}
