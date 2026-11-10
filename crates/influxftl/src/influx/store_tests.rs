// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the datastore on InfluxDB 3, on a transport that answers what a test scripts and
//! records what it is asked.

use std::sync::{Arc, Mutex, MutexGuard};

use indexmap::IndexMap;
use serde_json::{Value as Json, json};
use warpscript::Value;
use warpscript::datastore::{Datastore, DeleteRequest, FetchRequest, Selector};
use warpscript::gts::{GeoTimeSerie, Metadata, NO_ELEVATION, NO_LOCATION, Type};

use super::super::error::InfluxError;
use super::super::transport::{Row, Transport};
use super::{InfluxDatastore, named, series_key_of};

/// What the fake transport was asked, and what it answers.
#[derive(Debug, Default)]
struct State {
    queries: Vec<String>,
    writes: Vec<String>,
    deletions: Vec<String>,
    /// The rows of the first fragment the query contains.
    answers: Vec<(String, Vec<Row>)>,
    /// The number of requests so far.
    calls: usize,
    /// The request that fails, by its rank.
    fail_at: Option<usize>,
}

/// A transport shared with the test.
#[derive(Clone, Debug, Default)]
struct Fake(Arc<Mutex<State>>);

fn rows(json: &Json) -> Vec<Row> {
    json.as_array()
        .expect("rows are a list")
        .iter()
        .map(|row| row.as_object().expect("a row is an object").clone())
        .collect()
}

impl Fake {
    fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().expect("the lock is not poisoned")
    }

    /// Answer the queries that contain `fragment` with `json` rows.
    fn answer(&self, fragment: &str, json: &Json) -> &Fake {
        self.state().answers.push((fragment.to_owned(), rows(json)));
        self
    }

    fn fail_at(&self, rank: usize) {
        self.state().fail_at = Some(rank);
    }

    fn queries(&self) -> Vec<String> {
        self.state().queries.clone()
    }

    fn writes(&self) -> Vec<String> {
        self.state().writes.clone()
    }

    fn deletions(&self) -> Vec<String> {
        self.state().deletions.clone()
    }

    /// A transport that has the tables of `standard_schema`.
    fn standard(attributes: bool) -> Fake {
        let fake = Fake::default();
        fake.answer("information_schema", &standard_schema(attributes));
        fake
    }

    /// Count a request, and fail it when it is the one that must fail.
    fn step(state: &mut State) -> Result<(), InfluxError> {
        state.calls += 1;
        if state.fail_at == Some(state.calls - 1) {
            return Err(InfluxError::Status {
                status: 500,
                message: "boom".to_owned(),
            });
        }
        Ok(())
    }
}

impl Transport for Fake {
    fn query(&self, sql: &str) -> Result<Vec<Row>, InfluxError> {
        let mut state = self.state();
        state.queries.push(sql.to_owned());
        Fake::step(&mut state)?;
        Ok(state
            .answers
            .iter()
            .find(|(fragment, _)| sql.contains(fragment.as_str()))
            .map(|(_, rows)| rows.clone())
            .unwrap_or_default())
    }

    fn write(&self, line_protocol: &str) -> Result<(), InfluxError> {
        let mut state = self.state();
        state.writes.push(line_protocol.to_owned());
        Fake::step(&mut state)
    }

    fn delete_table(&self, table: &str) -> Result<(), InfluxError> {
        let mut state = self.state();
        state.deletions.push(table.to_owned());
        Fake::step(&mut state)
    }
}

const TAG: &str = "Dictionary(Int32, Utf8)";
const TIME: &str = "Timestamp(Nanosecond, None)";

fn table_columns(table: &str, columns: &[(&str, &str)]) -> Vec<Json> {
    columns
        .iter()
        .map(|(column, data_type)| {
            json!({"table_name": table, "column_name": column, "data_type": data_type})
        })
        .collect()
}

/// The tables of a database: `cpu` (tags `dc` and `host`, every column), `mem` (no tag, longs),
/// a table InfluxDB 3 deleted, a table of somebody else, and the table of the attributes.
fn standard_schema(attributes: bool) -> Json {
    let mut columns = table_columns(
        "cpu",
        &[
            ("dc", TAG),
            ("host", TAG),
            ("time", TIME),
            ("value_type", "Utf8"),
            ("value_l", "Int64"),
            ("value_d", "Float64"),
            ("value_b", "Boolean"),
            ("value_s", "Utf8"),
            ("hhcode", "Int64"),
            ("elevation", "Int64"),
        ],
    );
    columns.extend(table_columns(
        "mem",
        &[("time", TIME), ("value_type", "Utf8"), ("value_l", "Int64")],
    ));
    columns.extend(table_columns(
        "old-20261110T132233",
        &[("time", TIME), ("value_type", "Utf8")],
    ));
    columns.extend(table_columns("foreign", &[("a", TAG), ("time", TIME)]));
    if attributes {
        columns.extend(table_columns(
            "_influxftl_series",
            &[
                ("class", TAG),
                ("labels", TAG),
                ("attributes", "Utf8"),
                ("deleted", "Boolean"),
                ("time", TIME),
            ],
        ));
    }
    Json::Array(columns)
}

/// The clock of the tests: one second, in microseconds.
const NOW: i64 = 1_000_000;

fn store(fake: &Fake) -> InfluxDatastore {
    InfluxDatastore::with_clock(Box::new(fake.clone()), || NOW)
}

fn selector(class: &str, labels: &[(&str, &str)]) -> Selector {
    Selector {
        class: class.to_owned(),
        labels: labels
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
    }
}

fn metadata(class: &str, labels: &[(&str, &str)], attributes: &[(&str, &str)]) -> Metadata {
    let pairs = |pairs: &[(&str, &str)]| -> IndexMap<String, String> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    };
    Metadata {
        name: Some(class.to_owned()),
        labels: pairs(labels),
        attributes: pairs(attributes),
        ..Metadata::default()
    }
}

/// A request for the datapoints of the selectors between two ticks.
fn request(selectors: Vec<Selector>, start: i64, end: i64) -> FetchRequest {
    FetchRequest {
        selectors,
        start,
        end,
        count: None,
        value_type: None,
        boundary_pre: 0,
        boundary_post: 0,
        keep_empty: false,
        skip: 0,
        step: 1,
        timestep: None,
        series_skip: 0,
        series_count: i64::MAX,
    }
}

