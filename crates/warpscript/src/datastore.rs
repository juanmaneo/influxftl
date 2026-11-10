// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The datastore of the runtime: what `FETCH`, `FIND`, `UPDATE`, `META` and `DELETE` read and
//! write. Warp 10 has its own storage (Directory and Store clients); here the host plugs one
//! in through the [`Datastore`] trait (the `influxftl` binary connects InfluxDB 3), and a
//! [`MemoryDatastore`] serves as the reference implementation of the semantics the functions
//! expect, and for tests.
//!
//! Times are in platform time units (microseconds), as everywhere in the runtime.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use indexmap::IndexMap;

use crate::error::datastore::{DatastoreError, TimeRangeError};
use crate::error::{Error, Result};
use crate::format::{parse_long, url_encode};
use crate::functions::time::{parse_duration, parse_timestamp};
use crate::gts::{GeoTimeSerie, Metadata, Type, Values};
use crate::selector::MetadataSelectorMatcher;
use crate::value::Value;

/// A class and labels selector (`class{label=value,label~regexp}`): the class selector and the
/// labels selectors, each `=value`, `~regexp` or a plain value (exact).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selector {
    /// The class selector.
    pub class: String,
    /// The selectors of the labels, by label name.
    pub labels: IndexMap<String, String>,
}

impl Selector {
    /// The selector in the text form `MetadataSelectorMatcher` reads.
    pub fn text(&self) -> String {
        let labels: Vec<String> = self
            .labels
            .iter()
            .map(|(name, selector)| {
                let (separator, value) = match selector.chars().next() {
                    Some(prefix @ ('=' | '~')) => (prefix, &selector[1..]),
                    _ => ('=', selector.as_str()),
                };
                format!("{}{separator}{}", url_encode(name), url_encode(value))
            })
            .collect();
        format!("{}{{{}}}", url_encode(&self.class), labels.join(","))
    }

    /// Whether the series of a metadata is selected.
    ///
    /// # Errors
    ///
    /// A selector that is not valid (a regular expression that does not compile…).
    pub fn matches(&self, metadata: &Metadata) -> Result<bool> {
        MetadataSelectorMatcher::new(&self.text())?.matches(metadata)
    }
}

/// What `FETCH` asks of the datastore.
#[derive(Clone, Debug, PartialEq)]
pub struct FetchRequest {
    /// The series to fetch: those selected by any of the selectors.
    pub selectors: Vec<Selector>,
    /// The oldest tick to fetch (inclusive).
    pub start: i64,
    /// The most recent tick to fetch (inclusive).
    pub end: i64,
    /// The number of datapoints to fetch: the most recent ones of the range.
    pub count: Option<i64>,
    /// Only the datapoints of this type.
    pub value_type: Option<Type>,
    /// The number of datapoints to fetch before `start`.
    pub boundary_pre: i64,
    /// The number of datapoints to fetch after `end`.
    pub boundary_post: i64,
    /// Whether to return the series that have no datapoint.
    pub keep_empty: bool,
    /// The number of datapoints to skip from the most recent one of the range.
    pub skip: i64,
    /// Only one datapoint in `step` is fetched (at least 1).
    pub step: i64,
    /// The least time between two fetched datapoints.
    pub timestep: Option<i64>,
    /// The number of selected series to skip.
    pub series_skip: i64,
    /// The number of selected series to fetch (after those skipped).
    pub series_count: i64,
}

/// What `DELETE` asks of the datastore.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteRequest {
    /// The series whose datapoints are deleted.
    pub selectors: Vec<Selector>,
    /// The ticks to delete, first and last (inclusive); all the datapoints, and the series
    /// themselves, without.
    pub range: Option<(i64, i64)>,
}

/// The storage the datastore functions work on. A datastore is shared by the scripts that run
/// at the same time, hence `Send` and `Sync`; the series it returns belong to the script.
pub trait Datastore: std::fmt::Debug + Send + Sync {
    /// The metadata of the series selected by any of the selectors, sorted by class then labels.
    ///
    /// # Errors
    ///
    /// A selector that is not valid, or a datastore that fails.
    fn find(&self, selectors: &[Selector]) -> Result<Vec<Metadata>>;

