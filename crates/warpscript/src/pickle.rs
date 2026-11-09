// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.
// Derived from Pyrolite 4.30 (Copyright Irmen de Jong, MIT License): net.razorvine.pickle.Pickler.

//! The Python pickle encoder of `->PICKLE` (Pyrolite's `Pickler`, protocol 2).
//!
//! Pyrolite memoizes by **value**: an object equal (Java `equals`) to one already written is
//! replaced by a memo reference, the memo being keyed by the Java `hashCode` (an object
//! whose hash code is already taken by a different object is not memoized).

use std::collections::HashMap;
use std::rc::Rc;

use indexmap::IndexMap;
use num_bigint::{BigInt, Sign};
use num_traits::Zero;

use crate::bigdecimal::BigDecimal;
use crate::error::{Error, Result, compat};
use crate::geoxp::from_geoxp_point;
use crate::gts::{GeoTimeSerie, NO_LOCATION};
use crate::javamap::JavaHashMap;
use crate::value::Value;

/// `Pickler.MAX_RECURSE_DEPTH`.
const MAX_RECURSE_DEPTH: usize = 1000;

/// The opcodes of the pickle protocol written here.
mod op {
    pub const MARK: u8 = b'(';
    pub const STOP: u8 = b'.';
    pub const INT: u8 = b'I';
    pub const BININT: u8 = b'J';
    pub const BININT1: u8 = b'K';
    pub const BININT2: u8 = b'M';
    pub const NONE: u8 = b'N';
    pub const REDUCE: u8 = b'R';
    pub const SHORT_BINSTRING: u8 = b'U';
    pub const BINUNICODE: u8 = b'X';
    pub const APPENDS: u8 = b'e';
    pub const GLOBAL: u8 = b'c';
    pub const EMPTY_DICT: u8 = b'}';
    pub const BINGET: u8 = b'h';
    pub const LONG_BINGET: u8 = b'j';
    pub const EMPTY_LIST: u8 = b']';
    pub const BINPUT: u8 = b'q';
    pub const LONG_BINPUT: u8 = b'r';
    pub const SETITEMS: u8 = b'u';
    pub const BINFLOAT: u8 = b'G';
    pub const PROTO: u8 = 0x80;
    pub const TUPLE1: u8 = 0x85;
    pub const TUPLE2: u8 = 0x86;
    pub const NEWTRUE: u8 = 0x88;
    pub const NEWFALSE: u8 = 0x89;
}

/// A `net.razorvine.pickle.PickleException`.
pub fn pickle_error(message: impl Into<String>) -> Error {
    Error::runtime("net.razorvine.pickle.PickleException", message)
}

/// The `StackOverflowError` of hashing a self-containing collection, or of nesting too deep.
fn stack_overflow(message: &str) -> Error {
    Error::runtime("java.lang.StackOverflowError", message)
}

/// A stand-in for the Java identity hash code of an object without value semantics.
fn identity<T: ?Sized>(rc: &Rc<T>) -> i32 {
    let address = Rc::as_ptr(rc).cast::<()>() as usize;
    (address ^ (address >> 32)) as i32
}

