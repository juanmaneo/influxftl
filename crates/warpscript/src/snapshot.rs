// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Rendering of values as WarpScript source (`SNAPSHOT`, macro `toString`).
//!
//! Mirrors `SNAPSHOT.addElement`: every element is followed by a space; in the default form
//! strings are percent-encoded and containers are rebuilt with `[] … +!`, `{} … PUT` and
//! `() … +!`, in the *readable* form (`SNAPSHOTREADABLE`) strings only escape `%`, `'` and
//! control characters and containers use the `[ … ]`, `{ … }` and `( … )` syntax.

use crate::ec::Key;
use crate::encoder::{DEFAULT_COMP_RATIO_THRESHOLD, encoder_to_wrapper, gts_to_wrapper};
use crate::encoding::opb64_encode;
use crate::error::Result;
use crate::error::platform::PlatformError;
use crate::format::{double_to_text, snapshot_encode_readable, url_encode};
use crate::macros::{Macro, Statement};
use crate::openpgp::key::key_id_hex;
use crate::rsa::RsaKey;
use crate::thrift::serialize_wrapper;
use crate::value::Value;

/// Maximum nesting of elements before giving up (`SNAPSHOT.MAX_RECURSION_LEVEL`).
pub(crate) const MAX_RECURSION_LEVEL: usize = 16;

/// WarpScript source that recreates `value`, followed by a space. Structures nested too deep
/// are replaced by the comment `Macro.snapshot` emits in that case.
pub fn value_snapshot(value: &Value) -> String {
    let mut out = String::new();
    match append(&mut out, value, 1, false) {
        Ok(()) => out,
        // Warp 10 writes this comment in place of a value it cannot snapshot, and the script
        // reads it, so the failure is reported as text here by design.
        Err(_) => format!(
            "/* Error while snapshoting element of type '{}' */ ",
            value.type_name()
        ),
    }
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
                    match append(&mut element, value, depth, false) {
                        Ok(()) => out.push_str(&element),
                        // The placeholder of `value_snapshot`, see there.
                        Err(_) => out.push_str(&format!(
                            "/* Error while snapshoting element of type '{}' */ ",
                            value.type_name()
                        )),
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

/// The source of a string literal, followed by a space. A string never fails to render.
pub fn string_element(out: &mut String, text: &str, readable: bool) {
    out.push('\'');
    if readable {
        out.push_str(&snapshot_encode_readable(text));
    } else {
        out.push_str(&url_encode(text));
    }
    out.push_str("' ");
}

fn append(out: &mut String, value: &Value, depth: usize, readable: bool) -> Result<()> {
    if depth > MAX_RECURSION_LEVEL {
        return Err(PlatformError::RecursiveStructure.into());
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
            out.push_str(&double_to_text(*d));
            out.push(' ');
        }
        // `[] [] 1.0 +! 2.0 +! +! … ->MAT` / `[] 1.0 +! 2.0 +! ->VEC`.
        Value::Matrix(m) => {
            out.push_str("[] ");
            for r in 0..m.rows {
                out.push_str("[] ");
                for c in 0..m.cols {
                    out.push_str(&double_to_text(m.get(r, c)));
                    out.push_str(" +! ");
                }
                out.push_str("+! ");
            }
            out.push_str("->MAT ");
        }
        Value::Vector(v) => {
            out.push_str("[] ");
            for x in v.iter() {
                out.push_str(&double_to_text(*x));
                out.push_str(" +! ");
            }
            out.push_str("->VEC ");
        }
        Value::BigDecimal(bd) => {
            out.push('\'');
            out.push_str(&bd.to_plain_string());
            out.push_str("' ->BD ");
        }
        Value::String(s) => string_element(out, s, readable),
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
        Value::Graphics(_) | Value::Image(_) | Value::Shape(_) | Value::Font(_) => {
            out.push_str("'UNSUPPORTED:");
            out.push_str(value.type_name());
            out.push_str("' ");
        }
        Value::Counter(c) => {
            out.push_str("COUNTER ");
            if c.get() != 0 {
                out.push_str(&format!("{} COUNTERSET ", c.get()));
            }
        }
        // `'…' UNWRAP`, the compressed `WRAP` of the series.
        Value::Gts(gts) => {
            let wrapper = gts_to_wrapper(
                &gts.borrow(),
                true,
                DEFAULT_COMP_RATIO_THRESHOLD,
                false,
                true,
            );
            out.push('\'');
            out.push_str(&opb64_encode(&serialize_wrapper(&wrapper)));
            out.push_str("' UNWRAP ");
        }
        // `'…' UNWRAPENCODER`, the `WRAP` of the encoder.
        Value::Encoder(encoder) => {
            let wrapper =
                encoder_to_wrapper(&encoder.borrow(), true, DEFAULT_COMP_RATIO_THRESHOLD, true);
            out.push('\'');
            out.push_str(&opb64_encode(&serialize_wrapper(&wrapper)));
            out.push_str("' UNWRAPENCODER ");
        }
        // `'…' GEOUNPACK`, the `GEOPACK` of the shape.
        Value::GeoShape(shape) => {
            out.push('\'');
            out.push_str(&shape.pack());
            out.push_str("' GEOUNPACK ");
        }
        // `'…' OPB64-> PGPRING 0 GET`: the ring of its encoded packets.
        Value::PgpRing(ring) => {
            out.push('\'');
            out.push_str(&opb64_encode(&ring.encode()));
            out.push_str("' OPB64-> PGPRING 0 GET ");
        }
        // `'…' OPB64-> PGPPUBLIC '<KEYID>' GET 'key' GET`: the armored key of its packets.
        Value::PgpKey(key) => {
            out.push('\'');
            out.push_str(&opb64_encode(&key.encode()));
            out.push_str("' OPB64-> PGPPUBLIC '");
            out.push_str(&key_id_hex(key.public.key_id));
            out.push_str("' GET 'key' GET ");
        }
        // The parameter maps `ECPRIVATE` and `ECPUBLIC` read.
        Value::Key(key) => match key.as_ref() {
            Key::Private(private) => {
                out.push_str("{ 'curve' '");
                out.push_str(&private.name);
                out.push_str("' 'd' '");
                out.push_str(&private.d.to_string());
                out.push_str("' } ECPRIVATE ");
            }
            Key::Public(public) => {
                out.push_str("{ 'curve' '");
                out.push_str(&public.name);
                out.push_str("' 'Q' '");
                out.push_str(&crate::encoding::hex_encode(&public.point.encode()));
                out.push_str("' } ECPUBLIC ");
            }
            // The parameter maps `RSAPRIVATE` and `RSAPUBLIC` read, their keys sorted.
            Key::RsaPrivate(key) => rsa_snapshot(out, key, "RSAPRIVATE"),
            Key::RsaPublic(key) => rsa_snapshot(out, key, "RSAPUBLIC"),
        },
    }
    Ok(())
}

/// The `SNAPSHOT` of an RSA key: its parameter map and the function that reads it.
fn rsa_snapshot(out: &mut String, key: &RsaKey, function: &str) {
    out.push_str("{ 'algorithm' 'RSA' 'exponent' '");
    out.push_str(&key.exponent.to_string());
    out.push_str("' 'modulus' '");
    out.push_str(&key.modulus.to_string());
    out.push_str("' } ");
    out.push_str(function);
    out.push(' ');
}