/// What a script sees of a failure.
fn message<T: std::fmt::Debug>(result: warpscript::Result<T>) -> String {
    result.expect_err("a failure").to_string()
}

/// What a series is: its class, its labels and its attributes, as text.
fn described(metadata: &Metadata) -> String {
    let pairs = |pairs: &IndexMap<String, String>| {
        pairs
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "{}{{{}}}{{{}}}",
        metadata.name.as_deref().unwrap_or("?"),
        pairs(&metadata.labels),
        pairs(&metadata.attributes)
    )
}

fn series_of(class: &str, labels: &[(&str, &str)], points: &[(i64, Value)]) -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::new();
    gts.metadata = metadata(class, labels, &[]);
    for (tick, value) in points {
        gts.set_value(*tick, NO_LOCATION, NO_ELEVATION, value, false);
    }
    gts
}

/// Run a scenario with every request failing in turn: each failure must be the one of the
/// request, reported to the script, and the scenario must end by succeeding.
fn every_request_fails<T: std::fmt::Debug>(
    prepare: impl Fn() -> Fake,
    scenario: impl Fn(&InfluxDatastore) -> warpscript::Result<T>,
) -> usize {
    let mut rank = 0;
    loop {
        assert!(rank < 100, "the scenario never succeeds");
        let fake = prepare();
        fake.fail_at(rank);
        match scenario(&store(&fake)) {
            Ok(_) => {
                assert!(
                    fake.state().calls <= rank,
                    "request {rank} did not fail the scenario"
                );
                return rank;
            }
            Err(err) => {
                let text = err.to_string();
                assert!(
                    text.contains("failed on the datastore: InfluxDB 3 answered HTTP 500: boom"),
                    "{rank}: {text}"
                );
            }
        }
        rank += 1;
    }
}

const ATTRIBUTES_QUERY: &str = "SELECT \"class\", \"labels\", \"attributes\", \"deleted\", CAST(time AS BIGINT) AS ts FROM \"_influxftl_series\"";

/// `cpu` with three series, `mem` with one, and the attributes of two series of `cpu` and
/// `ghost`, which only has attributes.
fn found_fake() -> Fake {
    let fake = Fake::standard(true);
    fake.answer(
        ATTRIBUTES_QUERY,
        &json!([
            {"class": "cpu", "labels": "{\"dc\":\"eu\",\"host\":\"a\"}", "attributes": "{\"unit\":\"%\"}", "deleted": false, "ts": 5},
            {"class": "ghost", "labels": "{\"x\":\"1\"}", "attributes": "{\"k\":\"v\"}", "ts": 6},
            {"class": "ghost", "labels": "{\"x\":\"2\"}", "attributes": "{}", "deleted": true, "ts": 7},
        ]),
    )
    .answer(
        "SELECT DISTINCT \"dc\", \"host\" FROM \"cpu\"",
        &json!([{"dc": "eu", "host": "a"}, {"dc": "us", "host": "b"}, {"host": "c"}]),
    )
    .answer(
        "SELECT DISTINCT 1 AS present FROM \"mem\"",
        &json!([{"present": 1}]),
    );
    fake
}

fn found(fake: &Fake, selectors: &[Selector]) -> Vec<String> {
    store(fake)
        .find(selectors)
        .expect("series")
        .iter()
        .map(described)
        .collect()
}

#[test]
fn every_class_selected_by_a_regular_expression_is_searched() {
    let fake = found_fake();
    assert_eq!(
        found(&fake, &[selector("~.*", &[])]),
        [
            "cpu{dc=eu,host=a}{unit=%}",
            "cpu{dc=us,host=b}{}",
            "cpu{host=c}{}",
            "ghost{x=1}{k=v}",
            "mem{}{}",
        ]
    );
    let queries = fake.queries();
    assert_eq!(queries.len(), 4, "{queries:?}");
    assert!(
        queries[0].contains("FROM information_schema.columns"),
        "{queries:?}"
    );
    assert_eq!(queries[1], format!("{ATTRIBUTES_QUERY} ORDER BY time"));
    assert_eq!(queries[2], "SELECT DISTINCT \"dc\", \"host\" FROM \"cpu\"");
    assert_eq!(queries[3], "SELECT DISTINCT 1 AS present FROM \"mem\"");
}

#[test]
fn a_class_that_is_named_is_the_only_table_searched() {
    let fake = found_fake();
    assert_eq!(
        found(
            &fake,
            &[selector("cpu", &[("host", "a")]), selector("=ghost", &[])]
        ),
        ["cpu{dc=eu,host=a}{unit=%}", "ghost{x=1}{k=v}"]
    );
    let queries = fake.queries();
    assert_eq!(queries.len(), 3, "{queries:?}");
    assert_eq!(
        queries[1],
        format!("{ATTRIBUTES_QUERY} WHERE \"class\" IN ('cpu', 'ghost') ORDER BY time")
    );
    assert_eq!(queries[2], "SELECT DISTINCT \"dc\", \"host\" FROM \"cpu\"");
}

#[test]
fn a_label_selector_selects_on_the_attributes_too() {
    assert_eq!(
        found(&found_fake(), &[selector("cpu", &[("unit", "%")])]),
        ["cpu{dc=eu,host=a}{unit=%}"]
    );
    assert_eq!(
        found(&found_fake(), &[selector("cpu", &[("host", "~[ab]")])]),
        ["cpu{dc=eu,host=a}{unit=%}", "cpu{dc=us,host=b}{}"]
    );
}

#[test]
fn nothing_is_selected_by_no_selector() {
    let fake = found_fake();
    assert_eq!(found(&fake, &[]).len(), 0);
    assert_eq!(fake.queries().len(), 1);
}

#[test]
fn a_database_without_attributes_has_no_table_for_them() {
    let fake = Fake::standard(false);
    fake.answer(
        "SELECT DISTINCT \"dc\", \"host\" FROM \"cpu\"",
        &json!([{"dc": "eu", "host": "a"}]),
    );
    assert_eq!(
        found(&fake, &[selector("cpu", &[])]),
        ["cpu{dc=eu,host=a}{}"]
    );
    assert_eq!(fake.queries().len(), 2);
}

