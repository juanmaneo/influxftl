#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreadable_literal,
    clippy::approx_constant,
    clippy::excessive_precision
)]

//! Reference values below were produced with OpenJDK 27 (`java.lang.Math`, `java.util.Random`)
//! and Apache Commons Math 3.6.1 and are kept verbatim, hence the lint exemptions.

use indexmap::IndexMap;

use crate::functions::call;
use crate::functions::math::{ATTRIBUTE_SEEDED_PRNG, WARP_IDENT, push_secure_double};
use crate::stack::Stack;
use crate::tests::{assert_fails, assert_top, exec, top};
use crate::value::Value;

fn long(l: i64) -> Value {
    Value::Long(l)
}

fn doubles(values: &[f64]) -> Value {
    Value::list(values.iter().copied().map(Value::Double).collect())
}

#[test]
fn predicates() {
    assert_top("NaN ISNaN", true);
    assert_top("1.0 ISNaN", false);
    assert_top("1 ISNaN", false);
    assert_top("'NaN' ISNaN", false);
    assert_fails("ISNaN", "Empty stack.");
    assert_top("NULL ISNULL", true);
    assert_top("0 ISNULL", false);
    assert_top("'' ISNULL", false);
    assert_fails("ISNULL", "Empty stack.");
}

#[test]
fn random_numbers_are_in_the_unit_interval() {
    for script in ["RAND", "RANDOM"] {
        for _ in 0..16 {
            let Value::Double(d) = top(script) else {
                panic!("{script} did not push a DOUBLE");
            };
            assert!((0.0..1.0).contains(&d), "{script} pushed {d}");
        }
    }
}

#[test]
fn entropy_errors_are_reported() {
    let mut stack = Stack::new();
    let err = push_secure_double(&mut stack, Err(getrandom::Error::UNSUPPORTED)).unwrap_err();
    assert!(
        err.to_string()
            .starts_with("Unable to obtain random data from the operating system ("),
        "{err}"
    );
    assert_eq!(stack.depth(), 0);
    // The top 53 bits of the entropy become the mantissa: all ones is the largest double < 1.
    push_secure_double(&mut stack, Ok(u64::MAX)).unwrap();
    assert_eq!(stack.pop().unwrap(), Value::Double(0.999_999_999_999_999_9));
    push_secure_double(&mut stack, Ok(0)).unwrap();
    assert_eq!(stack.pop().unwrap(), Value::Double(0.0));
}

#[test]
fn seeded_generator_replays_java_util_random() {
    assert_top("42 PRNG SRAND", 0.7275636800328681);
    assert_top("42 PRNG SRAND SRAND", 0.6832234717598454);
    assert_top("0 PRNG SRAND", 0.730967787376657);
    assert_top("-1 PRNG SRAND", 0.26894263088050496);
    // Reseeding restarts the sequence.
    assert_top("42 PRNG SRAND DROP 42 PRNG SRAND", 0.7275636800328681);
    assert_fails("SRAND", "SRAND seeded PRNG was not initialized.");
    assert_fails("'a' PRNG", "PRNG expects a LONG seed on top of the stack.");
    assert_fails("1.0 PRNG", "expects a LONG seed");
    assert_fails("PRNG", "Empty stack.");
    // The attribute is a LONG holding the generator state; anything else is "not initialized".
    let mut stack = Stack::new();
    stack.set_attribute(ATTRIBUTE_SEEDED_PRNG, Some(Value::Boolean(true)));
    assert!(call(&mut stack, "SRAND").is_err());
    let stack = exec("7 PRNG");
    assert!(matches!(
        stack.attribute(ATTRIBUTE_SEEDED_PRNG),
        Some(Value::Long(_))
    ));
}

