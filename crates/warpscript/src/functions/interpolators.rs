// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Interpolating functions: `INTERPOLATOR.1D.LINEAR`, `INTERPOLATOR.1D.SPLINE` and
//! `INTERPOLATOR.1D.AKIMA` build a function of one variable usable on the stack and as a
//! mapper.

use std::rc::Rc;

use super::Entry;
use super::args::fail;
use crate::aggregators::{Aggregated, Aggregator, AggregatorArgs, Roles};
use crate::error::{Error, Result};
use crate::interpolation::{PolynomialSplineFunction, akima, linear, spline};
use crate::macros::{Callable, StackFunction, StackObject};
use crate::stack::Stack;
use crate::value::Value;

/// The kind of a one dimensional interpolator.
#[derive(Clone, Copy)]
enum Kind {
    Linear,
    Spline,
    Akima,
}

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    (
        "INTERPOLATOR.1D.LINEAR",
        Entry::Stack(|s| interpolator_1d(s, "INTERPOLATOR.1D.LINEAR", Kind::Linear)),
    ),
    (
        "INTERPOLATOR.1D.SPLINE",
        Entry::Stack(|s| interpolator_1d(s, "INTERPOLATOR.1D.SPLINE", Kind::Spline)),
    ),
    (
        "INTERPOLATOR.1D.AKIMA",
        Entry::Stack(|s| interpolator_1d(s, "INTERPOLATOR.1D.AKIMA", Kind::Akima)),
    ),
];

/// The function `INTERPOLATOR.1D.*` builds: the interpolation of the points, `NaN` outside
/// their range (or everywhere for fewer than two points).
#[derive(Debug)]
struct Interpolator1D {
    name: &'static str,
    function: Option<PolynomialSplineFunction>,
    /// The abscissas and values as given, for the snapshot.
    xval: Vec<Value>,
    fval: Vec<Value>,
}

impl Interpolator1D {
    fn value(&self, x: f64) -> f64 {
        match &self.function {
            Some(function) if function.is_valid_point(x) => function.value(x),
            _ => f64::NAN,
        }
    }
}

impl StackObject for Interpolator1D {
    fn name(&self) -> &str {
        self.name
    }

    fn to_snapshot(&self) -> String {
        let list = |values: &[Value]| {
            let mut out = String::from("[ ");
            for value in values {
                out.push_str(&value.to_string());
                out.push(' ');
            }
            out.push(']');
            out
        };
        format!("{} {} {}", list(&self.xval), list(&self.fval), self.name)
    }

    fn type_name(&self) -> &'static str {
        "AGGREGATOR"
    }

    fn as_aggregator(&self) -> Option<&dyn Aggregator> {
        Some(self)
    }

    fn apply(&self, stack: &mut Stack) -> Result<()> {
        let x = stack.pop()?;
        let Some(x) = x.as_f64() else {
            return Err(fail(self.name, "expects a DOUBLE or a LONG"));
        };
        stack.push(Value::Double(self.value(x)));
        Ok(())
    }
}

impl Aggregator for Interpolator1D {
    fn roles(&self) -> Roles {
        Roles {
            bucketizer: false,
            mapper: true,
            reducer: false,
        }
    }

    /// The interpolation of the single value of the window.
    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.values.len() != 1 {
            return Err(fail(
                self.name,
                &format!("expects 1 element but got {}", args.values.len()),
            ));
        }
        let Some(x) = args.values[0].as_f64() else {
            return Err(fail(
                self.name,
                "expects a numeric GTS as argument when used as a mapper, but instead got a non numeric element",
            ));
        };
        Ok(Aggregated {
            tick: args.tick,
            location: args.locations[0],
            elevation: args.elevations[0],
            value: Value::Double(self.value(x)),
        })
    }
}

/// `INTERPOLATOR.1D.*`: the interpolating function of `[ x… ] [ f(x)… ]`.
fn interpolator_1d(stack: &mut Stack, name: &'static str, kind: Kind) -> Result<()> {
    let Value::List(fval) = stack.pop()? else {
        return Err(fail(name, "expects a LIST as 2nd argument"));
    };
    let Value::List(xval) = stack.pop()? else {
        return Err(fail(name, "expects a sorted LIST as 1st argument"));
    };
    let (xval, fval) = (xval.borrow().clone(), fval.borrow().clone());
    if xval.len() != fval.len() {
        return Err(Error::new(format!(
            "{name}: incoherent argument sizes, both input LIST should have the same size."
        )));
    }
    let mut xs = Vec::with_capacity(xval.len());
    let mut fs = Vec::with_capacity(fval.len());
    for (x, f) in xval.iter().zip(&fval) {
        let Some(x) = x.as_f64() else {
            return Err(fail(
                name,
                "expects the first argument to be a numeric LIST",
            ));
        };
        let Some(f) = f.as_f64() else {
            return Err(fail(name, "expects the last argument to be a numeric LIST"));
        };
        xs.push(x);
        fs.push(f);
    }
    let n = xs.len();
    let function = match kind {
        Kind::Spline if n > 2 => Some(spline(&xs, &fs)?),
        Kind::Akima if n > 4 => Some(akima(&xs, &fs)?),
        _ if n > 1 => Some(linear(&xs, &fs)?),
        _ => None,
    };
    stack.push(StackFunction::value(Callable::Object(Rc::new(
        Interpolator1D {
            name,
            function,
            xval,
            fval,
        },
    ))));
    Ok(())
}
