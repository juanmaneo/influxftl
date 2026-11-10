// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The datastore against a running InfluxDB 3 Core, which a test needs to be told about:
//!
//! ```sh
//! podman run -d --name idb3 -p 127.0.0.1:18181:8181 docker.io/library/influxdb:3-core \
//!     influxdb3 serve --node-id node0 --object-store memory --without-auth
//! INFLUXFTL_TEST_INFLUXDB3_URL=http://127.0.0.1:18181 cargo test -p influxftl --test influxdb3
//! ```
//!
//! Without `INFLUXFTL_TEST_INFLUXDB3_URL` the test says it is skipped: the coverage and the
//! other tests of the crate do not depend on a server.

use std::ffi::OsString;
use std::time::Duration;

use influxftl::influx::{InfluxDatastore, InfluxError};
use warpscript::Value;
use warpscript::datastore::{Datastore, DeleteRequest, FetchRequest, Selector};
use warpscript::gts::{GeoTimeSerie, Metadata, NO_ELEVATION, NO_LOCATION};

/// The datastore on the server the environment names, or nothing.
fn connected() -> Result<Option<InfluxDatastore>, InfluxError> {
    let Some(url) = std::env::var_os("INFLUXFTL_TEST_INFLUXDB3_URL") else {
        return Ok(None);
    };
    let lookup = move |name: &str| match name {
        "INFLUXDB3_HOST_URL" => Some(url.clone()),
        "INFLUXDB3_DATABASE_NAME" => Some(OsString::from("influxftl_it")),
        _ => std::env::var_os(name),
    };
    influxftl::influx::connect(&lookup, Duration::from_secs(30)).map(Some)
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

fn series(class: &str, host: &str, values: &[(i64, Value)]) -> GeoTimeSerie {
    let mut gts = GeoTimeSerie::new();
    gts.metadata.name = Some(class.to_owned());
    gts.metadata.labels = [("host".to_owned(), host.to_owned())].into_iter().collect();
    for (tick, value) in values {
        gts.set_value(*tick, NO_LOCATION, NO_ELEVATION, value, false);
    }
    gts
}

fn request(class: &str, end: i64, count: Option<i64>) -> FetchRequest {
    FetchRequest {
        selectors: vec![selector(class, &[])],
        start: 0,
        end,
        count,
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

#[test]
fn a_series_is_written_read_described_and_deleted() {
    let Some(store) = connected().expect("a datastore on the server") else {
        eprintln!("INFLUXFTL_TEST_INFLUXDB3_URL is not set: skipped");
        return;
    };
    // The class is new for every run, whatever the previous ones left in the database.
    let class = format!("it_{}", std::process::id());
    let points: Vec<(i64, Value)> = (1..=5).map(|tick| (tick, Value::Long(tick * 10))).collect();
    store
        .update(&[
            series(&class, "a", &points),
            series(&class, "b", &[(2, Value::Long(7))]),
        ])
        .expect("an update");

    let fetched = store.fetch(&request(&class, 10, Some(3))).expect("a fetch");
    assert_eq!(fetched.len(), 2);
    assert_eq!(fetched[0].ticks, [5, 4, 3]);
    assert_eq!(fetched[1].ticks, [2]);

    // A write at an existing tick replaces the datapoint.
    store
        .update(&[series(&class, "b", &[(2, Value::from("text"))])])
        .expect("an update");
    let fetched = store.fetch(&request(&class, 10, None)).expect("a fetch");
    assert_eq!(fetched[1].value_at(0), Value::from("text"));

    let meta = Metadata {
        attributes: [("unit".to_owned(), "m".to_owned())].into_iter().collect(),
        ..series(&class, "a", &[]).metadata
    };
    store.update_metadata(&[meta], true).expect("a META");
    let found = store
        .find(&[selector(&class, &[("unit", "m")])])
        .expect("a find");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].labels.get("host").map(String::as_str), Some("a"));

    let partial = store.delete(&DeleteRequest {
        selectors: vec![selector(&class, &[("host", "a")])],
        range: None,
    });
    assert!(partial.is_err(), "the table would be left with a series");
    let deleted = store
        .delete(&DeleteRequest {
            selectors: vec![selector(&class, &[])],
            range: None,
        })
        .expect("a deletion");
    assert_eq!(deleted.len(), 2);
    assert_eq!(
        store.find(&[selector(&class, &[])]).expect("a find").len(),
        0
    );
}