#[test]
fn the_tables_that_are_not_classes_are_never_read() {
    let fake = found_fake();
    found(&fake, &[selector("~.*", &[])]);
    for query in fake.queries() {
        assert!(!query.contains("foreign"), "{query}");
        assert!(!query.contains("old-"), "{query}");
    }
    // A class that has the name of a table that is not one has no series.
    assert_eq!(found(&found_fake(), &[selector("foreign", &[])]).len(), 0);
    assert_eq!(
        found(&found_fake(), &[selector("old-20261110T132233", &[])]).len(),
        0
    );
}

#[test]
fn a_selector_that_cannot_be_used_fails_the_search() {
    let err = message(store(&found_fake()).find(&[selector("~(", &[])]));
    assert!(!err.contains("failed on the datastore"), "{err}");
    let err = message(store(&found_fake()).find(&[selector("cpu", &[("a", "~(")])]));
    assert!(!err.contains("failed on the datastore"), "{err}");
    // The backtracking limit of the regular expression engine.
    let fake = Fake::default();
    fake.answer(
        "information_schema",
        &Json::Array(table_columns(
            &"a".repeat(46),
            &[("time", TIME), ("value_type", "Utf8")],
        )),
    );
    fake.answer("SELECT DISTINCT", &json!([{"present": 1}]));
    let err = message(store(&fake).find(&[selector("~(a|a)*\\1b", &[])]));
    assert!(!err.contains("failed on the datastore"), "{err}");
}

#[test]
fn a_failing_request_fails_the_search() {
    let requests = every_request_fails(found_fake, |store| store.find(&[selector("~.*", &[])]));
    assert_eq!(requests, 4);
}

/// The failure of the search a script sees, when InfluxDB 3 answers the queries that contain
/// `fragment` with rows that are not understood.
fn bad_rows(fragment: &str, answer: &Json) -> String {
    let fake = found_fake();
    fake.state()
        .answers
        .insert(0, (fragment.to_owned(), rows(answer)));
    message(store(&fake).find(&[selector("~.*", &[])]))
}

fn bad_column(column: &str, expected: &str) -> String {
    format!(
        "find failed on the datastore: the column '{column}' of a row of InfluxDB 3 is not {expected}"
    )
}

#[test]
fn a_schema_that_is_not_understood_is_reported() {
    for (row, column) in [
        (
            json!({"column_name": "a", "data_type": "Utf8"}),
            "table_name",
        ),
        (
            json!({"table_name": "t", "data_type": "Utf8"}),
            "column_name",
        ),
        (json!({"table_name": "t", "column_name": "a"}), "data_type"),
        (
            json!({"table_name": 1, "column_name": "a", "data_type": "x"}),
            "table_name",
        ),
    ] {
        assert_eq!(
            bad_rows("information_schema", &json!([row])),
            bad_column(column, "a text")
        );
    }
}

#[test]
fn a_series_that_is_not_understood_is_reported() {
    assert_eq!(
        bad_rows("SELECT DISTINCT \"dc\"", &json!([{"dc": 5}])),
        bad_column("dc", "a text")
    );
}

#[test]
fn attributes_that_are_not_understood_are_reported() {
    let row =
        json!({"class": "cpu", "labels": "{}", "attributes": "{}", "deleted": false, "ts": 1});
    let without = |column: &str| {
        let mut row = row.clone();
        row.as_object_mut().expect("an object").remove(column);
        row
    };
    let with = |column: &str, value: Json| {
        let mut row = row.clone();
        row[column] = value;
        row
    };
    for (bad, column, expected) in [
        (without("class"), "class", "a text"),
        (without("labels"), "labels", "a text"),
        (without("attributes"), "attributes", "a text"),
        (with("deleted", json!(1)), "deleted", "a boolean"),
        (without("ts"), "ts", "an integer"),
        (with("ts", json!("1")), "ts", "an integer"),
    ] {
        assert_eq!(
            bad_rows(ATTRIBUTES_QUERY, &json!([bad])),
            bad_column(column, expected),
            "{column}"
        );
    }
    for attributes in ["{", "[1]", "{\"k\":1}"] {
        let err = bad_rows(
            ATTRIBUTES_QUERY,
            &json!([with("attributes", json!(attributes))]),
        );
        assert!(
            err.starts_with("find failed on the datastore: InfluxDB 3 sent invalid JSON: "),
            "{attributes}: {err}"
        );
    }
    // The labels of a series that only has attributes.
    let err = bad_rows(
        ATTRIBUTES_QUERY,
        &json!([with("class", json!("ghost")), with("labels", json!("nope"))][1..]),
    );
    assert!(err.contains("InfluxDB 3 sent invalid JSON"), "{err}");
}

/// `cpu` with one series, `dc=eu,host=a`.
fn cpu_fake() -> Fake {
    let fake = Fake::standard(true);
    fake.answer(
        "SELECT DISTINCT \"dc\", \"host\" FROM \"cpu\"",
        &json!([{"dc": "eu", "host": "a"}]),
    );
    fake
}

fn fetch(fake: &Fake, request: &FetchRequest) -> Vec<GeoTimeSerie> {
    store(fake).fetch(request).expect("series")
}

fn points(gts: &GeoTimeSerie) -> Vec<(i64, Value)> {
    (0..gts.size())
        .map(|index| (gts.tick_at(index), gts.value_at(index)))
        .collect()
}

fn ticks(series: &[GeoTimeSerie]) -> Vec<Vec<i64>> {
    series.iter().map(|gts| gts.ticks.clone()).collect()
}

/// The rows of `count` longs, the most recent first: the ticks `10 * count` down to `10`, each
/// with its rank as the value.
fn longs(count: i64) -> Json {
    Json::Array(
        (1..=count)
            .rev()
            .map(|tick| json!({"ts": tick * 10_000, "value_type": "l", "value_l": tick}))
            .collect(),
    )
}

const POINTS: &str = "SELECT CAST(time AS BIGINT) AS ts, \"value_type\", \"value_l\", \"value_d\", \"value_b\", \"value_s\", \"hhcode\", \"elevation\" FROM \"cpu\"";

