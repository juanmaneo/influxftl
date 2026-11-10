// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The discrete wavelet transform `FDWT`, `IDWT` and `DWTSPLIT` apply (`io.warp10.script.fwt`):
//! a fast transform of a signal whose length is a power of two, by convolution with the
//! decomposition filters of a wavelet, level after level.

use crate::wavelet_data::WAVELETS;

/// A wavelet: the filters of its decomposition and of its reconstruction, all of the same
/// length (the length of the mother wavelet).
#[derive(Debug)]
pub struct Wavelet {
    /// The name `FDWT` knows it by.
    pub(crate) name: &'static str,
    /// The smallest length of signal a level is computed for (`2`, `4`, `8`…).
    pub(crate) transform_wavelength: usize,
    /// The low pass filter of the decomposition, reversed to be applied as a convolution.
    pub(crate) scaling_decomposition: &'static [f64],
    /// The high pass filter of the decomposition, reversed to be applied as a convolution.
    pub(crate) detail_decomposition: &'static [f64],
    /// The low pass filter of the reconstruction.
    pub(crate) scaling_reconstruction: &'static [f64],
    /// The high pass filter of the reconstruction.
    pub(crate) detail_reconstruction: &'static [f64],
}

impl Wavelet {
    /// `WaveletRegistry.find`: the wavelet of a name (`haar`, `db1` to `db20`, `sym2` to
    /// `sym20`, `coif1` to `coif5`, `bior1.1` to `bior6.8`, `rbio1.1` to `rbio6.8`, `dmey`).
    pub fn find(name: &str) -> Option<&'static Wavelet> {
        WAVELETS.iter().find(|wavelet| wavelet.name == name)
    }

    /// `FWT.forward`: the transform of `signal`, whose length is a power of two (the callers
    /// check it): the details of each level follow the approximation of the next one,
    /// `[ approximation | coarsest details | … | finest details ]`.
    pub fn forward(&self, signal: &[f64]) -> Vec<f64> {
        let mut transformed = signal.to_vec();
        let mut length = transformed.len();
        while length >= self.transform_wavelength {
            let level = self.decompose(&transformed, length);
            transformed[..length].copy_from_slice(&level);
            length >>= 1;
        }
        transformed
    }

    /// `FWT.reverse`: the signal of a transform, whose length is a power of two (the callers
    /// check it).
    pub fn reverse(&self, transformed: &[f64]) -> Vec<f64> {
        let mut signal = transformed.to_vec();
        let mut length = self.transform_wavelength;
        while length <= signal.len() {
            let level = self.reconstruct(&signal, length);
            signal[..length].copy_from_slice(&level);
            length <<= 1;
        }
        signal
    }

    /// `Wavelet.forward`: one level over the first `length` values, the filtered and
    /// subsampled signal (approximation) then the details. The signal wraps around when a
    /// filter is longer than what remains of it.
    fn decompose(&self, input: &[f64], length: usize) -> Vec<f64> {
        let mut output = vec![0.0; length];
        let (approximation, details) = output.split_at_mut(length >> 1);
        for (i, (approximation, detail)) in approximation.iter_mut().zip(details).enumerate() {
            let filters = self
                .scaling_decomposition
                .iter()
                .zip(self.detail_decomposition);
            for (j, (scaling, wavelet)) in filters.enumerate() {
                let sample = input[(2 * i + j) % length];
                *approximation += sample * scaling;
                *detail += sample * wavelet;
            }
        }
        output
    }

    /// `Wavelet.reverse`: one level over the first `length` values, adding up the
    /// approximation and the details filtered by the reconstruction filters.
    fn reconstruct(&self, input: &[f64], length: usize) -> Vec<f64> {
        let mut output = vec![0.0; length];
        let (approximation, details) = input[..length].split_at(length >> 1);
        for (i, (approximation, detail)) in approximation.iter().zip(details).enumerate() {
            let filters = self
                .scaling_reconstruction
                .iter()
                .zip(self.detail_reconstruction);
            for (j, (scaling, wavelet)) in filters.enumerate() {
                output[(2 * i + j) % length] += approximation * scaling + detail * wavelet;
            }
        }
        output
    }
}
