// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The datastore of the datastore functions (`FETCH`, `FIND`, `UPDATE`, `META`, `DELETE`) on
//! InfluxDB 3: SQL for what is read, line protocol for what is written (`schema.rs` tells how
//! Warp 10 concepts are laid out). The selectors are evaluated here, on the series found, and
//! not in SQL, so that they mean what they mean everywhere else (a label selector also
//! selects on an attribute, regular expressions are Java's).

use std::collections::{BTreeMap, BTreeSet};

use indexmap::IndexMap;
use serde_json::Value as Json;
use warpscript::Value;
use warpscript::datastore::{Datastore, DeleteRequest, FetchRequest, Selector, length, thin_range};
use warpscript::error::datastore::DatastoreError;
use warpscript::functions::runtime::current_time;
use warpscript::gts::{GeoTimeSerie, Metadata, NO_ELEVATION, NO_LOCATION};
use warpscript::selector::MetadataSelectorMatcher;

use super::error::InfluxError;
use super::lineprotocol::{meta_line, series_lines};
use super::schema::{
    ELEVATION, HHCODE, META_ATTRIBUTES, META_CLASS, META_DELETED, META_LABELS, META_TABLE,
    VALUE_BOOLEAN, VALUE_DOUBLE, VALUE_LONG, VALUE_STRING, VALUE_TYPE, is_deleted_table,
    type_columns,
};
use super::sql::{identifier, literal, nanoseconds, timestamp};
use super::transport::{Row, Transport};

/// The most lines written in one request.
const WRITE_BATCH: usize = 10_000;

/// A failure of the datastore: InfluxDB 3, or what the script asked.
#[derive(Debug, thiserror::Error)]
enum StoreError {
    #[error(transparent)]
    Influx(#[from] InfluxError),
    #[error(transparent)]
    Script(#[from] warpscript::Error),
    #[error(transparent)]
    Datastore(#[from] DatastoreError),
}

/// The failure a script sees.
fn script_error(operation: &'static str, err: StoreError) -> warpscript::Error {
    match err {
        StoreError::Script(err) => err,
        StoreError::Datastore(err) => err.into(),
        StoreError::Influx(err) => DatastoreError::Backend {
            operation,
            message: err.to_string(),
        }
        .into(),
    }
}

/// The columns of a table.
#[derive(Debug, Default)]
struct Table {
    /// Every column.
    columns: BTreeSet<String>,
    /// The columns that are tags, sorted: the labels of the series.
    tags: Vec<String>,
}

/// The tables of the database that hold classes, and whether the attributes have a table.
#[derive(Debug, Default)]
struct Schema {
    classes: BTreeMap<String, Table>,
    has_attributes: bool,
}

/// What the table of the attributes says of a series.
#[derive(Debug, Default)]
struct Attributes {
    values: IndexMap<String, String>,
    deleted: bool,
    time: i64,
}

/// A series by class and labels (sorted), the way the datastore sorts them.
type SeriesKey = (String, Vec<(String, String)>);

/// The labels of a series sorted by name.
fn sorted_labels(labels: &IndexMap<String, String>) -> Vec<(String, String)> {
    let mut labels: Vec<(String, String)> = labels
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    labels.sort();
    labels
}

fn series_key_of(metadata: &Metadata) -> Option<SeriesKey> {
    Some((metadata.name.clone()?, sorted_labels(&metadata.labels)))
}

/// The table a class selector names, when it is not a regular expression.
fn exact_class(selector: &str) -> Option<&str> {
    if selector.starts_with('~') {
        None
    } else {
        Some(selector.strip_prefix('=').unwrap_or(selector))
    }
}

/// A datapoint read back.
#[derive(Debug)]
struct Datapoint {
    location: i64,
    elevation: i64,
    value: Value,
}

/// The column of a row, when it has it.
fn column<'a, T>(
    row: &'a Row,
    name: &str,
    expected: &'static str,
    convert: impl Fn(&'a Json) -> Option<T>,
) -> Result<Option<T>, InfluxError> {
    row.get(name)
        .map(|json| {
            convert(json).ok_or_else(|| InfluxError::BadColumn {
                column: name.to_owned(),
                expected,
            })
        })
        .transpose()
}

fn optional_text<'a>(row: &'a Row, name: &str) -> Result<Option<&'a str>, InfluxError> {
    column(row, name, "a text", Json::as_str)
}

fn optional_integer(row: &Row, name: &str) -> Result<Option<i64>, InfluxError> {
    column(row, name, "an integer", Json::as_i64)
}

fn text<'a>(row: &'a Row, name: &str) -> Result<&'a str, InfluxError> {
    optional_text(row, name)?.ok_or_else(|| InfluxError::BadColumn {
        column: name.to_owned(),
        expected: "a text",
    })
}

