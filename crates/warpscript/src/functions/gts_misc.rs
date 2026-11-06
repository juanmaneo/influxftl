// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `CORRELATE`, `LOWERHULL`, `UPPERHULL`, `LOCATIONOFFSET` and `MOTIONSPLIT`.

use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::gts::{element_or_list, gts_or_list_try};
use super::gts_stats::standardize_gts;
use super::gts_time::timeclip_gts;
use super::misc::TIME_UNITS_PER_MS;
use crate::error::{Error, Result};
use crate::geoxp::{loxodromic_distance, orthodromic_distance};
use crate::gts::{GeoTimeSerie, Gts, NO_ELEVATION, NO_LOCATION, Values};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("CORRELATE", Entry::Stack(correlate)),
    ("LOWERHULL", Entry::Stack(|s| hull(s, "LOWERHULL", false))),
    ("UPPERHULL", Entry::Stack(|s| hull(s, "UPPERHULL", true))),
    ("LOCATIONOFFSET", Entry::Stack(locationoffset)),
    ("MOTIONSPLIT", Entry::Stack(motionsplit)),
];

/// The `double` of a numeric value (`NaN` otherwise, which numeric series never hold).
fn double(value: &Value) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}

/// Whether a series is bucketized, filled and numeric.
fn filled_numeric(gts: &GeoTimeSerie) -> bool {
    gts.is_bucketized()
        && gts.size() as i64 == gts.bucketcount
        && matches!(gts.values, Values::Long(_) | Values::Double(_))
}

// -----------------------------------------------------------------------------------------
// CORRELATE
// -----------------------------------------------------------------------------------------

/// `CORRELATE.correlate` of `a` (sorted) and `b` (sorted) at `offset`: the sum of the products
/// of the standardized values over their common span, divided by `count - 1`.
fn cross_correlation(a: &GeoTimeSerie, b: &GeoTimeSerie, offset: i64) -> f64 {
    let (na, nb) = (a.size(), b.size());
    let (mut ia, mut ib) = (0, 0);
    while ia < na && ib < nb {
        let shifted = a.ticks[ia].wrapping_add(offset);
        match shifted.cmp(&b.ticks[ib]) {
            std::cmp::Ordering::Less => ia += 1,
            std::cmp::Ordering::Greater => ib += 1,
            std::cmp::Ordering::Equal => break,
        }
    }
    if ia == na || ib == nb {
        return 0.0;
    }
    let end = a.ticks[na - 1].min(b.ticks[nb - 1].wrapping_sub(offset));
    let clipped_a = standardize_gts(&timeclip_gts(&mut a.clone(), a.ticks[ia], end));
    let clipped_b = standardize_gts(&timeclip_gts(
        &mut b.clone(),
        b.ticks[ib],
        end.wrapping_add(offset),
    ));
    let count = clipped_a.size().min(clipped_b.size());
    let mut sum = 0.0;
    for i in 0..count {
        sum += double(&clipped_a.value_at(i)) * double(&clipped_b.value_at(i));
    }
    // `count` is at least 1: both clipped series hold their first common tick.
    if count > 1 {
        sum / (count - 1) as f64
    } else {
        sum
    }
}

