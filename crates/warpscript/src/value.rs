// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The dynamic value type held on the WarpScript stack.
//!
//! Warp 10 stacks hold arbitrary Java objects; [`Value`] is the closed set of types the Rust
//! runtime knows. Containers (`LIST`, `MAP`, `SET`, GTS…) have **reference semantics** exactly
//! like in Java: `DUP` yields another handle to the same list, `CLONE` deep-copies.
//!
//! Equality and hashing follow Java's `equals` / `hashCode` contracts (a `LONG` never equals a
//! `DOUBLE`, `NaN` equals `NaN` as a map key…). The numeric comparison used by `==`, `<`… is a
//! separate function in the function library.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};

use crate::bitset::BitSet;
use crate::encoder::GtsEncoder;
use crate::format::double_to_text;
use crate::geoshape::GeoShape;
use crate::gts::Gts;
use crate::macros::Macro;

/// A shared, mutable list (`java.util.List`).
pub type List = Rc<RefCell<Vec<Value>>>;
/// A shared, mutable insertion-ordered map (`java.util.LinkedHashMap`).
pub type Map = Rc<RefCell<IndexMap<Value, Value>>>;
/// A shared, mutable insertion-ordered set (`java.util.LinkedHashSet`).
pub type Set = Rc<RefCell<IndexSet<Value>>>;

/// A function object that lives on the stack (mapper, reducer, filter, operator…), as opposed
/// to a stack function which is executed immediately when encountered.
pub trait FunctionObject {
    /// WarpScript name of the object, e.g. `mapper.sum`.
    fn name(&self) -> &str;

    /// Type name reported by `TYPEOF` (`AGGREGATOR`, `FILTER`, `OPERATOR`, `FILLER`…).
    fn type_name(&self) -> &'static str;

    /// WarpScript snippet that recreates this object (Java `toString()`), e.g. `3 mapper.add`.
    /// The default is the reference snapshot of `NamedWarpScriptFunction`: `'name' FUNCREF`.
    fn snapshot(&self) -> String {
        format!("'{}' FUNCREF", crate::format::url_encode(self.name()))
    }

    /// The concrete object, for the functions that consume a specific kind of object (a
    /// `MATCHER` fed to `MATCH`…). Objects that are only ever applied keep the default `None`.
    fn as_any(&self) -> Option<&dyn Any> {
        None
    }

    /// The object as a filter (`WarpScriptFilterFunction`), for `FILTER`.
    fn as_filter(&self) -> Option<&dyn crate::filters::Filter> {
        None
    }

    /// The object as an aggregator (`WarpScriptAggregatorFunction`), for the `BUCKETIZE`,
    /// `MAP` and `REDUCE` frameworks.
    fn as_aggregator(&self) -> Option<&dyn crate::aggregators::Aggregator> {
        None
    }
}

/// A saved execution context (`SAVE` / `RESTORE`).
#[derive(Clone)]
pub struct Context {
    /// Symbol table at the time of `SAVE`.
    pub symbols: IndexMap<String, Value>,
    /// Registers at the time of `SAVE`.
    pub registers: Vec<Value>,
    /// Functions redefined with `DEF` (or undefined with `UNDEF`) at the time of `SAVE`.
    pub defined: IndexMap<String, Option<Rc<Macro>>>,
}