fn integer(row: &Row, name: &str) -> Result<i64, InfluxError> {
    optional_integer(row, name)?.ok_or_else(|| InfluxError::BadColumn {
        column: name.to_owned(),
        expected: "an integer",
    })
}

/// The value of the row of a datapoint, in the column of its type.
fn datapoint_value(row: &Row) -> Result<Value, InfluxError> {
    let missing = |name: &str, expected| InfluxError::BadColumn {
        column: name.to_owned(),
        expected,
    };
    Ok(match text(row, VALUE_TYPE)? {
        "l" => Value::Long(
            optional_integer(row, VALUE_LONG)?.ok_or_else(|| missing(VALUE_LONG, "an integer"))?,
        ),
        "d" => Value::Double(
            column(row, VALUE_DOUBLE, "a number", Json::as_f64)?
                .ok_or_else(|| missing(VALUE_DOUBLE, "a number"))?,
        ),
        "b" => Value::Boolean(
            column(row, VALUE_BOOLEAN, "a boolean", Json::as_bool)?
                .ok_or_else(|| missing(VALUE_BOOLEAN, "a boolean"))?,
        ),
        "s" => Value::from(
            optional_text(row, VALUE_STRING)?.ok_or_else(|| missing(VALUE_STRING, "a text"))?,
        ),
        _ => return Err(missing(VALUE_TYPE, "one of l, d, b, s")),
    })
}

/// A JSON object of texts, with its members sorted by name.
fn json_object<'a>(members: impl Iterator<Item = (&'a String, &'a String)>) -> String {
    let sorted: BTreeMap<&String, &String> = members.collect();
    Json::Object(
        sorted
            .into_iter()
            .map(|(name, value)| (name.clone(), Json::String(value.clone())))
            .collect(),
    )
    .to_string()
}

/// The labels of a series, as the JSON object the table of the attributes holds.
fn labels_json(labels: &[(String, String)]) -> String {
    json_object(labels.iter().map(|(name, value)| (name, value)))
}

fn attributes_json(attributes: &IndexMap<String, String>) -> String {
    json_object(attributes.iter())
}

/// The datastore on InfluxDB 3.
#[derive(Debug)]
pub struct InfluxDatastore {
    transport: Box<dyn Transport>,
    /// The time, in microseconds.
    clock: fn() -> i64,
}

impl InfluxDatastore {
    /// A datastore on a transport.
    pub fn new(transport: Box<dyn Transport>) -> InfluxDatastore {
        InfluxDatastore::with_clock(transport, current_time)
    }

    /// A datastore on a transport, with a clock (the time, in microseconds).
    pub fn with_clock(transport: Box<dyn Transport>, clock: fn() -> i64) -> InfluxDatastore {
        InfluxDatastore { transport, clock }
    }

