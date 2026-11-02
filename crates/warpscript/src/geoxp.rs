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
