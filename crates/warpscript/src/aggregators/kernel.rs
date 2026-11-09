// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The kernel smoothers (`step width mapper.kernel.gaussian`…): the weighted mean of the
//! values, locations and elevations of the window, the weight of a datapoint depending on its
//! distance in steps to the current tick (`MapperKernel`, `MapperKernelSmoother`).

use std::f64::consts::PI;

use super::{Aggregated, Aggregator, AggregatorArgs, Named, Num, Roles};
use crate::error::fault::{BoundsError, ResourceError};
use crate::error::{Error, Result};
use crate::functions::Entry;
use crate::geoxp;
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::jmath;
use crate::stack::Stack;
use crate::value::{Value, double_to_long};

/// The registered names of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "mapper.kernel.cosine",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Cosine)),
    ),
    (
        "mapper.kernel.epanechnikov",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Epanechnikov)),
    ),
    (
        "mapper.kernel.gaussian",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Gaussian)),
    ),
    (
        "mapper.kernel.logistic",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Logistic)),
    ),
    (
        "mapper.kernel.quartic",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Quartic)),
    ),
    (
        "mapper.kernel.silverman",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Silverman)),
    ),
    (
        "mapper.kernel.triangular",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Triangular)),
    ),
    (
        "mapper.kernel.tricube",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Tricube)),
    ),
    (
        "mapper.kernel.triweight",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Triweight)),
    ),
    (
        "mapper.kernel.uniform",
        Entry::Builder(|s, n| kernel_builder(s, n, Kernel::Uniform)),
    ),
];

/// The most weights a kernel may have here (a width up to `2^25`); Java allocates up to the
/// heap size.
const MAX_WEIGHTS: usize = 1 << 24;

/// A smoothing kernel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kernel {
    /// `MapperKernelCosine`.
    Cosine,
    /// `MapperKernelEpanechnikov`.
    Epanechnikov,
    /// `MapperKernelGaussian`.
    Gaussian,
    /// `MapperKernelLogistic`.
    Logistic,
    /// `MapperKernelQuartic`.
    Quartic,
    /// `MapperKernelSilverman`.
    Silverman,
    /// `MapperKernelTriangular`.
    Triangular,
    /// `MapperKernelTricube`.
    Tricube,
    /// `MapperKernelTriweight`.
    Triweight,
    /// `MapperKernelUniform`.
    Uniform,
}

impl Kernel {
    /// `getWeights`: the weight of each distance in steps, `0..count`, `u` going from 0 to 1
    /// (a single weight divides 0 by 0, as in Java).
    pub fn weights(self, count: usize) -> Vec<f64> {
        let last = count as f64 - 1.0;
        (0..count)
            .map(|i| {
                let u = i as f64 / last;
                match self {
                    Kernel::Cosine => (PI / 4.0) * ((PI / 2.0) * u).cos(),
                    Kernel::Epanechnikov => (3.0 / 4.0) * (1.0 - (u * u)),
                    Kernel::Gaussian => (1.0 / (2.0 * PI).sqrt()) * (-0.5 * u * u).exp(),
                    Kernel::Logistic => 1.0 / (u.exp() + 2.0 + (-u).exp()),
                    Kernel::Quartic => (15.0 / 16.0) * jmath::pow(1.0 - (u * u), 2.0),
                    Kernel::Silverman => {
                        0.5 * (-u.abs() / 2f64.sqrt()).exp()
                            * ((PI / 4.0) * u.abs() / 2f64.sqrt()).sin()
                    }
                    Kernel::Triangular => 1.0 - u,
                    Kernel::Tricube => {
                        (70.0 / 81.0) * jmath::pow(1.0 - jmath::pow(u.abs(), 3.0), 3.0)
                    }
                    Kernel::Triweight => (35.0 / 32.0) * jmath::pow(1.0 - (u * u), 3.0),
                    Kernel::Uniform => 0.5,
                }
            })
            .collect()
    }
}