#[test]
fn unary_double_functions() {
    assert_top("1 COS", 0.5403023058681398);
    // `Math.cosh(1.0)` is 1.543080634815244 while the host libm gives 1.5430806348152437 (1 ulp
    // apart), see docs/DIVERGENCES.md: only exact cases are asserted until fdlibm is ported.
    assert_top("0 COSH", 1.0);
    assert_top("0.5 ACOS", 1.0471975511965979);
    assert_top("1 SIN", 0.8414709848078965);
    assert_top("1 SINH", 1.1752011936438014);
    assert_top("0.5 ASIN", 0.5235987755982989);
    assert_top("1 TAN", 1.5574077246549023);
    assert_top("1 TANH", 0.7615941559557649);
    assert_top("1 ATAN", 0.7853981633974483);
    assert_top("-2.5 SIGNUM", -1.0);
    assert_top("2 SIGNUM", 1.0);
    assert_top("0.0 SIGNUM", 0.0);
    assert_top("-0.0 SIGNUM", -0.0);
    assert_top("NaN SIGNUM", f64::NAN);
    assert_top("-1.5 FLOOR", -2.0);
    assert_top("-1.5 CEIL", -1.0);
    assert_top("2 CEIL", 2.0);
    assert_top("2.5 RINT", 2.0);
    assert_top("3.5 RINT", 4.0);
    assert_top("-2.5 RINT", -2.0);
    assert_top("1.0 ULP", 2.220446049250313E-16);
    assert_top("1 ULP", 2.220446049250313E-16);
    assert_top("1.0 NEXTUP", 1.0000000000000002);
    assert_top("1.0 NEXTDOWN", 0.9999999999999999);
    assert_top("2 SQRT", 1.4142135623730951);
    assert_top("27 CBRT", 3.0);
    assert_top("1 EXP", 2.718281828459045);
    assert_top("1e-10 EXPM1", 1.00000000005E-10);
    assert_top("10 LOG", 2.302585092994046);
    assert_top("1000 LOG10", 3.0);
    assert_top("1e-10 LOG1P", 9.999999999500001E-11);
    assert_top("180 TORADIANS", 3.141592653589793);
    assert_top("1 TORADIANS", 0.017453292519943295);
    assert_top("PI TODEGREES", 180.0);
    assert_top("1 TODEGREES", 57.29577951308232);
    assert_fails("'a' SIN", "SIN can only operate on numerical values");
}

#[test]
fn rounding_and_absolute_value() {
    assert_top("2.5 ROUND", 3);
    assert_top("-2.5 ROUND", -2);
    assert_top("0.49999999999999994 ROUND", 0);
    assert_top("NaN ROUND", 0);
    assert_top("Infinity ROUND", i64::MAX);
    assert_top("-7 ROUND", -7);
    assert_top("-3 ABS", 3);
    assert_top("-3.25 ABS", 3.25);
    assert_top("-0.0 ABS", 0.0);
    assert_top("MINLONG ABS", i64::MIN);
}

#[test]
fn exact_unary_arithmetic() {
    assert_top("1 INCREMENTEXACT", 2);
    assert_top("1 DECREMENTEXACT", 0);
    assert_top("1 NEGATEEXACT", -1);
    assert_top("2147483647 TOINTEXACT", 2147483647);
    assert_top("-2147483648 TOINTEXACT", -2147483648);
    assert_top("2.9 TOINTEXACT", 2);
    assert_fails("MAXLONG INCREMENTEXACT", "long overflow");
    assert_fails("MINLONG DECREMENTEXACT", "long overflow");
    assert_fails("MINLONG NEGATEEXACT", "long overflow");
    assert_fails("2147483648 TOINTEXACT", "integer overflow");
    assert_fails("-2147483649 TOINTEXACT", "integer overflow");
}

#[test]
fn binary_double_functions() {
    assert_top("0.0 -0.0 MAX", 0.0);
    assert_top("0.0 -0.0 MIN", -0.0);
    assert_top("NaN 1.0 MAX", f64::NAN);
    assert_top("1.0 NaN MIN", f64::NAN);
    assert_top("1 2.5 MAX", 2.5);
    assert_top("1 2 MAX", 2);
    assert_top("1 2 MIN", 1);
    assert_top("3 -0.0 COPYSIGN", -3.0);
    assert_top("3 4 HYPOT", 5.0);
    assert_top("5 3 IEEEREMAINDER", -1.0);
    assert_top("5.5 2 IEEEREMAINDER", -0.5);
    assert_top("1.0 2.0 NEXTAFTER", 1.0000000000000002);
    assert_top("1.0 0.0 NEXTAFTER", 0.9999999999999999);
    assert_top("1 1 ATAN2", 0.7853981633974483);
    assert_fails(
        "1 'a' HYPOT",
        "HYPOT can only operate on 2 numerical values",
    );
    assert_fails("1 HYPOT", "Empty stack.");
    assert_fails("HYPOT", "Empty stack.");
}

