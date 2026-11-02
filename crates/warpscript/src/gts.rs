// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Geo Time Series: the Warp 10 `GeoTimeSerie` model — ticks, optional locations and
//! elevations, values of a single type, bucketization parameters and metadata — with the
//! `GTSHelper` primitives every GTS function builds on (`setValue`, `sort`, accessors…).

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use base64::Engine as _;
use indexmap::IndexMap;

use crate::format::{java_double_to_string, url_encode};
use crate::geoxp;
use crate::value::Value;

/// The elevation of a datapoint without elevation (`GeoTimeSerie.NO_ELEVATION`).
pub const NO_ELEVATION: i64 = i64::MIN;
/// The location of a datapoint without location (`GeoTimeSerie.NO_LOCATION`).
pub const NO_LOCATION: i64 = 0x0145_0145_0145_0145;

/// A Geo Time Series as held by the stack: shared and mutable, like the Java object.
pub type Gts = Rc<RefCell<GeoTimeSerie>>;

/// The type of the values of a series (`GeoTimeSerie.TYPE`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    /// No value was ever added.
    Undefined,
    /// `LONG` values.
    Long,
    /// `DOUBLE` values.
    Double,
    /// `BOOLEAN` values.
    Boolean,
    /// `STRING` values.
    String,
}

/// The values of a series, stored by type like the Java arrays (`longValues`,
/// `doubleValues`, `booleanValues`, `stringValues`).
#[derive(Clone, Debug, Default)]
pub enum Values {
    /// No value was ever added (`TYPE.UNDEFINED`).
    #[default]
    Undefined,
    /// `LONG` values.
    Long(Vec<i64>),
    /// `DOUBLE` values.
    Double(Vec<f64>),
    /// `BOOLEAN` values (a `BitSet` in Java).
    Boolean(Vec<bool>),
    /// `STRING` values.
    String(Vec<Rc<str>>),
}

impl Values {
    /// The type of the values.
    pub fn type_of(&self) -> Type {
        match self {
            Values::Undefined => Type::Undefined,
            Values::Long(_) => Type::Long,
            Values::Double(_) => Type::Double,
            Values::Boolean(_) => Type::Boolean,
            Values::String(_) => Type::String,
        }
    }

    /// The value at `idx` (`GTSHelper.valueAtIndex`): `null` when out of range.
    pub fn get(&self, idx: usize) -> Value {
        match self {
            Values::Undefined => Value::Null,
            Values::Long(v) => v.get(idx).map_or(Value::Null, |l| Value::Long(*l)),
            Values::Double(v) => v.get(idx).map_or(Value::Null, |d| Value::Double(*d)),
            Values::Boolean(v) => v.get(idx).map_or(Value::Null, |b| Value::Boolean(*b)),
            Values::String(v) => v
                .get(idx)
                .map_or(Value::Null, |s| Value::String(Rc::clone(s))),
        }
    }

    /// The value array, `None` for an untyped series (which has no datapoint).
    fn column(&mut self) -> Option<Column<'_>> {
        match self {
            Values::Undefined => None,
            Values::Long(v) => Some(Column::Long(v)),
            Values::Double(v) => Some(Column::Double(v)),
            Values::Boolean(v) => Some(Column::Boolean(v)),
            Values::String(v) => Some(Column::String(v)),
        }
    }

    fn reverse(&mut self) {
        if let Some(column) = self.column() {
            column.reverse();
        }
    }

    /// Keep the values at indexes `start..end` only.
    fn keep_range(&mut self, start: usize, end: usize) {
        if let Some(column) = self.column() {
            column.keep_range(start, end);
        }
    }

    /// Store `value` at `idx` (appending when `idx` is the length), converting it to the type
    /// of the series like `GTSHelper.setValue`; the first value fixes the type.
    fn store(&mut self, idx: usize, value: &Scalar) {
        match self {
            Values::Undefined => {
                *self = match value {
                    Scalar::Boolean(_) => Values::Boolean(Vec::new()),
                    Scalar::Long(_) => Values::Long(Vec::new()),
                    Scalar::Double(_) => Values::Double(Vec::new()),
                    Scalar::String(_) => Values::String(Vec::new()),
                };
                self.store(idx, value);
            }
            Values::Long(v) => put(v, idx, value.to_long()),
            Values::Double(v) => put(v, idx, value.to_double()),
            Values::Boolean(v) => put(v, idx, value.to_boolean()),
            Values::String(v) => put(v, idx, value.to_text()),
        }
    }
}

