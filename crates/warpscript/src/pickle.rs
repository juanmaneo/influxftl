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
use crate::error::Result;
use crate::error::compat::{
    self, JAVA_CLASS_CAST_EXCEPTION, JAVA_ILLEGAL_ARGUMENT_EXCEPTION,
    JAVA_INVOCATION_TARGET_EXCEPTION, JAVA_IO_EXCEPTION, JAVA_NO_SUCH_METHOD_EXCEPTION,
    JAVA_NULL_POINTER_EXCEPTION, JAVA_OBJECT, JAVA_STRING, JAVA_UNSUPPORTED_ENCODING_EXCEPTION,
};
use crate::error::fault::BoundsError;
use crate::error::fault::NumberFormatError;
use crate::geoxp::from_geoxp_point;
use crate::gts::{GeoTimeSerie, NO_LOCATION};
use crate::hash_order_map::HashOrderMap;
use crate::unpickle::PyKind;
use crate::value::{Value, ValueKind};

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

/// A failure of the pickle encoder or decoder, worded like Pyrolite's (`PickleException`,
/// unless the variant says otherwise) because a script reads it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PickleError {
    /// The input ends in the middle of an opcode (an `IOException`).
    #[error("{JAVA_IO_EXCEPTION}: premature end of file")]
    PrematureEof,
    /// The input ends in the middle of an operand (an `IOException`).
    #[error("{JAVA_IO_EXCEPTION}: expected more bytes in input stream")]
    ExpectedMoreBytes,
    /// A self-containing structure was hashed (a `StackOverflowError`).
    #[error("StackOverflowError")]
    HashRecursion,
    /// Objects nested too deep to be converted to a value (a `StackOverflowError`).
    #[error("StackOverflowError")]
    ConversionTooDeep,
    /// Values nested too deep to be pickled (a `StackOverflowError`).
    #[error("recursion too deep in Pickler.save (>1000)")]
    SaveRecursion,
    /// A persistent id instruction that popped nothing (a `NullPointerException`).
    #[error(
        "Cannot invoke \"Object.toString()\" because the return value of \"net.razorvine.pickle.UnpickleStack.pop()\" is null"
    )]
    PersistentIdNull,
    /// An array reconstruction without its constructor (a `NullPointerException`).
    #[error(
        "Cannot invoke \"net.razorvine.pickle.objects.ArrayConstructor.construct(char, int, byte[])\" because \"constructor\" is null"
    )]
    NullArrayConstructor,
    /// An operand of more than `i32::MAX` bytes.
    #[error("pickle too large, can't read more than maxint")]
    TooLarge,
    /// A memo reference to an index nothing was stored at.
    #[error("invalid memo key")]
    InvalidMemoKey,
    /// A string pickle that is not quoted.
    #[error("insecure string pickle")]
    InsecureString,
    /// A protocol version the decoder does not know.
    #[error("unsupported pickle protocol: {0}")]
    UnsupportedProtocol(i16),
    /// An extension code opcode.
    #[error(
        "Unimplemented opcode EXT1/EXT2/EXT4 encountered. Don't use extension codes when pickling via copyreg.add_extension() to avoid this error."
    )]
    ExtensionOpcode,
    /// A `NEWOBJ_EX` with keyword arguments.
    #[error("newobj_ex with keyword arguments not supported")]
    KeywordArguments,
    /// A reference to out-of-band data.
    #[error(
        "pickle stream refers to out-of-band data but no user-overridden next_buffer() method is used\n"
    )]
    OutOfBandData,
    /// An opcode the decoder does not know.
    #[error("invalid pickle opcode: {0}")]
    InvalidOpcode(u8),
    /// A persistent id instruction.
    #[error(
        "A load persistent id instruction was encountered, but no persistentLoad function was specified. (implement it in custom Unpickler subclass)"
    )]
    PersistentLoad,
    /// A `BUILD` on an object that is not a dictionary.
    #[error(
        "failed to __setstate__() ({}.__setstate__({}))",
        compat::py_class(*target),
        compat::py_class(*state)
    )]
    SetState {
        /// What the state was set on.
        target: PyKind,
        /// What the state was.
        state: PyKind,
    },
    /// A constructor given a null argument.
    #[error(
        "problem construction object: {JAVA_NULL_POINTER_EXCEPTION}: Cannot invoke \"Object.getClass()\" because \"args[i]\" is null"
    )]
    NullConstructorArgument,
    /// A constructor given arguments of types it does not take.
    #[error(
        "problem construction object: {JAVA_NO_SUCH_METHOD_EXCEPTION}: {}.<init>({})",
        compat::py_class(*class),
        compat::py_classes(arguments)
    )]
    NoSuchConstructor {
        /// What the constructor builds.
        class: PyKind,
        /// The kinds of the arguments given.
        arguments: Vec<PyKind>,
    },
    /// A `ClassDict` constructor given arguments.
    #[error("expected zero arguments for construction of ClassDict (for {0})")]
    ClassDictArguments(String),
    /// An exception constructor given a message that is not a string.
    #[error(
        "problem construction object: {JAVA_CLASS_CAST_EXCEPTION}: class {} cannot be cast to class {JAVA_STRING}",
        compat::py_class(*.0)
    )]
    ExceptionMessageNotString(PyKind),
    /// A `BigDecimal` constructor that refused its argument.
    #[error("problem construction object: {JAVA_INVOCATION_TARGET_EXCEPTION}")]
    ConstructionFailed(#[source] NumberFormatError),
    /// A `copy_reg._reconstructor` not given three arguments.
    #[error("invalid pickle data; expecting 3 args to copy_reg reconstructor but recieved {0}")]
    ReconstructorArity(usize),
    /// A `copy_reg._reconstructor` that cannot rebuild its class.
    #[error(
        "failed to reconstruct() ({}.reconstruct({JAVA_OBJECT}, {JAVA_OBJECT}))",
        compat::py_class(*.0)
    )]
    Reconstruct(PyKind),
    /// An `operator.attrgetter` of something else than `localize`.
    #[error("expected 'localize' string argument for construction of AttrGetter")]
    AttrGetterArgument,
    /// An `operator.attrgetter` not given exactly one argument.
    #[error("expected exactly one string argument for construction of AttrGetter")]
    AttrGetterArity,
    /// A calendar localizer not given a time zone.
    #[error("expected exactly one TimeZone argument for construction of CalendarLocalizer")]
    LocalizerArgument,
    /// A calendar localization not given a calendar.
    #[error("expected exactly one Calendar argument for construction of Calendar with timezone")]
    CalendarArgument,
    /// A bytearray in a character set the decoder does not know.
    #[error("error creating bytearray: {JAVA_UNSUPPORTED_ENCODING_EXCEPTION}: {0}")]
    UnsupportedEncoding(String),
    /// A constructor given the wrong number of arguments.
    #[error("invalid pickle data for {what}; expected {expected}, got {got}")]
    Arity {
        /// What was being built.
        what: &'static str,
        /// The numbers of arguments it takes.
        expected: &'static str,
        /// The number given.
        got: usize,
    },
    /// A packed value of the wrong length.
    #[error("invalid pickle data for {what}; expected arg of length {expected}, got length {got}")]
    PackedLength {
        /// What was being built.
        what: &'static str,
        /// The length it takes.
        expected: usize,
        /// The length given.
        got: usize,
    },
    /// An array type code that does not exist.
    #[error("invalid array typecode: {0}")]
    InvalidTypecode(char),
    /// A Python 2.6 array.
    #[error("unsupported Python 2.6 array pickle format")]
    Python26Array,
    /// A machine format code that does not exist.
    #[error("unknown machine type format")]
    UnknownMachineType,
    /// A machine format code the array type does not accept.
    #[error("for {0}")]
    MachineCode(&'static str),
    /// A byte count that is not a multiple of the item size.
    #[error("data size alignment error")]
    DataAlignment,
    /// An array of 64-bits unsigned longs.
    #[error("unsupported datatype: 64-bits unsigned long")]
    UnsignedLong,
    /// A UTF-32 character outside of the basic plane.
    #[error("cannot process UTF-32 character codepoint {0}")]
    Utf32Codepoint(u32),
    /// A datetime whose second argument is not a time zone.
    #[error("invalid pickle data for datetime; expected arg 2 to be a Tzinfo or TimeZone")]
    DatetimeZoneArgument,
    /// A pytz time zone whose first argument is not a string.
    #[error(
        "invalid pickle data for pytz timezone; expected string argument as first tuple member"
    )]
    PytzArgument,
    /// A zoneinfo path without a time zone identifier.
    #[error("couldn't parse timezone identifier from zoneinfo path{0}")]
    ZoneinfoPath(String),
    /// A string with an escape sequence that does not exist.
    #[error("invalid escape sequence char '{c}' in string \"{shown} [...]\" (possibly truncated)")]
    InvalidEscape {
        /// The character after the backslash.
        c: char,
        /// The beginning of the string.
        shown: String,
    },
    /// A matrix, vector or bit set, which Pyrolite introspects as a bean and fails on.
    #[error(
        "couldn't introspect javabean: {JAVA_ILLEGAL_ARGUMENT_EXCEPTION}: wrong number of arguments: 0 expected: {expected}"
    )]
    BeanIntrospection {
        /// The number of arguments of the constructor Pyrolite looks for.
        expected: u8,
    },
    /// A value of a kind Pyrolite cannot pickle.
    #[error("couldn't pickle object of type class {}", compat::value_class(*.0))]
    Unpicklable(ValueKind),
}