/// A WarpScript value.
#[derive(Clone)]
pub enum Value {
    /// Java `null`.
    Null,
    /// `BOOLEAN`.
    Boolean(bool),
    /// `LONG` (Java `long`, 64-bit two's complement with wrapping arithmetic).
    Long(i64),
    /// `DOUBLE` (Java `double`).
    Double(f64),
    /// `BIGDECIMAL` (Java `java.math.BigDecimal`).
    BigDecimal(Rc<crate::bigdecimal::BigDecimal>),
    /// `MATRIX` (Commons Math `RealMatrix`).
    Matrix(Rc<crate::functions::matrix::Matrix>),
    /// `VECTOR` (Commons Math `RealVector`).
    Vector(Rc<[f64]>),
    /// `STRING` (immutable, shared).
    String(Rc<str>),
    /// `BYTES` (Java `byte[]`).
    Bytes(Rc<[u8]>),
    /// `BITSET` (`java.util.BitSet`, immutable here).
    BitSet(Rc<BitSet>),
    /// `LIST`.
    List(List),
    /// `MAP`.
    Map(Map),
    /// `SET`.
    Set(Set),
    /// `MACRO`.
    Macro(Rc<Macro>),
    /// A function object (mapper, reducer, filter, operator…).
    Function(Rc<dyn FunctionObject>),
    /// The `MARK` pushed by `[`, `{`, `(` and `MARK`.
    Mark,
    /// A saved execution context (`SAVE`).
    Context(Rc<Context>),
    /// A mutable counter (`COUNTER`, a `java.util.concurrent.atomic.AtomicLong`).
    Counter(Rc<Cell<i64>>),
    /// `GTS`: a Geo Time Series, shared and mutable like the Java object.
    Gts(Gts),
    /// `GTSENCODER`: an encoder of datapoints, shared and mutable like the Java object.
    Encoder(Encoder),
    /// `GEOSHAPE`: a set of geocells (immutable).
    GeoShape(Rc<GeoShape>),
    /// An elliptic curve key (`ECPRIVATE`, `ECPUBLIC`: a `java.security.Key` in Warp 10).
    Key(Rc<crate::ec::Key>),
}

/// The kind of a [`Value`], without its content. Failures carry it instead of the value; the
/// Java class a script reads in their message is named in `error::compat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// `null`.
    Null,
    /// `BOOLEAN`.
    Boolean,
    /// `LONG`.
    Long,
    /// `DOUBLE`.
    Double,
    /// `BIGDECIMAL`.
    BigDecimal,
    /// `MATRIX`.
    Matrix,
    /// `VECTOR`.
    Vector,
    /// `STRING`.
    String,
    /// `BYTES`.
    Bytes,
    /// `BITSET`.
    BitSet,
    /// `LIST`.
    List,
    /// `MAP`.
    Map,
    /// `SET`.
    Set,
    /// `MACRO`.
    Macro,
    /// A function object.
    Function,
    /// `MARK`.
    Mark,
    /// A saved execution context.
    Context,
    /// `COUNTER`.
    Counter,
    /// `GTS`.
    Gts,
    /// `GTSENCODER`.
    Encoder,
    /// `GEOSHAPE`.
    GeoShape,
    /// An elliptic curve key.
    Key,
}

/// A `GTSENCODER` as held by the stack.
pub type Encoder = Rc<RefCell<GtsEncoder>>;

impl Value {
    /// Build a string value.
    pub fn string(s: impl AsRef<str>) -> Self {
        Value::String(Rc::from(s.as_ref()))
    }

    /// Build a list value from a vector.
    pub fn list(items: Vec<Value>) -> Self {
        Value::List(Rc::new(RefCell::new(items)))
    }

    /// Build a map value.
    pub fn map(entries: IndexMap<Value, Value>) -> Self {
        Value::Map(Rc::new(RefCell::new(entries)))
    }

    /// Build a set value.
    pub fn set(items: IndexSet<Value>) -> Self {
        Value::Set(Rc::new(RefCell::new(items)))
    }

    /// Build a byte array value.
    pub fn bytes(bytes: impl AsRef<[u8]>) -> Self {
        Value::Bytes(Rc::from(bytes.as_ref()))
    }

