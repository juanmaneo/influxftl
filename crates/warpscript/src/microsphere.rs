// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The interpolating microsphere of Commons Math (`InterpolatingMicrosphere`,
//! `MicrosphereProjectionInterpolator`, <http://www.dudziak.com/microsphere.pdf>): a
//! multivariate interpolation in which the facets of a sphere around the interpolated point
//! are lit by the samples, nearer ones more than the others.

use crate::error::math::MathError;
use crate::error::{Error, Result};
use crate::interpolation::linear_combination;
use crate::jmath;
use crate::jrandom::JavaRandom;

/// The parameters of a microsphere.
#[derive(Clone, Copy, Debug)]
pub struct MicrosphereParams {
    /// The number of dimensions.
    pub dimension: usize,
    /// The number of facets.
    pub size: i32,
    /// The largest fraction of the facets that may stay dark.
    pub max_dark_fraction: f64,
    /// The illumination under which a facet is dark.
    pub dark_threshold: f64,
    /// The value where too many facets are dark.
    pub background: f64,
}

/// An interpolating microsphere: the normals of its facets.
#[derive(Clone, Debug)]
pub struct Microsphere {
    params: MicrosphereParams,
    normals: Vec<Vec<f64>>,
}

/// `MathArrays.safeNorm`: the Euclidean norm of a vector, without overflow or underflow.
pub(crate) fn safe_norm(v: &[f64]) -> f64 {
    let rdwarf = 3.834e-20;
    let rgiant = 1.304e19;
    let (mut s1, mut s2, mut s3) = (0.0, 0.0, 0.0);
    let (mut x1max, mut x3max) = (0.0, 0.0);
    let agiant = rgiant / v.len() as f64;
    for &component in v {
        let xabs = component.abs();
        if xabs < rdwarf || xabs > agiant {
            if xabs > rdwarf {
                if xabs > x1max {
                    let r = x1max / xabs;
                    s1 = 1.0 + s1 * r * r;
                    x1max = xabs;
                } else {
                    let r = xabs / x1max;
                    s1 += r * r;
                }
            } else if xabs > x3max {
                let r = x3max / xabs;
                s3 = 1.0 + s3 * r * r;
                x3max = xabs;
            } else if xabs != 0.0 {
                let r = xabs / x3max;
                s3 += r * r;
            }
        } else {
            s2 += xabs * xabs;
        }
    }
    if s1 != 0.0 {
        x1max * (s1 + (s2 / x1max) / x1max).sqrt()
    } else if s2 == 0.0 {
        x3max * s3.sqrt()
    } else if s2 >= x3max {
        (s2 * (1.0 + (x3max / s2) * (x3max * s3))).sqrt()
    } else {
        (x3max * ((s2 / x3max) + (x3max * s3))).sqrt()
    }
}

/// `MathArrays.cosAngle`.
pub(crate) fn cos_angle(a: &[f64], b: &[f64]) -> f64 {
    // A single term is a plain product in Commons Math.
    let dot = match (a, b) {
        ([x], [y]) => x * y,
        _ => linear_combination(a, b),
    };
    dot / (safe_norm(a) * safe_norm(b))
}

impl Microsphere {
    /// `new InterpolatingMicrosphere(dimension, size, maxDarkFraction, darkThreshold,
    /// background, new UnitSphereRandomVectorGenerator(dimension, random))`: a microsphere of
    /// `size` facets whose normals are random points of the unit sphere.
    ///
    /// # Errors
    ///
    /// A dimension or a size that is not positive, a dark fraction outside of `[0, 1]`, or a
    /// negative dark threshold.
    pub fn new(params: MicrosphereParams, random: &mut JavaRandom) -> Result<Microsphere> {
        let MicrosphereParams {
            dimension,
            size,
            max_dark_fraction,
            dark_threshold,
            ..
        } = params;
        if dimension == 0 {
            return Err(MathError::NotStrictlyPositive {
                value: 0,
                minimum: 0,
            }
            .into());
        }
        if size <= 0 {
            return Err(MathError::NotStrictlyPositive {
                value: i64::from(size),
                minimum: 0,
            }
            .into());
        }
        let facets = size as usize;
        if !(0.0..=1.0).contains(&max_dark_fraction) {
            return Err(MathError::OutOfRange {
                value: max_dark_fraction,
                low: 0.0,
                high: 1.0,
            }
            .into());
        }
        if dark_threshold < 0.0 {
            return Err(MathError::NotPositive {
                value: dark_threshold,
            }
            .into());
        }
        let normals = (0..facets)
            .map(|_| {
                // A standard Gaussian for each component, then normalized to unit length.
                let mut v: Vec<f64> = (0..dimension).map(|_| random.next_gaussian()).collect();
                let norm_squared = v
                    .iter()
                    .fold(0.0, |sum, component| sum + component * component);
                let factor = 1.0 / norm_squared.sqrt();
                for component in &mut v {
                    *component *= factor;
                }
                v
            })
            .collect();
        Ok(Microsphere { params, normals })
    }

    /// The number of dimensions.
    pub fn dimension(&self) -> usize {
        self.params.dimension
    }

    /// `InterpolatingMicrosphere.value`: the interpolation at `point` of the samples (and their
    /// values) with the weights `distance ^ -exponent`, the value of a sample that is (within
    /// `tolerance` of) the point.
    ///
    /// # Errors
    ///
    /// A sample whose dimension is not that of the point.
    pub fn value(
        &self,
        point: &[f64],
        samples: &[Vec<f64>],
        values: &[f64],
        exponent: f64,
        tolerance: f64,
    ) -> Result<f64> {
        // The illumination of each facet, and the value of the sample that gives it.
        let mut facets = vec![(0.0, 0.0); self.normals.len()];
        for (sample, &sample_value) in samples.iter().zip(values) {
            if sample.len() != point.len() {
                return Err(MathError::DimensionMismatch {
                    wrong: sample.len(),
                    expected: point.len(),
                }
                .into());
            }
            // The vector between the interpolated point and the sample.
            let diff: Vec<f64> = sample.iter().zip(point).map(|(s, p)| s - p).collect();
            let distance = safe_norm(&diff);
            if distance.abs() < tolerance {
                // The point is (very close to) a sample: nothing to interpolate.
                return Ok(sample_value);
            }
            let weight = jmath::pow(distance, -exponent);
            for (normal, facet) in self.normals.iter().zip(&mut facets) {
                let cos = cos_angle(normal, &diff);
                if cos > 0.0 {
                    let illumination = cos * weight;
                    if illumination > self.params.dark_threshold && illumination > facet.0 {
                        *facet = (illumination, sample_value);
                    }
                }
            }
        }
        let (mut value, mut total_weight, mut dark) = (0.0, 0.0, 0_u32);
        for &(illumination, sample_value) in &facets {
            if illumination == 0.0 {
                dark += 1;
            } else {
                value += illumination * sample_value;
                total_weight += illumination;
            }
        }
        let dark_fraction = f64::from(dark) / f64::from(self.params.size);
        Ok(if dark_fraction <= self.params.max_dark_fraction {
            value / total_weight
        } else {
            self.params.background
        })
    }
}

/// `MicrosphereProjectionInterpolator`'s check of the exponent of the weights.
///
/// # Errors
///
/// A negative exponent.
pub fn check_exponent(exponent: f64) -> Result<()> {
    if exponent < 0.0 {
        return Err(Error::from(MathError::NotPositive { value: exponent }));
    }
    Ok(())
}
