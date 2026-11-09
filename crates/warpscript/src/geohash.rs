// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! GeoHashes (`com.geoxp.geo.GeoHashHelper`): conversions between GeoHash strings and the
//! HHCodes and geocells of GeoXP.

use crate::coverage::Coverage;
use crate::error::Result;
use crate::error::geo::GeoError;
use crate::geoxp::{build_hhcode, split_hhcode};
use crate::javamap::JavaHashMap;
use crate::pickle::string_hash;

/// The GeoHash base 32 alphabet.
const GEOHASH_CHAR_MAP: &[u8; 32] = b"0123456789bcdefghjkmnpqrstuvwxyz";

/// `GeoHashHelper.fromHHCode`: the GeoHash of the first `resolution` bits pairs of an HHCode
/// (as many base 32 characters as fit).
pub fn from_hhcode(hhcode: i64, resolution: i32) -> String {
    // A GeoHash interleaves the bits starting with the longitude, an HHCode with the
    // latitude: swap each pair of bits.
    let mut hhcode = hhcode as u64;
    let mut codehh: u64 = 0;
    for _ in 0..32 {
        codehh >>= 2;
        if hhcode & 1 != 0 {
            codehh |= 0x8000_0000_0000_0000;
        }
        hhcode >>= 1;
        if hhcode & 1 != 0 {
            codehh |= 0x4000_0000_0000_0000;
        }
        hhcode >>= 1;
    }
    let mut bits = 2 * resolution;
    bits -= bits % 5;
    // `>>>` takes the shift modulo 64: a resolution of 0 leaves the value, and the loop below
    // does not run.
    codehh = codehh.wrapping_shr((64 - bits) as u32);
    let mut chars = Vec::new();
    while bits > 0 {
        chars.push(char::from(GEOHASH_CHAR_MAP[(codehh & 0x1f) as usize]));
        codehh >>= 5;
        bits -= 5;
    }
    chars.iter().rev().collect()
}

/// `GeoHashHelper.toHHCode`: the HHCode of the lower left corner of a GeoHash (its first 12
/// characters count).
pub fn to_hhcode(geohash: &str) -> Result<i64> {
    let units: Vec<u16> = geohash.encode_utf16().collect();
    let mut hhcode: i64 = 0;
    let used = units.len().min(12);
    for (i, unit) in units.iter().take(used).enumerate() {
        if i > 0 {
            hhcode <<= 5;
        }
        let Some(idx) = GEOHASH_CHAR_MAP.iter().position(|c| u16::from(*c) == *unit) else {
            return Err(GeoError::InvalidGeoHash {
                geohash: geohash.to_owned(),
                index: i,
            }
            .into());
        };
        hhcode |= idx as i64;
    }
    // Java shifts by 64 - 5 * 0 = 64, that is by 0, for the empty GeoHash: the value is 0.
    hhcode = hhcode.wrapping_shl((64 - 5 * used) as u32);
    let (lat, lon) = split_hhcode(hhcode);
    Ok(build_hhcode(lon, lat))
}

/// `GeoHashHelper.toCoverage`: the cells of the GeoHashes, optimized as they are added. The
/// GeoHashes of 1, 3, 5, 7, 9 and 11 characters (5 bits per character) cover half the cells
/// of the even resolution holding them: only those are kept.
pub fn to_coverage(geohashes: &[String]) -> Result<Coverage> {
    let mut coverage = Coverage::new();
    coverage.set_auto_thresholds(0);
    coverage.set_auto_dedup(true);
    for geohash in geohashes {
        let hhcode = to_hhcode(geohash)?;
        let mut nbits = 5 * geohash.encode_utf16().count().min(12) as i32;
        let mut mask = ((-1i64 as u64).wrapping_shr((64 - nbits) as u32))
            .wrapping_shl((64 - nbits) as u32) as i64;
        let mut target = nbits;
        match nbits {
            5 => (nbits, mask, target) = (4, 0xF4i64 << 56, 8),
            10 => target = 12,
            15 => (nbits, mask, target) = (14, 0xFFFDi64 << 48, 16),
            25 => (nbits, mask, target) = (24, 0xFFF_FFF4i64 << 36, 28),
            30 => target = 32,
            35 => (nbits, mask, target) = (34, 0xF_FFFF_FFFDi64 << 28, 36),
            45 => (nbits, mask, target) = (44, 0xFFFF_FFFF_FFF4i64 << 16, 48),
            50 => target = 52,
            55 => (nbits, mask, target) = (54, 0xFF_FFFF_FFFF_FFFDi64 << 8, 56),
            _ => {}
        }
        let deltabits = target - nbits;
        let resolution = target >> 1;
        if deltabits > 0 {
            let prefix = ((hhcode as u64) >> (64 - nbits)) << deltabits;
            for delta in 0..(1u64 << deltabits) {
                let cell = ((prefix | delta) << (64 - target)) as i64;
                if cell & mask == hhcode {
                    coverage.add_cell(resolution, cell);
                }
            }
        } else {
            coverage.add_cell(resolution, hhcode);
        }
    }
    Ok(coverage)
}