    /// The type name reported by `TYPEOF`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "NULL",
            Value::Boolean(_) => "BOOLEAN",
            Value::Long(_) => "LONG",
            Value::Double(_) => "DOUBLE",
            Value::BigDecimal(_) => "BIGDECIMAL",
            Value::Matrix(_) => "MATRIX",
            Value::Vector(_) => "VECTOR",
            Value::String(_) => "STRING",
            Value::Bytes(_) => "BYTES",
            Value::BitSet(_) => "BITSET",
            Value::List(_) => "LIST",
            Value::Map(_) => "MAP",
            Value::Set(_) => "SET",
            Value::Macro(_) => "MACRO",
            Value::Function(f) => f.type_name(),
            Value::Mark => "MARK",
            Value::Context(_) => "CONTEXT",
            Value::Counter(_) => "COUNTER",
            Value::Gts(_) => "GTS",
            Value::Encoder(_) => "GTSENCODER",
            Value::GeoShape(_) => "GEOSHAPE",
            Value::Key(_) => "KEY",
        }
    }

    /// What the value is, for the failures that name it (a failed cast, an unsupported type).
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::Null => ValueKind::Null,
            Value::Boolean(_) => ValueKind::Boolean,
            Value::Long(_) => ValueKind::Long,
            Value::Double(_) => ValueKind::Double,
            Value::BigDecimal(_) => ValueKind::BigDecimal,
            Value::Matrix(_) => ValueKind::Matrix,
            Value::Vector(_) => ValueKind::Vector,
            Value::String(_) => ValueKind::String,
            Value::Bytes(_) => ValueKind::Bytes,
            Value::BitSet(_) => ValueKind::BitSet,
            Value::List(_) => ValueKind::List,
            Value::Map(_) => ValueKind::Map,
            Value::Set(_) => ValueKind::Set,
            Value::Macro(_) => ValueKind::Macro,
            Value::Function(_) => ValueKind::Function,
            Value::Mark => ValueKind::Mark,
            Value::Context(_) => ValueKind::Context,
            Value::Counter(_) => ValueKind::Counter,
            Value::Gts(_) => ValueKind::Gts,
            Value::Encoder(_) => ValueKind::Encoder,
            Value::GeoShape(_) => ValueKind::GeoShape,
            Value::Key(_) => ValueKind::Key,
        }
    }

    /// `true` for `LONG`, `DOUBLE` and `BIGDECIMAL` (Java `Number`).
    pub fn is_number(&self) -> bool {
        matches!(
            self,
            Value::Long(_) | Value::Double(_) | Value::BigDecimal(_)
        )
    }

    /// `true` for `null`.
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// The value as a `f64` when it is a number (Java `Number.doubleValue()`).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Long(l) => Some(*l as f64),
            Value::Double(d) => Some(*d),
            Value::BigDecimal(bd) => Some(bd.double_value()),
            _ => None,
        }
    }

    /// The value as an `i64` when it is a number (Java `Number.longValue()`, saturating for
    /// doubles like the JVM's `d2l` conversion).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Long(l) => Some(*l),
            Value::Double(d) => Some(double_to_long(*d)),
            Value::BigDecimal(bd) => Some(bd.long_value()),
            _ => None,
        }
    }

    /// The string content when the value is a `STRING`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// The boolean when the value is a `BOOLEAN`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// `true` when both values are the same object (Java `==` on references), used by
    /// identity-based equality of macros, functions and contexts.
    pub fn same_object(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::List(a), Value::List(b)) => Rc::ptr_eq(a, b),
            (Value::Map(a), Value::Map(b)) => Rc::ptr_eq(a, b),
            (Value::Set(a), Value::Set(b)) => Rc::ptr_eq(a, b),
            (Value::Macro(a), Value::Macro(b)) => Rc::ptr_eq(a, b),
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            (Value::Context(a), Value::Context(b)) => Rc::ptr_eq(a, b),
            (Value::Counter(a), Value::Counter(b)) => Rc::ptr_eq(a, b),
            (Value::Gts(a), Value::Gts(b)) => Rc::ptr_eq(a, b),
            (Value::Encoder(a), Value::Encoder(b)) => Rc::ptr_eq(a, b),
            (Value::GeoShape(a), Value::GeoShape(b)) => Rc::ptr_eq(a, b),
            (Value::Key(a), Value::Key(b)) => Rc::ptr_eq(a, b),
            (Value::String(a), Value::String(b)) => Rc::ptr_eq(a, b),
            (Value::Bytes(a), Value::Bytes(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

thread_local! {
    /// The collections made read-only by `IMMUTABLE` (Java's `Collections.unmodifiable*`
    /// views). Weak handles so that a dropped collection never pins memory and a recycled
    /// address is never mistaken for a frozen collection.
    static FROZEN: RefCell<Vec<std::rc::Weak<dyn Any>>> = const { RefCell::new(Vec::new()) };
}

/// Mark the collection `rc` as read-only (see [`ensure_mutable`]).
pub fn freeze<T: 'static>(rc: &Rc<T>) {
    let handle: Rc<dyn Any> = rc.clone();
    FROZEN.with(|frozen| {
        let mut frozen = frozen.borrow_mut();
        frozen.retain(|weak| weak.strong_count() > 0);
        frozen.push(Rc::downgrade(&handle));
    });
}

/// `true` when the collection `rc` was made read-only by [`freeze`].
pub fn is_frozen<T: 'static>(rc: &Rc<T>) -> bool {
    let address = Rc::as_ptr(rc).cast::<()>();
    FROZEN.with(|frozen| {
        frozen
            .borrow()
            .iter()
            .any(|weak| weak.strong_count() > 0 && weak.as_ptr().cast::<()>() == address)
    })
}

