// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Tests of `mapper.product` and the `mapper.dotproduct` family. The expected values were
//! checked against Warp 10 3.5.0 (`warp10io/warp10:3.5.0-alpine`).

use indexmap::IndexMap;

use super::product::{Activation, DotProduct, Exact, Product};
use super::tests::{Window, doubles, longs};
use super::{Aggregated, Aggregator, AggregatorArgs, Roles, Window as Parameters};
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::tests::{assert_fails, assert_top, fail};
use crate::value::Value;

/// The `[ ticks values ]` of the series `datapoints` mapped with `parameters`, as JSON.
fn mapped(datapoints: &str, parameters: &str) -> String {
    format!(
        "[ NEWGTS {datapoints} 'g' STORE $g {parameters} ] MAP 0 GET 'r' STORE \
         $r TICKLIST $r VALUES 2 ->LIST ->JSON"
    )
}

const SERIES: &str = "1 NaN NaN NaN 2 ADDVALUE 2 NaN NaN NaN 3 ADDVALUE 3 NaN NaN NaN 4 ADDVALUE";
const LOCATED: &str = "1 10 20 NaN 2 ADDVALUE 3 30 40 7 3 ADDVALUE";

/// `aggregator` applied to `values` with the window parameters `window`.
fn apply(
    aggregator: &dyn Aggregator,
    values: &[Value],
    window: Option<Parameters>,
) -> crate::error::Result<Aggregated> {
    let names = ["g".to_owned()];
    let labels = [IndexMap::new()];
    let ticks: Vec<i64> = (1..=values.len() as i64).collect();
    let locations: Vec<i64> = (11..11 + values.len() as i64).collect();
    let elevations: Vec<i64> = (21..21 + values.len() as i64).collect();
    aggregator.apply(&AggregatorArgs {
        tick: 100,
        names: &names,
        labels: &labels,
        ticks: &ticks,
        locations: &locations,
        elevations: &elevations,
        values,
        window,
    })
}

fn parameters(tick_index: Option<i64>) -> Option<Parameters> {
    Some(Parameters {
        prewindow: 0,
        postwindow: 0,
        start: 0,
        stop: 0,
        tick_index,
    })
}

#[test]
fn products() {
    for (datapoints, parameters, expected) in [
        (SERIES, "mapper.product 1 1 0", "[[1,2,3],[6,24,12]]"),
        (
            "1 NaN NaN NaN 4611686018427387904 ADDVALUE 2 NaN NaN NaN 4 ADDVALUE",
            "mapper.product 1 0 0",
            "[[1,2],[4611686018427387904,0]]",
        ),
        // Exact: the double arithmetic would give 0.006000000000000001.
        (
            "1 NaN NaN NaN 0.1 ADDVALUE 2 NaN NaN NaN 0.2 ADDVALUE 3 NaN NaN NaN 0.3 ADDVALUE",
            "mapper.product 2 0 0",
            "[[1,2,3],[0.1,0.020000000000000004,0.006]]",
        ),
        (
            "1 NaN NaN NaN -0.0 ADDVALUE 2 NaN NaN NaN 5.0 ADDVALUE",
            "mapper.product 1 0 0",
            "[[1,2],[0.0,0.0]]",
        ),
        (
            "1 NaN NaN NaN 1e300 ADDVALUE 2 NaN NaN NaN 1e300 ADDVALUE 3 NaN NaN NaN 1e-300 ADDVALUE",
            "mapper.product 2 0 0",
            "[[1,2,3],[1.0E300,Infinity,1.0000000000000002E300]]",
        ),
        (
            "1 NaN NaN NaN 1e-200 ADDVALUE 2 NaN NaN NaN 1e-200 ADDVALUE",
            "mapper.product 1 0 0",
            "[[1,2],[1.0E-200,0.0]]",
        ),
        (LOCATED, "mapper.product 0 1 0", "[[1,3],[6,3]]"),
    ] {
        assert_top(&mapped(datapoints, parameters), expected);
    }
    assert_top(
        &format!("[ NEWGTS {LOCATED} 'g' STORE $g mapper.product 0 1 0 ] MAP 0 GET LOCATIONS DROP"),
        Value::list(doubles(&[9.999_999_990_686_774, 29.999_999_972_060_323])),
    );
    // `BigDecimal` rejects the infinite and `NaN` doubles with a runtime exception.
    let message = fail(&mapped(
        "1 NaN NaN NaN NaN ADDVALUE",
        "mapper.product 0 0 0",
    ))
    .to_string();
    assert!(message.contains("(Infinite or NaN)"), "{message}");
    assert_fails(
        &mapped("1 NaN NaN NaN 'a' ADDVALUE", "mapper.product 0 0 0"),
        "MAP was given invalid parameters. (mapper.product can only be applied to LONG or DOUBLE values.)",
    );
    // An output tick without datapoint has no index in the window.
    let message = fail(&mapped(LOCATED, "mapper.product 1 0 [ 2 ]")).to_string();
    assert!(
        message.contains("(Index -1 out of bounds for length 1)"),
        "{message}"
    );
}

