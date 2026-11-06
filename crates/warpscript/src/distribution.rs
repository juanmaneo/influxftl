// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Apache Commons Math 3.6.1 (Copyright 2001-2016 The Apache Software Foundation, Apache-2.0), ported to Rust and modified.

//! Student's t distribution of Commons Math (`TDistribution`): its cumulative probability
//! (`Beta.regularizedBeta`) and its inverse (`AbstractRealDistribution`'s bracketing then
//! `BrentSolver`), as the outlier tests of Warp 10 use them.
//!
//! Only what `TDistribution` needs is ported: the regularized beta function is evaluated with
//! `b = 1/2` (or `a = 1/2` after its symmetry), so `logBeta` reduces to its branch for a
//! smallest argument below 1, and `Gamma.gamma` to arguments in `(0, 20]`. Commons Math
//! computes with `FastMath`, the port with the standard library: results may differ in the
//! last bits.

#![allow(clippy::many_single_char_names)] // the names of Commons Math

use crate::error::{Error, Result};
use crate::interpolation::message_number;

/// `Gamma.invGamma1pm1`: `1 / Γ(1 + x) - 1` for `-0.5 <= x <= 1.5`.
#[allow(clippy::excessive_precision, clippy::unreadable_literal)]
fn inv_gamma1pm1(x: f64) -> f64 {
    const A0: f64 = 0.611609510448141581788E-08;
    const A1: f64 = 0.624730830116465516210E-08;
    const B: [f64; 8] = [
        0.203610414066806987300E+00,
        0.266205348428949217746E-01,
        0.493944979382446875238E-03,
        -0.851419432440314906588E-05,
        -0.643045481779353022248E-05,
        0.992641840672773722196E-06,
        -0.607761895722825260739E-07,
        0.195755836614639731882E-09,
    ];
    const P: [f64; 7] = [
        0.6116095104481415817861E-08,
        0.6871674113067198736152E-08,
        0.6820161668496170657918E-09,
        0.4686843322948848031080E-10,
        0.1572833027710446286995E-11,
        -0.1249441572276366213222E-12,
        0.4343529937408594255178E-14,
    ];
    const Q: [f64; 4] = [
        0.3056961078365221025009E+00,
        0.5464213086042296536016E-01,
        0.4956830093825887312020E-02,
        0.2692369466186361192876E-03,
    ];
    const C: f64 = -0.422784335098467139393487909917598E+00;
    const C0: f64 = std::f64::consts::EULER_GAMMA;
    // C1 to C13.
    const CS: [f64; 13] = [
        -0.655878071520253881077019515145390E+00,
        -0.420026350340952355290039348754298E-01,
        0.166538611382291489501700795102105E+00,
        -0.421977345555443367482083012891874E-01,
        -0.962197152787697356211492167234820E-02,
        0.721894324666309954239501034044657E-02,
        -0.116516759185906511211397108401839E-02,
        -0.215241674114950972815729963053648E-03,
        0.128050282388116186153198626328164E-03,
        -0.201348547807882386556893914210218E-04,
        -0.125049348214267065734535947383309E-05,
        0.113302723198169588237412962033074E-05,
        -0.205633841697760710345015413002057E-06,
    ];
    let t = if x <= 0.5 { x } else { (x - 0.5) - 0.5 };
    // `c = C13 + t * ratio`, then Horner down to C1, then the constant term.
    let horner = |ratio: f64, last: f64| {
        let mut c = CS[12] + t * ratio;
        for coefficient in CS[..12].iter().rev() {
            c = coefficient + t * c;
        }
        last + t * c
    };
    if t < 0.0 {
        let a = A0 + t * A1;
        let mut b = B[7];
        for coefficient in B[..7].iter().rev() {
            b = coefficient + t * b;
        }
        b = 1.0 + t * b;
        let c = horner(a / b, C);
        // `t` is only negative for `x = -1/2` here (Commons Math divides by `x` for `x`
        // in `(1/2, 1)`).
        x * ((c + 0.5) + 0.5)
    } else {
        let mut p = P[6];
        for coefficient in P[..6].iter().rev() {
            p = coefficient + t * p;
        }
        let mut q = Q[3];
        for coefficient in Q[..3].iter().rev() {
            q = coefficient + t * q;
        }
        q = 1.0 + t * q;
        let c = horner(p / q, C0);
        if x > 0.5 {
            (t / x) * ((c - 0.5) - 0.5)
        } else {
            x * c
        }
    }
}

