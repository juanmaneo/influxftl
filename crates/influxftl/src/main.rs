// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! `influxftl` — WarpScript / WarpLib for InfluxDB 3.

use std::io::{self, Write};

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    run(&mut io::stdout().lock())
}

/// Report the size of the function library on `out`.
fn run(out: &mut impl Write) -> anyhow::Result<()> {
    writeln!(
        out,
        "influxftl: {} functions registered",
        warpscript::functions::registered_names().len()
    )
    .context("writing to stdout")?;
    out.flush().context("flushing stdout")
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::run;

    /// A writer on a full device: it accepts the bytes only when `accepts` is set, and never flushes.
    struct Full {
        accepts: bool,
    }

    impl io::Write for Full {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.accepts {
                Ok(buf.len())
            } else {
                Err(io::Error::new(io::ErrorKind::StorageFull, "device full"))
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::StorageFull, "device full"))
        }
    }

    #[test]
    fn reports_the_function_count() {
        let mut out = Vec::new();
        run(&mut out).expect("writing to a vector cannot fail");
        let text = String::from_utf8(out).expect("the report is UTF-8");
        assert!(text.starts_with("influxftl: "), "{text}");
        assert!(text.ends_with(" functions registered\n"), "{text}");
    }

    #[test]
    fn a_failing_write_is_reported_with_its_context() {
        let err = run(&mut Full { accepts: false }).expect_err("the device is full");
        assert_eq!(err.to_string(), "writing to stdout");
        let source = err.source().expect("the I/O error is the source");
        assert_eq!(source.to_string(), "device full");
    }

    #[test]
    fn a_failing_flush_is_reported_with_its_context() {
        let err = run(&mut Full { accepts: true }).expect_err("the device is full");
        assert_eq!(err.to_string(), "flushing stdout");
        let source = err.source().expect("the I/O error is the source");
        assert_eq!(source.to_string(), "device full");
    }
}
