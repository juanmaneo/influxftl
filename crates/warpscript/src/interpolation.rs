// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Apache Commons Math 3.6.1 (Copyright 2001-2016 The Apache Software Foundation, Apache-2.0), ported to Rust and modified.

//! The univariate interpolators of Commons Math 3 (`analysis.interpolation`) the fillers of
//! `FILL` rely on: linear, natural cubic spline, Akima spline, divided differences (Newton
//! form) and LOESS, with the piecewise polynomial functions they build.
//!
//! The algorithms are reproduced operation for operation so that the interpolated values are
//! bit-identical to Warp 10's. The failures are the Commons Math exceptions Warp 10 lets
//! through, with their (en_US) messages. The argument checks the callers make impossible
//! (arrays of different lengths, empty arrays, out of range evaluation points) are left out.

use crate::error::{Error, Result};

const NON_MONOTONIC: &str = "org.apache.commons.math3.exception.NonMonotonicSequenceException";
const TOO_SMALL: &str = "org.apache.commons.math3.exception.NumberIsTooSmallException";
const NOT_FINITE: &str = "org.apache.commons.math3.exception.NotFiniteNumberException";

/// A piecewise polynomial function (`PolynomialSplineFunction`): polynomial `i`, evaluated
/// at `x - knots[i]`, applies on `[knots[i], knots[i + 1]]`.
#[derive(Clone, Debug)]
pub struct PolynomialSplineFunction {
    knots: Vec<f64>,
    polynomials: Vec<Vec<f64>>,
}

impl PolynomialSplineFunction {
    /// `isValidPoint`: whether `x` lies between the first and the last knots.
    pub fn is_valid_point(&self, x: f64) -> bool {
        !(x < self.knots[0] || x > self.knots[self.knots.len() - 1])
    }

    /// `value`: the value at `x`, which must be a valid point. The polynomial is the one of
    /// the last knot not greater than `x` (the last polynomial for the last knot), as found
    /// by the binary search of Commons Math on the strictly increasing knots.
    pub fn value(&self, x: f64) -> f64 {
        let i = self
            .knots
            .partition_point(|knot| *knot <= x)
            .saturating_sub(1)
            .min(self.polynomials.len() - 1);
        horner(&self.polynomials[i], x - self.knots[i])
    }
}

/// A polynomial in Newton form (`PolynomialFunctionNewtonForm`), defined everywhere.
#[derive(Clone, Debug)]
pub struct NewtonForm {
    a: Vec<f64>,
    c: Vec<f64>,
}

impl NewtonForm {
    /// `evaluate`: the nested multiplication of the Newton form.
    pub fn value(&self, z: f64) -> f64 {
        let n = self.c.len();
        let mut value = self.a[n];
        for i in (0..n).rev() {
            value = self.a[i] + (z - self.c[i]) * value;
        }
        value
    }
}

/// `new PolynomialFunction(c)`: the coefficients without their trailing zeros (but one).
#[allow(clippy::float_cmp)]
fn polynomial(c: &[f64]) -> Vec<f64> {
    let mut n = c.len();
    while n > 1 && c[n - 1] == 0.0 {
        n -= 1;
    }
    c[..n].to_vec()
}

/// `PolynomialFunction.evaluate`: Horner's method.
fn horner(c: &[f64], x: f64) -> f64 {
    let n = c.len();
    c[..n - 1]
        .iter()
        .rev()
        .fold(c[n - 1], |result, cj| x * result + cj)
}

/// The `MessageFormat` (en_US) rendering of a number argument: digits grouped by three,
/// `∞` for infinities. Integral values only (ticks, counts and indexes), which `{}` renders
/// with the shortest digits padded with zeros like `DecimalFormat`.
pub(crate) fn message_number(d: f64) -> String {
    if d.is_nan() {
        return "NaN".to_owned();
    }
    let digits = if d.is_infinite() {
        "∞".to_owned()
    } else {
        group(&format!("{}", d.abs()))
    };
    if d.is_sign_negative() {
        format!("-{digits}")
    } else {
        digits
    }
}

