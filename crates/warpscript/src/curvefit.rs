// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Apache Commons Math 3.6.1 (Copyright 2001-2016 The Apache Software Foundation, Apache-2.0), ported to Rust and modified.

//! Weighted least-squares polynomial fitting: Commons Math's `PolynomialCurveFitter`, i.e.
//! `AbstractCurveFitter` solved by `LevenbergMarquardtOptimizer` with its default settings
//! (a port of MINPACK's `lmder`). Every floating point operation is performed in the same
//! order as in Java, so that the coefficients are bit-identical to Warp 10's (`POLYFIT`,
//! `RLOWESS` with a degree above one).

// The index loops mirror those of Commons Math (and MINPACK) one for one, so that the
// floating point operations happen in the same order.
#![allow(clippy::needless_range_loop, clippy::many_single_char_names)]

use crate::error::{Error, Result};
use crate::jmath;

/// `Precision.SAFE_MIN`: 2⁻¹⁰²².
const SAFE_MIN: f64 = f64::MIN_POSITIVE;
/// The default `initialStepBoundFactor`.
const INITIAL_STEP_BOUND_FACTOR: f64 = 100.0;
/// The default `costRelativeTolerance`.
const COST_RELATIVE_TOLERANCE: f64 = 1e-10;
/// The default `parRelativeTolerance`.
const PAR_RELATIVE_TOLERANCE: f64 = 1e-10;
/// The default `orthoTolerance`.
const ORTHO_TOLERANCE: f64 = 1e-10;
/// The default `qrRankingThreshold`.
const QR_RANKING_THRESHOLD: f64 = SAFE_MIN;

/// A weighted observation (`WeightedObservedPoint`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeightedPoint {
    /// The weight of the observation.
    pub weight: f64,
    /// The abscissa.
    pub x: f64,
    /// The observed value.
    pub y: f64,
}

/// `MessageFormat` rendering of a count (`Locale.US`: grouped by thousands).
fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A Commons Math exception.
fn math_error(class: &'static str, message: String) -> Error {
    Error::runtime(class, message)
}

/// `TooManyIterationsException`.
fn too_many_iterations(max: i32) -> Error {
    math_error(
        "org.apache.commons.math3.exception.TooManyIterationsException",
        format!(
            "illegal state: maximal count ({}) exceeded: iterations",
            grouped(u64::from(max.unsigned_abs()))
        ),
    )
}

/// The weighted least-squares problem of a polynomial of `degree` through the points.
struct Problem {
    x: Vec<f64>,
    target: Vec<f64>,
    /// The square roots of the weights (`LeastSquaresFactory.squareRoot` of the diagonal).
    sqrt_weights: Vec<f64>,
}

/// `PolynomialFunction.evaluate`: Horner's scheme from the highest coefficient (there is at
/// least one).
pub fn polynomial_value(coefficients: &[f64], x: f64) -> f64 {
    let n = coefficients.len();
    let mut result = coefficients[n - 1];
    for j in (0..n - 1).rev() {
        result = x * result + coefficients[j];
    }
    result
}

impl Problem {
    /// The weighted residuals `√w (target - model)` (`DenseWeightedEvaluation`).
    fn residuals(&self, point: &[f64]) -> Vec<f64> {
        self.x
            .iter()
            .zip(&self.target)
            .zip(&self.sqrt_weights)
            .map(|((&x, &target), &w)| w * (target - polynomial_value(point, x)))
            .collect()
    }

    /// The weighted Jacobian of the model, negated (`qrDecomposition` works on
    /// `jacobian.scalarMultiply(-1)`): `-√w xᵏ`.
    fn negated_jacobian(&self, n_parameters: usize) -> Vec<Vec<f64>> {
        self.x
            .iter()
            .zip(&self.sqrt_weights)
            .map(|(&x, &w)| {
                let mut row = Vec::with_capacity(n_parameters);
                let mut xn = 1.0;
                for _ in 0..n_parameters {
                    row.push(-(w * xn));
                    xn *= x;
                }
                row
            })
            .collect()
    }
}

