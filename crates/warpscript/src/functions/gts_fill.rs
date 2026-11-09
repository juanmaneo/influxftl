// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Filling the gaps of Geo Time Series: `FILL` (driving the fillers of [`crate::fillers`]),
//! `FILLVALUE`, `FILLPREVIOUS`, `FILLNEXT`, `FILLTICKS` and `INTERPOLATE` — the `fill*`
//! methods of `GTSHelper` and `INTERPOLATE.interpolate`.
//!
//! Like Warp 10, the fills read the series they fill live: a macro filler, which adds its
//! datapoints to the original series rather than to the filled copy, makes the bucket and
//! cross fills run until a limit of the stack stops them.

use super::Entry;
use super::gts::{gts_or_list, gts_or_list_try};
use crate::aggregators::double_value;
use crate::error::{Error, Result};
use crate::fillers::{FillerRef, Roles, Single, Slot, Window, both};
use crate::geoxp::{from_geoxp_point, to_geoxp_point};
use crate::gts::{
    GeoTimeSerie, Gts, Metadata, NO_ELEVATION, NO_LOCATION, java_binary_search,
    java_binary_search_range,
};
use crate::stack::Stack;
use crate::value::{List, Map, Value, ValueKind};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("FILL", Entry::Stack(fill)),
    ("FILLVALUE", Entry::Stack(fillvalue)),
    ("FILLPREVIOUS", Entry::Stack(fillprevious)),
    ("FILLNEXT", Entry::Stack(fillnext)),
    ("FILLTICKS", Entry::Stack(fillticks)),
    ("INTERPOLATE", Entry::Stack(interpolate)),
];

// -----------------------------------------------------------------------------------------
// FILL
// -----------------------------------------------------------------------------------------

/// `FILL`: `a b filler FILL` fills each series with the ticks of the other (cross fill),
/// `gts|[gts…] { 'filler' … } FILL` and `[ gts|[gts…] filler [ticks] ] FILL` fill the empty
/// buckets or the given ticks of each series (univariate fill).
fn fill(stack: &mut Stack) -> Result<()> {
    let top = stack.pop()?;
    if let Some(filler) = FillerRef::of(&top) {
        return cross_fill(stack, &filler);
    }
    match top {
        Value::Map(map) => fill_from_map(stack, &map),
        Value::List(list) => {
            let params = list.borrow().clone();
            fill_from_list(stack, &params)
        }
        other => {
            stack.push(other);
            Err(Error::new(
                "FILL's argument types do not match the expected ones.",
            ))
        }
    }
}

/// The parameters of a univariate fill.
struct Params<'a> {
    filler: FillerRef<'a>,
    ticks: Option<Vec<i64>>,
    verify: bool,
    invalid: Value,
    force: bool,
}

/// The error messages of a form of univariate fill, and whether its series were on the stack.
struct Form {
    list_element: &'static str,
    not_series: &'static str,
    on_stack: bool,
}

const MAP_FORM: Form = Form {
    list_element: "FILL expects a LIST of GTS, but instead the list contains a",
    not_series: "FILLexpects a GTS or a LIST of GTS before the MAP of parameters",
    on_stack: true,
};

const LIST_FORM: Form = Form {
    list_element: "FILL expects a LIST of GTS as first parameter in the input LIST, but instead the list contains a",
    not_series: "FILLexpects the first parameter of the input LIST to be a GTS or a LIST of GTS",
    on_stack: false,
};

/// `FILL.applyFromMap`: `gts|[gts…] { 'filler' 'ticks' 'verify' 'invalid.value' } FILL`.
fn fill_from_map(stack: &mut Stack, map: &Map) -> Result<()> {
    let get = |key: &str| {
        map.borrow()
            .get(&Value::from(key))
            .cloned()
            .unwrap_or(Value::Null)
    };
    let verify = match get("verify") {
        Value::Null => true,
        Value::Boolean(verify) => verify,
        other => {
            return Err(Error::new(format!(
                "FILL expects parameter verify to be a BOOLEAN, but instead got a {}",
                other.type_name()
            )));
        }
    };
    let ticks = match get("ticks") {
        Value::Null => None,
        Value::List(list) => tick_list(
            &list,
            verify,
            "FILL expects parameter ticks to be a LIST of LONG, but it contains a",
        )?,
        other => {
            return Err(Error::new(format!(
                "FILL expects parameter ticks to be a LIST, but instead got a {}",
                other.type_name()
            )));
        }
    };
    let filler = get("filler");
    let Some(filler) = FillerRef::of(&filler) else {
        return Err(Error::new(format!(
            "FILL expects parameter filler to be a filler or a macro, but instead got a {}",
            filler.type_name()
        )));
    };
    let params = Params {
        filler,
        ticks,
        verify,
        invalid: get("invalid.value"),
        // Undocumented flag forcing the windowed role of the fillers that have both.
        force: matches!(get("force.old.filler"), Value::Boolean(true)),
    };
    let target = stack.pop()?;
    let results = fill_target(stack, target, &params, &MAP_FORM)?;
    stack.push(Value::list(results));
    Ok(())
}

