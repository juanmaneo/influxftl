// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Symbolic pattern extraction on Geo Time Series (bSAX): `PATTERNS` / `ZPATTERNS`,
//! `PATTERNDETECTION` / `ZPATTERNDETECTION` and `DISCORDS` / `ZDISCORDS` (the `Z` forms do
//! not standardize the piecewise aggregate approximations, they expect Z-normalized data).

use std::collections::BTreeMap;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::gts::gts_or_list_try;
use super::gts_points::dedup_gts;
use crate::encoding::opb64_encode;
use crate::error::fault::BoundsError;
use crate::error::series::SeriesError;
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Gts, NO_ELEVATION, NO_LOCATION, Type, Values};
use crate::jmath;
use crate::sax::{SAX_MAX_LEVELS, bsax, sax};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("PATTERNS", Entry::Stack(patterns)),
    ("ZPATTERNS", Entry::Stack(zpatterns)),
    ("PATTERNDETECTION", Entry::Stack(patterndetection)),
    ("ZPATTERNDETECTION", Entry::Stack(zpatterndetection)),
    ("DISCORDS", Entry::Stack(discords)),
    ("ZDISCORDS", Entry::Stack(zdiscords)),
];

/// The bSAX parameters.
#[derive(Clone, Copy)]
struct Sax {
    alphabet_size: i32,
    word_len: i32,
    window_len: i32,
    standardize: bool,
}

/// `((Number) value).doubleValue()` on a numeric series.
fn number_at(gts: &GeoTimeSerie, idx: usize) -> f64 {
    gts.value_at(idx).as_f64().unwrap_or_default()
}

/// `GTSHelper.bSAX`: the bSAX word (order preserving base64) of each window of `window_len`
/// datapoints, split in `word_len` piecewise aggregates quantized on `alphabet_size` levels,
/// at the tick of the window's first datapoint. The series is sorted in place.
fn bsax_gts(gts: &Gts, params: Sax) -> Result<GeoTimeSerie> {
    let Sax {
        alphabet_size,
        word_len,
        window_len,
        standardize,
    } = params;
    {
        let g = gts.borrow();
        if !g.is_bucketized() || !matches!(g.type_of(), Type::Long | Type::Double) {
            return Err(SeriesError::PatternSeries.into());
        }
    }
    if word_len == 0 {
        return Err(Error::division_by_zero());
    }
    if window_len.wrapping_rem(word_len) != 0 {
        return Err(SeriesError::WordlenDivide.into());
    }
    let mut alphabet = alphabet_size as u32;
    if alphabet == 0 {
        return Err(SeriesError::AlphabetPowerOfTwo.into());
    }
    let levels = alphabet.trailing_zeros();
    alphabet >>= levels;
    if alphabet != 1 {
        return Err(SeriesError::AlphabetPowerOfTwo.into());
    }
    if !(1..=SAX_MAX_LEVELS).contains(&levels) {
        return Err(SeriesError::AlphabetRange(SAX_MAX_LEVELS).into());
    }
    let paa_len = window_len.wrapping_div(word_len);
    gts.borrow_mut().sort(false);
    let g = gts.borrow();
    let mut sax_gts = GeoTimeSerie::with_buckets(g.lastbucket, g.bucketcount, g.bucketspan);
    sax_gts.metadata = g.metadata.clone();
    let Ok(word_size) = usize::try_from(word_len) else {
        return Err(BoundsError::NegativeSize(i64::from(word_len)).into());
    };
    let n = g.size() as i64;
    if window_len < 1 {
        // Warp 10 reads past the datapoints, into the end of the (exactly sized) tick array.
        return Err(Error::index_out_of_bounds(n, g.size()));
    }
    let windows = (n - i64::from(window_len) + 1).max(0) as usize;
    let paa_count = paa_len.max(0) as usize;
    let wl = f64::from(word_len);
    let mut paa_sum = vec![0.0; word_size];
    let mut symbols = vec![0usize; word_size];
    for i in 0..windows {
        let mut sum = 0.0;
        let mut sumsq = 0.0;
        for (w, paa) in paa_sum.iter_mut().enumerate() {
            *paa = 0.0;
            for k in 0..paa_count {
                *paa += number_at(&g, i + w * paa_count + k);
            }
            if standardize {
                let mean = *paa / f64::from(paa_len);
                sum += mean;
                sumsq += mean * mean;
            }
        }
        let mut mu = 0.0;
        let mut sigma = 0.0;
        if standardize {
            mu = sum / wl;
            let mut variance = (sumsq / wl) - (sum * sum) / (wl * wl);
            if word_len > 1 {
                variance = variance * wl / (wl - 1.0);
            }
            sigma = variance.sqrt();
        }
        for (symbol, paa) in symbols.iter_mut().zip(&paa_sum) {
            let mean = paa / f64::from(paa_len);
            *symbol = if !standardize {
                sax(levels, mean)
            } else if sigma != 0.0 {
                sax(levels, (mean - mu) / sigma)
            } else {
                sax(levels, mean - mu)
            };
        }
        let word = opb64_encode(&bsax(levels, &symbols));
        sax_gts.set_value(
            g.ticks[i],
            NO_LOCATION,
            NO_ELEVATION,
            &Value::from(word),
            false,
        );
    }
    Ok(sax_gts)
}

