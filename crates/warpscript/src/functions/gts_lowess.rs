// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Local and global polynomial regression on Geo Time Series: `LOWESS` / `RLOWESS`
//! (`GTSHelper.rlowess` and `pointwise_lowess`) and `POLYFIT`.

// The single letter names are those of the Java code and of the LOWESS literature.
#![allow(clippy::many_single_char_names)]

use super::Entry;
use super::args::fail;
use super::gts::gts_or_list_try;
use crate::curvefit::{WeightedPoint, coefficient_count, polynomial_fit};
use crate::error::series::SeriesError;
use crate::error::{Error, Result};
use crate::gts::{Gts, Type, Values};
use crate::lowess::{Grid, Lowess, Samples, Smoothed, smooth};
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("LOWESS", Entry::Stack(lowess)),
    ("RLOWESS", Entry::Stack(rlowess)),
    ("POLYFIT", Entry::Stack(polyfit)),
];

/// `GTSHelper.rlowess`: the robust locally weighted regression of a numeric series, at each
/// of its ticks (each of its buckets when bucketized). The series is sorted in place.
fn rlowess_gts(gts: &Gts, params: Lowess) -> Result<Value> {
    if !matches!(gts.borrow().type_of(), Type::Long | Type::Double) {
        return Err(SeriesError::LowessNotNumeric.into());
    }
    params.validate()?;
    gts.borrow_mut().sort(false);
    let g = gts.borrow();
    let values = g.values.to_doubles();
    let samples = Samples {
        ticks: &g.ticks,
        values: &values,
    };
    let grid = g.is_bucketized().then_some(Grid {
        lastbucket: g.lastbucket,
        bucketspan: g.bucketspan,
        bucketcount: g.bucketcount,
    });
    let Smoothed {
        ticks,
        values: smoothed,
    } = smooth(samples, grid, g.bucketcount, params, None)?;
    let mut result = g.clone_empty();
    if grid.is_none_or(|grid| grid.is_filled_by(&g.ticks)) {
        result.locations.clone_from(&g.locations);
        result.elevations.clone_from(&g.elevations);
    } else if g.locations.is_some() || g.elevations.is_some() {
        // Warp 10 reads the ticks of the (empty) result to place the locations.
        return Err(Error::null_pointer());
    }
    result.ticks = ticks;
    result.values = Values::Double(smoothed);
    result.sorted = false;
    Ok(Value::Gts(result.shared()))
}

/// Pop an argument of `RLOWESS` / `LOWESS`, which must be a `LONG`.
fn pop_long(stack: &mut Stack, name: &str, message: &str) -> Result<i64> {
    match stack.pop()? {
        Value::Long(l) => Ok(l),
        _ => Err(fail(name, message)),
    }
}

/// `LOWESS`: `gts q LOWESS`, the local linear regression over the `q` nearest neighbours.
fn lowess(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "LOWESS";
    let q = pop_long(
        stack,
        NAME,
        "expects a bandwidth (a LONG) on top of the stack.",
    )? as i32;
    gts_or_list_try(stack, NAME, |gts| {
        rlowess_gts(
            gts,
            Lowess {
                q,
                r: 0,
                d: 0,
                p: 1,
            },
        )
    })
}

/// `RLOWESS`: `gts q r d p RLOWESS`, with `r` robustifying iterations, the regression
/// interpolated within `d` time units of the last one and local polynomials of degree `p`.
fn rlowess(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "RLOWESS";
    let p = pop_long(
        stack,
        NAME,
        "expects a degree (a LONG) for the polynomial fit on top of the stack.",
    )? as i32;
    let d = pop_long(
        stack,
        NAME,
        "expects a delta radius (a LONG in µs) within which LOWESS is computed only once, in 2nd position from the top of the stack.",
    )?;
    let r = pop_long(
        stack,
        NAME,
        "expects a number of robustness iteration (a LONG) in 3rd position from the top of the stack.",
    )? as i32;
    let q = pop_long(
        stack,
        NAME,
        "expects a bandwidth (a LONG) in 4th position from the top of the stack.",
    )? as i32;
    // A negative radius was meant as a fraction of the time span, but Warp 10 truncates
    // `10^d` to zero first.
    let d = d.max(0);
    gts_or_list_try(stack, NAME, |gts| rlowess_gts(gts, Lowess { q, r, d, p }))
}

/// `POLYFIT`: `gts degree (useBucketId) POLYFIT`, the coefficients (constant term first) of
/// the least-squares polynomial through the datapoints, against their ticks or their bucket
/// indexes.
fn polyfit(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "POLYFIT";
    let mut top = stack.pop()?;
    let mut use_bucket_id = false;
    if let Value::Boolean(b) = top {
        use_bucket_id = b;
        top = stack.pop()?;
    }
    let Value::Long(degree) = top else {
        return Err(fail(
            NAME,
            "expects a polynomial degree on top if the stack.",
        ));
    };
    let count = coefficient_count(degree as i32)?;
    let Value::Gts(gts) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a Geo Time Series below the degree of the polynomial.",
        ));
    };
    let g = gts.borrow().clone();
    let n = g.size();
    if n == 0 {
        return Err(fail(NAME, "operates on a non empty Geo Time Series."));
    }
    if use_bucket_id && !g.is_bucketized() {
        return Err(fail(
            NAME,
            "use of bucket id is only possible for bucketized Geo Time Series.",
        ));
    }
    if !matches!(g.type_of(), Type::Long | Type::Double) {
        return Err(fail(NAME, "can only operate on numerical Geo Time Series."));
    }
    let firsttick = g.first_tick();
    let numbers = g.values.to_doubles();
    let points: Vec<WeightedPoint> = (0..n)
        .map(|i| {
            let tick = g.ticks[i];
            let x = if use_bucket_id {
                tick.wrapping_sub(firsttick).wrapping_div(g.bucketspan) as i32 as f64
            } else {
                tick as f64
            };
            WeightedPoint {
                weight: 1.0,
                x,
                y: numbers[i],
            }
        })
        .collect();
    // Warp 10 means to limit the iterations to twice the number of datapoints, but drops the
    // fitter `withMaxIterations` returns: the limit stays `Integer.MAX_VALUE`.
    let coefficients = polynomial_fit(&points, count, i32::MAX)?;
    stack.push(Value::list(
        coefficients.into_iter().map(Value::Double).collect(),
    ));
    Ok(())
}