/// `CORRELATE`: the cross-correlations of a series with each of a list at the given offsets.
fn correlate(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "CORRELATE";
    const NOT_FILLED: &str = "operates on bucketized, filled numeric Geo Time Series.";
    let Value::List(offsets) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a list of offsets on the top of the stack.",
        ));
    };
    let mut offset_values = Vec::new();
    for offset in offsets.borrow().iter() {
        // `((Number) o).longValue()`.
        offset_values.push(match offset {
            Value::Null => return Err(Error::null_pointer()),
            other => other
                .as_i64()
                .ok_or_else(|| Error::class_cast(other.java_class(), "java.lang.Number"))?,
        });
    }
    let top = stack.pop()?;
    let series: Vec<Gts> = match &top {
        Value::Gts(gts) => vec![Rc::clone(gts)],
        Value::List(list) => {
            let mut series = Vec::new();
            for element in list.borrow().iter() {
                let Value::Gts(gts) = element else {
                    stack.push(top.clone());
                    return Err(fail(NAME, "can only operate on Geo Time Series instances."));
                };
                series.push(Rc::clone(gts));
            }
            series
        }
        _ => {
            stack.push(top.clone());
            return Err(fail(NAME, "can only operate on Geo Time Series instances."));
        }
    };
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series two levels below the top of the stack.",
        ));
    };
    if !filled_numeric(&gts.borrow()) {
        return Err(fail(NAME, NOT_FILLED));
    }
    let bucketspan = gts.borrow().bucketspan;
    for g in &series {
        let g = g.borrow();
        if !filled_numeric(&g) {
            return Err(fail(NAME, NOT_FILLED));
        }
        if g.bucketspan != bucketspan {
            return Err(fail(
                NAME,
                &format!(
                    "operates on bucketized Geo Time Series with all the same bucketspan. The expected bucketspan is {bucketspan}"
                ),
            ));
        }
    }
    if offset_values
        .iter()
        .any(|o| o.wrapping_rem(bucketspan) != 0)
    {
        return Err(fail(
            NAME,
            &format!("expects offsets to be multiples of the bucketspan ({bucketspan})."),
        ));
    }
    // Warp 10 sorts the series in place (the list ones only when there are offsets).
    gts.borrow_mut().sort(false);
    if !offset_values.is_empty() {
        for g in &series {
            g.borrow_mut().sort(false);
        }
    }
    let a = gts.borrow().clone();
    let mut results = Vec::with_capacity(series.len());
    for g in &series {
        let b = g.borrow().clone();
        let mut correlation = GeoTimeSerie::new();
        correlation.metadata = b.metadata.clone();
        for &offset in &offset_values {
            correlation.set_value(
                offset,
                NO_LOCATION,
                NO_ELEVATION,
                &Value::Double(cross_correlation(&a, &b, offset)),
                false,
            );
        }
        results.push(Value::Gts(correlation.shared()));
    }
    stack.push(Value::list(results));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// LOWERHULL / UPPERHULL
// -----------------------------------------------------------------------------------------

/// `GTSHelper.crossProduct` of the datapoints `a`, `b` and `c` (tick, value).
fn cross_product(gts: &GeoTimeSerie, a: usize, b: usize, c: usize) -> f64 {
    let (ax, ay) = (gts.ticks[a] as f64, double(&gts.value_at(a)));
    let (bx, by) = (gts.ticks[b] as f64, double(&gts.value_at(b)));
    let (cx, cy) = (gts.ticks[c] as f64, double(&gts.value_at(c)));
    (ax - bx) * (cy - by) - (ay - by) * (cx - bx)
}

/// `GTSHelper.lowerHull` / `upperHull`: Andrew's monotone chain over the series sorted in
/// place.
fn hull_gts(gts: &mut GeoTimeSerie, upper: bool) -> GeoTimeSerie {
    gts.sort(false);
    if gts.size() < 3 {
        return gts.clone();
    }
    let mut chain: Vec<usize> = Vec::new();
    for i in 0..gts.size() {
        while let [.., b, c] = chain[..] {
            let cross = cross_product(gts, b, c, i);
            if (upper && cross < 0.0) || (!upper && cross > 0.0) {
                chain.pop();
            } else {
                break;
            }
        }
        chain.push(i);
    }
    gts.select(&chain)
}

/// `LOWERHULL` / `UPPERHULL`: the lower or upper convex hull of numeric series.
fn hull(stack: &mut Stack, name: &str, upper: bool) -> Result<()> {
    gts_or_list_try(stack, name, |gts| {
        let mut gts = gts.borrow_mut();
        if gts.size() == 0 {
            return Ok(Value::Gts(gts.clone().shared()));
        }
        if !matches!(gts.values, Values::Long(_) | Values::Double(_)) {
            return Err(fail(name, "expects a numeric GTS."));
        }
        Ok(Value::Gts(hull_gts(&mut gts, upper).shared()))
    })
}

