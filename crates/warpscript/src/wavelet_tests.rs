// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the wavelet registry and of the transform.

use crate::wavelet::Wavelet;

/// The names of the registry, in the order of Warp 10.
fn names() -> Vec<String> {
    let mut names = vec!["haar".to_owned(), "dmey".to_owned()];
    names.extend((1..=20).map(|n| format!("db{n}")));
    names.extend((2..=20).map(|n| format!("sym{n}")));
    names.extend((1..=5).map(|n| format!("coif{n}")));
    for family in ["bior", "rbio"] {
        for (a, b) in [
            (1, 1),
            (1, 3),
            (1, 5),
            (2, 2),
            (2, 4),
            (2, 6),
            (2, 8),
            (3, 1),
            (3, 3),
            (3, 5),
            (3, 7),
            (3, 9),
            (4, 4),
            (5, 5),
            (6, 8),
        ] {
            names.push(format!("{family}{a}.{b}"));
        }
    }
    names
}

#[test]
fn every_wavelet_is_found() {
    let names = names();
    assert_eq!(names.len(), 76);
    for name in &names {
        let wavelet = Wavelet::find(name).unwrap_or_else(|| panic!("{name} should be known"));
        assert_eq!(&wavelet.name, name);
        let length = wavelet.scaling_decomposition.len();
        assert!(length >= 2 && length.is_multiple_of(2), "{name}");
        assert_eq!(wavelet.detail_decomposition.len(), length, "{name}");
        assert_eq!(wavelet.scaling_reconstruction.len(), length, "{name}");
        assert_eq!(wavelet.detail_reconstruction.len(), length, "{name}");
    }
}

#[test]
fn an_unknown_wavelet_is_not_found() {
    assert!(Wavelet::find("foo").is_none());
    assert!(Wavelet::find("HAAR").is_none());
    assert!(Wavelet::find("").is_none());
}

#[test]
fn haar_transforms_and_reverses() {
    let haar = Wavelet::find("haar").expect("haar should be known");
    let signal = [1.0, 2.0, 4.0, 3.0];
    let transformed = haar.forward(&signal);
    let h = std::f64::consts::FRAC_1_SQRT_2;
    // The coarsest approximation, then the details of the coarsest and of the finest level.
    let expected = [5.0, -2.0, -h, h];
    for (actual, expected) in transformed.iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-12,
            "{actual} instead of {expected}"
        );
    }
    let back = haar.reverse(&transformed);
    for (actual, expected) in back.iter().zip(signal) {
        assert!(
            (actual - expected).abs() < 1e-12,
            "{actual} instead of {expected}"
        );
    }
}

#[test]
fn every_wavelet_reverses_its_transform() {
    let signal: Vec<f64> = (0..32).map(|i| f64::from(i * i % 7) - 2.5).collect();
    for name in names() {
        let wavelet = Wavelet::find(&name).expect("the wavelet should be known");
        let back = wavelet.reverse(&wavelet.forward(&signal));
        for (actual, expected) in back.iter().zip(&signal) {
            // The discrete Meyer wavelet only approximates a perfect reconstruction.
            let tolerance = if name == "dmey" { 0.05 } else { 1e-6 };
            assert!(
                (actual - expected).abs() < tolerance,
                "{name}: {actual} instead of {expected}"
            );
        }
    }
}

#[test]
fn a_signal_shorter_than_the_wavelength_is_left_alone() {
    let db2 = Wavelet::find("db2").expect("db2 should be known");
    assert_eq!(db2.forward(&[3.0]), [3.0]);
    assert_eq!(db2.reverse(&[3.0]), [3.0]);
    assert_eq!(db2.forward(&[]), Vec::<f64>::new());
    assert_eq!(db2.reverse(&[]), Vec::<f64>::new());
}