/// `((Number) value).intValue()`.
fn int_value(value: &Value) -> Option<i32> {
    match value {
        Value::Long(l) => Some(*l as i32),
        Value::Double(d) => Some(*d as i32),
        _ => None,
    }
}

/// Pop the quantization scale, pattern length and window size of `PATTERNS` and
/// `PATTERNDETECTION`.
fn pop_sax(stack: &mut Stack, name: &str, standardize: bool) -> Result<Sax> {
    let mut next = |message: &str| -> Result<i32> {
        int_value(&stack.pop()?).ok_or_else(|| fail(name, message))
    };
    let alphabet_size = next("expects a quantization scale on top of the stack.")?;
    let word_len = next("expects a pattern length at the second level of the stack.")?;
    let window_len = next("expects a window size at the third level of the stack.")?;
    Ok(Sax {
        alphabet_size,
        word_len,
        window_len,
        standardize,
    })
}

/// Apply `f` to the series on top of the stack or to each series of a list (pushing the
/// argument back when it is neither), like `PATTERNS` and `PATTERNDETECTION`.
fn on_series(
    stack: &mut Stack,
    name: &str,
    f: &dyn Fn(&Gts) -> Result<GeoTimeSerie>,
) -> Result<()> {
    gts_or_list_try(stack, name, |gts| Ok(Value::Gts(f(gts)?.shared())))
}

/// `PATTERNS` / `ZPATTERNS`: `gts window wordlen alphabetsize PATTERNS`.
fn patterns_impl(stack: &mut Stack, name: &str, standardize: bool) -> Result<()> {
    let params = pop_sax(stack, name, standardize)?;
    on_series(stack, name, &|gts| bsax_gts(gts, params))
}

/// `PATTERNS`: standardized aggregates.
fn patterns(stack: &mut Stack) -> Result<()> {
    patterns_impl(stack, "PATTERNS", true)
}

/// `ZPATTERNS`: raw aggregates.
fn zpatterns(stack: &mut Stack) -> Result<()> {
    patterns_impl(stack, "ZPATTERNS", false)
}

/// `GTSHelper.detect`: the datapoints of the windows whose bSAX word is one of `patterns`.
fn detect(gts: &Gts, params: Sax, patterns: &[Value]) -> Result<GeoTimeSerie> {
    let words = bsax_gts(gts, params)?;
    let g = gts.borrow();
    let mut detected = GeoTimeSerie::with_buckets(g.lastbucket, g.bucketcount, g.bucketspan);
    detected.metadata = g.metadata.clone();
    let mut lastidx = -1i64;
    for i in 0..words.size() {
        if !patterns.contains(&words.value_at(i)) {
            continue;
        }
        for j in 0..i64::from(params.window_len) {
            let idx = i as i64 + j;
            if idx > lastidx {
                lastidx = idx;
                detected.copy_datapoint(&g, idx as usize);
            }
        }
    }
    Ok(detected)
}