// -----------------------------------------------------------------------------------------
// LOCATIONOFFSET
// -----------------------------------------------------------------------------------------

/// `LOCATIONOFFSET.gtsOp`: the first and last datapoints and those at least `dist` meters
/// (loxodromic) away from the previously kept location.
fn location_offset(gts: &mut GeoTimeSerie, dist: f64) -> GeoTimeSerie {
    let mut offset = gts.clone_empty();
    gts.sort(false);
    let n = gts.size();
    if n == 0 {
        return offset;
    }
    let keep = |offset: &mut GeoTimeSerie, i: usize| {
        offset.set_value(
            gts.tick_at(i),
            gts.location_at(i),
            gts.elevation_at(i),
            &gts.value_at(i),
            false,
        );
    };
    let mut last = gts.location_at(0);
    keep(&mut offset, 0);
    for i in 1..n.saturating_sub(1) {
        let location = gts.location_at(i);
        if location == NO_LOCATION {
            continue;
        }
        if last == NO_LOCATION || loxodromic_distance(last, location) >= dist {
            last = location;
            keep(&mut offset, i);
        }
    }
    if n > 1 {
        keep(&mut offset, n - 1);
    }
    offset
}

/// `LOCATIONOFFSET`: thin out series to datapoints at least a distance apart.
fn locationoffset(stack: &mut Stack) -> Result<()> {
    let dist = stack.pop()?;
    let Some(dist) = dist.as_f64() else {
        return Err(fail(
            "LOCATIONOFFSET",
            "expects a distance in meters on top of the stack.",
        ));
    };
    gts_or_list_try(stack, "LOCATIONOFFSET", |gts| {
        Ok(Value::Gts(
            location_offset(&mut gts.borrow_mut(), dist).shared(),
        ))
    })
}

// -----------------------------------------------------------------------------------------
// MOTIONSPLIT
// -----------------------------------------------------------------------------------------

/// The parameters of `MOTIONSPLIT`.
struct Motion {
    split_number_label: Option<String>,
    split_type_label: Option<String>,
    stopped_time_label: Option<String>,
    time_threshold: i64,
    distance_threshold: f64,
    zone_radius: f64,
    zone_max_speed: f64,
    zone_time: i64,
    zone_max_mean_speed: f64,
    zone_split: bool,
}

/// Put a label on a series, when both exist.
fn label(gts: Option<&mut GeoTimeSerie>, name: Option<&String>, value: &str) {
    if let (Some(gts), Some(name)) = (gts, name) {
        gts.metadata.labels.insert(name.clone(), value.to_owned());
    }
}

impl Motion {
    /// Split the stopped part off `split` (the datapoints after its last tick minus
    /// `time_stopped`), the moving part first; `number` labels the stopped part.
    fn split_stopped(
        &self,
        split: &mut GeoTimeSerie,
        time_stopped: i64,
        number: Option<i64>,
    ) -> [GeoTimeSerie; 2] {
        let last = split.ticks.iter().copied().max().unwrap_or(i64::MIN);
        let stop = last.wrapping_sub(time_stopped);
        let mut moving = timeclip_gts(split, i64::MIN, stop);
        let mut stopped = timeclip_gts(split, stop.wrapping_add(1), i64::MAX);
        label(Some(&mut moving), self.split_type_label.as_ref(), "moving");
        if let Some(number) = number {
            label(
                Some(&mut stopped),
                self.split_number_label.as_ref(),
                &number.to_string(),
            );
        }
        [moving, stopped]
    }