/// `AbstractEvaluation.getCost`: the Euclidean norm of the residuals.
fn cost(residuals: &[f64]) -> f64 {
    let mut dot = 0.0;
    for r in residuals {
        dot += r * r;
    }
    dot.sqrt()
}

/// The QR decomposition (with column pivoting) of the Jacobian (`InternalData`).
struct Qr {
    /// The Householder vectors below the diagonal, `R` above.
    weighted_jacobian: Vec<Vec<f64>>,
    permutation: Vec<usize>,
    rank: usize,
    diag_r: Vec<f64>,
    jac_norm: Vec<f64>,
    beta: Vec<f64>,
}

/// `LevenbergMarquardtOptimizer.qrDecomposition`.
fn qr_decomposition(mut a: Vec<Vec<f64>>, n_c: usize, solved_cols: usize) -> Result<Qr> {
    let n_r = a.len();
    let mut permutation: Vec<usize> = (0..n_c).collect();
    let mut diag_r = vec![0.0; n_c];
    let mut jac_norm = vec![0.0; n_c];
    let mut beta = vec![0.0; n_c];
    for (k, norm) in jac_norm.iter_mut().enumerate() {
        let mut norm2 = 0.0;
        for row in &a {
            norm2 += row[k] * row[k];
        }
        *norm = norm2.sqrt();
    }
    for k in 0..n_c {
        let mut next_column = 0;
        let mut ak2 = f64::NEG_INFINITY;
        for i in k..n_c {
            let mut norm2 = 0.0;
            for row in &a[k..] {
                let aki = row[permutation[i]];
                norm2 += aki * aki;
            }
            if !norm2.is_finite() {
                return Err(math_error(
                    "org.apache.commons.math3.exception.ConvergenceException",
                    format!(
                        "illegal state: unable to perform Q.R decomposition on the {}x{} jacobian matrix",
                        grouped(n_r as u64),
                        grouped(n_c as u64)
                    ),
                ));
            }
            if norm2 > ak2 {
                next_column = i;
                ak2 = norm2;
            }
        }
        if ak2 <= QR_RANKING_THRESHOLD {
            return Ok(Qr {
                weighted_jacobian: a,
                permutation,
                rank: k,
                diag_r,
                jac_norm,
                beta,
            });
        }
        let pk = permutation[next_column];
        permutation.swap(next_column, k);
        let akk = a[k][pk];
        let alpha = if akk > 0.0 { -ak2.sqrt() } else { ak2.sqrt() };
        let betak = 1.0 / (ak2 - akk * alpha);
        beta[pk] = betak;
        diag_r[pk] = alpha;
        a[k][pk] -= alpha;
        for dk in (1..n_c - k).rev() {
            let pkdk = permutation[k + dk];
            let mut gamma = 0.0;
            for row in &a[k..] {
                gamma += row[pk] * row[pkdk];
            }
            gamma *= betak;
            for row in &mut a[k..] {
                row[pkdk] -= gamma * row[pk];
            }
        }
    }
    Ok(Qr {
        weighted_jacobian: a,
        permutation,
        rank: solved_cols,
        diag_r,
        jac_norm,
        beta,
    })
}

/// `LevenbergMarquardtOptimizer.qTy`: apply the Householder reflections to `y`.
fn q_ty(y: &mut [f64], qr: &Qr) {
    let a = &qr.weighted_jacobian;
    for (k, &pk) in qr.permutation.iter().enumerate() {
        let mut gamma = 0.0;
        for (row, yi) in a.iter().zip(y.iter()).skip(k) {
            gamma += row[pk] * yi;
        }
        gamma *= qr.beta[pk];
        for (row, yi) in a.iter().zip(y.iter_mut()).skip(k) {
            *yi -= gamma * row[pk];
        }
    }
}

/// The work arrays of `determineLMParameter` / `determineLMDirection`.
struct Work {
    work1: Vec<f64>,
    work2: Vec<f64>,
    work3: Vec<f64>,
    lm_dir: Vec<f64>,
}

