// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of the kernel smoothers. The expected values were checked against Warp 10 3.5.0
//! (`warp10io/warp10:3.5.0-alpine`).

use super::kernel::{Kernel, Smoother};
use super::tests::{Window, longs};
use super::{Aggregator, Roles};
use crate::tests::{assert_fails, assert_top, fail};
use crate::value::Value;

/// A located series: some datapoints without location or elevation, a `LONG` value.
const SERIES: &str = "NEWGTS 10 48.0 2.0 100 1.0 ADDVALUE 20 NaN NaN NaN 4.0 ADDVALUE \
    30 48.5 2.5 300 2.0 ADDVALUE 40 49.0 3.0 NaN 8 ADDVALUE 50 NaN NaN 500 5.0 ADDVALUE";

/// The ticks, values, latitudes, longitudes and elevations of the series smoothed by
/// `step width kernel` over a window of 20 ticks each side, as JSON.
fn smoothed(step: i64, width: i64, kernel: &str) -> String {
    format!(
        "[ {SERIES} {step} {width} {kernel} 20 20 0 ] MAP 0 GET 'r' STORE \
         $r TICKLIST $r VALUES $r LOCATIONS $r ELEVATIONS 5 ->LIST ->JSON"
    )
}

/// The values of the series smoothed by `step width kernel`, as JSON.
fn values(step: i64, width: i64, kernel: &str) -> String {
    format!("[ {SERIES} {step} {width} {kernel} 20 20 0 ] MAP 0 GET VALUES ->JSON")
}

#[test]
fn kernels() {
    // The transcendental kernels (cosine, gaussian, logistic, silverman) may differ from Java
    // in the last digit (DIVERGENCES §5.1); these match it exactly.
    for (kernel, expected) in [
        (
            "epanechnikov",
            "[[10,20,30,40,50],[2.2857142857142856,2.5,4.4,5.3,6.285714285714286],\
             [47.99999998882413,48.24999997392297,48.7142856605351,48.785714227706194,48.999999971129],\
             [1.9999999646097422,2.249999949708581,2.71428563632071,2.785714203491807,2.999999988824129],\
             [100,200,300,400,500]]",
        ),
        (
            "quartic",
            "[[10,20,30,40,50],[2.08,2.676470588235294,4.117647058823529,5.617647058823529,6.08],\
             [47.99999998882413,48.24999997392297,48.67999994661659,48.8199999416247,48.999999971129],\
             [1.9999999646097422,2.249999949708581,2.6799998804926872,2.81999995931983,2.999999988824129],\
             [100,200,300,400,500]]",
        ),
        (
            "triangular",
            "[[10,20,30,40,50],[2.0,2.75,4.0,5.75,6.0],\
             [47.99999998882413,48.24999997392297,48.66666660178453,48.833333286456764,48.999999971129],\
             [1.9999999646097422,2.249999949708581,2.666666619479656,2.8333333041518927,2.999999988824129],\
             [100,200,300,400,500]]",
        ),
        (
            "tricube",
            "[[10,20,30,40,50],[2.2035087719298243,2.568447412353923,4.290484140233723,5.423205342237062,6.203508771929824],\
             [47.99999998882413,48.24999997392297,48.70058472733945,48.799415160901845,48.999999971129],\
             [1.9999999646097422,2.249999949708581,2.7005847450345755,2.7994150947779417,2.999999988824129],\
             [100,200,300,400,500]]",
        ),
        (
            "triweight",
            "[[10,20,30,40,50],[1.89010989010989,2.8559322033898304,3.830508474576271,5.940677966101695,5.8901098901098905],\
             [47.99999998882413,48.24999997392297,48.64835159853101,48.85164828971028,48.999999971129],\
             [1.9999999646097422,2.249999949708581,2.648351574316621,2.8516482654958963,2.999999988824129],\
             [100,200,300,400,500]]",
        ),
        (
            "uniform",
            "[[10,20,30,40,50],[2.3333333333333335,3.75,4.0,4.75,5.0],\
             [48.24999997392297,48.49999995902181,48.49999995902181,48.749999944120646,48.749999944120646],\
             [2.249999949708581,2.4999999348074198,2.4999999348074198,2.7499999199062586,2.7499999199062586],\
             [200,200,300,400,400]]",
        ),
    ] {
        assert_top(
            &smoothed(10, 5, &format!("mapper.kernel.{kernel}")),
            expected,
        );
    }
    assert_top(
        &values(7, 7, "mapper.kernel.epanechnikov"),
        "[2.3181818181818183,3.3333333333333344,4.114285714285715,4.933333333333333,5.409090909090909]",
    );
    // A width of 1: a single weight, `0 / 0` for the kernels of `u`.
    assert_top(
        &values(10, 1, "mapper.kernel.uniform"),
        "[1.0,4.0,2.0,8.0,5.0]",
    );
    assert_top(
        &values(10, 1, "mapper.kernel.triangular"),
        "[NaN,NaN,NaN,NaN,NaN]",
    );
}

