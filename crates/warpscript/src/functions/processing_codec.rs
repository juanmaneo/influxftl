// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The Processing codecs: `Pencode` writes an image as a `data:image/…;base64,…` URI, PNG or JPEG
//! according to its options, and `Pdecode` reads an image back from such a URI or from a byte
//! array.

use indexmap::IndexMap;

use super::Entry;
use super::args::{fail, fail_with_cause};
use super::processing_image::image_value;
use crate::error::processing::ProcessingError;
use crate::error::{Error, Result};
use crate::finite::ensure_finite;
use crate::processing::codec::{self, Format, Options, TextChunk};
use crate::processing::finite;
use crate::processing::image::Image;
use crate::stack::Stack;
use crate::value::Value;

/// The functions of this module.
pub const FUNCTIONS: &[(&str, Entry)] = &[
    ("Pencode", Entry::Stack(pencode)),
    ("Pdecode", Entry::Stack(pdecode)),
];

/// `Pencode`: `graphics [options] Pencode` writes the graphics, or a `PIMAGE`, as a data URI, and
/// consumes it. The options are a MAP: `format` (`png`, the default, or `jpeg`); for PNG, the text
/// chunks `tEXt`, `zTXt` and `iTXt`, each a LIST of MAPs with a `keyword` and a `text`; for JPEG,
/// the `quality`, a `DOUBLE` from 0.0 to 1.0 or a `LONG` from 0 to 100.
fn pencode(stack: &mut Stack) -> Result<()> {
    const NAME: &str = "Pencode";
    let (options, target) = match stack.pop()? {
        Value::Map(map) => (Some(map), stack.pop()?),
        other => (None, other),
    };
    let uri = match &target {
        Value::Graphics(graphics) => encode(&graphics.borrow().image, options.as_ref())?,
        Value::Image(image) => encode(&image.borrow(), options.as_ref())?,
        _ => return Err(fail(NAME, "operates on a PGRAPHICS or PIMAGE instance.")),
    };
    stack.push(Value::from(uri));
    Ok(())
}

/// The data URI of `image` written with the options of a MAP, if any. The failures of the options
/// and of the writer are `Pencode`'s, with its cause; the runtime failures Java raises without that
/// wrapper (an empty image, a quality out of bounds) pass as they are.
fn encode(image: &Image, options: Option<&crate::value::Map>) -> Result<String> {
    let options = match options {
        Some(map) => options_of(&map.borrow()).map_err(wrap)?,
        None => Options::default(),
    };
    codec::encode(image, &options).map_err(wrap)
}

/// A failure inside `Pencode`, as Warp 10 reports it.
fn wrap(cause: Error) -> Error {
    if cause.is_runtime() {
        cause
    } else {
        fail_with_cause("Pencode", "failed.", cause)
    }
}

/// The options of a `Pencode` MAP. Under PNG every entry is a text chunk, so a key that is not one
/// of the three chunk types is refused, `format` included; under JPEG only `quality` is read.
fn options_of(entries: &IndexMap<Value, Value>) -> Result<Options> {
    let format = format_of(entries)?;
    let mut options = Options {
        format,
        ..Options::default()
    };
    match format {
        Format::Png => {
            for (key, value) in entries {
                png_chunks(key, value, &mut options.text)?;
            }
        }
        Format::Jpeg => {
            if let Some(value) = entries.get(&Value::from("quality")) {
                options.quality = quality_of(value)?;
            }
        }
    }
    Ok(options)
}

/// The format named by the `format` entry, PNG when there is none.
fn format_of(entries: &IndexMap<Value, Value>) -> Result<Format> {
    match entries.get(&Value::from("format")) {
        None => Ok(Format::Png),
        Some(Value::String(name)) => match &**name {
            "png" => Ok(Format::Png),
            "jpeg" => Ok(Format::Jpeg),
            _ => Err(ProcessingError::UnsupportedFormat.into()),
        },
        Some(_) => Err(ProcessingError::UnsupportedFormat.into()),
    }
}

/// The text of a `STRING` value, `None` for any other value.
fn string_of(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.to_string()),
        _ => None,
    }
}

/// The text Java's `toString()` gives an entry of a chunk, `""` when it has none. A `null` has no
/// text, and Java refuses to call `toString()` on it.
fn text_of(value: Option<&Value>) -> Result<String> {
    match value {
        None => Ok(String::new()),
        Some(Value::Null) => Err(Error::null_pointer()),
        Some(other) => Ok(other.to_string()),
    }
}

/// The text chunks of one PNG entry: `tEXt`, `zTXt` or `iTXt`, whose value is a LIST of MAPs, each
/// with a `keyword` and a `text` of type `STRING`. Any other key is refused.
fn png_chunks(key: &Value, value: &Value, out: &mut Vec<TextChunk>) -> Result<()> {
    let Some(name @ ("tEXt" | "zTXt" | "iTXt")) = key.as_str() else {
        return Err(ProcessingError::UnknownChunk.into());
    };
    let Value::List(list) = value else {
        return Err(ProcessingError::ChunkNotList.into());
    };
    let kinds = if name == "iTXt" {
        "iTXt"
    } else {
        "tEXt and zTXt"
    };
    for element in list.borrow().iter() {
        let Value::Map(map) = element else {
            return Err(ProcessingError::ChunkNotMap { kinds }.into());
        };
        let map = map.borrow();
        let (Some(keyword), Some(text)) = (
            string_of(map.get(&Value::from("keyword"))),
            string_of(map.get(&Value::from("text"))),
        ) else {
            return Err(ProcessingError::ChunkFields { kinds }.into());
        };
        out.push(match name {
            "tEXt" => TextChunk::Plain { keyword, text },
            "zTXt" => TextChunk::Compressed { keyword, text },
            _ => TextChunk::International {
                keyword,
                text,
                compressed: matches!(
                    map.get(&Value::from("compressionFlag")),
                    Some(Value::Boolean(true))
                ),
                language: text_of(map.get(&Value::from("languageTag")))?,
                translated: text_of(map.get(&Value::from("translatedKeyword")))?,
            },
        });
    }
    Ok(())
}

/// The JPEG quality of a `quality` entry: a `DOUBLE` as it is, or a `LONG` out of 100, which must
/// come out within 0.0 to 1.0.
fn quality_of(value: &Value) -> Result<f32> {
    let quality = match value {
        Value::Double(double) => finite(ensure_finite(*double)? as f32)?,
        Value::Long(long) => (*long as f32) / 100.0,
        _ => return Err(ProcessingError::QualityType.into()),
    };
    if !(0.0..=1.0).contains(&quality) {
        return Err(ProcessingError::QualityOutOfBounds.into());
    }
    Ok(quality)
}

/// `Pdecode`: the `PIMAGE` a `data:image/…` URI or a byte array holds, PNG, JPEG, GIF or BMP, which
/// is pushed.
fn pdecode(stack: &mut Stack) -> Result<()> {
    let image = match stack.pop()? {
        Value::String(uri) if uri.starts_with("data:image/") => {
            codec::decode(&codec::base64_payload(&uri)?)?
        }
        Value::Bytes(bytes) => codec::decode(&bytes)?,
        _ => {
            return Err(fail(
                "Pdecode",
                "expects a base64 data URI or a byte array on top of the stack.",
            ));
        }
    };
    stack.push(image_value(image));
    Ok(())
}