/// The `UnsupportedOperationException` a Java unmodifiable collection throws when modified.
pub fn ensure_mutable<T: 'static>(rc: &Rc<T>) -> crate::error::Result<()> {
    if is_frozen(rc) {
        Err(crate::error::Error::Immutable)
    } else {
        Ok(())
    }
}

/// Java's `(long) d` conversion: `NaN` → 0, saturating at the `long` bounds.
pub fn double_to_long(d: f64) -> i64 {
    // Rust's `as` has exactly the JVM `d2l` semantics (NaN → 0, saturation).
    d as i64
}

/// Java `Double.equals`: compares the canonical bit patterns, so `NaN` equals `NaN` and
/// `0.0` differs from `-0.0`.
fn double_bits(d: f64) -> u64 {
    if d.is_nan() {
        f64::NAN.to_bits()
    } else {
        d.to_bits()
    }
}

impl PartialEq for Value {
    /// Java `equals` semantics.
    fn eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) | (Value::Mark, Value::Mark) => true,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            (Value::Long(a), Value::Long(b)) => a == b,
            (Value::Double(a), Value::Double(b)) => double_bits(*a) == double_bits(*b),
            (Value::BigDecimal(a), Value::BigDecimal(b)) => a == b,
            (Value::Matrix(a), Value::Matrix(b)) => Rc::ptr_eq(a, b) || a.same_entries(b),
            (Value::Vector(a), Value::Vector(b)) => {
                Rc::ptr_eq(a, b) || crate::functions::matrix::same_vector(a, b)
            }
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::BitSet(a), Value::BitSet(b)) => a == b,
            (Value::List(a), Value::List(b)) => Rc::ptr_eq(a, b) || *a.borrow() == *b.borrow(),
            (Value::Map(a), Value::Map(b)) => Rc::ptr_eq(a, b) || *a.borrow() == *b.borrow(),
            (Value::Set(a), Value::Set(b)) => Rc::ptr_eq(a, b) || *a.borrow() == *b.borrow(),
            (Value::Macro(_), Value::Macro(_))
            | (Value::Function(_), Value::Function(_))
            | (Value::Context(_), Value::Context(_))
            | (Value::Counter(_), Value::Counter(_))
            | (Value::Gts(_), Value::Gts(_))
            | (Value::Encoder(_), Value::Encoder(_))
            | (Value::Key(_), Value::Key(_)) => self.same_object(other),
            // `GeoXPShape` has no `equals`; `EQ` compares the geocells.
            (Value::GeoShape(a), Value::GeoShape(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Hash for Value {
    /// Consistent with [`PartialEq`]: equal values hash equally.
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::Null => 0u8.hash(state),
            Value::Boolean(b) => b.hash(state),
            Value::Long(l) => l.hash(state),
            Value::Double(d) => double_bits(*d).hash(state),
            Value::BigDecimal(bd) => bd.hash(state),
            // Consistent with the `==` comparison of the entries: the dimensions only.
            Value::Matrix(m) => (m.rows, m.cols).hash(state),
            Value::Vector(v) => v.len().hash(state),
            Value::String(s) => s.hash(state),
            Value::Bytes(b) => b.hash(state),
            Value::BitSet(b) => b.hash(state),
            Value::List(l) => {
                for item in l.borrow().iter() {
                    item.hash(state);
                }
            }
            Value::Map(m) => {
                for (k, v) in m.borrow().iter() {
                    k.hash(state);
                    v.hash(state);
                }
            }
            Value::Set(s) => {
                // Order independent, like `AbstractSet.hashCode`.
                let mut sum = 0u64;
                for item in s.borrow().iter() {
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    item.hash(&mut h);
                    sum = sum.wrapping_add(h.finish());
                }
                sum.hash(state);
            }
            Value::Macro(m) => (Rc::as_ptr(m) as usize).hash(state),
            Value::Function(f) => (Rc::as_ptr(f).cast::<()>() as usize).hash(state),
            Value::Mark => 1u8.hash(state),
            Value::Context(c) => (Rc::as_ptr(c) as usize).hash(state),
            Value::Counter(c) => (Rc::as_ptr(c) as usize).hash(state),
            Value::Gts(g) => (Rc::as_ptr(g) as usize).hash(state),
            Value::Encoder(e) => (Rc::as_ptr(e) as usize).hash(state),
            // `GeoXPShape` overrides `equals` but not `hashCode`: sets tell two shapes apart.
            Value::GeoShape(s) => (Rc::as_ptr(s) as usize).hash(state),
            Value::Key(k) => (Rc::as_ptr(k) as usize).hash(state),
        }
    }
}