#[test]
fn products_of_reducer_windows() {
    let product = Product {
        name: "reducer.product",
    };
    let roles = product.roles();
    assert!(roles.reducer && roles.mapper && !roles.bucketizer);
    // No window parameters: the location of the first datapoint; `null`s are skipped.
    assert_eq!(
        apply(
            &product,
            &[Value::Null, Value::Long(-3), Value::Double(0.5)],
            None
        )
        .unwrap(),
        Aggregated {
            tick: 100,
            location: 11,
            elevation: 21,
            value: Value::Double(-1.5),
        }
    );
    assert_eq!(
        apply(&product, &[Value::Null], None).unwrap().value,
        Value::Long(1)
    );
    assert_eq!(
        apply(&product, &longs(&[-2, 3]), None).unwrap().value,
        Value::Long(-6)
    );
    assert_eq!(
        apply(&product, &[Value::Double(-2.0), Value::Double(-0.25)], None)
            .unwrap()
            .value,
        Value::Double(0.5)
    );
    assert_eq!(
        apply(
            &product,
            &[Value::Double(f64::MIN_POSITIVE / 4.0), Value::Long(3)],
            None
        )
        .unwrap()
        .value,
        Value::Double(f64::MIN_POSITIVE / 4.0 * 3.0)
    );
    assert_eq!(
        Window::of(&[]).apply(&product).unwrap(),
        Aggregated {
            tick: 100,
            location: NO_LOCATION,
            elevation: NO_ELEVATION,
            value: Value::Null,
        }
    );
    assert_eq!(
        apply(&product, &longs(&[2]), parameters(None))
            .unwrap_err()
            .to_string(),
        "Index 4 out of bounds for length 4"
    );
    assert_eq!(
        apply(&product, &longs(&[2, 3]), parameters(Some(1))).unwrap(),
        Aggregated {
            tick: 100,
            location: 12,
            elevation: 22,
            value: Value::Long(6),
        }
    );
}

#[test]
fn exact_rounding() {
    let exact = |words: Vec<u64>, exponent| Exact { words, exponent }.to_f64();
    assert_eq!(exact(vec![0, 0], 0), 0.0);
    assert_eq!(exact(vec![u64::MAX], 0), 18_446_744_073_709_551_615.0);
    assert_eq!(exact(vec![3], -1), 1.5);
    // Below the 64 kept bits: the rest of the next word, then the words beneath.
    assert_eq!(exact(vec![1, 0, 1], 0), 2f64.powi(128));
    assert_eq!(exact(vec![1, 1 << 52], 0), 2f64.powi(116));
    assert_eq!(
        exact(vec![1, 1 << 52, 1], 0),
        2f64.powi(128) + 2f64.powi(116)
    );
    assert_eq!(
        exact(vec![(1 << 63) + 1, 1 << 52], 0),
        2f64.powi(116) + 2f64.powi(64)
    );
    assert_eq!(exact(vec![0, 1], -64), 1.0);
    let mut product = Exact {
        words: vec![1],
        exponent: 0,
    };
    product.multiply(u64::MAX, 1);
    product.multiply(u64::MAX, -2);
    assert_eq!(product.words, vec![1, u64::MAX - 1]);
    assert_eq!(product.exponent, -1);
}