/// `LevenbergMarquardtOptimizer.determineLMDirection`: solve the regularized system by Givens
/// rotations; `diag` is `work1`, `lm_diag` is `work2`, `work` is `work3`.
fn determine_lm_direction(qy: &[f64], qr: &mut Qr, solved_cols: usize, w: &mut Work) {
    let permutation = &qr.permutation;
    let a = &mut qr.weighted_jacobian;
    let diag = &w.work1;
    let lm_diag = &mut w.work2;
    let work = &mut w.work3;
    let lm_dir = &mut w.lm_dir;
    for j in 0..solved_cols {
        let pj = permutation[j];
        for i in j + 1..solved_cols {
            a[i][pj] = a[j][permutation[i]];
        }
        lm_dir[j] = qr.diag_r[pj];
        work[j] = qy[j];
    }
    for j in 0..solved_cols {
        let pj = permutation[j];
        let dpj = diag[pj];
        // The diagonal below is reset unless the regularization term underflowed to zero
        // (branch-free: an empty range then).
        let reset_from = (j + 1).max(lm_diag.len() * usize::from(dpj == 0.0));
        lm_diag[reset_from..].fill(0.0);
        lm_diag[j] = dpj;
        let mut qtbpj = 0.0;
        for k in j..solved_cols {
            let pk = permutation[k];
            if lm_diag[k] != 0.0 {
                let rkk = a[k][pk];
                let (sin, cos) = if rkk.abs() < lm_diag[k].abs() {
                    let cotan = rkk / lm_diag[k];
                    let sin = 1.0 / (1.0 + cotan * cotan).sqrt();
                    (sin, sin * cotan)
                } else {
                    let tan = lm_diag[k] / rkk;
                    let cos = 1.0 / (1.0 + tan * tan).sqrt();
                    (cos * tan, cos)
                };
                a[k][pk] = cos * rkk + sin * lm_diag[k];
                let temp = cos * work[k] + sin * qtbpj;
                qtbpj = -sin * work[k] + cos * qtbpj;
                work[k] = temp;
                for i in k + 1..solved_cols {
                    let rik = a[i][pk];
                    let temp2 = cos * rik + sin * lm_diag[i];
                    lm_diag[i] = -sin * rik + cos * lm_diag[i];
                    a[i][pk] = temp2;
                }
            }
        }
        lm_diag[j] = a[j][permutation[j]];
        a[j][permutation[j]] = lm_dir[j];
    }
    // A singular system is solved on its leading non singular part.
    let n_sing = lm_diag[..solved_cols]
        .iter()
        .position(|&v| v == 0.0)
        .unwrap_or(solved_cols);
    work[n_sing..solved_cols].fill(0.0);
    for j in (0..n_sing).rev() {
        let pj = permutation[j];
        let mut sum = 0.0;
        for i in j + 1..n_sing {
            sum += a[i][pj] * work[i];
        }
        work[j] = (work[j] - sum) / lm_diag[j];
    }
    for j in 0..lm_dir.len() {
        lm_dir[permutation[j]] = work[j];
    }
}