#[test]
fn a_fetch_reads_the_datapoints_of_a_series_in_a_range() {
    let fake = cpu_fake();
    fake.answer(
        "ORDER BY time DESC",
        &json!([
            {"ts": 20_000, "value_type": "l", "value_l": 2, "hhcode": 7, "elevation": -3},
            {"ts": 10_999, "value_type": "l", "value_l": 1},
        ]),
    );
    let fetched = fetch(
        &fake,
        &request(vec![selector("cpu", &[("host", "a")])], 10, 20),
    );
    assert_eq!(fetched.len(), 1);
    let gts = &fetched[0];
    assert_eq!(described(&gts.metadata), "cpu{dc=eu,host=a}{}");
    assert_eq!(points(gts), [(20, Value::Long(2)), (10, Value::Long(1))]);
    assert_eq!((gts.location_at(0), gts.elevation_at(0)), (7, -3));
    assert_eq!(
        (gts.location_at(1), gts.elevation_at(1)),
        (NO_LOCATION, NO_ELEVATION)
    );
    let queries = fake.queries();
    assert_eq!(queries.len(), 4, "{queries:?}");
    assert_eq!(
        queries[3],
        format!(
            "{POINTS} WHERE \"dc\" = 'eu' AND \"host\" = 'a' AND time >= arrow_cast(10000, 'Timestamp(Nanosecond, None)') AND time <= arrow_cast(20999, 'Timestamp(Nanosecond, None)') ORDER BY time DESC"
        )
    );
}

#[test]
fn the_datapoints_of_every_type_are_read() {
    for (row, expected) in [
        (
            json!({"ts": 1000, "value_type": "d", "value_d": 1.5}),
            Value::Double(1.5),
        ),
        (
            json!({"ts": 1000, "value_type": "d", "value_d": 2}),
            Value::Double(2.0),
        ),
        (
            json!({"ts": 1000, "value_type": "b", "value_b": true}),
            Value::Boolean(true),
        ),
        (
            json!({"ts": 1000, "value_type": "s", "value_s": "a\nb"}),
            Value::from("a\nb"),
        ),
    ] {
        let fake = cpu_fake();
        fake.answer("ORDER BY time DESC", &json!([row]));
        let fetched = fetch(&fake, &request(vec![selector("cpu", &[])], 0, 9));
        assert_eq!(points(&fetched[0]), [(1, expected)]);
    }
}

#[test]
fn a_tick_is_the_microsecond_a_time_is_in() {
    let fake = cpu_fake();
    fake.answer(
        "ORDER BY time DESC",
        &json!([
            {"ts": 1999, "value_type": "l", "value_l": 3},
            {"ts": 1000, "value_type": "l", "value_l": 2},
            {"ts": -1, "value_type": "l", "value_l": 1},
            {"ts": -1000, "value_type": "l", "value_l": 0},
            {"ts": -1001, "value_type": "l", "value_l": -1},
        ]),
    );
    let fetched = fetch(&fake, &request(vec![selector("cpu", &[])], -5, 5));
    assert_eq!(ticks(&fetched), [[1, 1, -1, -1, -2]]);
}

#[test]
fn the_bounds_of_a_range_are_saturated() {
    let fake = cpu_fake();
    fetch(
        &fake,
        &request(vec![selector("cpu", &[])], i64::MIN, i64::MAX),
    );
    let query = fake.queries().pop().expect("a query");
    assert!(
        query.contains(&format!(
            "time >= arrow_cast({}, 'Timestamp(Nanosecond, None)') AND time <= arrow_cast({}, 'Timestamp(Nanosecond, None)')",
            i64::MIN,
            i64::MAX
        )),
        "{query}"
    );
}

#[test]
fn a_series_that_lacks_a_label_is_read_by_its_null_tag() {
    let fake = Fake::standard(true);
    fake.answer(
        "SELECT DISTINCT \"dc\", \"host\" FROM \"cpu\"",
        &json!([{"host": "c"}]),
    )
    .answer("ORDER BY time DESC", &longs(1));
    let fetched = fetch(&fake, &request(vec![selector("cpu", &[])], 0, 9));
    assert_eq!(described(&fetched[0].metadata), "cpu{host=c}{}");
    let query = fake.queries().pop().expect("a query");
    assert!(
        query.contains(" WHERE \"dc\" IS NULL AND \"host\" = 'c' AND time >= "),
        "{query}"
    );
}

#[test]
fn a_class_without_tags_and_columns_is_read_with_what_it_has() {
    let fake = Fake::standard(true);
    fake.answer(
        "SELECT DISTINCT 1 AS present FROM \"mem\"",
        &json!([{"present": 1}]),
    )
    .answer("ORDER BY time DESC", &longs(1));
    let fetched = fetch(&fake, &request(vec![selector("mem", &[])], 0, 9));
    assert_eq!(points(&fetched[0]), [(10, Value::Long(1))]);
    let query = fake.queries().pop().expect("a query");
    assert!(
        query.starts_with(
            "SELECT CAST(time AS BIGINT) AS ts, \"value_type\", \"value_l\" FROM \"mem\" WHERE time >= "
        ),
        "{query}"
    );
}

#[test]
fn the_datapoints_are_thinned_as_the_request_says() {
    let thinned = |skip: i64, step: i64, timestep: Option<i64>, count: Option<i64>| {
        let fake = cpu_fake();
        fake.answer("ORDER BY time DESC", &longs(5));
        let fetched = fetch(
            &fake,
            &FetchRequest {
                skip,
                step,
                timestep,
                count,
                ..request(vec![selector("cpu", &[])], 0, 100)
            },
        );
        let query = fake.queries().pop().expect("a query");
        (ticks(&fetched), query)
    };
    let (ticks, query) = thinned(1, 1, None, Some(2));
    assert_eq!(ticks, [[40, 30]]);
    assert!(query.ends_with(" ORDER BY time DESC LIMIT 3"), "{query}");
    let (ticks, query) = thinned(0, 1, None, None);
    assert_eq!(ticks, [[50, 40, 30, 20, 10]]);
    assert!(query.ends_with(" ORDER BY time DESC"), "{query}");
    // A step or a time step needs more datapoints than the count: no limit.
    let (ticks, query) = thinned(1, 2, None, Some(2));
    assert_eq!(ticks, [[40, 20]]);
    assert!(query.ends_with(" ORDER BY time DESC"), "{query}");
    let (ticks, query) = thinned(0, 1, Some(20), Some(2));
    assert_eq!(ticks, [[50, 30]]);
    assert!(query.ends_with(" ORDER BY time DESC"), "{query}");
    let (ticks, query) = thinned(0, 1, None, Some(0));
    assert_eq!(ticks.len(), 0);
    assert!(query.ends_with(" ORDER BY time DESC LIMIT 0"), "{query}");
}

