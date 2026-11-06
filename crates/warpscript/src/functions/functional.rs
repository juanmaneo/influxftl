// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Higher-order collection functions of WarpLib: the macro-driven transforms and filters
//! (`LMAP`, `LFLATMAP`, `MMAP`, `LFILTER`, `MFILTER`, `FILTERBY`, `GROUPBY`), the sorts
//! (`LSORT`, `MSORT`, `SORTBY`, `SORTWITH`), `SHUFFLE` and `NONNULL`.
//!
//! The macros run through [`Stack::exec_macro`]; the `BREAK` / `CONTINUE` / `STOP` signals they
//! raise are not intercepted (Java does not catch them either) and propagate to the enclosing
//! loop or to the caller.
//!
//! # Natural ordering
//!
//! `LSORT`, `MSORT` and `SORTBY` use Java's natural ordering: `LONG` by value, `DOUBLE` by
//! `Double.compare` (`-0.0 < 0.0`, `NaN` greater than everything and equal to itself),
//! `STRING` by UTF-16 code unit (`String.compareTo`) and `BOOLEAN` with `false < true`. Java
//! raises a `ClassCastException` (or a `NullPointerException`) when the elements are not
//! mutually comparable; the message is replaced by a descriptive one here.

use std::cmp::Ordering;
use std::rc::Rc;

use indexmap::IndexMap;

use super::Entry;
use super::args::{fail, pop_list, pop_macro};
use super::math::entropy_error;
use crate::error::{Error, Result};
use crate::jrandom::JavaRandom;
use crate::macros::Macro;
use crate::stack::Stack;
use crate::value::{List, Value, ensure_mutable, is_frozen};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("LMAP", Entry::Stack(lmap)),
    ("LFLATMAP", Entry::Stack(lflatmap)),
    ("MMAP", Entry::Stack(mmap)),
    ("LFILTER", Entry::Stack(lfilter)),
    ("MFILTER", Entry::Stack(mfilter)),
    ("NONNULL", Entry::Stack(nonnull)),
    ("LSORT", Entry::Stack(lsort)),
    ("MSORT", Entry::Stack(msort)),
    ("SORTBY", Entry::Stack(sortby)),
    ("SORTWITH", Entry::Stack(sortwith)),
    ("SHUFFLE", Entry::Stack(shuffle)),
    ("GROUPBY", Entry::Stack(groupby)),
    ("FILTERBY", Entry::Stack(filterby)),
];

const EXPECTS_MACRO: &str = "expects a macro on top of the stack.";
const LIST_OR_MAP: &str = "operates on a list or a map created by {} or ->MAP.";

/// Pop the optional `BOOLEAN` on top of the stack (`pushIndex`, `true` by default) and the
/// value below it.
fn pop_index_flag(stack: &mut Stack) -> Result<(bool, Value)> {
    let top = stack.pop()?;
    if let Value::Boolean(flag) = top {
        let next = stack.pop()?;
        Ok((flag, next))
    } else {
        Ok((true, top))
    }
}

/// Push `args`, run the macro and pop its result.
fn apply_macro(stack: &mut Stack, m: &Rc<Macro>, args: &[Value]) -> Result<Value> {
    for arg in args {
        stack.push(arg.clone());
    }
    stack.exec_macro(m)?;
    stack.pop()
}

/// `list.get(i)` with the Java `IndexOutOfBoundsException` message when the macro shrank the
/// list under the iteration.
fn list_element(list: &List, index: usize) -> Result<Value> {
    let items = list.borrow();
    items.get(index).cloned().ok_or_else(|| {
        Error::new(format!(
            "Index {index} out of bounds for length {}",
            items.len()
        ))
    })
}

/// The entries of a map, cloned so that the macro may touch the map.
fn map_entries(value: &Value) -> Option<Vec<(Value, Value)>> {
    match value {
        Value::Map(map) => Some(
            map.borrow()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        ),
        _ => None,
    }
}

