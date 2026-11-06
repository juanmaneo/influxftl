// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Time transforms of Geo Time Series: `TIMESHIFT`, `TIMESCALE`, `TIMEMODULO`, `TIMECLIP`,
//! `CLIP`, `TIMESPLIT`, `CHUNK`, `CROP`, `MERGE`, `FUSE` and `SHRINK`.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use indexmap::IndexMap;

use super::Entry;
use super::args::{fail, fail_with_cause};
use super::gts::{decoded, element_or_list, gts_or_list_try};
use super::time::parse_iso8601;
use crate::encoder::{EncodedValue, GtsEncoder};
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Gts, Metadata, Type};
use crate::selector::build_selector_ordered;
use crate::stack::Stack;
use crate::value::{Encoder, Value, double_to_long};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("TIMESHIFT", Entry::Stack(timeshift)),
    ("TIMESCALE", Entry::Stack(timescale)),
    ("TIMEMODULO", Entry::Stack(timemodulo)),
    ("TIMECLIP", Entry::Stack(timeclip)),
    ("CLIP", Entry::Stack(clip)),
    ("TIMESPLIT", Entry::Stack(timesplit)),
    ("CHUNK", Entry::Stack(chunk)),
    ("CROP", Entry::Stack(crop)),
    ("MERGE", Entry::Stack(merge)),
    ("FUSE", Entry::Stack(fuse)),
    ("SHRINK", Entry::Stack(shrink)),
];

/// A list of series as a value.
fn series_list(series: Vec<GeoTimeSerie>) -> Value {
    Value::list(
        series
            .into_iter()
            .map(|gts| Value::Gts(gts.shared()))
            .collect(),
    )
}

// -----------------------------------------------------------------------------------------
// TIMESHIFT, TIMESCALE, TIMEMODULO
// -----------------------------------------------------------------------------------------

/// `TIMESHIFT`: a copy of the Geo Time Series (or of each of a list) with every tick, and the
/// last bucket of a bucketized series, shifted by an offset.
fn timeshift(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "TIMESHIFT";
    let Value::Long(delta) = stack.pop()? else {
        return Err(fail(NAME, "expects a long offset as parameter."));
    };
    element_or_list(stack, |element| {
        let Value::Gts(gts) = element else {
            return Err(fail(
                NAME,
                "expects a Geo Time Series or a list thereof under the offset parameter.",
            ));
        };
        // `GTSHelper.timeshift`
        let mut shifted = gts.borrow().clone();
        for tick in &mut shifted.ticks {
            *tick = tick.wrapping_add(delta);
        }
        if shifted.is_bucketized() {
            shifted.lastbucket = shifted.lastbucket.wrapping_add(delta);
        }
        Ok(Value::Gts(shifted.shared()))
    })
}

/// `TIMESCALE`: a copy of the Geo Time Series (or of each of a list) with every tick
/// multiplied by a scale (and truncated).
fn timescale(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "TIMESCALE";
    let scale = match stack.pop()? {
        Value::Long(l) => l as f64,
        Value::Double(d) => d,
        _ => return Err(fail(NAME, "expects a time scale on top of the stack.")),
    };
    element_or_list(stack, |element| {
        let Value::Gts(gts) = element else {
            return Err(fail(
                NAME,
                "expects a Geo Time Series or a list thereof under the scale parameter.",
            ));
        };
        // `GTSHelper.timescale`
        let gts = gts.borrow();
        if gts.is_bucketized() {
            return Err(Error::new(
                "Cannot apply timescale on a bucketized GTS. Unbucketize it first.",
            ));
        }
        let mut scaled = gts.clone();
        for tick in &mut scaled.ticks {
            *tick = double_to_long(*tick as f64 * scale);
        }
        if scaled.sorted && scale < 0.0 {
            scaled.reversed = !scaled.reversed;
        }
        Ok(Value::Gts(scaled.shared()))
    })
}

/// `TIMEMODULO`: split a Geo Time Series by the quotient of its ticks by a modulo, each
/// resulting series (labelled with the quotient) keeping the remainders as ticks.
fn timemodulo(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "TIMEMODULO";
    let Value::String(label) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a quotient label name on top of the stack.",
        ));
    };
    let Value::Long(modulo) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a LONG modulo under the quotient label name.",
        ));
    };
    gts_or_list_try(stack, NAME, |gts| {
        let gts = gts.borrow();
        let mut output: BTreeMap<i64, GeoTimeSerie> = BTreeMap::new();
        for i in 0..gts.size() {
            if modulo == 0 {
                return Err(Error::new("/ by zero"));
            }
            let tick = gts.tick_at(i);
            let quotient = tick.wrapping_div(modulo);
            let remainder = tick.wrapping_rem(modulo);
            let qgts = output.entry(quotient).or_insert_with(|| {
                let mut qgts = GeoTimeSerie::new();
                qgts.metadata = gts.metadata.clone();
                qgts.metadata
                    .labels
                    .insert(label.to_string(), quotient.to_string());
                qgts
            });
            qgts.set_value(
                remainder,
                gts.location_at(i),
                gts.elevation_at(i),
                &gts.value_at(i),
                false,
            );
        }
        Ok(series_list(output.into_values().collect()))
    })
}

// -----------------------------------------------------------------------------------------
// TIMECLIP, CLIP
// -----------------------------------------------------------------------------------------

