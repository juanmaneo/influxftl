//! The `java.lang.Math` based functions of WarpLib: unary and binary numerical functions,
//! exact integer arithmetic, random numbers and interval bounds (the constants `PI`, `E`,
//! `MINLONG`, `MAXLONG` and `NaN` live in [`super::misc`], `RANGE` in [`super::control`]).

use super::Entry;
use super::args::fail;
use super::numerical::{self, Binary, Unary};
use crate::cmath;
use crate::error::{Error, Result};
use crate::jmath;
use crate::jrandom::JavaRandom;
use crate::stack::Stack;
use crate::value::Value;

/// Stack attribute holding the state of the generator seeded by `PRNG`.
pub const ATTRIBUTE_SEEDED_PRNG: &str = "seeded.prng";
/// Configuration property pushed by `IDENT`.
pub const WARP_IDENT: &str = "warp.ident";

/// Define `fn $rust(stack)` as the unary numerical function `$name` with operator `$op`.
macro_rules! unary_functions {
    ($($rust:ident => $name:literal, $op:expr;)*) => {
        $(
            fn $rust(stack: &mut Stack) -> Result<()> {
                numerical::unary(stack, $name, $op)
            }
        )*
    };
}

/// Define `fn $rust(stack)` as the binary numerical function `$name` with operator `$op`;
/// `$in_list` is the `applyOnSingleList` flag.
macro_rules! binary_functions {
    ($($rust:ident => $name:literal, $op:expr, $in_list:literal;)*) => {
        $(
            fn $rust(stack: &mut Stack) -> Result<()> {
                numerical::binary(stack, $name, $op, $in_list)
            }
        )*
    };
}

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("ISNaN", Entry::Stack(isnan)),
    ("ISNULL", Entry::Stack(isnull)),
    ("RAND", Entry::Stack(rand)),
    ("RANDOM", Entry::Stack(random)),
    ("PRNG", Entry::Stack(prng)),
    ("SRAND", Entry::Stack(srand)),
    ("ABS", Entry::Stack(abs)),
    ("COS", Entry::Stack(cos)),
    ("COSH", Entry::Stack(cosh)),
    ("ACOS", Entry::Stack(acos)),
    ("SIN", Entry::Stack(sin)),
    ("SINH", Entry::Stack(sinh)),
    ("ASIN", Entry::Stack(asin)),
    ("TAN", Entry::Stack(tan)),
    ("TANH", Entry::Stack(tanh)),
    ("ATAN", Entry::Stack(atan)),
    ("SIGNUM", Entry::Stack(signum)),
    ("FLOOR", Entry::Stack(floor)),
    ("CEIL", Entry::Stack(ceil)),
    ("ROUND", Entry::Stack(round)),
    ("RINT", Entry::Stack(rint)),
    ("ULP", Entry::Stack(ulp)),
    ("NEXTUP", Entry::Stack(nextup)),
    ("NEXTDOWN", Entry::Stack(nextdown)),
    ("SQRT", Entry::Stack(sqrt)),
    ("CBRT", Entry::Stack(cbrt)),
    ("EXP", Entry::Stack(exp)),
    ("EXPM1", Entry::Stack(expm1)),
    ("LOG", Entry::Stack(log)),
    ("LOG10", Entry::Stack(log10)),
    ("LOG1P", Entry::Stack(log1p)),
    ("TORADIANS", Entry::Stack(toradians)),
    ("TODEGREES", Entry::Stack(todegrees)),
    ("INCREMENTEXACT", Entry::Stack(incrementexact)),
    ("DECREMENTEXACT", Entry::Stack(decrementexact)),
    ("NEGATEEXACT", Entry::Stack(negateexact)),
    ("TOINTEXACT", Entry::Stack(tointexact)),
    ("MAX", Entry::Stack(max)),
    ("MIN", Entry::Stack(min)),
    ("COPYSIGN", Entry::Stack(copysign)),
    ("HYPOT", Entry::Stack(hypot)),
    ("IEEEREMAINDER", Entry::Stack(ieeeremainder)),
    ("NEXTAFTER", Entry::Stack(nextafter)),
    ("ATAN2", Entry::Stack(atan2)),
    ("FLOORDIV", Entry::Stack(floordiv)),
    ("FLOORMOD", Entry::Stack(floormod)),
    ("ADDEXACT", Entry::Stack(addexact)),
    ("SUBTRACTEXACT", Entry::Stack(subtractexact)),
    ("MULTIPLYEXACT", Entry::Stack(multiplyexact)),
    ("SCALB", Entry::Stack(scalb)),
    ("GETEXPONENT", Entry::Stack(getexponent)),
    ("IDENT", Entry::Stack(ident)),
    ("LBOUNDS", Entry::Stack(lbounds)),
    ("NBOUNDS", Entry::Stack(nbounds)),
];

// -----------------------------------------------------------------------------------------
// Predicates
// -----------------------------------------------------------------------------------------

