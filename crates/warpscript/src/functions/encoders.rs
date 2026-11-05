// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Encoders and wrapped series: `NEWENCODER`, the `WRAP` family (`WRAP`, `WRAPRAW`,
//! `WRAPFAST`, `WRAPOPT`, `WRAPRAWOPT`, `WRAPMV`, `WRAPMV!`) and `UNWRAP`, `UNWRAPEMPTY`,
//! `UNWRAPSIZE`, `UNWRAPENCODER`, on the `GTSWrapper` Thrift structure of `thrift.rs` and the
//! encoders of `encoder.rs`.

use std::cell::RefCell;
use std::rc::Rc;

use super::Entry;
use super::args::fail;
use super::gts::element_or_list;
use crate::encoder::{
    DEFAULT_COMP_RATIO_THRESHOLD, GtsEncoder, encoder_to_wrapper, gts_to_wrapper,
    wrapper_to_encoder, wrapper_to_gts,
};
use crate::encoding::{opb64_decode, opb64_encode};
use crate::error::{Error, Result};
use crate::stack::Stack;
use crate::thrift::{GtsWrapper, deserialize_wrapper, serialize_wrapper};
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("NEWENCODER", Entry::Stack(newencoder)),
    ("WRAP", Entry::Builder(wrap)),
    ("WRAPRAW", Entry::Builder(wrap)),
    ("WRAPFAST", Entry::Builder(wrap)),
    ("WRAPOPT", Entry::Builder(wrap)),
    ("WRAPRAWOPT", Entry::Builder(wrap)),
    ("WRAPMV", Entry::Builder(wrap)),
    ("WRAPMV!", Entry::Builder(wrap)),
    ("UNWRAP", Entry::Builder(unwrap)),
    ("UNWRAPEMPTY", Entry::Builder(unwrap)),
    ("UNWRAPSIZE", Entry::Builder(unwrap)),
    ("UNWRAPENCODER", Entry::Builder(unwrapencoder)),
];

/// `NEWENCODER`: an empty encoder named `""`.
fn newencoder(stack: &mut Stack) -> Result<()> {
    let mut encoder = GtsEncoder::new(0);
    encoder.metadata.name = Some(String::new());
    stack.push(Value::Encoder(Rc::new(RefCell::new(encoder))));
    Ok(())
}

/// `WRAP` and its variants, `(opt, compress, raw, mv)` from the registration of each name.
fn wrap(stack: &mut Stack, name: &'static str) -> Result<()> {
    let (opt, compress, raw, mv) = match name {
        "WRAPRAW" => (false, true, true, false),
        "WRAPFAST" => (false, false, true, false),
        "WRAPOPT" => (true, true, false, false),
        "WRAPRAWOPT" => (true, true, true, false),
        "WRAPMV" => (true, true, true, true),
        "WRAPMV!" => (true, false, true, true),
        _ => (false, true, false, false),
    };
    let compratio = if opt {
        1.0
    } else {
        DEFAULT_COMP_RATIO_THRESHOLD
    };
    element_or_list(stack, |element| {
        let mut wrapper = match element {
            Value::Gts(gts) => gts_to_wrapper(&gts.borrow(), compress, compratio, opt, true),
            Value::Encoder(encoder) => {
                encoder_to_wrapper(&encoder.borrow(), compress, compratio, true)
            }
            _ => {
                return Err(fail(
                    name,
                    "expects a Geo Time Series, a GTSEncoder or a list thereof.",
                ));
            }
        };
        if mv {
            wrapper.metadata = None;
            wrapper.count = None;
        }
        let bytes = serialize_wrapper(&wrapper);
        Ok(if raw {
            Value::bytes(bytes)
        } else {
            Value::string(opb64_encode(&bytes))
        })
    })
}

/// A wrapped series as given to the `UNWRAP` functions.
enum Wrapped {
    /// OPB64 text.
    Text(Rc<str>),
    /// Raw bytes.
    Raw(Rc<[u8]>),
}

impl Wrapped {
    fn of(value: &Value) -> Option<Wrapped> {
        match value {
            Value::String(s) => Some(Wrapped::Text(Rc::clone(s))),
            Value::Bytes(b) => Some(Wrapped::Raw(Rc::clone(b))),
            _ => None,
        }
    }

    /// The deserialized wrapper. A string is decoded from OPB64 (its non-ASCII characters
    /// are `?`, as `getBytes(US_ASCII)` makes them); Thrift errors are the cause of
    /// `<name> <what>`.
    fn read(&self, name: &str, what: &str) -> Result<GtsWrapper> {
        let bytes = match self {
            Wrapped::Text(s) => {
                let ascii: String = s
                    .chars()
                    .map(|c| if c.is_ascii() { c } else { '?' })
                    .collect();
                opb64_decode(&ascii)?
            }
            Wrapped::Raw(b) => b.to_vec(),
        };
        deserialize_wrapper(&bytes).map_err(|err| wrap_error(name, what, err))
    }
}

/// Java's `catch (TException | IOException e)`: the checked exceptions become the cause of
/// `<name> <what>`, the runtime ones go through.
fn wrap_error(name: &str, what: &str, err: Error) -> Error {
    if err.is_runtime() {
        err
    } else {
        Error::with_cause(format!("{name} {what}"), err)
    }
}

/// `UNWRAP`, `UNWRAPEMPTY` (the series without their datapoints) and `UNWRAPSIZE` (the
/// number of datapoints): a string or byte array, or a list thereof.
fn unwrap(stack: &mut Stack, name: &'static str) -> Result<()> {
    let invalid = || {
        fail(
            name,
            "operates on a string or byte array or a list thereof.",
        )
    };
    let top = stack.pop()?;
    let elements = match &top {
        Value::List(list) => list.borrow().clone(),
        other => vec![other.clone()],
    };
    let inputs = elements
        .iter()
        .map(Wrapped::of)
        .collect::<Option<Vec<_>>>()
        .ok_or_else(invalid)?;
    let mut outputs = Vec::with_capacity(inputs.len());
    for input in &inputs {
        let wrapper = input.read(name, "failed to unwrap GTS.")?;
        outputs.push(if name == "UNWRAPSIZE" {
            Value::Long(wrapper.count())
        } else {
            let gts = wrapper_to_gts(&wrapper, name == "UNWRAPEMPTY")?;
            Value::Gts(gts.shared())
        });
    }
    stack.push(match top {
        Value::List(_) => Value::list(outputs),
        _ => outputs.pop().unwrap_or(Value::Null),
    });
    Ok(())
}

/// `UNWRAPENCODER`: the encoder of a wrapped series.
fn unwrapencoder(stack: &mut Stack, name: &'static str) -> Result<()> {
    let input = Wrapped::of(&stack.pop()?)
        .ok_or_else(|| fail(name, "operates on a string or byte array."))?;
    let what = "failed to unwrap encoder.";
    let wrapper = input.read(name, what)?;
    let encoder = wrapper_to_encoder(&wrapper).map_err(|err| wrap_error(name, what, err))?;
    stack.push(Value::Encoder(Rc::new(RefCell::new(encoder))));
    Ok(())
}
