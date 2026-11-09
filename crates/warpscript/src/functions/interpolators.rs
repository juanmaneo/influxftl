// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Interpolating functions: `INTERPOLATOR.1D.LINEAR`, `INTERPOLATOR.1D.SPLINE` and
//! `INTERPOLATOR.1D.AKIMA` build a function of one variable usable on the stack and as a
//! mapper, `INTERPOLATOR.2D.BICUBIC` and `INTERPOLATOR.3D.TRICUBIC` functions of two and
//! three variables usable on the stack and as reducers.

use std::rc::Rc;

use super::Entry;
use super::args::fail;
use crate::aggregators::{Aggregated, Aggregator, AggregatorArgs, Roles};
use crate::error::series::SeriesError;
use crate::error::{Error, Result};
use crate::gts::{NO_ELEVATION, NO_LOCATION};
use crate::interpolation::{
    BicubicInterpolatingFunction, PolynomialSplineFunction, TricubicInterpolatingFunction, akima,
    bicubic, linear, spline, tricubic,
};
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
    ("INTERPOLATOR.2D.BICUBIC", Entry::Stack(interpolator_2d)),
    ("INTERPOLATOR.3D.TRICUBIC", Entry::Stack(interpolator_3d)),
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
        return Err(SeriesError::InterpolatorSizes(name.to_string()).into());
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

/// The Java `toString()` of the doubles of a grid axis: `[ 1.0 2.0 ]`.
fn doubles_snapshot(values: &[f64]) -> String {
    let mut out = String::from("[ ");
    for value in values {
        out.push_str(&Value::Double(*value).to_string());
        out.push(' ');
    }
    out.push(']');
    out
}

/// The numbers of a list argument, `message` for a non numeric element.
fn numbers(list: &[Value], name: &str, message: &str) -> Result<Vec<f64>> {
    list.iter()
        .map(|v| v.as_f64().ok_or_else(|| fail(name, message)))
        .collect()
}

/// The numeric components of a point of `count` dimensions given to an interpolating
/// function on the stack.
fn point(stack: &mut Stack, name: &str, count: usize, numeric: &str) -> Result<Vec<f64>> {
    let Value::List(list) = stack.pop()? else {
        return Err(fail(name, "expects a LIST"));
    };
    let list = list.borrow();
    if list.len() != count {
        return Err(fail(
            name,
            &format!("expects a LIST with {count} components {}", list.len()),
        ));
    }
    numbers(&list, name, numeric)
}

/// The reducer role of the interpolating functions of several variables.
const REDUCER: Roles = Roles {
    bucketizer: false,
    mapper: false,
    reducer: true,
};

/// The function `INTERPOLATOR.2D.BICUBIC` builds (`BICUBE`).
#[derive(Debug)]
struct Bicube {
    function: BicubicInterpolatingFunction,
}

impl Bicube {
    fn value(&self, x: f64, y: f64) -> Result<f64> {
        if self.function.is_valid_point(x, y) {
            self.function.value(x, y)
        } else {
            Ok(f64::NAN)
        }
    }
}

impl StackObject for Bicube {
    fn name(&self) -> &'static str {
        "BICUBE"
    }

    /// The axes, then the values of the spline on the grid (`NaN` where Java's evaluation
    /// would fail and abort the snapshot).
    fn to_snapshot(&self) -> String {
        let (xval, yval) = (&self.function.xval, &self.function.yval);
        let mut out = format!("{} {} [ ", doubles_snapshot(xval), doubles_snapshot(yval));
        for &x in xval {
            let row: Vec<f64> = yval
                .iter()
                .map(|&y| self.function.value(x, y).unwrap_or(f64::NAN))
                .collect();
            out.push_str(&doubles_snapshot(&row));
            out.push(' ');
        }
        out.push_str("] INTERPOLATOR.2D.BICUBIC");
        out
    }

    fn type_name(&self) -> &'static str {
        "AGGREGATOR"
    }

    fn as_aggregator(&self) -> Option<&dyn Aggregator> {
        Some(self)
    }

    fn apply(&self, stack: &mut Stack) -> Result<()> {
        let p = point(
            stack,
            "BICUBE",
            2,
            "expects a numeric LIST as argument, but it contains at least one non-numeric element.",
        )?;
        stack.push(Value::Double(self.value(p[0], p[1])?));
        Ok(())
    }
}

