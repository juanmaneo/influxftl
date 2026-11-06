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

/// `Arrays.binarySearch(double[], double)`: `Double.compare` order, `-(insertion) - 1` when
/// absent.
pub(crate) fn java_binary_search_doubles(bounds: &[f64], key: f64) -> i64 {
    let mut low = 0i64;
    let mut high = bounds.len() as i64 - 1;
    while low <= high {
        let mid = (low + high) >> 1;
        let mid_val = bounds[mid as usize];
        if mid_val < key {
            low = mid + 1;
        } else if mid_val > key {
            high = mid - 1;
        } else {
            let (mid_bits, key_bits) = (mid_val.to_bits() as i64, key.to_bits() as i64);
            match mid_bits.cmp(&key_bits) {
                std::cmp::Ordering::Equal => return mid,
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid - 1,
            }
        }
    }
    -(low + 1)
}

/// `MathArrays.linearCombination` of at least two terms (a single one is a plain product in
/// Commons Math): `Σ a[i] * b[i]` computed with twice the working precision (Ogita, Rump and
/// Oishi), the naive sum when that gives `NaN`.
pub fn linear_combination(a: &[f64], b: &[f64]) -> f64 {
    let len = a.len();
    let split = |v: f64| f64::from_bits(v.to_bits() & (u64::MAX << 27));
    let mut prod_high = vec![0.0; len];
    let mut prod_low_sum = 0.0;
    for i in 0..len {
        let (ai, bi) = (a[i], b[i]);
        let (a_high, b_high) = (split(ai), split(bi));
        let (a_low, b_low) = (ai - a_high, bi - b_high);
        prod_high[i] = ai * bi;
        let prod_low =
            a_low * b_low - (((prod_high[i] - a_high * b_high) - a_low * b_high) - a_high * b_low);
        prod_low_sum += prod_low;
    }
    let prod_high_cur = prod_high[0];
    let mut prod_high_next = prod_high[1];
    let mut s_high_prev = prod_high_cur + prod_high_next;
    let mut s_prime = s_high_prev - prod_high_next;
    let mut s_low_sum = (prod_high_next - (s_high_prev - s_prime)) + (prod_high_cur - s_prime);
    for &next in &prod_high[2..] {
        prod_high_next = next;
        let s_high_cur = s_high_prev + prod_high_next;
        s_prime = s_high_cur - prod_high_next;
        s_low_sum += (prod_high_next - (s_high_cur - s_prime)) + (s_high_prev - s_prime);
        s_high_prev = s_high_cur;
    }
    let result = s_high_prev + (prod_low_sum + s_low_sum);
    if result.is_nan() {
        a.iter().zip(b).fold(0.0, |sum, (x, y)| sum + x * y)
    } else {
        result
    }
}

/// `NoDataException`.
fn no_data() -> Error {
    Error::runtime(
        "org.apache.commons.math3.exception.NoDataException",
        "no data",
    )
}

/// `OutOfRangeException`: `{value} out of [{lo}, {hi}] range`.
fn out_of_range(value: f64, lo: f64, hi: f64) -> Error {
    Error::runtime(
        "org.apache.commons.math3.exception.OutOfRangeException",
        format!(
            "{} out of [{}, {}] range",
            message_decimal(value),
            message_decimal(lo),
            message_decimal(hi)
        ),
    )
}

/// The `MessageFormat` (en_US) rendering of a double argument: at most three fraction digits
/// (half even), digits grouped by three.
pub(crate) fn message_decimal(d: f64) -> String {
    let rounded = format!("{:.3}", d.abs());
    let rounded = rounded.trim_end_matches('0').trim_end_matches('.');
    let (int, frac) = rounded.split_once('.').unwrap_or((rounded, ""));
    let int: f64 = int.parse().unwrap_or_default();
    let mut out = message_number(int);
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    if d.is_sign_negative() && !d.is_nan() {
        out.insert(0, '-');
    }
    out
}

