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
//! stack.exec("42 'answer' STORE [ $answer $answer ] DROP $answer").unwrap();
//! assert_eq!(stack.pop().unwrap(), Value::Long(42));
//! ```

pub mod aggregators;
pub mod argon2;
pub mod bitset;
pub mod cmath;
pub mod encoding;
pub mod error;
pub mod fillers;
#[cfg(test)]
mod fillers_tests;
pub mod filters;
#[cfg(test)]
mod filters_tests;
pub mod format;
pub mod functions;
pub mod geoxp;
pub mod gts;
pub mod interpolation;
#[cfg(test)]
mod interpolation_tests;
pub mod jmath;
pub mod jrandom;
pub mod macros;
pub mod snapshot;
pub mod stack;
pub mod value;

pub use error::{Error, Result, ScriptError};
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
mod format_tests;
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
mod value_tests;

#[cfg(kani)]
mod proofs;