/// `Gamma.logGamma1p`: `log Γ(1 + x)` for `-0.5 <= x <= 1.5`.
fn log_gamma1p(x: f64) -> f64 {
    -inv_gamma1pm1(x).ln_1p()
}

/// `Gamma.gamma` for `0 < x <= 20`.
fn gamma(x: f64) -> f64 {
    if x >= 1.0 {
        let mut prod = 1.0;
        let mut t = x;
        while t > 2.5 {
            t -= 1.0;
            prod *= t;
        }
        prod / (1.0 + inv_gamma1pm1(t - 1.0))
    } else {
        // `x` lies in `(0, 1)`: no recursion towards `-0.5` is needed.
        1.0 / (x * (1.0 + inv_gamma1pm1(x)))
    }
}

/// `Beta.DELTA`.
#[allow(clippy::excessive_precision, clippy::unreadable_literal)]
const DELTA: [f64; 15] = [
    0.833333333333333333333333333333E-01,
    -0.277777777777777777777777752282E-04,
    0.793650793650793650791732130419E-07,
    -0.595238095238095232389839236182E-09,
    0.841750841750832853294451671990E-11,
    -0.191752691751854612334149171243E-12,
    0.641025640510325475730918472625E-14,
    -0.295506514125338232839867823991E-15,
    0.179643716359402238723287696452E-16,
    -0.139228964661627791231203060395E-17,
    0.133802855014020915603275339093E-18,
    -0.154246009867966094273710216533E-19,
    0.197701992980957427278370133333E-20,
    -0.234065664793997056856992426667E-21,
    0.171348014966398575409015466667E-22,
];

/// `Beta.deltaMinusDeltaSum(a, b)` for `0 <= a <= b` and `b >= 10`.
fn delta_minus_delta_sum(a: f64, b: f64) -> f64 {
    let h = a / b;
    let p = h / (1.0 + h);
    let q = 1.0 / (1.0 + h);
    let q2 = q * q;
    let mut s = [0.0; DELTA.len()];
    s[0] = 1.0;
    for i in 1..s.len() {
        s[i] = 1.0 + (q + q2 * s[i - 1]);
    }
    let sqrt_t = 10.0 / b;
    let t = sqrt_t * sqrt_t;
    let mut w = DELTA[DELTA.len() - 1] * s[s.len() - 1];
    for i in (0..DELTA.len() - 1).rev() {
        w = t * w + DELTA[i] * s[i];
    }
    w * p / b
}

/// `Beta.logGammaMinusLogGammaSum(a, b)` (`log Γ(b) - log Γ(a + b)`) for `0 <= a <= b` and
/// `b >= 10`.
fn log_gamma_minus_log_gamma_sum(a: f64, b: f64) -> f64 {
    let d = b + (a - 0.5);
    let w = delta_minus_delta_sum(a, b);
    let u = d * (a / b).ln_1p();
    let v = a * (b.ln() - 1.0);
    // Commons Math subtracts the smaller of `u` and `v` first: `u` it is for `a = 1/2` and
    // `b >= 10`, the only arguments the t distribution passes.
    (w - u) - v
}

/// `Beta.logBeta(1/2, b)` (the branch of a smallest argument below 1).
fn log_beta_half(b: f64) -> f64 {
    if b >= 10.0 {
        // `Gamma.logGamma(0.5)` is `logGamma1p((0.5 - 0.5) - 0.5)`.
        log_gamma1p(-0.5) + log_gamma_minus_log_gamma_sum(0.5, b)
    } else {
        (gamma(0.5) * gamma(b) / gamma(0.5 + b)).ln()
    }
}