/// The coefficients `AINV × beta` of a spline (every term summed, as Commons Math does).
fn spline_coefficients<const N: usize>(ainv: &[[i8; N]; N], beta: &[f64; N]) -> [f64; N] {
    let mut a = [0.0; N];
    for (coefficient, row) in a.iter_mut().zip(ainv) {
        let mut result = 0.0;
        for (r, b) in row.iter().zip(beta) {
            result += f64::from(*r) * b;
        }
        *coefficient = result;
    }
    a
}

/// `BicubicInterpolatingFunction.AINV`.
#[rustfmt::skip]
const BICUBIC_AINV: [[i8; 16]; 16] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [-3, 3, 0, 0, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, -2, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, -3, 3, 0, 0, -2, -1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, -2, 0, 0, 1, 1, 0, 0],
    [-3, 0, 3, 0, 0, 0, 0, 0, -2, 0, -1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, -3, 0, 3, 0, 0, 0, 0, 0, -2, 0, -1, 0],
    [9, -9, -9, 9, 6, 3, -6, -3, 6, -6, 3, -3, 4, 2, 2, 1],
    [-6, 6, 6, -6, -3, -3, 3, 3, -4, 4, -2, 2, -2, -2, -1, -1],
    [2, 0, -2, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 2, 0, -2, 0, 0, 0, 0, 0, 1, 0, 1, 0],
    [-6, 6, 6, -6, -4, -2, 4, 2, -3, 3, -3, 3, -2, -1, -2, -1],
    [4, -4, -4, 4, 2, 2, -2, -2, 2, -2, 2, -2, 1, 1, 1, 1],
];

/// `BicubicInterpolatingFunction`: a bicubic spline on a grid.
#[derive(Clone, Debug)]
pub struct BicubicInterpolatingFunction {
    /// The abscissas.
    pub xval: Vec<f64>,
    /// The ordinates.
    pub yval: Vec<f64>,
    /// `a[j][i]` of the `BicubicFunction` of each cell.
    splines: Vec<Vec<[[f64; 4]; 4]>>,
}

impl BicubicInterpolatingFunction {
    /// `isValidPoint`.
    pub fn is_valid_point(&self, x: f64, y: f64) -> bool {
        !(x < self.xval[0]
            || x > self.xval[self.xval.len() - 1]
            || y < self.yval[0]
            || y > self.yval[self.yval.len() - 1])
    }

    /// `searchIndex`: the cell of `c`, by binary search.
    fn search_index(c: f64, val: &[f64]) -> Result<i64> {
        let n = val.len() as i64;
        let r = java_binary_search_doubles(val, c);
        if r == -1 || r == -n - 1 {
            return Err(out_of_range(c, val[0], val[val.len() - 1]));
        }
        Ok(if r < 0 {
            -r - 2
        } else if r == n - 1 {
            n - 2
        } else {
            r
        })
    }

    /// `value`: the spline of the cell at the normalized coordinates (a single row or column
    /// reads before the arrays, as Java does).
    #[allow(clippy::many_single_char_names)]
    pub fn value(&self, x: f64, y: f64) -> Result<f64> {
        let i = Self::search_index(x, &self.xval)?;
        let j = Self::search_index(y, &self.yval)?;
        if i < 0 {
            return Err(crate::error::Error::index_out_of_bounds(i, self.xval.len()));
        }
        if j < 0 {
            return Err(crate::error::Error::index_out_of_bounds(j, self.yval.len()));
        }
        let (i, j) = (i as usize, j as usize);
        let xn = (x - self.xval[i]) / (self.xval[i + 1] - self.xval[i]);
        let yn = (y - self.yval[j]) / (self.yval[j + 1] - self.yval[j]);
        // `BicubicFunction.value`: the normalized coordinates lie in [0, 1].
        let (x2, y2) = (xn * xn, yn * yn);
        let px = [1.0, xn, x2, x2 * xn];
        let py = [1.0, yn, y2, y2 * yn];
        let a = &self.splines[i][j];
        let mut result = 0.0;
        for (row, p) in a.iter().zip(px) {
            result += linear_combination(row, &py) * p;
        }
        Ok(result)
    }
}