#[test]
fn transcendental_kernels() {
    for (kernel, expected) in [
        (
            "cosine",
            [
                2.242_640_687_119_285,
                2.535_533_905_932_737_8,
                4.343_145_750_507_619,
                5.363_961_030_678_928,
                6.242_640_687_119_286,
            ],
        ),
        (
            "gaussian",
            [
                2.307_346_458_013_163_3,
                3.410_841_625_275_861_8,
                4.079_668_652_390_890_5,
                4.924_656_822_125_322,
                5.332_619_349_482_729,
            ],
        ),
        (
            "logistic",
            [
                2.322_773_436_722_062_7,
                3.576_082_672_920_513,
                4.042_031_680_843_401,
                4.834_584_477_644_163,
                5.168_974_021_723_738,
            ],
        ),
        (
            "silverman",
            [
                2.850_853_519_975_124,
                4.120_075_730_943_507,
                4.276_280_279_962_685,
                3.701_544_286_995_654,
                4.552_560_559_925_371,
            ],
        ),
    ] {
        let Value::List(list) = crate::tests::top(&format!(
            "[ {SERIES} 10 5 mapper.kernel.{kernel} 20 20 0 ] MAP 0 GET VALUES"
        )) else {
            panic!("{kernel}: not a list");
        };
        let actual: Vec<f64> = list.borrow().iter().map(|v| v.as_f64().unwrap()).collect();
        for (a, e) in actual.iter().zip(expected) {
            assert!(
                (a - e).abs() <= 4.0 * f64::EPSILON * e.abs(),
                "{kernel}: {a} {e}"
            );
        }
    }
}

#[test]
fn errors_and_edge_cases() {
    let message = fail(&values(0, 5, "mapper.kernel.uniform")).to_string();
    assert!(message.contains("(/ by zero)"), "{message}");
    assert!(!message.contains("invalid parameters"), "{message}");
    let message = fail(&values(-10, 5, "mapper.kernel.uniform")).to_string();
    assert!(
        message.contains("(Index -1 out of bounds for length 3)"),
        "{message}"
    );
    // Non numeric values are ignored.
    assert_top(
        "[ NEWGTS 1 NaN NaN NaN 'a' ADDVALUE 1 3 mapper.kernel.uniform 0 0 0 ] MAP 0 GET VALUES",
        Value::list(vec![]),
    );
    let smoother = Smoother {
        step: 1,
        width: 3,
        weights: Kernel::Uniform.weights(2),
    };
    assert_eq!(smoother.roles(), Roles::MAPPER);
    assert_eq!(Window::of(&[]).apply(&smoother).unwrap().value, Value::Null);
    // A negative distance in steps for a non numeric value is skipped before the lookup.
    let smoother = Smoother {
        step: -1,
        ..smoother
    };
    assert_eq!(
        Window::of(&[Value::string("a"), Value::Boolean(true)])
            .apply(&smoother)
            .unwrap()
            .value,
        Value::Null
    );
    assert_eq!(
        Window::of(&longs(&[1])).error(&smoother),
        "Index -99 out of bounds for length 2"
    );
}

#[test]
fn builders() {
    for (script, message) in [
        (
            "10 4 mapper.kernel.uniform",
            "mapper.kernel.uniform window width MUST be odd (we count ticks, not intervals).",
        ),
        (
            "10 'a' mapper.kernel.uniform",
            "mapper.kernel.uniform expects a window width in ticks on top of the stack.",
        ),
        (
            "'a' 3 mapper.kernel.uniform",
            "mapper.kernel.uniform expects a step length as a LONG.",
        ),
        ("10 -1 mapper.kernel.uniform", "-2147483648"),
        (
            "10 -3 mapper.kernel.uniform",
            "Requested array size exceeds VM limit",
        ),
        ("10 67108865 mapper.kernel.uniform", "Java heap space"),
        ("mapper.kernel.uniform", "Empty stack"),
        ("3 mapper.kernel.uniform", "Empty stack"),
    ] {
        assert_fails(script, message);
    }
    assert_eq!(
        fail("10 -1 mapper.kernel.uniform")
            .root_cause()
            .java_class(),
        "java.lang.NegativeArraySizeException"
    );
    // The width is an `int`.
    assert_top(
        "10 4294967297 mapper.kernel.uniform SNAPSHOT",
        "10 1 mapper.kernel.uniform ",
    );
    for (name, _) in super::kernel::FUNCTIONS {
        assert_top(&format!("10 5 {name} SNAPSHOT"), format!("10 5 {name} "));
        assert_top(&format!("10 5 {name} TYPEOF"), "AGGREGATOR");
    }
    assert_fails(
        "[ NEWGTS 1 NaN NaN NaN 1 ADDVALUE 1 3 mapper.kernel.uniform 0 10 0 ] BUCKETIZE",
        "BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
    );
}