/// Digits grouped by three with `,`.
fn group(digits: &str) -> String {
    let mut out = String::with_capacity(digits.len() * 4 / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `NumberIsTooSmallException` with the message pattern `pattern` (whose `{}` is `wrong`).
fn too_small(pattern: &str, wrong: usize) -> Error {
    Error::runtime(TOO_SMALL, pattern.replace("{}", &group(&wrong.to_string())))
}

/// `MathArrays.checkOrder(x)`: `x` must be strictly increasing.
pub fn check_order(x: &[f64]) -> Result<()> {
    for index in 1..x.len() {
        let (previous, wrong) = (x[index - 1], x[index]);
        if wrong <= previous {
            return Err(Error::runtime(
                NON_MONOTONIC,
                format!(
                    "points {} and {} are not strictly increasing ({} >= {})",
                    group(&(index - 1).to_string()),
                    group(&index.to_string()),
                    message_number(previous),
                    message_number(wrong)
                ),
            ));
        }
    }
    Ok(())
}

/// `LinearInterpolator.interpolate`: the piecewise linear function through the points.
pub fn linear(x: &[f64], y: &[f64]) -> Result<PolynomialSplineFunction> {
    if x.len() < 2 {
        return Err(too_small("number of points ({})", x.len()));
    }
    check_order(x)?;
    let polynomials = (0..x.len() - 1)
        .map(|i| polynomial(&[y[i], (y[i + 1] - y[i]) / (x[i + 1] - x[i])]))
        .collect();
    Ok(PolynomialSplineFunction {
        knots: x.to_vec(),
        polynomials,
    })
}

/// `SplineInterpolator.interpolate`: the natural cubic spline through the points.
#[allow(clippy::many_single_char_names)] // The names of Commons Math.
pub fn spline(x: &[f64], y: &[f64]) -> Result<PolynomialSplineFunction> {
    if x.len() < 3 {
        return Err(too_small("number of points ({})", x.len()));
    }
    let n = x.len() - 1;
    check_order(x)?;
    let h: Vec<f64> = (0..n).map(|i| x[i + 1] - x[i]).collect();
    let mut mu = vec![0.0; n];
    let mut z = vec![0.0; n + 1];
    for i in 1..n {
        let g = 2.0 * (x[i + 1] - x[i - 1]) - h[i - 1] * mu[i - 1];
        mu[i] = h[i] / g;
        z[i] = (3.0 * (y[i + 1] * h[i - 1] - y[i] * (x[i + 1] - x[i - 1]) + y[i - 1] * h[i])
            / (h[i - 1] * h[i])
            - h[i - 1] * z[i - 1])
            / g;
    }
    let mut b = vec![0.0; n];
    let mut c = vec![0.0; n + 1];
    let mut d = vec![0.0; n];
    for j in (0..n).rev() {
        c[j] = z[j] - mu[j] * c[j + 1];
        b[j] = (y[j + 1] - y[j]) / h[j] - h[j] * (c[j + 1] + 2.0 * c[j]) / 3.0;
        d[j] = (c[j + 1] - c[j]) / (3.0 * h[j]);
    }
    let polynomials = (0..n)
        .map(|i| polynomial(&[y[i], b[i], c[i], d[i]]))
        .collect();
    Ok(PolynomialSplineFunction {
        knots: x.to_vec(),
        polynomials,
    })
}

/// `AkimaSplineInterpolator.interpolate`: the Akima cubic spline through the points.
pub fn akima(x: &[f64], y: &[f64]) -> Result<PolynomialSplineFunction> {
    if x.len() < 5 {
        return Err(too_small("number of points ({})", x.len()));
    }
    check_order(x)?;
    let n = x.len();
    let differences: Vec<f64> = (0..n - 1)
        .map(|i| (y[i + 1] - y[i]) / (x[i + 1] - x[i]))
        .collect();
    let mut weights = vec![0.0; n - 1];
    for i in 1..n - 1 {
        weights[i] = (differences[i] - differences[i - 1]).abs();
    }
    let mut derivatives = vec![0.0; n];
    for i in 2..n - 2 {
        let (wp, wm) = (weights[i + 1], weights[i - 1]);
        derivatives[i] = if is_zero(wp) && is_zero(wm) {
            let (xv, xvp, xvm) = (x[i], x[i + 1], x[i - 1]);
            ((xvp - xv) * differences[i - 1] + (xv - xvm) * differences[i]) / (xvp - xvm)
        } else {
            (wp * differences[i - 1] + wm * differences[i]) / (wp + wm)
        };
    }
    derivatives[0] = differentiate_three_point(x, y, 0, 0, 1, 2);
    derivatives[1] = differentiate_three_point(x, y, 1, 0, 1, 2);
    derivatives[n - 2] = differentiate_three_point(x, y, n - 2, n - 3, n - 2, n - 1);
    derivatives[n - 1] = differentiate_three_point(x, y, n - 1, n - 3, n - 2, n - 1);
    let polynomials = (0..n - 1)
        .map(|i| {
            let w = x[i + 1] - x[i];
            let w2 = w * w;
            let (yv, yvp) = (y[i], y[i + 1]);
            let (fd, fdp) = (derivatives[i], derivatives[i + 1]);
            polynomial(&[
                yv,
                fd,
                (3.0 * (yvp - yv) / w - 2.0 * fd - fdp) / w,
                (2.0 * (yv - yvp) / w + fd + fdp) / w2,
            ])
        })
        .collect();
    Ok(PolynomialSplineFunction {
        knots: x.to_vec(),
        polynomials,
    })
}

/// `Precision.equals(w, 0.0)` (within one ulp) for a weight, which is never negative.
fn is_zero(w: f64) -> bool {
    w.to_bits() <= 1
}

/// `AkimaSplineInterpolator.differentiateThreePoint`.
#[allow(clippy::many_single_char_names)] // The names of Commons Math.
fn differentiate_three_point(
    x: &[f64],
    y: &[f64],
    at: usize,
    first: usize,
    second: usize,
    third: usize,
) -> f64 {
    let (x0, x1, x2) = (y[first], y[second], y[third]);
    let t = x[at] - x[first];
    let t1 = x[second] - x[first];
    let t2 = x[third] - x[first];
    let a = (x2 - x0 - (t2 / t1 * (x1 - x0))) / (t2 * t2 - t1 * t2);
    let b = (x1 - x0 - a * t1 * t1) / t1;
    2.0 * a * t + b
}

/// `DividedDifferenceInterpolator.interpolate`: the interpolating polynomial in Newton form.
pub fn divided_difference(x: &[f64], y: &[f64]) -> Result<NewtonForm> {
    if x.len() < 2 {
        return Err(Error::runtime(
            TOO_SMALL,
            format!(
                "2 points are required, got only {}",
                group(&x.len().to_string())
            ),
        ));
    }
    check_order(x)?;
    let n = x.len();
    let mut divdiff = y.to_vec();
    let mut a = vec![0.0; n];
    a[0] = divdiff[0];
    for i in 1..n {
        for j in 0..n - i {
            divdiff[j] = (divdiff[j + 1] - divdiff[j]) / (x[j + i] - x[j]);
        }
        a[i] = divdiff[0];
    }
    Ok(NewtonForm {
        a,
        c: x[..n - 1].to_vec(),
    })
}

/// `new LoessInterpolator(bandwidth, robustness_iters, accuracy).interpolate(x, y)`: the
/// natural cubic spline through the LOESS-smoothed points. `bandwidth` must lie in `[0, 1]`
/// and `robustness_iters` must not be negative.
pub fn loess(
    x: &[f64],
    y: &[f64],
    bandwidth: f64,
    robustness_iters: i32,
    accuracy: f64,
) -> Result<PolynomialSplineFunction> {
    spline(
        x,
        &loess_smooth(x, y, bandwidth, robustness_iters, accuracy)?,
    )
}

/// `MathUtils.checkFinite` on every value.
fn check_finite(values: &[f64]) -> Result<()> {
    match values.iter().find(|v| !v.is_finite()) {
        Some(v) => Err(Error::runtime(
            NOT_FINITE,
            format!("{} is not a finite number", message_number(*v)),
        )),
        None => Ok(()),
    }
}

/// `LoessInterpolator.smooth(x, y)` (unit weights) on at least one point.
pub fn loess_smooth(
    x: &[f64],
    y: &[f64],
    bandwidth: f64,
    robustness_iters: i32,
    accuracy: f64,
) -> Result<Vec<f64>> {
    let n = x.len();
    check_finite(x)?;
    check_finite(y)?;
    check_order(x)?;
    if n <= 2 {
        return Ok(y.to_vec());
    }
    let bandwidth_in_points = (bandwidth * n as f64) as i32;
    if bandwidth_in_points < 2 {
        return Err(Error::runtime(
            TOO_SMALL,
            format!("bandwidth ({bandwidth_in_points})"),
        ));
    }
    let mut res = vec![0.0; n];
    let mut residuals = vec![0.0; n];
    let mut robustness_weights = vec![1.0; n];
    for iter in 0..=robustness_iters {
        let (mut left, mut right) = (0, bandwidth_in_points as usize - 1);
        for i in 0..n {
            let xi = x[i];
            // `updateBandwidthInterval`, all the weights being 1.
            if i > 0 && right + 1 < n && x[right + 1] - xi < xi - x[left] {
                left += 1;
                right += 1;
            }
            let edge = if xi - x[left] > x[right] - xi {
                left
            } else {
                right
            };
            let (mut sum_weights, mut sum_x, mut sum_x_squared, mut sum_y, mut sum_xy) =
                (0.0, 0.0, 0.0, 0.0, 0.0);
            let denom = (1.0 / (x[edge] - xi)).abs();
            for k in left..=right {
                let (xk, yk) = (x[k], y[k]);
                let dist = if k < i { xi - xk } else { xk - xi };
                let w = tricube(dist * denom) * robustness_weights[k];
                let xkw = xk * w;
                sum_weights += w;
                sum_x += xkw;
                sum_x_squared += xk * xkw;
                sum_y += yk * w;
                sum_xy += yk * xkw;
            }
            let mean_x = sum_x / sum_weights;
            let mean_y = sum_y / sum_weights;
            let mean_xy = sum_xy / sum_weights;
            let mean_x_squared = sum_x_squared / sum_weights;
            let beta = if (mean_x_squared - mean_x * mean_x).abs().sqrt() < accuracy {
                0.0
            } else {
                (mean_xy - mean_x * mean_y) / (mean_x_squared - mean_x * mean_x)
            };
            let alpha = mean_y - beta * mean_x;
            res[i] = beta * xi + alpha;
            residuals[i] = (y[i] - res[i]).abs();
        }
        if iter == robustness_iters {
            break;
        }
        let mut sorted = residuals.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[n / 2];
        if median.abs() < accuracy {
            break;
        }
        for (weight, residual) in robustness_weights.iter_mut().zip(&residuals) {
            let arg = residual / (6.0 * median);
            *weight = if arg >= 1.0 {
                0.0
            } else {
                let w = 1.0 - arg * arg;
                w * w
            };
        }
    }
    Ok(res)
}

/// The tricube weight function.
fn tricube(x: f64) -> f64 {
    let abs = x.abs();
    if abs >= 1.0 {
        return 0.0;
    }
    let tmp = 1.0 - abs * abs * abs;
    tmp * tmp * tmp
}
