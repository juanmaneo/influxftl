// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Dynamic Time Warping: `DTW` (min-max normalized), `ZDTW` (Z-normalized), `RAWDTW` and
//! `OPTDTW` (the best matches of a query in a sequence).

use super::Entry;
use super::args::{fail, fail_with_cause};
use crate::error::{Class, Error, Result};
use crate::geoxp;
use crate::gts::{GeoTimeSerie, Gts, NO_ELEVATION, NO_LOCATION, Type};
use crate::jmath;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("DTW", Entry::Stack(dtw)),
    ("ZDTW", Entry::Stack(zdtw)),
    ("RAWDTW", Entry::Stack(rawdtw)),
    ("OPTDTW", Entry::Stack(optdtw)),
];

/// The characteristic of the series the warping compares.
#[derive(Clone, Copy, PartialEq)]
enum Characteristic {
    Values,
    Locations,
    Elevations,
    Timestamps,
}

/// The distance between two elements (`DTW.DTWDistance`).
#[derive(Clone, Copy)]
enum Distance {
    Manhattan,
    Euclidean,
    SquaredEuclidean,
    Loxodromic,
    Orthodromic,
}

/// The data compared: one array per dimension.
type Series = Vec<Vec<f64>>;

/// The distance between element `i1` of `v1` and element `i2` of `v2`.
fn measure(distance: Distance, v1: &Series, i1: usize, v2: &Series, i2: usize) -> Result<f64> {
    let squares = || {
        let mut d = 0.0;
        for (a, b) in v1.iter().zip(v2) {
            d += jmath::pow(a[i1] - b[i2], 2.0);
        }
        d
    };
    let degrees = |name: &str| -> Result<(f64, f64, f64, f64)> {
        if v1.len() != 2 {
            return Err(Error::new(format!(
                "{name} distance must be given two dimensions: lat, lon."
            )));
        }
        Ok((
            v1[0][i1].to_radians(),
            v1[1][i1].to_radians(),
            v2[0][i2].to_radians(),
            v2[1][i2].to_radians(),
        ))
    };
    // Nautical miles per radian.
    let scale = (180.0 / std::f64::consts::PI) * (1852.0 * 60.0);
    Ok(match distance {
        Distance::Manhattan => {
            let mut d = 0.0;
            for (a, b) in v1.iter().zip(v2) {
                d += (a[i1] - b[i2]).abs();
            }
            d
        }
        Distance::Euclidean => squares().sqrt(),
        Distance::SquaredEuclidean => squares(),
        Distance::Loxodromic => {
            let (lat1, lon1, lat2, lon2) = degrees("Loxodromic")?;
            // Java's `(a + b) / 2`.
            #[allow(clippy::manual_midpoint)]
            let x = (lon1 - lon2) * ((lat1 + lat2) / 2.0).cos();
            let y = lat1 - lat2;
            scale * (x * x + y * y).sqrt()
        }
        Distance::Orthodromic => {
            let (lat1, lon1, lat2, lon2) = degrees("Orthodromic")?;
            scale * (lat1.sin() * lat2.sin() + lat1.cos() * lat2.cos() * (lon2 - lon1).cos()).acos()
        }
    })
}

/// `DTW.compute` on raw data: the warping pseudo-distance within `window` (an index
/// difference), `-1` when it exceeds `threshold`.
fn warp<'a>(
    mut v1: &'a Series,
    mut v2: &'a Series,
    window: i32,
    threshold: f64,
    distance: Distance,
) -> Result<f64> {
    let mut len1 = v1[0].len() as i32;
    let mut len2 = v2[0].len() as i32;
    if len1 > len2 {
        std::mem::swap(&mut v1, &mut v2);
        std::mem::swap(&mut len1, &mut len2);
    }
    let mut a = vec![0.0; len1 as usize];
    let mut b = vec![0.0; len1 as usize];
    let mut below_threshold = false;
    let window = window.min(len2 - 1).max(len2 - len1);
    for i in 0..len2 {
        let max_window = (i + 1).checked_add(window).unwrap_or(i32::MAX);
        for j in (i - window).max(0)..len1.min(max_window) {
            let ju = j as usize;
            let best = if i == 0 && j == 0 {
                0.0
            } else {
                let left = if i > 0 && i.wrapping_add(window) != j {
                    a[ju]
                } else {
                    f64::INFINITY
                };
                let bottom = if j > 0 && i - window != j {
                    b[ju - 1]
                } else {
                    f64::INFINITY
                };
                let bottom_left = if j > 0 { a[ju - 1] } else { f64::INFINITY };
                jmath::min(left, jmath::min(bottom, bottom_left))
            };
            if threshold >= best {
                b[ju] = measure(distance, v1, ju, v2, i as usize)? + best;
                if b[ju] <= threshold {
                    below_threshold = true;
                }
            } else {
                b[ju] = f64::INFINITY;
            }
        }
        if !below_threshold {
            return Ok(-1.0);
        }
        std::mem::swap(&mut a, &mut b);
    }
    // Without any element (only reachable for the locations or elevations of a series
    // emptied in place, where Warp 10 reads its stale arrays) there is no distance.
    let last = a
        .get((len1 as usize).wrapping_sub(1))
        .copied()
        .unwrap_or(f64::NAN);
    Ok(if last <= threshold { last } else { -1.0 })
}