/// `PATTERNDETECTION` / `ZPATTERNDETECTION`: `gts patterns window wordlen alphabetsize
/// PATTERNDETECTION`.
fn patterndetection_impl(stack: &mut Stack, name: &str, standardize: bool) -> Result<()> {
    let params = pop_sax(stack, name, standardize)?;
    let Value::List(patterns) = stack.pop()? else {
        return Err(fail(
            name,
            "expects a list of patterns as the fourth level of the stack.",
        ));
    };
    let patterns = patterns.borrow().clone();
    on_series(stack, name, &|gts| detect(gts, params, &patterns))
}

/// `PATTERNDETECTION`: standardized aggregates.
fn patterndetection(stack: &mut Stack) -> Result<()> {
    patterndetection_impl(stack, "PATTERNDETECTION", true)
}

/// `ZPATTERNDETECTION`: raw aggregates.
fn zpatterndetection(stack: &mut Stack) -> Result<()> {
    patterndetection_impl(stack, "ZPATTERNDETECTION", false)
}

/// A discord candidate: the index of its window and the distance to its nearest neighbour.
#[derive(Clone, Copy)]
struct Candidate {
    location: usize,
    nndist: f64,
}

/// The parameters of `DISCORDS.discords`.
#[derive(Clone, Copy)]
struct Discords {
    sax: Sax,
    count: i32,
    may_overlap: bool,
    dist_ratio: f64,
}

/// `ArrayList` index errors (`Objects.checkIndex`).
fn index_error(index: i64, length: usize) -> Error {
    BoundsError::ListIndex { index, length }.into()
}

/// `DISCORDS.discords`: the windows farthest from their nearest non overlapping neighbour,
/// looked up among the windows with the rarest bSAX words first (HOT SAX).
fn discords_gts(gts: &Gts, params: Discords) -> Result<GeoTimeSerie> {
    let Discords {
        sax: sax_params,
        count,
        may_overlap,
        dist_ratio,
    } = params;
    let window_len = sax_params.window_len;
    let words = bsax_gts(gts, sax_params)?;
    let g = gts.borrow();
    let mut location_lists: BTreeMap<Rc<str>, Vec<usize>> = BTreeMap::new();
    if let Values::String(words) = &words.values {
        for (i, word) in words.iter().enumerate() {
            location_lists.entry(Rc::clone(word)).or_default().push(i);
        }
    }
    let raw_symbols: Vec<Rc<str>> = location_lists.keys().cloned().collect();
    let mut by_occurrences = raw_symbols.clone();
    by_occurrences.sort_by_key(|symbol| location_lists[symbol].len());
    // The squared distance between the windows starting at `a` and `b`.
    let distance = |a: usize, b: usize| -> f64 {
        let mut dist = 0.0;
        for k in 0..window_len as usize {
            dist += jmath::pow(number_at(&g, a + k) - number_at(&g, b + k), 2.0);
        }
        dist
    };
    let overlaps = |a: usize, b: usize| (a as i64 - b as i64).abs() < i64::from(window_len);
    let mut discords: Vec<Candidate> = Vec::new();
    let mut worst = 0.0;
    let count_len = i64::from(count);
    for symbol in &by_occurrences {
        let locations = &location_lists[symbol];
        if discords.len() as i64 >= count_len
            && locations.len() as i64 > i64::from(count.wrapping_add(window_len))
        {
            break;
        }
        for &location in locations {
            let pruned = |discords: &Vec<Candidate>, nndist: f64| {
                discords.len() as i64 == count_len && nndist < worst
            };
            let mut nndist = f64::INFINITY;
            // The nearest neighbour among the windows of the same word, then of the previous
            // words in lexicographic order until one is found.
            let idx = raw_symbols.binary_search(symbol).unwrap_or_default();
            for (pass, other) in std::iter::once(symbol)
                .chain(raw_symbols[..idx].iter().rev())
                .enumerate()
            {
                for &neighbour in &location_lists[other] {
                    if overlaps(location, neighbour) {
                        continue;
                    }
                    let dist = distance(location, neighbour);
                    if dist < nndist {
                        nndist = dist;
                    }
                    if pruned(&discords, nndist) {
                        break;
                    }
                }
                if pruned(&discords, nndist) || (pass > 0 && nndist != f64::INFINITY) {
                    break;
                }
            }
            if pruned(&discords, nndist) {
                continue;
            }
            discords.push(Candidate { location, nndist });
            // `(int) Math.signum(o2.nndist - o1.nndist)`, a stable sort.
            discords.sort_by(|a, b| {
                let signum = jmath::signum(b.nndist - a.nndist) as i32;
                signum.cmp(&0)
            });
            if !may_overlap {
                let mut h = discords.len() as i64 - 1;
                while h > 0 {
                    let hu = h as usize;
                    if overlaps(discords[hu].location, discords[hu - 1].location) {
                        discords.remove(hu);
                        h -= 1;
                    }
                    h -= 1;
                }
            }
            while discords.len() as i64 > count_len {
                if count < 0 {
                    return Err(index_error(count_len, discords.len()));
                }
                discords.remove(count as usize);
            }
            // Warp 10 loops forever once the last candidate is close enough to the first.
            while dist_ratio != 0.0 && discords.len() > 1 {
                let last = discords[discords.len() - 1].nndist;
                if last * dist_ratio < discords[0].nndist {
                    discords.pop();
                } else {
                    break;
                }
            }
            let Some(last) = discords.last() else {
                return Err(index_error(-1, 0));
            };
            worst = last.nndist;
        }
    }
    let mut result = g.clone_empty();
    for discord in &discords {
        for i in 0..window_len as usize {
            result.copy_datapoint(&g, discord.location + i);
        }
    }
    Ok(dedup_gts(&result))
}