    /// The datapoints of a request, a series for each selected series that has some (or all of
    /// them for `keep_empty`), the most recent datapoint first.
    ///
    /// # Errors
    ///
    /// A selector that is not valid, or a datastore that fails.
    fn fetch(&self, request: &FetchRequest) -> Result<Vec<GeoTimeSerie>>;

    /// Write the datapoints of series, which replace those at the same ticks.
    ///
    /// # Errors
    ///
    /// A series without a class name, or a datastore that fails.
    fn update(&self, series: &[GeoTimeSerie]) -> Result<()>;

    /// Set the attributes of series, which are created when they do not exist: they replace
    /// the attributes the series had, or (`delta`) are changes to them, an empty value
    /// removing an attribute.
    ///
    /// # Errors
    ///
    /// A series without a class name, or a datastore that fails.
    fn update_metadata(&self, metadata: &[Metadata], delta: bool) -> Result<()>;

    /// Delete datapoints (and the series themselves when the range is all of time), and tell
    /// which series were selected.
    ///
    /// # Errors
    ///
    /// A selector that is not valid, or a datastore that fails.
    fn delete(&self, request: &DeleteRequest) -> Result<Vec<Metadata>>;
}

/// A value of a datapoint, owned so that the datastore can be shared.
#[derive(Clone, Debug, PartialEq)]
enum Scalar {
    Long(i64),
    Double(f64),
    Boolean(bool),
    String(String),
}

impl Scalar {
    fn value(&self) -> Value {
        match self {
            Scalar::Long(long) => Value::Long(*long),
            Scalar::Double(double) => Value::Double(*double),
            Scalar::Boolean(boolean) => Value::Boolean(*boolean),
            Scalar::String(text) => Value::from(text.as_str()),
        }
    }

    fn value_type(&self) -> Type {
        match self {
            Scalar::Long(_) => Type::Long,
            Scalar::Double(_) => Type::Double,
            Scalar::Boolean(_) => Type::Boolean,
            Scalar::String(_) => Type::String,
        }
    }
}

/// The values of a series.
fn scalars(gts: &GeoTimeSerie) -> Vec<Scalar> {
    match &gts.values {
        Values::Undefined => Vec::new(),
        Values::Long(values) => values.iter().map(|&long| Scalar::Long(long)).collect(),
        Values::Double(values) => values
            .iter()
            .map(|&double| Scalar::Double(double))
            .collect(),
        Values::Boolean(values) => values
            .iter()
            .map(|&boolean| Scalar::Boolean(boolean))
            .collect(),
        Values::String(values) => values
            .iter()
            .map(|text| Scalar::String(text.to_string()))
            .collect(),
    }
}

/// A datapoint of a stored series.
#[derive(Clone, Debug)]
struct Point {
    location: i64,
    elevation: i64,
    value: Scalar,
}

/// A series in memory.
#[derive(Debug)]
struct Stored {
    metadata: Metadata,
    points: BTreeMap<i64, Point>,
}

/// A datastore in memory: the reference semantics of the datastore functions.
#[derive(Debug, Default)]
pub struct MemoryDatastore {
    series: Mutex<Vec<Stored>>,
}

/// The identity of a series: its class and labels.
fn same_series(a: &Metadata, b: &Metadata) -> bool {
    a.name == b.name && a.labels == b.labels
}

impl MemoryDatastore {
    /// An empty datastore.
    pub fn new() -> MemoryDatastore {
        MemoryDatastore::default()
    }

