// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Quaternion functions of WarpLib: `->Q`, `Q->`, `QCONJUGATE`, `QMULTIPLY`, `QDIVIDE`,
//! `QROTATE`, `QROTATION` and `ROTATIONQ`.
//!
//! A quaternion is packed in a `LONG`: the four components of the unit quaternion are
//! quantised on 16 bits each (`w` in the most significant bits, then `x`, `y`, `z`).

use super::Entry;
use super::args::{fail, fail_with_cause};
use crate::error::{Error, Result};
use crate::format::java_double_to_string;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("->Q", Entry::Stack(toquaternion)),
    ("Q->", Entry::Stack(quaternionto)),
    ("QCONJUGATE", Entry::Stack(qconjugate)),
    ("QMULTIPLY", Entry::Stack(qmultiply)),
    ("QDIVIDE", Entry::Stack(qdivide)),
    ("QROTATE", Entry::Stack(qrotate)),
    ("QROTATION", Entry::Stack(qrotation)),
    ("ROTATIONQ", Entry::Stack(rotationq)),
];

/// Tolerance on the norm of a decoded quaternion (`QUATERNIONTO.PRECISION`).
const PRECISION: f64 = 4.0 / 65535.0;

/// `TOQUATERNION.toQuaternion`: normalise and pack the components into a `LONG`.
// The quantisation mirrors the Java expression `((c + 1.0) / 2.0) * 65535` literally.
#[allow(clippy::manual_midpoint)]
pub fn to_quaternion(w: f64, x: f64, y: f64, z: f64) -> i64 {
    let norm = (w * w + x * x + y * y + z * z).sqrt();
    let (w, x, y, z) = if norm == 0.0 {
        (w, x, y, z)
    } else {
        (w / norm, x / norm, y / norm, z / norm)
    };
    // `(int) Math.floor(((c + 1) / 2) * 65535)`, kept on 16 bits.
    let quantise = |c: f64| (((c + 1.0) / 2.0) * 65535.0).floor() as i32 as i64 & 0xffff;
    (quantise(w) << 48) | (quantise(x) << 32) | (quantise(y) << 16) | quantise(z)
}

/// `QUATERNIONTO.fromQuaternion`: unpack the components of a `LONG`; the norm must be within
/// `PRECISION` of 1.
pub fn from_quaternion(q: i64) -> Result<[f64; 4]> {
    let component = |shift: u32| 2.0 * (f64::from(((q >> shift) & 0xffff) as i32) / 65535.0) - 1.0;
    let elements = [component(48), component(32), component(16), component(0)];
    let norm: f64 = elements.iter().map(|c| c * c).sum();
    if (1.0 - norm).abs() > PRECISION {
        return Err(Error::new(format!(
            "Input does not appear to be a unit quaternion, norm was {}.",
            java_double_to_string(norm)
        )));
    }
    Ok(elements)
}

/// `->Q`: pack four `DOUBLE` components (w x y z, z on top) into a quaternion.
fn toquaternion(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "->Q";
    const MESSAGE: &str =
        "expects the quaternion components w x y z as doubles on the stack (z on top).";
    let mut components = [0.0; 4];
    for component in components.iter_mut().rev() {
        let Value::Double(d) = stack.pop()? else {
            return Err(fail(NAME, MESSAGE));
        };
        *component = d;
    }
    if components.iter().any(|c| !c.is_finite()) {
        return Err(fail(NAME, "expects finite arguments."));
    }
    let [w, x, y, z] = components;
    stack.push(Value::Long(to_quaternion(w, x, y, z)));
    Ok(())
}

/// `Q->`: unpack a quaternion into its four components (z on top).
fn quaternionto(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "Q->";
    let Value::Long(q) = stack.pop()? else {
        return Err(fail(
            NAME,
            "expects a quaternion as a LONG on top of the stack.",
        ));
    };
    let elements = from_quaternion(q).map_err(|err| fail_with_cause(NAME, "failed.", err))?;
    for component in elements {
        stack.push(Value::Double(component));
    }
    Ok(())
}

/// Pop a quaternion with the given error message when the top is not a `LONG`.
fn pop_quaternion(stack: &mut Stack, name: &str, message: &str) -> Result<[f64; 4]> {
    let Value::Long(q) = stack.pop()? else {
        return Err(fail(name, message));
    };
    from_quaternion(q)
}

/// `QCONJUGATE`: the conjugate of a quaternion.
fn qconjugate(stack: &mut Stack) -> Result<()> {
    let [w, x, y, z] = pop_quaternion(stack, "QCONJUGATE", "can only operate on a quaternion.")?;
    stack.push(Value::Long(to_quaternion(w, -x, -y, -z)));
    Ok(())
}

/// Pop the two quaternions of `QMULTIPLY` / `QDIVIDE`: `(q, r)` with `r` on top.
fn pop_pair(stack: &mut Stack, name: &str) -> Result<([f64; 4], [f64; 4])> {
    let r = pop_quaternion(stack, name, "expects a quaternion on top of the stack.")?;
    let q = pop_quaternion(
        stack,
        name,
        "expects another quaternion below the quaternion on top of the stack.",
    )?;
    Ok((q, r))
}