#[test]
fn the_datapoints_around_the_range_are_the_boundaries() {
    let fake = cpu_fake();
    fake.answer(
        "time > arrow_cast(",
        &json!([
            {"ts": 21_000, "value_type": "l", "value_l": 21},
            {"ts": 22_000, "value_type": "l", "value_l": 22},
        ]),
    )
    .answer(
        "time < arrow_cast(",
        &json!([{"ts": 5000, "value_type": "l", "value_l": 5}]),
    )
    .answer("ORDER BY time DESC", &longs(2));
    let fetched = fetch(
        &fake,
        &FetchRequest {
            boundary_post: 2,
            boundary_pre: 1,
            ..request(vec![selector("cpu", &[])], 10, 20)
        },
    );
    assert_eq!(ticks(&fetched), [[22, 21, 20, 10, 5]]);
    let queries = fake.queries();
    assert_eq!(queries.len(), 6, "{queries:?}");
    assert!(
        queries[4].ends_with(
            " AND time > arrow_cast(20999, 'Timestamp(Nanosecond, None)') ORDER BY time ASC LIMIT 2"
        ),
        "{}",
        queries[4]
    );
    assert!(
        queries[5].ends_with(" AND time < arrow_cast(10000, 'Timestamp(Nanosecond, None)') ORDER BY time DESC LIMIT 1"),
        "{}",
        queries[5]
    );
}

#[test]
fn only_the_boundaries_asked_for_are_read() {
    let fake = cpu_fake();
    fake.answer("ORDER BY time DESC", &longs(1));
    fetch(
        &fake,
        &FetchRequest {
            boundary_pre: 1,
            ..request(vec![selector("cpu", &[])], 0, 100)
        },
    );
    assert_eq!(fake.queries().len(), 5);
    let fake = cpu_fake();
    fake.answer("ORDER BY time DESC", &longs(1));
    fetch(
        &fake,
        &FetchRequest {
            boundary_post: 1,
            ..request(vec![selector("cpu", &[])], 0, 100)
        },
    );
    assert_eq!(fake.queries().len(), 5);
}

#[test]
fn the_datapoints_of_a_type_are_read() {
    let fake = cpu_fake();
    fake.answer("ORDER BY time DESC", &longs(1));
    fetch(
        &fake,
        &FetchRequest {
            value_type: Some(Type::Long),
            ..request(vec![selector("cpu", &[])], 0, 100)
        },
    );
    let query = fake.queries().pop().expect("a query");
    assert!(
        query.contains(" AND \"value_type\" = 'l' AND time >= "),
        "{query}"
    );
    // A type that does not exist has no datapoint, and needs no query.
    let fake = cpu_fake();
    let fetched = fetch(
        &fake,
        &FetchRequest {
            value_type: Some(Type::Undefined),
            keep_empty: true,
            ..request(vec![selector("cpu", &[])], 0, 100)
        },
    );
    assert_eq!(ticks(&fetched), [Vec::<i64>::new()]);
    assert_eq!(fake.queries().len(), 3);
}

#[test]
fn a_series_without_datapoints_is_fetched_when_asked_for() {
    let fake = cpu_fake();
    let wanted = |keep_empty: bool| {
        fetch(
            &fake,
            &FetchRequest {
                keep_empty,
                ..request(vec![selector("cpu", &[])], 0, 100)
            },
        )
        .len()
    };
    assert_eq!(wanted(false), 0);
    assert_eq!(wanted(true), 1);
}

#[test]
fn the_series_fetched_are_a_page_of_the_series_found() {
    let fake = found_fake();
    fake.answer("ORDER BY time DESC", &longs(1));
    let page = |series_skip: i64, series_count: i64| {
        fetch(
            &fake,
            &FetchRequest {
                series_skip,
                series_count,
                ..request(vec![selector("cpu", &[])], 0, 100)
            },
        )
        .iter()
        .map(|gts| described(&gts.metadata))
        .collect::<Vec<_>>()
    };
    assert_eq!(page(0, i64::MAX).len(), 3);
    assert_eq!(page(1, 1), ["cpu{dc=us,host=b}{}"]);
    assert_eq!(page(2, 5), ["cpu{host=c}{}"]);
    assert_eq!(page(3, 5).len(), 0);
    assert_eq!(page(0, 0).len(), 0);
    assert_eq!(page(-1, -1).len(), 0);
}

#[test]
fn a_series_without_a_table_or_a_column_has_no_datapoints() {
    let fake = found_fake();
    fake.answer("ATTRIBUTES", &json!([]));
    {
        let mut state = fake.state();
        let attributes = state
            .answers
            .iter_mut()
            .find(|(fragment, _)| fragment == ATTRIBUTES_QUERY)
            .expect("the attributes");
        attributes.1.push(
            rows(&json!([{
                "class": "cpu", "labels": "{\"zone\":\"z\"}", "attributes": "{}", "ts": 9
            }]))[0]
                .clone(),
        );
    }
    let fetched = fetch(
        &fake,
        &FetchRequest {
            keep_empty: true,
            ..request(
                vec![
                    selector("~.*", &[("x", "~.*")]),
                    selector("ghost", &[]),
                    selector("cpu", &[("zone", "z")]),
                ],
                0,
                100,
            )
        },
    );
    // `ghost{x=1}` has no table, `cpu{zone=z}` a label that has no column.
    assert_eq!(
        fetched
            .iter()
            .map(|gts| described(&gts.metadata))
            .collect::<Vec<_>>(),
        ["cpu{zone=z}{}", "ghost{x=1}{k=v}"]
    );
    assert!(
        !fake
            .queries()
            .iter()
            .any(|query| query.contains("CAST(time AS BIGINT) AS ts, \"value_type\"")),
        "{:?}",
        fake.queries()
    );
}