    /// The tables of the database.
    fn schema(&self) -> Result<Schema, StoreError> {
        let rows = self.transport.query(
            "SELECT table_name, column_name, data_type FROM information_schema.columns \
             WHERE table_schema = 'iox' ORDER BY table_name, column_name",
        )?;
        let mut tables: BTreeMap<String, Table> = BTreeMap::new();
        for row in &rows {
            let table = tables
                .entry(text(row, "table_name")?.to_owned())
                .or_default();
            let column = text(row, "column_name")?;
            if text(row, "data_type")?.starts_with("Dictionary") {
                table.tags.push(column.to_owned());
            }
            table.columns.insert(column.to_owned());
        }
        for table in tables.values_mut() {
            table.tags.sort();
        }
        let has_attributes = tables.contains_key(META_TABLE);
        tables.retain(|name, table| table.columns.contains(VALUE_TYPE) && !is_deleted_table(name));
        Ok(Schema {
            classes: tables,
            has_attributes,
        })
    }

    /// The labels of the series that have datapoints in a table.
    fn labels_of_class(
        &self,
        class: &str,
        table: &Table,
    ) -> Result<Vec<Vec<(String, String)>>, StoreError> {
        let mut selected: Vec<String> = table.tags.iter().map(|tag| identifier(tag)).collect();
        if selected.is_empty() {
            selected.push("1 AS present".to_owned());
        }
        let rows = self.transport.query(&format!(
            "SELECT DISTINCT {} FROM {}",
            selected.join(", "),
            identifier(class)
        ))?;
        let mut series = Vec::new();
        for row in &rows {
            let mut labels = Vec::new();
            for tag in &table.tags {
                if let Some(value) = optional_text(row, tag)? {
                    labels.push((tag.clone(), value.to_owned()));
                }
            }
            series.push(labels);
        }
        Ok(series)
    }

    /// What the table of the attributes says of the series (of the classes, or of all of them
    /// when there are none given), the oldest rows first so that the latest ones prevail.
    fn attributes(
        &self,
        schema: &Schema,
        classes: Option<&BTreeSet<&str>>,
    ) -> Result<BTreeMap<(String, String), Attributes>, StoreError> {
        let mut states = BTreeMap::new();
        if !schema.has_attributes || classes.is_some_and(BTreeSet::is_empty) {
            return Ok(states);
        }
        let filter = classes.map_or_else(String::new, |classes| {
            let classes: Vec<String> = classes.iter().map(|class| literal(class)).collect();
            format!(
                " WHERE {} IN ({})",
                identifier(META_CLASS),
                classes.join(", ")
            )
        });
        let rows = self.transport.query(&format!(
            "SELECT {}, {}, {}, {}, CAST(time AS BIGINT) AS ts FROM {}{filter} ORDER BY time",
            identifier(META_CLASS),
            identifier(META_LABELS),
            identifier(META_ATTRIBUTES),
            identifier(META_DELETED),
            identifier(META_TABLE),
        ))?;
        for row in &rows {
            let key = (
                text(row, META_CLASS)?.to_owned(),
                text(row, META_LABELS)?.to_owned(),
            );
            let values: BTreeMap<String, String> =
                serde_json::from_str(text(row, META_ATTRIBUTES)?).map_err(InfluxError::from)?;
            let deleted = column(row, META_DELETED, "a boolean", Json::as_bool)?.unwrap_or(false);
            states.insert(
                key,
                Attributes {
                    // Whatever a series that was deleted had is gone with it.
                    values: if deleted {
                        IndexMap::new()
                    } else {
                        values.into_iter().collect()
                    },
                    deleted,
                    time: integer(row, "ts")?,
                },
            );
        }
        Ok(states)
    }