    /// `MOTIONSPLIT.motionSplit`.
    #[allow(clippy::too_many_lines)]
    fn split(&self, gts: &mut GeoTimeSerie) -> Vec<GeoTimeSerie> {
        const UNITS_PER_S: f64 = (TIME_UNITS_PER_MS * 1000) as f64;
        gts.sort(false);
        let n = gts.size();
        if n <= 1 {
            let mut result = gts.clone();
            label(Some(&mut result), self.split_type_label.as_ref(), "end");
            label(Some(&mut result), self.split_number_label.as_ref(), "1");
            return vec![result];
        }
        let mut done: Vec<GeoTimeSerie> = Vec::new();
        // The split being filled (Warp 10's `split`, `null` until the first datapoint).
        let mut current = GeoTimeSerie::new();
        let mut started = false;
        let mut gtsid = 1i64;
        let mut must_split = true;
        let mut ref_location = NO_LOCATION;
        let mut ref_timestamp = i64::MIN;
        let mut previous_timestamp = i64::MIN;
        let mut previous_timestamp_with_location = i64::MIN;
        let mut previous_valid_location = NO_LOCATION;
        let mut traveled = 0.0;
        let mut reason = "end";
        for idx in 0..n {
            let timestamp = gts.tick_at(idx);
            let location = gts.location_at(idx);
            if ref_location == NO_LOCATION && location != NO_LOCATION {
                ref_location = location;
                ref_timestamp = timestamp;
            }
            let time_stopped = previous_timestamp_with_location.wrapping_sub(ref_timestamp);
            if self.zone_radius < f64::MAX
                && self.zone_time < i64::MAX
                && ref_location != NO_LOCATION
                && location != NO_LOCATION
            {
                let mut speed = 0.0;
                if previous_valid_location != NO_LOCATION
                    && timestamp != previous_timestamp_with_location
                {
                    speed = orthodromic_distance(previous_valid_location, location)
                        / (timestamp.wrapping_sub(previous_timestamp_with_location) as f64
                            / UNITS_PER_S);
                }
                if orthodromic_distance(ref_location, location) > self.zone_radius
                    || speed > self.zone_max_speed
                {
                    let mut mean_speed = 0.0;
                    if previous_timestamp_with_location != ref_timestamp {
                        mean_speed = traveled
                            / (previous_timestamp_with_location.wrapping_sub(ref_timestamp) as f64
                                / UNITS_PER_S);
                    }
                    if time_stopped > self.zone_time && mean_speed < self.zone_max_mean_speed {
                        reason = "stopped";
                        must_split = true;
                        label(
                            started.then_some(&mut current),
                            self.stopped_time_label.as_ref(),
                            &time_stopped.to_string(),
                        );
                    }
                    ref_location = location;
                    ref_timestamp = timestamp;
                    traveled = 0.0;
                } else if previous_valid_location != NO_LOCATION {
                    traveled += orthodromic_distance(previous_valid_location, location);
                }
            }
            if !must_split {
                if previous_valid_location != NO_LOCATION
                    && location != NO_LOCATION
                    && orthodromic_distance(location, previous_valid_location)
                        > self.distance_threshold
                {
                    reason = "distancesplit";
                    must_split = true;
                } else if timestamp.wrapping_sub(previous_timestamp) > self.time_threshold {
                    reason = "timesplit";
                    must_split = true;
                }
            }
            if must_split {
                if started {
                    let mut split = std::mem::take(&mut current);
                    label(Some(&mut split), self.split_type_label.as_ref(), reason);
                    let first = split.ticks.iter().copied().min().unwrap_or(i64::MAX);
                    let last = split.ticks.iter().copied().max().unwrap_or(i64::MIN);
                    if self.zone_split
                        && reason == "stopped"
                        && last.wrapping_sub(time_stopped) != first
                    {
                        let number = self.split_number_label.as_ref().map(|_| gtsid);
                        if number.is_some() {
                            gtsid += 1;
                        }
                        done.extend(self.split_stopped(&mut split, time_stopped, number));
                    } else {
                        done.push(split);
                    }
                }
                current = gts.clone_empty();
                started = true;
                if self.split_number_label.is_some() {
                    label(
                        Some(&mut current),
                        self.split_number_label.as_ref(),
                        &gtsid.to_string(),
                    );
                    gtsid += 1;
                }
                must_split = false;
                ref_location = location;
                ref_timestamp = timestamp;
                traveled = 0.0;
            }
            current.set_value(
                timestamp,
                location,
                gts.elevation_at(idx),
                &gts.value_at(idx),
                false,
            );
            if idx == n - 1 {
                let mut split = std::mem::take(&mut current);
                if previous_timestamp_with_location.wrapping_sub(ref_timestamp) > self.zone_time {
                    label(
                        Some(&mut split),
                        self.stopped_time_label.as_ref(),
                        &time_stopped.to_string(),
                    );
                    label(Some(&mut split), self.split_type_label.as_ref(), "stopped");
                    if self.zone_split {
                        let number = self.split_number_label.as_ref().map(|_| gtsid);
                        done.extend(self.split_stopped(&mut split, time_stopped, number));
                    } else {
                        done.push(split);
                    }
                } else {
                    let kind = if self.zone_split { "moving" } else { "end" };
                    label(Some(&mut split), self.split_type_label.as_ref(), kind);
                    done.push(split);
                }
            }
            previous_timestamp = timestamp;
            if location != NO_LOCATION {
                previous_timestamp_with_location = timestamp;
                previous_valid_location = location;
            }
        }
        done
    }
}

