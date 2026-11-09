// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.
// Derived from Pyrolite 4.30 (Copyright Irmen de Jong, MIT License): net.razorvine.pickle.Unpickler.

//! The Python pickle decoder of `PICKLE->` (Pyrolite's `Unpickler`, protocols 0 to 5).
//!
//! The pickle is first decoded to [`Py`], a model of the Java objects Pyrolite builds
//! (`Integer` and `Long` are distinct, dictionaries and sets are `HashMap`s / `HashSet`s whose
//! iteration order follows the Java hash codes), then converted to stack values.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::bigdecimal::{BigDecimal, parse_bigint};
use crate::error::{Error, Result};
use crate::functions::operators::parse_java_double;
use crate::javamap::JavaHashMap;
use crate::pickle::{
    bigdecimal_hash, bigint_hash, double_hash, long_hash, pickle_error, string_hash,
};
use crate::value::Value;

/// `Unpickler.HIGHEST_PROTOCOL`.
const HIGHEST_PROTOCOL: i16 = 5;

/// Deepest nesting converted to stack values (deeper structures are rejected like a Java
/// stack overflow rather than overflowing the native stack).
const MAX_DEPTH: usize = 2000;

/// A Java `HashMap` of decoded objects.
type Dict = Rc<RefCell<JavaHashMap<Py, Py>>>;

/// The object constructors a `GLOBAL` resolves to (`IObjectConstructor`).
#[derive(Debug, PartialEq)]
enum Ctor {
    /// `ClassDictConstructor` (any unknown class).
    ClassDict(String),
    /// `ExceptionConstructor` (`exceptions.*`, `builtins.*Error`…).
    Exception(String),
    /// `AnyClassConstructor(ComplexNumber)`.
    Complex,
    /// `AnyClassConstructor(BigDecimal)`.
    Decimal,
    /// `ArrayConstructor`.
    Array,
    /// `ByteArrayConstructor` (also `_codecs.encode`).
    ByteArray,
    /// `SetConstructor`.
    Set,
    /// `DateTimeConstructor`.
    DateTime(DateKind),
    /// `TimeZoneConstructor` (UTC, pytz, dateutil `tzutc` / `gettz`).
    Zone(ZoneKind),
    /// `Reconstructor` (`copy_reg._reconstructor`).
    Reconstructor,
    /// `OperatorAttrGetterForCalendarTz` and the objects it builds.
    AttrGetter,
    /// `AttrGetterForTz`.
    AttrGetterForTz,
    /// `CalendarLocalizer`.
    Localizer(String),
}