impl Aggregator for Bicube {
    fn roles(&self) -> Roles {
        REDUCER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.values.len() != 2 {
            return Err(fail(
                "BICUBE",
                &format!("expects 2 components but got {}", args.values.len()),
            ));
        }
        let (Some(x), Some(y)) = (args.values[0].as_f64(), args.values[1].as_f64()) else {
            return Err(fail(
                "BICUBE",
                "expects two numeric GTS when used as a reducer, but at least one value was not numeric.",
            ));
        };
        Ok(Aggregated {
            tick: args.tick,
            location: args.locations[0],
            elevation: args.elevations[0],
            value: Value::Double(self.value(x, y)?),
        })
    }
}

/// The incoherent sizes message of `INTERPOLATOR.2D.BICUBIC`.
fn sizes_2d(d1: usize, d2: usize) -> Error {
    SeriesError::BicubicSizes {
        rows: d1,
        columns: d2,
    }
    .into()
}

/// `INTERPOLATOR.2D.BICUBIC`: the bicubic interpolation of `[ x… ] [ y… ] [ [ f(x, y)… ]… ]`.
fn interpolator_2d(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "INTERPOLATOR.2D.BICUBIC";
    const VALUES: &str = "expects the last argument to be a LIST of LIST of numbers";
    let pop_list = |stack: &mut Stack, message: &str| match stack.pop()? {
        Value::List(list) => Ok(list.borrow().clone()),
        _ => Err(fail(NAME, message)),
    };
    let o3 = pop_list(stack, "expects a LIST as 3rd argument")?;
    let o2 = pop_list(stack, "expects a sorted LIST as 2nd argument")?;
    let o1 = pop_list(stack, "expects a sorted LIST as 1st argument")?;
    let xval = numbers(
        &o1,
        NAME,
        "expects a numeric LIST as 1st argument, but it contains at least one non-numeric element.",
    )?;
    let yval = numbers(
        &o2,
        NAME,
        "expects a numeric LIST as 2nd argument, but it contains at least one non-numeric element.",
    )?;
    let (d1, d2) = (xval.len(), yval.len());
    if o3.len() != d1 {
        return Err(sizes_2d(d1, d2));
    }
    let mut fval = Vec::with_capacity(d1);
    for row in &o3 {
        let Value::List(row) = row else {
            return Err(fail(NAME, VALUES));
        };
        let row = row.borrow();
        if row.len() != d2 {
            return Err(sizes_2d(d1, d2));
        }
        fval.push(numbers(&row, NAME, VALUES)?);
    }
    let function = bicubic(&xval, &yval, &fval)?;
    stack.push(StackFunction::value(Callable::Object(Rc::new(Bicube {
        function,
    }))));
    Ok(())
}

/// The function `INTERPOLATOR.3D.TRICUBIC` builds (`TRICUBE`).
#[derive(Debug)]
struct Tricube {
    function: TricubicInterpolatingFunction,
}

impl Tricube {
    /// The value, `NaN` outside the grid (including where `OutOfRangeException` is caught).
    fn value(&self, x: f64, y: f64, z: f64) -> f64 {
        if self.function.is_valid_point(x, y, z) {
            self.function.value(x, y, z).unwrap_or(f64::NAN)
        } else {
            f64::NAN
        }
    }
}

impl StackObject for Tricube {
    fn name(&self) -> &'static str {
        "TRICUBE"
    }

    fn to_snapshot(&self) -> String {
        let f = &self.function;
        let mut out = format!(
            "{} {} {} [ ",
            doubles_snapshot(&f.xval),
            doubles_snapshot(&f.yval),
            doubles_snapshot(&f.zval)
        );
        for &x in &f.xval {
            out.push_str("[ ");
            for &y in &f.yval {
                let line: Vec<f64> = f
                    .zval
                    .iter()
                    .map(|&z| f.value(x, y, z).unwrap_or(f64::NAN))
                    .collect();
                out.push_str(&doubles_snapshot(&line));
                out.push(' ');
            }
            out.push_str("] ");
        }
        out.push_str("] INTERPOLATOR.3D.TRICUBIC");
        out
    }

    fn type_name(&self) -> &'static str {
        "AGGREGATOR"
    }

    fn as_aggregator(&self) -> Option<&dyn Aggregator> {
        Some(self)
    }

    fn apply(&self, stack: &mut Stack) -> Result<()> {
        let p = point(
            stack,
            "TRICUBE",
            3,
            "expects a numeric LIST as argument, instead got at least one non numeric element",
        )?;
        stack.push(Value::Double(self.value(p[0], p[1], p[2])));
        Ok(())
    }
}

