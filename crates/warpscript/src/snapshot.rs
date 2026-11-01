//! Rendering of values as WarpScript source (`SNAPSHOT`, macro `toString`).
//!
//! Mirrors `SNAPSHOT.addElement` in its non-readable form: every element is followed by a
//! space, strings are percent-encoded, containers are rebuilt with `[] … +!`, `{} … PUT` and
//! `() … +!`.

use crate::encoding::opb64_encode;
use crate::format::{java_double_to_string, url_encode};
use crate::value::Value;

/// Maximum nesting of containers before giving up (`SNAPSHOT.MAX_RECURSION_LEVEL`).
const MAX_RECURSION_LEVEL: usize = 1000;

/// WarpScript source that recreates `value`, followed by a space.
pub fn value_snapshot(value: &Value) -> String {
    let mut out = String::new();
    append(&mut out, value, 0);
    out
}

fn append(out: &mut String, value: &Value, depth: usize) {
    if depth >= MAX_RECURSION_LEVEL {
        out.push_str("/* Recursive data structures exceeded 1000 levels. */ ");
        return;
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
            out.push_str(&url_encode(s));
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
            out.push_str("[] ");
            for item in list.borrow().iter() {
                append(out, item, depth + 1);
                out.push_str("+! ");
            }
        }
        Value::Set(set) => {
            out.push_str("() ");
            for item in set.borrow().iter() {
                append(out, item, depth + 1);
                out.push_str("+! ");
            }
        }
        Value::Map(map) => {
            out.push_str("{} ");
            for (k, v) in map.borrow().iter() {
                append(out, v, depth + 1);
                append(out, k, depth + 1);
                out.push_str("PUT ");
            }
        }
        Value::Macro(m) => {
            out.push_str(&m.snapshot(true));
            out.push(' ');
        }
        Value::Function(f) => {
            out.push_str(&f.snapshot());
            out.push(' ');
        }
        Value::Mark => out.push_str("MARK "),
        Value::Context(_) => {
            out.push_str("/* Error while snapshoting element of type 'CONTEXT' */ ");
        }
    }
}