impl PickleError {
    /// `true` for the failures Warp 10 raises as runtime exceptions, `false` for the
    /// `IOException`s of a truncated input (which `PICKLE->` wraps).
    pub fn is_runtime(&self) -> bool {
        !matches!(
            self,
            PickleError::PrematureEof | PickleError::ExpectedMoreBytes
        )
    }
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
            return Err(PickleError::HashRecursion.into());
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
        Value::GeoShape(s) => identity(s),
        Value::Key(k) => identity(k),
        Value::PgpRing(r) => identity(r),
        Value::PgpKey(k) => identity(k),
        Value::Graphics(g) => identity(g),
        Value::Image(i) => identity(i),
        Value::Shape(s) => identity(s),
        Value::Font(f) => identity(f),
    })
}

/// Java `equals` as the memo compares objects: byte arrays and vectors by identity.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Bytes(x), Value::Bytes(y)) => Rc::ptr_eq(x, y),
        (Value::List(x), Value::List(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| values_equal(a, b))
        }
        (Value::Map(x), Value::Map(y)) => {
            let (x, y) = (x.borrow(), y.borrow());
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| values_equal(v, w)))
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
            return Err(PickleError::SaveRecursion.into());
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
            Some((memoized, index)) if values_equal(memoized, value) => {
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
                let mut ordered = HashOrderMap::new();
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
            Value::GeoShape(_)
            | Value::Key(_)
            | Value::Graphics(_)
            | Value::Image(_)
            | Value::Shape(_)
            | Value::Font(_) => {
                // A bean without a property (a key does not give its secret): the dictionary of
                // its class.
                let class = Value::string(compat::value_class(value.kind()));
                let bean = Value::map(IndexMap::from([(Value::string("__class__"), class)]));
                return self.save(&bean);
            }
            Value::Counter(_) => {
                return Err(BoundsError::StringIndex {
                    index: 1,
                    length: 0,
                }
                .into());
            }
            Value::BitSet(_) | Value::Matrix(_) | Value::Vector(_) => {
                let expected = if matches!(value, Value::Vector(_)) {
                    1
                } else {
                    2
                };
                return Err(PickleError::BeanIntrospection { expected }.into());
            }
            _ => {
                return Err(PickleError::Unpicklable(value.kind()).into());
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
        } else if (i64::from(i32::MIN)..=i64::from(i32::MAX)).contains(&v) {
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
            let mut map = HashOrderMap::new();
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

#[cfg(test)]
#[path = "pickle_tests.rs"]
mod pickle_tests;