/// `DoubleUtils.muvar`, faithfully: the shift meant to avoid cancellation ends up being the
/// first zero value (or the last one) and is divided by the count when added back. Java
/// fails on an empty array; callers check it when it matters (the results are `NaN`).
fn muvar(values: &[f64]) -> (f64, f64) {
    let first = values.first().copied().unwrap_or_default();
    let n = values.len();
    let mut shift = first;
    let mut i = 0;
    while shift != 0.0 && i < n {
        shift = values[i];
        i += 1;
    }
    let mut sum = 0.0;
    let mut sumsq = 0.0;
    for value in values {
        let term = value - shift;
        sum += term;
        sumsq += term * term;
    }
    let mean = sum / n as f64;
    let var = (sumsq / n as f64) - (mean * mean);
    (mean + (shift / n as f64), var)
}

/// `DoubleUtils.musigma` with Bessel's correction.
fn musigma(values: &[f64]) -> (f64, f64) {
    let (mu, mut var) = muvar(values);
    let n = values.len();
    if n > 1 {
        var = var * n as f64 / (n - 1) as f64;
    }
    (mu, var.sqrt())
}

/// Min-max normalize `values` in place.
fn normalize_range(values: &mut [f64], name: &str) -> Result<()> {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for &v in values.iter() {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    let range = max - min;
    if range == 0.0 {
        return Err(fail(name, "cannot normalize a constant GTS."));
    }
    for v in values {
        *v = (*v - min) / range;
    }
    Ok(())
}

/// The data of `characteristic` of a series (sorted in place first).
fn series_data(gts: &Gts, characteristic: Characteristic, name: &str) -> Result<Series> {
    gts.borrow_mut().sort(false);
    let g = gts.borrow();
    let n = g.size();
    Ok(match characteristic {
        Characteristic::Values => vec![
            (0..n)
                .map(|i| g.value_at(i).as_f64().unwrap_or_default())
                .collect(),
        ],
        // `GTSHelper.getTicks` copies the ticks array, which a series that never had a
        // datapoint does not have.
        Characteristic::Timestamps if n == 0 => return Err(Error::null_pointer()),
        Characteristic::Timestamps => vec![g.ticks.iter().map(|&t| t as f64).collect()],
        Characteristic::Locations => {
            let missing = || {
                fail(
                    name,
                    "expects GTSs to have locations when DTW is applied to them.",
                )
            };
            let locations = g.locations.as_ref().ok_or_else(missing)?;
            let mut lats = Vec::with_capacity(n);
            let mut lons = Vec::with_capacity(n);
            for &location in locations {
                if location == NO_LOCATION {
                    return Err(missing());
                }
                let (lat, lon) = geoxp::from_geoxp_point(location);
                lats.push(lat);
                lons.push(lon);
            }
            vec![lats, lons]
        }
        Characteristic::Elevations => {
            let missing = || {
                fail(
                    name,
                    "expects GTSs to have elevations when DTW is applied to them.",
                )
            };
            let elevations = g.elevations.as_ref().ok_or_else(missing)?;
            let mut out = Vec::with_capacity(n);
            for &elevation in elevations {
                if elevation == NO_ELEVATION {
                    return Err(missing());
                }
                out.push(elevation as f64);
            }
            vec![out]
        }
    })
}

/// The normalization applied before warping.
#[derive(Clone, Copy, PartialEq)]
enum Normalization {
    None,
    Range,
    Z,
}

/// The parameters of a warping of two series.
struct Warping<'a> {
    window: i32,
    threshold: f64,
    characteristic: Characteristic,
    distance: Distance,
    normalization: Normalization,
    name: &'a str,
}