impl Aggregator for Tricube {
    fn roles(&self) -> Roles {
        REDUCER
    }

    fn apply(&self, args: &AggregatorArgs) -> Result<Aggregated> {
        if args.values.len() != 3 {
            return Err(fail(
                "TRICUBE",
                &format!("expects 3 components but got {}", args.values.len()),
            ));
        }
        if args.values.iter().any(|v| matches!(v, Value::Null)) {
            return Ok(Aggregated {
                tick: args.tick,
                location: NO_LOCATION,
                elevation: NO_ELEVATION,
                value: Value::Null,
            });
        }
        let p: Option<Vec<f64>> = args.values.iter().map(Value::as_f64).collect();
        let Some(p) = p else {
            return Err(fail(
                "TRICUBE",
                "expects a numeric GTS as argument when used as a reducer, instead got at least one non numeric element",
            ));
        };
        Ok(Aggregated {
            tick: args.tick,
            location: args.locations[0],
            elevation: args.elevations[0],
            value: Value::Double(self.value(p[0], p[1], p[2])),
        })
    }
}

/// The incoherent sizes message of `INTERPOLATOR.3D.TRICUBIC`.
fn sizes_3d(d1: usize, d2: usize, d3: usize) -> Error {
    SeriesError::TricubicSizes {
        x: d1,
        y: d2,
        z: d3,
    }
    .into()
}

/// `INTERPOLATOR.3D.TRICUBIC`: the tricubic interpolation of
/// `[ x… ] [ y… ] [ z… ] [ [ [ f(x, y, z)… ]… ]… ]`.
fn interpolator_3d(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "INTERPOLATOR.3D.TRICUBIC";
    const VALUES: &str = "expects the last argument to be a LIST of LIST of LIST of numbers";
    let pop_list = |stack: &mut Stack, message: &str| match stack.pop()? {
        Value::List(list) => Ok(list.borrow().clone()),
        _ => Err(fail(NAME, message)),
    };
    let o4 = pop_list(stack, "expects a LIST as 4th argument")?;
    let o3 = pop_list(stack, "expects a sorted LIST as 3rd argument")?;
    let o2 = pop_list(stack, "expects a sorted LIST as 2nd argument")?;
    let o1 = pop_list(stack, "expects a sorted LIST as 1st argument")?;
    let axis = |list: &[Value], rank: &str| {
        numbers(
            list,
            NAME,
            &format!(
                "expects a numeric LIST as {rank} argument, instead got at least one non numeric element"
            ),
        )
    };
    let xval = axis(&o1, "1st")?;
    let yval = axis(&o2, "2nd")?;
    let zval = axis(&o3, "3rd")?;
    let (d1, d2, d3) = (xval.len(), yval.len(), zval.len());
    if o4.len() != d1 {
        return Err(sizes_3d(d1, d2, d3));
    }
    let mut fval = Vec::with_capacity(d1);
    for plane in &o4 {
        let Value::List(plane) = plane else {
            return Err(fail(
                NAME,
                "expects the last argument to be a LIST of LIST of LIST of numbers.",
            ));
        };
        let plane = plane.borrow();
        if plane.len() != d2 {
            return Err(sizes_3d(d1, d2, d3));
        }
        let mut rows = Vec::with_capacity(d2);
        for line in plane.iter() {
            let Value::List(line) = line else {
                return Err(fail(NAME, VALUES));
            };
            let line = line.borrow();
            if line.len() != d3 {
                return Err(sizes_3d(d1, d2, d3));
            }
            rows.push(numbers(&line, NAME, VALUES)?);
        }
        fval.push(rows);
    }
    let function = tricubic(&xval, &yval, &zval, &fval)?;
    stack.push(StackFunction::value(Callable::Object(Rc::new(Tricube {
        function,
    }))));
    Ok(())
}