/// `LevenbergMarquardtOptimizer.determineLMParameter`: the Levenberg-Marquardt parameter
/// whose step stays within `delta`, the step being left in `w.lm_dir`.
fn determine_lm_parameter(
    qy: &[f64],
    delta: f64,
    diag: &[f64],
    qr: &mut Qr,
    solved_cols: usize,
    w: &mut Work,
    mut lm_par: f64,
) -> f64 {
    let n_c = qr.weighted_jacobian[0].len();
    let rank = qr.rank;
    {
        let permutation = &qr.permutation;
        let a = &qr.weighted_jacobian;
        for j in 0..rank {
            w.lm_dir[permutation[j]] = qy[j];
        }
        for j in rank..n_c {
            w.lm_dir[permutation[j]] = 0.0;
        }
        for k in (0..rank).rev() {
            let pk = permutation[k];
            let ypk = w.lm_dir[pk] / qr.diag_r[pk];
            for i in 0..k {
                w.lm_dir[permutation[i]] -= ypk * a[i][pk];
            }
            w.lm_dir[pk] = ypk;
        }
    }
    let mut dx_norm = 0.0;
    for j in 0..solved_cols {
        let pj = qr.permutation[j];
        let s = diag[pj] * w.lm_dir[pj];
        w.work1[pj] = s;
        dx_norm += s * s;
    }
    dx_norm = dx_norm.sqrt();
    let mut fp = dx_norm - delta;
    if fp <= 0.1 * delta {
        return 0.0;
    }
    let mut parl = 0.0;
    if rank == solved_cols {
        let permutation = &qr.permutation;
        let a = &qr.weighted_jacobian;
        for &pj in &permutation[..solved_cols] {
            w.work1[pj] *= diag[pj] / dx_norm;
        }
        let mut sum2 = 0.0;
        for j in 0..solved_cols {
            let pj = permutation[j];
            let mut sum = 0.0;
            for i in 0..j {
                sum += a[i][pj] * w.work1[permutation[i]];
            }
            let s = (w.work1[pj] - sum) / qr.diag_r[pj];
            w.work1[pj] = s;
            sum2 += s * s;
        }
        parl = fp / (delta * sum2);
    }
    let mut sum2 = 0.0;
    for j in 0..solved_cols {
        let pj = qr.permutation[j];
        let mut sum = 0.0;
        for i in 0..=j {
            sum += qr.weighted_jacobian[i][pj] * qy[i];
        }
        sum /= diag[pj];
        sum2 += sum * sum;
    }
    let g_norm = sum2.sqrt();
    let mut paru = g_norm / delta;
    if paru == 0.0 {
        paru = SAFE_MIN / jmath::min(delta, 0.1);
    }
    lm_par = jmath::min(paru, jmath::max(lm_par, parl));
    if lm_par == 0.0 {
        lm_par = g_norm / dx_norm;
    }
    for _countdown in 0..=10 {
        if lm_par == 0.0 {
            lm_par = jmath::max(SAFE_MIN, 0.001 * paru);
        }
        let s_par = lm_par.sqrt();
        for j in 0..solved_cols {
            let pj = qr.permutation[j];
            w.work1[pj] = s_par * diag[pj];
        }
        determine_lm_direction(qy, qr, solved_cols, w);
        dx_norm = 0.0;
        for j in 0..solved_cols {
            let pj = qr.permutation[j];
            let s = diag[pj] * w.lm_dir[pj];
            w.work3[pj] = s;
            dx_norm += s * s;
        }
        dx_norm = dx_norm.sqrt();
        let previous_fp = fp;
        fp = dx_norm - delta;
        if fp.abs() <= 0.1 * delta || (parl == 0.0 && fp <= previous_fp && previous_fp < 0.0) {
            return lm_par;
        }
        let permutation = &qr.permutation;
        let a = &qr.weighted_jacobian;
        for &pj in &permutation[..solved_cols] {
            w.work1[pj] = w.work3[pj] * diag[pj] / dx_norm;
        }
        for j in 0..solved_cols {
            let pj = permutation[j];
            w.work1[pj] /= w.work2[j];
            let tmp = w.work1[pj];
            for i in j + 1..solved_cols {
                w.work1[permutation[i]] -= a[i][pj] * tmp;
            }
        }
        let mut sum2 = 0.0;
        for &pj in &permutation[..solved_cols] {
            let s = w.work1[pj];
            sum2 += s * s;
        }
        let correction = fp / (delta * sum2);
        if fp > 0.0 {
            parl = jmath::max(parl, lm_par);
        } else if fp < 0.0 {
            paru = jmath::min(paru, lm_par);
        }
        lm_par = jmath::max(parl, lm_par + correction);
    }
    lm_par
}

