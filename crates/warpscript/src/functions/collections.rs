// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Collection functions of WarpLib: conversions between the stack and lists, sets and maps
//! (`LIST->`, `UNLIST`, `MAP->`…), set algebra, element access and mutation (`GET`, `SET`,
//! `PUT`, `REMOVE`, `SUBLIST`, `SUBMAP`…), structural helpers (`FLATTEN`, `ZIP`, `CLONE`,
//! `REVERSE`, `UNIQUE`, `SIZE`), `MAPID` and the bulk variable stores (`LSTORE`, `MSTORE` and
//! their conditional variants).
//!
//! The macro-driven functions (`LMAP`, `LFILTER`, `SORTBY`…) and the sorts live in
//! [`super::functional`], `PACK` / `UNPACK` in [`super::pack`].
//!
//! # Reference semantics
//!
//! Lists, maps and sets are shared handles: the functions that mutate their operand in Java
//! (`SET`, `PUT`, `REMOVE`, `REVERSE`, `APPEND`…) mutate the shared object here too, while the
//! ones that build a new collection (`CLONEREVERSE`, `SUBLIST`, `UNIQUE`…) leave the operand
//! untouched. Byte arrays are immutable in this runtime, so `SET` and `REVERSE` on `BYTES`
//! push a modified copy instead of altering the operand in place.
//!
//! # Operands that are not modelled
//!
//! Geo Time Series, encoders, matrices and vectors are not modelled yet (`->V` builds a plain
//! list); the functions that accept them in Java raise the error a script gets for an
//! unsupported type. Strings are indexed by UTF-16 code unit like in Java. See
//! `docs/DIVERGENCES.md`.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};

use super::Entry;
use super::args::{compute_and_check_index, fail, fail_with_cause, pop_list, pop_long, pop_map};
use super::gts_reduce::{java_hash_order, java_string_hash, table_size_for};
use crate::error::{Error, Result};
use crate::gts::{GeoTimeSerie, Gts};
use crate::stack::Stack;
use crate::value::{List, Value};

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("LIST->", Entry::Stack(listto)),
    ("UNLIST", Entry::Stack(unlist)),
    ("UNSET", Entry::Stack(unset)),
    ("SET->", Entry::Stack(setto)),
    ("V->", Entry::Stack(vectorto)),
    ("MAP->", Entry::Stack(mapto)),
    ("UNMAP", Entry::Stack(unmap)),
    ("MAPID", Entry::Stack(mapid)),
    ("UNION", Entry::Stack(union)),
    ("INTERSECTION", Entry::Stack(intersection)),
    ("DIFFERENCE", Entry::Stack(difference)),
    ("GET", Entry::Stack(get)),
    ("SET", Entry::Stack(set)),
    ("PUT", Entry::Stack(put)),
    ("SUBMAP", Entry::Stack(submap)),
    ("SUBLIST", Entry::Stack(sublist)),
    ("KEYLIST", Entry::Stack(keylist)),
    ("VALUELIST", Entry::Stack(valuelist)),
    ("SIZE", Entry::Stack(size)),
    ("REMOVE", Entry::Stack(remove)),
    ("INDEXOF", Entry::Stack(indexof)),
    ("UNIQUE", Entry::Stack(unique)),
    ("CONTAINS", Entry::Stack(contains)),
    ("CONTAINSKEY", Entry::Stack(containskey)),
    ("CONTAINSVALUE", Entry::Stack(containsvalue)),
    ("REVERSE", Entry::Stack(reverse)),
    ("CLONEREVERSE", Entry::Stack(clonereverse)),
    ("APPEND", Entry::Stack(append)),
    ("FLATTEN", Entry::Stack(flatten)),
    ("ZIP", Entry::Stack(zip)),
    ("CLONE", Entry::Stack(clone_fn)),
    ("PIVOT", Entry::Stack(pivot)),
    ("PIVOTSTRICT", Entry::Stack(pivotstrict)),
    ("LSTORE", Entry::Stack(lstore)),
    ("LCSTORE", Entry::Stack(lcstore)),
    ("MSTORE", Entry::Stack(mstore)),
    ("MCSTORE", Entry::Stack(mcstore)),
];

const OPERATES_ON_MAP: &str = "operates on a map.";
const NESTED_PATH: &str =
    "tried to set an element at a nested path that does not exist in the input list.";
const VARIABLE_NAMES: &str =
    "expects a list of variable names or register numbers as second argument.";

/// Java `ArrayList.set` / `remove` on an invalid index (`Objects.checkIndex`).
fn index_out_of_bounds(index: i64, length: usize) -> Error {
    Error::new(format!("Index {index} out of bounds for length {length}"))
}

/// The string made of a single UTF-16 code unit (`String.valueOf(char)`); a lone surrogate
/// becomes U+FFFD.
fn utf16_unit_string(unit: u16) -> String {
    String::from_utf16_lossy(&[unit])
}

/// The elements of a list or set (Java `Collection`), `None` for anything else.
fn collection_items(value: &Value) -> Option<Vec<Value>> {
    match value {
        Value::List(list) => Some(list.borrow().clone()),
        Value::Set(set) => Some(set.borrow().iter().cloned().collect()),
        _ => None,
    }
}

/// Replace the element at `index`; out of range indices (which the callers have already
/// ruled out) are ignored.
pub(crate) fn set_at<T>(items: &mut [T], index: usize, value: T) {
    if let Some(slot) = items.get_mut(index) {
        *slot = value;
    }
}

// -----------------------------------------------------------------------------------------
// Conversions between collections and the stack
// -----------------------------------------------------------------------------------------

/// `LIST->`: push the elements of a list, then its size.
fn listto(stack: &mut Stack) -> Result<()> {
    let list = pop_list(stack, "LIST->", "operates on a list.")?;
    let items = list.borrow().clone();
    let n = items.len();
    for item in items {
        stack.push(item);
    }
    stack.push(n as i64);
    Ok(())
}

/// `UNLIST`: push a `MARK` followed by the elements of a list.
fn unlist(stack: &mut Stack) -> Result<()> {
    let list = pop_list(stack, "UNLIST", "expects a LIST.")?;
    let items = list.borrow().clone();
    stack.push(Value::Mark);
    for item in items {
        stack.push(item);
    }
    Ok(())
}

/// `UNSET`: push a `MARK` followed by the elements of a set.
fn unset(stack: &mut Stack) -> Result<()> {
    let Value::Set(set) = stack.pop()? else {
        return Err(fail("UNSET", "expects a SET."));
    };
    let items: Vec<Value> = set.borrow().iter().cloned().collect();
    stack.push(Value::Mark);
    for item in items {
        stack.push(item);
    }
    Ok(())
}

/// `SET->`: convert a set into a new list.
fn setto(stack: &mut Stack) -> Result<()> {
    let Value::Set(set) = stack.pop()? else {
        return Err(fail("SET->", "operates on a SET."));
    };
    let items: Vec<Value> = set.borrow().iter().cloned().collect();
    stack.push(Value::list(items));
    Ok(())
}