/// A mutable view on the value array of a typed series.
enum Column<'a> {
    Long(&'a mut Vec<i64>),
    Double(&'a mut Vec<f64>),
    Boolean(&'a mut Vec<bool>),
    String(&'a mut Vec<Rc<str>>),
}

impl Column<'_> {
    fn swap(&mut self, i: usize, j: usize) {
        match self {
            Column::Long(v) => v.swap(i, j),
            Column::Double(v) => v.swap(i, j),
            Column::Boolean(v) => v.swap(i, j),
            Column::String(v) => v.swap(i, j),
        }
    }

    fn reverse(self) {
        match self {
            Column::Long(v) => v.reverse(),
            Column::Double(v) => v.reverse(),
            Column::Boolean(v) => v.reverse(),
            Column::String(v) => v.reverse(),
        }
    }

    fn keep_range(self, start: usize, end: usize) {
        fn keep<T>(v: &mut Vec<T>, start: usize, end: usize) {
            v.truncate(end);
            v.drain(..start);
        }
        match self {
            Column::Long(v) => keep(v, start, end),
            Column::Double(v) => keep(v, start, end),
            Column::Boolean(v) => keep(v, start, end),
            Column::String(v) => keep(v, start, end),
        }
    }
}

/// A value a series can store.
enum Scalar {
    Boolean(bool),
    Long(i64),
    Double(f64),
    String(Rc<str>),
}

impl Scalar {
    /// The value stored into a `LONG` series.
    fn to_long(&self) -> i64 {
        match self {
            Scalar::Boolean(b) => i64::from(*b),
            Scalar::Long(l) => *l,
            Scalar::Double(d) => crate::value::double_to_long(*d),
            Scalar::String(s) => string_to_long(s),
        }
    }

    /// The value stored into a `DOUBLE` series.
    fn to_double(&self) -> f64 {
        match self {
            Scalar::Boolean(b) => f64::from(u8::from(*b)),
            Scalar::Long(l) => *l as f64,
            Scalar::Double(d) => *d,
            Scalar::String(s) => string_to_double(s),
        }
    }

    /// The value stored into a `BOOLEAN` series.
    fn to_boolean(&self) -> bool {
        match self {
            Scalar::Boolean(b) => *b,
            Scalar::Long(l) => *l != 0,
            Scalar::Double(d) => *d != 0.0,
            Scalar::String(s) => !s.is_empty(),
        }
    }

    /// The value stored into a `STRING` series.
    fn to_text(&self) -> Rc<str> {
        match self {
            Scalar::Boolean(b) => Rc::from(if *b { "T" } else { "F" }),
            Scalar::Long(l) => Rc::from(l.to_string()),
            Scalar::Double(d) => Rc::from(java_double_to_string(*d)),
            Scalar::String(s) => Rc::clone(s),
        }
    }
}

/// Write `item` at `idx`, appending when `idx` is the length.
fn put<T>(v: &mut Vec<T>, idx: usize, item: T) {
    if idx == v.len() {
        v.push(item);
    } else {
        v[idx] = item;
    }
}

/// A string stored into a `LONG` series: `Long.parseLong`, else the truncated
/// `Double.parseDouble`, else `0`.
fn string_to_long(s: &str) -> i64 {
    s.parse::<i64>().unwrap_or_else(|_| {
        crate::functions::operators::parse_java_double(s)
            .map(crate::value::double_to_long)
            .unwrap_or_default()
    })
}

/// A string stored into a `DOUBLE` series: `Double.parseDouble`, else `Long.parseLong`
/// (which `parseDouble` already accepts), else `0.0`.
fn string_to_double(s: &str) -> f64 {
    crate::functions::operators::parse_java_double(s).unwrap_or_default()
}

/// The metadata of a series (`io.warp10.continuum.store.thrift.data.Metadata`).
#[derive(Clone, Debug, Default)]
pub struct Metadata {
    /// The class name (`null` for a series that was never named).
    pub name: Option<String>,
    /// The labels, part of the identity of the series.
    pub labels: IndexMap<String, String>,
    /// The attributes, free-form metadata.
    pub attributes: IndexMap<String, String>,
    /// The class id (`classId`), computed by the datastore.
    pub class_id: i64,
    /// The labels id (`labelsId`), computed by the datastore.
    pub labels_id: i64,
    /// The last activity timestamp (`lastActivity`).
    pub last_activity: i64,
}

impl Metadata {
    /// `GTSHelper.relabel`: apply `changes` to the labels. A `null` key means the existing
    /// labels are discarded first; a `null` or empty value removes the label.
    pub fn relabel(&mut self, changes: &[(Option<&str>, Option<&str>)]) {
        apply_changes(&mut self.labels, changes);
    }

    /// `SETATTRIBUTES.updateAttribute`: apply `changes` to the attributes, with the same
    /// `null` key and `null` / empty value conventions as [`Metadata::relabel`].
    pub fn update_attributes(&mut self, changes: &[(Option<&str>, Option<&str>)]) {
        apply_changes(&mut self.attributes, changes);
    }
}

/// Apply label / attribute `changes` to `map`.
fn apply_changes(map: &mut IndexMap<String, String>, changes: &[(Option<&str>, Option<&str>)]) {
    if changes.iter().any(|(name, _)| name.is_none()) {
        map.clear();
    }
    for (name, value) in changes {
        match (name, value) {
            (Some(name), Some(value)) if !value.is_empty() => {
                map.insert((*name).to_owned(), (*value).to_owned());
            }
            (Some(name), _) => {
                map.shift_remove(*name);
            }
            (None, _) => {}
        }
    }
}

/// A Geo Time Series.
#[derive(Debug, Default)]
pub struct GeoTimeSerie {
    /// Name, labels and attributes.
    pub metadata: Metadata,
    /// Timestamps, in time units.
    pub ticks: Vec<i64>,
    /// GeoXP points, once a datapoint has a location (`NO_LOCATION` elsewhere).
    pub locations: Option<Vec<i64>>,
    /// Elevations, once a datapoint has one (`NO_ELEVATION` elsewhere).
    pub elevations: Option<Vec<i64>>,
    /// The values, one per tick.
    pub values: Values,
    /// Bucket span of a bucketized series (`0` otherwise).
    pub bucketspan: i64,
    /// Bucket count of a bucketized series (`0` otherwise).
    pub bucketcount: i64,
    /// Last bucket of a bucketized series (`0` otherwise).
    pub lastbucket: i64,
    /// Whether the ticks are known to be sorted (in the `reversed` direction).
    pub sorted: bool,
    /// Whether the sort order is descending.
    pub reversed: bool,
    /// Set by `RENAME` (the series no longer matches its original selector).
    pub renamed: bool,
}

impl Clone for GeoTimeSerie {
    /// `GeoTimeSerie.clone`: everything but the `renamed` flag.
    fn clone(&self) -> Self {
        GeoTimeSerie {
            metadata: self.metadata.clone(),
            ticks: self.ticks.clone(),
            locations: self.locations.clone(),
            elevations: self.elevations.clone(),
            values: self.values.clone(),
            bucketspan: self.bucketspan,
            bucketcount: self.bucketcount,
            lastbucket: self.lastbucket,
            sorted: self.sorted,
            reversed: self.reversed,
            renamed: false,
        }
    }
}

impl GeoTimeSerie {
    /// An empty, unnamed series (`new GeoTimeSerie()`).
    pub fn new() -> Self {
        GeoTimeSerie::default()
    }

    /// An empty, unnamed series with bucketization parameters
    /// (`new GeoTimeSerie(lastbucket, bucketcount, bucketspan, sizehint)`).
    pub fn with_buckets(lastbucket: i64, bucketcount: i64, bucketspan: i64) -> Self {
        GeoTimeSerie {
            bucketspan,
            bucketcount,
            lastbucket,
            ..GeoTimeSerie::default()
        }
    }

    /// Append datapoint `idx` of `source` (`GTSHelper.setValue` with the tick, location,
    /// elevation and value at that index).
    pub fn copy_datapoint(&mut self, source: &GeoTimeSerie, idx: usize) {
        self.set_value(
            source.tick_at(idx),
            source.location_at(idx),
            source.elevation_at(idx),
            &source.value_at(idx),
            false,
        );
    }

    /// Keep the datapoints at indexes `start..end` only.
    pub fn keep_range(&mut self, start: usize, end: usize) {
        self.ticks.truncate(end);
        self.ticks.drain(..start);
        if let Some(locations) = &mut self.locations {
            locations.truncate(end);
            locations.drain(..start);
        }
        if let Some(elevations) = &mut self.elevations {
            elevations.truncate(end);
            elevations.drain(..start);
        }
        self.values.keep_range(start, end);
    }

    /// `GTSHelper.subSerie(gts, start, stop, false)`: a new series with the datapoints whose
    /// tick lies in `[start, stop]`, carrying the name, labels and attributes but not the
    /// bucketization. The series is sorted (ascending) in place beforehand.
    #[must_use]
    pub fn subserie(&mut self, start: i64, stop: i64) -> GeoTimeSerie {
        let (first, end) = if self.ticks.is_empty() || start > stop {
            (0, 0)
        } else {
            self.sort(false);
            self.index_range(start, stop)
        };
        let mut sub = GeoTimeSerie {
            metadata: Metadata {
                name: self.metadata.name.clone(),
                labels: self.metadata.labels.clone(),
                attributes: self.metadata.attributes.clone(),
                ..Metadata::default()
            },
            ..self.clone()
        };
        sub.bucketspan = 0;
        sub.bucketcount = 0;
        sub.lastbucket = 0;
        sub.keep_range(first, end);
        if first == end {
            // An empty result is a fresh series in Warp 10, not flagged as sorted.
            sub.sorted = false;
            sub.reversed = false;
        }
        sub
    }

    /// The indexes `first..end` of the datapoints of a sorted series whose tick lies in
    /// `[start, stop]`, searched like `subSerie` does (binary searches then scans over the
    /// duplicates of the bounds); `(0, 0)` when there is none.
    fn index_range(&self, start: i64, stop: i64) -> (usize, usize) {
        let n = self.ticks.len() as i64;
        let mut lastidx = java_binary_search_range(&self.ticks, 0, n, stop);
        if lastidx == -1 {
            return (0, 0);
        } else if lastidx < 0 {
            lastidx = -lastidx - 2;
        } else if lastidx < n - 1 {
            let mut lastlastidx = lastidx + 1;
            while lastlastidx < n && self.ticks[lastlastidx as usize] == stop {
                lastlastidx += 1;
            }
            lastidx = lastlastidx - 1;
        }
        let mut firstidx = java_binary_search_range(&self.ticks, 0, lastidx + 1, start);
        if firstidx == -n - 1 {
            return (0, 0);
        } else if firstidx < 0 {
            firstidx = -firstidx - 1;
        } else if firstidx > 0 {
            let mut firstfirstidx = firstidx - 1;
            while firstfirstidx >= 0 && self.ticks[firstfirstidx as usize] == start {
                firstfirstidx -= 1;
            }
            firstidx = firstfirstidx + 1;
        }
        if firstidx > lastidx {
            return (0, 0);
        }
        (firstidx as usize, lastidx as usize + 1)
    }

    /// A shared handle on `self`.
    pub fn shared(self) -> Gts {
        Rc::new(RefCell::new(self))
    }

    /// Number of datapoints (`GeoTimeSerie.size`).
    pub fn size(&self) -> usize {
        self.ticks.len()
    }

    /// The type of the values.
    pub fn type_of(&self) -> Type {
        self.values.type_of()
    }

    /// `GTSHelper.isBucketized`: bucket span, count and last bucket all set.
    pub fn is_bucketized(&self) -> bool {
        self.bucketcount != 0 && self.bucketspan != 0 && self.lastbucket != 0
    }

    /// The name (`null` when the series was never named).
    pub fn name(&self) -> Option<&str> {
        self.metadata.name.as_deref()
    }

    /// `GTSHelper.tickAtIndex`: `Long.MIN_VALUE` when out of range.
    pub fn tick_at(&self, idx: usize) -> i64 {
        self.ticks.get(idx).copied().unwrap_or(i64::MIN)
    }

    /// `GTSHelper.locationAtIndex`: `NO_LOCATION` when out of range or without locations.
    pub fn location_at(&self, idx: usize) -> i64 {
        self.locations
            .as_ref()
            .and_then(|l| l.get(idx).copied())
            .unwrap_or(NO_LOCATION)
    }

    /// `GTSHelper.elevationAtIndex`: `NO_ELEVATION` when out of range or without elevations.
    pub fn elevation_at(&self, idx: usize) -> i64 {
        self.elevations
            .as_ref()
            .and_then(|e| e.get(idx).copied())
            .unwrap_or(NO_ELEVATION)
    }

    /// `GTSHelper.valueAtIndex`: `null` when out of range.
    pub fn value_at(&self, idx: usize) -> Value {
        self.values.get(idx)
    }

    /// `GTSHelper.setValue`: add a datapoint, or replace the value at `tick` when `overwrite`
    /// is set and the tick exists. `null` values and values of unsupported types are ignored;
    /// the first value fixes the type of the series and later ones are converted to it.
    pub fn set_value(
        &mut self,
        tick: i64,
        location: i64,
        elevation: i64,
        value: &Value,
        overwrite: bool,
    ) {
        let value = match value {
            Value::Boolean(b) => Scalar::Boolean(*b),
            Value::Long(l) => Scalar::Long(*l),
            Value::Double(d) => Scalar::Double(*d),
            Value::String(s) => Scalar::String(Rc::clone(s)),
            // `new String(bytes, ISO_8859_1)`
            Value::Bytes(bytes) => Scalar::String(Rc::from(
                bytes.iter().map(|b| *b as char).collect::<String>(),
            )),
            Value::Null => return,
            // Ignored, but Warp 10 still fixes the type of an untyped series to BOOLEAN.
            _ => {
                if self.type_of() == Type::Undefined {
                    self.values = Values::Boolean(Vec::new());
                }
                return;
            }
        };
        let n = self.size();
        let idx = if overwrite {
            self.index_of_tick(tick).unwrap_or(n)
        } else {
            n
        };
        if idx == n {
            // Keep the `sorted` flag when the new tick extends the order.
            match n {
                0 => {
                    self.sorted = true;
                    self.reversed = false;
                }
                1 => {
                    self.sorted = true;
                    self.reversed = self.ticks[0] > tick;
                }
                _ if self.sorted => {
                    let last = self.ticks[n - 1];
                    self.sorted = if self.reversed {
                        last >= tick
                    } else {
                        last <= tick
                    };
                }
                _ => {}
            }
        }
        if location != NO_LOCATION && self.locations.is_none() {
            self.locations = Some(vec![NO_LOCATION; n]);
        }
        if elevation != NO_ELEVATION && self.elevations.is_none() {
            self.elevations = Some(vec![NO_ELEVATION; n]);
        }
        self.values.store(idx, &value);
        put(&mut self.ticks, idx, tick);
        if let Some(locations) = &mut self.locations {
            put(locations, idx, location);
        }
        if let Some(elevations) = &mut self.elevations {
            put(elevations, idx, elevation);
        }
    }

    /// The index of the first datapoint at `tick`, by binary search when the series is sorted
    /// (`GTSHelper.binarySearchTick` with `FIRST`), by a scan otherwise.
    fn index_of_tick(&self, tick: i64) -> Option<usize> {
        if !self.sorted {
            return self.ticks.iter().position(|t| *t == tick);
        }
        let mut low = 0i64;
        let mut high = self.size() as i64 - 1;
        let mut found = None;
        while low <= high {
            let mid = (low + high) >> 1;
            let mut comp = self.ticks[mid as usize].cmp(&tick);
            if self.reversed {
                comp = comp.reverse();
            }
            match comp {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid - 1,
                std::cmp::Ordering::Equal => {
                    found = Some(mid as usize);
                    high = mid - 1;
                }
            }
        }
        found
    }

    /// `GTSHelper.sort`: sort the datapoints by tick, descending when `reversed`. A series
    /// already sorted the other way is simply reversed; the quicksort used otherwise is the
    /// one of Warp 10, so that datapoints sharing a tick end up in the same order.
    pub fn sort(&mut self, reversed: bool) {
        if self.sorted && self.reversed != reversed {
            self.ticks.reverse();
            if let Some(locations) = &mut self.locations {
                locations.reverse();
            }
            if let Some(elevations) = &mut self.elevations {
                elevations.reverse();
            }
            self.values.reverse();
            self.reversed = reversed;
            return;
        }
        if !self.sorted {
            self.quicksort(reversed);
        }
        self.sorted = true;
        self.reversed = reversed;
    }

    /// `GTSHelper.quicksort`: iterative Hoare partitioning around the middle tick. An untyped
    /// series (no datapoint) and a typed one left empty by ignored values have nothing to
    /// sort.
    fn quicksort(&mut self, reversed: bool) {
        let Some(mut values) = self.values.column() else {
            return;
        };
        let ticks = &mut self.ticks;
        if ticks.is_empty() {
            return;
        }
        let mut ranges = vec![(0i64, ticks.len() as i64 - 1)];
        let mut next = 0;
        while next < ranges.len() {
            let (low, high) = ranges[next];
            next += 1;
            let (mut i, mut j) = (low, high);
            let pivot = ticks[(low + (high - low) / 2) as usize];
            while i <= j {
                while before(ticks[i as usize], pivot, reversed) {
                    i += 1;
                }
                while before(pivot, ticks[j as usize], reversed) {
                    j -= 1;
                }
                if i <= j {
                    if i != j {
                        let (i, j) = (i as usize, j as usize);
                        ticks.swap(i, j);
                        if let Some(locations) = &mut self.locations {
                            locations.swap(i, j);
                        }
                        if let Some(elevations) = &mut self.elevations {
                            elevations.swap(i, j);
                        }
                        values.swap(i, j);
                    }
                    i += 1;
                    j -= 1;
                }
            }
            if low < j {
                ranges.push((low, j));
            }
            if i < high {
                ranges.push((i, high));
            }
        }
    }

    /// `GeoTimeSerie.cloneEmpty`: the metadata and bucketization parameters without any
    /// datapoint (nor type).
    #[must_use]
    pub fn clone_empty(&self) -> GeoTimeSerie {
        GeoTimeSerie {
            metadata: self.metadata.clone(),
            bucketspan: self.bucketspan,
            bucketcount: self.bucketcount,
            lastbucket: self.lastbucket,
            ..GeoTimeSerie::default()
        }
    }

    /// `GTSHelper.firsttick`: the first bucket of a bucketized series, else the smallest tick
    /// (`Long.MAX_VALUE` for an empty series).
    pub fn first_tick(&self) -> i64 {
        if self.is_bucketized() {
            return self
                .lastbucket
                .wrapping_sub((self.bucketcount - 1).wrapping_mul(self.bucketspan));
        }
        if self.sorted && !self.ticks.is_empty() {
            return if self.reversed {
                self.ticks[self.size() - 1]
            } else {
                self.ticks[0]
            };
        }
        self.ticks.iter().copied().min().unwrap_or(i64::MAX)
    }

    /// `GTSHelper.lasttick`: the last bucket of a bucketized series, else the greatest tick
    /// (`Long.MIN_VALUE` for an empty series).
    pub fn last_tick(&self) -> i64 {
        if self.is_bucketized() {
            return self.lastbucket;
        }
        if self.sorted && !self.ticks.is_empty() {
            return if self.reversed {
                self.ticks[0]
            } else {
                self.ticks[self.size() - 1]
            };
        }
        self.ticks.iter().copied().max().unwrap_or(i64::MIN)
    }

    /// `GTSHelper.rename`: set the name, or append to it when `name` starts with `+`.
    pub fn rename(&mut self, name: &str) {
        let new_name = match name.strip_prefix('+') {
            // `null + suffix` is `"null" + suffix` in Java.
            Some(suffix) => format!(
                "{}{suffix}",
                self.metadata.name.as_deref().unwrap_or("null")
            ),
            None => name.to_owned(),
        };
        self.metadata.name = Some(new_name);
        self.renamed = true;
    }
}

impl GeoTimeSerie {
    /// `GeoTimeSerie.dump`: the series in the GTS input format, one
    /// `tick/lat:lon/elev value` line per datapoint (CR LF terminated), the first one carrying
    /// ` name{labels}{attributes} ` and the next ones starting with `=`.
    pub fn dump(&self) -> String {
        let encoded_map = |map: &IndexMap<String, String>| {
            let entries: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
                .collect();
            format!("{{{}}}", entries.join(","))
        };
        let class_and_labels = format!(
            " {}{}{} ",
            url_encode(self.name().unwrap_or_default()),
            encoded_map(&self.metadata.labels),
            encoded_map(&self.metadata.attributes)
        );
        let mut out = String::new();
        for i in 0..self.size() {
            if i > 0 {
                out.push('=');
            }
            out.push_str(&self.ticks[i].to_string());
            out.push('/');
            let location = self.location_at(i);
            if location != NO_LOCATION {
                let (lat, lon) = geoxp::from_geoxp_point(location);
                out.push_str(&java_double_to_string(lat));
                out.push(':');
                out.push_str(&java_double_to_string(lon));
            }
            out.push('/');
            let elevation = self.elevation_at(i);
            if elevation != NO_ELEVATION {
                out.push_str(&elevation.to_string());
            }
            out.push_str(if i == 0 { &class_and_labels } else { " " });
            encode_value(&mut out, &self.value_at(i));
            out.push_str("\r\n");
        }
        out
    }
}

/// `Arrays.binarySearch`: the index of `key` in the sorted `ticks` (whichever of equal ticks
/// Java's loop lands on), `None` when absent.
pub fn java_binary_search(ticks: &[i64], key: i64) -> Option<usize> {
    usize::try_from(java_binary_search_range(ticks, 0, ticks.len() as i64, key)).ok()
}

/// `Arrays.binarySearch(ticks, from, to, key)`: the index of `key` in `ticks[from..to]`, or
/// `-(insertion point) - 1` when absent.
pub fn java_binary_search_range(ticks: &[i64], from: i64, to: i64, key: i64) -> i64 {
    let mut low = from;
    let mut high = to - 1;
    while low <= high {
        let mid = (low + high) >> 1;
        match ticks[mid as usize].cmp(&key) {
            std::cmp::Ordering::Less => low = mid + 1,
            std::cmp::Ordering::Greater => high = mid - 1,
            std::cmp::Ordering::Equal => return mid,
        }
    }
    -(low + 1)
}

/// Strict ordering of two ticks in the requested direction.
fn before(a: i64, b: i64, reversed: bool) -> bool {
    if reversed { a > b } else { a < b }
}

/// `GTSHelper.encodeValue`: the text form of a value in the GTS input format.
pub fn encode_value(out: &mut String, value: &Value) {
    match value {
        Value::Long(l) => out.push_str(&l.to_string()),
        Value::Double(d) => out.push_str(&java_double_to_string(*d)),
        Value::Boolean(b) => out.push(if *b { 'T' } else { 'F' }),
        Value::String(s) => {
            out.push('\'');
            out.push_str(&url_encode(s));
            out.push('\'');
        }
        Value::Bytes(bytes) => {
            out.push_str("b64:");
            out.push_str(&base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes));
        }
        _ => {}
    }
}

impl fmt::Display for GeoTimeSerie {
    /// `GeoTimeSerie.toString`: the name and labels, then one `=tick/lat:lon/elev value`
    /// line per datapoint.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        if let Some(name) = &self.metadata.name {
            out.push_str(&url_encode(name));
        }
        out.push('{');
        let labels: Vec<String> = self
            .metadata
            .labels
            .iter()
            .map(|(k, v)| format!("{}={}", url_encode(k), url_encode(v)))
            .collect();
        out.push_str(&labels.join(","));
        out.push_str("}\n");
        for i in 0..self.size() {
            out.push('=');
            out.push_str(&self.ticks[i].to_string());
            out.push('/');
            let location = self.location_at(i);
            if location != NO_LOCATION {
                let (lat, lon) = geoxp::from_geoxp_point(location);
                out.push_str(&java_double_to_string(lat));
                out.push(':');
                out.push_str(&java_double_to_string(lon));
            }
            out.push('/');
            let elevation = self.elevation_at(i);
            if elevation != NO_ELEVATION {
                out.push_str(&elevation.to_string());
            }
            out.push(' ');
            encode_value(&mut out, &self.value_at(i));
            out.push('\n');
        }
        f.write_str(&out)
    }
}
