// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Nested lists as tensors: `SHAPE`, `CHECKSHAPE`, `HULLSHAPE`, `RESHAPE` and `PERMUTE`.

use std::collections::HashSet;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::collections::{flatten_value, nested_get};
use crate::error::{Error, Result};
use crate::stack::Stack;
use crate::value::{List, Value};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("SHAPE", Entry::Stack(shape)),
    ("CHECKSHAPE", Entry::Stack(checkshape)),
    ("HULLSHAPE", Entry::Stack(hullshape)),
    ("RESHAPE", Entry::Stack(reshape)),
    ("PERMUTE", Entry::Stack(permute)),
];

/// The `IndexOutOfBoundsException` of `List.get`.
fn list_index_error(index: i64, length: usize) -> Error {
    Error::runtime(
        "java.lang.IndexOutOfBoundsException",
        format!("Index {index} out of bounds for length {length}"),
    )
}

/// `SHAPE.candidate_shape`: the sizes of the first element of each nesting level.
fn candidate_shape(list: &List) -> Vec<i64> {
    let mut shape = Vec::new();
    let mut current = Rc::clone(list);
    loop {
        let first = {
            let items = current.borrow();
            shape.push(items.len() as i64);
            items.first().cloned()
        };
        match first {
            Some(Value::List(next)) => current = next,
            _ => break,
        }
    }
    shape
}

/// `CHECKSHAPE.recValidateShape`: whether every nested list has the size of its level in
/// `shape` (never empty).
fn validate_shape(list: &List, shape: &[i64]) -> bool {
    let items = list.borrow();
    if items.len() as i64 != shape[0] {
        return false;
    }
    let rest = &shape[1..];
    if !items.iter().any(|item| matches!(item, Value::List(_))) {
        return rest.is_empty();
    }
    !rest.is_empty()
        && items.iter().all(|item| match item {
            Value::List(nested) => validate_shape(nested, rest),
            _ => false,
        })
}

/// Pop the list of `SHAPE` / `CHECKSHAPE` / `HULLSHAPE`.
fn pop_list(stack: &mut Stack, name: &str) -> Result<List> {
    match stack.pop()? {
        Value::List(list) => Ok(list),
        _ => Err(fail(name, "expects a LIST.")),
    }
}

/// The message of a list which is not a tensor.
const NOT_A_TENSOR: &str = "expects that the sizes of the nested lists are coherent together to form a tensor (or multidimensional array).";

/// Pop the optional `fast` flag of `SHAPE` / `PERMUTE` and the value below it.
fn pop_fast(stack: &mut Stack) -> Result<(bool, Value)> {
    match stack.pop()? {
        Value::Boolean(fast) => Ok((fast, stack.pop()?)),
        other => Ok((false, other)),
    }
}

/// The list of `LONG`s of a shape.
fn shape_value(shape: &[i64]) -> Value {
    Value::list(shape.iter().map(|&d| Value::Long(d)).collect())
}

/// `SHAPE`: the dimensions of a tensor (`fast`: of its first elements, unchecked).
fn shape(stack: &mut Stack) -> Result<()> {
    let (fast, top) = pop_fast(stack)?;
    let Value::List(list) = top else {
        return Err(fail("SHAPE", "expects a LIST."));
    };
    let shape = candidate_shape(&list);
    if !fast && !validate_shape(&list, &shape) {
        return Err(fail("SHAPE", NOT_A_TENSOR));
    }
    stack.push(shape_value(&shape));
    Ok(())
}

/// `CHECKSHAPE`: whether a list is a tensor.
fn checkshape(stack: &mut Stack) -> Result<()> {
    let list = pop_list(stack, "CHECKSHAPE")?;
    let shape = candidate_shape(&list);
    stack.push(Value::Boolean(validate_shape(&list, &shape)));
    Ok(())
}

/// `HULLSHAPE.maximizeHull`: the element-wise maximum of two shapes, the longer one's extra
/// dimensions kept.
fn maximize_hull(a: Vec<i64>, b: Vec<i64>) -> Vec<i64> {
    let (smaller, mut taller) = if a.len() < b.len() { (a, b) } else { (b, a) };
    for (t, s) in taller.iter_mut().zip(smaller) {
        *t = (*t).max(s);
    }
    taller
}

/// `HULLSHAPE.recHullShape`: the smallest shape holding every nested list.
fn hull_shape(list: &List) -> Vec<i64> {
    let items = list.borrow();
    let mut hull = Vec::new();
    for item in items.iter() {
        if let Value::List(nested) = item {
            hull = maximize_hull(hull, hull_shape(nested));
        }
    }
    let mut shape = vec![items.len() as i64];
    shape.extend(hull);
    shape
}

/// `HULLSHAPE`: the hull of the shapes of the nested lists.
fn hullshape(stack: &mut Stack) -> Result<()> {
    let list = pop_list(stack, "HULLSHAPE")?;
    stack.push(shape_value(&hull_shape(&list)));
    Ok(())
}

