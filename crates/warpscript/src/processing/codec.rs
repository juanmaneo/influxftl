// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! The codecs of `PIMAGE`: `Pencode` writes an image as a data URI of PNG or JPEG bytes, and
//! `Pdecode` reads PNG, JPEG, GIF or BMP bytes back. The bytes are not Java's: the compressor and
//! the JPEG tables differ, so an encoding is compared with Warp 10 by what it decodes to, not by
//! its bytes.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use super::finite;
use super::image::{Image, argb, channels};
use crate::error::processing::{ProcessingError, SharedError};
use crate::error::{Error, Result};

/// The JPEG quality `ImageIO` writes when none is given: 0.75.
pub const DEFAULT_JPEG_QUALITY: f32 = 0.75;

/// The formats `Pencode` writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// `png`, the default.
    Png,
    /// `jpeg`, without alpha.
    Jpeg,
}

impl Format {
    /// The name of the format, as the data URI and the `format` option spell it.
    pub fn name(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Jpeg => "jpeg",
        }
    }
}

/// A text chunk of a PNG, given by `Pencode`'s options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextChunk {
    /// `tEXt`: Latin-1 text.
    Plain {
        /// The keyword, 1 to 79 Latin-1 characters.
        keyword: String,
        /// The text.
        text: String,
    },
    /// `zTXt`: Latin-1 text, compressed.
    Compressed {
        /// The keyword, 1 to 79 Latin-1 characters.
        keyword: String,
        /// The text.
        text: String,
    },
    /// `iTXt`: UTF-8 text, optionally compressed, with a language and a translated keyword.
    International {
        /// The keyword, 1 to 79 Latin-1 characters.
        keyword: String,
        /// The text.
        text: String,
        /// Whether the text is compressed.
        compressed: bool,
        /// The language tag (`languageTag`), empty by default.
        language: String,
        /// The translated keyword (`translatedKeyword`), empty by default.
        translated: String,
    },
}

/// What `Pencode` writes: the format, the PNG text chunks, and the JPEG quality as a fraction of
/// one.
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// The format of the bytes.
    pub format: Format,
    /// The text chunks, written in order (PNG only).
    pub text: Vec<TextChunk>,
    /// The JPEG quality, from 0.0 to 1.0 (JPEG only).
    pub quality: f32,
}

impl Default for Options {
    /// A PNG without text chunks: what `Pencode` writes when it is given no options.
    fn default() -> Self {
        Options {
            format: Format::Png,
            text: Vec::new(),
            quality: DEFAULT_JPEG_QUALITY,
        }
    }
}

/// The image as a data URI, `data:image/<format>;base64,<bytes>`: the bytes of the format, base64
/// encoded without line breaks.
pub fn encode(image: &Image, options: &Options) -> Result<String> {
    if image.width == 0 || image.height == 0 {
        return Err(ProcessingError::EmptyImage {
            width: image.width as i64,
            height: image.height as i64,
        }
        .into());
    }
    let bytes = match options.format {
        Format::Png => png_bytes(image, &options.text)?,
        Format::Jpeg => jpeg_bytes(image, options.quality)?,
    };
    Ok(format!(
        "data:image/{};base64,{}",
        options.format.name(),
        STANDARD.encode(bytes)
    ))
}

/// The PNG of the image, RGBA with its text chunks.
fn png_bytes(image: &Image, text: &[TextChunk]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width as u32, image.height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(encode_failure)?;
    for chunk in text {
        match chunk {
            TextChunk::Plain { keyword, text } => writer
                .write_text_chunk(&png::text_metadata::TEXtChunk::new(
                    keyword.as_str(),
                    text.as_str(),
                ))
                .map_err(encode_failure)?,
            TextChunk::Compressed { keyword, text } => writer
                .write_text_chunk(&png::text_metadata::ZTXtChunk::new(
                    keyword.as_str(),
                    text.as_str(),
                ))
                .map_err(encode_failure)?,
            TextChunk::International {
                keyword,
                text,
                compressed,
                language,
                translated,
            } => {
                let mut itxt = png::text_metadata::ITXtChunk::new(keyword.as_str(), text.as_str());
                itxt.compressed = *compressed;
                itxt.language_tag.clone_from(language);
                itxt.translated_keyword.clone_from(translated);
                writer.write_text_chunk(&itxt).map_err(encode_failure)?;
            }
        }
    }
    let mut data = Vec::with_capacity(image.pixels.len() * 4);
    for pixel in &image.pixels {
        let (alpha, red, green, blue) = channels(*pixel);
        data.extend_from_slice(&[red, green, blue, alpha]);
    }
    writer.write_image_data(&data).map_err(encode_failure)?;
    writer.finish().map_err(encode_failure)?;
    Ok(out)
}

/// The JPEG of the image, RGB (the alpha is dropped, as `TYPE_INT_RGB` does) at the quality
/// `quality` (a fraction of one, from 0 to 100 on the JPEG scale, never below 1).
fn jpeg_bytes(image: &Image, quality: f32) -> Result<Vec<u8>> {
    let level = (finite(quality * 100.0)?.round() as u8).max(1);
    let mut rgb = Vec::with_capacity(image.pixels.len() * 3);
    for pixel in &image.pixels {
        let (_, red, green, blue) = channels(*pixel);
        rgb.extend_from_slice(&[red, green, blue]);
    }
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, level)
        .encode(
            &rgb,
            image.width as u32,
            image.height as u32,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(encode_failure)?;
    Ok(out)
}

/// The image a data URI or a byte array holds, as ARGB pixels. The format is found from the bytes.
pub fn decode(bytes: &[u8]) -> Result<Image> {
    let decoded = image::load_from_memory(bytes).map_err(decode_failure)?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba
        .pixels()
        .map(|pixel| {
            let [red, green, blue, alpha] = pixel.0;
            argb(alpha, red, green, blue)
        })
        .collect();
    Ok(Image::from_pixels(width as usize, height as usize, pixels))
}

/// The bytes of the base64 part of a data URI: what follows its first comma, or the whole text
/// when it has none. White space is skipped, since Java's decoder skips the characters that are
/// not in its alphabet.
pub fn base64_payload(uri: &str) -> Result<Vec<u8>> {
    let encoded = uri.split_once(',').map_or(uri, |(_, data)| data);
    let compact: String = encoded
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    STANDARD.decode(compact).map_err(decode_failure)
}

/// A PNG or JPEG failure of the writer, as a failure of `Pencode`.
fn encode_failure(err: impl std::error::Error + Send + Sync + 'static) -> Error {
    ProcessingError::Encode(SharedError::new(err)).into()
}

/// A failure of a reader (the image crate or base64), as a failure of `Pdecode`.
fn decode_failure(err: impl std::error::Error + Send + Sync + 'static) -> Error {
    ProcessingError::Decode(SharedError::new(err)).into()
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod codec_tests;