#[test]
fn floor_division() {
    assert_top("-7 2 FLOORDIV", -4);
    assert_top("-7 2 FLOORMOD", 1);
    assert_top("7 -2 FLOORDIV", -4);
    assert_top("7 -2 FLOORMOD", -1);
    assert_top("7 2 FLOORDIV", 3);
    assert_top("7 2 FLOORMOD", 1);
    assert_top("1.9 2 FLOORDIV", 0);
    assert_top("MINLONG -1 FLOORDIV", i64::MIN);
    assert_top("MINLONG -1 FLOORMOD", 0);
    assert_fails("1 0 FLOORDIV", "/ by zero");
    assert_fails("1 0 FLOORMOD", "/ by zero");
    assert_fails("[ 1 2 ] FLOORDIV", "Empty stack.");
}

#[test]
fn exact_binary_arithmetic() {
    assert_top("1 2 ADDEXACT", 3);
    assert_top("1 2 SUBTRACTEXACT", -1);
    assert_top("3 4 MULTIPLYEXACT", 12);
    assert_top("[ 1 2 3 ] ADDEXACT", 6);
    assert_top("[ 10 2 3 ] SUBTRACTEXACT", 5);
    assert_top("[ 2 3 4 ] MULTIPLYEXACT", 24);
    assert_top("[ ] ADDEXACT", Value::Null);
    assert_top("[ 1 2 ] 10 ADDEXACT", Value::list(vec![long(11), long(12)]));
    assert_top("1.5 2.5 ADDEXACT", 3);
    assert_fails("MAXLONG 1 ADDEXACT", "long overflow");
    assert_fails("MINLONG 1 SUBTRACTEXACT", "long overflow");
    assert_fails("MAXLONG 2 MULTIPLYEXACT", "long overflow");
    assert_fails("MINLONG -1 MULTIPLYEXACT", "long overflow");
    assert_fails(
        "'a' 1 ADDEXACT",
        "ADDEXACT can only operate on 2 numerical values, or a numerical value and a list of numerical values, or a numerical value and a GTS of numerical values, or a list of numerical values.",
    );
}

#[test]
fn scalb() {
    assert_top("3 2 SCALB", 12.0);
    assert_top("3.0 2 SCALB", 12.0);
    assert_top("3.0 -1 SCALB", 1.5);
    // `Number.intValue()` keeps the low 32 bits of the scale factor.
    assert_top("3.0 4294967298 SCALB", 12.0);
    assert_top("1.0 -1074 SCALB", 4.9E-324);
    assert_top("1.0 1024 SCALB", f64::INFINITY);
    assert_top("1.0 -1075 SCALB", 0.0);
    assert_top("NaN 3 SCALB", f64::NAN);
    assert_fails(
        "3.0 2.5 SCALB",
        "SCALB is given values of incorrect type. Expecting [ TOP:int, 2:double ].",
    );
    assert_fails(
        "3.0 'a' SCALB",
        "SCALB can only operate on numerical values.",
    );
    assert_fails("'a' 2 SCALB", "SCALB can only operate on numerical values.");
    assert_fails("2 SCALB", "Empty stack.");
    assert_fails("SCALB", "Empty stack.");
}

#[test]
fn getexponent() {
    assert_top("1.0 GETEXPONENT", 0);
    assert_top("0.0 GETEXPONENT", -1023);
    assert_top("4.9E-324 GETEXPONENT", -1023);
    assert_top("NaN GETEXPONENT", 1024);
    assert_top("Infinity GETEXPONENT", 1024);
    assert_top("1024.0 GETEXPONENT", 10);
    assert_top("3 GETEXPONENT", 1);
    assert_fails(
        "'a' GETEXPONENT",
        "GETEXPONENT can only operate on numerical values.",
    );
    assert_fails("GETEXPONENT", "Empty stack.");
}