/// `FILL.applyFromList`: `[ gts|[gts…] filler [ticks] ] FILL`.
fn fill_from_list(stack: &mut Stack, params: &[Value]) -> Result<()> {
    if params.len() < 2 {
        return Err(Error::new(format!(
            "FILL expects an input LIST containing at least two parameters, but got only {}",
            params.len()
        )));
    }
    if params.len() > 3 {
        return Err(Error::new(format!(
            "FILL expects an input LIST containing at most three parameters, but got {}",
            params.len()
        )));
    }
    let Some(filler) = FillerRef::of(&params[1]) else {
        return Err(Error::new(format!(
            "FILL expects the second parameter of the input LIST to be a filler or a macro, but instead got a {}",
            params[1].type_name()
        )));
    };
    let ticks = match params.get(2) {
        None | Some(Value::Null) => None,
        Some(Value::List(list)) => tick_list(
            list,
            true,
            "FILL expects the last parameter of the input LIST to be a LIST of LONG, but it contains a",
        )?,
        Some(_) => {
            return Err(Error::new(
                "FILLexpects the last parameter of the input LIST to be a LIST",
            ));
        }
    };
    let fill_params = Params {
        filler,
        ticks,
        verify: true,
        invalid: Value::Null,
        force: false,
    };
    let results = fill_target(stack, params[0].clone(), &fill_params, &LIST_FORM)?;
    stack.push(Value::list(results));
    Ok(())
}

/// The ticks of a `ticks` parameter: `None` when empty, sorted and deduplicated (`sort`)
/// like `FILL.sortDedupTicks`.
fn tick_list(list: &List, sort: bool, not_long: &str) -> Result<Option<Vec<i64>>> {
    let ticks = {
        let elements = list.borrow();
        if elements.is_empty() {
            return Ok(None);
        }
        let mut ticks = Vec::with_capacity(elements.len());
        for element in elements.iter() {
            let Value::Long(tick) = element else {
                return Err(Error::new(format!("{not_long} {}", element.type_name())));
            };
            ticks.push(*tick);
        }
        ticks
    };
    Ok(Some(if sort { sort_dedup(list, ticks) } else { ticks }))
}

/// `FILL.sortDedupTicks`: sort the ticks (and the list they come from) when they are not
/// sorted, then drop the duplicates — and, like Warp 10, the first occurrence of the first
/// duplicated tick.
fn sort_dedup(list: &List, mut ticks: Vec<i64>) -> Vec<i64> {
    if ticks.windows(2).any(|pair| pair[1] < pair[0]) {
        list.borrow_mut().sort_by_key(Value::as_i64);
        ticks.sort_unstable();
    }
    let mut deduplicated: Option<Vec<i64>> = None;
    for idx in 1..ticks.len() {
        if ticks[idx] == ticks[idx - 1] {
            deduplicated.get_or_insert_with(|| ticks[..idx - 1].to_vec());
        } else if let Some(deduplicated) = &mut deduplicated {
            deduplicated.push(ticks[idx]);
        }
    }
    deduplicated.unwrap_or(ticks)
}