/// Warp 10's `BicubicInterpolator.interpolate`: finite differences inside the grid, zero
/// derivatives on its border.
#[allow(clippy::many_single_char_names, clippy::needless_range_loop)]
pub fn bicubic(
    xval: &[f64],
    yval: &[f64],
    fval: &[Vec<f64>],
) -> Result<BicubicInterpolatingFunction> {
    if xval.is_empty() || yval.is_empty() {
        return Err(no_data());
    }
    check_order(xval)?;
    check_order(yval)?;
    let (x_len, y_len) = (xval.len(), yval.len());
    let mut dfdx = vec![vec![0.0; y_len]; x_len];
    let mut dfdy = vec![vec![0.0; y_len]; x_len];
    let mut d2 = vec![vec![0.0; y_len]; x_len];
    for i in 1..x_len.saturating_sub(1) {
        let delta_x = xval[i + 1] - xval[i - 1];
        for j in 1..y_len.saturating_sub(1) {
            let delta_y = yval[j + 1] - yval[j - 1];
            dfdx[i][j] = (fval[i + 1][j] - fval[i - 1][j]) / delta_x;
            dfdy[i][j] = (fval[i][j + 1] - fval[i][j - 1]) / delta_y;
            d2[i][j] = (fval[i + 1][j + 1] - fval[i + 1][j - 1] - fval[i - 1][j + 1]
                + fval[i - 1][j - 1])
                / (delta_x * delta_y);
        }
    }
    let mut splines = Vec::with_capacity(x_len - 1);
    for i in 0..x_len - 1 {
        let xr = xval[i + 1] - xval[i];
        let mut row = Vec::with_capacity(y_len - 1);
        for j in 0..y_len - 1 {
            let yr = yval[j + 1] - yval[j];
            let xryr = xr * yr;
            let beta = [
                fval[i][j],
                fval[i + 1][j],
                fval[i][j + 1],
                fval[i + 1][j + 1],
                dfdx[i][j] * xr,
                dfdx[i + 1][j] * xr,
                dfdx[i][j + 1] * xr,
                dfdx[i + 1][j + 1] * xr,
                dfdy[i][j] * yr,
                dfdy[i + 1][j] * yr,
                dfdy[i][j + 1] * yr,
                dfdy[i + 1][j + 1] * yr,
                d2[i][j] * xryr,
                d2[i + 1][j] * xryr,
                d2[i][j + 1] * xryr,
                d2[i + 1][j + 1] * xryr,
            ];
            let coeff = spline_coefficients(&BICUBIC_AINV, &beta);
            // `BicubicFunction`: `a[j][i] = coeff[i * 4 + j]`, row `a[i]` weighted by `x^i`.
            let mut a = [[0.0; 4]; 4];
            for (j, column) in a.iter_mut().enumerate() {
                for (i, cell) in column.iter_mut().enumerate() {
                    *cell = coeff[i * 4 + j];
                }
            }
            row.push(a);
        }
        splines.push(row);
    }
    Ok(BicubicInterpolatingFunction {
        xval: xval.to_vec(),
        yval: yval.to_vec(),
        splines,
    })
}

