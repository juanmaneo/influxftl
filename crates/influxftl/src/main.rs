// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `influxftl` — WarpScript / WarpLib for InfluxDB 3.

fn main() {
    println!(
        "influxftl: {} functions registered",
        warpscript::functions::registered_names().len()
    );
}