#[test]
fn a_series_without_a_class_has_no_datapoints() {
    let fake = cpu_fake();
    let datastore = store(&fake);
    let schema = datastore.schema().expect("a schema");
    let request = request(vec![selector("cpu", &[])], 0, 100);
    assert_eq!(
        datastore
            .datapoints(&schema, &Metadata::default(), &request)
            .expect("no datapoints")
            .len(),
        0
    );
    assert_eq!(series_key_of(&Metadata::default()), None);
    assert_eq!(
        series_key_of(&metadata("c", &[("b", "2"), ("a", "1")], &[("k", "v")])),
        Some((
            "c".to_owned(),
            vec![
                ("a".to_owned(), "1".to_owned()),
                ("b".to_owned(), "2".to_owned())
            ]
        ))
    );
}

fn bad_fetch(row: &Json) -> String {
    let fake = cpu_fake();
    fake.answer("ORDER BY time DESC", &json!([row]));
    message(store(&fake).fetch(&request(vec![selector("cpu", &[])], 0, 100)))
}

fn bad_datapoint(column: &str, expected: &str) -> String {
    format!(
        "fetch failed on the datastore: the column '{column}' of a row of InfluxDB 3 is not {expected}"
    )
}

#[test]
fn a_datapoint_that_is_not_understood_is_reported() {
    for (row, column, expected) in [
        (json!({"value_type": "l", "value_l": 1}), "ts", "an integer"),
        (
            json!({"ts": "1", "value_type": "l", "value_l": 1}),
            "ts",
            "an integer",
        ),
        (json!({"ts": 1}), "value_type", "a text"),
        (json!({"ts": 1, "value_type": 1}), "value_type", "a text"),
        (
            json!({"ts": 1, "value_type": "x"}),
            "value_type",
            "one of l, d, b, s",
        ),
        (json!({"ts": 1, "value_type": "l"}), "value_l", "an integer"),
        (
            json!({"ts": 1, "value_type": "l", "value_l": 1.5}),
            "value_l",
            "an integer",
        ),
        (json!({"ts": 1, "value_type": "d"}), "value_d", "a number"),
        (
            json!({"ts": 1, "value_type": "d", "value_d": "x"}),
            "value_d",
            "a number",
        ),
        (json!({"ts": 1, "value_type": "b"}), "value_b", "a boolean"),
        (
            json!({"ts": 1, "value_type": "b", "value_b": 1}),
            "value_b",
            "a boolean",
        ),
        (json!({"ts": 1, "value_type": "s"}), "value_s", "a text"),
        (
            json!({"ts": 1, "value_type": "s", "value_s": 1}),
            "value_s",
            "a text",
        ),
        (
            json!({"ts": 1, "value_type": "l", "value_l": 1, "hhcode": "x"}),
            "hhcode",
            "an integer",
        ),
        (
            json!({"ts": 1, "value_type": "l", "value_l": 1, "elevation": "x"}),
            "elevation",
            "an integer",
        ),
    ] {
        assert_eq!(bad_fetch(&row), bad_datapoint(column, expected), "{row}");
    }
}

#[test]
fn a_failing_request_fails_the_fetch() {
    let prepare = || {
        let fake = cpu_fake();
        fake.answer("time > arrow_cast(", &longs(1))
            .answer("time < arrow_cast(", &longs(1))
            .answer("ORDER BY time DESC", &longs(1));
        fake
    };
    let requests = every_request_fails(prepare, |store| {
        store.fetch(&FetchRequest {
            boundary_pre: 1,
            boundary_post: 1,
            ..request(vec![selector("cpu", &[])], 0, 100)
        })
    });
    assert_eq!(requests, 6);
}

fn long_series(
    class: &str,
    labels: &[(&str, &str)],
    ticks: std::ops::RangeInclusive<i64>,
) -> GeoTimeSerie {
    let points: Vec<(i64, Value)> = ticks.map(|tick| (tick, Value::Long(tick))).collect();
    series_of(class, labels, &points)
}

#[test]
fn an_update_writes_the_datapoints_as_lines() {
    let fake = Fake::default();
    store(&fake)
        .update(&[
            long_series("cpu", &[("host", "a")], 1..=2),
            long_series("mem", &[], 3..=3),
        ])
        .expect("an update");
    let location = NO_LOCATION;
    let elevation = NO_ELEVATION;
    assert_eq!(
        fake.writes(),
        [format!(
            "cpu,host=a value_type=\"l\",value_l=1i,hhcode={location}i,elevation={elevation}i 1000\n\
             cpu,host=a value_type=\"l\",value_l=2i,hhcode={location}i,elevation={elevation}i 2000\n\
             mem value_type=\"l\",value_l=3i,hhcode={location}i,elevation={elevation}i 3000"
        )]
    );
    assert_eq!(fake.queries().len(), 0);
}

#[test]
fn an_update_without_datapoints_writes_nothing() {
    let fake = Fake::default();
    store(&fake).update(&[]).expect("an update");
    store(&fake)
        .update(&[series_of("cpu", &[], &[])])
        .expect("an update");
    assert_eq!(fake.writes().len(), 0);
}

#[test]
fn an_update_is_written_in_batches() {
    let fake = Fake::default();
    store(&fake)
        .update(&[long_series("cpu", &[], 1..=10_001)])
        .expect("an update");
    let writes = fake.writes();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].lines().count(), 10_000);
    assert_eq!(writes[1].lines().count(), 1);
    assert!(writes[1].ends_with(" 10001000"), "{}", writes[1]);
}