/// `GTSHelper.timeclip`: the datapoints whose tick lies in `[start, end]`: a sub-series of a
/// sorted series (which loses the bucketization), a scan into an empty clone otherwise.
fn timeclip_gts(gts: &mut GeoTimeSerie, start: i64, end: i64) -> GeoTimeSerie {
    if gts.sorted {
        return gts.subserie(start, end);
    }
    let mut clipped = gts.clone_empty();
    for i in 0..gts.size() {
        let tick = gts.tick_at(i);
        if tick >= start && tick <= end {
            clipped.copy_datapoint(gts, i);
        }
    }
    clipped
}

/// `GTSHelper.timeclip` of an encoder: a new encoder (base 0, same metadata) with the
/// datapoints whose tick lies in `[start, end]`.
fn timeclip_encoder(encoder: &GtsEncoder, start: i64, end: i64) -> Result<GtsEncoder> {
    let mut clipped = GtsEncoder::new(0);
    clipped.metadata = encoder.metadata.clone();
    for (tick, location, elevation, value) in decoded(encoder)? {
        if tick >= start && tick <= end {
            clipped.add_value(tick, location, elevation, &value);
        }
    }
    Ok(clipped)
}

/// An encoder as a value.
fn encoder_value(encoder: GtsEncoder) -> Value {
    Value::Encoder(Rc::new(RefCell::new(encoder)))
}

/// `TIMECLIP`: the datapoints of a Geo Time Series (or of each of a list) within a time
/// frame given as an end and either a duration or an ISO 8601 start.
fn timeclip(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "TIMECLIP";
    let (mut start, iso8601) = match stack.pop()? {
        Value::String(s) => (parse_iso8601(&s)?, true),
        Value::Long(l) => (l, false),
        _ => {
            return Err(fail(
                NAME,
                "expects either an ISO8601 timestamp as the origin timestamp or a duration.",
            ));
        }
    };
    let end = match stack.pop()? {
        Value::String(s) => parse_iso8601(&s)?,
        Value::Long(l) => l,
        _ => {
            return Err(fail(
                NAME,
                "expects either an ISO8601 timestamp or a delta since Unix Epoch as 'now' parameter.",
            ));
        }
    };
    if !iso8601 {
        start = end.wrapping_sub(start).wrapping_add(1);
    }
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => Ok(Value::Gts(
            timeclip_gts(&mut gts.borrow_mut(), start, end).shared(),
        )),
        Value::Encoder(encoder) => Ok(encoder_value(timeclip_encoder(
            &encoder.borrow(),
            start,
            end,
        )?)),
        _ => Err(fail(
            NAME,
            "expects a Geo Time Series, a GTSEncoder or a list thereof under the timeframe definition.",
        )),
    })
}

/// `CLIP`: the list of the clips of a Geo Time Series (or of each of a list) to a list of
/// `[ lower upper ]` tick pairs (reordered in place when reversed).
fn clip(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "CLIP";
    const EXPECTS: &str = "expects a list of limit pairs on top of the stack.";
    let Value::List(limits) = stack.pop()? else {
        return Err(fail(NAME, EXPECTS));
    };
    let mut pairs = Vec::new();
    for limit in limits.borrow().iter() {
        let Value::List(pair) = limit else {
            return Err(fail(NAME, EXPECTS));
        };
        let mut pair = pair.borrow_mut();
        if pair.len() != 2 {
            return Err(fail(NAME, EXPECTS));
        }
        let (Some(lower), Some(upper)) = (pair[0].as_i64(), pair[1].as_i64()) else {
            return Err(fail(NAME, "expects the limits to be numeric."));
        };
        if lower > upper {
            pair.swap(0, 1);
        }
        pairs.push((lower.min(upper), lower.max(upper)));
    }
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => {
            let mut gts = gts.borrow_mut();
            Ok(series_list(
                pairs
                    .iter()
                    .map(|&(lower, upper)| timeclip_gts(&mut gts, lower, upper))
                    .collect(),
            ))
        }
        Value::Encoder(encoder) => {
            let encoder = encoder.borrow();
            Ok(Value::list(
                pairs
                    .iter()
                    .map(|&(lower, upper)| {
                        timeclip_encoder(&encoder, lower, upper).map(encoder_value)
                    })
                    .collect::<Result<_>>()?,
            ))
        }
        _ => Err(fail(
            NAME,
            "expects a GeoTimeSeries, a GTSEncoder or a list thereof under the list of limit.",
        )),
    })
}

// -----------------------------------------------------------------------------------------
// TIMESPLIT
// -----------------------------------------------------------------------------------------

