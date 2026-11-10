// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The discrete Fourier transform of complex data, which `FFT`, `FFTAP` and `IFFT` apply to
//! the values of a series.
//!
//! Warp 10 delegates to JTransforms (`DoubleFFT_1D`), which is licensed under the MPL 1.1,
//! the GPL 2 or the LGPL 2.1. This module is an independent implementation of the same
//! mathematical transform, written from the algorithms alone (an iterative radix-2
//! Cooley-Tukey transform for the lengths that are powers of two, Bluestein's chirp-z
//! algorithm for the others), so that the crate stays under the Apache License 2.0. The
//! coefficients are the same up to rounding errors (`docs/DIVERGENCES.md`).

use std::cmp::Ordering;
use std::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_2};
use std::ops::{Add, Mul, Sub};

/// A complex number.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    /// The real part.
    pub re: f64,
    /// The imaginary part.
    pub im: f64,
}

impl Complex {
    /// `exp(-2 pi i k / n)`, the `k`-th of the `n` roots of unity of the forward transform
    /// (`k < n`). The turn is reduced to an angle of at most an eighth of a turn, so that the
    /// roots on an axis or a diagonal are exact (`cos(pi / 2)` is not `0` in floating point).
    fn root(k: usize, n: usize) -> Complex {
        let quadrant = 4 * k / n;
        // The remainder of the quarter turn, in units of `1 / (4n)` of a turn.
        let rest = 4 * k - quadrant * n;
        let (cos, sin) = match (2 * rest).cmp(&n) {
            Ordering::Equal => (FRAC_1_SQRT_2, FRAC_1_SQRT_2),
            Ordering::Less => {
                let (sin, cos) = (FRAC_PI_2 * rest as f64 / n as f64).sin_cos();
                (cos, sin)
            }
            Ordering::Greater => {
                let (sin, cos) = (FRAC_PI_2 * (n - rest) as f64 / n as f64).sin_cos();
                (sin, cos)
            }
        };
        // Rotate by the whole quarter turns, then conjugate: the transform turns clockwise.
        let (re, im) = match quadrant {
            0 => (cos, sin),
            1 => (-sin, cos),
            2 => (-cos, -sin),
            _ => (sin, -cos),
        };
        Complex { re, im: -im }
    }

    /// The complex conjugate.
    fn conj(self) -> Complex {
        Complex {
            re: self.re,
            im: -self.im,
        }
    }
}

impl Add for Complex {
    type Output = Complex;

    fn add(self, other: Complex) -> Complex {
        Complex {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }
}

impl Sub for Complex {
    type Output = Complex;

    fn sub(self, other: Complex) -> Complex {
        Complex {
            re: self.re - other.re,
            im: self.im - other.im,
        }
    }
}

impl Mul for Complex {
    type Output = Complex;

    fn mul(self, other: Complex) -> Complex {
        Complex {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }
}

/// The forward transform, in place: `X[k] = sum over j of x[j] * exp(-2 pi i j k / n)`.
pub fn forward(data: &mut [Complex]) {
    let n = data.len();
    if n < 2 {
        return;
    }
    if n.is_power_of_two() {
        radix2(data);
    } else {
        bluestein(data);
    }
}

/// The forward transform of real values. The spectrum of real values is conjugate symmetric
/// (`X[n - k]` is the conjugate of `X[k]`, the mean and the Nyquist coefficient are real):
/// it is enforced exactly, so that rounding errors do not give the coefficients that are real
/// by nature a tiny imaginary part, which would turn their phase from `0` or `pi` to noise.
pub fn forward_real(values: &[f64]) -> Vec<Complex> {
    let mut data: Vec<Complex> = values.iter().map(|&re| Complex { re, im: 0.0 }).collect();
    forward(&mut data);
    let n = data.len();
    if let Some(mean) = data.first_mut() {
        mean.im = 0.0;
    }
    if n > 0 && n.is_multiple_of(2) {
        data[n / 2].im = 0.0;
    }
    for k in 1..n.div_ceil(2) {
        data[n - k] = data[k].conj();
    }
    data
}

/// The inverse transform, in place and scaled by `1 / n`, so that it undoes [`forward`].
pub fn inverse(data: &mut [Complex]) {
    // The inverse is the conjugate of the forward transform of the conjugate.
    conjugate(data);
    forward(data);
    conjugate(data);
    let scale = 1.0 / data.len() as f64;
    for value in data.iter_mut() {
        value.re *= scale;
        value.im *= scale;
    }
}

fn conjugate(data: &mut [Complex]) {
    for value in data.iter_mut() {
        *value = value.conj();
    }
}

/// The transform of a length that is a power of two (at least 2): bit reversal, then
/// butterflies of growing span.
fn radix2(data: &mut [Complex]) {
    let n = data.len();
    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = i.reverse_bits() >> (usize::BITS - bits);
        if i < j {
            data.swap(i, j);
        }
    }
    // The roots of unity of the first half of the circle.
    let twiddles: Vec<Complex> = (0..n / 2).map(|k| Complex::root(k, n)).collect();
    let mut half = 1;
    while half < n {
        let stride = n / (2 * half);
        for block in data.chunks_exact_mut(2 * half) {
            let (low, high) = block.split_at_mut(half);
            for ((a, b), twiddle) in low
                .iter_mut()
                .zip(high.iter_mut())
                .zip(twiddles.iter().step_by(stride))
            {
                let product = *twiddle * *b;
                *b = *a - product;
                *a = *a + product;
            }
        }
        half *= 2;
    }
}

/// The transform of any length (at least 2) as a convolution with a chirp, computed by
/// transforms of a power of two length: `x[j] * w[j]` convolved with `conj(w[j])`, where
/// `w[j] = exp(-i pi j^2 / n)`, then multiplied by `w[k]`.
fn bluestein(data: &mut [Complex]) {
    let n = data.len();
    let size = (2 * n - 1).next_power_of_two();
    // `j^2 mod 2n` keeps the angle small, hence accurate, for long series.
    let mut square = 0;
    let chirp: Vec<Complex> = (0..n)
        .map(|j| {
            let w = Complex::root(square, 2 * n);
            square = (square + 2 * j + 1) % (2 * n);
            w
        })
        .collect();
    let mut a = vec![Complex::default(); size];
    for ((slot, value), w) in a.iter_mut().zip(data.iter()).zip(&chirp) {
        *slot = *value * *w;
    }
    let mut b = vec![Complex::default(); size];
    for (j, w) in chirp.iter().enumerate() {
        b[j] = w.conj();
        if j > 0 {
            b[size - j] = w.conj();
        }
    }
    radix2(&mut a);
    radix2(&mut b);
    for (x, y) in a.iter_mut().zip(&b) {
        *x = *x * *y;
    }
    inverse(&mut a);
    for ((value, convolved), w) in data.iter_mut().zip(&a).zip(&chirp) {
        *value = *convolved * *w;
    }
}
