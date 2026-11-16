// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Symbolic Aggregate approXimation (`SAXUtils`): the breakpoints of the standard normal
//! distribution that split it into `2^levels` equiprobable intervals (Acklam's inverse CDF
//! refined with Cody's `erfc`), the SAX symbol of a value and the bit-interleaved binary
//! form of a SAX word (bSAX).

// The coefficients are written as in the reference implementations.
#![allow(clippy::unreadable_literal, clippy::excessive_precision)]

use std::sync::OnceLock;

use crate::jmath;

/// `SAXUtils.SAX_MAX_LEVELS`.
pub const SAX_MAX_LEVELS: u32 = 16;

/// Acklam's rational approximation of the inverse normal CDF.
const ICDF_A: [f64; 6] = [
    -3.969683028665376e+01,
    2.209460984245205e+02,
    -2.759285104469687e+02,
    1.383577518672690e+02,
    -3.066479806614716e+01,
    2.506628277459239e+00,
];
const ICDF_B: [f64; 5] = [
    -5.447609879822406e+01,
    1.615858368580409e+02,
    -1.556989798598866e+02,
    6.680131188771972e+01,
    -1.328068155288572e+01,
];
const ICDF_C: [f64; 6] = [
    -7.784894002430293e-03,
    -3.223964580411365e-01,
    -2.400758277161838e+00,
    -2.549732539343734e+00,
    4.374664141464968e+00,
    2.938163982698783e+00,
];
const ICDF_D: [f64; 4] = [
    7.784695709041462e-03,
    3.224671290700398e-01,
    2.445134137142996e+00,
    3.754408661907416e+00,
];
const P_LOW: f64 = 0.02425;
const P_HIGH: f64 = 1.0 - P_LOW;

/// Cody's rational Chebyshev approximations of `erf` / `erfc`.
const ERF_A: [f64; 5] = [
    3.16112374387056560E00,
    1.13864154151050156E02,
    3.77485237685302021E02,
    3.20937758913846947E03,
    1.85777706184603153E-1,
];
const ERF_B: [f64; 4] = [
    2.36012909523441209E01,
    2.44024637934444173E02,
    1.28261652607737228E03,
    2.84423683343917062E03,
];
const ERF_C: [f64; 9] = [
    5.64188496988670089E-1,
    8.88314979438837594E0,
    6.61191906371416295E01,
    2.98635138197400131E02,
    8.81952221241769090E02,
    1.71204761263407058E03,
    2.05107837782607147E03,
    1.23033935479799725E03,
    2.15311535474403846E-8,
];
const ERF_D: [f64; 8] = [
    1.57449261107098347E01,
    1.17693950891312499E02,
    5.37181101862009858E02,
    1.62138957456669019E03,
    3.29079923573345963E03,
    4.36261909014324716E03,
    3.43936767414372164E03,
    1.23033935480374942E03,
];
const THRESHOLD: f64 = 0.46875;
const X_SMALL: f64 = 1.110223024625156663E-16;

/// `exp(-Y²)` split as Cody does to limit the rounding error.
fn exp_minus_square(y: f64) -> f64 {
    let y_sq = jmath::round(y * 16.0) as f64 / 16.0;
    let del = (y - y_sq) * (y + y_sq);
    (-y_sq * y_sq).exp() * (-del).exp()
}

/// `SAXUtils.erfc` (`calerf(x, 1)`) for `|x| <= 4`, the only arguments the refinement of
/// the breakpoints passes (`|x| < 2.9` for 2^16 levels); Cody's third interval is not ported.
fn erfc(x: f64) -> f64 {
    let y = x.abs();
    if y <= THRESHOLD {
        let y_sq = if y > X_SMALL { y * y } else { 0.0 };
        let mut num = ERF_A[4] * y_sq;
        let mut den = y_sq;
        for i in 0..3 {
            num = (num + ERF_A[i]) * y_sq;
            den = (den + ERF_B[i]) * y_sq;
        }
        return 1.0 - x * (num + ERF_A[3]) / (den + ERF_B[3]);
    }
    let mut num = ERF_C[8] * y;
    let mut den = y;
    for i in 0..7 {
        num = (num + ERF_C[i]) * y;
        den = (den + ERF_D[i]) * y;
    }
    let result = (num + ERF_C[7]) / (den + ERF_D[7]) * exp_minus_square(y);
    if x < 0.0 { 2.0 - result } else { result }
}

