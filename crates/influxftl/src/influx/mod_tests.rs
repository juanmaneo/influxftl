// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the connection of the datastore.

use std::ffi::OsString;
use std::time::Duration;

use super::{InfluxError, connect};

/// An environment of the given settings.
fn environment(
    settings: &'static [(&'static str, &'static str)],
) -> impl Fn(&str) -> Option<OsString> {
    move |name| {
        settings
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

fn failure(settings: &'static [(&'static str, &'static str)]) -> InfluxError {
    connect(&environment(settings), Duration::from_secs(1)).expect_err("a connection that fails")
}

#[test]
fn the_database_is_all_a_connection_needs() {
    connect(
        &environment(&[("INFLUXDB3_DATABASE_NAME", "warp")]),
        Duration::from_secs(1),
    )
    .expect("a datastore");
    connect(
        &environment(&[
            ("INFLUXDB3_DATABASE_NAME", "warp"),
            ("INFLUXDB3_HOST_URL", "http://db:8181/prefix"),
            ("INFLUXDB3_AUTH_TOKEN", "apiv3_token"),
        ]),
        Duration::from_secs(1),
    )
    .expect("a datastore");
}

#[test]
fn a_missing_database_is_reported() {
    let err = failure(&[]);
    assert!(
        matches!(
            err,
            InfluxError::MissingSetting {
                name: "INFLUXDB3_DATABASE_NAME"
            }
        ),
        "{err}"
    );
    assert_eq!(
        err.to_string(),
        "INFLUXDB3_DATABASE_NAME must be set to connect InfluxDB 3"
    );
}

#[test]
fn a_setting_that_is_not_valid_is_reported() {
    let err = failure(&[
        ("INFLUXDB3_DATABASE_NAME", "d"),
        ("INFLUXDB3_HOST_URL", "https://db"),
    ]);
    assert!(matches!(err, InfluxError::InvalidUrl { .. }), "{err}");
    let err = failure(&[
        ("INFLUXDB3_DATABASE_NAME", "d"),
        ("INFLUXDB3_AUTH_TOKEN", "a\nb"),
    ]);
    assert!(matches!(err, InfluxError::InvalidToken), "{err}");
}

#[cfg(unix)]
#[test]
fn a_setting_that_is_not_unicode_is_reported() {
    use std::os::unix::ffi::OsStringExt;

    for broken in [
        "INFLUXDB3_HOST_URL",
        "INFLUXDB3_DATABASE_NAME",
        "INFLUXDB3_AUTH_TOKEN",
    ] {
        let err = connect(
            &|name| {
                if name == broken {
                    Some(OsString::from_vec(vec![0xff]))
                } else {
                    Some(OsString::from("http://db"))
                }
            },
            Duration::from_secs(1),
        )
        .expect_err("a setting that is not UTF-8");
        assert!(
            matches!(&err, InfluxError::NotUnicode { name } if *name == broken),
            "{broken}: {err}"
        );
        assert_eq!(err.to_string(), format!("{broken} is not valid UTF-8"));
    }
}