/// `String.hashCode`.
pub fn string_hash(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

/// `Long.hashCode`.
pub fn long_hash(l: i64) -> i32 {
    (l ^ ((l as u64) >> 32) as i64) as i32
}

/// `Double.hashCode`.
pub fn double_hash(d: f64) -> i32 {
    let bits = if d.is_nan() {
        f64::NAN.to_bits()
    } else {
        d.to_bits()
    };
    long_hash(bits as i64)
}

/// `BigInteger.hashCode`.
pub fn bigint_hash(value: &BigInt) -> i32 {
    let (sign, digits) = value.to_u32_digits();
    let hash = digits
        .iter()
        .rev()
        .fold(0i32, |h, d| h.wrapping_mul(31).wrapping_add(*d as i32));
    match sign {
        Sign::Minus => hash.wrapping_neg(),
        _ => hash,
    }
}

/// `BigDecimal.hashCode`.
pub fn bigdecimal_hash(value: &BigDecimal) -> i32 {
    let unscaled = &value.unscaled;
    let scale = value.scale;
    match i64::try_from(unscaled) {
        Ok(compact) if compact != i64::MIN => {
            let magnitude = compact.unsigned_abs();
            let temp = (((magnitude >> 32) as i32).wrapping_mul(31) as i64)
                .wrapping_add((magnitude & 0xffff_ffff) as i64) as i32;
            let signed = if compact < 0 {
                temp.wrapping_neg()
            } else {
                temp
            };
            signed.wrapping_mul(31).wrapping_add(scale)
        }
        _ => bigint_hash(unscaled).wrapping_mul(31).wrapping_add(scale),
    }
}

/// The Java `hashCode` of a value; `visiting` holds the collections being hashed, to detect
/// the self-containing ones on which Java overflows its stack.
fn hash_code(value: &Value, visiting: &mut Vec<usize>) -> Result<i32> {
    fn enter<T: ?Sized>(rc: &Rc<T>, visiting: &mut Vec<usize>) -> Result<()> {
        let address = Rc::as_ptr(rc).cast::<()>() as usize;
        if visiting.contains(&address) {
            return Err(stack_overflow("StackOverflowError"));
        }
        visiting.push(address);
        Ok(())
    }
    Ok(match value {
        Value::Null | Value::Mark => 0,
        Value::Boolean(true) => 1231,
        Value::Boolean(false) => 1237,
        Value::Long(l) => long_hash(*l),
        Value::Double(d) => double_hash(*d),
        Value::BigDecimal(bd) => bigdecimal_hash(bd),
        Value::String(s) => string_hash(s),
        Value::List(list) => {
            enter(list, visiting)?;
            let mut hash = 1i32;
            for item in list.borrow().iter() {
                hash = hash
                    .wrapping_mul(31)
                    .wrapping_add(hash_code(item, visiting)?);
            }
            visiting.pop();
            hash
        }
        Value::Map(map) => {
            enter(map, visiting)?;
            let mut hash = 0i32;
            for (k, v) in map.borrow().iter() {
                hash = hash.wrapping_add(hash_code(k, visiting)? ^ hash_code(v, visiting)?);
            }
            visiting.pop();
            hash
        }
        Value::Set(set) => {
            enter(set, visiting)?;
            let mut hash = 0i32;
            for item in set.borrow().iter() {
                hash = hash.wrapping_add(hash_code(item, visiting)?);
            }
            visiting.pop();
            hash
        }
        Value::Matrix(m) => identity(m),
        Value::Vector(v) => identity(v),
        Value::Bytes(b) => identity(b),
        Value::BitSet(b) => identity(b),
        Value::Macro(m) => identity(m),
        Value::Function(f) => identity(f),
        Value::Context(c) => identity(c),
        Value::Counter(c) => identity(c),
        Value::Gts(g) => identity(g),
        Value::Encoder(e) => identity(e),
    })
}

/// Java `equals` as the memo compares objects: byte arrays and vectors by identity.
fn java_equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Bytes(x), Value::Bytes(y)) => Rc::ptr_eq(x, y),
        (Value::List(x), Value::List(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| java_equals(a, b))
        }
        (Value::Map(x), Value::Map(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| java_equals(v, w)))
        }
        _ => a == b,
    }
}

/// `BigDecimal.toEngineeringString`.
pub fn engineering_string(value: &BigDecimal) -> String {
    let scale = i64::from(value.scale);
    let unscaled = &value.unscaled;
    if scale == 0 {
        return unscaled.to_string();
    }
    let digits = unscaled.magnitude().to_string();
    let coeff_len = digits.len() as i64;
    let mut adjusted = coeff_len - 1 - scale;
    if scale > 0 && adjusted >= -6 {
        // Plain notation, as `toString`.
        return value.to_string();
    }
    let mut out = String::new();
    if unscaled.sign() == Sign::Minus {
        out.push('-');
    }
    let mut sig = adjusted % 3;
    if sig < 0 {
        sig += 3;
    }
    adjusted -= sig;
    sig += 1;
    if unscaled.is_zero() {
        match sig {
            1 => out.push('0'),
            2 => {
                out.push_str("0.00");
                adjusted += 3;
            }
            _ => {
                out.push_str("0.0");
                adjusted += 3;
            }
        }
    } else if sig >= coeff_len {
        out.push_str(&digits);
        out.push_str(&"0".repeat((sig - coeff_len) as usize));
    } else {
        out.push_str(&digits[..sig as usize]);
        out.push('.');
        out.push_str(&digits[sig as usize..]);
    }
    if adjusted != 0 {
        out.push('E');
        if adjusted > 0 {
            out.push('+');
        }
        out.push_str(&adjusted.to_string());
    }
    out
}

/// The pickler state of one `dumps`.
struct Pickler {
    out: Vec<u8>,
    recurse: usize,
    /// Hash code → (memoized object, memo index).
    memo: HashMap<i32, (Value, usize)>,
    /// The `long[0]` arrays (one per series, distinct for the memo) series without values
    /// pickle as their values.
    empty_values: Vec<Rc<[f64]>>,
}