/// `ContinuedFraction.evaluate` (modified Lentz) of the regularized beta function, whose
/// `a` coefficients are all 1.
fn beta_fraction(x: f64, a: f64, b: f64) -> f64 {
    const EPSILON: f64 = 1e-14;
    let coefficient = |n: i32| {
        if n % 2 == 0 {
            let m = f64::from(n) / 2.0;
            (m * (b - m) * x) / ((a + (2.0 * m) - 1.0) * (a + (2.0 * m)))
        } else {
            let m = (f64::from(n) - 1.0) / 2.0;
            -((a + m) * (a + b + m) * x) / ((a + (2.0 * m)) * (a + (2.0 * m) + 1.0))
        }
    };
    let mut h_prev = 1.0;
    let mut d_prev = 0.0;
    let mut c_prev = h_prev;
    let mut n = 1;
    loop {
        let bn = coefficient(n);
        // Commons Math replaces a `d_n` or `c_n` within 1e-50 of zero by 1e-50, which the
        // coefficients of the regularized beta function at `b = 1/2` never come close to.
        let d_n = 1.0 / (1.0 + bn * d_prev);
        let c_n = 1.0 + bn / c_prev;
        let delta = c_n * d_n;
        let h_n = h_prev * delta;
        if (delta - 1.0).abs() < EPSILON {
            return h_n;
        }
        d_prev = d_n;
        c_prev = c_n;
        h_prev = h_n;
        n += 1;
    }
}

/// `Beta.regularizedBeta(x, a, b)` where `a` or `b` is `1/2`, `NaN` for a `NaN` `x`.
fn regularized_beta(x: f64, a: f64, b: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x > (a + 1.0) / (2.0 + b + a) && 1.0 - x <= (b + 1.0) / (2.0 + b + a) {
        return 1.0 - regularized_beta(1.0 - x, b, a);
    }
    // `logBeta` is symmetric, one of `a` and `b` is 1/2.
    let log_beta = log_beta_half(a.max(b));
    ((a * x.ln()) + (b * (-x).ln_1p()) - a.ln() - log_beta).exp() / beta_fraction(x, a, b)
}

/// Student's t distribution of `degrees_of_freedom` (positive) degrees of freedom.
pub struct TDistribution {
    degrees_of_freedom: f64,
}

impl TDistribution {
    /// `new TDistribution(degreesOfFreedom)`.
    pub fn new(degrees_of_freedom: f64) -> TDistribution {
        TDistribution { degrees_of_freedom }
    }

    /// `cumulativeProbability`.
    pub fn cumulative_probability(&self, x: f64) -> f64 {
        if x == 0.0 {
            return 0.5;
        }
        let n = self.degrees_of_freedom;
        let t = regularized_beta(n / (n + x * x), 0.5 * n, 0.5);
        if x < 0.0 { 0.5 * t } else { 1.0 - 0.5 * t }
    }

    /// `AbstractRealDistribution.inverseCumulativeProbability`: the bounds from Chebyshev's
    /// inequality when the variance is finite (doubled until they bracket `p` otherwise),
    /// then a Brent search to `1e-9`.
    #[allow(clippy::float_cmp)]
    pub fn inverse_cumulative_probability(&self, p: f64) -> Result<f64> {
        if !(0.0..=1.0).contains(&p) && !p.is_nan() {
            return Err(Error::runtime(
                "org.apache.commons.math3.exception.OutOfRangeException",
                format!("{} out of [0, 1] range", message_decimal(p)),
            ));
        }
        if p == 0.0 {
            return Ok(f64::NEG_INFINITY);
        }
        if p == 1.0 {
            return Ok(f64::INFINITY);
        }
        let df = self.degrees_of_freedom;
        let (lower, upper) = if df > 2.0 {
            // Mean 0, variance `df / (df - 2)`.
            let sig = (df / (df - 2.0)).sqrt();
            (-sig * ((1.0 - p) / p).sqrt(), sig * (p / (1.0 - p)).sqrt())
        } else {
            let mut lower = -1.0;
            while self.cumulative_probability(lower) >= p {
                lower *= 2.0;
            }
            let mut upper = 1.0;
            while self.cumulative_probability(upper) < p {
                upper *= 2.0;
            }
            (lower, upper)
        };
        brent_solve(|x| self.cumulative_probability(x) - p, lower, upper, 1e-9)
    }
}