/// `QMULTIPLY`: the Hamilton product `r * q` of the two quaternions on the stack.
#[allow(clippy::many_single_char_names)]
fn qmultiply(stack: &mut Stack) -> Result<()> {
    let (q, r) = pop_pair(stack, "QMULTIPLY")?;
    let w = r[0] * q[0] - r[1] * q[1] - r[2] * q[2] - r[3] * q[3];
    let x = r[0] * q[1] + r[1] * q[0] - r[2] * q[3] + r[3] * q[2];
    let y = r[0] * q[2] + r[1] * q[3] + r[2] * q[0] - r[3] * q[1];
    let z = r[0] * q[3] - r[1] * q[2] + r[2] * q[1] + r[3] * q[0];
    stack.push(Value::Long(to_quaternion(w, x, y, z)));
    Ok(())
}

/// `QDIVIDE`: the quotient of the two quaternions on the stack.
#[allow(clippy::many_single_char_names)]
fn qdivide(stack: &mut Stack) -> Result<()> {
    let (q, r) = pop_pair(stack, "QDIVIDE")?;
    let n = r[0] * r[0] + r[1] * r[1] + r[2] * r[2] + r[3] * r[3];
    let w = r[0] * q[0] + r[1] * q[1] + r[2] * q[2] + r[3] * q[3];
    let x = r[0] * q[1] - r[1] * q[0] - r[2] * q[3] + r[3] * q[2];
    let y = r[0] * q[2] + r[1] * q[3] - r[2] * q[0] - r[3] * q[1];
    let z = r[0] * q[3] - r[1] * q[2] + r[2] * q[1] - r[3] * q[0];
    stack.push(Value::Long(to_quaternion(w / n, x / n, y / n, z / n)));
    Ok(())
}

/// `QROTATE`: rotate the vector `x y z` (doubles below the quaternion) by a quaternion.
fn qrotate(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "QROTATE";
    let Value::Long(top) = stack.pop()? else {
        return Err(fail(NAME, "expects a quaternion on top of the stack."));
    };
    let zo = stack.pop()?;
    let yo = stack.pop()?;
    let xo = stack.pop()?;
    let (Value::Double(z), Value::Double(y), Value::Double(x)) = (zo, yo, xo) else {
        return Err(fail(
            NAME,
            "operates on a vector whose x,y,z coordinates are doubles below the quaternion.",
        ));
    };
    let q = from_quaternion(top)?;
    let v0 = (1.0 - 2.0 * q[2] * q[2] - 2.0 * q[3] * q[3]) * x
        + 2.0 * (q[1] * q[2] + q[0] * q[3]) * y
        + 2.0 * (q[1] * q[3] - q[0] * q[2]) * z;
    let v1 = 2.0 * (q[1] * q[2] - q[0] * q[3]) * x
        + (1.0 - 2.0 * q[1] * q[1] - 2.0 * q[3] * q[3]) * y
        + 2.0 * (q[2] * q[3] + q[0] * q[1]) * z;
    let v2 = 2.0 * (q[1] * q[3] + q[0] * q[2]) * x
        + 2.0 * (q[2] * q[3] - q[0] * q[1]) * y
        + (1.0 - 2.0 * q[1] * q[1] - 2.0 * q[2] * q[2]) * z;
    stack.push(Value::Double(v0));
    stack.push(Value::Double(v1));
    stack.push(Value::Double(v2));
    Ok(())
}

/// `QROTATION`: the rotation axis (x y z) and angle in degrees of a quaternion.
fn qrotation(stack: &mut Stack) -> Result<()> {
    let mut q = pop_quaternion(stack, "QROTATION", "can only operate on a quaternion.")?;
    let angle = q[0].acos();
    let sin = angle.sin();
    if sin != 0.0 {
        q[1] /= sin;
        q[2] /= sin;
        q[3] /= sin;
    }
    stack.push(Value::Double(q[1]));
    stack.push(Value::Double(q[2]));
    stack.push(Value::Double(q[3]));
    stack.push(Value::Double((angle * 2.0).to_degrees()));
    Ok(())
}

/// `ROTATIONQ`: the quaternion of a rotation of `angle` degrees around the axis `x y z`.
fn rotationq(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ROTATIONQ";
    const MESSAGE: &str = "expects a vector coordinates and an angle in degrees on the stack.";
    let mut values = [0.0; 4];
    for value in values.iter_mut().rev() {
        let Value::Double(d) = stack.pop()? else {
            return Err(fail(NAME, MESSAGE));
        };
        *value = d;
    }
    let [mut x, mut y, mut z, angle] = values;
    let norm = (x * x + y * y + z * z).sqrt();
    if norm != 0.0 {
        x /= norm;
        y /= norm;
        z /= norm;
    }
    let half = (angle / 2.0).to_radians();
    let (sin, cos) = half.sin_cos();
    stack.push(Value::Long(to_quaternion(cos, x * sin, y * sin, z * sin)));
    Ok(())
}