/// `V->`: convert a vector into a new list. Vectors are modelled as plain lists, so any list
/// is accepted.
fn vectorto(stack: &mut Stack) -> Result<()> {
    let list = pop_list(stack, "V->", "expects a vector on top of the stack.")?;
    let items = list.borrow().clone();
    stack.push(Value::list(items));
    Ok(())
}

/// `MAP->`: push the key and value of every entry of a map, then twice the number of entries.
fn mapto(stack: &mut Stack) -> Result<()> {
    let Value::Map(map) = stack.pop()? else {
        return Err(Error::new("Invalid type, expected a map."));
    };
    let entries: Vec<(Value, Value)> = map
        .borrow()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let n = entries.len();
    for (key, value) in entries {
        stack.push(key);
        stack.push(value);
    }
    stack.push((n * 2) as i64);
    Ok(())
}

/// `UNMAP`: push a `MARK` followed by the key and value of every entry of a map.
fn unmap(stack: &mut Stack) -> Result<()> {
    let map = pop_map(stack, "UNMAP", "expects a MAP.")?;
    let entries: Vec<(Value, Value)> = map
        .borrow()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    stack.push(Value::Mark);
    for (key, value) in entries {
        stack.push(key);
        stack.push(value);
    }
    Ok(())
}

// -----------------------------------------------------------------------------------------
// MAPID (SipHash based identifier of a map)
// -----------------------------------------------------------------------------------------

/// First SipHash key of `MAPID` (most significant half of the UUID).
const MAPID_KEY1: (u64, u64) = (0x39A9_DD7D_71B6_4E3C, 0xA14C_3749_DCAA_B408);
/// Second SipHash key of `MAPID` (least significant half of the UUID).
const MAPID_KEY2: (u64, u64) = (0xB5BB_EC10_71A6_4C48, 0xB872_C16B_37A0_7597);

/// One round of SipHash.
fn sipround(v: &mut [u64; 4]) {
    v[0] = v[0].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(13);
    v[1] ^= v[0];
    v[0] = v[0].rotate_left(32);
    v[2] = v[2].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(16);
    v[3] ^= v[2];
    v[0] = v[0].wrapping_add(v[3]);
    v[3] = v[3].rotate_left(21);
    v[3] ^= v[0];
    v[2] = v[2].wrapping_add(v[1]);
    v[1] = v[1].rotate_left(17);
    v[1] ^= v[2];
    v[2] = v[2].rotate_left(32);
}

/// SipHash-2-4 of `data` with the key words `k0`, `k1` (`SipHashInline.hash24`).
pub(crate) fn siphash24(k0: u64, k1: u64, data: &[u8]) -> u64 {
    let mut v = [
        k0 ^ 0x736f_6d65_7073_6575,
        k1 ^ 0x646f_7261_6e64_6f6d,
        k0 ^ 0x6c79_6765_6e65_7261,
        k1 ^ 0x7465_6462_7974_6573,
    ];
    let (chunks, remainder) = data.as_chunks::<8>();
    for chunk in chunks {
        let m = u64::from_le_bytes(*chunk);
        v[3] ^= m;
        sipround(&mut v);
        sipround(&mut v);
        v[0] ^= m;
    }
    let mut b = (data.len() as u64) << 56;
    for (i, &byte) in remainder.iter().enumerate() {
        b |= u64::from(byte) << (8 * i);
    }
    v[3] ^= b;
    sipround(&mut v);
    sipround(&mut v);
    v[0] ^= b;
    v[2] ^= 0xff;
    for _ in 0..4 {
        sipround(&mut v);
    }
    v[0] ^ v[1] ^ v[2] ^ v[3]
}

/// `SipHashInline.hash24_palindromic`: SipHash-2-4 of `data` followed by its mirror image.
pub(crate) fn siphash24_palindromic(key: (u64, u64), data: &[u8]) -> u64 {
    let mut palindrome = Vec::with_capacity(data.len() * 2);
    palindrome.extend_from_slice(data);
    palindrome.extend(data.iter().rev());
    siphash24(key.0, key.1, &palindrome)
}

/// `GTSHelper.labelsId`: hash of a set of labels, independent of their order.
pub(crate) fn labels_id(key: (u64, u64), labels: &IndexMap<String, String>) -> u64 {
    let mut hashes: Vec<(i64, i64)> = labels
        .iter()
        .map(|(name, value)| {
            (
                siphash24_palindromic(key, name.as_bytes()) as i64,
                siphash24_palindromic(key, value.as_bytes()) as i64,
            )
        })
        .collect();
    hashes.sort_unstable();
    let mut buf = Vec::with_capacity(hashes.len() * 16);
    for (name_hash, value_hash) in &hashes {
        buf.extend_from_slice(&name_hash.to_be_bytes());
        buf.extend_from_slice(&value_hash.to_be_bytes());
    }
    siphash24_palindromic(key, &buf)
}

/// `new UUID(msb, lsb).toString()`: the bits are kept as they are (no version or variant).
fn raw_uuid_string(msb: u64, lsb: u64) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        msb >> 32,
        (msb >> 16) & 0xFFFF,
        msb & 0xFFFF,
        lsb >> 48,
        lsb & 0xFFFF_FFFF_FFFF
    )
}