// -----------------------------------------------------------------------------------------
// LMAP, LFLATMAP, MMAP
// -----------------------------------------------------------------------------------------

/// `LMAP` / `LFLATMAP`: apply a macro to every element of a list (with its index unless the
/// optional `BOOLEAN` is `false`) and collect the results in a new list; `LFLATMAP` splices
/// the results which are lists.
fn lmap_impl(stack: &mut Stack, name: &str, flat: bool) -> Result<()> {
    let (push_index, top) = pop_index_flag(stack)?;
    let Value::Macro(m) = top else {
        return Err(fail(name, EXPECTS_MACRO));
    };
    let list = pop_list(
        stack,
        name,
        "expects a list below the macro on top of the stack.",
    )?;
    let n = list.borrow().len();
    let mut result = Vec::with_capacity(n);
    for i in 0..n {
        let element = list_element(&list, i)?;
        stack.push(element);
        if push_index {
            stack.push(i as i64);
        }
        stack.exec_macro(&m)?;
        match stack.pop()? {
            Value::List(items) if flat => result.extend(items.borrow().iter().cloned()),
            other => result.push(other),
        }
    }
    stack.push(Value::list(result));
    Ok(())
}

/// `LMAP`: map a macro over a list.
fn lmap(stack: &mut Stack) -> Result<()> {
    lmap_impl(stack, "LMAP", false)
}

/// `LFLATMAP`: map a macro over a list, splicing the results which are lists.
fn lflatmap(stack: &mut Stack) -> Result<()> {
    lmap_impl(stack, "LFLATMAP", true)
}

