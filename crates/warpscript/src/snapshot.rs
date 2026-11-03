// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Rendering of values as WarpScript source (`SNAPSHOT`, macro `toString`).
//!
//! Mirrors `SNAPSHOT.addElement`: every element is followed by a space; in the default form
//! strings are percent-encoded and containers are rebuilt with `[] … +!`, `{} … PUT` and
//! `() … +!`, in the *readable* form (`SNAPSHOTREADABLE`) strings only escape `%`, `'` and
//! control characters and containers use the `[ … ]`, `{ … }` and `( … )` syntax.

use indexmap::IndexMap;

use crate::encoding::opb64_encode;
use crate::error::{Error, Result};
use crate::format::{java_double_to_string, snapshot_encode_readable, url_encode};
use crate::geoxp;
use crate::gts::{self, GeoTimeSerie};
use crate::macros::{Macro, Statement};
use crate::value::Value;

/// Maximum nesting of elements before giving up (`SNAPSHOT.MAX_RECURSION_LEVEL`).
const MAX_RECURSION_LEVEL: usize = 16;

/// WarpScript source that recreates `value`, followed by a space. Structures nested too deep
/// are replaced by the comment `Macro.snapshot` emits in that case.
pub fn value_snapshot(value: &Value) -> String {
    let mut out = String::new();
    if append(&mut out, value, 1, false).is_err() {
        out = format!(
            "/* Error while snapshoting element of type '{}' */ ",
            value.type_name()
        );
    }
    out
}

/// `SNAPSHOT.addElement`: append the source of `value` (and a space) to `out`, in the
/// readable form when `readable` is set. Fails with `Recursive data structures exceeded 16
/// levels.` on structures nested too deep.
pub fn add_element(out: &mut String, value: &Value, readable: bool) -> Result<()> {
    append(out, value, 1, readable)
}

/// `Macro.snapshot(hideSecure)`: the source of the macro, with every statement that cannot
/// be rendered replaced by a comment.
pub fn macro_snapshot(m: &Macro, hide_secure: bool) -> String {
    macro_snapshot_at(m, hide_secure, 1)
}

fn macro_snapshot_at(m: &Macro, hide_secure: bool, depth: usize) -> String {
    let mut out = String::from("<% ");
    if m.is_secure() && hide_secure {
        out.push_str("/* Secure Macro */ ");
    } else {
        for stmt in &m.statements {
            match stmt {
                Statement::Push(Value::Macro(inner)) => {
                    out.push_str(&macro_snapshot_at(inner, hide_secure, depth));
                    out.push(' ');
                }
                Statement::Push(value) => {
                    let mut element = String::new();
                    if append(&mut element, value, depth, false).is_ok() {
                        out.push_str(&element);
                    } else {
                        out.push_str(&format!(
                            "/* Error while snapshoting element of type '{}' */ ",
                            value.type_name()
                        ));
                    }
                }
                Statement::Call(callable) => {
                    out.push_str(&callable.snapshot());
                    out.push(' ');
                }
            }
        }
    }
    out.push_str("%>");
    out
}