    /// The series any selector selects, sorted by class then labels, with their attributes.
    fn find_series(
        &self,
        schema: &Schema,
        selectors: &[Selector],
    ) -> Result<Vec<Metadata>, StoreError> {
        let matchers: Vec<MetadataSelectorMatcher> = selectors
            .iter()
            .map(|selector| MetadataSelectorMatcher::new(&selector.text()))
            .collect::<warpscript::Result<_>>()?;
        // A class selector that is not a regular expression names its table; the others
        // select among every class.
        let exact: Vec<Option<&str>> = selectors
            .iter()
            .map(|selector| exact_class(&selector.class))
            .collect();
        let all_exact = exact.iter().all(Option::is_some);
        let classes: BTreeSet<&str> = if all_exact {
            exact.iter().flatten().copied().collect()
        } else {
            schema.classes.keys().map(String::as_str).collect()
        };
        let attributes = self.attributes(schema, all_exact.then_some(&classes))?;
        let mut universe: BTreeMap<SeriesKey, Metadata> = BTreeMap::new();
        for class in &classes {
            let Some(table) = schema.classes.get(*class) else {
                continue;
            };
            for labels in self.labels_of_class(class, table)? {
                universe.insert(
                    ((*class).to_owned(), labels.clone()),
                    Metadata {
                        name: Some((*class).to_owned()),
                        labels: labels.into_iter().collect(),
                        ..Metadata::default()
                    },
                );
            }
        }
        // Series that only have attributes, because `META` created them.
        for ((class, labels), state) in &attributes {
            if state.deleted {
                continue;
            }
            let labels: BTreeMap<String, String> =
                serde_json::from_str(labels).map_err(InfluxError::from)?;
            let labels: Vec<(String, String)> = labels.into_iter().collect();
            universe
                .entry((class.clone(), labels.clone()))
                .or_insert_with(|| Metadata {
                    name: Some(class.clone()),
                    labels: labels.into_iter().collect(),
                    ..Metadata::default()
                });
        }
        let mut found = Vec::new();
        for (key, mut metadata) in universe {
            if let Some(state) = attributes.get(&(key.0.clone(), labels_json(&key.1))) {
                metadata.attributes.clone_from(&state.values);
            }
            let mut selected = false;
            for matcher in &matchers {
                selected |= matcher.matches(&metadata)?;
            }
            if selected {
                found.push(metadata);
            }
        }
        Ok(found)
    }

    /// The datapoints of a series, most recent first.
    fn datapoints(
        &self,
        schema: &Schema,
        metadata: &Metadata,
        request: &FetchRequest,
    ) -> Result<Vec<(i64, Datapoint)>, StoreError> {
        let Some(class) = metadata.name.as_deref() else {
            return Ok(Vec::new());
        };
        // A series without a table, or with a label that has no column, has no datapoint.
        let Some(table) = schema.classes.get(class) else {
            return Ok(Vec::new());
        };
        if !metadata
            .labels
            .keys()
            .all(|label| table.tags.contains(label))
        {
            return Ok(Vec::new());
        }
        let mut conditions: Vec<String> = table
            .tags
            .iter()
            .map(|tag| match metadata.labels.get(tag) {
                Some(value) => format!("{} = {}", identifier(tag), literal(value)),
                None => format!("{} IS NULL", identifier(tag)),
            })
            .collect();
        if let Some(wanted) = request.value_type {
            let Some((marker, _)) = type_columns(wanted) else {
                return Ok(Vec::new());
            };
            conditions.push(format!("{} = {}", identifier(VALUE_TYPE), literal(marker)));
        }
        let mut selected = vec!["CAST(time AS BIGINT) AS ts".to_owned()];
        selected.extend(
            [
                VALUE_TYPE,
                VALUE_LONG,
                VALUE_DOUBLE,
                VALUE_BOOLEAN,
                VALUE_STRING,
                HHCODE,
                ELEVATION,
            ]
            .into_iter()
            .filter(|name| table.columns.contains(*name))
            .map(identifier),
        );
        // A datapoint belongs to the tick its nanoseconds are in.
        let first = nanoseconds_saturating(request.start);
        let last = nanoseconds_saturating(request.end).saturating_add(999);
        let query = |extra: String, order: &str, limit: Option<usize>| {
            let mut conditions = conditions.clone();
            conditions.push(extra);
            format!(
                "SELECT {} FROM {} WHERE {} ORDER BY time {order}{}",
                selected.join(", "),
                identifier(class),
                conditions.join(" AND "),
                limit.map_or_else(String::new, |limit| format!(" LIMIT {limit}")),
            )
        };
        let within = format!(
            "time >= {} AND time <= {}",
            timestamp(first),
            timestamp(last)
        );
        // Without thinning, the datapoints that are skipped and kept are all that are needed.
        let limit = (request.step <= 1 && request.timestep.is_none())
            .then_some(request.count)
            .flatten()
            .map(|count| length(request.skip).saturating_add(length(count)));
        let range = self.read_datapoints(&query(within, "DESC", limit))?;
        let mut points = thin_range(range, request);
        if request.boundary_post > 0 {
            let mut after = self.read_datapoints(&query(
                format!("time > {}", timestamp(last)),
                "ASC",
                Some(length(request.boundary_post)),
            ))?;
            after.reverse();
            after.append(&mut points);
            points = after;
        }
        if request.boundary_pre > 0 {
            points.extend(self.read_datapoints(&query(
                format!("time < {}", timestamp(first)),
                "DESC",
                Some(length(request.boundary_pre)),
            ))?);
        }
        Ok(points)
    }