#[test]
fn ident_reads_the_configuration() {
    assert_top("IDENT", Value::Null);
    let mut stack = Stack::new();
    stack.set_config(IndexMap::from([(
        WARP_IDENT.to_owned(),
        "influxftl-test".to_owned(),
    )]));
    stack.exec_checked("IDENT").unwrap();
    assert_eq!(stack.pop().unwrap(), Value::string("influxftl-test"));
}

#[test]
fn lbounds() {
    assert_top("0.0 1.0 4 LBOUNDS", doubles(&[0.0, 0.25, 0.5, 0.75, 1.0]));
    assert_top("1 10 3 LBOUNDS", doubles(&[1.0, 4.0, 7.0, 10.0]));
    assert_top("0.1 0.7 3 LBOUNDS", doubles(&[0.1, 0.3, 0.5, 0.7]));
    assert_top("0 1 1 LBOUNDS", doubles(&[0.0, 1.0]));
    assert_fails(
        "0 1 2.0 LBOUNDS",
        "LBOUNDS expects a number of intervals on top of the stack.",
    );
    assert_fails(
        "0 1 0 LBOUNDS",
        "LBOUNDS cannot generate bounds for less than 1 interval.",
    );
    assert_fails(
        "0 1 65537 LBOUNDS",
        "LBOUNDS cannot generate bounds for more than 65536 intervals.",
    );
    // `intValue()` keeps the low 32 bits: 2^32 + 2 intervals are 2 intervals.
    assert_top("0 1 4294967298 LBOUNDS", doubles(&[0.0, 0.5, 1.0]));
    assert_fails(
        "0 'b' 2 LBOUNDS",
        "LBOUNDS expects an upper bound below the number of intervals.",
    );
    assert_fails(
        "'a' 1 2 LBOUNDS",
        "LBOUNDS expects a lower bounds below the upper bound.",
    );
    assert_fails(
        "1 1 2 LBOUNDS",
        "LBOUNDS expects the lower bound to be strictly less than the upper bound.",
    );
    assert_fails(
        "2 1 2 LBOUNDS",
        "LBOUNDS expects the lower bound to be strictly less than the upper bound.",
    );
    // The step underflows to zero when the bounds are too close.
    assert_fails(
        "0.0 4.9E-324 2 LBOUNDS",
        "LBOUNDS cannot operate if bounds are too close.",
    );
    assert_fails("1 2 LBOUNDS", "Empty stack.");
    assert_fails("LBOUNDS", "Empty stack.");
}

#[test]
fn nbounds() {
    assert_top(
        "10.0 2.0 4 NBOUNDS",
        doubles(&[8.651020499607837, 10.0, 11.348979500392163]),
    );
    assert_top("0 1 2 NBOUNDS", doubles(&[0.0]));
    assert_top(
        "0 1 3 NBOUNDS",
        doubles(&[-0.4307272992954576, 0.43072729929545744]),
    );
    assert_top("0 1 1 NBOUNDS", doubles(&[]));
    assert_fails(
        "0 1 'n' NBOUNDS",
        "NBOUNDS expects a number of intervals on top of the stack.",
    );
    assert_fails(
        "0 1 0 NBOUNDS",
        "NBOUNDS cannot generate bounds for less than 2 intervals.",
    );
    assert_fails(
        "0 1 65537 NBOUNDS",
        "NBOUNDS cannot generate bounds for more than 65536 intervals.",
    );
    assert_fails(
        "0 'a' 2 NBOUNDS",
        "NBOUNDS expects a standard deviation below the number of intervals.",
    );
    assert_fails(
        "0 0 2 NBOUNDS",
        "NBOUNDS expects a standard deviation strictly positive.",
    );
    assert_fails(
        "0 -1.0 2 NBOUNDS",
        "NBOUNDS expects a standard deviation strictly positive.",
    );
    assert_fails(
        "'a' 1 2 NBOUNDS",
        "NBOUNDS expects a mean below the standard deviation.",
    );
    assert_fails("1 2 NBOUNDS", "Empty stack.");
}