    /// The series, which no operation leaves half written: a lock held by a thread that
    /// panicked is still good.
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Stored>> {
        self.series.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The indexes of the stored series that any of the selectors selects, sorted by class
    /// then labels.
    fn selected(stored: &[Stored], selectors: &[Selector]) -> Result<Vec<usize>> {
        let matchers: Vec<MetadataSelectorMatcher> = selectors
            .iter()
            .map(|selector| MetadataSelectorMatcher::new(&selector.text()))
            .collect::<Result<_>>()?;
        let mut selected = Vec::new();
        for (index, series) in stored.iter().enumerate() {
            let mut matches = false;
            for matcher in &matchers {
                matches |= matcher.matches(&series.metadata)?;
            }
            if matches {
                selected.push(index);
            }
        }
        selected.sort_by_key(|&index| {
            let metadata = &stored[index].metadata;
            let labels: Vec<(String, String)> = metadata
                .labels
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect();
            (metadata.name.clone(), labels)
        });
        Ok(selected)
    }
}

/// A count as a length: a negative count is none.
pub fn length(count: i64) -> usize {
    count.max(0) as usize
}

/// The datapoints of the range of a request, most recent first, that the request keeps: those
/// left after it skips `skip` of them, one in `step`, `timestep` apart, and `count` of them.
/// A datastore applies it to the datapoints it selected in the range (and of the wanted type).
pub fn thin_range<T>(
    range: impl IntoIterator<Item = (i64, T)>,
    request: &FetchRequest,
) -> Vec<(i64, T)> {
    let mut thinned = Vec::new();
    let mut last = None;
    for (seen, (tick, payload)) in range.into_iter().skip(length(request.skip)).enumerate() {
        if request
            .count
            .is_some_and(|count| thinned.len() >= length(count))
        {
            break;
        }
        let one_in_step = seen as i64 % request.step.max(1) == 0;
        let spaced = match (request.timestep, last) {
            (Some(timestep), Some(last)) => last - tick >= timestep,
            _ => true,
        };
        if one_in_step && spaced {
            thinned.push((tick, payload));
            last = Some(tick);
        }
    }
    thinned
}

/// The datapoints of a series a request fetches, the most recent first: those of the wanted
/// type in the range, from its end back (skipped, one in `step`, `timestep` apart, `count`
/// of them), and the boundaries around the range.
fn datapoints<'a>(series: &'a Stored, request: &FetchRequest) -> Vec<(i64, &'a Point)> {
    let wanted = |point: &Point| {
        request
            .value_type
            .is_none_or(|wanted| point.value.value_type() == wanted)
    };
    let points = || {
        series
            .points
            .iter()
            .filter(move |(_, point)| wanted(point))
            .map(|(tick, point)| (*tick, point))
    };
    let range = thin_range(
        points()
            .filter(|(tick, _)| request.start <= *tick && *tick <= request.end)
            .collect::<Vec<_>>()
            .into_iter()
            .rev(),
        request,
    );
    // The oldest datapoints after the range come first (the most recent of them last).
    let mut after: Vec<(i64, &Point)> = points()
        .filter(|(tick, _)| *tick > request.end)
        .take(length(request.boundary_post))
        .collect();
    after.reverse();
    let before = points()
        .filter(|(tick, _)| *tick < request.start)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take(length(request.boundary_pre));
    after.into_iter().chain(range).chain(before).collect()
}

/// The index of the stored series of a metadata, which is created when there is none.
fn position_of(stored: &mut Vec<Stored>, metadata: &Metadata) -> usize {
    if let Some(position) = stored
        .iter()
        .position(|s| same_series(&s.metadata, metadata))
    {
        return position;
    }
    stored.push(Stored {
        metadata: Metadata {
            name: metadata.name.clone(),
            labels: metadata.labels.clone(),
            ..Metadata::default()
        },
        points: BTreeMap::new(),
    });
    stored.len() - 1
}

impl Datastore for MemoryDatastore {
    fn find(&self, selectors: &[Selector]) -> Result<Vec<Metadata>> {
        let stored = self.lock();
        let selected = MemoryDatastore::selected(&stored, selectors)?;
        Ok(selected
            .into_iter()
            .map(|index| stored[index].metadata.clone())
            .collect())
    }

    fn fetch(&self, request: &FetchRequest) -> Result<Vec<GeoTimeSerie>> {
        let stored = self.lock();
        let selected = MemoryDatastore::selected(&stored, &request.selectors)?;
        let mut fetched = Vec::new();
        for index in selected
            .into_iter()
            .skip(length(request.series_skip))
            .take(length(request.series_count))
        {
            let series = &stored[index];
            let points = datapoints(series, request);
            if points.is_empty() && !request.keep_empty {
                continue;
            }
            let mut gts = GeoTimeSerie::new();
            gts.metadata = series.metadata.clone();
            for (tick, point) in points {
                gts.set_value(
                    tick,
                    point.location,
                    point.elevation,
                    &point.value.value(),
                    false,
                );
            }
            fetched.push(gts);
        }
        Ok(fetched)
    }