    /// The datapoints a query selects, in the order of the query.
    fn read_datapoints(&self, sql: &str) -> Result<Vec<(i64, Datapoint)>, StoreError> {
        let mut points = Vec::new();
        for row in &self.transport.query(sql)? {
            points.push((
                integer(row, "ts")?.div_euclid(1000),
                Datapoint {
                    location: optional_integer(row, HHCODE)?.unwrap_or(NO_LOCATION),
                    elevation: optional_integer(row, ELEVATION)?.unwrap_or(NO_ELEVATION),
                    value: datapoint_value(row)?,
                },
            ));
        }
        Ok(points)
    }

    /// The time of a write of attributes, in nanoseconds.
    fn now(&self) -> Result<i64, InfluxError> {
        nanoseconds((self.clock)()).ok_or(InfluxError::Clock)
    }

    fn write_lines(&self, lines: &[String]) -> Result<(), InfluxError> {
        for batch in lines.chunks(WRITE_BATCH) {
            self.transport.write(&batch.join("\n"))?;
        }
        Ok(())
    }

    fn fetch_series(&self, request: &FetchRequest) -> Result<Vec<GeoTimeSerie>, StoreError> {
        let schema = self.schema()?;
        let series = self.find_series(&schema, &request.selectors)?;
        let mut fetched = Vec::new();
        for metadata in series
            .into_iter()
            .skip(length(request.series_skip))
            .take(length(request.series_count))
        {
            let points = self.datapoints(&schema, &metadata, request)?;
            if points.is_empty() && !request.keep_empty {
                continue;
            }
            let mut gts = GeoTimeSerie::new();
            gts.metadata = metadata;
            for (tick, point) in points {
                gts.set_value(tick, point.location, point.elevation, &point.value, false);
            }
            fetched.push(gts);
        }
        Ok(fetched)
    }

    fn update_series(&self, series: &[GeoTimeSerie]) -> Result<(), StoreError> {
        let mut lines = Vec::new();
        for gts in series {
            let class = named(&gts.metadata)?;
            lines.extend(series_lines(class, gts)?);
        }
        Ok(self.write_lines(&lines)?)
    }

    fn update_attributes(&self, metadata: &[Metadata], delta: bool) -> Result<(), StoreError> {
        let named_series: Vec<(&str, &Metadata)> = metadata
            .iter()
            .map(|meta| Ok((named(meta)?, meta)))
            .collect::<Result<_, DatastoreError>>()?;
        let classes: BTreeSet<&str> = named_series.iter().map(|(class, _)| *class).collect();
        let schema = self.schema()?;
        let mut states = self.attributes(&schema, Some(&classes))?;
        let now = self.now()?;
        let mut lines = Vec::new();
        let mut next = now;
        for (class, meta) in named_series {
            let labels = labels_json(&sorted_labels(&meta.labels));
            let state = states
                .entry((class.to_owned(), labels.clone()))
                .or_default();
            if !delta {
                state.values.clear();
            }
            for (name, value) in &meta.attributes {
                if value.is_empty() {
                    state.values.shift_remove(name);
                } else {
                    state.values.insert(name.clone(), value.clone());
                }
            }
            state.deleted = false;
            // Rows of a series are told apart by their time: each one is after the last.
            state.time = next.max(state.time.saturating_add(1));
            next = state.time.saturating_add(1);
            lines.push(meta_line(
                class,
                &labels,
                &attributes_json(&state.values),
                false,
                state.time,
            )?);
        }
        Ok(self.write_lines(&lines)?)
    }