/// `MMAP`: apply a macro to every entry of a map (key, value and index unless the optional
/// `BOOLEAN` is `false`); the macro leaves the new key and value, collected in a new map.
fn mmap(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MMAP";
    let (push_index, top) = pop_index_flag(stack)?;
    let Value::Macro(m) = top else {
        return Err(fail(NAME, EXPECTS_MACRO));
    };
    let map = stack.pop()?;
    let Some(entries) = map_entries(&map) else {
        return Err(fail(
            NAME,
            "expects a MAP below the macro on top of the stack.",
        ));
    };
    let mut result: IndexMap<Value, Value> = IndexMap::with_capacity(entries.len());
    for (i, (key, value)) in entries.into_iter().enumerate() {
        stack.push(key);
        stack.push(value);
        if push_index {
            stack.push(i as i64);
        }
        stack.exec_macro(&m)?;
        let value = stack.pop()?;
        let key = stack.pop()?;
        result.insert(key, value);
    }
    stack.push(Value::map(result));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// LFILTER, MFILTER, NONNULL, FILTERBY
// -----------------------------------------------------------------------------------------

/// The `BOOLEAN` left by a filter macro.
fn filter_result(stack: &mut Stack, name: &str) -> Result<bool> {
    match stack.pop()? {
        Value::Boolean(keep) => Ok(keep),
        _ => Err(fail(
            name,
            "expects the filter macro to return a BOOLEAN for each element.",
        )),
    }
}

/// `LFILTER`: the elements of a list for which a macro (given the element and its index
/// unless the optional `BOOLEAN` is `false`) leaves `true`.
fn lfilter(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "LFILTER";
    let (push_index, top) = pop_index_flag(stack)?;
    let Value::Macro(m) = top else {
        return Err(fail(NAME, "expects a MACRO."));
    };
    let list = pop_list(stack, NAME, "expects a LIST.")?;
    let n = list.borrow().len();
    let mut result = Vec::new();
    for i in 0..n {
        let element = list_element(&list, i)?;
        stack.push(element.clone());
        if push_index {
            stack.push(i as i64);
        }
        stack.exec_macro(&m)?;
        if filter_result(stack, NAME)? {
            result.push(element);
        }
    }
    stack.push(Value::list(result));
    Ok(())
}

/// `MFILTER`: the entries of a map for which a macro (given the key, the value and the index
/// unless the optional `BOOLEAN` is `false`) leaves `true`.
fn mfilter(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MFILTER";
    let (push_index, top) = pop_index_flag(stack)?;
    let Value::Macro(m) = top else {
        return Err(fail(NAME, "expects a MACRO."));
    };
    let map = stack.pop()?;
    let Some(entries) = map_entries(&map) else {
        return Err(fail(NAME, "expects a MAP."));
    };
    let mut result: IndexMap<Value, Value> = IndexMap::new();
    for (i, (key, value)) in entries.into_iter().enumerate() {
        stack.push(key.clone());
        stack.push(value.clone());
        if push_index {
            stack.push(i as i64);
        }
        stack.exec_macro(&m)?;
        if filter_result(stack, NAME)? {
            result.insert(key, value);
        }
    }
    stack.push(Value::map(result));
    Ok(())
}

/// `NONNULL`: a new list without the `NULL` elements of a list.
fn nonnull(stack: &mut Stack) -> Result<()> {
    let list = pop_list(stack, "NONNULL", "operates on a list.")?;
    let kept: Vec<Value> = list
        .borrow()
        .iter()
        .filter(|v| !v.is_null())
        .cloned()
        .collect();
    stack.push(Value::list(kept));
    Ok(())
}

/// `FILTERBY`: the elements of a list for which a macro leaves `true`.
fn filterby(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "FILTERBY";
    let m = pop_macro(stack, NAME, EXPECTS_MACRO)?;
    let list = pop_list(stack, NAME, "operates on a list.")?;
    let items = list.borrow().clone();
    let mut filtered = Vec::new();
    for element in items {
        match apply_macro(stack, &m, std::slice::from_ref(&element))? {
            Value::Boolean(true) => filtered.push(element),
            Value::Boolean(false) => {}
            _ => {
                return Err(fail(
                    NAME,
                    "expects its macro to leave a BOOLEAN on the stack.",
                ));
            }
        }
    }
    stack.push(Value::list(filtered));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Natural ordering: LSORT, MSORT
// -----------------------------------------------------------------------------------------

/// `Double.compare`: `-0.0 < 0.0`, `NaN` is the greatest value and equals itself.
pub(crate) fn java_double_compare(x: f64, y: f64) -> Ordering {
    if x < y {
        Ordering::Less
    } else if x > y {
        Ordering::Greater
    } else {
        let bits = |d: f64| -> i64 {
            if d.is_nan() {
                f64::NAN.to_bits() as i64
            } else {
                d.to_bits() as i64
            }
        };
        bits(x).cmp(&bits(y))
    }
}

/// Java natural ordering (`Comparable.compareTo`) of two values of the same comparable type;
/// `None` when they are not mutually comparable.
pub(crate) fn natural_compare(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Long(x), Value::Long(y)) => Some(x.cmp(y)),
        (Value::Double(x), Value::Double(y)) => Some(java_double_compare(*x, *y)),
        (Value::String(x), Value::String(y)) => Some(x.encode_utf16().cmp(y.encode_utf16())),
        (Value::Boolean(x), Value::Boolean(y)) => Some(x.cmp(y)),
        (Value::BigDecimal(x), Value::BigDecimal(y)) => Some(x.compare(y)),
        _ => None,
    }
}

/// `Collections.sort` with the natural ordering: a stable sort of `items`, which fails when
/// they are not all of the same comparable type (Java throws `ClassCastException`). Lists of
/// fewer than two elements are never compared, hence never fail.
pub(crate) fn natural_sort(name: &str, items: &mut [Value]) -> Result<()> {
    let Some((first, rest)) = items.split_first() else {
        return Ok(());
    };
    if rest.is_empty() {
        return Ok(());
    }
    for other in rest {
        if std::mem::discriminant(first) != std::mem::discriminant(other) {
            return Err(fail(
                name,
                &format!(
                    "cannot compare {} and {}.",
                    first.type_name(),
                    other.type_name()
                ),
            ));
        }
    }
    if !matches!(
        first,
        Value::Long(_)
            | Value::Double(_)
            | Value::BigDecimal(_)
            | Value::String(_)
            | Value::Boolean(_)
    ) {
        return Err(fail(
            name,
            &format!("cannot sort elements of type {}.", first.type_name()),
        ));
    }
    items.sort_by(|a, b| natural_compare(a, b).unwrap_or(Ordering::Equal));
    Ok(())
}