impl fmt::Display for Value {
    /// Java `String.valueOf(o)` / `toString()` semantics.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Long(l) => write!(f, "{l}"),
            Value::Double(d) => f.write_str(&double_to_text(*d)),
            Value::BigDecimal(bd) => write!(f, "{bd}"),
            Value::Matrix(m) => f.write_str(&m.matrix_text()),
            Value::Vector(v) => f.write_str(&crate::functions::matrix::vector_string(v)),
            Value::String(s) => f.write_str(s),
            Value::Bytes(b) => write!(f, "[B@{:x}", b.len()),
            Value::BitSet(b) => write!(f, "{b}"),
            Value::List(l) => {
                f.write_str("[")?;
                for (idx, item) in l.borrow().iter().enumerate() {
                    if idx > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Value::Set(s) => {
                f.write_str("[")?;
                for (idx, item) in s.borrow().iter().enumerate() {
                    if idx > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Value::Map(m) => {
                f.write_str("{")?;
                for (idx, (k, v)) in m.borrow().iter().enumerate() {
                    if idx > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{k}={v}")?;
                }
                f.write_str("}")
            }
            Value::Macro(m) => f.write_str(&m.snapshot(true)),
            Value::Function(func) => f.write_str(&func.snapshot()),
            Value::Mark => f.write_str("MARK"),
            Value::Context(_) => f.write_str("CONTEXT"),
            Value::Counter(c) => write!(f, "{}", c.get()),
            Value::Gts(g) => write!(f, "{}", g.borrow()),
            // `Object.toString`, with the size standing for the identity hash.
            Value::Encoder(e) => write!(
                f,
                "{}@{:x}",
                crate::error::compat::value_class(ValueKind::Encoder),
                e.borrow().size()
            ),
            Value::GeoShape(s) => write!(
                f,
                "{}@{:x}",
                crate::error::compat::value_class(ValueKind::GeoShape),
                s.cells().len()
            ),
            // The identity hash is not shown: the object holds a secret.
            Value::Key(_) => write!(f, "{}@0", crate::error::compat::value_class(ValueKind::Key)),
        }
    }
}

impl fmt::Debug for Value {
    /// Type-tagged rendering used in test failures: `LONG(3)`, `STRING("a")`…
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "STRING({s:?})"),
            Value::Double(d) => write!(f, "DOUBLE({})", double_to_text(*d)),
            other => write!(f, "{}({other})", other.type_name()),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Boolean(b)
    }
}

impl From<i64> for Value {
    fn from(l: i64) -> Self {
        Value::Long(l)
    }
}

impl From<f64> for Value {
    fn from(d: f64) -> Self {
        Value::Double(d)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::string(s)
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(Rc::from(s))
    }
}

impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Self {
        Value::list(items)
    }
}

impl From<Rc<Macro>> for Value {
    fn from(m: Rc<Macro>) -> Self {
        Value::Macro(m)
    }
}