/// `GTSHelper.timesplit`: cut the (sorted in place) series at every gap of at least
/// `quiet_period`, keeping the pieces with at least `min_values` datapoints and labelling
/// them with their sequence number; a series without datapoint or already carrying the label
/// yields a single clone.
fn timesplit_gts(
    gts: &mut GeoTimeSerie,
    quiet_period: i64,
    min_values: i32,
    label: &str,
) -> Vec<GeoTimeSerie> {
    if gts.size() == 0 || gts.metadata.labels.contains_key(label) {
        return vec![gts.clone()];
    }
    gts.sort(false);
    let gts: &GeoTimeSerie = gts;
    let piece = |gtsid: i32| {
        let mut serie = GeoTimeSerie::with_buckets(gts.lastbucket, gts.bucketcount, gts.bucketspan);
        serie.metadata.name.clone_from(&gts.metadata.name);
        serie.metadata.labels.clone_from(&gts.metadata.labels);
        serie
            .metadata
            .labels
            .insert(label.to_owned(), gtsid.to_string());
        serie
            .metadata
            .attributes
            .clone_from(&gts.metadata.attributes);
        serie
    };
    let keep =
        |serie: &GeoTimeSerie| serie.size() > 0 && serie.size() as i64 >= i64::from(min_values);
    let mut series = Vec::new();
    let mut gtsid = 1i32;
    let mut serie = piece(gtsid);
    let mut lasttick = gts.ticks[0];
    for idx in 0..gts.size() {
        if gts.ticks[idx].wrapping_sub(lasttick) >= quiet_period {
            gtsid = gtsid.wrapping_add(1);
            let finished = std::mem::replace(&mut serie, piece(gtsid));
            if keep(&finished) {
                series.push(finished);
            }
        }
        serie.copy_datapoint(gts, idx);
        lasttick = gts.ticks[idx];
    }
    if keep(&serie) {
        series.push(serie);
    }
    series
}

/// `TIMESPLIT`: split a Geo Time Series (or each of a list) at its quiet periods.
fn timesplit(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "TIMESPLIT";
    let Value::String(label) = stack.pop()? else {
        return Err(fail(NAME, "expects a label on top of the stack."));
    };
    let Value::Long(min_values) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a minimum number of values under the label.",
        ));
    };
    let Value::Long(quiet_period) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a quiet period under the minimum number of values.",
        ));
    };
    element_or_list(stack, |element| {
        let Value::Gts(gts) = element else {
            return Err(fail(
                NAME,
                "expects a Geo Time Series or a list thereof under the label, minimum values and the quiet period parameters.",
            ));
        };
        Ok(series_list(timesplit_gts(
            &mut gts.borrow_mut(),
            quiet_period,
            min_values as i32,
            &label,
        )))
    })
}

// -----------------------------------------------------------------------------------------
// CHUNK
// -----------------------------------------------------------------------------------------

/// The parameters of `CHUNK`.
#[derive(Clone, Copy)]
struct Chunking<'a> {
    lastchunk: i64,
    chunkwidth: i64,
    chunkcount: i64,
    chunklabel: &'a str,
    keepempty: bool,
    overlap: i64,
}