/// `RESHAPE`: the flattened elements of a list arranged in nested lists of the given shape.
fn reshape(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "RESHAPE";
    let Value::List(shape) = stack.pop()? else {
        return Err(fail(NAME, "expects a shape (a list of dimensions)."));
    };
    let mut dims = Vec::new();
    let mut ncells = 1i64;
    for dim in shape.borrow().iter() {
        let Value::Long(dim) = dim else {
            return Err(fail(NAME, "expects dimensions to be LONGs."));
        };
        if *dim <= 0 {
            return Err(fail(NAME, "dimension cannot be negative or null."));
        }
        ncells = ncells.wrapping_mul(*dim);
        dims.push(*dim);
    }
    // Warp 10 peeks then pops: the operand is consumed whatever its type.
    let top = stack.pop()?;
    if !matches!(top, Value::List(_)) {
        return Err(fail(NAME, "operates on a list."));
    }
    let mut elts = flatten_value(&top, &mut 0)?;
    if elts.len() as i64 != ncells {
        return Err(fail(
            NAME,
            &format!("expected {ncells} elements, but found {}", elts.len()),
        ));
    }
    for &dim in dims.iter().skip(1).rev() {
        // `intValue()`.
        let dim = dim as i32;
        let len = elts.len() as i32;
        if dim == 0 {
            return Err(Error::runtime("java.lang.ArithmeticException", "/ by zero"));
        }
        let size = len / dim;
        if size == 0 {
            return Err(fail(
                NAME,
                &format!("invalid shape, cannot create {dim} elements from {len}"),
            ));
        }
        if size < 0 {
            return Err(Error::runtime(
                "java.lang.IllegalArgumentException",
                format!("Illegal Capacity: {size}"),
            ));
        }
        let mut grouped = Vec::with_capacity(size as usize);
        let mut idx = 0i32;
        while idx < len {
            let end = idx.wrapping_add(dim);
            if end > len {
                return Err(Error::runtime(
                    "java.lang.IndexOutOfBoundsException",
                    format!("toIndex = {end}"),
                ));
            }
            grouped.push(Value::list(elts[idx as usize..end as usize].to_vec()));
            idx = end;
        }
        elts = grouped;
    }
    stack.push(Value::list(elts));
    Ok(())
}

/// `PERMUTE.recPermute`: the elements of the permuted tensor at `indices` and below.
fn rec_permute(
    tensor: &List,
    indices: &[i64],
    pattern: &[i64],
    new_shape: &[i64],
) -> Result<Vec<Value>> {
    let dimension = indices.len();
    let Some(&size) = new_shape.get(dimension) else {
        return Err(list_index_error(dimension as i64, new_shape.len()));
    };
    let mut result = Vec::new();
    let mut i = 0i64;
    while i < size {
        let mut new_indices = indices.to_vec();
        new_indices.push(i);
        if new_shape.len() - 1 == dimension {
            let mut permuted = Vec::with_capacity(pattern.len());
            for r in 0..pattern.len() as i64 {
                // `new_indices` has an index per axis of the pattern.
                let Some(at) = pattern.iter().rposition(|&p| p == r) else {
                    return Err(list_index_error(-1, new_indices.len()));
                };
                permuted.push(new_indices.get(at).copied().unwrap_or_default());
            }
            result.push(nested_get(tensor, &permuted)?);
        } else {
            result.push(Value::list(rec_permute(
                tensor,
                &new_indices,
                pattern,
                new_shape,
            )?));
        }
        i += 1;
    }
    Ok(result)
}

/// `PERMUTE`: the tensor with its axes permuted (`fast`: unchecked shape).
fn permute(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PERMUTE";
    const PATTERN: &str = "expects a LIST of LONG as second argument.";
    let (fast, top) = pop_fast(stack)?;
    let Value::List(pattern) = top else {
        return Err(fail(NAME, PATTERN));
    };
    let mut axes = Vec::new();
    for axis in pattern.borrow().iter() {
        let Value::Long(axis) = axis else {
            return Err(fail(NAME, PATTERN));
        };
        axes.push(*axis);
    }
    let Value::List(tensor) = stack.pop()? else {
        return Err(fail(NAME, "expects a LIST as first argument."));
    };
    if axes.iter().collect::<HashSet<_>>().len() < axes.len() {
        return Err(fail(NAME, "error: duplicate axis in permutation pattern."));
    }
    let shape = candidate_shape(&tensor);
    if !fast && !validate_shape(&tensor, &shape) {
        return Err(fail(NAME, NOT_A_TENSOR));
    }
    let mut new_shape = Vec::with_capacity(axes.len());
    for &axis in &axes {
        // `intValue()`.
        let axis = axis as i32;
        let Some(&dim) = usize::try_from(axis).ok().and_then(|a| shape.get(a)) else {
            return Err(list_index_error(i64::from(axis), shape.len()));
        };
        new_shape.push(dim);
    }
    let result = rec_permute(&tensor, &[], &axes, &new_shape)?;
    stack.push(Value::list(result));
    Ok(())
}