    fn update(&self, series: &[GeoTimeSerie]) -> Result<()> {
        if series
            .iter()
            .any(|gts| gts.metadata.name.as_deref().is_none_or(str::is_empty))
        {
            return Err(DatastoreError::Unnamed.into());
        }
        let mut stored = self.lock();
        for gts in series {
            let position = position_of(&mut stored, &gts.metadata);
            for (index, value) in scalars(gts).into_iter().enumerate() {
                stored[position].points.insert(
                    gts.tick_at(index),
                    Point {
                        location: gts.location_at(index),
                        elevation: gts.elevation_at(index),
                        value,
                    },
                );
            }
        }
        Ok(())
    }

    fn update_metadata(&self, metadata: &[Metadata], delta: bool) -> Result<()> {
        if metadata
            .iter()
            .any(|m| m.name.as_deref().is_none_or(str::is_empty))
        {
            return Err(DatastoreError::Unnamed.into());
        }
        let mut stored = self.lock();
        for meta in metadata {
            let position = position_of(&mut stored, meta);
            let attributes = &mut stored[position].metadata.attributes;
            if !delta {
                attributes.clear();
            }
            for (name, value) in &meta.attributes {
                if value.is_empty() {
                    attributes.shift_remove(name);
                } else {
                    attributes.insert(name.clone(), value.clone());
                }
            }
        }
        Ok(())
    }

    fn delete(&self, request: &DeleteRequest) -> Result<Vec<Metadata>> {
        let mut stored = self.lock();
        let selected = MemoryDatastore::selected(&stored, &request.selectors)?;
        let deleted: Vec<Metadata> = selected
            .iter()
            .map(|&index| stored[index].metadata.clone())
            .collect();
        match request.range {
            Some((start, end)) => {
                for index in selected {
                    stored[index]
                        .points
                        .retain(|tick, _| *tick < start || *tick > end);
                }
            }
            None => {
                // Everything goes, the series too.
                let mut position = 0;
                stored.retain(|_| {
                    let doomed = selected.contains(&position);
                    position += 1;
                    !doomed
                });
            }
        }
        Ok(deleted)
    }
}

// -----------------------------------------------------------------------------------------
// The time range of a fetch
// -----------------------------------------------------------------------------------------

/// A time range: the first and the last tick, and the timespan when it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeRange {
    /// The oldest tick (inclusive).
    pub start: i64,
    /// The most recent tick (inclusive).
    pub end: i64,
    /// The timespan the parameters gave.
    pub timespan: Option<i64>,
}

/// The names of the parameters a time range is computed from, for the errors.
#[derive(Clone, Copy, Debug)]
pub struct RangeNames<'a> {
    /// The start.
    pub start: &'a str,
    /// The end.
    pub end: &'a str,
    /// The timespan.
    pub timespan: &'a str,
    /// The count.
    pub count: &'a str,
}

/// `FETCH.getTimestamp`: a tick given as a `LONG`, as the text of a `LONG`, as an ISO 8601
/// date or as `now`; `None` when absent.
pub fn get_timestamp(value: Option<&Value>, name: &str, now: i64) -> Result<Option<i64>> {
    let invalid = || TimeRangeError::InvalidFormat(name.to_owned());
    Ok(match value {
        None | Some(Value::Null) => None,
        Some(Value::Long(long)) => Some(*long),
        Some(Value::String(text)) if &**text == "now" => Some(now),
        // A text that is not a number is a date.
        Some(Value::String(text)) => Some(parse_long(text).or_else(|_not_a_number| {
            parse_timestamp(text).map_err(|_not_a_date| Error::from(invalid()))
        })?),
        Some(_) => return Err(invalid().into()),
    })
}

/// The start of a range that ends at `last` and lasts `span` ticks.
fn start_before(last: i64, span: i64, names: RangeNames<'_>) -> Result<i64> {
    if span == 0 && last == i64::MAX {
        return Err(TimeRangeError::EmptyBeforeMaxEnd {
            timespan: names.timespan.to_owned(),
            end: names.end.to_owned(),
        }
        .into());
    }
    Ok(last.checked_sub(span).map_or(i64::MIN, |first| first + 1))
}