/// `GTSHelper.chunk`: cut the (reverse sorted in place) series into chunks of `chunkwidth`
/// ending at `lastchunk`, `lastchunk - chunkwidth`…, each labelled with its end tick;
/// `chunkcount` `0` means as many as needed, `overlap` extends every chunk on both sides.
fn chunk_gts(gts: &mut GeoTimeSerie, params: &Chunking) -> Result<Vec<GeoTimeSerie>> {
    let Chunking {
        mut lastchunk,
        chunkwidth,
        mut chunkcount,
        chunklabel,
        keepempty,
        overlap,
    } = *params;
    if overlap < 0 || overlap > chunkwidth {
        return Err(Error::new("Overlap cannot exceed chunk width."));
    }
    if gts.metadata.labels.contains_key(chunklabel) {
        return Err(Error::new(format!(
            "Cannot operate on Geo Time Series which already have a label named '{chunklabel}'"
        )));
    }
    let bucketized = gts.is_bucketized();
    if bucketized {
        if gts.bucketspan > chunkwidth {
            return Err(Error::new(
                "Cannot operate on Geo Time Series with a bucketspan greater than the chunk width.",
            ));
        }
    } else if gts.size() == 0 && lastchunk == 0 {
        return Ok(Vec::new());
    }
    // Warp 10 divides by the chunk width along most of its paths.
    if chunkwidth == 0 {
        return Err(Error::new("/ by zero"));
    }
    let zero_chunk_count = chunkcount == 0;
    if zero_chunk_count {
        chunkcount = i64::from(i32::MAX);
    }
    gts.sort(true);
    let gts: &GeoTimeSerie = gts;
    let n = gts.size();
    let (bucketspan, bucketcount, lastbucket) = (gts.bucketspan, gts.bucketcount, gts.lastbucket);
    if lastchunk == 0 {
        if bucketized {
            lastchunk = lastbucket;
        } else {
            // The most recent tick, aligned on the next chunk boundary.
            lastchunk = gts.ticks[0];
            if lastchunk.wrapping_rem(chunkwidth) != 0 {
                lastchunk = lastchunk
                    .wrapping_sub(lastchunk.wrapping_rem(chunkwidth))
                    .wrapping_add(chunkwidth);
            }
        }
    }
    if !keepempty {
        // A non bucketized series without datapoint has no last tick: Warp 10 overflows
        // here, no chunk can be produced anyway.
        if n == 0 && !bucketized {
            return Ok(Vec::new());
        }
        let lasttick = gts.last_tick();
        if lastchunk > lasttick {
            let empty_chunks = lastchunk.wrapping_sub(lasttick).wrapping_div(chunkwidth);
            if !zero_chunk_count {
                chunkcount = chunkcount.wrapping_sub(empty_chunks);
            }
            lastchunk = lastchunk.wrapping_sub(chunkwidth.wrapping_mul(empty_chunks));
        }
    }
    if overlap > 0 {
        chunkcount = chunkcount.wrapping_add(2);
        lastchunk = lastchunk.wrapping_add(chunkwidth);
    }
    let mut chunks: BTreeMap<i64, GeoTimeSerie> = BTreeMap::new();
    let mut add = |chunkend: i64, mut chunkgts: GeoTimeSerie| {
        chunkgts.metadata = gts.metadata.clone();
        chunkgts
            .metadata
            .labels
            .insert(chunklabel.to_owned(), chunkend.to_string());
        chunks.insert(chunkend, chunkgts);
    };
    let mut idx = 0;
    let mut i = 0i64;
    while i < chunkcount {
        if idx >= n && zero_chunk_count {
            break;
        }
        let chunkend = lastchunk.wrapping_sub(i.wrapping_mul(chunkwidth));
        let chunkstart = chunkend.wrapping_sub(chunkwidth).wrapping_add(1);
        i += 1;
        let mut chunkgts = GeoTimeSerie::with_buckets(lastbucket, bucketcount, bucketspan);
        if bucketized {
            // A chunk outside the buckets is empty.
            if lastbucket < chunkstart
                || chunkend <= lastbucket.wrapping_sub(bucketcount.wrapping_mul(bucketspan))
            {
                if keepempty || overlap > 0 {
                    add(chunkend, chunkgts);
                }
                continue;
            }
            // Chunks are bucketized only when the bucket span divides the chunk width.
            if chunkwidth % bucketspan == 0 {
                chunkgts.bucketspan = bucketspan;
                chunkgts.lastbucket = chunkend;
                chunkgts.bucketcount = i64::from(
                    (chunkend.wrapping_sub(chunkstart).wrapping_add(1) / bucketspan) as i32,
                );
            } else {
                chunkgts.bucketspan = 0;
                chunkgts.lastbucket = 0;
            }
        }
        // Skip the datapoints more recent than the chunk.
        while idx < n && gts.ticks[idx] > chunkend {
            idx += 1;
        }
        // No datapoint left, or the next one is before the chunk: the chunk is empty.
        if idx >= n || gts.ticks[idx] < chunkstart {
            if keepempty || overlap > 0 {
                add(chunkend, chunkgts);
            }
            continue;
        }
        while idx < n && gts.ticks[idx] >= chunkstart {
            chunkgts.copy_datapoint(gts, idx);
            idx += 1;
        }
        // At least one datapoint was copied: the chunk is always kept.
        add(chunkend, chunkgts);
    }
    // Oldest chunk first, like the `TreeMap` of Warp 10.
    let mut entries: Vec<(i64, GeoTimeSerie)> = chunks.into_iter().collect();
    if overlap > 0 {
        let sizes: Vec<usize> = entries.iter().map(|(_, chunk)| chunk.size()).collect();
        for ci in 0..entries.len() {
            let key = entries[ci].0;
            let lower_bound = key
                .wrapping_sub(chunkwidth)
                .wrapping_add(1)
                .wrapping_sub(overlap);
            let upper_bound = key.wrapping_add(overlap);
            let mut extra = Vec::new();
            if ci > 0 {
                // The most recent datapoints of the previous (older) chunk, in reverse order.
                let prev = &entries[ci - 1].1;
                for j in 0..sizes[ci - 1] {
                    if prev.tick_at(j) < lower_bound {
                        break;
                    }
                    extra.push((
                        prev.tick_at(j),
                        prev.location_at(j),
                        prev.elevation_at(j),
                        prev.value_at(j),
                    ));
                }
            }
            if ci < entries.len() - 1 {
                // The oldest datapoints of the next (newer) chunk.
                let next = &entries[ci + 1].1;
                for j in (0..sizes[ci + 1]).rev() {
                    if next.tick_at(j) > upper_bound {
                        break;
                    }
                    extra.push((
                        next.tick_at(j),
                        next.location_at(j),
                        next.elevation_at(j),
                        next.value_at(j),
                    ));
                }
            }
            let current = &mut entries[ci].1;
            for (tick, location, elevation, value) in &extra {
                current.set_value(*tick, *location, *elevation, value, false);
            }
        }
        // The extra chunks computed on both ends for the overlaps.
        let oldest = lastchunk.wrapping_sub(chunkcount.wrapping_sub(1).wrapping_mul(chunkwidth));
        entries.retain(|(key, _)| *key != lastchunk && *key != oldest);
    }
    Ok(entries
        .into_iter()
        .filter_map(|(_, chunk)| (keepempty || chunk.size() != 0).then_some(chunk))
        .collect())
}