/// `LevenbergMarquardtOptimizer.optimize` from the zero start point, at most
/// `max_iterations` iterations (the evaluations are unlimited, there is no convergence
/// checker). Returns the optimal point.
fn optimize(problem: &Problem, n_c: usize, max_iterations: i32) -> Result<Vec<f64>> {
    let n_r = problem.x.len();
    let solved_cols = n_r.min(n_c);
    let mut w = Work {
        work1: vec![0.0; n_c],
        work2: vec![0.0; n_c],
        work3: vec![0.0; n_c],
        lm_dir: vec![0.0; n_c],
    };
    let mut lm_par = 0.0;
    let mut delta = 0.0;
    let mut x_norm = 0.0;
    let mut diag = vec![0.0; n_c];
    let mut old_x = vec![0.0; n_c];
    let mut qtf = vec![0.0; n_r];
    let mut current_point = vec![0.0; n_c];
    let mut current_residuals = problem.residuals(&current_point);
    let mut current_cost = cost(&current_residuals);
    let mut first_iteration = true;
    let mut iterations = 0i32;
    let mut previous_state = Vec::new();
    loop {
        iterations = iterations.saturating_add(1);
        // The whole state an iteration starts from: when it repeats (non finite costs never
        // improve), Commons Math iterates until the counter is exhausted, reported at once.
        let state: Vec<u64> = [
            current_cost,
            lm_par,
            delta,
            x_norm,
            f64::from(u8::from(first_iteration)),
        ]
        .iter()
        .chain(&current_point)
        .chain(&diag)
        .chain(&w.work1)
        .chain(&w.work2)
        .chain(&w.work3)
        .chain(&w.lm_dir)
        .map(|v| v.to_bits())
        .collect();
        if iterations > max_iterations || state == previous_state {
            return Err(too_many_iterations(max_iterations));
        }
        previous_state = state;
        let previous_point = current_point.clone();
        let previous_residuals = current_residuals.clone();
        let mut qr = qr_decomposition(problem.negated_jacobian(n_c), n_c, solved_cols)?;
        qtf.copy_from_slice(&current_residuals);
        q_ty(&mut qtf, &qr);
        for k in 0..solved_cols {
            let pk = qr.permutation[k];
            qr.weighted_jacobian[k][pk] = qr.diag_r[pk];
        }
        if first_iteration {
            // Until a step is accepted the point is still the zero start point (the columns
            // are finite, or the decomposition failed), so its scaled norm is zero and the
            // initial step bound is the bare factor.
            x_norm = 0.0;
            for k in 0..n_c {
                let dk = qr.jac_norm[k];
                diag[k] = if dk == 0.0 { 1.0 } else { dk };
            }
            delta = INITIAL_STEP_BOUND_FACTOR;
        }
        let mut max_cosine = 0.0;
        if current_cost != 0.0 {
            for j in 0..solved_cols {
                let pj = qr.permutation[j];
                let s = qr.jac_norm[pj];
                if s != 0.0 {
                    let mut sum = 0.0;
                    for i in 0..=j {
                        sum += qr.weighted_jacobian[i][pj] * qtf[i];
                    }
                    max_cosine = jmath::max(max_cosine, sum.abs() / (s * current_cost));
                }
            }
        }
        if max_cosine <= ORTHO_TOLERANCE {
            return Ok(current_point);
        }
        for j in 0..n_c {
            diag[j] = jmath::max(diag[j], qr.jac_norm[j]);
        }
        let mut ratio = 0.0;
        while ratio < 1.0e-4 {
            for j in 0..solved_cols {
                let pj = qr.permutation[j];
                old_x[pj] = current_point[pj];
            }
            let previous_cost = current_cost;
            lm_par =
                determine_lm_parameter(&qtf, delta, &diag, &mut qr, solved_cols, &mut w, lm_par);
            let mut lm_norm = 0.0;
            for j in 0..solved_cols {
                let pj = qr.permutation[j];
                w.lm_dir[pj] = -w.lm_dir[pj];
                current_point[pj] = old_x[pj] + w.lm_dir[pj];
                let s = diag[pj] * w.lm_dir[pj];
                lm_norm += s * s;
            }
            lm_norm = lm_norm.sqrt();
            if first_iteration {
                delta = jmath::min(delta, lm_norm);
            }
            current_residuals = problem.residuals(&current_point);
            current_cost = cost(&current_residuals);
            let mut act_red = -1.0;
            if 0.1 * current_cost < previous_cost {
                let r = current_cost / previous_cost;
                act_red = 1.0 - r * r;
            }
            for j in 0..solved_cols {
                let pj = qr.permutation[j];
                let dir_j = w.lm_dir[pj];
                w.work1[j] = 0.0;
                for i in 0..=j {
                    w.work1[i] += qr.weighted_jacobian[i][pj] * dir_j;
                }
            }
            let mut coeff1 = 0.0;
            for j in 0..solved_cols {
                coeff1 += w.work1[j] * w.work1[j];
            }
            let pc2 = previous_cost * previous_cost;
            coeff1 /= pc2;
            let coeff2 = lm_par * lm_norm * lm_norm / pc2;
            let pre_red = coeff1 + 2.0 * coeff2;
            let dir_der = -(coeff1 + coeff2);
            ratio = if pre_red == 0.0 {
                0.0
            } else {
                act_red / pre_red
            };
            if ratio <= 0.25 {
                let mut tmp = if act_red < 0.0 {
                    0.5 * dir_der / (dir_der + 0.5 * act_red)
                } else {
                    0.5
                };
                if 0.1 * current_cost >= previous_cost || tmp < 0.1 {
                    tmp = 0.1;
                }
                delta = tmp * jmath::min(delta, 10.0 * lm_norm);
                lm_par /= tmp;
            } else if lm_par == 0.0 || ratio >= 0.75 {
                delta = 2.0 * lm_norm;
                lm_par *= 0.5;
            }
            if ratio >= 1.0e-4 {
                first_iteration = false;
                x_norm = 0.0;
                for k in 0..n_c {
                    let x_k = diag[k] * current_point[k];
                    x_norm += x_k * x_k;
                }
                x_norm = x_norm.sqrt();
            } else {
                // Back to the previous evaluation.
                current_cost = previous_cost;
                current_point.clone_from(&previous_point);
                current_residuals.clone_from(&previous_residuals);
            }
            if (act_red.abs() <= COST_RELATIVE_TOLERANCE
                && pre_red <= COST_RELATIVE_TOLERANCE
                && ratio <= 2.0)
                || delta <= PAR_RELATIVE_TOLERANCE * x_norm
            {
                return Ok(current_point);
            }
            // Commons Math then gives up when the tolerances are below what the precision
            // allows (`ConvergenceException`s), which cannot happen with the defaults: every
            // state they catch was accepted by the convergence test above.
        }
    }
}

