// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::bigdecimal::{BigDecimal, RoundingMode};

#[test]
fn divide_rounded_keeps_the_scale_of_the_dividend() {
    // `new BigDecimal(a).divide(new BigDecimal(b), RoundingMode.HALF_UP)` on JDK 27.
    for (a, b, expected) in [
        ("12.34", "-1E+2", "-0.12"),
        ("7.5", "1E+1", "0.8"),
        ("-7.5", "-3", "2.5"),
    ] {
        let a = BigDecimal::parse(a).unwrap();
        let b = BigDecimal::parse(b).unwrap();
        let quotient = a.divide_rounded(&b, RoundingMode::HalfUp).unwrap();
        assert_eq!(quotient.to_string(), expected);
    }
}