/// `GTSHelper.chunk` of an encoder: each datapoint goes to the chunk ending at the first
/// `lastchunk + k × chunkwidth` at or after its tick (and to the adjacent chunks it overlaps),
/// then the chunks from the most recent one down are kept according to `chunkcount` and
/// `keepempty`, and returned oldest first.
fn chunk_encoder(encoder: &GtsEncoder, params: &Chunking) -> Result<Vec<GtsEncoder>> {
    let Chunking {
        lastchunk,
        chunkwidth,
        mut chunkcount,
        chunklabel,
        keepempty,
        overlap,
    } = *params;
    if overlap < 0 || overlap > chunkwidth {
        return Err(Error::new("Overlap cannot exceed chunk width."));
    }
    if encoder.metadata.labels.contains_key(chunklabel) {
        return Err(Error::new(format!(
            "Cannot operate on encoders which already have a label named '{chunklabel}'"
        )));
    }
    if encoder.count() == 0 && encoder.size() == 0 && lastchunk == 0 {
        return Ok(Vec::new());
    }
    // As for series, a zero width is refused where Warp 10 divides by it (or loops forever).
    if chunkwidth == 0 {
        return Err(Error::new("/ by zero"));
    }
    let zero_chunk_count = chunkcount == 0;
    if zero_chunk_count {
        chunkcount = i64::from(i32::MAX);
    }
    let new_chunk = |chunkid: i64| {
        let mut chunk = GtsEncoder::new(0);
        chunk.metadata = encoder.metadata.clone();
        chunk
            .metadata
            .labels
            .insert(chunklabel.to_owned(), chunkid.to_string());
        chunk
    };
    let mut chunks: HashMap<i64, GtsEncoder> = HashMap::new();
    let mut oldest = i64::MAX;
    let mut newest = i64::MIN;
    let mut decoder = encoder.decoder();
    while decoder.next()? {
        let tick = decoder.timestamp();
        let (location, elevation, value) =
            (decoder.location(), decoder.elevation(), decoder.value());
        let mut delta = tick.wrapping_sub(lastchunk);
        if delta < 0 {
            delta = delta.wrapping_add(delta.wrapping_neg().wrapping_rem(chunkwidth));
        } else if delta.wrapping_rem(chunkwidth) != 0 {
            delta = delta
                .wrapping_sub(delta.wrapping_rem(chunkwidth))
                .wrapping_add(chunkwidth);
        }
        let chunkid = lastchunk.wrapping_add(delta);
        chunks
            .entry(chunkid)
            .or_insert_with(|| new_chunk(chunkid))
            .add_value(tick, location, elevation, &value);
        if overlap > 0 {
            if tick >= chunkid.wrapping_add(1).wrapping_sub(overlap) {
                let next = chunkid.wrapping_add(chunkwidth);
                chunks
                    .entry(next)
                    .or_insert_with(|| {
                        newest = newest.max(next);
                        new_chunk(next)
                    })
                    .add_value(tick, location, elevation, &value);
            }
            if tick <= chunkid.wrapping_sub(chunkwidth).wrapping_add(overlap) {
                let previous = chunkid.wrapping_sub(chunkwidth);
                chunks
                    .entry(previous)
                    .or_insert_with(|| {
                        oldest = oldest.min(previous);
                        new_chunk(previous)
                    })
                    .add_value(tick, location, elevation, &value);
            }
        }
        oldest = oldest.min(chunkid);
        newest = newest.max(chunkid);
    }
    let firstchunkid = if zero_chunk_count {
        oldest
    } else {
        lastchunk.wrapping_sub(chunkcount.wrapping_sub(1).wrapping_mul(chunkwidth))
    };
    let mut lastchunkid = if lastchunk == 0 { newest } else { lastchunk };
    if !keepempty && lastchunkid > newest {
        if !zero_chunk_count {
            chunkcount = chunkcount.wrapping_sub(lastchunkid.wrapping_sub(newest) / chunkwidth);
        }
        lastchunkid = newest;
    }
    let mut kept = Vec::new();
    let mut chunkid = lastchunkid;
    while chunkid >= firstchunkid
        && (zero_chunk_count || lastchunkid.wrapping_sub(chunkid) / chunkwidth < chunkcount)
    {
        if let Some(chunk) = chunks.remove(&chunkid) {
            kept.push(chunk);
        } else if keepempty {
            kept.push(new_chunk(chunkid));
        }
        let Some(previous) = chunkid.checked_sub(chunkwidth) else {
            break;
        };
        chunkid = previous;
    }
    kept.reverse();
    Ok(kept)
}

/// `CHUNK`: cut a Geo Time Series (or each of a list) into chunks:
/// `lastchunk chunkwidth overlap chunkcount label keepempty CHUNK`.
fn chunk(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "CHUNK";
    let Value::Boolean(keepempty) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects on top of the stack a boolean indicating whether or not to keep empty chunks.",
        ));
    };
    let Value::String(chunklabel) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects the name of the 'chunk' label below the top of the stack.",
        ));
    };
    let Value::Long(chunkcount) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a number of chunks under the 'chunk' label.",
        ));
    };
    let Value::Long(overlap) = stack.pop()? else {
        return Err(fail(NAME, "expects an overlap below the number of chunks."));
    };
    let Value::Long(chunkwidth) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a chunk width on top of the end timestamp of the most recent chunk.",
        ));
    };
    let Value::Long(lastchunk) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects the end timestamp of the most recent chunk under the chunk width.",
        ));
    };
    let params = Chunking {
        lastchunk,
        chunkwidth,
        chunkcount,
        chunklabel: &chunklabel,
        keepempty,
        overlap: overlap.max(0),
    };
    element_or_list(stack, |element| match element {
        Value::Gts(gts) => Ok(series_list(chunk_gts(&mut gts.borrow_mut(), &params)?)),
        Value::Encoder(encoder) => Ok(Value::list(
            chunk_encoder(&encoder.borrow(), &params)?
                .into_iter()
                .map(encoder_value)
                .collect(),
        )),
        _ => Err(fail(
            NAME,
            "expects a Geo Time Series, a GTSEncoder or a list thereof under the lastchunk parameter.",
        )),
    })
}

// -----------------------------------------------------------------------------------------
// CROP
// -----------------------------------------------------------------------------------------