#[test]
fn what_cannot_be_written_fails_the_update() {
    let unnamed = || {
        let mut gts = long_series("cpu", &[], 1..=1);
        gts.metadata.name = None;
        gts
    };
    assert_eq!(
        message(store(&Fake::default()).update(&[unnamed()])),
        "Cannot write a Geo Time Series without a class name."
    );
    let mut empty = unnamed();
    empty.metadata.name = Some(String::new());
    assert_eq!(
        message(store(&Fake::default()).update(&[empty])),
        "Cannot write a Geo Time Series without a class name."
    );
    let fake = Fake::default();
    assert_eq!(
        message(store(&fake).update(&[long_series("cpu", &[("time", "x")], 1..=1)])),
        "update failed on the datastore: 'time' cannot be the name of a label stored in InfluxDB 3: it is a column of the datapoints"
    );
    assert_eq!(
        message(store(&fake).update(&[long_series("_influxftl_series", &[], 1..=1)])),
        "update failed on the datastore: '_influxftl_series' cannot be the name of a class stored in InfluxDB 3: it is the table of the attributes"
    );
    let not_finite = series_of("cpu", &[], &[(1, Value::Double(f64::NAN))]);
    assert!(
        message(store(&fake).update(&[not_finite])).starts_with("update failed on the datastore: ")
    );
    assert_eq!(fake.writes().len(), 0);
}

#[test]
fn a_failing_write_fails_the_update() {
    let requests = every_request_fails(Fake::default, |store| {
        store.update(&[long_series("cpu", &[], 1..=10_001)])
    });
    assert_eq!(requests, 2);
}

/// The attributes of `cpu{dc=eu,host=a}`: `keep` and `unit`, written at `ts`.
fn attributes_fake(ts: i64, deleted: bool) -> Fake {
    let fake = Fake::standard(true);
    fake.answer(
        ATTRIBUTES_QUERY,
        &json!([{
            "class": "cpu",
            "labels": "{\"dc\":\"eu\",\"host\":\"a\"}",
            "attributes": "{\"keep\":\"k\",\"unit\":\"%\"}",
            "deleted": deleted,
            "ts": ts,
        }]),
    );
    fake
}

fn attributes_line(attributes: &str, deleted: bool, time: i64) -> String {
    format!(
        "_influxftl_series,class=cpu,labels={{\"dc\":\"eu\"\\,\"host\":\"a\"}} attributes=\"{attributes}\",deleted={deleted} {time}"
    )
}

#[test]
fn attributes_are_merged_with_those_of_the_series() {
    let fake = attributes_fake(5, false);
    store(&fake)
        .update_metadata(
            &[metadata(
                "cpu",
                &[("host", "a"), ("dc", "eu")],
                &[("unit", "C"), ("new", "n"), ("keep", "")],
            )],
            true,
        )
        .expect("an update");
    assert_eq!(
        fake.writes(),
        [attributes_line(
            "{\\\"new\\\":\\\"n\\\",\\\"unit\\\":\\\"C\\\"}",
            false,
            1_000_000_000
        )]
    );
    let queries = fake.queries();
    assert_eq!(
        queries[1],
        format!("{ATTRIBUTES_QUERY} WHERE \"class\" IN ('cpu') ORDER BY time")
    );
}

#[test]
fn attributes_replace_those_of_the_series() {
    let fake = attributes_fake(5, false);
    store(&fake)
        .update_metadata(
            &[metadata(
                "cpu",
                &[("dc", "eu"), ("host", "a")],
                &[("new", "n")],
            )],
            false,
        )
        .expect("an update");
    assert_eq!(
        fake.writes(),
        [attributes_line(
            "{\\\"new\\\":\\\"n\\\"}",
            false,
            1_000_000_000
        )]
    );
}

#[test]
fn the_attributes_of_a_series_that_was_deleted_start_again() {
    let fake = attributes_fake(5, true);
    store(&fake)
        .update_metadata(
            &[metadata(
                "cpu",
                &[("dc", "eu"), ("host", "a")],
                &[("new", "n")],
            )],
            true,
        )
        .expect("an update");
    assert_eq!(
        fake.writes(),
        [attributes_line(
            "{\\\"new\\\":\\\"n\\\"}",
            false,
            1_000_000_000
        )]
    );
}

#[test]
fn a_new_row_of_attributes_is_after_the_ones_it_replaces() {
    // The row of the series is after the clock: the new one is the next nanosecond.
    let fake = attributes_fake(2_000_000_000, false);
    let same = metadata("cpu", &[("dc", "eu"), ("host", "a")], &[("n", "1")]);
    let other = metadata("cpu", &[("dc", "us")], &[("n", "2")]);
    store(&fake)
        .update_metadata(&[same.clone(), other, same], true)
        .expect("an update");
    let writes = fake.writes();
    assert_eq!(writes.len(), 1);
    let times: Vec<&str> = writes[0]
        .lines()
        .map(|line| line.rsplit(' ').next().expect("a time"))
        .collect();
    assert_eq!(times, ["2000000001", "2000000002", "2000000003"]);
}

#[test]
fn the_first_attributes_of_a_series_are_written_at_the_time_of_the_clock() {
    let fake = Fake::standard(false);
    store(&fake)
        .update_metadata(&[metadata("cpu", &[("host", "a")], &[("k", "v")])], true)
        .expect("an update");
    assert_eq!(fake.queries().len(), 1);
    assert_eq!(
        fake.writes(),
        [
            "_influxftl_series,class=cpu,labels={\"host\":\"a\"} attributes=\"{\\\"k\\\":\\\"v\\\"}\",deleted=false 1000000000"
        ]
    );
}

#[test]
fn attributes_of_a_series_that_cannot_be_stored_fail_the_update() {
    let fake = attributes_fake(5, false);
    let datastore = store(&fake);
    let mut unnamed = metadata("cpu", &[], &[("k", "v")]);
    unnamed.name = None;
    assert_eq!(
        message(datastore.update_metadata(&[unnamed], true)),
        "Cannot write a Geo Time Series without a class name."
    );
    assert_eq!(fake.queries().len(), 0);
    assert!(
        message(datastore.update_metadata(&[metadata("a\nb", &[], &[])], true))
            .starts_with("update metadata failed on the datastore: ")
    );
    assert_eq!(fake.writes().len(), 0);
}

#[test]
fn a_clock_that_is_out_of_range_fails_the_update_of_attributes() {
    let fake = attributes_fake(5, false);
    let datastore = InfluxDatastore::with_clock(Box::new(fake.clone()), || i64::MAX);
    assert_eq!(
        message(datastore.update_metadata(&[metadata("cpu", &[], &[])], true)),
        "update metadata failed on the datastore: the system clock cannot be read as nanoseconds since the epoch"
    );
    assert_eq!(fake.writes().len(), 0);
}

