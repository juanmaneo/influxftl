// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the GeoXP point conversions, against values computed with `HHCodeHelper`.

use crate::geoxp::{from_geoxp_point, to_geoxp_point};

#[test]
fn hhcodes_match_geoxp_lib() {
    // (lat, lon, HHCode, lat back, lon back) from the Java library.
    let cases: [(f64, f64, i64, f64, f64); 8] = [
        (
            48.8566,
            2.3522,
            -2_295_869_407_736_226_379,
            48.856_599_982_827_9,
            2.352_199_992_164_969_4,
        ),
        (0.0, 0.0, -4_611_686_018_427_387_904, 0.0, 0.0),
        (-90.0, -180.0, 0, -90.0, -180.0),
        // The pole wraps onto the other side of the planisphere.
        (
            90.0,
            180.0,
            -1_537_228_672_809_129_302,
            89.999_999_958_090_48,
            0.0,
        ),
        (
            89.9999,
            179.9999,
            -9_594_442,
            89.999_899_961_985_65,
            179.999_899_920_076_13,
        ),
        (
            -33.8688,
            151.2093,
            8_426_202_144_485_016_707,
            -33.868_800_024_501_98,
            151.209_299_946_203_83,
        ),
        (
            91.0,
            10.0,
            -6_143_609_890_913_690_947,
            88.999_999_975_785_61,
            -170.000_000_009_313_23,
        ),
        (
            45.5,
            -73.6,
            -5_744_958_798_190_134_255,
            45.499_999_970_197_68,
            -73.600_000_021_979_21,
        ),
    ];
    for (lat, lon, hhcode, lat_back, lon_back) in cases {
        assert_eq!(to_geoxp_point(lat, lon), hhcode, "{lat} {lon}");
        let (la, lo) = from_geoxp_point(hhcode);
        assert_eq!(la.to_bits(), lat_back.to_bits(), "{lat} {lon} -> lat {la}");
        assert_eq!(lo.to_bits(), lon_back.to_bits(), "{lat} {lon} -> lon {lo}");
    }
}