/// `GTSHelper.crop`: a bucketized series (sorted in place) reduced to the datapoints on its
/// bucket boundaries, re-bucketized on the span they cover; a clone otherwise.
fn crop_gts(gts: &mut GeoTimeSerie) -> Result<GeoTimeSerie> {
    if !gts.is_bucketized() {
        return Ok(gts.clone());
    }
    gts.sort(false);
    let firstbucket = gts
        .lastbucket
        .wrapping_sub(gts.bucketcount.wrapping_sub(1).wrapping_mul(gts.bucketspan));
    let mut cropped = GeoTimeSerie::new();
    cropped.metadata = gts.metadata.clone();
    for i in 0..gts.size() {
        let tick = gts.ticks[i];
        if tick >= firstbucket
            && tick <= gts.lastbucket
            && tick.wrapping_rem(gts.bucketspan) == gts.lastbucket.wrapping_rem(gts.bucketspan)
        {
            cropped.copy_datapoint(gts, i);
        }
    }
    // Warp 10 reads the last tick of the (unallocated) result.
    let Some(&last) = cropped.ticks.last() else {
        return Err(Error::new(
            "Cannot load from long array because \"cropped.ticks\" is null",
        ));
    };
    cropped.bucketspan = gts.bucketspan;
    cropped.lastbucket = last;
    cropped.bucketcount = i64::from(
        1i32.wrapping_add((last.wrapping_sub(cropped.ticks[0]) / cropped.bucketspan) as i32),
    );
    Ok(cropped)
}

/// `CROP`: crop a Geo Time Series, or every series of a list (which may nest lists of
/// series; other elements are ignored).
fn crop(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "CROP";
    const EXPECTS: &str = "expects a LIST or a Geo Time Series as input.";
    match stack.pop()? {
        Value::List(params) => {
            let mut series: Vec<Gts> = Vec::new();
            for param in params.borrow().iter() {
                match param {
                    Value::Gts(gts) => series.push(gts.clone()),
                    Value::List(list) => {
                        for element in list.borrow().iter() {
                            let Value::Gts(gts) = element else {
                                return Err(fail(NAME, EXPECTS));
                            };
                            series.push(gts.clone());
                        }
                    }
                    _ => {}
                }
            }
            let mut result = Vec::with_capacity(series.len());
            for gts in &series {
                result.push(crop_gts(&mut gts.borrow_mut())?);
            }
            stack.push(series_list(result));
        }
        Value::Gts(gts) => {
            let cropped = crop_gts(&mut gts.borrow_mut())?;
            stack.push(Value::Gts(cropped.shared()));
        }
        _ => return Err(fail(NAME, EXPECTS)),
    }
    Ok(())
}

// -----------------------------------------------------------------------------------------
// MERGE, FUSE
// -----------------------------------------------------------------------------------------