/// `PolynomialCurveFitter.create(degree)`: the number of coefficients of a polynomial of
/// `degree` (the size of the zero initial guess, which Java fails to allocate for a degree
/// below `-1`).
pub fn coefficient_count(degree: i32) -> Result<usize> {
    let n_parameters = degree.wrapping_add(1);
    usize::try_from(n_parameters).map_err(|_| {
        Error::runtime(
            "java.lang.NegativeArraySizeException",
            n_parameters.to_string(),
        )
    })
}

/// `PolynomialCurveFitter.withMaxIterations(max_iterations).fit(points)` with
/// `n_parameters` coefficients (see [`coefficient_count`]): the coefficients of the weighted
/// least-squares polynomial, constant term first.
pub fn polynomial_fit(
    points: &[WeightedPoint],
    n_parameters: usize,
    max_iterations: i32,
) -> Result<Vec<f64>> {
    // The square root of the empty weight matrix cannot be allocated.
    if points.is_empty() {
        return Err(math_error(
            "org.apache.commons.math3.exception.NotStrictlyPositiveException",
            "0 is smaller than, or equal to, the minimum (0)".to_owned(),
        ));
    }
    if n_parameters == 0 {
        return Err(math_error(
            "org.apache.commons.math3.exception.NoDataException",
            "empty polynomials coefficients array".to_owned(),
        ));
    }
    let problem = Problem {
        x: points.iter().map(|p| p.x).collect(),
        target: points.iter().map(|p| p.y).collect(),
        sqrt_weights: points.iter().map(|p| p.weight.sqrt()).collect(),
    };
    optimize(&problem, n_parameters, max_iterations)
}