/// The end of a range that starts at `first` and lasts `span` ticks.
fn end_after(first: i64, span: i64, names: RangeNames<'_>) -> Result<i64> {
    if span == 0 && first == i64::MIN {
        return Err(TimeRangeError::EmptyAfterMinStart {
            timespan: names.timespan.to_owned(),
            start: names.start.to_owned(),
        }
        .into());
    }
    Ok(first.checked_add(span).map_or(i64::MAX, |last| last - 1))
}

/// `FETCH.computeTimeRange`: the time range of the start, the end, the timespan and the count
/// of a fetch (at most two of the first three).
///
/// # Errors
///
/// A range that is not specified or is contradictory, a value that is not a time.
pub fn compute_time_range(
    start: Option<&Value>,
    end: Option<&Value>,
    timespan: Option<&Value>,
    count: Option<&Value>,
    names: RangeNames<'_>,
    now: i64,
) -> Result<TimeRange> {
    let RangeNames {
        start: start_name,
        end: end_name,
        timespan: timespan_name,
        count: count_name,
    } = names;
    let mut start_tick = get_timestamp(start, start_name, now)?;
    let mut end_tick = get_timestamp(end, end_name, now)?;
    if start_tick.is_none() && end_tick.is_none() {
        return Err(TimeRangeError::MissingStartOrEnd {
            start: start_name.to_owned(),
            end: end_name.to_owned(),
        }
        .into());
    }
    // A start after the end swaps them.
    if let (Some(first), Some(last)) = (start_tick, end_tick)
        && first > last
    {
        start_tick = Some(last);
        end_tick = Some(first);
    }
    let mut numeric_timespan = None;
    if let Some(span) = timespan.filter(|span| !matches!(span, Value::Null)) {
        if start_tick.is_some() && end_tick.is_some() {
            return Err(TimeRangeError::TooManyBounds {
                start: start_name.to_owned(),
                end: end_name.to_owned(),
                timespan: timespan_name.to_owned(),
            }
            .into());
        }
        let invalid = || TimeRangeError::InvalidFormat(timespan_name.to_owned());
        // A text that is no timespan is an invalid format, because of what could not be read.
        let invalid_because = |cause: Error| Error::with_cause(invalid().to_string(), cause);
        let span_ticks = match span {
            Value::Long(long) => *long,
            Value::String(text) if text.is_empty() => {
                return Err(TimeRangeError::Empty(timespan_name.to_owned()).into());
            }
            Value::String(text) if text.starts_with('P') => {
                parse_duration(text).map_err(invalid_because)?
            }
            Value::String(text) => parse_long(text).map_err(invalid_because)?,
            _ => return Err(invalid().into()),
        };
        if span_ticks < 0 {
            return Err(TimeRangeError::Negative(timespan_name.to_owned()).into());
        }
        numeric_timespan = Some(span_ticks);
        // Exactly one of the bounds is given: the timespan gives the other.
        (start_tick, end_tick) = match (start_tick, end_tick) {
            (None, Some(last)) => (Some(start_before(last, span_ticks, names)?), Some(last)),
            (first, _) => (
                first,
                first
                    .map(|first| end_after(first, span_ticks, names))
                    .transpose()?,
            ),
        };
    }
    let Some(end_tick) = end_tick else {
        return Err(TimeRangeError::MissingEnd {
            end: end_name.to_owned(),
            start: start_name.to_owned(),
            timespan: timespan_name.to_owned(),
        }
        .into());
    };
    let start_tick = match start_tick {
        Some(first) => first,
        None if count.is_some_and(|count| !matches!(count, Value::Null)) => i64::MIN,
        None => {
            return Err(TimeRangeError::CountMandatory {
                count: count_name.to_owned(),
                start: start_name.to_owned(),
                timespan: timespan_name.to_owned(),
            }
            .into());
        }
    };
    Ok(TimeRange {
        start: start_tick,
        end: end_tick,
        timespan: numeric_timespan,
    })
}