/// `DISCORDS` / `ZDISCORDS`: `gts window wordlen alphabetsize count overlap distratio
/// DISCORDS`.
fn discords_impl(stack: &mut Stack, name: &str, standardize: bool) -> Result<()> {
    let Value::Double(dist_ratio) = stack.pop()? else {
        return Err(fail(name, "expects a double distance ratio."));
    };
    if dist_ratio < 0.0 {
        return Err(fail(name, "expects a positive distance ratio."));
    }
    let Value::Boolean(may_overlap) = stack.pop()? else {
        return Err(fail(
            name,
            "expects a boolean flag to indicate tolerance to overlapping.",
        ));
    };
    let Value::Long(count) = stack.pop()? else {
        return Err(fail(name, "expects an integer discord count."));
    };
    let Value::Long(alphabet_size) = stack.pop()? else {
        return Err(fail(name, "expects an integer quantization scale."));
    };
    let alphabet_size = alphabet_size as i32;
    if alphabet_size & alphabet_size.wrapping_sub(1) != 0 || alphabet_size < 2 {
        return Err(fail(
            name,
            "expects a quantization scale which is a power of 2 greater or equal to 2.",
        ));
    }
    let word_len = stack.pop()?;
    let window_len = stack.pop()?;
    let Value::Long(word_len) = word_len else {
        return Err(fail(name, "expects an integer pattern length."));
    };
    let Value::Long(window_len) = window_len else {
        return Err(fail(name, "expects an integer detection window length."));
    };
    let (word_len, window_len) = (word_len as i32, window_len as i32);
    if word_len == 0 {
        return Err(Error::division_by_zero());
    }
    if window_len.wrapping_rem(word_len) != 0 {
        return Err(fail(
            name,
            "expects pattern length to divide detection window length.",
        ));
    }
    let params = Discords {
        sax: Sax {
            alphabet_size,
            word_len,
            window_len,
            standardize,
        },
        count: count as i32,
        may_overlap,
        dist_ratio,
    };
    on_series(stack, name, &|gts| discords_gts(gts, params))
}

/// `DISCORDS`: standardized aggregates.
fn discords(stack: &mut Stack) -> Result<()> {
    discords_impl(stack, "DISCORDS", true)
}

/// `ZDISCORDS`: raw aggregates.
fn zdiscords(stack: &mut Stack) -> Result<()> {
    discords_impl(stack, "ZDISCORDS", false)
}