/// `MapperKernelSmoother`: the kernel weighted mean of the numeric values of the window, at
/// the current tick; the location and elevation are the weighted means of the datapoints
/// that have one.
pub struct Smoother {
    /// The step: the tick distance of consecutive weights.
    pub step: i64,
    /// The window width given to the builder (for the snapshot).
    pub width: i64,
    /// The weights, by distance in steps.
    pub weights: Vec<f64>,
}

impl Aggregator for Smoother {
    fn roles(&self) -> Roles {
        Roles::MAPPER
    }

    #[allow(clippy::float_cmp)]
    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        let (mut value, mut lat, mut lon, mut elevation) = (0.0, 0.0, 0.0, 0.0);
        let (mut value_weight, mut location_weight, mut elevation_weight) = (0.0, 0.0, 0.0);
        for (i, &tick) in args.ticks.iter().enumerate() {
            if self.step == 0 {
                return Err(Error::division_by_zero());
            }
            // `(int) (Math.abs(ticks[i] - tick) / step)`
            let distance = tick
                .wrapping_sub(args.tick)
                .wrapping_abs()
                .wrapping_div(self.step);
            let index = i64::from(distance as i32);
            let Some(number) = Num::of(&args.values[i]).map(Num::as_f64) else {
                continue;
            };
            if index >= self.weights.len() as i64 {
                continue;
            }
            let weight = usize::try_from(index)
                .ok()
                .map(|index| self.weights[index])
                .ok_or_else(|| Error::index_out_of_bounds(index, self.weights.len()))?;
            value += weight * number;
            value_weight += weight;
            if args.locations[i] != NO_LOCATION {
                let (la, lo) = geoxp::from_geoxp_point(args.locations[i]);
                lat += weight * la;
                lon += weight * lo;
                location_weight += weight;
            }
            if args.elevations[i] != NO_ELEVATION {
                elevation += weight * args.elevations[i] as f64;
                elevation_weight += weight;
            }
        }
        let mut result = Aggregated {
            tick: args.tick,
            location: NO_LOCATION,
            elevation: NO_ELEVATION,
            value: Value::Null,
        };
        // As Java computes it: a `NaN` total weight is not zero.
        if value_weight != 0.0 {
            result.value = Value::Double(value / value_weight);
            if elevation_weight != 0.0 {
                result.elevation = double_to_long(elevation / elevation_weight);
            }
            if location_weight != 0.0 {
                result.location =
                    geoxp::to_geoxp_point(lat / location_weight, lon / location_weight);
            }
        }
        Ok(result)
    }

    fn parameters(&self) -> Option<String> {
        Some(format!("{} {}", self.step, self.width))
    }
}

/// `MapperKernel`: `step width name` pushes the smoother; the width (an `int`) must be odd.
fn kernel_builder(stack: &mut Stack, name: &'static str, kernel: Kernel) -> Result<()> {
    let Value::Long(width) = stack.pop()? else {
        return Err(Error::new(format!(
            "{name} expects a window width in ticks on top of the stack."
        )));
    };
    let width = width as i32;
    if width % 2 == 0 {
        return Err(Error::new(format!(
            "{name} window width MUST be odd (we count ticks, not intervals)."
        )));
    }
    let Value::Long(step) = stack.pop()? else {
        return Err(Error::new(format!(
            "{name} expects a step length as a LONG."
        )));
    };
    // `new double[1 + (width >>> 1)]`
    let count = 1i32.wrapping_add((width as u32 >> 1) as i32);
    let Ok(count) = usize::try_from(count) else {
        return Err(BoundsError::NegativeSize(i64::from(count)).into());
    };
    if count > MAX_WEIGHTS {
        // HotSpot arrays hold at most `Integer.MAX_VALUE - 2` elements.
        return Err(if count > (i32::MAX - 2) as usize {
            ResourceError::ArrayTooLong
        } else {
            ResourceError::OutOfMemory
        }
        .into());
    }
    stack.push(Value::Function(Named::object(
        name,
        Smoother {
            step,
            width: i64::from(width),
            weights: kernel.weights(count),
        },
    )));
    Ok(())
}
