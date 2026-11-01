//! `influxftl` — WarpScript / WarpLib for InfluxDB 3.

fn main() {
    println!(
        "influxftl: {} functions registered",
        warpscript::functions::registered_names().len()
    );
}
