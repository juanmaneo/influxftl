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
fn build_hhcode(mut lat: i64, mut lon: i64) -> i64 {
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
fn split_hhcode(hhcode: i64) -> (i64, i64) {
    let mut lat: i64 = 0;
    let mut lon: i64 = 0;
    for i in (0..32).rev() {
        lat = (lat << 1) | ((hhcode >> (1 + (i << 1))) & 1);
        lon = (lon << 1) | ((hhcode >> (i << 1)) & 1);
    }
    (lat, lon)
}