/// `PickleSerializer.serializeData`: the protocol 2 pickle of `value`.
pub fn dumps(value: &Value) -> Result<Vec<u8>> {
    let mut pickler = Pickler {
        out: vec![op::PROTO, 2],
        recurse: 0,
        memo: HashMap::new(),
        empty_values: Vec::new(),
    };
    pickler.save(value)?;
    pickler.out.push(op::STOP);
    Ok(pickler.out)
}

impl Pickler {
    fn save(&mut self, value: &Value) -> Result<()> {
        self.recurse += 1;
        if self.recurse > MAX_RECURSE_DEPTH {
            return Err(stack_overflow("recursion too deep in Pickler.save (>1000)"));
        }
        if value.is_null() {
            self.out.push(op::NONE);
        } else {
            let hash = Pickler::hash(value)?;
            if !self.lookup_memo(value, hash) {
                self.dispatch(value, hash)?;
            }
        }
        self.recurse -= 1;
        Ok(())
    }

    fn hash(value: &Value) -> Result<i32> {
        hash_code(value, &mut Vec::new())
    }

    /// The hash code of a part of a value whose hash code was computed without error.
    fn part_hash(value: &Value) -> i32 {
        Pickler::hash(value).unwrap_or(0)
    }

    fn write_memo(&mut self, value: &Value, hash: i32) {
        if self.memo.contains_key(&hash) {
            return;
        }
        let index = self.memo.len();
        self.memo.insert(hash, (value.clone(), index));
        if index <= 0xff {
            self.out.extend([op::BINPUT, index as u8]);
        } else {
            self.out.push(op::LONG_BINPUT);
            self.out.extend((index as i32).to_le_bytes());
        }
    }

    fn lookup_memo(&mut self, value: &Value, hash: i32) -> bool {
        match self.memo.get(&hash) {
            Some((memoized, index)) if java_equals(memoized, value) => {
                let index = *index;
                if index <= 0xff {
                    self.out.extend([op::BINGET, index as u8]);
                } else {
                    self.out.push(op::LONG_BINGET);
                    self.out.extend((index as i32).to_le_bytes());
                }
                true
            }
            _ => false,
        }
    }

    fn dispatch(&mut self, value: &Value, hash: i32) -> Result<()> {
        match value {
            Value::Boolean(b) => self.out.push(if *b { op::NEWTRUE } else { op::NEWFALSE }),
            Value::Long(l) => self.put_long(*l),
            Value::Double(d) => {
                self.out.push(op::BINFLOAT);
                self.out.extend(d.to_bits().to_be_bytes());
            }
            Value::String(s) => self.put_string(s),
            Value::BigDecimal(bd) => {
                self.global("decimal\nDecimal\n");
                self.put_string(&Rc::from(engineering_string(bd)));
                self.out.extend([op::TUPLE1, op::REDUCE]);
                self.write_memo(value, hash);
            }
            Value::Bytes(bytes) => {
                self.global("__builtin__\nbytearray\n");
                let latin1: String = bytes.iter().map(|b| char::from(*b)).collect();
                self.put_string(&Rc::from(latin1));
                self.put_string(&Rc::from("latin-1"));
                self.out.extend([op::TUPLE2, op::REDUCE]);
                self.write_memo(value, hash);
            }
            Value::Vector(v) if self.empty_values.iter().any(|e| Rc::ptr_eq(v, e)) => {
                self.global("array\narray\n");
                self.out
                    .extend([op::SHORT_BINSTRING, 1, b'l', op::EMPTY_LIST, op::MARK]);
                self.out.extend([op::APPENDS, op::TUPLE2, op::REDUCE]);
                self.write_memo(value, hash);
            }
            Value::List(list) => {
                self.out.push(op::EMPTY_LIST);
                self.write_memo(value, hash);
                self.out.push(op::MARK);
                let items = list.borrow().clone();
                for item in &items {
                    self.save(item)?;
                }
                self.out.push(op::APPENDS);
            }
            Value::Map(map) => {
                self.out.push(op::EMPTY_DICT);
                self.write_memo(value, hash);
                self.out.push(op::MARK);
                let entries: Vec<(Value, Value)> = map
                    .borrow()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                for (k, v) in &entries {
                    self.save(k)?;
                    self.save(v)?;
                }
                self.out.push(op::SETITEMS);
            }
            Value::Set(set) => {
                self.global("__builtin__\nset\n");
                self.out.extend([op::EMPTY_LIST, op::MARK]);
                let mut ordered = JavaHashMap::new();
                for item in set.borrow().iter() {
                    ordered.put(Pickler::part_hash(item), item.clone(), ());
                }
                for (_, item, ()) in ordered.into_entries() {
                    self.save(&item)?;
                }
                self.out.extend([op::APPENDS, op::TUPLE1, op::REDUCE]);
                self.write_memo(value, hash);
            }
            Value::Gts(gts) => {
                let map = self.gts_map(&gts.borrow());
                self.save(&map)?;
                self.write_memo(value, hash);
            }
            Value::Counter(_) => {
                return Err(Error::runtime(
                    "java.lang.StringIndexOutOfBoundsException",
                    "Index 1 out of bounds for length 0",
                ));
            }
            Value::BitSet(_) | Value::Matrix(_) | Value::Vector(_) => {
                let expected = if matches!(value, Value::Vector(_)) {
                    1
                } else {
                    2
                };
                return Err(pickle_error(format!(
                    "couldn't introspect javabean: java.lang.IllegalArgumentException: wrong number of arguments: 0 expected: {expected}"
                )));
            }
            _ => {
                return Err(pickle_error(format!(
                    "couldn't pickle object of type class {}",
                    compat::value_class(value.kind())
                )));
            }
        }
        Ok(())
    }