fn isnan(stack: &mut Stack) -> Result<()> {
    let top = stack.pop()?;
    stack.push(matches!(top, Value::Double(d) if d.is_nan()));
    Ok(())
}

fn isnull(stack: &mut Stack) -> Result<()> {
    let top = stack.pop()?;
    stack.push(top.is_null());
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Random numbers
// -----------------------------------------------------------------------------------------

/// The error raised when the operating system cannot provide entropy.
pub(crate) fn entropy_error(err: getrandom::Error) -> Error {
    Error::new(format!(
        "Unable to obtain random data from the operating system ({err})."
    ))
}

/// Push a uniformly distributed double in `[0, 1)` built from the top 53 bits of `entropy`,
/// which is what `SecureRandom.nextDouble()` and `Math.random()` deliver.
pub(crate) fn push_secure_double(
    stack: &mut Stack,
    entropy: core::result::Result<u64, getrandom::Error>,
) -> Result<()> {
    let bits = entropy.map_err(entropy_error)?;
    stack.push((bits >> 11) as f64 / (1u64 << 53) as f64);
    Ok(())
}

fn rand(stack: &mut Stack) -> Result<()> {
    push_secure_double(stack, getrandom::u64())
}

fn random(stack: &mut Stack) -> Result<()> {
    push_secure_double(stack, getrandom::u64())
}

fn prng(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        Value::Long(seed) => {
            let generator = JavaRandom::new(seed);
            stack.set_attribute(ATTRIBUTE_SEEDED_PRNG, Some(Value::Long(generator.state())));
            Ok(())
        }
        _ => Err(fail("PRNG", "expects a LONG seed on top of the stack.")),
    }
}