/// `MERGE`: a single Geo Time Series with the datapoints of every series of a list (which may
/// nest lists of series; other elements are ignored), carrying the metadata of the first one.
fn merge(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MERGE";
    let mut top = stack.pop()?;
    let mut reversed = None;
    if let Value::Boolean(b) = top {
        reversed = Some(b);
        top = stack.pop()?;
    }
    let Value::List(params) = top else {
        return Err(fail(NAME, "operates on a LIST containing GTS or ENCODERs."));
    };
    if let Some(reversed) = reversed {
        // Runtime exceptions (corrupt encoders) escape unwrapped.
        let merged = sorted_merge(&params.borrow(), reversed).map_err(|e| {
            if e.is_runtime() {
                e
            } else {
                fail_with_cause(NAME, "encountered an error while merging.", e)
            }
        })?;
        stack.push(encoder_value(merged));
        return Ok(());
    }
    let mut series: Vec<Gts> = Vec::new();
    let mut encoders: Vec<Encoder> = Vec::new();
    for param in params.borrow().iter() {
        match param {
            Value::Gts(gts) => series.push(gts.clone()),
            Value::Encoder(encoder) => encoders.push(encoder.clone()),
            Value::List(list) => {
                for element in list.borrow().iter() {
                    match element {
                        Value::Gts(gts) => series.push(gts.clone()),
                        Value::Encoder(encoder) => encoders.push(encoder.clone()),
                        _ => {
                            return Err(fail(
                                NAME,
                                "expects a list of Geo Time Series or encoders as first parameter.",
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(first) = encoders.first() {
        if !series.is_empty() {
            return Err(fail(
                NAME,
                "can only operate on homogeneous lists of Geo Time Series or encoders.",
            ));
        }
        let mut merged = GtsEncoder::new(0);
        merged.metadata = first.borrow().metadata.clone();
        for encoder in &encoders {
            merged.merge(&encoder.borrow())?;
        }
        stack.push(encoder_value(merged));
        return Ok(());
    }
    // `GTSHelper.mergeViaEncoders`: every datapoint goes through an encoder carrying the
    // metadata of the first series, then is decoded into a new series.
    let mut merged = GeoTimeSerie::new();
    if let Some(first) = series.first() {
        merged.metadata = first.borrow().metadata.clone();
    }
    for gts in &series {
        let gts = gts.borrow();
        for i in 0..gts.size() {
            merged.copy_datapoint(&gts, i);
        }
    }
    stack.push(Value::Gts(merged.shared()));
    Ok(())
}

/// A source of `sortedMerge`: the datapoints of an encoder (with its metadata, to report it
/// unsorted) or of a series, and the index of the current one.
struct MergeSource {
    points: Vec<(i64, i64, i64, EncodedValue)>,
    encoder: Option<Metadata>,
    idx: usize,
}

/// `GTSHelper.sortedMerge`: the datapoints of encoders and of series sorted in the requested
/// order, in a single encoder, merged through a `java.util.PriorityQueue` of the sources keyed
/// by their current tick (so that the sources sharing a tick are visited in the same order).
fn sorted_merge(params: &[Value], reversed: bool) -> Result<GtsEncoder> {
    let mut encoders: Vec<Encoder> = Vec::new();
    let mut series: Vec<Gts> = Vec::new();
    let mut add = |value: &Value, nested: bool| -> Result<()> {
        match value {
            Value::Gts(gts) => {
                let g = gts.borrow();
                if g.size() > 1 {
                    if !g.sorted {
                        return Err(Error::new(format!(
                            "GTS {} is not sorted.",
                            build_selector_ordered(&g.metadata, false)
                        )));
                    }
                    if g.reversed != reversed {
                        return Err(Error::new(format!(
                            "GTS {} is not sorted in the expected order.",
                            build_selector_ordered(&g.metadata, false)
                        )));
                    }
                }
                series.push(gts.clone());
            }
            Value::Encoder(encoder) => encoders.push(encoder.clone()),
            _ if nested => {
                return Err(Error::new(
                    "expects a list of Geo Time Series or encoders or of lists thereof.",
                ));
            }
            _ => {}
        }
        Ok(())
    };
    for param in params {
        match param {
            Value::List(list) => {
                for element in list.borrow().iter() {
                    add(element, true)?;
                }
            }
            other => add(other, false)?,
        }
    }
    let mut merged = GtsEncoder::new(0);
    if let Some(first) = encoders.first() {
        merged.metadata = first.borrow().metadata.clone();
    } else if let Some(first) = series.first() {
        merged.metadata = first.borrow().metadata.clone();
    }
    let mut sources = Vec::new();
    for encoder in &encoders {
        let encoder = encoder.borrow();
        let points = decoded(&encoder)?;
        if !points.is_empty() {
            sources.push(MergeSource {
                points,
                encoder: Some(encoder.metadata.clone()),
                idx: 0,
            });
        }
    }
    for gts in &series {
        let gts = gts.borrow();
        if gts.size() == 0 {
            continue;
        }
        // Series values always convert.
        let points = (0..gts.size())
            .map(|i| {
                (
                    gts.tick_at(i),
                    gts.location_at(i),
                    gts.elevation_at(i),
                    EncodedValue::from_value(&gts.value_at(i)).unwrap_or(EncodedValue::Null),
                )
            })
            .collect();
        sources.push(MergeSource {
            points,
            encoder: None,
            idx: 0,
        });
    }
    let tick = |sources: &[MergeSource], s: usize| sources[s].points[sources[s].idx].0;
    let order = |a: i64, b: i64| if reversed { b.cmp(&a) } else { a.cmp(&b) };
    let mut queue = JavaQueue::default();
    for s in 0..sources.len() {
        queue.add(s, |a, b| order(tick(&sources, a), tick(&sources, b)));
    }
    while let Some(&first) = queue.heap.first() {
        let last_tick = tick(&sources, first);
        while let Some(s) = queue.poll(|a, b| order(tick(&sources, a), tick(&sources, b))) {
            if tick(&sources, s) != last_tick {
                queue.add(s, |a, b| order(tick(&sources, a), tick(&sources, b)));
                break;
            }
            let source = &mut sources[s];
            let mut exhausted = false;
            while let Some((t, location, elevation, value)) = source.points.get(source.idx) {
                if *t != last_tick {
                    break;
                }
                merged.add_value(*t, *location, *elevation, value);
                source.idx += 1;
                match (source.points.get(source.idx), &source.encoder) {
                    (None, _) => exhausted = true,
                    (Some(next), Some(metadata))
                        if (reversed && next.0 > last_tick)
                            || (!reversed && next.0 < last_tick) =>
                    {
                        return Err(Error::new(format!(
                            "ENCODER {} is not sorted in expected order.",
                            build_selector_ordered(metadata, false)
                        )));
                    }
                    _ => {}
                }
            }
            if !exhausted {
                queue.add(s, |a, b| order(tick(&sources, a), tick(&sources, b)));
            }
        }
    }
    Ok(merged)
}

/// A `java.util.PriorityQueue` of source indexes (`siftUp` / `siftDown` with the given
/// comparator), whose layout decides the order of the sources sharing a tick.
#[derive(Default)]
struct JavaQueue {
    heap: Vec<usize>,
}

impl JavaQueue {
    fn add(&mut self, entry: usize, cmp: impl Fn(usize, usize) -> Ordering) {
        let mut k = self.heap.len();
        self.heap.push(entry);
        while k > 0 {
            let parent = (k - 1) >> 1;
            if cmp(entry, self.heap[parent]).is_ge() {
                break;
            }
            self.heap[k] = self.heap[parent];
            k = parent;
        }
        self.heap[k] = entry;
    }

    fn poll(&mut self, cmp: impl Fn(usize, usize) -> Ordering) -> Option<usize> {
        let last = self.heap.pop()?;
        let n = self.heap.len();
        if n == 0 {
            return Some(last);
        }
        let result = self.heap[0];
        let mut k = 0;
        while k < n >> 1 {
            let mut child = (k << 1) + 1;
            if child + 1 < n && cmp(self.heap[child], self.heap[child + 1]).is_gt() {
                child += 1;
            }
            if cmp(last, self.heap[child]).is_le() {
                break;
            }
            self.heap[k] = self.heap[child];
            k = child;
        }
        self.heap[k] = last;
        Some(result)
    }
}

/// `GTSHelper.fuse`: the datapoints of all the chunks in a single series, bucketized when the
/// chunks are compatibly bucketized, named when all the chunks share a name and keeping the
/// labels whose value no chunk contradicts.
fn fuse_gts(chunks: &[Gts]) -> Result<GeoTimeSerie> {
    if chunks.is_empty() {
        return Ok(GeoTimeSerie::new());
    }
    let mut kind: Option<Type> = None;
    for chunk in chunks {
        let chunk = chunk.borrow();
        match kind {
            None => {
                if chunk.type_of() != Type::Undefined {
                    kind = Some(chunk.type_of());
                }
            }
            Some(kind) => {
                if chunk.size() != 0 && kind != chunk.type_of() {
                    return Err(Error::new("Inconsistent types for chunks to fuse."));
                }
            }
        }
    }
    let mut lastbucket = i64::MIN;
    let mut bucketspan = 0i64;
    let mut firstbucket = i64::MAX;
    for chunk in chunks {
        let chunk = chunk.borrow();
        if !chunk.is_bucketized() {
            bucketspan = 0;
            break;
        }
        let first = chunk.lastbucket.wrapping_sub(
            chunk
                .bucketcount
                .wrapping_sub(1)
                .wrapping_mul(chunk.bucketspan),
        );
        if bucketspan == 0 {
            bucketspan = chunk.bucketspan;
            lastbucket = chunk.lastbucket;
            firstbucket = first;
        } else {
            if bucketspan != chunk.bucketspan
                || lastbucket.wrapping_rem(bucketspan) != chunk.lastbucket.wrapping_rem(bucketspan)
            {
                bucketspan = 0;
                break;
            }
            lastbucket = lastbucket.max(chunk.lastbucket);
            firstbucket = firstbucket.min(first);
        }
    }
    let mut bucketcount = 0i64;
    if bucketspan == 0 {
        lastbucket = 0;
    } else {
        bucketcount = i64::from((1 + lastbucket.wrapping_sub(firstbucket) / bucketspan) as i32);
    }
    let mut fused = GeoTimeSerie::with_buckets(lastbucket, bucketcount, bucketspan);
    let mut classname: Option<String> = None;
    let mut has_class = false;
    let mut labels: Option<IndexMap<String, String>> = None;
    for chunk in chunks {
        let chunk = chunk.borrow();
        if classname.is_none() {
            classname.clone_from(&chunk.metadata.name);
            has_class = true;
        } else if classname != chunk.metadata.name {
            has_class = false;
        }
        match &mut labels {
            None => labels = Some(chunk.metadata.labels.clone()),
            Some(labels) => {
                for (name, value) in &chunk.metadata.labels {
                    if labels.get(name) != Some(value) {
                        labels.shift_remove(name);
                    }
                }
            }
        }
        for i in 0..chunk.size() {
            fused.copy_datapoint(&chunk, i);
        }
    }
    if has_class {
        fused.metadata.name = classname;
    }
    fused.metadata.labels = labels.unwrap_or_default();
    Ok(fused)
}

/// `FUSE`: fuse a list of Geo Time Series (chunks) into one.
fn fuse(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "FUSE";
    const EXPECTS: &str = "expects a list of GTS as input.";
    let Value::List(list) = stack.pop()? else {
        return Err(fail(NAME, EXPECTS));
    };
    let chunks = list
        .borrow()
        .iter()
        .map(|element| match element {
            Value::Gts(gts) => Ok(gts.clone()),
            _ => Err(fail(NAME, EXPECTS)),
        })
        .collect::<Result<Vec<_>>>()?;
    stack.push(Value::Gts(fuse_gts(&chunks)?.shared()));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// SHRINK
// -----------------------------------------------------------------------------------------

/// The recursive step of `SHRINK`: a Geo Time Series is shrunk in place, a list is
/// processed element by element.
fn shrink_value(value: &Value, shrinkto: i64) -> Result<Value> {
    match value {
        Value::Gts(gts) => {
            let mut serie = gts.borrow_mut();
            if shrinkto > i64::from(i32::MIN) && serie.size() as i64 > shrinkto.abs() {
                serie.sort(shrinkto < 0);
                // `GTSHelper.shrinkTo`: the size is below the number of datapoints.
                serie.keep_range(0, shrinkto.unsigned_abs() as usize);
            }
            Ok(value.clone())
        }
        Value::List(list) => {
            let items = list.borrow().clone();
            let mut result = Vec::with_capacity(items.len());
            for item in &items {
                result.push(shrink_value(item, shrinkto)?);
            }
            Ok(Value::list(result))
        }
        _ => Err(fail("SHRINK", "can only handle GTS and list thereof.")),
    }
}

/// `SHRINK`: keep the first `n` (or, for a negative `n`, the last `-n`) datapoints of a Geo
/// Time Series or of every series of a (nested) list, sorting them first.
fn shrink(stack: &mut Stack) -> Result<()> {
    let Value::Long(shrinkto) = stack.pop()? else {
        return Err(fail("SHRINK", "expects a size on top of the stack."));
    };
    let value = stack.pop()?;
    let result = shrink_value(&value, shrinkto)?;
    stack.push(result);
    Ok(())
}