/// `LSORT`: sort a list in place with the natural ordering of its elements. The list stays on
/// the stack.
fn lsort(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "LSORT";
    let Value::List(list) = stack.peek()?.clone() else {
        return Err(fail(NAME, "operates on a list."));
    };
    ensure_mutable(&list)?;
    let mut items = list.borrow().clone();
    natural_sort(NAME, &mut items)?;
    *list.borrow_mut() = items;
    Ok(())
}

/// `MSORT`: reorder the entries of a map in place by the natural ordering of its keys. The
/// map stays on the stack.
fn msort(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MSORT";
    let map = match stack.peek()? {
        Value::Map(map) if !is_frozen(map) => map.clone(),
        _ => return Err(fail(NAME, "operates on a map created by {} or ->MAP.")),
    };
    let mut keys: Vec<Value> = map.borrow().keys().cloned().collect();
    natural_sort(NAME, &mut keys)?;
    let mut entries = map.borrow_mut();
    let sorted: IndexMap<Value, Value> = keys
        .into_iter()
        .map(|key| {
            let value = entries.get(&key).cloned().unwrap_or(Value::Null);
            (key, value)
        })
        .collect();
    *entries = sorted;
    Ok(())
}

// -----------------------------------------------------------------------------------------
// SORTBY
// -----------------------------------------------------------------------------------------

/// Run the `SORTBY` macro on every element (`args` per element) and collect the sort keys,
/// which must all be `LONG`, all `DOUBLE` or all `STRING`.
fn sort_keys(stack: &mut Stack, m: &Rc<Macro>, args: &[Vec<Value>]) -> Result<Vec<Value>> {
    let mut keys: Vec<Value> = Vec::with_capacity(args.len());
    for arg in args {
        let key = apply_macro(stack, m, arg)?;
        let comparable = matches!(key, Value::Long(_) | Value::Double(_) | Value::String(_));
        let consistent = keys
            .first()
            .is_none_or(|first| std::mem::discriminant(first) == std::mem::discriminant(&key));
        if !comparable || !consistent {
            return Err(fail(
                "SORTBY",
                "expects its macro to return a non null double, long or string in a consistent manner.",
            ));
        }
        keys.push(key);
    }
    Ok(keys)
}

/// Stable sort of `items` by the parallel `keys`.
fn sort_by_keys<T>(items: Vec<T>, keys: Vec<Value>) -> Vec<T> {
    let mut pairs: Vec<(Value, T)> = keys.into_iter().zip(items).collect();
    pairs.sort_by(|a, b| natural_compare(&a.0, &b.0).unwrap_or(Ordering::Equal));
    pairs.into_iter().map(|(_, item)| item).collect()
}

/// `SORTBY`: sort a list (or the entries of a map) in place by the keys a macro computes from
/// each element (key and value for a map). The collection is pushed back.
fn sortby(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SORTBY";
    let m = pop_macro(stack, NAME, EXPECTS_MACRO)?;
    let collection = stack.pop()?;
    match &collection {
        Value::List(list) => {
            let items = list.borrow().clone();
            let args: Vec<Vec<Value>> = items.iter().map(|v| vec![v.clone()]).collect();
            let keys = sort_keys(stack, &m, &args)?;
            ensure_mutable(list)?;
            *list.borrow_mut() = sort_by_keys(items, keys);
        }
        Value::Map(map) if !is_frozen(map) => {
            let entries = map_entries(&collection).unwrap_or_default();
            let args: Vec<Vec<Value>> = entries
                .iter()
                .map(|(k, v)| vec![k.clone(), v.clone()])
                .collect();
            let keys = sort_keys(stack, &m, &args)?;
            let sorted = sort_by_keys(entries, keys);
            let mut target = map.borrow_mut();
            target.clear();
            target.extend(sorted);
        }
        _ => return Err(fail(NAME, LIST_OR_MAP)),
    }
    stack.push(collection);
    Ok(())
}