/// `MOTIONSPLIT`: split series into trips on time gaps, distance jumps and stops.
fn motionsplit(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MOTIONSPLIT";
    let Value::Map(params) = stack.pop()? else {
        return Err(fail(NAME, "expects a parameter MAP on top of the stack."));
    };
    let params = params.borrow();
    let get = |key: &str| params.get(&Value::from(key));
    let number = |key: &str, suffix: &str| -> Result<Option<Value>> {
        match get(key) {
            None => Ok(None),
            Some(value) if value.is_number() => Ok(Some(value.clone())),
            Some(_) => Err(fail(NAME, &format!("{key} must be a number{suffix}"))),
        }
    };
    let string = |key: &str| -> Result<Option<String>> {
        match get(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.to_string())),
            Some(_) => Err(fail(NAME, &format!("{key} must be a string."))),
        }
    };
    let long = |v: Option<Value>| v.and_then(|v| v.as_i64()).unwrap_or(i64::MAX);
    let dbl = |v: Option<Value>| v.and_then(|v| v.as_f64()).unwrap_or(f64::MAX);
    let time_threshold = long(number("timesplit", ".")?);
    let split_number_label = string("label.split.number")?;
    let split_type_label = string("label.split.type")?;
    let stopped_time_label = string("label.stopped.time")?;
    let distance_threshold = dbl(number("distance.split", ".")?);
    let zone_max_speed = dbl(number("stopped.max.speed", ": speed in m/s.")?);
    let zone_max_mean_speed = dbl(number("stopped.max.mean.speed", ": speed in m/s.")?);
    let zone_time = long(number("stopped.min.time", ": time in platform time unit")?);
    let zone_radius = dbl(number("stopped.max.radius", ": radius in meters.")?);
    let zone_split = match get("stopped.split") {
        None => false,
        Some(Value::Boolean(b)) => *b,
        Some(_) => return Err(fail(NAME, "stopped.split must be a boolean.")),
    };
    let motion = Motion {
        split_number_label,
        split_type_label,
        stopped_time_label,
        time_threshold,
        distance_threshold,
        zone_radius,
        zone_max_speed,
        zone_time,
        zone_max_mean_speed,
        zone_split,
    };
    drop(params);
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => Ok(Value::list(
            motion
                .split(&mut gts.borrow_mut())
                .into_iter()
                .map(|g| Value::Gts(g.shared()))
                .collect(),
        )),
        _ => Err(fail(NAME, "can only be applied on Geo Time Series.")),
    })
}