/// `SAXUtils.getInvCDF(d, true)` for `0 < d < 1`: Acklam's approximation, refined by one
/// step of Halley's method.
fn inverse_cdf(d: f64) -> f64 {
    let z = if d < P_LOW {
        let q = (-2.0 * d.ln()).sqrt();
        (((((ICDF_C[0] * q + ICDF_C[1]) * q + ICDF_C[2]) * q + ICDF_C[3]) * q + ICDF_C[4]) * q
            + ICDF_C[5])
            / ((((ICDF_D[0] * q + ICDF_D[1]) * q + ICDF_D[2]) * q + ICDF_D[3]) * q + 1.0)
    } else if P_HIGH < d {
        let q = (-2.0 * (1.0 - d).ln()).sqrt();
        -(((((ICDF_C[0] * q + ICDF_C[1]) * q + ICDF_C[2]) * q + ICDF_C[3]) * q + ICDF_C[4]) * q
            + ICDF_C[5])
            / ((((ICDF_D[0] * q + ICDF_D[1]) * q + ICDF_D[2]) * q + ICDF_D[3]) * q + 1.0)
    } else {
        let q = d - 0.5;
        let r = q * q;
        (((((ICDF_A[0] * r + ICDF_A[1]) * r + ICDF_A[2]) * r + ICDF_A[3]) * r + ICDF_A[4]) * r
            + ICDF_A[5])
            * q
            / (((((ICDF_B[0] * r + ICDF_B[1]) * r + ICDF_B[2]) * r + ICDF_B[3]) * r + ICDF_B[4])
                * r
                + 1.0)
    };
    // `refine`
    let e = 0.5 * erfc(-z / 2.0f64.sqrt()) - d;
    let u = e * (2.0 * std::f64::consts::PI).sqrt() * ((z * z) / 2.0).exp();
    z - u / (1.0 + z * u / 2.0)
}

/// `SAXUtils.getBounds(2^levels)`, computed once per number of levels.
fn bounds(levels: u32) -> &'static [f64] {
    static BOUNDS: [OnceLock<Vec<f64>>; SAX_MAX_LEVELS as usize] =
        [const { OnceLock::new() }; SAX_MAX_LEVELS as usize];
    BOUNDS[levels as usize - 1].get_or_init(|| {
        let n = 1usize << levels;
        let area = 1.0 / n as f64;
        (0..n - 1)
            .map(|i| inverse_cdf((i + 1) as f64 * area))
            .collect()
    })
}

/// `Arrays.binarySearch(double[], key)` on the breakpoints. Java tells equal values apart
/// by their bits: `NaN` sorts last, and `-0.0` before `0.0`, which cannot occur here (the
/// breakpoints are never `-0.0`, nor are the aggregates looked up).
fn binary_search_f64(a: &[f64], key: f64) -> Result<usize, usize> {
    let mut low = 0i64;
    let mut high = a.len() as i64 - 1;
    while low <= high {
        let mid = (low + high) >> 1;
        let mid_val = a[mid as usize];
        if mid_val < key || key.is_nan() {
            low = mid + 1;
        } else if mid_val > key {
            high = mid - 1;
        } else {
            return Ok(mid as usize);
        }
    }
    Err(low as usize)
}

/// `SAXUtils.SAX`: the index of the interval of the standard normal distribution split in
/// `2^levels` (`1 <= levels <= 16`) equiprobable intervals that `value` falls in.
pub fn sax(levels: u32, value: f64) -> usize {
    match binary_search_f64(bounds(levels), value) {
        Ok(idx) | Err(idx) => idx,
    }
}

/// `SAXUtils.bSAX`: the bits of the symbols interleaved, most significant level first, padded
/// with zeros to a whole number of bytes.
pub fn bsax(levels: u32, symbols: &[usize]) -> Vec<u8> {
    let bitlen = levels as usize * symbols.len();
    let mut binary = vec![0u8; bitlen.div_ceil(8)];
    let mut idx = 0;
    let mut bits = 0u32;
    let mut bitcount = 0;
    for bit in 0..levels {
        for &symbol in symbols {
            bits = (bits << 1) | ((symbol >> (levels - bit - 1)) & 1) as u32;
            bitcount += 1;
            if bitcount == 8 {
                binary[idx] = bits as u8;
                idx += 1;
                bitcount = 0;
            }
        }
    }
    if idx < binary.len() {
        bits <<= 8 - bitcount;
        binary[idx] = bits as u8;
    }
    binary
}