// -----------------------------------------------------------------------------------------
// SORTWITH
// -----------------------------------------------------------------------------------------

/// `TimSort.countRunAndMakeAscending` + `binarySort`: the exact comparison sequence of
/// `java.util.TimSort` for fewer than 32 elements.
fn binary_insertion_sort<T: Clone, F>(items: &mut [T], compare: &mut F) -> Result<()>
where
    F: FnMut(&T, &T) -> Result<i32>,
{
    let n = items.len();
    let mut run_hi = 2;
    if compare(&items[1], &items[0])? < 0 {
        while run_hi < n && compare(&items[run_hi], &items[run_hi - 1])? < 0 {
            run_hi += 1;
        }
        items[..run_hi].reverse();
    } else {
        while run_hi < n && compare(&items[run_hi], &items[run_hi - 1])? >= 0 {
            run_hi += 1;
        }
    }
    for start in run_hi..n {
        let pivot = items[start].clone();
        let mut left = 0;
        let mut right = start;
        while left < right {
            let mid = usize::midpoint(left, right);
            if compare(&pivot, &items[mid])? < 0 {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        items[left..=start].rotate_right(1);
    }
    Ok(())
}

/// Merge two sorted runs, taking from `left` first when the comparator says "equal".
fn merge_runs<T: Clone, F>(left: &[T], right: &[T], out: &mut Vec<T>, compare: &mut F) -> Result<()>
where
    F: FnMut(&T, &T) -> Result<i32>,
{
    let mut l = 0;
    let mut r = 0;
    while let (Some(a), Some(b)) = (left.get(l), right.get(r)) {
        if compare(b, a)? < 0 {
            out.push(b.clone());
            r += 1;
        } else {
            out.push(a.clone());
            l += 1;
        }
    }
    out.extend_from_slice(left.get(l..).unwrap_or_default());
    out.extend_from_slice(right.get(r..).unwrap_or_default());
    Ok(())
}

/// A stable sort driven by a fallible comparator (a macro): Java's `List.sort` comparison
/// sequence is reproduced for fewer than 32 elements, a bottom-up merge sort is used beyond.
pub(crate) fn sort_with<T: Clone, F>(items: &mut Vec<T>, mut compare: F) -> Result<()>
where
    F: FnMut(&T, &T) -> Result<i32>,
{
    let n = items.len();
    if n < 2 {
        return Ok(());
    }
    if n < 32 {
        return binary_insertion_sort(items, &mut compare);
    }
    let mut width = 1;
    while width < n {
        let mut merged = Vec::with_capacity(n);
        for chunk in items.chunks(2 * width) {
            let (left, right) = chunk.split_at(width.min(chunk.len()));
            merge_runs(left, right, &mut merged, &mut compare)?;
        }
        *items = merged;
        width *= 2;
    }
    Ok(())
}

/// Run the `SORTWITH` comparator macro on `args` and read its `LONG` result as a Java `int`.
fn compare_with(stack: &mut Stack, m: &Rc<Macro>, args: &[Value]) -> Result<i32> {
    let Value::Long(result) = apply_macro(stack, m, args)? else {
        return Err(fail(
            "SORTWITH",
            "was given a macro which doesn't return a LONG. This LONG must be within the bounds of a 32-bit integer.",
        ));
    };
    i32::try_from(result).map_err(|_| Error::new("integer overflow"))
}

/// The error `SORTWITH` raises when its comparator fails.
fn comparator_error(cause: Error) -> Error {
    Error::with_cause(
        format!(
            "SORTWITH encountered an error with comparator: {}",
            cause.message()
        ),
        cause,
    )
}

/// `SORTWITH`: sort a list (or the entries of a map) in place with a comparator macro, which
/// is given two elements (key and value of two entries for a map) and leaves a `LONG`. The
/// collection is pushed back.
fn sortwith(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SORTWITH";
    let m = pop_macro(stack, NAME, EXPECTS_MACRO)?;
    let collection = stack.pop()?;
    match &collection {
        Value::List(list) => {
            ensure_mutable(list)?;
            let mut items = list.borrow().clone();
            let outcome = sort_with(&mut items, |a, b| {
                compare_with(stack, &m, &[a.clone(), b.clone()])
            });
            match outcome {
                Ok(()) => {}
                Err(err) if err.is_signal() => return Err(err),
                Err(err) => return Err(comparator_error(err)),
            }
            *list.borrow_mut() = items;
        }
        Value::Map(map) if !is_frozen(map) => {
            let mut entries = map_entries(&collection).unwrap_or_default();
            sort_with(&mut entries, |a, b| {
                compare_with(
                    stack,
                    &m,
                    &[a.0.clone(), a.1.clone(), b.0.clone(), b.1.clone()],
                )
            })
            .map_err(comparator_error)?;
            let mut target = map.borrow_mut();
            target.clear();
            target.extend(entries);
        }
        _ => return Err(fail(NAME, LIST_OR_MAP)),
    }
    stack.push(collection);
    Ok(())
}

// -----------------------------------------------------------------------------------------
// SHUFFLE, GROUPBY
// -----------------------------------------------------------------------------------------

/// `Collections.shuffle(list, rnd)`: swap every position, from the last one down, with a
/// random earlier one.
pub(crate) fn shuffle_with(items: &mut [Value], rnd: &mut JavaRandom) {
    for i in (1..items.len()).rev() {
        let j = rnd.next_int_bounded(i as i32 + 1) as usize;
        items.swap(i, j);
    }
}

/// `SHUFFLE` seeded with `entropy`: shuffle a list in place. The list stays on the stack.
pub(crate) fn shuffle_impl(
    stack: &mut Stack,
    entropy: core::result::Result<u64, getrandom::Error>,
) -> Result<()> {
    let Value::List(list) = stack.peek()?.clone() else {
        return Err(fail("SHUFFLE", "operates on a list."));
    };
    let seed = entropy.map_err(entropy_error)? as i64;
    if list.borrow().len() > 1 {
        ensure_mutable(&list)?;
    }
    let mut rnd = JavaRandom::new(seed);
    shuffle_with(&mut list.borrow_mut(), &mut rnd);
    Ok(())
}

/// `SHUFFLE`: shuffle a list in place with a `java.util.Random` seeded from the operating
/// system.
fn shuffle(stack: &mut Stack) -> Result<()> {
    shuffle_impl(stack, getrandom::u64())
}

/// `GROUPBY`: group the elements of a list by the key a macro computes for each of them; a
/// new list of `[ key [ elements ] ]` pairs in first-seen order.
fn groupby(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "GROUPBY";
    let m = pop_macro(stack, NAME, EXPECTS_MACRO)?;
    let list = pop_list(stack, NAME, "operates on a list.")?;
    let items = list.borrow().clone();
    let mut groups: IndexMap<Value, Vec<Value>> = IndexMap::new();
    for element in items {
        let key = apply_macro(stack, &m, std::slice::from_ref(&element))?;
        groups.entry(key).or_default().push(element);
    }
    let result: Vec<Value> = groups
        .into_iter()
        .map(|(key, members)| Value::list(vec![key, Value::list(members)]))
        .collect();
    stack.push(Value::list(result));
    Ok(())
}