    fn global(&mut self, name: &str) {
        self.out.push(op::GLOBAL);
        self.out.extend(name.as_bytes());
    }

    fn put_string(&mut self, s: &Rc<str>) {
        self.out.push(op::BINUNICODE);
        self.out.extend((s.len() as i32).to_le_bytes());
        self.out.extend(s.as_bytes());
        self.write_memo(&Value::String(s.clone()), string_hash(s));
    }

    fn put_long(&mut self, v: i64) {
        if (0..=0xff).contains(&v) {
            self.out.extend([op::BININT1, v as u8]);
        } else if (0..=0xffff).contains(&v) {
            self.out.push(op::BININT2);
            self.out.extend((v as u16).to_le_bytes());
        } else if i32::try_from(v).is_ok() {
            self.out.push(op::BININT);
            self.out.extend((v as i32).to_le_bytes());
        } else {
            self.out.push(op::INT);
            self.out.extend(v.to_string().as_bytes());
            self.out.push(b'\n');
        }
    }

    /// `GTSPickler`: the map a series is pickled as, in Java `HashMap` order.
    fn gts_map(&mut self, gts: &GeoTimeSerie) -> Value {
        fn hash_map(entries: Vec<(Value, Value)>) -> Value {
            let mut map = JavaHashMap::new();
            for (k, v) in entries {
                map.put(Pickler::part_hash(&k), k, v);
            }
            let ordered: IndexMap<Value, Value> = map
                .into_entries()
                .into_iter()
                .map(|(_, k, v)| (k, v))
                .collect();
            Value::Map(Rc::new(std::cell::RefCell::new(ordered)))
        }
        let strings = |m: &IndexMap<String, String>| -> Value {
            hash_map(
                m.iter()
                    .map(|(k, v)| (Value::from(k.as_str()), Value::from(v.as_str())))
                    .collect(),
            )
        };
        let n = gts.size();
        let mut entries = vec![
            (
                Value::from("classname"),
                Value::from(gts.name().unwrap_or("")),
            ),
            (Value::from("labels"), strings(&gts.metadata.labels)),
            (Value::from("attributes"), strings(&gts.metadata.attributes)),
            (
                Value::from("timestamps"),
                Value::list((0..n).map(|i| Value::Long(gts.tick_at(i))).collect()),
            ),
            (
                Value::from("values"),
                if n == 0 {
                    let marker: Rc<[f64]> = Rc::from(Vec::new());
                    self.empty_values.push(marker.clone());
                    Value::Vector(marker)
                } else {
                    Value::list((0..n).map(|i| gts.value_at(i)).collect())
                },
            ),
        ];
        if let Some(locations) = &gts.locations {
            let (lats, lons): (Vec<Value>, Vec<Value>) = locations
                .iter()
                .map(|&location| {
                    let (lat, lon) = if location == NO_LOCATION {
                        (f64::NAN, f64::NAN)
                    } else {
                        from_geoxp_point(location)
                    };
                    (
                        Value::Double(f64::from(lat as f32)),
                        Value::Double(f64::from(lon as f32)),
                    )
                })
                .unzip();
            entries.push((Value::from("latitudes"), Value::list(lats)));
            entries.push((Value::from("longitudes"), Value::list(lons)));
        }
        if let Some(elevations) = &gts.elevations {
            entries.push((
                Value::from("elevations"),
                Value::list(elevations.iter().map(|e| Value::Long(*e)).collect()),
            ));
        }
        hash_map(entries)
    }
}