    fn delete_series(&self, request: &DeleteRequest) -> Result<Vec<Metadata>, StoreError> {
        if request.range.is_some() {
            return Err(InfluxError::PartialDelete(
                "delete a whole series, with NULL NULL as the time range".to_owned(),
            )
            .into());
        }
        let schema = self.schema()?;
        let selected = self.find_series(&schema, &request.selectors)?;
        let selected_keys: BTreeSet<SeriesKey> =
            selected.iter().filter_map(series_key_of).collect();
        let classes: BTreeSet<&str> = selected
            .iter()
            .filter_map(|metadata| metadata.name.as_deref())
            .collect();
        // A table is deleted with all its series: they must all be selected.
        let mut doomed = Vec::new();
        for class in &classes {
            let Some(table) = schema.classes.get(*class) else {
                continue;
            };
            for labels in self.labels_of_class(class, table)? {
                if !selected_keys.contains(&((*class).to_owned(), labels)) {
                    return Err(InfluxError::PartialDelete(format!(
                        "a series of '{class}' would be left in its table"
                    ))
                    .into());
                }
            }
            doomed.push(*class);
        }
        let states = self.attributes(&schema, Some(&classes))?;
        let now = self.now()?;
        let mut lines = Vec::new();
        let mut next = now;
        for key in &selected_keys {
            let labels = labels_json(&key.1);
            if let Some(state) = states.get(&(key.0.clone(), labels.clone()))
                && !state.deleted
            {
                let time = next.max(state.time.saturating_add(1));
                next = time.saturating_add(1);
                lines.push(meta_line(&key.0, &labels, "{}", true, time)?);
            }
        }
        for class in doomed {
            self.transport.delete_table(class)?;
        }
        self.write_lines(&lines)?;
        Ok(selected)
    }
}

/// The class of a series that is named.
fn named(metadata: &Metadata) -> Result<&str, DatastoreError> {
    metadata
        .name
        .as_deref()
        .filter(|name| !name.is_empty())
        .ok_or(DatastoreError::Unnamed)
}

/// The nanoseconds of a tick, saturated when they do not fit.
fn nanoseconds_saturating(tick: i64) -> i64 {
    tick.saturating_mul(1000)
}

impl Datastore for InfluxDatastore {
    fn find(&self, selectors: &[Selector]) -> warpscript::Result<Vec<Metadata>> {
        self.schema()
            .and_then(|schema| self.find_series(&schema, selectors))
            .map_err(|err| script_error("find", err))
    }

    fn fetch(&self, request: &FetchRequest) -> warpscript::Result<Vec<GeoTimeSerie>> {
        self.fetch_series(request)
            .map_err(|err| script_error("fetch", err))
    }

    fn update(&self, series: &[GeoTimeSerie]) -> warpscript::Result<()> {
        self.update_series(series)
            .map_err(|err| script_error("update", err))
    }

    fn update_metadata(&self, metadata: &[Metadata], delta: bool) -> warpscript::Result<()> {
        self.update_attributes(metadata, delta)
            .map_err(|err| script_error("update metadata", err))
    }

    fn delete(&self, request: &DeleteRequest) -> warpscript::Result<Vec<Metadata>> {
        self.delete_series(request)
            .map_err(|err| script_error("delete", err))
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod store_tests;