#[test]
fn dot_products() {
    for (datapoints, parameters, expected) in [
        (
            SERIES,
            "[ 1.0 2.0 ] mapper.dotproduct 1 0 0",
            "[[2,3],[8.0,11.0]]",
        ),
        (
            SERIES,
            "[ 0.5 -1.0 ] mapper.dotproduct.tanh 1 0 0",
            "[[2,3],[-0.9640275800758169,-0.9866142981514303]]",
        ),
        (
            SERIES,
            "[ 0.5 -1.0 ] mapper.dotproduct.sigmoid 1 0 0",
            "[[2,3],[0.11920292202211755,0.07585818002124355]]",
        ),
        (
            SERIES,
            "[ 0.5 -1.0 ] mapper.dotproduct.positive 1 0 0",
            "[[2,3],[0.0,0.0]]",
        ),
        (
            SERIES,
            "[ 0.5 1.0 ] mapper.dotproduct.positive 1 0 0",
            "[[2,3],[4.0,5.5]]",
        ),
        (
            LOCATED,
            "[ 1.0 1.0 ] mapper.dotproduct 0 2 0",
            "[[1],[5.0]]",
        ),
        (
            LOCATED,
            "[ 1.0 1.0 ] mapper.dotproduct 1 0 [ 3 ]",
            "[[3],[5.0]]",
        ),
    ] {
        assert_top(&mapped(datapoints, parameters), expected);
    }
    for (datapoints, vector) in [
        (
            "1 NaN NaN NaN 'a' ADDVALUE 2 NaN NaN NaN 'b' ADDVALUE",
            "[ 1.0 1.0 ]",
        ),
        ("1 NaN NaN NaN true ADDVALUE", "[ 1.0 ]"),
    ] {
        assert_fails(
            &mapped(datapoints, &format!("{vector} mapper.dotproduct 1 0 0")),
            "MAP was given invalid parameters. (mapper.dotproduct can only be applied to LONG or DOUBLE values.)",
        );
    }
    let message = fail(&mapped(LOCATED, "[ 1.0 ] mapper.dotproduct 1 0 [ 2 ]")).to_string();
    assert!(
        message.contains("(Index -1 out of bounds for length 1)"),
        "{message}"
    );
    let dot = DotProduct {
        name: "mapper.x",
        omega: vec![1.0, 2.0],
        activation: Activation::Identity,
    };
    assert_eq!(dot.roles(), Roles::MAPPER);
    assert_eq!(
        apply(&dot, &longs(&[1, 2]), None).unwrap_err().to_string(),
        "Index 7 out of bounds for length 7"
    );
    // Only the first value is checked; a later one fails the cast to a `Number`.
    assert_eq!(
        apply(
            &dot,
            &[Value::Long(1), Value::string("a")],
            parameters(Some(0))
        )
        .unwrap_err()
        .to_string(),
        "class java.lang.String cannot be cast to class java.lang.Number"
    );
    assert_eq!(
        Window::of(&longs(&[1])).apply(&dot).unwrap().value,
        Value::Null
    );
    assert_eq!(Window::of(&[]).apply(&dot).unwrap().value, Value::Null);
}

#[test]
fn builders() {
    for (script, snapshot) in [
        (
            "[ 1.0 2.5 ] mapper.dotproduct.tanh",
            "[ 1.0 2.5 ] mapper.dotproduct.tanh ",
        ),
        ("[ ] mapper.dotproduct", "[ ] mapper.dotproduct "),
        (
            "[ -0.0 ] mapper.dotproduct.sigmoid",
            "[ -0.0 ] mapper.dotproduct.sigmoid ",
        ),
        (
            "[ 1.0 ] mapper.dotproduct.positive",
            "[ 1.0 ] mapper.dotproduct.positive ",
        ),
        ("mapper.product", "'mapper.product' FUNCREF "),
        ("reducer.product", "'reducer.product' FUNCREF "),
    ] {
        assert_top(&format!("{script} SNAPSHOT"), snapshot);
        assert_top(&format!("{script} TYPEOF"), "AGGREGATOR");
    }
    assert_fails(
        "[ 1 2 ] mapper.dotproduct",
        "class java.lang.Long cannot be cast to class java.lang.Double",
    );
    assert_fails(
        "[ 1.0 2 ] mapper.dotproduct.tanh",
        "class java.lang.Long cannot be cast to class java.lang.Double",
    );
    assert_fails("[ NULL ] mapper.dotproduct", "NullPointerException");
    assert_fails(
        "1 mapper.dotproduct",
        "mapper.dotproduct expects an input vector (list of doubles) on top of the stack.",
    );
    assert_fails("mapper.dotproduct", "Empty stack");
    for mapper in ["mapper.product", "[ 1.0 ] mapper.dotproduct"] {
        assert_fails(
            &format!("[ NEWGTS 1 NaN NaN NaN 2 ADDVALUE {mapper} 0 10 0 ] BUCKETIZE"),
            "BUCKETIZE expects a bucketizer function, a macro, or NULL as fourth to last parameter.",
        );
    }
}