/// The role a filler plays in a univariate fill: the single value role unless `force`d (or
/// absent), else the windowed role (`None` when absent).
enum Role<'a> {
    Window(Window<'a>),
    Single(Single<'a>),
}

fn univariate_role(roles: Roles<'_>, force: bool) -> Option<Role<'_>> {
    match (roles, force) {
        (Roles::Single(single) | Roles::Both(_, single), false) => Some(Role::Single(single)),
        (Roles::Window(window) | Roles::Both(window, _), _) => Some(Role::Window(window)),
        (Roles::Single(_), true) => None,
    }
}

fn role_error() -> Error {
    Error::new("FILL expects parameter filler to be a filler, but instead got a FILLER")
}

/// The univariate fill of a series or of each series of a list.
fn fill_target(
    stack: &mut Stack,
    target: Value,
    params: &Params<'_>,
    form: &Form,
) -> Result<Vec<Value>> {
    match target {
        Value::Gts(gts) => {
            let Some(role) = univariate_role(params.filler.roles(), params.force) else {
                // Only the map form forces the windowed role; its series stays on the stack.
                stack.push(Value::Gts(gts));
                return Err(role_error());
            };
            Ok(vec![Value::Gts(
                fill_series(stack, &gts, &role, params)?.shared(),
            )])
        }
        Value::List(list) => {
            let elements = list.borrow().clone();
            let mut results = Vec::with_capacity(elements.len());
            for element in &elements {
                let Value::Gts(gts) = element else {
                    return Err(Error::new(format!(
                        "{} {}",
                        form.list_element,
                        element.type_name()
                    )));
                };
                let Some(role) = univariate_role(params.filler.roles(), params.force) else {
                    return Err(role_error());
                };
                results.push(Value::Gts(fill_series(stack, gts, &role, params)?.shared()));
            }
            Ok(results)
        }
        other => {
            if form.on_stack {
                stack.push(other);
            }
            Err(Error::new(form.not_series))
        }
    }
}

/// `GTSHelper.fill(gts, filler, ticks, verify, invalidValue)`: a copy of `gts` (sorted in
/// place) whose empty buckets or given ticks are filled; unbucketized when ticks are given.
fn fill_series(
    stack: &mut Stack,
    gts: &Gts,
    role: &Role<'_>,
    params: &Params<'_>,
) -> Result<GeoTimeSerie> {
    gts.borrow_mut().sort(false);
    let mut filled = gts.borrow().clone();
    if filled.is_bucketized() {
        if params.ticks.is_some() {
            filled.bucketspan = 0;
            filled.bucketcount = 0;
            filled.lastbucket = 0;
        }
    } else if params.ticks.is_none() {
        return Ok(filled);
    }
    match role {
        Role::Window(window) => fill_windowed(stack, gts, window, params, filled),
        Role::Single(single) => fill_single(stack, gts, single, params, filled),
    }
}

/// The name, labels and attributes of a series.
fn identity(gts: &GeoTimeSerie) -> Metadata {
    Metadata {
        name: gts.metadata.name.clone(),
        labels: gts.metadata.labels.clone(),
        attributes: gts.metadata.attributes.clone(),
        ..Metadata::default()
    }
}

/// `pre` and `post` windows clamped to 0.
fn windows(window: &Window<'_>) -> (usize, usize) {
    (window.pre().max(0) as usize, window.post().max(0) as usize)
}

/// The slots of a windowed filler: the `pre` datapoints of `gts` before `pre_end`, the
/// `current` one and the `post` ones from `post_start`, empty beyond the series.
fn window_slots(
    gts: &GeoTimeSerie,
    (pre_end, post_start): (usize, usize),
    (pre, post): (usize, usize),
    current: Slot,
) -> Vec<Slot> {
    let mut slots = Vec::with_capacity(pre + 1 + post);
    for i in 0..pre {
        slots.push(
            (pre_end + i)
                .checked_sub(pre)
                .map_or_else(Slot::empty, |idx| Slot::at(gts, idx)),
        );
    }
    slots.push(current);
    for idx in post_start..post_start + post {
        slots.push(if idx < gts.size() {
            Slot::at(gts, idx)
        } else {
            Slot::empty()
        });
    }
    slots
}

/// Add the datapoint a windowed filler computed, unless its value is `null`.
fn set_result(filled: &mut GeoTimeSerie, result: &Slot) {
    if !result.value.is_null() {
        filled.set_value(
            result.tick.unwrap_or_default(),
            result.location.unwrap_or(NO_LOCATION),
            result.elevation.unwrap_or(NO_ELEVATION),
            &result.value,
            false,
        );
    }
}

/// The first bucket of a bucketized series.
fn first_bucket(gts: &GeoTimeSerie) -> i64 {
    gts.lastbucket
        .wrapping_sub((gts.bucketcount - 1).wrapping_mul(gts.bucketspan))
}

/// The empty buckets of a bucketized series sorted in ascending order (ticks which are not
/// on a bucket are ignored).
fn bucket_gaps(gts: &GeoTimeSerie) -> Vec<i64> {
    let span = gts.bucketspan;
    let mut gaps = Vec::new();
    let mut prev = first_bucket(gts);
    for &bucket in &gts.ticks {
        if bucket.wrapping_sub(prev).wrapping_rem(span) != 0 {
            continue;
        }
        while prev != bucket {
            gaps.push(prev);
            prev = prev.wrapping_add(span);
        }
        prev = prev.wrapping_add(span);
    }
    while prev <= gts.lastbucket {
        gaps.push(prev);
        prev = prev.wrapping_add(span);
    }
    gaps
}

/// The univariate fill with a windowed filler.
fn fill_windowed(
    stack: &mut Stack,
    gts: &Gts,
    window: &Window<'_>,
    params: &Params<'_>,
    mut filled: GeoTimeSerie,
) -> Result<GeoTimeSerie> {
    let meta = identity(&filled);
    let sizes = windows(window);
    let (ticks, verify) = match &params.ticks {
        Some(ticks) => (ticks.clone(), params.verify),
        None => (bucket_gaps(&gts.borrow()), false),
    };
    for tick in ticks {
        let slots = {
            let mut g = gts.borrow_mut();
            if verify {
                g.sort(false);
                if java_binary_search(&g.ticks, tick).is_some() {
                    continue;
                }
            }
            let mut index = 0;
            while index < g.size() && g.ticks[index] < tick {
                index += 1;
            }
            let pre_end = index;
            let current = if g.ticks.get(index) == Some(&tick) {
                index += 1;
                Slot::at(&g, pre_end)
            } else {
                Slot {
                    tick: Some(tick),
                    ..Slot::empty()
                }
            };
            window_slots(&g, (pre_end, index), sizes, current)
        };
        let result = window.apply(stack, (&meta, None), &slots)?;
        if result.value.is_null() {
            if !params.invalid.is_null() {
                filled.set_value(
                    result.tick.unwrap_or(tick),
                    result.location.unwrap_or(NO_LOCATION),
                    result.elevation.unwrap_or(NO_ELEVATION),
                    &params.invalid,
                    false,
                );
            }
        } else {
            set_result(&mut filled, &result);
        }
    }
    Ok(filled)
}

/// The univariate fill with a single value filler.
fn fill_single(
    stack: &mut Stack,
    gts: &Gts,
    single: &Single<'_>,
    params: &Params<'_>,
    mut filled: GeoTimeSerie,
) -> Result<GeoTimeSerie> {
    let mut ticker = single.compute(gts)?;
    let invalid = &params.invalid;
    match &params.ticks {
        Some(ticks) if !ticks.is_empty() && params.verify && gts.borrow().size() > 0 => {
            // Warp 10's verification walks the ticks of the series along the ticks to fill.
            let last_tick = gts.borrow().last_tick();
            let mut idx = {
                let g = gts.borrow();
                let idx = java_binary_search_range(&g.ticks, 0, g.size() as i64, ticks[0]);
                (if idx < 0 { -idx - 1 } else { idx }) as usize
            };
            for &tick in ticks {
                if tick != gts.borrow().tick_at(idx) {
                    ticker.fill_tick(stack, tick, &mut filled, invalid)?;
                }
                while tick <= last_tick
                    && tick >= gts.borrow().tick_at(idx)
                    && idx < gts.borrow().size()
                {
                    idx += 1;
                }
            }
        }
        Some(ticks) if !ticks.is_empty() => {
            for &tick in ticks {
                ticker.fill_tick(stack, tick, &mut filled, invalid)?;
            }
        }
        // The empty buckets, also when the deduplication of the ticks left none (even if the
        // series is not bucketized: Warp 10 then divides by its zero bucket span, or loops
        // forever on an empty series).
        _ => {
            let (span, lastbucket, mut prev) = {
                let g = gts.borrow();
                (g.bucketspan, g.lastbucket, first_bucket(&g))
            };
            if span == 0 {
                return Err(Error::division_by_zero());
            }
            let mut i = 0;
            while i < gts.borrow().size() {
                let bucket = gts.borrow().ticks[i];
                i += 1;
                if bucket.wrapping_sub(prev).wrapping_rem(span) != 0 {
                    continue;
                }
                while prev != bucket {
                    ticker.fill_tick(stack, prev, &mut filled, invalid)?;
                    prev = prev.wrapping_add(span);
                }
                prev = prev.wrapping_add(span);
            }
            while prev <= lastbucket {
                ticker.fill_tick(stack, prev, &mut filled, invalid)?;
                prev = prev.wrapping_add(span);
            }
        }
    }
    Ok(filled)
}

/// `a b filler FILL`.
fn cross_fill(stack: &mut Stack, filler: &FillerRef<'_>) -> Result<()> {
    let b = pop_gts(stack)?;
    let a = pop_gts(stack)?;
    let [ga, gb] = match filler.roles() {
        Roles::Window(window) | Roles::Both(window, _) => cross_windowed(stack, &a, &b, &window)?,
        Roles::Single(single) => cross_single(stack, &a, &b, &single)?,
    };
    stack.push(Value::Gts(ga.shared()));
    stack.push(Value::Gts(gb.shared()));
    Ok(())
}

fn pop_gts(stack: &mut Stack) -> Result<Gts> {
    match stack.pop()? {
        Value::Gts(gts) => Ok(gts),
        other => Err(Error::new(format!(
            "FILL expected a GTS, but instead got a {}",
            other.type_name()
        ))),
    }
}

/// The sweep of a cross fill over the ticks of `a` and `b` (sorted): skip the common ticks
/// and tell whether the next gap is in `a` (`Some(true)`) or in `b`, `None` at the end.
fn next_gap(
    a: &GeoTimeSerie,
    b: &GeoTimeSerie,
    idxa: &mut usize,
    idxb: &mut usize,
) -> Result<Option<bool>> {
    loop {
        match (a.ticks.get(*idxa), b.ticks.get(*idxb)) {
            (None, None) => return Ok(None),
            (Some(ta), Some(tb)) if ta == tb => {
                *idxa += 1;
                *idxb += 1;
                if a.ticks.get(*idxa) == Some(ta) || b.ticks.get(*idxb) == Some(tb) {
                    return Err(Error::new(
                        "Cannot fill Geo Time Series with duplicate timestamps.",
                    ));
                }
            }
            (Some(ta), Some(tb)) => return Ok(Some(ta > tb)),
            (ta, _) => return Ok(Some(ta.is_none())),
        }
    }
}

/// `GTSHelper.fill(gtsa, gtsb, WarpScriptFillerFunction)`.
fn cross_windowed(
    stack: &mut Stack,
    a: &Gts,
    b: &Gts,
    window: &Window<'_>,
) -> Result<[GeoTimeSerie; 2]> {
    a.borrow_mut().sort(false);
    b.borrow_mut().sort(false);
    let mut filled = [a.borrow().clone(), b.borrow().clone()];
    let metas = [identity(&filled[0]), identity(&filled[1])];
    let sizes = windows(window);
    let (mut idxa, mut idxb) = (0, 0);
    loop {
        let (target, slots) = {
            let (ga, gb) = (a.borrow(), b.borrow());
            let Some(fill_a) = next_gap(&ga, &gb, &mut idxa, &mut idxb)? else {
                break;
            };
            if fill_a {
                idxb += 1;
                let current = Slot::at(&gb, idxb - 1);
                (0, window_slots(&ga, (idxa, idxa), sizes, current))
            } else {
                idxa += 1;
                let current = Slot::at(&ga, idxa - 1);
                (1, window_slots(&gb, (idxb, idxb), sizes, current))
            }
        };
        let metas = (&metas[target], Some(&metas[1 - target]));
        let result = window.apply(stack, metas, &slots)?;
        set_result(&mut filled[target], &result);
    }
    Ok(filled)
}

/// `GTSHelper.fill(gtsa, gtsb, WarpScriptSingleValueFillerFunction)`.
fn cross_single(
    stack: &mut Stack,
    a: &Gts,
    b: &Gts,
    single: &Single<'_>,
) -> Result<[GeoTimeSerie; 2]> {
    a.borrow_mut().sort(false);
    b.borrow_mut().sort(false);
    let mut filled = [a.borrow().clone(), b.borrow().clone()];
    let mut tickers = [single.compute(a)?, single.compute(b)?];
    let (mut idxa, mut idxb) = (0, 0);
    loop {
        let (target, tick) = {
            let (ga, gb) = (a.borrow(), b.borrow());
            let Some(fill_a) = next_gap(&ga, &gb, &mut idxa, &mut idxb)? else {
                break;
            };
            if fill_a {
                idxb += 1;
                (0, gb.ticks[idxb - 1])
            } else {
                idxa += 1;
                (1, ga.ticks[idxa - 1])
            }
        };
        tickers[target].fill_tick(stack, tick, &mut filled[target], &Value::Null)?;
    }
    Ok(filled)
}

// -----------------------------------------------------------------------------------------
// FILLVALUE, FILLPREVIOUS, FILLNEXT, FILLTICKS, INTERPOLATE
// -----------------------------------------------------------------------------------------

/// Apply `f` to the series on top of the stack, or to the series of a list (its series and
/// those of its lists, other elements being ignored).
fn fill_each(
    stack: &mut Stack,
    name: &str,
    f: impl Fn(&GeoTimeSerie) -> GeoTimeSerie,
) -> Result<()> {
    let not_series = || {
        Error::new(format!(
            "{name} expects a LIST or a Geo Time Series as input."
        ))
    };
    match stack.pop()? {
        Value::Gts(gts) => {
            let filled = f(&gts.borrow());
            stack.push(Value::Gts(filled.shared()));
        }
        Value::List(list) => {
            let mut series = Vec::new();
            for element in list.borrow().iter() {
                match element {
                    Value::Gts(gts) => series.push(gts.clone()),
                    Value::List(inner) => {
                        for element in inner.borrow().iter() {
                            let Value::Gts(gts) = element else {
                                return Err(not_series());
                            };
                            series.push(gts.clone());
                        }
                    }
                    _ => {}
                }
            }
            let filled = series
                .iter()
                .map(|gts| Value::Gts(f(&gts.borrow()).shared()))
                .collect();
            stack.push(Value::list(filled));
        }
        _ => return Err(not_series()),
    }
    Ok(())
}

/// `(double) object`.
fn cast_double(value: &Value) -> Result<f64> {
    match value {
        Value::Double(d) => Ok(*d),
        Value::Null => Err(Error::null_pointer()),
        other => Err(Error::class_cast(other.kind(), ValueKind::Double)),
    }
}

/// `(long) object`.
fn cast_long(value: &Value) -> Result<i64> {
    match value {
        Value::Long(l) => Ok(*l),
        Value::Null => Err(Error::null_pointer()),
        other => Err(Error::class_cast(other.kind(), ValueKind::Long)),
    }
}

/// The location (`lat lon`, `NaN` for none) and elevation (`NaN` for none) parameters of
/// `FILLVALUE` and `FILLTICKS`.
fn geo_params(params: &[Value]) -> Result<(i64, i64)> {
    let lat = cast_double(&params[0])?;
    let lon = cast_double(&params[1])?;
    let location = if lat.is_nan() || lon.is_nan() {
        NO_LOCATION
    } else {
        to_geoxp_point(lat, lon)
    };
    let elevation = match &params[2] {
        Value::Double(d) if d.is_nan() => NO_ELEVATION,
        other => cast_long(other)?,
    };
    Ok((location, elevation))
}

/// `FILLVALUE`: `gts|[gts…] [ lat lon elev value ] FILLVALUE`.
fn fillvalue(stack: &mut Stack) -> Result<()> {
    let params = match stack.pop()? {
        Value::List(list) if list.borrow().len() == 4 => list.borrow().clone(),
        _ => {
            return Err(Error::new(
                "FILLVALUE expects a LIST of 4 parameters: latitude longitude elevation value.",
            ));
        }
    };
    let (location, elevation) = geo_params(&params)?;
    fill_each(stack, "FILLVALUE", |gts| {
        fill_value(gts, location, elevation, &params[3])
    })
}

/// `GTSHelper.fillvalue`: a sorted copy of a bucketized series with the datapoint added at
/// its empty buckets — by the loop of Warp 10, which also skips the bucket following a tick
/// that is not on a bucket.
fn fill_value(gts: &GeoTimeSerie, location: i64, elevation: i64, value: &Value) -> GeoTimeSerie {
    let mut filled = gts.clone();
    if !filled.is_bucketized() || value.is_null() {
        return filled;
    }
    filled.sort(false);
    let n = filled.size();
    let bucket_ts = |bucket: i64| {
        filled
            .lastbucket
            .wrapping_sub(bucket.wrapping_mul(filled.bucketspan))
    };
    let mut bucket = filled.bucketcount - 1;
    let mut fills = Vec::new();
    let mut idx = 0;
    while bucket >= 0 {
        let mut ts = bucket_ts(bucket);
        while (idx < n && filled.ticks[idx] > ts) || (idx >= n && bucket >= 0) {
            fills.push(ts);
            bucket -= 1;
            ts = bucket_ts(bucket);
        }
        idx += 1;
        bucket -= 1;
    }
    for tick in fills {
        filled.set_value(tick, location, elevation, value, false);
    }
    filled
}

/// `FILLPREVIOUS`: fill the empty buckets with the previous datapoint.
fn fillprevious(stack: &mut Stack) -> Result<()> {
    fill_each(stack, "FILLPREVIOUS", fill_previous)
}

/// `FILLNEXT`: fill the empty buckets with the next datapoint.
fn fillnext(stack: &mut Stack) -> Result<()> {
    fill_each(stack, "FILLNEXT", fill_next)
}

/// The location, elevation and value at `idx`.
fn datapoint(gts: &GeoTimeSerie, idx: usize) -> (i64, i64, Value) {
    (
        gts.location_at(idx),
        gts.elevation_at(idx),
        gts.value_at(idx),
    )
}

/// `GTSHelper.fillprevious`: a copy of a bucketized series, sorted in ascending order, with
/// its empty buckets filled with the datapoint of the previous bucket.
fn fill_previous(gts: &GeoTimeSerie) -> GeoTimeSerie {
    let mut filled = gts.clone();
    if !filled.is_bucketized() {
        return filled;
    }
    filled.sort(false);
    let (span, lastbucket) = (filled.bucketspan, filled.lastbucket);
    let bucket_ts = |bucket: i64| lastbucket.wrapping_sub(bucket.wrapping_mul(span));
    let offset = lastbucket.wrapping_rem(span);
    let ticks = filled.ticks.clone();
    let mut fills = Vec::new();
    let mut previous = None;
    let mut bucket = filled.bucketcount - 1;
    let mut idx = 0;
    while bucket >= 0 {
        while idx < ticks.len() && offset != ticks[idx].wrapping_rem(span) {
            idx += 1;
        }
        if idx >= ticks.len() {
            break;
        }
        while bucket >= 0 && ticks[idx] > bucket_ts(bucket) {
            fills.push((bucket_ts(bucket), previous));
            bucket -= 1;
        }
        bucket -= 1;
        previous = Some(idx);
        idx += 1;
    }
    while bucket >= 0 {
        fills.push((bucket_ts(bucket), previous));
        bucket -= 1;
    }
    add_fills(&mut filled, fills);
    filled
}

/// `GTSHelper.fillnext`: a copy of a bucketized series, sorted in descending order, with its
/// empty buckets filled with the datapoint of the next bucket.
fn fill_next(gts: &GeoTimeSerie) -> GeoTimeSerie {
    let mut filled = gts.clone();
    if !filled.is_bucketized() {
        return filled;
    }
    filled.sort(true);
    let (span, lastbucket, count) = (filled.bucketspan, filled.lastbucket, filled.bucketcount);
    let bucket_ts = |bucket: i64| lastbucket.wrapping_sub(bucket.wrapping_mul(span));
    let offset = lastbucket.wrapping_rem(span);
    let ticks = filled.ticks.clone();
    let mut fills = Vec::new();
    let mut next = None;
    let mut bucket = 0;
    let mut idx = 0;
    while bucket < count {
        while idx < ticks.len() && offset != ticks[idx].wrapping_rem(span) {
            idx += 1;
        }
        if idx >= ticks.len() {
            break;
        }
        // Unbounded like in Warp 10: ticks before the first bucket fill buckets before it.
        while ticks[idx] < bucket_ts(bucket) {
            fills.push((bucket_ts(bucket), next));
            bucket += 1;
        }
        bucket += 1;
        next = Some(idx);
        idx += 1;
    }
    while bucket < count {
        fills.push((bucket_ts(bucket), next));
        bucket += 1;
    }
    add_fills(&mut filled, fills);
    filled
}

/// Append, at each tick, the datapoint at the index paired with it (if any).
fn add_fills(filled: &mut GeoTimeSerie, fills: Vec<(i64, Option<usize>)>) {
    for (tick, idx) in fills {
        if let Some(idx) = idx {
            let (location, elevation, value) = datapoint(filled, idx);
            filled.set_value(tick, location, elevation, &value, false);
        }
    }
}

/// `FILLTICKS`: `gts|[gts…] [ lat lon elev value [ ticks ] ] FILLTICKS`.
fn fillticks(stack: &mut Stack) -> Result<()> {
    let Value::List(list) = stack.pop()? else {
        return Err(Error::new(
            "FILLTICKS expects a list of 5 parameters (lat, lon, elevation, value, ticks).",
        ));
    };
    let params = list.borrow().clone();
    if params.len() != 5 {
        return Err(Error::new(
            "FILLTICKS expects 5 parameters (lat, lon, elevation, value, ticks).",
        ));
    }
    let (location, elevation) = geo_params(&params)?;
    let not_ticks = || {
        Error::new(
            "FILLTICKS expects a list of ticks as the fifth parameter of its parameter list.",
        )
    };
    let Value::List(ticks) = &params[4] else {
        return Err(not_ticks());
    };
    let mut ticks = ticks
        .borrow()
        .iter()
        .map(|tick| match tick {
            Value::Long(tick) => Ok(*tick),
            _ => Err(not_ticks()),
        })
        .collect::<Result<Vec<i64>>>()?;
    ticks.sort_unstable();
    let value = &params[3];
    gts_or_list(stack, "FILLTICKS", |gts| {
        let mut filled = gts.borrow().clone();
        // `GTSHelper.fillticks` adds the datapoint at every tick, existing ones included.
        if !value.is_null() && !filled.is_bucketized() {
            for &tick in &ticks {
                filled.set_value(tick, location, elevation, value, false);
            }
        }
        Value::Gts(filled.shared())
    })
}

/// `INTERPOLATE`: fill the empty buckets by linear interpolation.
fn interpolate(stack: &mut Stack) -> Result<()> {
    gts_or_list_try(stack, "INTERPOLATE", |gts| {
        Ok(Value::Gts(interpolate_gts(&gts.borrow())?.shared()))
    })
}

/// `INTERPOLATE.interpolate`: a sorted copy of a bucketized series with its empty buckets
/// between two datapoints filled by linear interpolation of the values; then the missing
/// elevations (truncated) and locations between two known ones are interpolated as well,
/// filled datapoints included.
fn interpolate_gts(gts: &GeoTimeSerie) -> Result<GeoTimeSerie> {
    let mut filled = gts.clone();
    if !filled.is_bucketized() {
        return Ok(filled);
    }
    filled.sort(false);
    let n = filled.size();
    if n < 2 {
        return Ok(filled);
    }
    let (span, lastbucket) = (filled.bucketspan, filled.lastbucket);
    let mut bucket = lastbucket.wrapping_sub(filled.bucketcount.wrapping_mul(span));
    let has_elevation = |g: &GeoTimeSerie, i: usize| g.elevation_at(i) != NO_ELEVATION;
    let has_location = |g: &GeoTimeSerie, i: usize| g.location_at(i) != NO_LOCATION;
    let mut elevations = usize::from(has_elevation(&filled, 0));
    let mut locations = usize::from(has_location(&filled, 0));
    for i in 1..n {
        elevations += usize::from(has_elevation(&filled, i));
        locations += usize::from(has_location(&filled, i));
        let (previous, tick) = (filled.ticks[i - 1], filled.ticks[i]);
        while bucket < lastbucket && bucket <= previous {
            bucket = bucket.wrapping_add(span);
        }
        if bucket == tick {
            continue;
        }
        let (value, previous_value) = (filled.value_at(i), filled.value_at(i - 1));
        let (value, previous_value) = both(double_value(&value), double_value(&previous_value))?;
        let rate = (value - previous_value) / tick.wrapping_sub(previous) as f64;
        while bucket < tick {
            let delta = bucket.wrapping_sub(previous) as f64;
            filled.set_value(
                bucket,
                NO_LOCATION,
                NO_ELEVATION,
                &Value::Double(previous_value + delta * rate),
                false,
            );
            bucket = bucket.wrapping_add(span);
        }
    }
    if elevations >= 2 {
        filled.sort(false);
        let elevations = filled.elevations.as_deref_mut().unwrap_or_default();
        interpolate_gaps(
            &filled.ticks,
            elevations,
            NO_ELEVATION,
            |a, b, span, delta| (a as f64 + b.wrapping_sub(a) as f64 / span * delta) as i64,
        );
    }
    if locations >= 2 {
        filled.sort(false);
        let locations = filled.locations.as_deref_mut().unwrap_or_default();
        interpolate_gaps(
            &filled.ticks,
            locations,
            NO_LOCATION,
            |a, b, span, delta| {
                let ((alat, alon), (blat, blon)) = (from_geoxp_point(a), from_geoxp_point(b));
                to_geoxp_point(
                    alat + (blat - alat) / span * delta,
                    alon + (blon - alon) / span * delta,
                )
            },
        );
    }
    Ok(filled)
}

/// Replace the `none` entries of `values` lying between two known ones (`a` and `b`, at
/// ticks `span` apart) by `between(a, b, span, delta)`, `delta` being the distance of their
/// tick to the tick of `a`.
fn interpolate_gaps(
    ticks: &[i64],
    values: &mut [i64],
    none: i64,
    between: impl Fn(i64, i64, f64, f64) -> i64,
) {
    let n = values.len();
    let mut idx = values.iter().position(|v| *v != none).unwrap_or(n);
    while idx < n {
        let mut i = idx + 1;
        while i < n && values[i] != none {
            i += 1;
        }
        idx = i - 1;
        while i < n && values[i] == none {
            i += 1;
        }
        if i < n {
            let (a, b, origin) = (values[idx], values[i], ticks[idx]);
            let span = ticks[i].wrapping_sub(origin) as f64;
            for (value, tick) in values[idx + 1..i].iter_mut().zip(&ticks[idx + 1..i]) {
                *value = between(a, b, span, tick.wrapping_sub(origin) as f64);
            }
        }
        idx = i;
    }
}