fn srand(stack: &mut Stack) -> Result<()> {
    let Some(Value::Long(state)) = stack.attribute(ATTRIBUTE_SEEDED_PRNG) else {
        return Err(fail("SRAND", "seeded PRNG was not initialized."));
    };
    let mut generator = JavaRandom::from_state(*state);
    let value = generator.next_double();
    stack.set_attribute(ATTRIBUTE_SEEDED_PRNG, Some(Value::Long(generator.state())));
    stack.push(value);
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Unary and binary numerical functions
// -----------------------------------------------------------------------------------------

unary_functions! {
    abs => "ABS", Unary::Both(jmath::abs_long, f64::abs);
    cos => "COS", Unary::Double(f64::cos);
    cosh => "COSH", Unary::Double(f64::cosh);
    acos => "ACOS", Unary::Double(f64::acos);
    sin => "SIN", Unary::Double(f64::sin);
    sinh => "SINH", Unary::Double(f64::sinh);
    asin => "ASIN", Unary::Double(f64::asin);
    tan => "TAN", Unary::Double(f64::tan);
    tanh => "TANH", Unary::Double(f64::tanh);
    atan => "ATAN", Unary::Double(f64::atan);
    signum => "SIGNUM", Unary::Double(jmath::signum);
    floor => "FLOOR", Unary::Double(f64::floor);
    ceil => "CEIL", Unary::Double(f64::ceil);
    round => "ROUND", Unary::DoubleToLong(jmath::round);
    rint => "RINT", Unary::Double(jmath::rint);
    ulp => "ULP", Unary::Double(jmath::ulp);
    nextup => "NEXTUP", Unary::Double(f64::next_up);
    nextdown => "NEXTDOWN", Unary::Double(f64::next_down);
    sqrt => "SQRT", Unary::Double(f64::sqrt);
    cbrt => "CBRT", Unary::Double(f64::cbrt);
    exp => "EXP", Unary::Double(f64::exp);
    expm1 => "EXPM1", Unary::Double(f64::exp_m1);
    log => "LOG", Unary::Double(f64::ln);
    log10 => "LOG10", Unary::Double(f64::log10);
    log1p => "LOG1P", Unary::Double(f64::ln_1p);
    toradians => "TORADIANS", Unary::Double(f64::to_radians);
    todegrees => "TODEGREES", Unary::Double(f64::to_degrees);
    incrementexact => "INCREMENTEXACT", Unary::Long(jmath::increment_exact);
    decrementexact => "DECREMENTEXACT", Unary::Long(jmath::decrement_exact);
    negateexact => "NEGATEEXACT", Unary::Long(jmath::negate_exact);
    tointexact => "TOINTEXACT", Unary::Long(jmath::to_int_exact);
}

binary_functions! {
    max => "MAX", Binary::Both(i64::max, jmath::max), true;
    min => "MIN", Binary::Both(i64::min, jmath::min), true;
    copysign => "COPYSIGN", Binary::Double(f64::copysign), false;
    hypot => "HYPOT", Binary::Double(f64::hypot), false;
    ieeeremainder => "IEEEREMAINDER", Binary::Double(jmath::ieee_remainder), false;
    nextafter => "NEXTAFTER", Binary::Double(jmath::next_after), false;
    atan2 => "ATAN2", Binary::Double(f64::atan2), false;
    floordiv => "FLOORDIV", Binary::Long(jmath::floor_div), false;
    floormod => "FLOORMOD", Binary::Long(jmath::floor_mod), false;
    addexact => "ADDEXACT", Binary::Long(jmath::add_exact), true;
    subtractexact => "SUBTRACTEXACT", Binary::Long(jmath::subtract_exact), true;
    multiplyexact => "MULTIPLYEXACT", Binary::Long(jmath::multiply_exact), true;
}

// -----------------------------------------------------------------------------------------
// Floating point structure
// -----------------------------------------------------------------------------------------

fn scalb(stack: &mut Stack) -> Result<()> {
    const MSG: &str = "can only operate on numerical values.";
    let op0 = stack.pop()?;
    if !op0.is_number() {
        return Err(fail("SCALB", MSG));
    }
    let op1 = stack.pop()?;
    if !op1.is_number() {
        return Err(fail("SCALB", MSG));
    }
    match (op0, op1) {
        (Value::Long(scale), d) => {
            // `Number.intValue()` keeps the low 32 bits of the long.
            stack.push(jmath::scalb(d.as_f64().unwrap_or(f64::NAN), scale as i32));
            Ok(())
        }
        _ => Err(fail(
            "SCALB",
            "is given values of incorrect type. Expecting [ TOP:int, 2:double ].",
        )),
    }
}

fn getexponent(stack: &mut Stack) -> Result<()> {
    match stack.pop()? {
        ref n if n.is_number() => {
            let exponent = jmath::get_exponent(n.as_f64().unwrap_or(f64::NAN));
            stack.push(i64::from(exponent));
            Ok(())
        }
        _ => Err(fail("GETEXPONENT", "can only operate on numerical values.")),
    }
}

// -----------------------------------------------------------------------------------------
// Miscellaneous
// -----------------------------------------------------------------------------------------

fn ident(stack: &mut Stack) -> Result<()> {
    let value = stack
        .config_property(WARP_IDENT)
        .map_or(Value::Null, Value::string);
    stack.push(value);
    Ok(())
}

/// Pop the number of intervals of `LBOUNDS` / `NBOUNDS` (a `LONG`, 1..=65536).
fn pop_intervals(stack: &mut Stack, name: &str, too_few: &str) -> Result<i32> {
    let n = match stack.pop()? {
        Value::Long(l) => l as i32,
        _ => {
            return Err(fail(
                name,
                "expects a number of intervals on top of the stack.",
            ));
        }
    };
    if n < 1 {
        return Err(fail(name, too_few));
    }
    if n > 65536 {
        return Err(fail(
            name,
            "cannot generate bounds for more than 65536 intervals.",
        ));
    }
    Ok(n)
}

/// Pop a number as a double, with the given error message otherwise.
fn pop_double(stack: &mut Stack, name: &str, message: &str) -> Result<f64> {
    match stack.pop()? {
        ref n if n.is_number() => Ok(n.as_f64().unwrap_or(f64::NAN)),
        _ => Err(fail(name, message)),
    }
}

fn lbounds(stack: &mut Stack) -> Result<()> {
    let n = pop_intervals(
        stack,
        "LBOUNDS",
        "cannot generate bounds for less than 1 interval.",
    )?;
    let upper = pop_double(
        stack,
        "LBOUNDS",
        "expects an upper bound below the number of intervals.",
    )?;
    let lower = pop_double(
        stack,
        "LBOUNDS",
        "expects a lower bounds below the upper bound.",
    )?;
    if lower >= upper {
        return Err(fail(
            "LBOUNDS",
            "expects the lower bound to be strictly less than the upper bound.",
        ));
    }
    let step = (upper - lower) / f64::from(n);
    if step == 0.0 {
        return Err(fail("LBOUNDS", "cannot operate if bounds are too close."));
    }
    let mut bounds = Vec::with_capacity(n as usize + 1);
    bounds.push(Value::Double(lower));
    for i in 1..n {
        bounds.push(Value::Double(lower + f64::from(i) * step));
    }
    bounds.push(Value::Double(upper));
    stack.push(Value::list(bounds));
    Ok(())
}

fn nbounds(stack: &mut Stack) -> Result<()> {
    let n = pop_intervals(
        stack,
        "NBOUNDS",
        "cannot generate bounds for less than 2 intervals.",
    )?;
    let sigma = pop_double(
        stack,
        "NBOUNDS",
        "expects a standard deviation below the number of intervals.",
    )?;
    if sigma <= 0.0 {
        return Err(fail(
            "NBOUNDS",
            "expects a standard deviation strictly positive.",
        ));
    }
    let mu = pop_double(
        stack,
        "NBOUNDS",
        "expects a mean below the standard deviation.",
    )?;
    let area = 1.0 / f64::from(n);
    let bounds = (1..n)
        .map(|i| Value::Double(cmath::normal_inverse_cdf(mu, sigma, f64::from(i) * area)))
        .collect();
    stack.push(Value::list(bounds));
    Ok(())
}