fn append(out: &mut String, value: &Value, depth: usize, readable: bool) -> Result<()> {
    if depth > MAX_RECURSION_LEVEL {
        return Err(Error::new(format!(
            "Recursive data structures exceeded {MAX_RECURSION_LEVEL} levels."
        )));
    }
    match value {
        Value::Null => out.push_str("NULL "),
        Value::Boolean(true) => out.push_str("true "),
        Value::Boolean(false) => out.push_str("false "),
        Value::Long(l) => {
            out.push_str(&l.to_string());
            out.push(' ');
        }
        Value::Double(d) => {
            out.push_str(&java_double_to_string(*d));
            out.push(' ');
        }
        Value::String(s) => {
            out.push('\'');
            if readable {
                out.push_str(&snapshot_encode_readable(s));
            } else {
                out.push_str(&url_encode(s));
            }
            out.push_str("' ");
        }
        Value::Bytes(b) => {
            out.push('\'');
            out.push_str(&opb64_encode(b));
            out.push_str("' OPB64-> ");
        }
        Value::BitSet(b) => {
            out.push('\'');
            out.push_str(&opb64_encode(&b.to_bytes()));
            out.push_str("' OPB64-> BYTESTOBITS ");
        }
        Value::List(list) => {
            if readable {
                out.push_str("[ ");
                for item in list.borrow().iter() {
                    append(out, item, depth + 1, true)?;
                }
                out.push_str("] ");
            } else {
                out.push_str("[] ");
                for item in list.borrow().iter() {
                    append(out, item, depth + 1, false)?;
                    out.push_str("+! ");
                }
            }
        }
        Value::Set(set) => {
            if readable {
                out.push_str("( ");
                for item in set.borrow().iter() {
                    append(out, item, depth + 1, true)?;
                }
                out.push_str(") ");
            } else {
                out.push_str("() ");
                for item in set.borrow().iter() {
                    append(out, item, depth + 1, false)?;
                    out.push_str("+! ");
                }
            }
        }
        Value::Map(map) => {
            if readable {
                out.push_str("{\n");
                for (k, v) in map.borrow().iter() {
                    append(out, k, depth + 1, true)?;
                    append(out, v, depth + 1, true)?;
                    out.push('\n');
                }
                out.push_str("} ");
            } else {
                out.push_str("{} ");
                for (k, v) in map.borrow().iter() {
                    append(out, v, depth + 1, false)?;
                    append(out, k, depth + 1, false)?;
                    out.push_str("PUT ");
                }
            }
        }
        Value::Macro(m) => {
            out.push_str(&macro_snapshot_at(m, true, depth + 1));
            out.push(' ');
        }
        Value::Function(f) => {
            out.push_str(&f.snapshot());
            out.push(' ');
        }
        Value::Mark => out.push_str("MARK "),
        // Unsupported types keep their stack level as an informative string.
        Value::Context(_) => out.push_str("'UNSUPPORTED:CONTEXT' "),
        Value::Counter(c) => {
            out.push_str("COUNTER ");
            if c.get() != 0 {
                out.push_str(&format!("{} COUNTERSET ", c.get()));
            }
        }
        // Until `WRAP` is available, a reconstruction of the series.
        Value::Gts(gts) => append_gts(out, &gts.borrow(), depth, readable)?,
    }
    Ok(())
}

/// `NEWGTS 'name' RENAME { labels } RELABEL { attributes } SETATTRIBUTES` followed by one
/// `tick lat lon elevation value ADDVALUE` per datapoint (`NaN` for the absent location and
/// elevation). The name, maps and values are rendered in the requested form, one level
/// deeper than the series like the items of a list.
fn append_gts(out: &mut String, gts: &GeoTimeSerie, depth: usize, readable: bool) -> Result<()> {
    let strings = |map: &IndexMap<String, String>| {
        Value::map(
            map.iter()
                .map(|(k, v)| (Value::from(k.as_str()), Value::from(v.as_str())))
                .collect(),
        )
    };
    // The source preceding each element, and the element.
    let mut parts = vec![
        (
            String::from("NEWGTS "),
            Value::from(gts.name().unwrap_or_default()),
        ),
        (String::from("RENAME "), strings(&gts.metadata.labels)),
        (String::from("RELABEL "), strings(&gts.metadata.attributes)),
    ];
    for i in 0..gts.size() {
        let mut prefix = String::from(if i == 0 {
            "SETATTRIBUTES "
        } else {
            "ADDVALUE "
        });
        prefix.push_str(&format!("{} ", gts.tick_at(i)));
        let location = gts.location_at(i);
        if location == gts::NO_LOCATION {
            prefix.push_str("NaN NaN ");
        } else {
            let (lat, lon) = geoxp::from_geoxp_point(location);
            prefix.push_str(&format!(
                "{} {} ",
                java_double_to_string(lat),
                java_double_to_string(lon)
            ));
        }
        let elevation = gts.elevation_at(i);
        if elevation == gts::NO_ELEVATION {
            prefix.push_str("NaN ");
        } else {
            prefix.push_str(&format!("{elevation} "));
        }
        parts.push((prefix, gts.value_at(i)));
    }
    for (prefix, value) in &parts {
        out.push_str(prefix);
        append(out, value, depth + 1, readable)?;
    }
    out.push_str(if gts.size() == 0 {
        "SETATTRIBUTES "
    } else {
        "ADDVALUE "
    });
    Ok(())
}