/// `DTW.compute` on two series.
fn compute(gts1: &Gts, gts2: &Gts, params: &Warping) -> Result<f64> {
    let Warping {
        window,
        threshold,
        characteristic,
        distance,
        normalization,
        name,
    } = *params;
    if characteristic == Characteristic::Values {
        let numeric = |g: &GeoTimeSerie| matches!(g.type_of(), Type::Long | Type::Double);
        if !numeric(&gts1.borrow()) || !numeric(&gts2.borrow()) {
            return Err(fail(name, "can only operate on numerical Geo Time Series."));
        }
    }
    let mut v1 = series_data(gts1, characteristic, name)?;
    let mut v2 = series_data(gts2, characteristic, name)?;
    for (d1, d2) in v1.iter_mut().zip(v2.iter_mut()) {
        match normalization {
            Normalization::None => {}
            Normalization::Range => {
                normalize_range(d1, name)?;
                normalize_range(d2, name)?;
            }
            Normalization::Z => {
                let (mu, sigma) = musigma(d1);
                for v in d1.iter_mut() {
                    *v = (*v - mu) / sigma;
                }
                // Warp 10 divides the second series by its variance.
                let (mu, var) = muvar(d2);
                for v in d2.iter_mut() {
                    *v = (*v - mu) / var;
                }
            }
        }
    }
    warp(&v1, &v2, window, threshold, distance)
}

/// `(int) Math.min(Integer.MAX_VALUE, window)`, a negative window meaning none.
fn window_of(window: i64) -> i32 {
    let window = window.min(i64::from(i32::MAX)) as i32;
    if window < 0 { i32::MAX } else { window }
}

/// `DTW.apply`: `gts gts (window) threshold (distance) (characteristic) DTW`.
fn dtw_impl(stack: &mut Stack, name: &str, normalization: Normalization) -> Result<()> {
    let mut top = stack.pop()?;
    let mut characteristic = Characteristic::Values;
    let mut distance = Distance::Manhattan;
    if let Value::String(s) = &top {
        characteristic = match s.to_lowercase().as_str() {
            "values" => Characteristic::Values,
            "locations" => Characteristic::Locations,
            "elevations" => Characteristic::Elevations,
            "timestamps" => Characteristic::Timestamps,
            _ => {
                return Err(fail(
                    name,
                    "expects the characteristic of the GTS to compute the DTW on to be values, locations, elevations or timestamps.",
                ));
            }
        };
        top = stack.pop()?;
    }
    if let Value::String(s) = &top {
        distance = match s.to_lowercase().as_str() {
            "manhattan" => Distance::Manhattan,
            "euclidean" => Distance::Euclidean,
            "squaredeuclidean" => Distance::SquaredEuclidean,
            "loxodromic" => Distance::Loxodromic,
            "orthodromic" => Distance::Orthodromic,
            _ => {
                return Err(fail(
                    name,
                    "expects the distance to use in the DTW to be manhattan, euclidean, loxodromic or orthodromic.",
                ));
            }
        };
        top = stack.pop()?;
    }
    let Some(mut threshold) = top.as_f64() else {
        return Err(fail(
            name,
            "expects a numeric threshold on top of the stack.",
        ));
    };
    if threshold <= 0.0 {
        threshold = f64::INFINITY;
    }
    let mut top = stack.pop()?;
    let mut window = i32::MAX;
    if let Value::Long(l) = top {
        window = window_of(l);
        top = stack.pop()?;
    }
    let below = "expects two Geo Time Series below the threshold.";
    let Value::Gts(gts1) = top else {
        return Err(fail(name, below));
    };
    let Value::Gts(gts2) = stack.pop()? else {
        return Err(fail(name, below));
    };
    let params = Warping {
        window,
        threshold,
        characteristic,
        distance,
        normalization,
        name,
    };
    let d = compute(&gts1, &gts2, &params).map_err(|err| {
        if err.is_runtime() {
            err
        } else {
            fail_with_cause(name, "failed.", err)
        }
    })?;
    stack.push(Value::Double(d));
    Ok(())
}

/// `DTW`: min-max normalized.
fn dtw(stack: &mut Stack) -> Result<()> {
    dtw_impl(stack, "DTW", Normalization::Range)
}

/// `ZDTW`: Z-normalized.
fn zdtw(stack: &mut Stack) -> Result<()> {
    dtw_impl(stack, "ZDTW", Normalization::Z)
}

/// `RAWDTW`: not normalized.
fn rawdtw(stack: &mut Stack) -> Result<()> {
    dtw_impl(stack, "RAWDTW", Normalization::None)
}

/// A `java.util.PriorityQueue` of `(index, distance)` ordered by distance: the binary heap
/// layout matters, as `OPTDTW` reads the heap array itself.
#[derive(Default)]
struct DistanceQueue {
    heap: Vec<(i64, f64)>,
}