impl Ctor {
    /// The Java class of the constructor.
    fn java_class(&self) -> &'static str {
        match self {
            Ctor::ClassDict(_) => "net.razorvine.pickle.objects.ClassDictConstructor",
            Ctor::Exception(_) => "net.razorvine.pickle.objects.ExceptionConstructor",
            Ctor::Complex | Ctor::Decimal => "net.razorvine.pickle.objects.AnyClassConstructor",
            Ctor::Array => "net.razorvine.pickle.objects.ArrayConstructor",
            Ctor::ByteArray => "net.razorvine.pickle.objects.ByteArrayConstructor",
            Ctor::Set => "net.razorvine.pickle.objects.SetConstructor",
            Ctor::DateTime(_) => "net.razorvine.pickle.objects.DateTimeConstructor",
            Ctor::Zone(_) => "net.razorvine.pickle.objects.TimeZoneConstructor",
            Ctor::Reconstructor => "net.razorvine.pickle.objects.Reconstructor",
            Ctor::AttrGetter => "net.razorvine.pickle.objects.OperatorAttrGetterForCalendarTz",
            Ctor::AttrGetterForTz => {
                "net.razorvine.pickle.objects.OperatorAttrGetterForCalendarTz$AttrGetterForTz"
            }
            Ctor::Localizer(_) => {
                "net.razorvine.pickle.objects.OperatorAttrGetterForCalendarTz$CalendarLocalizer"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DateKind {
    DateTime,
    Date,
    Time,
    TimeDelta,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ZoneKind {
    Utc,
    Pytz,
    DateutilUtc,
    DateutilGettz,
    DateutilTzfile,
    Unsupported,
}

/// A decoded object: the Java object Pyrolite would have built.
#[derive(Clone, Debug)]
enum Py {
    None,
    Bool(bool),
    Int(i32),
    Long(i64),
    Big(Rc<BigInt>),
    Float(f64),
    Str(Rc<str>),
    /// `byte[]` (identity semantics).
    Bytes(Rc<[u8]>),
    /// `Object[]`, a tuple (identity semantics).
    Tuple(Rc<[Py]>),
    /// A primitive array (`int[]`, `double[]`…; identity semantics), as its boxed elements.
    Array(Rc<[Py]>),
    List(Rc<RefCell<Vec<Py>>>),
    /// A `HashMap`, or a `ClassDict` (a `HashMap` with a class name).
    Dict(Dict, Option<Rc<str>>),
    Set(Rc<RefCell<JavaHashMap<Py, ()>>>),
    Decimal(Rc<BigDecimal>),
    Complex(f64, f64),
    /// A `GregorianCalendar`, as its instant in milliseconds.
    Calendar(i64),
    Time([i32; 4]),
    TimeDelta([i32; 3]),
    /// A `java.util.TimeZone`, as its id.
    Zone(Rc<str>),
    /// A `Tzinfo` (dateutil time zones).
    Tzinfo(Option<Rc<str>>),
    Ctor(Rc<Ctor>),
    /// A Python exception (`PythonException`), as its message.
    Exception(Rc<str>),
    Mark,
}

impl PartialEq for Py {
    /// Java `equals`.
    fn eq(&self, other: &Py) -> bool {
        match (self, other) {
            (Py::None, Py::None) | (Py::Mark, Py::Mark) => true,
            (Py::Bool(a), Py::Bool(b)) => a == b,
            (Py::Int(a), Py::Int(b)) => a == b,
            (Py::Long(a), Py::Long(b)) | (Py::Calendar(a), Py::Calendar(b)) => a == b,
            (Py::Big(a), Py::Big(b)) => a == b,
            (Py::Float(a), Py::Float(b)) => canonical_bits(*a) == canonical_bits(*b),
            (Py::Str(a), Py::Str(b)) | (Py::Zone(a), Py::Zone(b)) => a == b,
            (Py::Bytes(a), Py::Bytes(b)) => Rc::ptr_eq(a, b),
            (Py::Tuple(a), Py::Tuple(b)) | (Py::Array(a), Py::Array(b)) => Rc::ptr_eq(a, b),
            (Py::List(a), Py::List(b)) => Rc::ptr_eq(a, b) || *a.borrow() == *b.borrow(),
            (Py::Dict(a, _), Py::Dict(b, _)) => {
                Rc::ptr_eq(a, b) || same_entries(&a.borrow(), &b.borrow())
            }
            (Py::Set(a), Py::Set(b)) => {
                Rc::ptr_eq(a, b) || {
                    let (a, b) = (a.borrow(), b.borrow());
                    a.len() == b.len()
                        && a.iter().all(|(_, k, ())| b.iter().any(|(_, j, ())| k == j))
                }
            }
            (Py::Decimal(a), Py::Decimal(b)) => a == b,
            (Py::Complex(a, b), Py::Complex(c, d)) => a == c && b == d,
            (Py::Time(a), Py::Time(b)) => a == b,
            (Py::TimeDelta(a), Py::TimeDelta(b)) => a == b,
            (Py::Tzinfo(a), Py::Tzinfo(b)) => a == b,
            (Py::Ctor(a), Py::Ctor(b)) => Rc::ptr_eq(a, b),
            (Py::Exception(a), Py::Exception(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// `Double.doubleToLongBits`.
fn canonical_bits(d: f64) -> u64 {
    if d.is_nan() {
        f64::NAN.to_bits()
    } else {
        d.to_bits()
    }
}

/// `AbstractMap.equals`.
fn same_entries(a: &JavaHashMap<Py, Py>, b: &JavaHashMap<Py, Py>) -> bool {
    a.len() == b.len()
        && a.iter()
            .all(|(_, k, v)| b.iter().any(|(_, j, w)| k == j && v == w))
}

/// A stand-in for a Java identity hash code.
fn identity<T: ?Sized>(rc: &Rc<T>) -> i32 {
    let address = Rc::as_ptr(rc).cast::<()>() as usize;
    (address ^ (address >> 32)) as i32
}

/// `timedelta.total_seconds` as Pyrolite computes it.
fn total_seconds(delta: [i32; 3]) -> f64 {
    f64::from(delta[0].wrapping_mul(86400).wrapping_add(delta[1]))
        + f64::from(delta[2]) / 1_000_000.0
}

impl Py {
    /// Java `hashCode`; `depth` guards against self-containing structures.
    fn hash(&self, depth: usize) -> Result<i32> {
        if depth > MAX_DEPTH {
            return Err(Error::runtime(
                "java.lang.StackOverflowError",
                "StackOverflowError",
            ));
        }
        Ok(match self {
            Py::None | Py::Mark | Py::Tzinfo(_) => 0,
            Py::Bool(true) => 1231,
            Py::Bool(false) => 1237,
            Py::Int(i) => *i,
            Py::Long(l) | Py::Calendar(l) => long_hash(*l),
            Py::Big(b) => bigint_hash(b),
            Py::Float(d) => double_hash(*d),
            Py::Str(s) | Py::Zone(s) => string_hash(s),
            Py::Bytes(b) => identity(b),
            Py::Tuple(t) | Py::Array(t) => identity(t),
            Py::List(list) => {
                let mut hash = 1i32;
                for item in list.borrow().iter() {
                    hash = hash.wrapping_mul(31).wrapping_add(item.hash(depth + 1)?);
                }
                hash
            }
            Py::Dict(dict, _) => {
                let mut hash = 0i32;
                for (h, _, v) in dict.borrow().iter() {
                    hash = hash.wrapping_add(h ^ v.hash(depth + 1)?);
                }
                hash
            }
            Py::Set(set) => set
                .borrow()
                .iter()
                .fold(0i32, |hash, (h, _, ())| hash.wrapping_add(*h)),
            Py::Decimal(d) => bigdecimal_hash(d),
            Py::Complex(r, i) => double_hash(*r) ^ double_hash(*i),
            Py::Time(t) => [t[0], t[3], t[1], t[2]]
                .iter()
                .fold(1i32, |h, x| h.wrapping_mul(31).wrapping_add(*x)),
            Py::TimeDelta(d) => [d[0], d[2], d[1], double_hash(total_seconds(*d))]
                .iter()
                .fold(1i32, |h, x| h.wrapping_mul(31).wrapping_add(*x)),
            Py::Ctor(c) => identity(c),
            Py::Exception(e) => identity(e),
        })
    }

    /// The Java class name, for the `ClassCastException` messages.
    fn java_class(&self) -> &'static str {
        match self {
            Py::Bool(_) => "java.lang.Boolean",
            Py::Int(_) => "java.lang.Integer",
            Py::Long(_) => "java.lang.Long",
            Py::Big(_) => "java.math.BigInteger",
            Py::Float(_) => "java.lang.Double",
            Py::Str(_) => "java.lang.String",
            Py::Bytes(_) => "[B",
            Py::Tuple(_) => "[Ljava.lang.Object;",
            Py::Array(_) => "[I",
            Py::List(_) => "java.util.ArrayList",
            Py::Dict(_, None) => "java.util.HashMap",
            Py::Dict(_, Some(_)) => "net.razorvine.pickle.objects.ClassDict",
            Py::Set(_) => "java.util.HashSet",
            Py::Decimal(_) => "java.math.BigDecimal",
            Py::Complex(..) => "net.razorvine.pickle.objects.ComplexNumber",
            Py::Calendar(_) => "java.util.GregorianCalendar",
            Py::Time(_) => "net.razorvine.pickle.objects.Time",
            Py::TimeDelta(_) => "net.razorvine.pickle.objects.TimeDelta",
            Py::Zone(_) => "sun.util.calendar.ZoneInfo",
            Py::Tzinfo(_) => "net.razorvine.pickle.objects.Tzinfo",
            Py::Ctor(ctor) => ctor.java_class(),
            Py::Exception(_) => "net.razorvine.pickle.PythonException",
            Py::None | Py::Mark => "java.lang.Object",
        }
    }

    fn as_int(&self) -> Result<i32> {
        match self {
            Py::Int(i) => Ok(*i),
            other => Err(cast_error(other, "java.lang.Integer")),
        }
    }

    /// `Number.intValue()` (and friends through `as`).
    fn number(&self) -> Result<f64> {
        match self {
            Py::Int(i) => Ok(f64::from(*i)),
            Py::Long(l) => Ok(*l as f64),
            Py::Big(b) => Ok(b.to_f64().unwrap_or(f64::NAN)),
            Py::Float(d) => Ok(*d),
            Py::Decimal(d) => Ok(d.double_value()),
            other => Err(cast_error(other, "java.lang.Number")),
        }
    }

    /// `Number.longValue()`.
    fn long_value(&self) -> Result<i64> {
        match self {
            Py::Int(i) => Ok(i64::from(*i)),
            Py::Long(l) => Ok(*l),
            Py::Big(b) => {
                Ok(crate::bigdecimal::BigDecimal::from_bigint((**b).clone()).long_value())
            }
            Py::Decimal(d) => Ok(d.long_value()),
            other => other.number().map(|d| d as i64),
        }
    }
}

/// The `ClassCastException` of casting `value` to `class`; Java casts `null` silently and
/// fails on its use instead.
fn cast_error(value: &Py, class: &str) -> Error {
    if *value == Py::None {
        return Error::null_pointer();
    }
    Error::runtime(
        "java.lang.ClassCastException",
        format!(
            "class {} cannot be cast to class {class}",
            value.java_class()
        ),
    )
}

/// A `(String)` cast.
fn str_of(value: Py) -> Result<Rc<str>> {
    match value {
        Py::Str(s) => Ok(s),
        other => Err(cast_error(&other, "java.lang.String")),
    }
}

/// An `IOException`, which `PICKLE->` wraps.
fn io_error(message: &str) -> Error {
    Error::new(format!("java.io.IOException: {message}"))
}

/// `ArrayList.get(-1)` on the empty unpickling stack.
fn empty_stack() -> Error {
    Error::runtime(
        "java.lang.IndexOutOfBoundsException",
        "Index -1 out of bounds for length 0",
    )
}

/// `NumberFormatException` with `For input string: "…"`.
fn number_format(input: &str) -> Error {
    Error::runtime(
        "java.lang.NumberFormatException",
        format!("For input string: \"{input}\""),
    )
}

/// `Long.parseLong`.
pub(crate) fn parse_long(text: &str) -> Result<i64> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(number_format(text));
    }
    text.parse::<i64>().map_err(|_| number_format(text))
}

/// `PickleUtils.optimizeBigint`.
fn optimize(big: BigInt) -> Py {
    match big.to_i64() {
        Some(l) => Py::Long(l),
        None => Py::Big(Rc::new(big)),
    }
}

/// The decoder state.
struct Unpickler<'a> {
    input: &'a [u8],
    pos: usize,
    stack: Vec<Py>,
    memo: HashMap<i32, Py>,
}

/// `PickleSerializer.deserializeData`: the value a pickle stands for.
pub fn loads(data: &[u8]) -> Result<Value> {
    let mut unpickler = Unpickler {
        input: data,
        pos: 0,
        stack: Vec::new(),
        memo: HashMap::new(),
    };
    let result = unpickler.load()?;
    to_value(&result, &mut HashMap::new(), 0)
}

impl Unpickler<'_> {
    fn load(&mut self) -> Result<Py> {
        loop {
            let key = self.readbyte();
            if key == -1 {
                return Err(io_error("premature end of file"));
            }
            if let Some(value) = self.dispatch(key as u8)? {
                return Ok(value);
            }
        }
    }

    /// `PickleUtils.readbyte`: `-1` at the end of the input.
    fn readbyte(&mut self) -> i16 {
        match self.input.get(self.pos) {
            Some(b) => {
                self.pos += 1;
                i16::from(*b)
            }
            None => -1,
        }
    }

    fn readbytes(&mut self, n: i64) -> Result<&[u8]> {
        if n < 0 {
            return Err(Error::runtime(
                "java.lang.NegativeArraySizeException",
                n.to_string(),
            ));
        }
        if n > i64::from(i32::MAX) {
            return Err(pickle_error(
                "pickle too large, can't read more than maxint",
            ));
        }
        let start = self.pos;
        let end = start.saturating_add(n as usize);
        if end > self.input.len() {
            return Err(io_error("expected more bytes in input stream"));
        }
        self.pos = end;
        Ok(&self.input[start..end])
    }

    fn read_i32(&mut self) -> Result<i32> {
        let bytes = self.readbytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_i64(&mut self) -> Result<i64> {
        let bytes = self.readbytes(8)?;
        let mut le = [0u8; 8];
        le.copy_from_slice(bytes);
        Ok(i64::from_le_bytes(le))
    }

    /// `PickleUtils.readline`: the bytes as ISO-8859-1 characters, up to `\n`.
    fn readline(&mut self, include_lf: bool) -> Result<String> {
        let mut line = String::new();
        loop {
            let c = self.readbyte();
            if c == -1 {
                if line.is_empty() {
                    return Err(io_error("premature end of file"));
                }
                break;
            }
            if c != i16::from(b'\n') || include_lf {
                line.push(char::from(c as u8));
            }
            if c == i16::from(b'\n') {
                break;
            }
        }
        Ok(line)
    }

    fn pop(&mut self) -> Result<Py> {
        self.stack.pop().ok_or_else(empty_stack)
    }

    fn peek(&self) -> Result<&Py> {
        self.stack.last().ok_or_else(empty_stack)
    }

    fn pop_all_since_marker(&mut self) -> Result<Vec<Py>> {
        let mut result = Vec::new();
        loop {
            match self.pop()? {
                Py::Mark => break,
                other => result.push(other),
            }
        }
        result.reverse();
        Ok(result)
    }

    fn push_str(&mut self, bytes: &[u8]) {
        let s = String::from_utf8_lossy(bytes).into_owned();
        self.stack.push(Py::Str(Rc::from(s)));
    }

    fn memo_get(&mut self, index: i32) -> Result<()> {
        let value = self
            .memo
            .get(&index)
            .cloned()
            .ok_or_else(|| pickle_error("invalid memo key"))?;
        self.stack.push(value);
        Ok(())
    }

    fn memo_put(&mut self, index: i32) -> Result<()> {
        let top = self.peek()?.clone();
        self.memo.insert(index, top);
        Ok(())
    }

    fn list_target(&self) -> Result<Rc<RefCell<Vec<Py>>>> {
        match self.peek()? {
            Py::List(list) => Ok(list.clone()),
            other => Err(cast_error(other, "java.util.ArrayList")),
        }
    }

    fn dict_target(&self) -> Result<Dict> {
        match self.peek()? {
            Py::Dict(dict, _) => Ok(dict.clone()),
            other => Err(cast_error(other, "java.util.Map")),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn dispatch(&mut self, key: u8) -> Result<Option<Py>> {
        match key {
            b'(' => self.stack.push(Py::Mark),
            b'.' => return self.pop().map(Some),
            b'0' => {
                self.pop()?;
            }
            b'1' => while self.pop()? != Py::Mark {},
            b'2' => {
                let top = self.peek()?.clone();
                self.stack.push(top);
            }
            b'F' => {
                let line = self.readline(true)?;
                let d = parse_java_double(&line).map_err(|_| {
                    let trimmed = line.trim_matches(|c: char| c <= ' ');
                    if trimmed.is_empty() {
                        Error::runtime("java.lang.NumberFormatException", "empty String")
                    } else {
                        number_format(trimmed)
                    }
                })?;
                self.stack.push(Py::Float(d));
            }
            b'I' => {
                let data = self.readline(true)?;
                let value = match data.as_str() {
                    "00\n" => Py::Bool(false),
                    "01\n" => Py::Bool(true),
                    _ => {
                        let mut number = data.clone();
                        number.pop();
                        match number.parse::<i32>() {
                            Ok(i) if parse_long(&number).is_ok() => Py::Int(i),
                            _ => Py::Long(parse_long(&number)?),
                        }
                    }
                };
                self.stack.push(value);
            }
            b'J' => {
                let i = self.read_i32()?;
                self.stack.push(Py::Int(i));
            }
            b'K' => {
                let b = self.readbyte();
                self.stack.push(Py::Int(i32::from(b)));
            }
            b'M' => {
                let bytes = self.readbytes(2)?;
                let i = i32::from(u16::from_le_bytes([bytes[0], bytes[1]]));
                self.stack.push(Py::Int(i));
            }
            b'L' => {
                let mut line = self.readline(false)?;
                if line.ends_with('L') {
                    line.pop();
                }
                let big = parse_bigint(&line, 10)?;
                self.stack.push(optimize(big));
            }
            0x8a | 0x8b => {
                let n = if key == 0x8a {
                    i64::from(self.readbyte())
                } else {
                    i64::from(self.read_i32()?)
                };
                let data = self.readbytes(n)?;
                let value = if data.is_empty() {
                    Py::Long(0)
                } else {
                    optimize(BigInt::from_signed_bytes_le(data))
                };
                self.stack.push(value);
            }
            b'N' => self.stack.push(Py::None),
            b'P' => {
                self.readline(false)?;
                return Err(Self::persistent());
            }
            b'Q' => {
                if self.pop()? == Py::None {
                    return Err(Error::runtime(
                        "java.lang.NullPointerException",
                        "Cannot invoke \"Object.toString()\" because the return value of \"net.razorvine.pickle.UnpickleStack.pop()\" is null",
                    ));
                }
                return Err(Self::persistent());
            }
            b'R' | 0x81 => self.reduce()?,
            b'S' => {
                let rep = self.readline(false)?;
                let inner = ['"', '\'']
                    .iter()
                    .find(|q| rep.starts_with(**q))
                    .map_or_else(
                        || Err(pickle_error("insecure string pickle")),
                        |q| {
                            if !rep.ends_with(*q) {
                                Err(pickle_error("insecure string pickle"))
                            } else if rep.len() < 2 {
                                Err(Error::runtime(
                                    "java.lang.StringIndexOutOfBoundsException",
                                    "Range [1, 0) out of bounds for length 1",
                                ))
                            } else {
                                Ok(rep[1..rep.len() - 1].to_owned())
                            }
                        },
                    )?;
                let decoded = decode_escaped(&inner, false)?;
                self.stack.push(Py::Str(Rc::from(decoded)));
            }
            b'T' | b'U' => {
                let len = if key == b'T' {
                    i64::from(self.read_i32()?)
                } else {
                    i64::from(self.readbyte())
                };
                let data: String = self
                    .readbytes(len)?
                    .iter()
                    .map(|b| char::from(*b))
                    .collect();
                self.stack.push(Py::Str(Rc::from(data)));
            }
            b'V' => {
                let line = self.readline(false)?;
                let decoded = decode_escaped(&line, true)?;
                self.stack.push(Py::Str(Rc::from(decoded)));
            }
            b'X' | 0x8c | 0x8d => {
                let len = match key {
                    b'X' => i64::from(self.read_i32()?),
                    0x8c => i64::from(self.readbyte()),
                    _ => self.read_i64()?,
                };
                let data = self.readbytes(len)?.to_vec();
                self.push_str(&data);
            }
            b'B' | b'C' | 0x8e | 0x96 => {
                let len = match key {
                    b'B' => i64::from(self.read_i32()?),
                    b'C' => i64::from(self.readbyte()),
                    _ => self.read_i64()?,
                };
                let data: Rc<[u8]> = Rc::from(self.readbytes(len)?);
                self.stack.push(Py::Bytes(data));
            }
            b'a' => {
                let value = self.pop()?;
                self.list_target()?.borrow_mut().push(value);
            }
            b'e' => {
                let items = self.pop_all_since_marker()?;
                self.list_target()?.borrow_mut().extend(items);
            }
            b'b' => self.build()?,
            b'c' => {
                let module = self.readline(false)?;
                let name = self.readline(false)?;
                self.global(&module, &name);
            }
            0x93 => {
                let name = match self.pop()? {
                    // Concatenated into the class name as "null".
                    Py::None => Rc::from("null"),
                    other => str_of(other)?,
                };
                let module = str_of(self.pop()?)?;
                self.global(&module, &name);
            }
            b'd' => {
                let items = self.pop_all_since_marker()?;
                let mut map = JavaHashMap::with_capacity(items.len());
                for pair in items.chunks(2) {
                    let [k, v] = pair else {
                        return Err(Error::runtime(
                            "java.lang.IndexOutOfBoundsException",
                            format!("Index {0} out of bounds for length {0}", items.len()),
                        ));
                    };
                    map.put(k.hash(0)?, k.clone(), v.clone());
                }
                self.stack.push(Py::Dict(Rc::new(RefCell::new(map)), None));
            }
            b'}' => {
                let map = JavaHashMap::with_capacity(0);
                self.stack.push(Py::Dict(Rc::new(RefCell::new(map)), None));
            }
            b'g' | b'p' => {
                let line = self.readline(false)?;
                let index = line
                    .parse::<i32>()
                    .ok()
                    .filter(|_| parse_long(&line).is_ok())
                    .ok_or_else(|| number_format(&line))?;
                if key == b'g' {
                    self.memo_get(index)?;
                } else {
                    self.memo_put(index)?;
                }
            }
            b'h' => {
                let index = i32::from(self.readbyte());
                self.memo_get(index)?;
            }
            b'j' => {
                let index = self.read_i32()?;
                self.memo_get(index)?;
            }
            b'q' => {
                let index = i32::from(self.readbyte());
                self.memo_put(index)?;
            }
            b'r' => {
                let index = self.read_i32()?;
                self.memo_put(index)?;
            }
            0x94 => {
                let index = self.memo.len() as i32;
                self.memo_put(index)?;
            }
            b'i' => {
                let module = self.readline(false)?;
                let name = self.readline(false)?;
                let mut args = self.pop_all_since_marker()?;
                let ctor = match known_ctor(&module, &name) {
                    Some(ctor) => ctor,
                    None => {
                        args.clear();
                        Ctor::ClassDict(format!("{module}.{name}"))
                    }
                };
                let object = construct(&ctor, &args)?;
                self.stack.push(object);
            }
            b'o' => {
                let mut args = self.pop_all_since_marker()?;
                if args.is_empty() {
                    return Err(Error::runtime(
                        "java.lang.IndexOutOfBoundsException",
                        "Index 0 out of bounds for length 0",
                    ));
                }
                let ctor = args.remove(0);
                let object = match ctor {
                    Py::Ctor(ctor) => construct(&ctor, &args)?,
                    other => {
                        return Err(cast_error(
                            &other,
                            "net.razorvine.pickle.IObjectConstructor",
                        ));
                    }
                };
                self.stack.push(object);
            }
            b'l' => {
                let items = self.pop_all_since_marker()?;
                self.stack.push(Py::List(Rc::new(RefCell::new(items))));
            }
            b']' => self.stack.push(Py::List(Rc::new(RefCell::new(Vec::new())))),
            b's' => {
                let value = self.pop()?;
                let k = self.pop()?;
                let hash = k.hash(0)?;
                self.dict_target()?.borrow_mut().put(hash, k, value);
            }
            b'u' => {
                let mut newitems = JavaHashMap::new();
                loop {
                    let value = self.pop()?;
                    if value == Py::Mark {
                        break;
                    }
                    let k = self.pop()?;
                    newitems.put(k.hash(0)?, k, value);
                }
                let target = self.dict_target()?;
                target.borrow_mut().put_all(newitems.into_entries());
            }
            b't' => {
                let items = self.pop_all_since_marker()?;
                self.stack.push(Py::Tuple(Rc::from(items)));
            }
            b')' => self.stack.push(Py::Tuple(Rc::from(Vec::new()))),
            0x85..=0x87 => {
                let n = usize::from(key - 0x84);
                if self.stack.len() < n {
                    return Err(empty_stack());
                }
                let items = self.stack.split_off(self.stack.len() - n);
                self.stack.push(Py::Tuple(Rc::from(items)));
            }
            b'G' => {
                let bytes = self.readbytes(8)?;
                let mut be = [0u8; 8];
                be.copy_from_slice(bytes);
                self.stack
                    .push(Py::Float(f64::from_bits(u64::from_be_bytes(be))));
            }
            0x80 => {
                let proto = self.readbyte();
                if !(0..=HIGHEST_PROTOCOL).contains(&proto) {
                    return Err(pickle_error(format!(
                        "unsupported pickle protocol: {proto}"
                    )));
                }
            }
            0x82..=0x84 => {
                return Err(pickle_error(
                    "Unimplemented opcode EXT1/EXT2/EXT4 encountered. Don't use extension codes when pickling via copyreg.add_extension() to avoid this error.",
                ));
            }
            0x88 => self.stack.push(Py::Bool(true)),
            0x89 => self.stack.push(Py::Bool(false)),
            0x8f => {
                let set = JavaHashMap::new();
                self.stack.push(Py::Set(Rc::new(RefCell::new(set))));
            }
            0x90 => {
                let items = self.pop_all_since_marker()?;
                let set = match self.pop()? {
                    Py::Set(set) => set,
                    other => {
                        return Err(cast_error(&other, "java.util.HashSet"));
                    }
                };
                for item in items {
                    let hash = item.hash(0)?;
                    set.borrow_mut().put(hash, item, ());
                }
                self.stack.push(Py::Set(set));
            }
            0x91 => {
                let items = self.pop_all_since_marker()?;
                let mut set = JavaHashMap::new();
                for item in items {
                    set.put(item.hash(0)?, item, ());
                }
                self.stack.push(Py::Set(Rc::new(RefCell::new(set))));
            }
            0x92 => {
                let empty = match self.pop()? {
                    Py::Dict(dict, _) => dict.borrow().is_empty(),
                    other => {
                        return Err(cast_error(&other, "java.util.HashMap"));
                    }
                };
                let args = self.pop()?;
                let ctor = self.pop()?;
                if !empty {
                    return Err(pickle_error(
                        "newobj_ex with keyword arguments not supported",
                    ));
                }
                self.stack.push(ctor);
                self.stack.push(args);
                self.reduce()?;
            }
            0x95 => {
                self.readbytes(8)?;
            }
            0x97 => {
                return Err(pickle_error(
                    "pickle stream refers to out-of-band data but no user-overridden next_buffer() method is used\n",
                ));
            }
            0x98 => {}
            _ => {
                return Err(pickle_error(format!("invalid pickle opcode: {key}")));
            }
        }
        Ok(None)
    }

    fn persistent() -> Error {
        pickle_error(
            "A load persistent id instruction was encountered, but no persistentLoad function was specified. (implement it in custom Unpickler subclass)",
        )
    }

    fn global(&mut self, module: &str, name: &str) {
        let ctor = known_ctor(module, name).unwrap_or_else(|| {
            let qualified = format!("{module}.{name}");
            let builtin = module == "builtins" || module == "__builtin__";
            let exception = name.ends_with("Error")
                || name.ends_with("Warning")
                || name.ends_with("Exception")
                || matches!(
                    name,
                    "GeneratorExit" | "KeyboardInterrupt" | "StopIteration" | "SystemExit"
                );
            if module == "exceptions" || (builtin && exception) {
                Ctor::Exception(qualified)
            } else {
                Ctor::ClassDict(qualified)
            }
        });
        self.stack.push(Py::Ctor(Rc::new(ctor)));
    }

    fn reduce(&mut self) -> Result<()> {
        // Java casts `null` silently, and fails on its use.
        let args = match self.pop()? {
            Py::Tuple(args) => Some(args),
            Py::None => None,
            other => return Err(cast_error(&other, "[Ljava.lang.Object;")),
        };
        let ctor = self.pop()?;
        let (Some(args), false) = (args, ctor == Py::None) else {
            return Err(Error::null_pointer());
        };
        let Py::Ctor(ctor) = ctor else {
            return Err(cast_error(&ctor, "net.razorvine.pickle.IObjectConstructor"));
        };
        let object = construct(&ctor, &args)?;
        self.stack.push(object);
        Ok(())
    }

    /// `BUILD`: only a `ClassDict` given a plain `HashMap` has a `__setstate__`.
    fn build(&mut self) -> Result<()> {
        let args = self.pop()?;
        let target = self.peek()?;
        match (target, &args) {
            (Py::Dict(dict, Some(class)), Py::Dict(values, None)) => {
                let entries: Vec<(i32, Py, Py)> = values.borrow().iter().cloned().collect();
                let mut state = JavaHashMap::with_capacity(0);
                state.put(
                    string_hash("__class__"),
                    Py::Str(Rc::from("__class__")),
                    Py::Str(class.clone()),
                );
                for (h, k, v) in entries {
                    state.put(h, k, v);
                }
                *dict.borrow_mut() = state;
                Ok(())
            }
            (target, _) => Err(pickle_error(format!(
                "failed to __setstate__() ({}.__setstate__({}))",
                target.java_class(),
                args.java_class()
            ))),
        }
    }
}

/// The registered constructors (`Unpickler.objectConstructors`).
fn known_ctor(module: &str, name: &str) -> Option<Ctor> {
    Some(match (module, name) {
        ("__builtin__" | "builtins", "complex") => Ctor::Complex,
        ("array", "array" | "_array_reconstructor") => Ctor::Array,
        ("__builtin__" | "builtins", "bytearray")
        | ("__builtin__", "bytes")
        | ("_codecs", "encode") => Ctor::ByteArray,
        ("__builtin__" | "builtins", "set") => Ctor::Set,
        ("datetime", "datetime") => Ctor::DateTime(DateKind::DateTime),
        ("datetime", "time") => Ctor::DateTime(DateKind::Time),
        ("datetime", "date") => Ctor::DateTime(DateKind::Date),
        ("datetime", "timedelta") => Ctor::DateTime(DateKind::TimeDelta),
        ("pytz", "_UTC") => Ctor::Zone(ZoneKind::Utc),
        ("pytz", "_p" | "timezone") => Ctor::Zone(ZoneKind::Pytz),
        ("dateutil.tz", "tzutc") => Ctor::Zone(ZoneKind::DateutilUtc),
        ("dateutil.zoneinfo", "gettz") => Ctor::Zone(ZoneKind::DateutilGettz),
        ("dateutil.tz", "tzfile") => Ctor::Zone(ZoneKind::DateutilTzfile),
        ("datetime", "tzinfo") => Ctor::Zone(ZoneKind::Unsupported),
        ("decimal", "Decimal") => Ctor::Decimal,
        ("copy_reg", "_reconstructor") => Ctor::Reconstructor,
        ("operator", "attrgetter") => Ctor::AttrGetter,
        _ => return None,
    })
}

/// `"problem construction object: java.lang.NoSuchMethodException: …"` of
/// `AnyClassConstructor`.
fn no_such_constructor(class: &str, args: &[Py]) -> Error {
    if args.contains(&Py::None) {
        return pickle_error(
            "problem construction object: java.lang.NullPointerException: Cannot invoke \"Object.getClass()\" because \"args[i]\" is null",
        );
    }
    let types: Vec<&str> = args.iter().map(Py::java_class).collect();
    pickle_error(format!(
        "problem construction object: java.lang.NoSuchMethodException: {class}.<init>({})",
        types.join(", ")
    ))
}

#[allow(clippy::too_many_lines)]
fn construct(ctor: &Ctor, args: &[Py]) -> Result<Py> {
    match ctor {
        Ctor::ClassDict(name) => {
            if !args.is_empty() {
                return Err(pickle_error(format!(
                    "expected zero arguments for construction of ClassDict (for {name})"
                )));
            }
            let mut map = JavaHashMap::new();
            map.put(
                string_hash("__class__"),
                Py::Str(Rc::from("__class__")),
                Py::Str(Rc::from(name.as_str())),
            );
            Ok(Py::Dict(
                Rc::new(RefCell::new(map)),
                Some(Rc::from(name.as_str())),
            ))
        }
        Ctor::Exception(name) => {
            let message = match args.first() {
                None => format!("[{name}]"),
                Some(Py::Str(s)) => format!("[{name}] {s}"),
                Some(other) => {
                    return Err(pickle_error(format!(
                        "problem construction object: java.lang.ClassCastException: class {} cannot be cast to class java.lang.String",
                        other.java_class()
                    )));
                }
            };
            Ok(Py::Exception(Rc::from(message)))
        }
        Ctor::Complex => match args {
            [Py::Float(r), Py::Float(i)] => Ok(Py::Complex(*r, *i)),
            _ => Err(no_such_constructor(
                "net.razorvine.pickle.objects.ComplexNumber",
                args,
            )),
        },
        Ctor::Decimal => match args {
            [Py::Str(s)] => BigDecimal::parse(s)
                .map(|d| Py::Decimal(Rc::new(d)))
                .map_err(|_| {
                    pickle_error(
                        "problem construction object: java.lang.reflect.InvocationTargetException",
                    )
                }),
            _ => Err(no_such_constructor("java.math.BigDecimal", args)),
        },
        Ctor::Set => match args.first() {
            Some(Py::List(list)) => {
                let items = list.borrow();
                let mut set = JavaHashMap::for_set_copy(items.len());
                for item in items.iter() {
                    set.put(item.hash(0)?, item.clone(), ());
                }
                Ok(Py::Set(Rc::new(RefCell::new(set))))
            }
            Some(other) => Err(cast_error(other, "java.util.ArrayList")),
            None => Err(Error::runtime(
                "java.lang.ArrayIndexOutOfBoundsException",
                "Index 0 out of bounds for length 0",
            )),
        },
        Ctor::ByteArray => byte_array(args),
        Ctor::Array => array(args),
        Ctor::DateTime(kind) => date_time(*kind, args),
        Ctor::Zone(kind) => zone(*kind, args),
        Ctor::Reconstructor => {
            if args.len() != 3 {
                return Err(pickle_error(format!(
                    "invalid pickle data; expecting 3 args to copy_reg reconstructor but recieved {}",
                    args.len()
                )));
            }
            Err(pickle_error(format!(
                "failed to reconstruct() ({}.reconstruct(java.lang.Object, java.lang.Object))",
                args[0].java_class()
            )))
        }
        Ctor::AttrGetter => match args {
            [Py::Str(s)] if &**s == "localize" => Ok(Py::Ctor(Rc::new(Ctor::AttrGetterForTz))),
            [_] => Err(pickle_error(
                "expected 'localize' string argument for construction of AttrGetter",
            )),
            _ => Err(pickle_error(
                "expected exactly one string argument for construction of AttrGetter",
            )),
        },
        Ctor::AttrGetterForTz => match args {
            [Py::Zone(id)] => Ok(Py::Ctor(Rc::new(Ctor::Localizer(id.to_string())))),
            _ => Err(pickle_error(
                "expected exactly one TimeZone argument for construction of CalendarLocalizer",
            )),
        },
        Ctor::Localizer(zone_id) => match args {
            [Py::Calendar(ms)] => Ok(Py::Calendar(local_to_utc(*ms, Some(zone_id)))),
            _ => Err(pickle_error(
                "expected exactly one Calendar argument for construction of Calendar with timezone",
            )),
        },
    }
}

/// `ByteArrayConstructor`.
fn byte_array(args: &[Py]) -> Result<Py> {
    match args {
        [Py::Bytes(b)] => Ok(Py::Bytes(b.clone())),
        [Py::List(values)] => {
            let data: Vec<u8> = values
                .borrow()
                .iter()
                .map(|v| v.long_value().map(|l| l as u8))
                .collect::<Result<_>>()?;
            Ok(Py::Bytes(Rc::from(data)))
        }
        [other] => Err(cast_error(other, "java.util.ArrayList")),
        [data, encoding] => {
            let (Py::Str(data), Py::Str(encoding)) = (data, encoding) else {
                let culprit = if matches!(data, Py::Str(_)) {
                    encoding
                } else {
                    data
                };
                return Err(cast_error(culprit, "java.lang.String"));
            };
            let encoding = match encoding.strip_prefix("latin-") {
                Some(n) => format!("ISO-8859-{n}"),
                None => encoding.to_string(),
            };
            let bytes = match encoding.to_ascii_uppercase().as_str() {
                "ISO-8859-1" | "LATIN1" | "ISO8859_1" => data
                    .chars()
                    .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
                    .collect(),
                "UTF-8" | "UTF8" => data.as_bytes().to_vec(),
                "US-ASCII" | "ASCII" => data
                    .chars()
                    .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
                    .collect(),
                _ => {
                    return Err(pickle_error(format!(
                        "error creating bytearray: java.io.UnsupportedEncodingException: {encoding}"
                    )));
                }
            };
            Ok(Py::Bytes(Rc::from(bytes)))
        }
        _ => Err(pickle_error(format!(
            "invalid pickle data for bytearray; expected 1 or 2 args, got {}",
            args.len()
        ))),
    }
}

/// The boxed elements of a primitive array of `typecode` built from `values`.
fn typed_elements(typecode: char, values: &[Py]) -> Result<Py> {
    let items: Vec<Py> = match typecode {
        'c' | 'u' => {
            let s: String = values
                .iter()
                .map(|v| match v {
                    Py::Str(s) => s.chars().next().ok_or_else(|| {
                        Error::runtime(
                            "java.lang.StringIndexOutOfBoundsException",
                            "Index 0 out of bounds for length 0",
                        )
                    }),
                    other => Err(cast_error(other, "java.lang.String")),
                })
                .collect::<Result<_>>()?;
            return Ok(Py::Str(Rc::from(s)));
        }
        'b' => {
            let data: Vec<u8> = values
                .iter()
                .map(|v| v.long_value().map(|l| l as u8))
                .collect::<Result<_>>()?;
            return Ok(Py::Bytes(Rc::from(data)));
        }
        'B' | 'h' => values
            .iter()
            .map(|v| v.long_value().map(|l| Py::Int(i32::from(l as i16))))
            .collect::<Result<_>>()?,
        'H' | 'i' | 'l' => values
            .iter()
            .map(|v| v.long_value().map(|l| Py::Int(l as i32)))
            .collect::<Result<_>>()?,
        'I' | 'L' => values
            .iter()
            .map(|v| v.long_value().map(Py::Long))
            .collect::<Result<_>>()?,
        'f' => values
            .iter()
            .map(|v| v.number().map(|d| Py::Float(widen(d as f32))))
            .collect::<Result<_>>()?,
        'd' => values
            .iter()
            .map(|v| v.number().map(Py::Float))
            .collect::<Result<_>>()?,
        _ => return Err(pickle_error(format!("invalid array typecode: {typecode}"))),
    };
    Ok(Py::Array(Rc::from(items)))
}

/// A `float` as the `double` of its shortest decimal form (as Jackson writes it).
fn widen(f: f32) -> f64 {
    f.to_string().parse().unwrap_or(f64::NAN)
}

/// `String.charAt(0)` of a type code.
fn first_char(typecode: &str) -> Result<char> {
    typecode.chars().next().ok_or_else(|| {
        Error::runtime(
            "java.lang.StringIndexOutOfBoundsException",
            "Index 0 out of bounds for length 0",
        )
    })
}

/// `ArrayConstructor`.
fn array(args: &[Py]) -> Result<Py> {
    match args {
        [constructor, typecode, machine, data] => {
            if !matches!(constructor, Py::None | Py::Ctor(_)) {
                return Err(cast_error(
                    constructor,
                    "net.razorvine.pickle.objects.ArrayConstructor",
                ));
            }
            let Py::Str(typecode) = typecode else {
                return Err(cast_error(typecode, "java.lang.String"));
            };
            let machine = machine.as_int()?;
            let Py::Bytes(data) = data else {
                return Err(cast_error(data, "[B"));
            };
            let typecode = first_char(typecode)?;
            if *constructor == Py::None {
                return Err(Error::runtime(
                    "java.lang.NullPointerException",
                    "Cannot invoke \"net.razorvine.pickle.objects.ArrayConstructor.construct(char, int, byte[])\" because \"constructor\" is null",
                ));
            }
            machine_array(typecode, machine, data)
        }
        [typecode, values] => {
            let Py::Str(typecode) = typecode else {
                return Err(cast_error(typecode, "java.lang.String"));
            };
            let values = match values {
                Py::Str(_) => {
                    return Err(pickle_error("unsupported Python 2.6 array pickle format"));
                }
                Py::List(values) => values.borrow().clone(),
                other => {
                    return Err(cast_error(other, "java.util.ArrayList"));
                }
            };
            typed_elements(first_char(typecode)?, &values)
        }
        _ => Err(pickle_error(format!(
            "invalid pickle data for array; expected 2 args, got {}",
            args.len()
        ))),
    }
}

/// `ArrayConstructor.construct(typecode, machinecode, data)` (`array._array_reconstructor`).
fn machine_array(typecode: char, machine: i32, data: &[u8]) -> Result<Py> {
    if machine < 0 {
        return Err(pickle_error("unknown machine type format"));
    }
    // The machine codes each type code accepts, as Pyrolite words them.
    let (allowed, codes): (&[i32], &str) = match typecode {
        'c' | 'u' => (&[18, 19, 20, 21], "c/u type must be 18/19/20/21"),
        'b' => (&[1], "b type must be 1"),
        'B' => (&[0], "B type must be 0"),
        'h' => (&[4, 5], "h type must be 4/5"),
        'H' => (&[2, 3], "H type must be 2/3"),
        'i' => (&[8, 9], "i type must be 8/9"),
        'l' => (&[8, 9, 12, 13], "l type must be 8/9/12/13"),
        'I' => (&[6, 7], "I type must be 6/7"),
        'L' => (&[6, 7, 10, 11], "L type must be 6/7/10/11"),
        'f' => (&[14, 15], "f type must be 14/15"),
        'd' => (&[16, 17], "d type must be 16/17"),
        _ => return Err(pickle_error(format!("invalid array typecode: {typecode}"))),
    };
    if !allowed.contains(&machine) {
        return Err(pickle_error(format!("for {codes}")));
    }
    if typecode == 'L' && machine >= 10 {
        if !data.len().is_multiple_of(8) {
            return Err(pickle_error("data size alignment error"));
        }
        return Err(pickle_error("unsupported datatype: 64-bits unsigned long"));
    }
    // (item size, signed, big endian, floating point) of the Python machine format.
    let (size, signed, big, float) = match machine {
        0 => (1, false, false, false),
        1 => (1, true, false, false),
        2 | 3 => (2, false, machine == 3, false),
        4 | 5 => (2, true, machine == 5, false),
        6 | 7 => (4, false, machine == 7, false),
        8 | 9 => (4, true, machine == 9, false),
        12 | 13 => (8, true, machine == 13, false),
        14 | 15 => (4, true, machine == 15, true),
        16 | 17 => (8, true, machine == 17, true),
        18 | 19 => (2, false, machine == 19, false),
        _ => (4, false, machine == 21, false),
    };
    if typecode == 'b' {
        return Ok(Py::Bytes(Rc::from(data)));
    }
    if !data.len().is_multiple_of(size) {
        return Err(pickle_error("data size alignment error"));
    }
    let raws = data.chunks(size).map(|c| {
        if big {
            c.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
        } else {
            c.iter()
                .rev()
                .fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
        }
    });
    if matches!(typecode, 'c' | 'u') {
        // `Character.toChars` of each code point, which must fit one UTF-16 unit.
        let mut units = Vec::new();
        for code in raws {
            let code = code as u32;
            if code > 0x10_ffff {
                return Err(Error::runtime(
                    "java.lang.IllegalArgumentException",
                    format!("Not a valid Unicode code point: 0x{code:X}"),
                ));
            }
            if code > 0xffff {
                return Err(pickle_error(format!(
                    "cannot process UTF-32 character codepoint {code}"
                )));
            }
            units.push(code as u16);
        }
        return Ok(Py::Str(Rc::from(String::from_utf16_lossy(&units))));
    }
    let numbers = raws.map(|raw| {
        if float && size == 4 {
            Py::Float(widen(f32::from_bits(raw as u32)))
        } else if float {
            Py::Float(f64::from_bits(raw))
        } else if signed {
            let shift = 64 - size * 8;
            Py::Long(((raw << shift) as i64) >> shift)
        } else {
            Py::Long(raw as i64)
        }
    });
    Ok(Py::Array(numbers.collect()))
}

/// Days since the epoch of a proleptic Gregorian date (`month` 1-based, lenient).
fn epoch_days(year: i64, month: i64, day: i64) -> i64 {
    let m0 = month - 1;
    let year = year + m0.div_euclid(12);
    let month = m0.rem_euclid(12) + 1;
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468 + (day - 1)
}

/// The instant of a calendar set to local `fields` (year, 1-based month, day, hour, minute,
/// second, millisecond) in the zone `zone` (UTC when unknown, as `TimeZone.getTimeZone`
/// falls back to GMT).
fn calendar_millis(fields: [i64; 7], zone: Option<&str>) -> i64 {
    let local = epoch_days(fields[0], fields[1], fields[2]) * 86_400_000
        + fields[3] * 3_600_000
        + fields[4] * 60_000
        + fields[5] * 1000
        + fields[6];
    local_to_utc(local, zone)
}

/// The instant of the local time `local` (milliseconds) in the zone `zone`.
fn local_to_utc(local: i64, zone: Option<&str>) -> i64 {
    let offset = zone
        .and_then(|id| id.parse::<chrono_tz::Tz>().ok())
        .map_or(0, |tz| {
            use chrono::{Offset, TimeZone};
            let naive = chrono::DateTime::from_timestamp_millis(local)
                .unwrap_or_default()
                .naive_utc();
            // Ambiguous local times take the standard (later) offset; skipped ones the
            // offset before the transition.
            tz.offset_from_local_datetime(&naive)
                .latest()
                .unwrap_or_else(|| {
                    tz.offset_from_utc_datetime(&(naive - chrono::Duration::days(1)))
                })
                .fix()
                .local_minus_utc()
        });
    local - i64::from(offset) * 1000
}

/// `DateTimeConstructor`.
fn date_time(kind: DateKind, args: &[Py]) -> Result<Py> {
    let packed = |arg: &Py, len: usize, what: &str| -> Result<Vec<i64>> {
        let codes: Vec<i64> = match arg {
            Py::Str(s) => s.encode_utf16().map(i64::from).collect(),
            Py::Bytes(b) => b.iter().map(|x| i64::from(*x)).collect(),
            other => return Err(cast_error(other, "[B")),
        };
        if codes.len() != len {
            return Err(pickle_error(format!(
                "invalid pickle data for {what}; expected arg of length {len}, got length {}",
                codes.len()
            )));
        }
        Ok(codes)
    };
    let ints = |args: &[Py]| -> Result<Vec<i64>> {
        args.iter().map(|a| a.as_int().map(i64::from)).collect()
    };
    match kind {
        DateKind::TimeDelta => {
            if args.len() != 3 {
                return Err(pickle_error(format!(
                    "invalid pickle data for timedelta; expected 3 args, got {}",
                    args.len()
                )));
            }
            let parts: Vec<i32> = args
                .iter()
                .map(|a| a.long_value().map(|l| l as i32))
                .collect::<Result<_>>()?;
            Ok(Py::TimeDelta([parts[0], parts[1], parts[2]]))
        }
        DateKind::Time => {
            if args.len() == 4 {
                let t = ints(args)?;
                return Ok(Py::Time([
                    t[0] as i32,
                    t[1] as i32,
                    t[2] as i32,
                    t[3] as i32,
                ]));
            }
            if args.len() != 1 {
                return Err(pickle_error(format!(
                    "invalid pickle data for time; expected 1 or 4 args, got {}",
                    args.len()
                )));
            }
            let what = if matches!(args[0], Py::Str(_)) {
                "time"
            } else {
                "datetime"
            };
            let p = packed(&args[0], 6, what)?;
            let micro = ((p[3] << 8) | p[4]) << 8 | p[5];
            Ok(Py::Time([
                p[0] as i32,
                p[1] as i32,
                p[2] as i32,
                micro as i32,
            ]))
        }
        DateKind::Date => {
            if args.len() == 3 {
                let d = ints(args)?;
                return Ok(Py::Calendar(calendar_millis(
                    [d[0], d[1], d[2], 0, 0, 0, 0],
                    Some("UTC"),
                )));
            }
            if args.len() != 1 {
                return Err(pickle_error(format!(
                    "invalid pickle data for date; expected 1 arg, got {}",
                    args.len()
                )));
            }
            let p = packed(&args[0], 4, "date")?;
            Ok(Py::Calendar(calendar_millis(
                [p[0] * 256 + p[1], p[2], p[3], 0, 0, 0, 0],
                Some("UTC"),
            )))
        }
        DateKind::DateTime => {
            if args.len() == 7 || args.len() == 8 {
                let f = ints(&args[..7])?;
                let zone = match args.get(7) {
                    Some(Py::Zone(id)) => Some(id.to_string()),
                    Some(other) => {
                        return Err(cast_error(other, "java.util.TimeZone"));
                    }
                    None => None,
                };
                let fields = [f[0], f[1], f[2], f[3], f[4], f[5], f[6] / 1000];
                return Ok(Py::Calendar(calendar_millis(
                    fields,
                    Some(zone.as_deref().unwrap_or("UTC")),
                )));
            }
            if args.len() != 1 && args.len() != 2 {
                return Err(pickle_error(format!(
                    "invalid pickle data for datetime; expected 1, 2, 7 or 8 args, got {}",
                    args.len()
                )));
            }
            let p = packed(&args[0], 10, "datetime")?;
            let micro = ((p[7] << 8) | p[8]) << 8 | p[9];
            let zone = match args.get(1) {
                None => Some("UTC".to_owned()),
                Some(Py::Zone(id)) => Some(id.to_string()),
                Some(Py::Tzinfo(id)) => id.as_deref().map(str::to_owned),
                Some(_) => {
                    return Err(pickle_error(
                        "invalid pickle data for datetime; expected arg 2 to be a Tzinfo or TimeZone",
                    ));
                }
            };
            let fields = [
                p[0] * 256 + p[1],
                p[2],
                p[3],
                p[4],
                p[5],
                p[6],
                micro / 1000,
            ];
            Ok(Py::Calendar(calendar_millis(
                fields,
                Some(zone.as_deref().unwrap_or("UTC")),
            )))
        }
    }
}

/// `TimeZoneConstructor`.
fn zone(kind: ZoneKind, args: &[Py]) -> Result<Py> {
    let one_string = |what: &str| -> Result<Rc<str>> {
        if args.len() != 1 {
            return Err(pickle_error(format!(
                "invalid pickle data for {what}; expected 1 args, got {}",
                args.len()
            )));
        }
        match &args[0] {
            Py::Str(s) => Ok(s.clone()),
            other => Err(cast_error(other, "java.lang.String")),
        }
    };
    match kind {
        ZoneKind::Utc => Ok(Py::Zone(Rc::from("UTC"))),
        ZoneKind::DateutilUtc => Ok(Py::Tzinfo(Some(Rc::from("UTC")))),
        ZoneKind::DateutilGettz => Ok(Py::Tzinfo(Some(one_string("dateutil gettz call")?))),
        ZoneKind::Pytz => {
            if args.len() != 4 && args.len() != 1 {
                return Err(pickle_error(format!(
                    "invalid pickle data for pytz timezone; expected 1 or 4 args, got {}",
                    args.len()
                )));
            }
            match &args[0] {
                Py::Str(s) => Ok(Py::Zone(s.clone())),
                _ => Err(pickle_error(
                    "invalid pickle data for pytz timezone; expected string argument as first tuple member",
                )),
            }
        }
        ZoneKind::DateutilTzfile => {
            let path = one_string("dateutil tzfile timezone")?;
            let index = path.find("zoneinfo").ok_or_else(|| {
                pickle_error(format!(
                    "couldn't parse timezone identifier from zoneinfo path{path}"
                ))
            })?;
            let id = path.get(index + 9..).ok_or_else(|| {
                Error::runtime(
                    "java.lang.StringIndexOutOfBoundsException",
                    format!(
                        "begin {}, end {}, length {}",
                        index + 9,
                        path.len(),
                        path.len()
                    ),
                )
            })?;
            Ok(Py::Tzinfo(Some(Rc::from(id))))
        }
        ZoneKind::Unsupported => Ok(Py::Tzinfo(None)),
    }
}

/// `PickleUtils.decode_escaped` (`unicode` for the `\uXXXX` flavour of `decode_unicode_escaped`).
fn decode_escaped(s: &str, unicode: bool) -> Result<String> {
    if !s.contains('\\') {
        return Ok(s.to_owned());
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out: Vec<u16> = Vec::new();
    let out_of_range = |i: usize| {
        Error::runtime(
            "java.lang.StringIndexOutOfBoundsException",
            format!("Index {i} out of bounds for length {}", chars.len()),
        )
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c != '\\' {
            let mut buf = [0u16; 2];
            out.extend_from_slice(c.encode_utf16(&mut buf));
            i += 1;
            continue;
        }
        i += 1;
        let c2 = *chars.get(i).ok_or_else(|| out_of_range(i))?;
        match c2 {
            '\\' => out.push(u16::from(b'\\')),
            'n' => out.push(u16::from(b'\n')),
            'r' => out.push(u16::from(b'\r')),
            't' => out.push(u16::from(b'\t')),
            '\'' if !unicode => out.push(u16::from(b'\'')),
            'x' | 'u' if (c2 == 'x') != unicode => {
                let width = if unicode { 4 } else { 2 };
                let mut hex = String::new();
                for _ in 0..width {
                    i += 1;
                    hex.push(*chars.get(i).ok_or_else(|| out_of_range(i))?);
                }
                let code = i32::from_str_radix(&hex, 16).map_err(|_| number_format(&hex))?;
                out.push(code as u16);
            }
            _ => {
                let shown: String = s.chars().take(80).collect();
                return Err(pickle_error(format!(
                    "invalid escape sequence char '{c2}' in string \"{shown} [...]\" (possibly truncated)"
                )));
            }
        }
        i += 1;
    }
    Ok(String::from_utf16_lossy(&out))
}

/// The stack value of a decoded object; `seen` maps the shared mutable containers to their
/// value, so that sharing (and cycles) survive the conversion.
fn to_value(py: &Py, seen: &mut HashMap<usize, Value>, depth: usize) -> Result<Value> {
    if depth > MAX_DEPTH {
        return Err(Error::runtime(
            "java.lang.StackOverflowError",
            "StackOverflowError",
        ));
    }
    let key = |rc_ptr: *const ()| rc_ptr as usize;
    Ok(match py {
        Py::None | Py::Mark | Py::Ctor(_) | Py::Tzinfo(_) => Value::Null,
        Py::Bool(b) => Value::Boolean(*b),
        Py::Int(i) => Value::Long(i64::from(*i)),
        Py::Long(l) => Value::Long(*l),
        Py::Big(b) => Value::BigDecimal(Rc::new(BigDecimal::from_bigint((**b).clone()))),
        Py::Float(d) => Value::Double(*d),
        Py::Str(s) | Py::Zone(s) | Py::Exception(s) => Value::String(s.clone()),
        Py::Bytes(b) => Value::Bytes(b.clone()),
        Py::Decimal(d) => Value::BigDecimal(d.clone()),
        Py::Calendar(ms) => Value::Long(*ms),
        Py::Complex(r, i) => fields(&[
            ("real", Value::Double(*r)),
            ("imaginary", Value::Double(*i)),
        ]),
        Py::Time(t) => fields(&[
            ("hours", Value::Long(i64::from(t[0]))),
            ("minutes", Value::Long(i64::from(t[1]))),
            ("seconds", Value::Long(i64::from(t[2]))),
            ("microseconds", Value::Long(i64::from(t[3]))),
        ]),
        Py::TimeDelta(d) => fields(&[
            ("days", Value::Long(i64::from(d[0]))),
            ("seconds", Value::Long(i64::from(d[1]))),
            ("microseconds", Value::Long(i64::from(d[2]))),
            ("total_seconds", Value::Double(total_seconds(*d))),
        ]),
        Py::Tuple(items) | Py::Array(items) => {
            let id = key(Rc::as_ptr(items).cast());
            if let Some(v) = seen.get(&id) {
                return Ok(v.clone());
            }
            let list = Rc::new(RefCell::new(Vec::new()));
            seen.insert(id, Value::List(list.clone()));
            for item in items.iter() {
                let v = to_value(item, seen, depth + 1)?;
                list.borrow_mut().push(v);
            }
            Value::List(list)
        }
        Py::List(items) => {
            let id = key(Rc::as_ptr(items).cast());
            if let Some(v) = seen.get(&id) {
                return Ok(v.clone());
            }
            let list = Rc::new(RefCell::new(Vec::new()));
            seen.insert(id, Value::List(list.clone()));
            let items = items.borrow().clone();
            for item in &items {
                let v = to_value(item, seen, depth + 1)?;
                list.borrow_mut().push(v);
            }
            Value::List(list)
        }
        Py::Dict(dict, _) => {
            let id = key(Rc::as_ptr(dict).cast());
            if let Some(v) = seen.get(&id) {
                return Ok(v.clone());
            }
            let map = Rc::new(RefCell::new(IndexMap::new()));
            seen.insert(id, Value::Map(map.clone()));
            let entries: Vec<(i32, Py, Py)> = dict.borrow().iter().cloned().collect();
            for (_, k, v) in &entries {
                let k = to_value(k, seen, depth + 1)?;
                let v = to_value(v, seen, depth + 1)?;
                map.borrow_mut().insert(k, v);
            }
            Value::Map(map)
        }
        Py::Set(set) => {
            let id = key(Rc::as_ptr(set).cast());
            if let Some(v) = seen.get(&id) {
                return Ok(v.clone());
            }
            let out = Rc::new(RefCell::new(IndexSet::new()));
            seen.insert(id, Value::Set(out.clone()));
            let items: Vec<Py> = set.borrow().iter().map(|(_, k, ())| k.clone()).collect();
            for item in &items {
                let v = to_value(item, seen, depth + 1)?;
                out.borrow_mut().insert(v);
            }
            Value::Set(out)
        }
    })
}

/// A map of named fields (the objects without a WarpScript counterpart).
fn fields(entries: &[(&str, Value)]) -> Value {
    let map: IndexMap<Value, Value> = entries
        .iter()
        .map(|(k, v)| (Value::from(*k), v.clone()))
        .collect();
    Value::Map(Rc::new(RefCell::new(map)))
}