/// `MAPID`: a UUID identifying the content of a map (keys and values converted to strings),
/// independent of the entry order.
fn mapid(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "MAPID";
    let Value::Map(map) = stack.pop()? else {
        return Err(fail(NAME, "can only operate on a map."));
    };
    let mut labels: IndexMap<String, String> = IndexMap::new();
    for (key, value) in map.borrow().iter() {
        if key.is_null() || value.is_null() {
            return Err(fail(
                NAME,
                "cannot operate on a map with NULL keys or values.",
            ));
        }
        labels.insert(key.to_string(), value.to_string());
    }
    let msb = labels_id(MAPID_KEY1, &labels);
    let lsb = labels_id(MAPID_KEY2, &labels);
    stack.push(raw_uuid_string(msb, lsb));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// Set algebra
// -----------------------------------------------------------------------------------------

/// Pop the two sets of `UNION` / `INTERSECTION` / `DIFFERENCE`: `(top, below)`.
fn pop_two_sets(stack: &mut Stack, name: &str) -> Result<(Vec<Value>, Vec<Value>)> {
    let top = stack.pop()?;
    let below = stack.pop()?;
    match (top, below) {
        (Value::Set(a), Value::Set(b)) => {
            let a: Vec<Value> = a.borrow().iter().cloned().collect();
            let b: Vec<Value> = b.borrow().iter().cloned().collect();
            Ok((a, b))
        }
        _ => Err(fail(name, "can only operate on sets.")),
    }
}

/// `UNION`: a new set with the elements of both sets.
fn union(stack: &mut Stack) -> Result<()> {
    let (a, b) = pop_two_sets(stack, "UNION")?;
    let union: IndexSet<Value> = a.into_iter().chain(b).collect();
    stack.push(Value::set(union));
    Ok(())
}

/// `INTERSECTION`: a new set with the elements present in both sets.
fn intersection(stack: &mut Stack) -> Result<()> {
    let (a, b) = pop_two_sets(stack, "INTERSECTION")?;
    let intersection: IndexSet<Value> = a.into_iter().filter(|v| b.contains(v)).collect();
    stack.push(Value::set(intersection));
    Ok(())
}

/// `DIFFERENCE`: a new set with the elements of the set below the top which are not in the set
/// on top.
fn difference(stack: &mut Stack) -> Result<()> {
    let (b, a) = pop_two_sets(stack, "DIFFERENCE")?;
    let difference: IndexSet<Value> = a.into_iter().filter(|v| !b.contains(v)).collect();
    stack.push(Value::set(difference));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// GET, SET, PUT
// -----------------------------------------------------------------------------------------

/// `GET.get`: one element of a list, map, string or byte array.
fn get_element(key: &Value, collection: &Value) -> Result<Value> {
    match collection {
        Value::List(list) => {
            let Value::Long(k) = key else {
                return Err(Error::new("Getting on LIST requires a LONG."));
            };
            let items = list.borrow();
            let idx = compute_and_check_index(*k as i32, items.len() as i32)?;
            Ok(items.get(idx as usize).cloned().unwrap_or(Value::Null))
        }
        Value::Map(map) => Ok(map.borrow().get(key).cloned().unwrap_or(Value::Null)),
        Value::String(s) => {
            let Value::Long(k) = key else {
                return Err(Error::new("Getting on STRING requires a LONG."));
            };
            let units: Vec<u16> = s.encode_utf16().collect();
            let idx = compute_and_check_index(*k as i32, units.len() as i32)?;
            let unit = units.get(idx as usize).copied().unwrap_or_default();
            Ok(Value::from(utf16_unit_string(unit)))
        }
        Value::Bytes(bytes) => {
            let Value::Long(k) = key else {
                return Err(Error::new("Getting on BYTES requires a LONG."));
            };
            let idx = compute_and_check_index(*k as i32, bytes.len() as i32)?;
            let byte = bytes.get(idx as usize).copied().unwrap_or_default();
            Ok(Value::Long(i64::from(byte)))
        }
        _ => Err(Error::new("Invalid OBJECT to GET on.")),
    }
}

/// Apply [`get_element`] successively with every key of `path`.
fn get_path(path: &[Value], collection: &Value) -> Result<Value> {
    let mut value = collection.clone();
    for key in path {
        value = get_element(key, &value)?;
    }
    Ok(value)
}

/// `GET`: the element of a collection at a key; a list key on a list is a path.
fn get(stack: &mut Stack) -> Result<()> {
    let key = stack.pop()?;
    let collection = stack.pop()?;
    let result = match (&key, &collection) {
        (Value::List(path), Value::List(_)) => {
            let path = path.borrow().clone();
            get_path(&path, &collection)
        }
        _ => get_element(&key, &collection),
    };
    let value = result.map_err(|err| fail_with_cause("GET", "failed.", err))?;
    stack.push(value);
    Ok(())
}

/// `GET.nestedGet`: follow `indices` down nested lists.
fn nested_get(root: &List, indices: &[i64]) -> Result<Value> {
    let mut current = Value::List(Rc::clone(root));
    for &index in indices {
        let next = {
            let Value::List(list) = &current else {
                return Err(Error::new(
                    "Tried to get an element at a nested path that does not exist in the input list.",
                ));
            };
            let items = list.borrow();
            let idx = compute_and_check_index(index as i32, items.len() as i32)?;
            items.get(idx as usize).cloned().unwrap_or(Value::Null)
        };
        current = next;
    }
    Ok(current)
}

/// `SET` with a list key: set the element at a nested path of a list.
fn set_nested(container: &Value, path: &List, value: Value) -> Result<()> {
    const NAME: &str = "SET";
    let mut indices = Vec::new();
    for element in path.borrow().iter() {
        let Value::Long(index) = element else {
            return Err(fail(
                NAME,
                "expects the key to be an integer or a list of integers when operating on a List.",
            ));
        };
        indices.push(*index);
    }
    let Some(last) = indices.pop() else {
        return Err(index_out_of_bounds(-1, 0));
    };
    let Value::List(root) = container else {
        return Err(fail(
            NAME,
            "expects a LONG key when operating on a byte array.",
        ));
    };
    let target = nested_get(root, &indices).map_err(|_| fail(NAME, NESTED_PATH))?;
    let Value::List(target) = target else {
        return Err(fail(NAME, NESTED_PATH));
    };
    let last = i64::from(last as i32);
    let mut items = target.borrow_mut();
    if last < 0 || last >= items.len() as i64 {
        return Err(index_out_of_bounds(last, items.len()));
    }
    set_at(&mut items, last as usize, value);
    Ok(())
}

/// The work of `SET` on `container`: `Some(bytes)` when a modified copy of a byte array must
/// replace the container on the stack, `None` when a list was modified in place.
fn set_into(container: &Value, key: &Value, value: Value) -> Result<Option<Value>> {
    const NAME: &str = "SET";
    match (container, key) {
        (Value::List(_) | Value::Bytes(_), Value::List(path)) => {
            set_nested(container, path, value)?;
            Ok(None)
        }
        (Value::List(list), Value::Long(k)) => {
            let mut items = list.borrow_mut();
            let idx = compute_and_check_index(*k as i32, items.len() as i32)?;
            set_at(&mut items, idx as usize, value);
            Ok(None)
        }
        (Value::Bytes(bytes), Value::Long(k)) => {
            let byte = match value {
                Value::Long(l) if (-128..=255).contains(&l) => l as u8,
                _ => {
                    return Err(fail(
                        NAME,
                        "expects the element to be a LONG in either [0,255] or [-128,127], when operating on a byte array.",
                    ));
                }
            };
            let idx = compute_and_check_index(*k as i32, bytes.len() as i32)?;
            let mut data = bytes.to_vec();
            set_at(&mut data, idx as usize, byte);
            Ok(Some(Value::bytes(data)))
        }
        (Value::List(_) | Value::Bytes(_), _) => Err(fail(
            NAME,
            "expects a key which is an LONG or a list of LONG.",
        )),
        _ => Err(fail(NAME, "operates on a list or byte array.")),
    }
}

/// `SET`: set the element of a list (possibly at a nested path) or of a byte array. The
/// container stays on the stack (Java peeks it), even when the function fails.
fn set(stack: &mut Stack) -> Result<()> {
    let key = stack.pop()?;
    let value = stack.pop()?;
    let container = stack.pop()?;
    match set_into(&container, &key, value) {
        Ok(Some(replacement)) => stack.push(replacement),
        Ok(None) => stack.push(container),
        Err(err) => {
            stack.push(container);
            return Err(err);
        }
    }
    Ok(())
}

/// `PUT`: put an entry into a map, or set an element of a list (no negative indices). The
/// container stays on the stack.
fn put(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "PUT";
    let key = stack.pop()?;
    let value = stack.pop()?;
    match stack.peek()? {
        Value::Map(map) => {
            map.borrow_mut().insert(key, value);
            Ok(())
        }
        Value::List(list) => {
            let Value::Long(k) = key else {
                return Err(fail(
                    NAME,
                    "expects a key which is an integer when operating on a list.",
                ));
            };
            let mut items = list.borrow_mut();
            let idx = i64::from(k as i32);
            if idx < 0 || idx >= items.len() as i64 {
                return Err(index_out_of_bounds(idx, items.len()));
            }
            set_at(&mut items, idx as usize, value);
            Ok(())
        }
        _ => Err(fail(NAME, "operates on a map or list.")),
    }
}

// -----------------------------------------------------------------------------------------
// SUBMAP, SUBLIST, KEYLIST, VALUELIST
// -----------------------------------------------------------------------------------------

/// `SUBMAP`: a new map with the entries of a map whose key is in a list.
fn submap(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SUBMAP";
    let keys = stack.pop()?;
    let map = pop_map(stack, NAME, OPERATES_ON_MAP)?;
    let Value::List(keys) = keys else {
        return Err(fail(
            NAME,
            "expects a list of map keys on top of the stack.",
        ));
    };
    let keys = keys.borrow().clone();
    let submap: IndexMap<Value, Value> = map
        .borrow()
        .iter()
        .filter(|(k, _)| keys.contains(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    stack.push(Value::map(submap));
    Ok(())
}

/// `SUBLIST.nearestValidBound`: move a bound to the closest valid index by jumping by `step`.
fn nearest_valid_bound(bound: i64, step: i64, size: i64) -> i64 {
    if bound >= size {
        bound
            .wrapping_sub(size)
            .wrapping_rem(step)
            .wrapping_sub(step.wrapping_abs())
            .wrapping_add(size)
    } else if bound < 0 {
        bound
            .wrapping_add(1)
            .wrapping_rem(step)
            .wrapping_add(step.wrapping_abs())
            .wrapping_sub(1)
    } else {
        bound
    }
}

/// The range form of `SUBLIST`: `start [end [step]]`.
fn sublist_range(items: &[Value], params: &[i64]) -> Result<Vec<Value>> {
    const NAME: &str = "SUBLIST";
    let size = items.len() as i64;
    let mut start = params.first().copied().unwrap_or_default();
    let mut end = match params.get(1) {
        Some(&end) => end,
        None => (size - 1).max(start),
    };
    if start < 0 {
        start += size;
    }
    if end < 0 {
        end += size;
    }
    let mut sublist = Vec::new();
    if (start < 0 && end < 0) || (start >= size && end >= size) {
        return Ok(sublist);
    }
    let step = match params.get(2) {
        Some(&step) => step,
        None if start > end => -1,
        None => 1,
    };
    if step == 0 {
        return Err(fail(
            NAME,
            "expects the step parameter to be a strictly positive or negative number.",
        ));
    } else if step > 0 && start > end {
        return Err(fail(
            NAME,
            "expects start to be before end when step is positive.",
        ));
    } else if step < 0 && end > start {
        return Err(fail(
            NAME,
            "expects start to be after end when step is negative.",
        ));
    }
    let ascending = step > 0;
    let mut i = nearest_valid_bound(start, step, size);
    end = if ascending {
        end.min(size - 1)
    } else {
        end.max(0)
    };
    while (ascending && i <= end) || (!ascending && i >= end) {
        sublist.push(items.get(i as usize).cloned().unwrap_or(Value::Null));
        // `Math.addExact` overflowed: the next index cannot be valid, stop (Java swallows the
        // ArithmeticException).
        match i.checked_add(step) {
            Some(next) => i = next,
            None => break,
        }
    }
    Ok(sublist)
}

/// The index list forms of `SUBLIST`: a pair of bounds, or individual indices.
fn sublist_indices(items: &[Value], indices: &[Value]) -> Result<Vec<Value>> {
    const NAME: &str = "SUBLIST";
    const NUMERIC: &str = "expects a list of indices which are numeric integers.";
    let size = items.len() as i64;
    let mut sublist = Vec::new();
    if let [Value::Long(b0), Value::Long(b1)] = indices {
        let mut bmin = i64::from(*b0 as i32);
        let mut bmax = i64::from(*b1 as i32);
        if bmin < 0 {
            bmin += size;
        }
        if bmax < 0 {
            bmax += size;
        }
        if bmin > bmax {
            std::mem::swap(&mut bmin, &mut bmax);
        }
        if (bmin < 0 && bmax < 0) || (size <= bmin && size <= bmax) {
            return Ok(sublist);
        }
        bmin = bmin.max(0);
        bmax = bmax.min(size - 1);
        for i in bmin..=bmax {
            sublist.push(items.get(i as usize).cloned().unwrap_or(Value::Null));
        }
        return Ok(sublist);
    }
    if indices.len() == 2 {
        return Err(fail(NAME, NUMERIC));
    }
    for index in indices {
        let Value::Long(index) = index else {
            return Err(fail(NAME, NUMERIC));
        };
        let idx = i64::from(*index as i32);
        if idx >= size || idx < -size {
            return Err(fail(NAME, "reported an out of bound index."));
        }
        let idx = if idx >= 0 { idx } else { size + idx };
        sublist.push(items.get(idx as usize).cloned().unwrap_or(Value::Null));
    }
    Ok(sublist)
}

/// `SUBLIST`: a new list made of a range (`start end step`, with optional `end` and `step`)
/// or of selected indices (`[ a b ]` is a range of bounds, any other list selects elements).
fn sublist(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "SUBLIST";
    const BAD_OPERANDS: &str = "expects a list of indices on top of the stack or a start end step and will operate on the list below it.";
    let mut indices: Option<Vec<Value>> = None;
    let mut elements: Option<List> = None;
    let mut params: Vec<i64> = Vec::new();
    for i in 0..4 {
        match stack.pop()? {
            Value::List(list) => {
                if i == 0 {
                    indices = Some(list.borrow().clone());
                    let Value::List(below) = stack.pop()? else {
                        return Err(fail(
                            NAME,
                            "expects a list of indices on top of the stack and will operate on the list below it.",
                        ));
                    };
                    elements = Some(below);
                } else {
                    elements = Some(list);
                }
                break;
            }
            other => {
                let Some(param) = other.as_i64() else {
                    return Err(fail(NAME, BAD_OPERANDS));
                };
                params.push(param);
            }
        }
    }
    let Some(elements) = elements else {
        return Err(fail(NAME, BAD_OPERANDS));
    };
    params.reverse();
    let items = elements.borrow().clone();
    let sublist = match indices {
        None => sublist_range(&items, &params)?,
        Some(indices) => sublist_indices(&items, &indices)?,
    };
    stack.push(Value::list(sublist));
    Ok(())
}

/// `KEYLIST`: a new list of the keys of a map.
fn keylist(stack: &mut Stack) -> Result<()> {
    let map = pop_map(stack, "KEYLIST", OPERATES_ON_MAP)?;
    let keys: Vec<Value> = map.borrow().keys().cloned().collect();
    stack.push(Value::list(keys));
    Ok(())
}

/// `VALUELIST`: a new list of the values of a map.
fn valuelist(stack: &mut Stack) -> Result<()> {
    let map = pop_map(stack, "VALUELIST", OPERATES_ON_MAP)?;
    let values: Vec<Value> = map.borrow().values().cloned().collect();
    stack.push(Value::list(values));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// SIZE, REMOVE, INDEXOF
// -----------------------------------------------------------------------------------------

/// `SIZE`: number of entries of a map, elements of a collection, UTF-16 units of a string,
/// bytes of a byte array or statements of a macro.
fn size(stack: &mut Stack) -> Result<()> {
    let n = match stack.pop()? {
        Value::Map(map) => map.borrow().len(),
        Value::List(list) => list.borrow().len(),
        Value::Set(set) => set.borrow().len(),
        Value::String(s) => s.encode_utf16().count(),
        Value::Bytes(bytes) => bytes.len(),
        // The number of values, not the number of buckets.
        Value::Gts(gts) => gts.borrow().size(),
        Value::Encoder(encoder) => {
            stack.push(encoder.borrow().count());
            return Ok(());
        }
        Value::Macro(m) => m.len(),
        _ => {
            return Err(fail(
                "SIZE",
                "operates on a MAP, a collection, a STRING, a byte array, a GTS or GTS Encoder, a GEOSHAPE or a Macro.",
            ));
        }
    };
    stack.push(n as i64);
    Ok(())
}

/// Java `Number.intValue()` of the `LONG` key of `REMOVE`, with negative indices counted
/// from the end.
fn remove_index(key: &Value, size: usize) -> Result<Option<usize>> {
    let Value::Long(k) = key else {
        return Err(fail("REMOVE", "expects an integer as key."));
    };
    let mut idx = i64::from(*k as i32);
    if idx < 0 {
        idx += size as i64;
    }
    if idx >= 0 && idx < size as i64 {
        Ok(Some(idx as usize))
    } else {
        Ok(None)
    }
}

/// `REMOVE`: remove an entry from a map, an element from a list or set (in place), a byte from
/// a byte array or a character from a string (copies). Pushes the collection then the removed
/// element (or `NULL`).
fn remove(stack: &mut Stack) -> Result<()> {
    let key = stack.pop()?;
    let collection = stack.pop()?;
    let (result, removed) = match &collection {
        Value::Map(map) => {
            let removed = map.borrow_mut().shift_remove(&key);
            (collection.clone(), removed.unwrap_or(Value::Null))
        }
        Value::List(list) => {
            let size = list.borrow().len();
            let removed = match remove_index(&key, size)? {
                Some(idx) => list.borrow_mut().remove(idx),
                None => Value::Null,
            };
            (collection.clone(), removed)
        }
        Value::Set(set) => {
            let found = set.borrow_mut().shift_remove(&key);
            (collection.clone(), if found { key } else { Value::Null })
        }
        Value::Bytes(bytes) => match remove_index(&key, bytes.len())? {
            Some(idx) => {
                let mut pruned = bytes.to_vec();
                let removed = pruned.remove(idx);
                (Value::bytes(pruned), Value::Long(i64::from(removed)))
            }
            None => (collection.clone(), Value::Null),
        },
        Value::String(s) => {
            let mut units: Vec<u16> = s.encode_utf16().collect();
            match remove_index(&key, units.len())? {
                Some(idx) => {
                    let removed = units.remove(idx);
                    (
                        Value::from(String::from_utf16_lossy(&units)),
                        Value::from(utf16_unit_string(removed)),
                    )
                }
                None => (collection.clone(), Value::Null),
            }
        }
        _ => {
            return Err(fail("REMOVE", "operates on a map, a list or a GTS."));
        }
    };
    stack.push(result);
    stack.push(removed);
    Ok(())
}

/// Indices of the occurrences of `needle` in `haystack` (Java `charAt` / `byte` loops of
/// `INDEXOF`), at most `limit` of them.
fn find_all<T: PartialEq>(haystack: &[T], needle: &[T], limit: i64) -> Vec<Value> {
    let mut indexes = Vec::new();
    let mut i = 0usize;
    while i + needle.len() <= haystack.len() {
        if haystack.get(i..i + needle.len()) == Some(needle) {
            indexes.push(Value::Long(i as i64));
            if indexes.len() as i64 >= limit {
                break;
            }
        }
        i += 1;
    }
    indexes
}

/// Indices (or keys) of the elements equal to `needle`, at most `limit` of them.
fn find_equal<'a>(
    items: impl Iterator<Item = (Value, &'a Value)>,
    needle: &Value,
    limit: i64,
) -> Vec<Value> {
    let mut indexes = Vec::new();
    for (index, element) in items {
        if element == needle {
            indexes.push(index);
            if indexes.len() as i64 >= limit {
                break;
            }
        }
    }
    indexes
}

/// `INDEXOF`: the indices (keys for a map) at which an element is found in a list, map, string
/// or byte array; the `LONG` on top of the stack limits their number (`<= 0` for all).
fn indexof(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "INDEXOF";
    let limit = pop_long(
        stack,
        NAME,
        "expects a LONG number of indexes to return. 0 to return all indexes.",
    )?;
    let limit = if limit <= 0 { i64::MAX } else { limit };
    let needle = stack.pop()?;
    let collection = stack.pop()?;
    let indexes = match &collection {
        Value::List(list) => {
            let items = list.borrow();
            find_equal(
                items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (Value::Long(i as i64), v)),
                &needle,
                limit,
            )
        }
        Value::Map(map) => {
            let entries = map.borrow();
            find_equal(entries.iter().map(|(k, v)| (k.clone(), v)), &needle, limit)
        }
        Value::String(s) => {
            let Value::String(wanted) = &needle else {
                return Err(fail(NAME, "must be given a STRING to look for in a STRING"));
            };
            let haystack: Vec<u16> = s.encode_utf16().collect();
            let wanted: Vec<u16> = wanted.encode_utf16().collect();
            find_all(&haystack, &wanted, limit)
        }
        Value::Bytes(bytes) => {
            let wanted: Vec<u8> = match &needle {
                Value::Long(l) => {
                    if *l != *l & 0xFF {
                        return Err(fail(
                            NAME,
                            "expects the LONG to look for to have only its least significant byte set when looking into BYTES.",
                        ));
                    }
                    vec![*l as u8]
                }
                Value::Bytes(wanted) => wanted.to_vec(),
                _ => {
                    return Err(fail(NAME, "must be given a BYTES to look for in a BYTES."));
                }
            };
            find_all(bytes, &wanted, limit)
        }
        _ => {
            return Err(fail(NAME, "expects a LIST, MAP, STRING, GTS or BYTES."));
        }
    };
    stack.push(Value::list(indexes));
    Ok(())
}

// -----------------------------------------------------------------------------------------
// UNIQUE, CONTAINS, CONTAINSKEY, CONTAINSVALUE
// -----------------------------------------------------------------------------------------

/// `UNIQUE`: a new list of the distinct elements of a list (first occurrence order).
fn unique(stack: &mut Stack) -> Result<()> {
    let list = pop_list(stack, "UNIQUE", "operates on a list.")?;
    let distinct: IndexSet<Value> = list.borrow().iter().cloned().collect();
    stack.push(Value::list(distinct.into_iter().collect()));
    Ok(())
}

/// `CONTAINS`: whether a list or set (left on the stack) contains an element, or whether a
/// string (consumed) contains a sub-string.
fn contains(stack: &mut Stack) -> Result<()> {
    let element = stack.pop()?;
    let collection = stack.pop()?;
    // `(collection stays on the stack, found)`.
    let outcome = match (&collection, &element) {
        (Value::List(list), _) => Some((true, list.borrow().contains(&element))),
        (Value::Set(set), _) => Some((true, set.borrow().contains(&element))),
        (Value::String(s), Value::String(needle)) => Some((false, s.contains(needle.as_ref()))),
        _ => None,
    };
    let Some((keep, found)) = outcome else {
        stack.push(collection);
        return Err(fail("CONTAINS", "operates on a list, set or STRING."));
    };
    if keep {
        stack.push(collection);
    }
    stack.push(found);
    Ok(())
}

/// `CONTAINSKEY`: whether a map (left on the stack) contains a key.
fn containskey(stack: &mut Stack) -> Result<()> {
    let key = stack.pop()?;
    let Value::Map(map) = stack.peek()? else {
        return Err(fail("CONTAINSKEY", OPERATES_ON_MAP));
    };
    let found = map.borrow().contains_key(&key);
    stack.push(found);
    Ok(())
}

/// `CONTAINSVALUE`: whether a map (left on the stack) contains a value.
fn containsvalue(stack: &mut Stack) -> Result<()> {
    let value = stack.pop()?;
    let Value::Map(map) = stack.peek()? else {
        return Err(fail("CONTAINSVALUE", OPERATES_ON_MAP));
    };
    let found = map.borrow().values().any(|v| *v == value);
    stack.push(found);
    Ok(())
}

// -----------------------------------------------------------------------------------------
// REVERSE, CLONEREVERSE, APPEND, FLATTEN, ZIP, CLONE
// -----------------------------------------------------------------------------------------

/// `REVERSE` / `CLONEREVERSE`: reverse a list (in place when `stable`, as a new list
/// otherwise), a string or a byte array.
fn reverse_impl(stack: &mut Stack, name: &str, stable: bool) -> Result<()> {
    let reversed = match stack.pop()? {
        Value::List(list) if stable => {
            list.borrow_mut().reverse();
            Value::List(list)
        }
        Value::List(list) => {
            let items: Vec<Value> = list.borrow().iter().rev().cloned().collect();
            Value::list(items)
        }
        Value::String(s) => Value::from(s.chars().rev().collect::<String>()),
        Value::Bytes(bytes) => {
            let data: Vec<u8> = bytes.iter().rev().copied().collect();
            Value::bytes(data)
        }
        _ => {
            return Err(fail(name, "operates on a list, byte array or String."));
        }
    };
    stack.push(reversed);
    Ok(())
}

/// `REVERSE`: reverse a list in place, or a string or byte array.
fn reverse(stack: &mut Stack) -> Result<()> {
    reverse_impl(stack, "REVERSE", true)
}

/// `CLONEREVERSE`: like `REVERSE` but a list is copied first.
fn clonereverse(stack: &mut Stack) -> Result<()> {
    reverse_impl(stack, "CLONEREVERSE", false)
}

/// `APPEND`: add the elements of the collection on top to the one below (in place), or the
/// entries of the map on top to the one below.
fn append(stack: &mut Stack) -> Result<()> {
    const MESSAGE: &str =
        "can only operate on 2 collections (LIST, SET, VECTOR), 2 MAPs or 2 GTSs.";
    let top = stack.pop()?;
    let below = stack.pop()?;
    match (&below, collection_items(&top)) {
        (Value::List(list), Some(items)) => list.borrow_mut().extend(items),
        (Value::Set(set), Some(items)) => set.borrow_mut().extend(items),
        (Value::Map(map), None) => {
            let Value::Map(added) = &top else {
                return Err(fail("APPEND", MESSAGE));
            };
            let entries: Vec<(Value, Value)> = added
                .borrow()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            map.borrow_mut().extend(entries);
        }
        _ => return Err(fail("APPEND", MESSAGE)),
    }
    stack.push(below);
    Ok(())
}

/// The recursive step of `FLATTEN`; `nesting` counts the levels entered so far.
fn flatten_value(value: &Value, nesting: &mut u32) -> Result<Vec<Value>> {
    *nesting += 1;
    if *nesting > 16 {
        return Err(fail(
            "FLATTEN",
            "cannot flatten lists nested more than 16 levels",
        ));
    }
    let result = match value {
        Value::List(list) => {
            let items = list.borrow().clone();
            let mut flat = Vec::new();
            for item in &items {
                flat.extend(flatten_value(item, nesting)?);
            }
            flat
        }
        other => vec![other.clone()],
    };
    *nesting -= 1;
    Ok(result)
}

/// `FLATTEN`: a new list with the elements of the nested lists of a list; anything else is
/// pushed back as is.
fn flatten(stack: &mut Stack) -> Result<()> {
    let top = stack.pop()?;
    if let Value::List(_) = &top {
        let mut nesting = 0;
        let flat = flatten_value(&top, &mut nesting)?;
        stack.push(Value::list(flat));
    } else {
        stack.push(top);
    }
    Ok(())
}

/// `ZIP`: transpose a list of lists; singletons are repeated.
fn zip(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "ZIP";
    const LISTS: &str = "operates on lists of lists.";
    let meta = pop_list(stack, NAME, LISTS)?;
    let meta = meta.borrow().clone();
    let mut lists: Vec<Vec<Value>> = Vec::with_capacity(meta.len());
    let mut len = 1usize;
    for element in &meta {
        let Value::List(list) = element else {
            return Err(fail(NAME, LISTS));
        };
        let items = list.borrow().clone();
        if items.is_empty() {
            return Err(fail(NAME, "cannot operate on empty lists."));
        }
        if items.len() != 1 {
            if len == 1 {
                len = items.len();
            } else if items.len() != len {
                return Err(fail(
                    NAME,
                    "operates on lists of lists. All lists which are not singletons must be of the same size.",
                ));
            }
        }
        lists.push(items);
    }
    let results: Vec<Value> = (0..len)
        .map(|i| {
            let tuple: Vec<Value> = lists
                .iter()
                .map(|items| {
                    let index = if items.len() == 1 { 0 } else { i };
                    items.get(index).cloned().unwrap_or(Value::Null)
                })
                .collect();
            Value::list(tuple)
        })
        .collect();
    stack.push(Value::list(results));
    Ok(())
}

/// `CLONE`: a shallow copy of a list, map or set, pushed on top of the original.
fn clone_fn(stack: &mut Stack) -> Result<()> {
    let original = stack.pop()?;
    let clone = match &original {
        Value::List(list) => Value::list(list.borrow().clone()),
        Value::Map(map) => Value::map(map.borrow().clone()),
        Value::Set(set) => Value::set(set.borrow().clone()),
        Value::Gts(gts) => Value::Gts(gts.borrow().clone().shared()),
        Value::Encoder(encoder) => Value::Encoder(Rc::new(RefCell::new(encoder.borrow().clone()))),
        _ => {
            return Err(fail(
                "CLONE",
                "operates on List, Map, Set, Vector, Geo Time Series or GTS Encoder.",
            ));
        }
    };
    stack.push(original);
    stack.push(clone);
    Ok(())
}

// -----------------------------------------------------------------------------------------
// PIVOT / PIVOTSTRICT
// -----------------------------------------------------------------------------------------

/// The series `PIVOT` builds, keyed by class name and sorted labels, with the Java hash of
/// their key.
type PivotTargets = IndexMap<(Option<String>, Vec<(String, String)>), (i32, GeoTimeSerie)>;

/// A series of `PIVOT`: encoders are decoded into a new series.
fn pivot_series(name: &str, value: &Value, wrong_type: &str) -> Result<Gts> {
    match value {
        Value::Gts(gts) => Ok(Rc::clone(gts)),
        Value::Encoder(encoder) => {
            let gts = encoder.borrow().decoder().decode().map_err(|err| {
                fail_with_cause(name, "encountered an error while decoding encoder.", err)
            })?;
            Ok(gts.shared())
        }
        _ => Err(fail(name, wrong_type)),
    }
}

/// `GTSHelper.tickAtIndex`: `Long.MIN_VALUE` past the last datapoint.
fn pivot_tick(gts: &GeoTimeSerie, idx: usize) -> i64 {
    if idx < gts.size() {
        gts.tick_at(idx)
    } else {
        i64::MIN
    }
}

/// The pivoted series of `gts` with the extra `labels` (whose `null` class names are
/// overwritten by the class name key Warp 10 adds), created on first use. The map is keyed
/// like the Java `HashMap` (class name and labels) and records the Java hash of its key.
fn pivot_target<'a>(
    labeled: &'a mut PivotTargets,
    gts: &GeoTimeSerie,
    labels: &[(Option<String>, String)],
) -> &'a mut GeoTimeSerie {
    let mut gts_labels = gts.metadata.labels.clone();
    for (key, value) in labels {
        if let Some(key) = key {
            gts_labels.insert(key.clone(), value.clone());
        }
    }
    let mut sorted: Vec<(String, String)> = gts_labels
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    sorted.sort();
    let name = gts.metadata.name.clone();
    &mut labeled
        .entry((name.clone(), sorted))
        .or_insert_with(|| {
            let hash = gts_labels
                .iter()
                .fold(name.as_deref().map_or(0, java_string_hash), |h, (k, v)| {
                    h.wrapping_add(java_string_hash(k) ^ java_string_hash(v))
                });
            // The labels of the Java `HashMap` copied from those of the series, which the
            // class name is put into (under the `null` key) before being removed.
            let copied = gts.metadata.labels.len();
            let capacity = if copied == 0 {
                16
            } else {
                table_size_for((copied as f32 / 0.75 + 1.0) as usize)
            };
            let mut keys: Vec<Option<(&String, &String)>> = gts_labels.iter().map(Some).collect();
            keys.push(None);
            let key_hashes: Vec<i32> = keys
                .iter()
                .map(|key| key.map_or(0, |(k, _)| java_string_hash(k)))
                .collect();
            // `setLabels` copies that map (null key included) into a `new HashMap(map)`.
            let order = java_hash_order(&key_hashes, capacity);
            let copy_hashes: Vec<i32> = order.iter().map(|&i| key_hashes[i]).collect();
            let copy_capacity = table_size_for((keys.len() as f32 / 0.75 + 1.0) as usize);
            let labels = java_hash_order(&copy_hashes, copy_capacity)
                .into_iter()
                .filter_map(|j| keys[order[j]])
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let mut target = GeoTimeSerie::new();
            target.metadata.name = name;
            target.metadata.labels = labels;
            target
                .metadata
                .attributes
                .clone_from(&gts.metadata.attributes);
            (hash, target)
        })
        .1
}

/// `PIVOT` / `PIVOTSTRICT`: label the datapoints of series with the values the labeling
/// series have at the same ticks (all of them for `PIVOTSTRICT`, which only keeps the ticks
/// where every labeling series has a datapoint).
#[allow(clippy::too_many_lines)]
fn pivot_impl(stack: &mut Stack, name: &str, synchronous: bool) -> Result<()> {
    let top = stack.pop()?;
    let labeling_values = match &top {
        Value::List(list) if !list.borrow().is_empty() => list.borrow().clone(),
        _ => {
            return Err(fail(
                name,
                "expects a non empty list of labeling Geo Time Series.",
            ));
        }
    };
    let mut labeling = Vec::with_capacity(labeling_values.len());
    let mut classes: HashSet<Option<String>> = HashSet::new();
    for value in &labeling_values {
        let gts = pivot_series(name, value, "expects a list of labeling Geo Time Series.")?;
        if gts.borrow().size() == 0 {
            return Err(fail(
                name,
                "expects labeling Geo Time Series to be non empty.",
            ));
        }
        if !classes.insert(gts.borrow().metadata.name.clone()) {
            return Err(fail(
                name,
                "labeling Geo Time Series must all have different class names.",
            ));
        }
        labeling.push(gts);
    }
    let top = stack.pop()?;
    let series_values = match &top {
        Value::List(list) if !list.borrow().is_empty() => list.borrow().clone(),
        _ => {
            return Err(fail(
                name,
                "operates on a non empty list of Geo Time Series or GTS Encoders.",
            ));
        }
    };
    let mut series = Vec::with_capacity(series_values.len());
    for value in &series_values {
        let gts = pivot_series(
            name,
            value,
            "operates on a list Geo Time Series or GTS Encoders.",
        )?;
        if gts.borrow().size() == 0 {
            return Err(fail(
                name,
                "operates on non empty Geo Time Series or GTS Encoders.",
            ));
        }
        if let Some(key) = gts
            .borrow()
            .metadata
            .labels
            .keys()
            .find(|key| classes.contains(&Some((*key).clone())))
        {
            return Err(fail(
                name,
                &format!(
                    "labeling class '{key}' is already a label of a Geo Time Series or GTS Encoder to label."
                ),
            ));
        }
        series.push(gts);
    }
    for gts in series.iter().chain(&labeling) {
        gts.borrow_mut().sort(false);
    }
    let lab: Vec<_> = labeling.iter().map(|g| g.borrow()).collect();
    let ser: Vec<_> = series.iter().map(|g| g.borrow()).collect();
    let mut li = vec![0usize; lab.len()];
    let mut gi = vec![0usize; ser.len()];
    let mut labeled = IndexMap::new();
    let mut commonts = if synchronous {
        pivot_tick(&lab[0], 0)
    } else {
        lab.iter()
            .map(|g| pivot_tick(g, 0))
            .fold(i64::MAX, i64::min)
    };
    let mut done = false;
    while !done {
        if synchronous {
            let mut skip = false;
            for (i, g) in lab.iter().enumerate() {
                let n = g.size();
                while li[i] < n && pivot_tick(g, li[i]) < commonts {
                    li[i] += 1;
                }
                if li[i] >= n {
                    done = true;
                    skip = true;
                    break;
                }
                if pivot_tick(g, li[i]) > commonts {
                    commonts = pivot_tick(g, li[i]);
                    skip = true;
                    break;
                }
            }
            if skip {
                continue;
            }
        }
        // `String.valueOf(valueAtIndex)`, which is `null` past the last datapoint.
        let labels: Vec<(Option<String>, String)> = lab
            .iter()
            .zip(&li)
            .filter(|(g, idx)| commonts == pivot_tick(g, **idx))
            .map(|(g, &idx)| {
                let value = if idx < g.size() {
                    g.value_at(idx)
                } else {
                    Value::Null
                };
                (g.metadata.name.clone(), value.to_string())
            })
            .collect();
        for (i, g) in ser.iter().enumerate() {
            let n = g.size();
            while gi[i] < n && g.tick_at(gi[i]) < commonts {
                if !synchronous {
                    // Before the common tick, only the labels of the series apply.
                    pivot_target(&mut labeled, g, &[]).set_value(
                        g.tick_at(gi[i]),
                        g.location_at(gi[i]),
                        g.elevation_at(gi[i]),
                        &g.value_at(gi[i]),
                        false,
                    );
                }
                gi[i] += 1;
            }
            if gi[i] >= n || g.tick_at(gi[i]) > commonts {
                continue;
            }
            let target = pivot_target(&mut labeled, g, &labels);
            while gi[i] < n && commonts == g.tick_at(gi[i]) {
                target.set_value(
                    commonts,
                    g.location_at(gi[i]),
                    g.elevation_at(gi[i]),
                    &g.value_at(gi[i]),
                    false,
                );
                gi[i] += 1;
            }
        }
        if synchronous {
            li[0] += 1;
            if li[0] >= lab[0].size() {
                done = true;
            } else {
                commonts = pivot_tick(&lab[0], li[0]);
            }
        } else {
            let mut newcommonts = i64::MAX;
            done = true;
            for (i, g) in lab.iter().enumerate() {
                if li[i] < g.size() {
                    if commonts == pivot_tick(g, li[i]) {
                        li[i] += 1;
                    }
                    newcommonts = newcommonts.min(pivot_tick(g, li[i]));
                    done = false;
                }
            }
            // Done once the labeling series are consumed, unless series still have ticks.
            if done {
                done = gi.iter().zip(&ser).all(|(&idx, g)| idx >= g.size());
            }
            commonts = newcommonts;
        }
    }
    let hashes: Vec<i32> = labeled.values().map(|(hash, _)| *hash).collect();
    let mut results: Vec<Option<GeoTimeSerie>> =
        labeled.into_values().map(|(_, gts)| Some(gts)).collect();
    stack.push(Value::list(
        java_hash_order(&hashes, 16)
            .into_iter()
            .filter_map(|i| results[i].take())
            .map(|gts| Value::Gts(gts.shared()))
            .collect(),
    ));
    Ok(())
}

/// `PIVOT`: label every datapoint with the values the labeling series have at its tick.
fn pivot(stack: &mut Stack) -> Result<()> {
    pivot_impl(stack, "PIVOT", false)
}

/// `PIVOTSTRICT`: `PIVOT` restricted to the ticks where every labeling series has a value.
fn pivotstrict(stack: &mut Stack) -> Result<()> {
    pivot_impl(stack, "PIVOTSTRICT", true)
}

// -----------------------------------------------------------------------------------------
// LSTORE / LCSTORE / MSTORE / MCSTORE
// -----------------------------------------------------------------------------------------

/// Store `value` into the variable or register `symbol` (`NULL` symbols are skipped); when
/// `conditional`, only if the variable is not already set (register is `NULL`).
fn store_symbol(stack: &mut Stack, symbol: &Value, value: Value, conditional: bool) -> Result<()> {
    match symbol {
        Value::Long(l) => {
            let register = i64::from(*l as i32);
            if !conditional || stack.load_register(register)?.is_null() {
                stack.store_register(register, value)?;
            }
        }
        Value::String(name) if !conditional || !stack.has_symbol(name) => {
            stack.store(name, value)?;
        }
        _ => {}
    }
    Ok(())
}

/// `true` when `symbol` can designate a variable: `NULL` (skipped), a name or a register.
fn is_symbol(symbol: &Value) -> bool {
    matches!(symbol, Value::Null | Value::String(_) | Value::Long(_))
}

/// `LSTORE` / `LCSTORE`: store the elements of a list into the variables named by a second
/// list (`NULL` names are skipped, extra values are ignored).
fn lstore_impl(stack: &mut Stack, name: &str, conditional: bool) -> Result<()> {
    let Value::List(symbols) = stack.pop()? else {
        return Err(fail(name, VARIABLE_NAMES));
    };
    let symbols = symbols.borrow().clone();
    if !symbols.iter().all(is_symbol) {
        return Err(fail(name, VARIABLE_NAMES));
    }
    let values = pop_list(stack, name, "expects a list as first argument.")?;
    let values = values.borrow().clone();
    for (symbol, value) in symbols.iter().zip(values) {
        store_symbol(stack, symbol, value, conditional)?;
    }
    Ok(())
}

/// `LSTORE`: store the elements of a list into variables.
fn lstore(stack: &mut Stack) -> Result<()> {
    lstore_impl(stack, "LSTORE", false)
}

/// `LCSTORE`: like `LSTORE`, but only for the variables which are not already set.
fn lcstore(stack: &mut Stack) -> Result<()> {
    lstore_impl(stack, "LCSTORE", true)
}

/// `MSTORE` / `MCSTORE`: store the values of a map into the variables named by its keys, or by
/// an optional list of keys on top of the stack.
fn mstore_impl(stack: &mut Stack, name: &str, conditional: bool) -> Result<()> {
    let mut top = stack.pop()?;
    let mut key_list: Option<Vec<Value>> = None;
    if let Value::List(keys) = &top {
        key_list = Some(keys.borrow().clone());
        top = stack.pop()?;
    }
    let Value::Map(map) = top else {
        return Err(fail(name, "expects a Map as argument."));
    };
    if let Some(keys) = &key_list
        && !keys.iter().all(is_symbol)
    {
        return Err(fail(name, VARIABLE_NAMES));
    }
    let symbols: Vec<Value> = key_list.unwrap_or_else(|| map.borrow().keys().cloned().collect());
    for symbol in symbols {
        if symbol.is_null() {
            continue;
        }
        let Some(value) = map.borrow().get(&symbol).cloned() else {
            continue;
        };
        if !is_symbol(&symbol) {
            return Err(fail(name, VARIABLE_NAMES));
        }
        store_symbol(stack, &symbol, value, conditional)?;
    }
    Ok(())
}

/// `MSTORE`: store the values of a map into variables.
fn mstore(stack: &mut Stack) -> Result<()> {
    mstore_impl(stack, "MSTORE", false)
}

/// `MCSTORE`: like `MSTORE`, but only for the variables which are not already set.
fn mcstore(stack: &mut Stack) -> Result<()> {
    mstore_impl(stack, "MCSTORE", true)
}
