// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreadable_literal
)]

use crate::distribution::TDistribution;

#[test]
fn quantiles_match_commons_math() {
    // `new TDistribution(df).inverseCumulativeProbability(p)` with Commons Math 3.6.1.
    for (df, p, expected) in [
        (38.0, 0.00625, -2.6221992176380753),
        (1.0, 0.05 / 6.0, -38.1884592970256),
        (2.0, 0.05 / 8.0, -8.860200034644315),
        (3.0, 0.01, -4.540702858647162),
        (38.0, 1.0 - 0.05 / 80.0, 3.4870456296590038),
        (118.0, 0.05 / 244.0, -3.6372859252889227),
    ] {
        let actual = TDistribution::new(df)
            .inverse_cumulative_probability(p)
            .unwrap();
        assert_eq!(actual.to_bits(), f64::to_bits(expected), "{df} {p}");
    }
    assert_eq!(TDistribution::new(5.0).cumulative_probability(0.0), 0.5);
}