impl DistanceQueue {
    /// `Double.compareTo` on the distances.
    fn less(a: (i64, f64), b: (i64, f64)) -> std::cmp::Ordering {
        a.1.total_cmp(&b.1)
    }

    /// `PriorityQueue.offer` (`siftUp`).
    fn add(&mut self, entry: (i64, f64)) {
        let mut k = self.heap.len();
        self.heap.push(entry);
        while k > 0 {
            let parent = (k - 1) >> 1;
            if Self::less(entry, self.heap[parent]).is_ge() {
                break;
            }
            self.heap[k] = self.heap[parent];
            k = parent;
        }
        self.heap[k] = entry;
    }

    /// `PriorityQueue.poll` (`siftDown`).
    fn poll(&mut self) -> Option<(i64, f64)> {
        let last = self.heap.pop()?;
        let n = self.heap.len();
        if n == 0 {
            return Some(last);
        }
        let result = self.heap[0];
        let mut k = 0;
        let half = n >> 1;
        while k < half {
            let mut child = (k << 1) + 1;
            let right = child + 1;
            if right < n && Self::less(self.heap[child], self.heap[right]).is_gt() {
                child = right;
            }
            if Self::less(last, self.heap[child]).is_le() {
                break;
            }
            self.heap[k] = self.heap[child];
            k = child;
        }
        self.heap[k] = last;
        Some(result)
    }
}

/// `((Number) element).doubleValue()` for each element of a list.
fn doubles(list: &[Value]) -> Result<Vec<f64>> {
    list.iter()
        .map(|value| match value {
            Value::Null => Err(Error::null_pointer()),
            other => other
                .as_f64()
                .ok_or_else(|| Error::class_cast(other.kind(), Class::Number)),
        })
        .collect()
}

/// Z-normalize `values` in place (`DoubleUtils.musigma` with Bessel's correction).
fn znormalize(values: &mut [f64]) {
    let (mu, sigma) = musigma(values);
    for v in values {
        *v = (*v - mu) / sigma;
    }
}

/// `OPTDTW`: `sequence query (window) count OPTDTW`, the `[ index distance ]` of the best
/// (Z-normalized, Manhattan) matches of the query in the sequence, best first.
fn optdtw(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "OPTDTW";
    // `((Number) o).intValue()`.
    let count = match stack.pop()? {
        Value::Long(l) => l as i32,
        Value::Double(d) => d as i32,
        _ => {
            return Err(fail(
                NAME,
                "expects a count of best results to return on top of the stack.",
            ));
        }
    };
    let mut top = stack.pop()?;
    let mut window = i32::MAX;
    if let Value::Long(l) = top {
        window = window_of(l);
        top = stack.pop()?;
    }
    let Value::List(query) = top else {
        return Err(fail(
            NAME,
            "expects a numeric list to use as query below the count.",
        ));
    };
    let mut query = doubles(&query.borrow())?;
    if query.is_empty() {
        // `DoubleUtils.muvar` reads the first value.
        return Err(Error::index_out_of_bounds(0, 0));
    }
    znormalize(&mut query);
    let Value::List(sequence) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a numeric list as the sequence in which to find best matches below the 'query' list.",
        ));
    };
    let sequence = doubles(&sequence.borrow())?;
    let q = query.len();
    if sequence.len() < q {
        return Err(fail(
            NAME,
            "expects the query list to be shorter than the sequence list.",
        ));
    }
    let query = vec![query];
    let mut mindist = f64::INFINITY;
    let mut distances = DistanceQueue::default();
    for i in 0..=sequence.len() - q {
        let mut subsequence = sequence[i..i + q].to_vec();
        znormalize(&mut subsequence);
        // The Manhattan distance on non empty data cannot fail.
        let dist = warp(
            &query,
            &vec![subsequence],
            window,
            mindist,
            Distance::Manhattan,
        )
        .unwrap_or(-1.0);
        if dist < 0.0 {
            continue;
        }
        distances.add((i as i64, dist));
        // Warp 10 reads the bound in the heap array, not at the `count`-th best distance.
        if count > 0 && distances.heap.len() >= count as usize {
            mindist = distances.heap[count as usize - 1].1;
        }
    }
    let mut results = Vec::new();
    while let Some((index, dist)) = distances.poll() {
        results.push(Value::list(vec![Value::Long(index), Value::Double(dist)]));
        if count > 0 && count as usize == results.len() {
            break;
        }
    }
    stack.push(Value::list(results));
    Ok(())
}
