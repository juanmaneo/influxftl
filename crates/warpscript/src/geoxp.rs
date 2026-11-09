// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from GeoXP Lib (Copyright 1999-2019 Mathias Herberts, 2019-2020 iroise.net S.A.S.,
// 2020- SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! GeoXP points: the 64-bit HHCodes Warp 10 stores as locations (`GeoXPLib`,
//! `HHCodeHelper`). An HHCode interleaves the bits of a 32-bit latitude and a 32-bit
//! longitude, each a fixed-point fraction of the 180° / 360° span.

/// Degrees per latitude unit (`180 / 2^32`).
const DEGREES_PER_LAT_UNIT: f64 = 180.0 / 4_294_967_296.0;
/// Degrees per longitude unit (`360 / 2^32`).
const DEGREES_PER_LON_UNIT: f64 = 360.0 / 4_294_967_296.0;

/// `GeoXPLib.toGeoXPPoint`: the HHCode of a location given in decimal degrees.
pub fn to_geoxp_point(lat: f64, lon: f64) -> i64 {
    // `HHCodeHelper.getHHCodeValue`: floor, so that a coordinate lands in the slot it lies
    // in rather than the next one.
    let lat = ((lat + 90.0) / DEGREES_PER_LAT_UNIT).floor() as i64;
    let lon = ((lon + 180.0) / DEGREES_PER_LON_UNIT).floor() as i64;
    build_hhcode(lat, lon)
}

/// `HHCodeHelper.buildHHCode` at the maximum resolution (32): interleave the latitude (odd
/// bits) and longitude (even bits) units, wrapping latitudes beyond the pole onto the other
/// hemisphere.
pub(crate) fn build_hhcode(mut lat: i64, mut lon: i64) -> i64 {
    if lat & 0x1_0000_0000 != 0 {
        lat ^= 0xFFFF_FFFF;
        lon ^= 0x8000_0000;
    }
    lat &= 0xFFFF_FFFF;
    lon &= 0xFFFF_FFFF;
    let mut hhcode: i64 = 0;
    for i in (0..32).rev() {
        hhcode = (hhcode << 1) | ((lat >> i) & 1);
        hhcode = (hhcode << 1) | ((lon >> i) & 1);
    }
    hhcode
}

/// `GeoXPLib.xyFromGeoXPPoint`: the `(latitude, longitude)` units of an HHCode (`0..2^32`).
pub fn xy_from_geoxp_point(hhcode: i64) -> (i64, i64) {
    split_hhcode(hhcode)
}

/// `GeoXPLib.toGeoXPPoint(long, long)`: the HHCode of `(latitude, longitude)` units.
pub fn to_geoxp_point_xy(lat: i64, lon: i64) -> i64 {
    build_hhcode(lat, lon)
}

/// `GeoXPLib.fromGeoXPPoint`: the `(latitude, longitude)` of an HHCode, in decimal degrees.
pub fn from_geoxp_point(hhcode: i64) -> (f64, f64) {
    let (lat, lon) = split_hhcode(hhcode);
    (
        lat as f64 * DEGREES_PER_LAT_UNIT - 90.0,
        lon as f64 * DEGREES_PER_LON_UNIT - 180.0,
    )
}

/// `GeoXPLib.loxodromicDistance`: the rhumb line distance in meters between two HHCodes, the
/// longitude delta (the shorter way round) scaled at the mean latitude, a nautical mile being
/// a minute of arc.
pub fn loxodromic_distance(from: i64, to: i64) -> f64 {
    const UNITS: f64 = 4_294_967_296.0;
    const METERS_PER_LAT_UNIT: f64 = 180.0 * 60.0 * 1852.0 / UNITS;
    const METERS_PER_LON_UNIT: f64 = 360.0 * 60.0 * 1852.0 / UNITS;
    const RADIANS_PER_LAT_UNIT: f64 = std::f64::consts::PI / UNITS;
    let (from_lat, from_lon) = split_hhcode(from);
    let (to_lat, to_lon) = split_hhcode(to);
    // `getLatScale`: the cosine of the mean latitude, as `cos(lat + PI / 2)` of the units.
    // The units are non-negative: the midpoint truncates like Java's `(f + t) / 2`.
    let scale = (from_lat.midpoint(to_lat) as f64 * RADIANS_PER_LAT_UNIT
        + std::f64::consts::PI / 2.0)
        .cos();
    let delta_lat = (from_lat - to_lat) as f64 * METERS_PER_LAT_UNIT;
    let mut dlon = (from_lon - to_lon).abs();
    if dlon > 0x7FFF_FFFF {
        dlon = 0xFFFF_FFFF - dlon;
    }
    let delta_lon = dlon as f64 * METERS_PER_LON_UNIT * scale;
    (delta_lat * delta_lat + delta_lon * delta_lon).sqrt()
}

