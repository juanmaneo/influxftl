// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! WarpScript language runtime and WarpLib function library.
//!
//! This crate is a Rust port of the analytics half of the Warp 10 platform: the WarpScript
//! stack language, its interpreter and the WarpLib functions. It is storage agnostic — the
//! `influxftl` binary plugs InfluxDB 3 Core in through the datastore abstraction.
//!
//! ```
//! use warpscript::{Stack, Value};
//!
//! let mut stack = Stack::new();
//! stack.exec("42 'answer' STORE [ $answer $answer ] DROP $answer")?;
//! assert_eq!(stack.pop()?, Value::Long(42));
//! # Ok::<(), warpscript::Error>(())
//! ```

pub mod aggregators;
pub mod argon2;
pub mod bigdecimal;
#[cfg(test)]
mod bigdecimal_tests;
pub mod bitset;
pub mod cmath;
pub mod coverage;
#[cfg(test)]
mod coverage_tests;
pub mod curvefit;
#[cfg(test)]
mod curvefit_tests;
pub mod distribution;
#[cfg(test)]
mod distribution_tests;
pub mod encoder;
#[cfg(test)]
mod encoder_tests;
pub mod encoding;
pub mod error;
pub mod fft;
#[cfg(test)]
mod fft_tests;
pub mod fillers;
#[cfg(test)]
mod fillers_tests;
pub mod filters;
#[cfg(test)]
mod filters_tests;
pub mod finite;
pub mod format;
pub mod functions;
pub mod geohash;
pub mod geoshape;
pub mod geoxp;
pub mod gts;
pub mod gtsparse;
pub mod interpolation;
#[cfg(test)]
mod interpolation_tests;
pub mod javamap;
pub mod jmath;
pub mod jrandom;
pub mod macros;
pub mod pickle;
pub mod sax;
pub mod selector;
pub mod snapshot;
pub mod stack;
pub mod thrift;
pub mod unpickle;
pub mod value;
pub mod valueparse;
pub mod wavelet;
mod wavelet_data;
#[cfg(test)]
mod wavelet_tests;

pub use error::{Error, Result};
pub use macros::{Callable, Macro, Statement};
pub use stack::{Limits, Stack};
pub use value::{FunctionObject, Value};

/// Version of the runtime (the crate version), reported by `REV`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod argon2_tests;
#[cfg(test)]
mod bitset_tests;
#[cfg(test)]
mod cmath_tests;
#[cfg(test)]
mod encoding_tests;
#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod finite_tests;
#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod geoshape_tests;
#[cfg(test)]
mod geoxp_tests;
#[cfg(test)]
mod gts_tests;
#[cfg(test)]
mod jmath_tests;
#[cfg(test)]
mod jrandom_tests;
#[cfg(test)]
mod macros_tests;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod stack_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod thrift_tests;
#[cfg(test)]
mod value_tests;

#[cfg(kani)]
mod proofs;
