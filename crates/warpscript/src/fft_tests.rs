// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the Fourier transform against the definition.

use std::f64::consts::PI;

use crate::fft::{self, Complex};

/// A reproducible signal of `n` complex values (a linear congruential generator).
fn signal(n: usize) -> Vec<Complex> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    (0..n)
        .map(|_| Complex {
            re: next(),
            im: next(),
        })
        .collect()
}

/// The sum of the definition, `X[k] = sum of x[j] exp(-2 pi i j k / n)`.
fn naive(data: &[Complex]) -> Vec<Complex> {
    let n = data.len();
    (0..n)
        .map(|k| {
            data.iter()
                .enumerate()
                .fold(Complex::default(), |sum, (j, x)| {
                    let angle = -2.0 * PI * ((j * k) % n) as f64 / n as f64;
                    sum + *x
                        * Complex {
                            re: angle.cos(),
                            im: angle.sin(),
                        }
                })
        })
        .collect()
}

fn assert_close(actual: &[Complex], expected: &[Complex], tolerance: f64, what: &str) {
    assert_eq!(actual.len(), expected.len(), "{what}: length");
    for (k, (a, e)) in actual.iter().zip(expected).enumerate() {
        assert!(
            (a.re - e.re).abs() <= tolerance && (a.im - e.im).abs() <= tolerance,
            "{what}: coefficient {k}: {a:?} instead of {e:?}"
        );
    }
}

#[test]
fn forward_matches_the_definition() {
    for n in (1..=33).chain([64, 100, 127, 128, 243, 1000]) {
        let data = signal(n);
        let mut transformed = data.clone();
        fft::forward(&mut transformed);
        assert_close(
            &transformed,
            &naive(&data),
            1e-9 * n as f64,
            &format!("n = {n}"),
        );
    }
}

#[test]
fn inverse_undoes_forward() {
    for n in (1..=33).chain([64, 100, 127, 128, 243, 1000]) {
        let data = signal(n);
        let mut transformed = data.clone();
        fft::forward(&mut transformed);
        fft::inverse(&mut transformed);
        assert_close(&transformed, &data, 1e-12 * n as f64, &format!("n = {n}"));
    }
}

#[test]
fn inverse_matches_the_definition() {
    for n in [1, 2, 3, 8, 12, 17, 64] {
        let data = signal(n);
        let mut transformed = data.clone();
        fft::inverse(&mut transformed);
        // The inverse of the spectrum is the conjugate of the transform of its conjugate.
        let conjugated: Vec<Complex> = data
            .iter()
            .map(|c| Complex {
                re: c.re,
                im: -c.im,
            })
            .collect();
        let expected: Vec<Complex> = naive(&conjugated)
            .into_iter()
            .map(|c| Complex {
                re: c.re / n as f64,
                im: -c.im / n as f64,
            })
            .collect();
        assert_close(&transformed, &expected, 1e-12, &format!("n = {n}"));
    }
}

#[test]
fn four_values_are_exact() {
    let mut data: Vec<Complex> = [1.0, 2.0, 4.0, 3.0]
        .iter()
        .map(|&re| Complex { re, im: 0.0 })
        .collect();
    fft::forward(&mut data);
    let expected = [(10.0, 0.0), (-3.0, 1.0), (0.0, 0.0), (-3.0, -1.0)];
    for (actual, (re, im)) in data.iter().zip(expected) {
        assert_eq!((actual.re, actual.im), (re, im));
    }
}

#[test]
fn eight_values_have_exact_diagonal_roots() {
    // The eighth roots of unity are `(+-1 +-i) / sqrt 2` and the unit vectors.
    let mut data = vec![Complex::default(); 8];
    data[1] = Complex { re: 1.0, im: 0.0 };
    fft::forward(&mut data);
    let h = std::f64::consts::FRAC_1_SQRT_2;
    let expected = [
        (1.0, 0.0),
        (h, -h),
        (0.0, -1.0),
        (-h, -h),
        (-1.0, 0.0),
        (-h, h),
        (0.0, 1.0),
        (h, h),
    ];
    for (k, (actual, (re, im))) in data.iter().zip(expected).enumerate() {
        assert_eq!((actual.re, actual.im), (re, im), "coefficient {k}");
    }
}

#[test]
fn a_value_or_none_is_left_alone() {
    let mut empty: Vec<Complex> = Vec::new();
    fft::forward(&mut empty);
    assert_eq!(empty, Vec::<Complex>::new());
    let mut one = vec![Complex { re: 3.0, im: -2.0 }];
    fft::forward(&mut one);
    assert_eq!(one, [Complex { re: 3.0, im: -2.0 }]);
}

#[test]
fn complex_arithmetic() {
    let a = Complex { re: 1.0, im: 2.0 };
    let b = Complex { re: 3.0, im: -1.0 };
    assert_eq!(a + b, Complex { re: 4.0, im: 1.0 });
    assert_eq!(a - b, Complex { re: -2.0, im: 3.0 });
    assert_eq!(a * b, Complex { re: 5.0, im: 5.0 });
}

#[test]
fn real_values_have_a_conjugate_symmetric_spectrum() {
    for n in [1, 2, 3, 4, 5, 6, 7, 8, 12, 15, 17, 64, 100] {
        let values: Vec<f64> = signal(n).iter().map(|c| c.re).collect();
        let spectrum = fft::forward_real(&values);
        let complex: Vec<Complex> = values.iter().map(|&re| Complex { re, im: 0.0 }).collect();
        let mut expected = complex.clone();
        fft::forward(&mut expected);
        assert_close(&spectrum, &expected, 1e-12 * n as f64, &format!("n = {n}"));
        assert_eq!(spectrum[0].im, 0.0, "n = {n}: the mean is real");
        assert!(spectrum[0].im.is_sign_positive(), "n = {n}");
        if n % 2 == 0 {
            assert_eq!(spectrum[n / 2].im, 0.0, "n = {n}: Nyquist is real");
            assert!(spectrum[n / 2].im.is_sign_positive(), "n = {n}");
        }
        for k in 1..n {
            assert_eq!(spectrum[n - k].re, spectrum[k].re, "n = {n}, k = {k}");
            assert_eq!(spectrum[n - k].im, -spectrum[k].im, "n = {n}, k = {k}");
        }
    }
}

#[test]
fn no_real_value_has_no_spectrum() {
    assert_eq!(fft::forward_real(&[]), Vec::<Complex>::new());
}