/// `TricubicInterpolatingFunction.AINV`: the coefficients of a tricubic spline from the values
/// and derivatives at the corners of its cell.
#[rustfmt::skip]
const TRICUBIC_AINV: [[i8; 64]; 64] = [
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [-3, 3, 0, 0, 0, 0, 0, 0, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, -2, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [-3, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, -3, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [9, -9, -9, 9, 0, 0, 0, 0, 6, 3, -6, -3, 0, 0, 0, 0, 6, -6, 3, -3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [-6, 6, 6, -6, 0, 0, 0, 0, -3, -3, 3, 3, 0, 0, 0, 0, -4, 4, -2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, -2, -1, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [-6, 6, 6, -6, 0, 0, 0, 0, -4, -2, 4, 2, 0, 0, 0, 0, -3, 3, -3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, -1, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [4, -4, -4, 4, 0, 0, 0, 0, 2, 2, -2, -2, 0, 0, 0, 0, 2, -2, 2, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 3, 0, 0, 0, 0, 0, 0, -2, -1, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, -2, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, -1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, -9, -9, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 3, -6, -3, 0, 0, 0, 0, 6, -6, 3, -3, 0, 0, 0, 0, 4, 2, 2, 1, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -6, 6, 6, -6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, -3, 3, 3, 0, 0, 0, 0, -4, 4, -2, 2, 0, 0, 0, 0, -2, -2, -1, -1, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -6, 6, 6, -6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -4, -2, 4, 2, 0, 0, 0, 0, -3, 3, -3, 3, 0, 0, 0, 0, -2, -1, -2, -1, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, -4, -4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, -2, -2, 0, 0, 0, 0, 2, -2, 2, -2, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0],
    [-3, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, -3, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [9, -9, 0, 0, -9, 9, 0, 0, 6, 3, 0, 0, -6, -3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, -6, 0, 0, 3, -3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 2, 0, 0, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [-6, 6, 0, 0, 6, -6, 0, 0, -3, -3, 0, 0, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -4, 4, 0, 0, -2, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, -2, 0, 0, -1, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, 0, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, 0, 0, -1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, -9, 0, 0, -9, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 3, 0, 0, -6, -3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, -6, 0, 0, 3, -3, 0, 0, 4, 2, 0, 0, 2, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -6, 6, 0, 0, 6, -6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, -3, 0, 0, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -4, 4, 0, 0, -2, 2, 0, 0, -2, -2, 0, 0, -1, -1, 0, 0],
    [9, 0, -9, 0, -9, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 3, 0, -6, 0, -3, 0, 6, 0, -6, 0, 3, 0, -3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 2, 0, 2, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 9, 0, -9, 0, -9, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 0, 3, 0, -6, 0, -3, 0, 6, 0, -6, 0, 3, 0, -3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 2, 0, 2, 0, 1, 0],
    [-27, 27, 27, -27, 27, -27, -27, 27, -18, -9, 18, 9, 18, 9, -18, -9, -18, 18, -9, 9, 18, -18, 9, -9, -18, 18, 18, -18, -9, 9, 9, -9, -12, -6, -6, -3, 12, 6, 6, 3, -12, -6, 12, 6, -6, -3, 6, 3, -12, 12, -6, 6, -6, 6, -3, 3, -8, -4, -4, -2, -4, -2, -2, -1],
    [18, -18, -18, 18, -18, 18, 18, -18, 9, 9, -9, -9, -9, -9, 9, 9, 12, -12, 6, -6, -12, 12, -6, 6, 12, -12, -12, 12, 6, -6, -6, 6, 6, 6, 3, 3, -6, -6, -3, -3, 6, 6, -6, -6, 3, 3, -3, -3, 8, -8, 4, -4, 4, -4, 2, -2, 4, 4, 2, 2, 2, 2, 1, 1],
    [-6, 0, 6, 0, 6, 0, -6, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 0, -3, 0, 3, 0, 3, 0, -4, 0, 4, 0, -2, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, -2, 0, -1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, -6, 0, 6, 0, 6, 0, -6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 0, -3, 0, 3, 0, 3, 0, -4, 0, 4, 0, -2, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, -2, 0, -1, 0, -1, 0],
    [18, -18, -18, 18, -18, 18, 18, -18, 12, 6, -12, -6, -12, -6, 12, 6, 9, -9, 9, -9, -9, 9, -9, 9, 12, -12, -12, 12, 6, -6, -6, 6, 6, 3, 6, 3, -6, -3, -6, -3, 8, 4, -8, -4, 4, 2, -4, -2, 6, -6, 6, -6, 3, -3, 3, -3, 4, 2, 4, 2, 2, 1, 2, 1],
    [-12, 12, 12, -12, 12, -12, -12, 12, -6, -6, 6, 6, 6, 6, -6, -6, -6, 6, -6, 6, 6, -6, 6, -6, -8, 8, 8, -8, -4, 4, 4, -4, -3, -3, -3, -3, 3, 3, 3, 3, -4, -4, 4, 4, -2, -2, 2, 2, -4, 4, -4, 4, -2, 2, -2, 2, -2, -2, -2, -2, -1, -1, -1, -1],
    [2, 0, 0, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [-6, 6, 0, 0, 6, -6, 0, 0, -4, -2, 0, 0, 4, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 3, 0, 0, -3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, -1, 0, 0, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [4, -4, 0, 0, -4, 4, 0, 0, 2, 2, 0, 0, -2, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, -2, 0, 0, 2, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -6, 6, 0, 0, 6, -6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -4, -2, 0, 0, 4, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3, 3, 0, 0, -3, 3, 0, 0, -2, -1, 0, 0, -2, -1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, -4, 0, 0, -4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, -2, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, -2, 0, 0, 2, -2, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0],
    [-6, 0, 6, 0, 6, 0, -6, 0, 0, 0, 0, 0, 0, 0, 0, 0, -4, 0, -2, 0, 4, 0, 2, 0, -3, 0, 3, 0, -3, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, -1, 0, -2, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, -6, 0, 6, 0, 6, 0, -6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -4, 0, -2, 0, 4, 0, 2, 0, -3, 0, 3, 0, -3, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, -2, 0, -1, 0, -2, 0, -1, 0],
    [18, -18, -18, 18, -18, 18, 18, -18, 12, 6, -12, -6, -12, -6, 12, 6, 12, -12, 6, -6, -12, 12, -6, 6, 9, -9, -9, 9, 9, -9, -9, 9, 8, 4, 4, 2, -8, -4, -4, -2, 6, 3, -6, -3, 6, 3, -6, -3, 6, -6, 3, -3, 6, -6, 3, -3, 4, 2, 2, 1, 4, 2, 2, 1],
    [-12, 12, 12, -12, 12, -12, -12, 12, -6, -6, 6, 6, 6, 6, -6, -6, -8, 8, -4, 4, 8, -8, 4, -4, -6, 6, 6, -6, -6, 6, 6, -6, -4, -4, -2, -2, 4, 4, 2, 2, -3, -3, 3, 3, -3, -3, 3, 3, -4, 4, -2, 2, -4, 4, -2, 2, -2, -2, -1, -1, -2, -2, -1, -1],
    [4, 0, -4, 0, -4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, -2, 0, -2, 0, 2, 0, -2, 0, 2, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 4, 0, -4, 0, -4, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, -2, 0, -2, 0, 2, 0, -2, 0, 2, 0, -2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0],
    [-12, 12, 12, -12, 12, -12, -12, 12, -8, -4, 8, 4, 8, 4, -8, -4, -6, 6, -6, 6, 6, -6, 6, -6, -6, 6, 6, -6, -6, 6, 6, -6, -4, -2, -4, -2, 4, 2, 4, 2, -4, -2, 4, 2, -4, -2, 4, 2, -3, 3, -3, 3, -3, 3, -3, 3, -2, -1, -2, -1, -2, -1, -2, -1],
    [8, -8, -8, 8, -8, 8, 8, -8, 4, 4, -4, -4, -4, -4, 4, 4, 4, -4, 4, -4, -4, 4, -4, 4, 4, -4, -4, 4, 4, -4, -4, 4, 2, 2, 2, 2, -2, -2, -2, -2, 2, 2, -2, -2, 2, 2, -2, -2, 2, -2, 2, -2, 2, -2, 2, -2, 1, 1, 1, 1, 1, 1, 1, 1],
];

/// `TricubicInterpolatingFunction`: a tricubic spline on a grid.
#[derive(Clone, Debug)]
pub struct TricubicInterpolatingFunction {
    /// The abscissas.
    pub xval: Vec<f64>,
    /// The ordinates.
    pub yval: Vec<f64>,
    /// The heights.
    pub zval: Vec<f64>,
    /// `a[i][j][k]` of the `TricubicFunction` of each cell.
    splines: Vec<Vec<Vec<[f64; 64]>>>,
}

impl TricubicInterpolatingFunction {
    /// `isValidPoint`.
    pub fn is_valid_point(&self, x: f64, y: f64, z: f64) -> bool {
        !(x < self.xval[0]
            || x > self.xval[self.xval.len() - 1]
            || y < self.yval[0]
            || y > self.yval[self.yval.len() - 1]
            || z < self.zval[0]
            || z > self.zval[self.zval.len() - 1])
    }

    /// `searchIndex`: the first cell whose upper bound is at least `c` (`None` for `NaN`; the
    /// points below the grid, which Commons Math also rejects, are not valid points).
    fn search_index(c: f64, val: &[f64]) -> Option<usize> {
        (1..val.len()).find(|&i| c <= val[i]).map(|i| i - 1)
    }

    /// `value`, `None` for the `OutOfRangeException` of a point outside the cells.
    #[allow(clippy::many_single_char_names)]
    pub fn value(&self, x: f64, y: f64, z: f64) -> Option<f64> {
        let i = Self::search_index(x, &self.xval)?;
        let j = Self::search_index(y, &self.yval)?;
        let k = Self::search_index(z, &self.zval)?;
        let xn = (x - self.xval[i]) / (self.xval[i + 1] - self.xval[i]);
        let yn = (y - self.yval[j]) / (self.yval[j + 1] - self.yval[j]);
        let zn = (z - self.zval[k]) / (self.zval[k + 1] - self.zval[k]);
        // `TricubicFunction.value`: the normalized coordinates lie in [0, 1].
        let powers = |v: f64| [1.0, v, v * v, v * v * v];
        let (px, py, pz) = (powers(xn), powers(yn), powers(zn));
        let a = &self.splines[i][j][k];
        let mut result = 0.0;
        for (ii, &x) in px.iter().enumerate() {
            for (jj, &y) in py.iter().enumerate() {
                for (kk, &z) in pz.iter().enumerate() {
                    result += a[ii + 4 * (jj + 4 * kk)] * x * y * z;
                }
            }
        }
        Some(result)
    }
}

/// Warp 10's `TricubicInterpolator.interpolate`: finite differences inside the grid, zero
/// derivatives on its border.
#[allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::needless_range_loop
)]
pub fn tricubic(
    xval: &[f64],
    yval: &[f64],
    zval: &[f64],
    fval: &[Vec<Vec<f64>>],
) -> Result<TricubicInterpolatingFunction> {
    if xval.is_empty() || yval.is_empty() || zval.is_empty() {
        return Err(no_data());
    }
    check_order(xval)?;
    check_order(yval)?;
    check_order(zval)?;
    let (xl, yl, zl) = (xval.len(), yval.len(), zval.len());
    let grid = || vec![vec![vec![0.0; zl]; yl]; xl];
    let (mut dx, mut dy, mut dz) = (grid(), grid(), grid());
    let (mut dxy, mut dxz, mut dyz, mut dxyz) = (grid(), grid(), grid(), grid());
    let f = fval;
    for i in 1..xl.saturating_sub(1) {
        let (ni, pi) = (i + 1, i - 1);
        let delta_x = xval[ni] - xval[pi];
        for j in 1..yl.saturating_sub(1) {
            let (nj, pj) = (j + 1, j - 1);
            let delta_y = yval[nj] - yval[pj];
            let delta_xy = delta_x * delta_y;
            for k in 1..zl.saturating_sub(1) {
                let (nk, pk) = (k + 1, k - 1);
                let delta_z = zval[nk] - zval[pk];
                dx[i][j][k] = (f[ni][j][k] - f[pi][j][k]) / delta_x;
                dy[i][j][k] = (f[i][nj][k] - f[i][pj][k]) / delta_y;
                dz[i][j][k] = (f[i][j][nk] - f[i][j][pk]) / delta_z;
                let delta_xz = delta_x * delta_z;
                let delta_yz = delta_y * delta_z;
                dxy[i][j][k] =
                    (f[ni][nj][k] - f[ni][pj][k] - f[pi][nj][k] + f[pi][pj][k]) / delta_xy;
                dxz[i][j][k] =
                    (f[ni][j][nk] - f[ni][j][pk] - f[pi][j][nk] + f[pi][j][pk]) / delta_xz;
                dyz[i][j][k] =
                    (f[i][nj][nk] - f[i][nj][pk] - f[i][pj][nk] + f[i][pj][pk]) / delta_yz;
                let delta_xyz = delta_xy * delta_z;
                dxyz[i][j][k] = (f[ni][nj][nk] - f[ni][pj][nk] - f[pi][nj][nk] + f[pi][pj][nk]
                    - f[ni][nj][pk]
                    + f[ni][pj][pk]
                    + f[pi][nj][pk]
                    - f[pi][pj][pk])
                    / delta_xyz;
            }
        }
    }
    let mut splines = Vec::with_capacity(xl - 1);
    for i in 0..xl - 1 {
        let ip1 = i + 1;
        let xr = xval[ip1] - xval[i];
        let mut plane = Vec::with_capacity(yl - 1);
        for j in 0..yl - 1 {
            let jp1 = j + 1;
            let yr = yval[jp1] - yval[j];
            let xryr = xr * yr;
            let mut line = Vec::with_capacity(zl - 1);
            for k in 0..zl - 1 {
                let kp1 = k + 1;
                let zr = zval[kp1] - zval[k];
                let xrzr = xr * zr;
                let yrzr = yr * zr;
                let xryrzr = xr * yrzr;
                let corners = |g: &Vec<Vec<Vec<f64>>>, factor: f64| {
                    [
                        g[i][j][k] * factor,
                        g[ip1][j][k] * factor,
                        g[i][jp1][k] * factor,
                        g[ip1][jp1][k] * factor,
                        g[i][j][kp1] * factor,
                        g[ip1][j][kp1] * factor,
                        g[i][jp1][kp1] * factor,
                        g[ip1][jp1][kp1] * factor,
                    ]
                };
                let values = [
                    f[i][j][k],
                    f[ip1][j][k],
                    f[i][jp1][k],
                    f[ip1][jp1][k],
                    f[i][j][kp1],
                    f[ip1][j][kp1],
                    f[i][jp1][kp1],
                    f[ip1][jp1][kp1],
                ];
                let mut beta = [0.0; 64];
                beta[..8].copy_from_slice(&values);
                beta[8..16].copy_from_slice(&corners(&dx, xr));
                beta[16..24].copy_from_slice(&corners(&dy, yr));
                beta[24..32].copy_from_slice(&corners(&dz, zr));
                beta[32..40].copy_from_slice(&corners(&dxy, xryr));
                beta[40..48].copy_from_slice(&corners(&dxz, xrzr));
                beta[48..56].copy_from_slice(&corners(&dyz, yrzr));
                beta[56..64].copy_from_slice(&corners(&dxyz, xryrzr));
                line.push(spline_coefficients(&TRICUBIC_AINV, &beta));
            }
            plane.push(line);
        }
        splines.push(plane);
    }
    Ok(TricubicInterpolatingFunction {
        xval: xval.to_vec(),
        yval: yval.to_vec(),
        zval: zval.to_vec(),
        splines,
    })
}