/// The `MessageFormat` (en_US) rendering of a double argument: at most three fraction digits
/// (half even), digits grouped by three.
fn message_decimal(d: f64) -> String {
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

/// `UnivariateSolverUtils.solve(f, min, max, absoluteAccuracy)`: `BrentSolver.doSolve` from
/// the middle of the interval (relative accuracy `1e-14`, function value accuracy `1e-15`).
fn brent_solve(f: impl Fn(f64) -> f64, min: f64, max: f64, accuracy: f64) -> Result<f64> {
    const FUNCTION_VALUE_ACCURACY: f64 = 1e-15;
    // The bounds of the t distribution always form an interval (or are `NaN`, which
    // `verifySequence` lets through), and the function is never within 1e-15 of 0 at the
    // lower bound, which is chosen with a probability below `p`.
    let initial = min + 0.5 * (max - min);
    let y_initial = f(initial);
    if y_initial.abs() <= FUNCTION_VALUE_ACCURACY {
        return Ok(initial);
    }
    let y_min = f(min);
    if y_initial * y_min < 0.0 {
        return Ok(brent(&f, min, initial, y_min, y_initial, accuracy));
    }
    let y_max = f(max);
    if y_max.abs() <= FUNCTION_VALUE_ACCURACY {
        return Ok(max);
    }
    if y_initial * y_max < 0.0 {
        return Ok(brent(&f, initial, max, y_initial, y_max, accuracy));
    }
    Err(Error::runtime(
        "org.apache.commons.math3.exception.NoBracketingException",
        format!(
            "function values at endpoints do not have different signs, endpoints: [{}, {}], values: [{}, {}]",
            message_decimal(min),
            message_decimal(max),
            message_decimal(y_min),
            message_decimal(y_max)
        ),
    ))
}

/// `BrentSolver.brent`.
#[allow(clippy::float_cmp)]
fn brent(f: &impl Fn(f64) -> f64, lo: f64, hi: f64, f_lo: f64, f_hi: f64, t: f64) -> f64 {
    const EPS: f64 = 1e-14;
    let (mut a, mut fa) = (lo, f_lo);
    let (mut b, mut fb) = (hi, f_hi);
    let (mut c, mut fc) = (a, fa);
    let mut d = b - a;
    let mut e = d;
    loop {
        if fc.abs() < fb.abs() {
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }
        let tol = 2.0 * EPS * b.abs() + t;
        let m = 0.5 * (c - b);
        // `Precision.equals(fb, 0)`: within one ulp of zero.
        if m.abs() <= tol || fb.abs() <= f64::from_bits(1) {
            return b;
        }
        if e.abs() < tol || fa.abs() <= fb.abs() {
            d = m;
            e = d;
        } else {
            let mut s = fb / fa;
            let (mut p, mut q);
            if a == c {
                p = 2.0 * m * s;
                q = 1.0 - s;
            } else {
                q = fa / fc;
                let r = fb / fc;
                p = s * (2.0 * m * q * (q - r) - (b - a) * (r - 1.0));
                q = (q - 1.0) * (r - 1.0) * (s - 1.0);
            }
            if p > 0.0 {
                q = -q;
            } else {
                p = -p;
            }
            s = e;
            e = d;
            if p >= 1.5 * m * q - (tol * q).abs() || p >= (0.5 * s * q).abs() {
                d = m;
                e = d;
            } else {
                d = p / q;
            }
        }
        a = b;
        fa = fb;
        if d.abs() > tol {
            b += d;
        } else if m > 0.0 {
            b += tol;
        } else {
            b -= tol;
        }
        fb = f(b);
        if (fb > 0.0 && fc > 0.0) || (fb <= 0.0 && fc <= 0.0) {
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }
    }
}
