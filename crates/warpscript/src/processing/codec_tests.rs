// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Tests of the codecs of `PIMAGE`: the refusal of an empty image, a JPEG quality that is not a
//! number or a size the format cannot hold, the text chunks the PNG writer refuses, a round trip
//! through a data URI, and the refusal of bytes that are not an image or not base64. The bytes of
//! the encoders are not compared, only what they decode to.

use std::io::{self, Write};

use super::{Format, Options, TextChunk, base64_payload, decode, encode, png_bytes};
use crate::error::Error;
use crate::error::processing::ProcessingError;
use crate::processing::image::Image;

/// A writer that takes `budget` bytes and then fails, as a full disk would.
struct FailAfter {
    budget: usize,
}

impl Write for FailAfter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.budget == 0 {
            return Err(io::Error::other("disk full"));
        }
        let taken = bytes.len().min(self.budget);
        self.budget -= taken;
        Ok(taken)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_png_writer_that_fails_at_any_byte_reports_the_failure() {
    let image = Image::from_pixels(
        2,
        2,
        vec![0xff00_0000, 0xff11_1111, 0xff22_2222, 0xff33_3333],
    );
    let mut whole = Vec::new();
    png_bytes(&mut whole, &image, &[]).expect("a small image encodes");
    for budget in 0..whole.len() {
        assert!(
            matches!(
                png_bytes(&mut FailAfter { budget }, &image, &[]),
                Err(Error::Processing(ProcessingError::Encode(_)))
            ),
            "a writer that takes {budget} of {} bytes must fail",
            whole.len()
        );
    }
    assert!(
        png_bytes(
            &mut FailAfter {
                budget: whole.len()
            },
            &image,
            &[]
        )
        .is_ok()
    );
}

#[test]
fn an_image_with_no_width_or_no_height_is_refused_with_its_size() {
    assert_eq!(
        encode(&Image::new(0, 2).expect("empty"), &Options::default()),
        Err(ProcessingError::EmptyImage {
            width: 0,
            height: 2,
        }
        .into())
    );
    assert_eq!(
        encode(&Image::new(2, 0).expect("empty"), &Options::default()),
        Err(ProcessingError::EmptyImage {
            width: 2,
            height: 0,
        }
        .into())
    );
}

#[test]
fn a_jpeg_quality_that_is_not_a_number_is_refused() {
    let options = Options {
        format: Format::Jpeg,
        quality: f32::NAN,
        ..Options::default()
    };
    assert!(encode(&Image::new(2, 2).expect("small"), &options).is_err());
}

#[test]
fn a_jpeg_is_a_data_uri_of_the_jpeg_format() {
    let options = Options {
        format: Format::Jpeg,
        ..Options::default()
    };
    let uri = encode(&Image::new(2, 2).expect("small"), &options).expect("encodable");
    assert!(uri.starts_with("data:image/jpeg;base64,"), "{uri}");
}

#[test]
fn a_jpeg_wider_than_the_format_allows_is_refused_by_the_encoder() {
    let options = Options {
        format: Format::Jpeg,
        ..Options::default()
    };
    assert!(matches!(
        encode(
            &Image::new(65536, 1).expect("a row fits in memory"),
            &options
        ),
        Err(Error::Processing(ProcessingError::Encode(_)))
    ));
}

#[test]
fn a_text_chunk_with_an_empty_keyword_is_refused_by_the_png_writer() {
    for chunk in [
        TextChunk::Plain {
            keyword: String::new(),
            text: "x".to_owned(),
        },
        TextChunk::Compressed {
            keyword: String::new(),
            text: "x".to_owned(),
        },
        TextChunk::International {
            keyword: String::new(),
            text: "x".to_owned(),
            compressed: false,
            language: String::new(),
            translated: String::new(),
        },
    ] {
        let options = Options {
            text: vec![chunk.clone()],
            ..Options::default()
        };
        assert!(
            matches!(
                encode(&Image::new(2, 2).expect("small"), &options),
                Err(Error::Processing(ProcessingError::Encode(_)))
            ),
            "{chunk:?}"
        );
    }
}

#[test]
fn a_png_of_no_width_is_refused_by_the_writer() {
    assert!(matches!(
        png_bytes(&mut Vec::new(), &Image::from_pixels(0, 1, Vec::new()), &[]),
        Err(Error::Processing(ProcessingError::Encode(_)))
    ));
}

#[test]
fn a_png_whose_pixels_do_not_fill_its_size_is_refused_by_the_writer() {
    assert!(matches!(
        encode(&Image::from_pixels(2, 2, vec![0; 3]), &Options::default()),
        Err(Error::Processing(ProcessingError::Encode(_)))
    ));
}

#[test]
fn a_png_data_uri_decodes_back_to_the_same_pixels() {
    let image = Image::from_pixels(
        2,
        2,
        vec![0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0x80ff_ffff],
    );
    let uri = encode(&image, &Options::default()).expect("encodable");
    assert!(uri.starts_with("data:image/png;base64,"), "{uri}");
    let bytes = base64_payload(&uri).expect("base64 payload");
    assert_eq!(decode(&bytes).expect("a PNG decodes"), image);
}

#[test]
fn bytes_that_are_not_an_image_are_refused() {
    assert!(matches!(
        decode(b"garbage"),
        Err(Error::Processing(ProcessingError::Decode(_)))
    ));
}

#[test]
fn a_base64_payload_is_what_follows_the_comma_without_white_space() {
    assert_eq!(
        base64_payload("data:x;base64, QUJD\n").expect("valid base64"),
        b"ABC".to_vec()
    );
    assert_eq!(
        base64_payload("QUJD").expect("valid base64"),
        b"ABC".to_vec()
    );
}

#[test]
fn a_base64_payload_outside_the_alphabet_is_refused() {
    assert!(matches!(
        base64_payload("!!!"),
        Err(Error::Processing(ProcessingError::Decode(_)))
    ));
}