/// `GeoHashHelper.toGeoCells`.
pub fn to_geocells(geohashes: &[String]) -> Result<Vec<i64>> {
    Ok(to_coverage(geohashes)?.to_geocells(32))
}

/// `GeoHashHelper.fromGeoCells(geocells, true)`: the GeoHashes covering the geocells, merged
/// by `optimize`, in the iteration order of the Java `HashSet` they come from.
pub fn from_geocells_optimized(geocells: &[i64]) -> Vec<String> {
    optimize(from_geocells(geocells))
}

/// `GeoHashHelper.fromGeoCells(geocells, false)`: a `HashSet` presized to the number of cells.
fn from_geocells(geocells: &[i64]) -> Vec<String> {
    let mut geohashes: JavaHashMap<String, ()> = JavaHashMap::with_capacity(geocells.len());
    for geocell in geocells {
        let resolution = (((*geocell as u64) >> 60) << 1) as i32;
        let nbits = resolution * 2;
        let hhcode = ((*geocell << 4) as u64).wrapping_shr((64 - nbits) as u32);
        let hhcode = hhcode.wrapping_shl((64 - nbits) as u32);
        if nbits % 5 == 0 {
            let geohash = from_hhcode(hhcode as i64, resolution);
            geohashes.put(string_hash(&geohash), geohash, ());
        } else {
            let mut deltabits = 5 - nbits % 5;
            if (nbits + deltabits) % 2 == 1 {
                deltabits += 1;
            }
            let shift = 64 - (nbits + deltabits);
            // `nbits + deltabits` is even.
            let resolution = i32::midpoint(nbits, deltabits);
            let prefix = hhcode >> shift;
            for offset in 0..(1u64 << deltabits) {
                let geohash = from_hhcode(((prefix | offset) << shift) as i64, resolution);
                geohashes.put(string_hash(&geohash), geohash, ());
            }
        }
    }
    geohashes
        .into_entries()
        .into_iter()
        .map(|(_, geohash, ())| geohash)
        .collect()
}

/// `GeoHashHelper.optimize`: replace the 32 GeoHashes sharing a parent by the parent, the
/// result being a new `HashSet` (so in its iteration order).
fn optimize(geohashes: Vec<String>) -> Vec<String> {
    let mut sorted = geohashes;
    sorted.sort();
    let mut optimized: JavaHashMap<String, ()> = JavaHashMap::new();
    let mut add = |geohash: String| optimized.put(string_hash(&geohash), geohash, ());
    let flush = |add: &mut dyn FnMut(String), prefix: &str, subcells: u32, mask: u32| {
        if subcells == 32 {
            add(prefix.to_owned());
        } else {
            for (i, c) in GEOHASH_CHAR_MAP.iter().enumerate() {
                if mask & (1 << i) != 0 {
                    add(format!("{prefix}{}", char::from(*c)));
                }
            }
        }
    };
    let mut lastprefix = String::from("-");
    let mut lastlen = 0;
    let mut subcells = 0;
    let mut mask = 0u32;
    for geohash in &sorted {
        if geohash.len() != lastlen || !geohash.starts_with(&lastprefix) {
            flush(&mut add, &lastprefix, subcells, mask);
            subcells = 0;
            mask = 0;
            lastlen = geohash.len();
            geohash[..lastlen - 1].clone_into(&mut lastprefix);
        }
        // The GeoHashes are made of alphabet characters only.
        let last = geohash.as_bytes()[lastlen - 1];
        mask |= 1
            << GEOHASH_CHAR_MAP
                .iter()
                .position(|c| *c == last)
                .unwrap_or_default();
        subcells += 1;
    }
    flush(&mut add, &lastprefix, subcells, mask);
    optimized
        .into_entries()
        .into_iter()
        .map(|(_, geohash, ())| geohash)
        .collect()
}