#[test]
fn a_failing_request_fails_the_update_of_attributes() {
    let requests = every_request_fails(
        || attributes_fake(5, false),
        |store| store.update_metadata(&[metadata("cpu", &[("host", "a")], &[("k", "v")])], true),
    );
    assert_eq!(requests, 3);
}

/// `cpu` with the series `dc=eu,host=a` (which has attributes) and `host=c`, and `ghost`, which
/// only has attributes.
fn delete_fake() -> Fake {
    let fake = Fake::standard(true);
    fake.answer(
        ATTRIBUTES_QUERY,
        &json!([
            {"class": "cpu", "labels": "{\"dc\":\"eu\",\"host\":\"a\"}", "attributes": "{\"unit\":\"%\"}", "deleted": false, "ts": 5},
            {"class": "ghost", "labels": "{\"x\":\"1\"}", "attributes": "{\"k\":\"v\"}", "ts": 6},
            {"class": "ghost", "labels": "{\"x\":\"2\"}", "attributes": "{}", "deleted": true, "ts": 7},
        ]),
    )
    .answer(
        "SELECT DISTINCT \"dc\", \"host\" FROM \"cpu\"",
        &json!([{"dc": "eu", "host": "a"}, {"host": "c"}]),
    );
    fake
}

fn delete(selectors: Vec<Selector>) -> DeleteRequest {
    DeleteRequest {
        selectors,
        range: None,
    }
}

#[test]
fn a_table_is_deleted_with_its_series() {
    let fake = delete_fake();
    let deleted = store(&fake)
        .delete(&delete(vec![selector("cpu", &[])]))
        .expect("a deletion");
    assert_eq!(
        deleted.iter().map(described).collect::<Vec<_>>(),
        ["cpu{dc=eu,host=a}{unit=%}", "cpu{host=c}{}"]
    );
    assert_eq!(fake.deletions(), ["cpu"]);
    // The series that had attributes is told deleted, the other has no row to tell it by.
    assert_eq!(fake.writes(), [attributes_line("{}", true, 1_000_000_000)]);
}

#[test]
fn a_series_that_only_has_attributes_is_deleted_by_a_tombstone() {
    let fake = delete_fake();
    let deleted = store(&fake)
        .delete(&delete(vec![selector("ghost", &[])]))
        .expect("a deletion");
    assert_eq!(
        deleted.iter().map(described).collect::<Vec<_>>(),
        ["ghost{x=1}{k=v}"]
    );
    assert_eq!(fake.deletions().len(), 0);
    assert_eq!(
        fake.writes(),
        [
            "_influxftl_series,class=ghost,labels={\"x\":\"1\"} attributes=\"{}\",deleted=true 1000000000"
        ]
    );
}

#[test]
fn a_series_that_was_deleted_is_not_deleted_again() {
    let fake = delete_fake();
    store(&fake)
        .delete(&delete(vec![selector("mem", &[])]))
        .expect("a deletion");
    assert_eq!(fake.writes().len(), 0);
    assert_eq!(fake.deletions().len(), 0);
}

#[test]
fn the_tombstones_are_after_the_rows_they_replace() {
    let fake = Fake::standard(true);
    fake.answer(
        ATTRIBUTES_QUERY,
        &json!([
            {"class": "ghost", "labels": "{\"x\":\"1\"}", "attributes": "{}", "ts": 4_000_000_000_i64},
            {"class": "ghost", "labels": "{\"x\":\"2\"}", "attributes": "{}", "ts": 5},
        ]),
    );
    store(&fake)
        .delete(&delete(vec![selector("ghost", &[])]))
        .expect("a deletion");
    let writes = fake.writes();
    let times: Vec<&str> = writes[0]
        .lines()
        .map(|line| line.rsplit(' ').next().expect("a time"))
        .collect();
    assert_eq!(times, ["4000000001", "4000000002"]);
}

#[test]
fn only_whole_tables_are_deleted() {
    let fake = delete_fake();
    let datastore = store(&fake);
    assert_eq!(
        message(datastore.delete(&delete(vec![selector("cpu", &[("host", "a")])]))),
        "delete failed on the datastore: InfluxDB 3 Core only deletes whole tables: a series of 'cpu' would be left in its table"
    );
    assert_eq!(fake.deletions().len(), 0);
    assert_eq!(fake.writes().len(), 0);
    let fake = delete_fake();
    assert_eq!(
        message(store(&fake).delete(&DeleteRequest {
            selectors: vec![selector("cpu", &[])],
            range: Some((1, 2)),
        })),
        "delete failed on the datastore: InfluxDB 3 Core only deletes whole tables: delete a whole series, with NULL NULL as the time range"
    );
    assert_eq!(fake.queries().len(), 0);
}

#[test]
fn a_clock_that_is_out_of_range_fails_the_deletion() {
    let fake = delete_fake();
    let datastore = InfluxDatastore::with_clock(Box::new(fake.clone()), || i64::MAX);
    assert_eq!(
        message(datastore.delete(&delete(vec![selector("cpu", &[])]))),
        "delete failed on the datastore: the system clock cannot be read as nanoseconds since the epoch"
    );
    assert_eq!(fake.deletions().len(), 0);
}

#[test]
fn a_class_that_cannot_be_written_fails_the_deletion() {
    let fake = Fake::standard(true);
    fake.answer(
        ATTRIBUTES_QUERY,
        &json!([{"class": "a\nb", "labels": "{}", "attributes": "{}", "ts": 5}]),
    );
    let err = message(store(&fake).delete(&delete(vec![selector("~.*", &[])])));
    assert!(err.starts_with("delete failed on the datastore: "), "{err}");
    assert_eq!(fake.deletions().len(), 0);
}

#[test]
fn a_failing_request_fails_the_deletion() {
    let requests = every_request_fails(delete_fake, |store| {
        store.delete(&delete(vec![selector("cpu", &[])]))
    });
    assert_eq!(requests, 7);
}

#[test]
fn a_class_must_be_named_to_be_stored() {
    assert_eq!(named(&metadata("cpu", &[], &[])).expect("a class"), "cpu");
    assert!(named(&Metadata::default()).is_err());
    assert!(named(&metadata("", &[], &[])).is_err());
}