/// `HHCodeHelper.splitHHCode` at the maximum resolution: de-interleave the latitude and
/// longitude units.
pub(crate) fn split_hhcode(hhcode: i64) -> (i64, i64) {
    let mut lat: i64 = 0;
    let mut lon: i64 = 0;
    for i in (0..32).rev() {
        lat = (lat << 1) | ((hhcode >> (1 + (i << 1))) & 1);
        lon = (lon << 1) | ((hhcode >> (i << 1)) & 1);
    }
    (lat, lon)
}

/// `HHCodeHelper.stableSplitHHCode`: the `(latitude, longitude)` units of an HHCode, keeping
/// the `resolution` leading bits of each (`0` to `32`) and clearing the others.
pub(crate) fn stable_split_hhcode(hhcode: i64, resolution: u32) -> (i64, i64) {
    let (lat, lon) = split_hhcode(hhcode);
    let mask = (0xFFFF_FFFFi64 << (32 - resolution)) & 0xFFFF_FFFF;
    (lat & mask, lon & mask)
}

/// `HHCodeHelper.deltaHHCode`: the HHCode `latdelta` / `londelta` cells of `resolution`
/// away (`northHHCode` and friends).
pub fn delta_hhcode(hhcode: i64, resolution: i32, latdelta: i64, londelta: i64) -> i64 {
    let (lat, lon) = split_hhcode(hhcode);
    let step = 1i64 << (32 - resolution);
    build_hhcode(
        lat.wrapping_add(latdelta.wrapping_mul(step)),
        lon.wrapping_add(londelta.wrapping_mul(step)),
    )
}

/// `HHCodeHelper.getCenter`: the HHCode of the center of the cell of `resolution`.
pub fn hhcode_center(hhcode: i64, resolution: i32) -> i64 {
    const CENTER: u64 = 0xC000_0000_0000_0000;
    if resolution == 0 {
        CENTER as i64
    } else if resolution < 32 {
        let trimmed = hhcode & ((u64::MAX << (64 - resolution * 2)) as i64);
        trimmed | (CENTER >> (2 * resolution)) as i64
    } else {
        hhcode
    }
}

/// `HHCodeHelper.getHHCodeBBox`: the `[ south west north east ]` bounds of the cell of
/// `resolution`.
pub fn hhcode_bbox(hhcode: i64, resolution: i32) -> [f64; 4] {
    let (lat, lon) = split_hhcode(hhcode);
    let mask = (1i64 << (32 - resolution)) - 1;
    let (north, east) = (lat | mask, lon | mask);
    let (south, west) = (north ^ mask, east ^ mask);
    [
        south as f64 * DEGREES_PER_LAT_UNIT - 90.0,
        west as f64 * DEGREES_PER_LON_UNIT - 180.0,
        north as f64 * DEGREES_PER_LAT_UNIT - 90.0,
        east as f64 * DEGREES_PER_LON_UNIT - 180.0,
    ]
}

/// `HHCodeHelper.toString(hhcode, resolution)`: the first `resolution / 2` hexadecimal
/// digits of the HHCode (the resolution made even).
pub fn hhcode_to_string(hhcode: i64, resolution: i32) -> String {
    let mut hex = format!("{hhcode:016x}");
    hex.truncate(((resolution & 0xfe) >> 1) as usize);
    hex
}

/// `HHCodeHelper.toGeoCell`: the geocell of an even `resolution` (2 to 30), `0` otherwise.
pub fn to_geocell(hhcode: i64, resolution: i32) -> i64 {
    if !(2..=31).contains(&resolution) || resolution % 2 == 1 {
        return 0;
    }
    let mut geocell = i64::from(resolution) << 59;
    geocell |= (hhcode >> 4) & 0x0fff_ffff_ffff_ffff;
    geocell & !((1i64 << (4 * (15 - (resolution >> 1)))) - 1)
}

/// `HHCodeHelper.orthodromicDistance`: the great circle distance in meters between two
/// HHCodes, a nautical mile being a minute of arc.
pub fn orthodromic_distance(from: i64, to: i64) -> f64 {
    use std::f64::consts::PI;
    const RADIANS_PER_LAT_UNIT: f64 = PI / 4_294_967_296.0;
    const RADIANS_PER_LON_UNIT: f64 = (PI + PI) / 4_294_967_296.0;
    let (from_lat, from_lon) = split_hhcode(from);
    let (to_lat, to_lon) = split_hhcode(to);
    let flat = from_lat as f64 * RADIANS_PER_LAT_UNIT - PI / 2.0;
    let flon = from_lon as f64 * RADIANS_PER_LON_UNIT - PI;
    let tlat = to_lat as f64 * RADIANS_PER_LAT_UNIT - PI / 2.0;
    let tlon = to_lon as f64 * RADIANS_PER_LON_UNIT - PI;
    let d = 2.0
        * (((flat - tlat) / 2.0).sin().powi(2)
            + flat.cos() * tlat.cos() * ((flon - tlon) / 2.0).sin().powi(2))
        .sqrt()
        .asin();
    d * (180.0 / PI) * (1852.0 * 60.0)
}
