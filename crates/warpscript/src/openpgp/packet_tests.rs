// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! Tests of the packet reader and writer: the four old-format length types, the new-format
//! lengths (one, two and five octets, and partial bodies), and the failures on truncated input.

use crate::error::pgp::PgpError;
use crate::openpgp::packet::{Packet, PacketReader, write_packet};

fn read_all(input: &[u8]) -> Result<Vec<Packet>, PgpError> {
    let mut reader = PacketReader::new(input);
    let mut packets = Vec::new();
    while reader.next_tag()?.is_some() {
        packets.push(reader.read()?);
    }
    Ok(packets)
}

fn packet(tag: u8, body: &[u8]) -> Packet {
    Packet {
        tag,
        body: body.to_vec(),
    }
}

#[test]
fn old_format_length_types() {
    assert_eq!(
        read_all(&[0x98, 3, 1, 2, 3]),
        Ok(vec![packet(6, &[1, 2, 3])])
    );
    assert_eq!(
        read_all(&[0x99, 0, 3, 1, 2, 3]),
        Ok(vec![packet(6, &[1, 2, 3])])
    );
    assert_eq!(
        read_all(&[0x9A, 0, 0, 0, 3, 1, 2, 3]),
        Ok(vec![packet(6, &[1, 2, 3])])
    );
    assert_eq!(read_all(&[0x9B, 1, 2, 3]), Ok(vec![packet(6, &[1, 2, 3])]));
}

#[test]
fn old_format_truncated() {
    assert_eq!(read_all(&[0x98]), Err(PgpError::PrematureEnd));
    assert_eq!(read_all(&[0x98, 5, 1]), Err(PgpError::PrematureEnd));
    assert_eq!(read_all(&[0x99, 0]), Err(PgpError::PrematureEnd));
    assert_eq!(read_all(&[0x9A, 0, 0, 0]), Err(PgpError::PrematureEnd));
}

#[test]
fn new_format_lengths() {
    assert_eq!(
        read_all(&[0xC6, 3, 1, 2, 3]),
        Ok(vec![packet(6, &[1, 2, 3])])
    );
    let body: Vec<u8> = (0..200).map(|i| i as u8).collect();
    let mut input = vec![0xC6, 192, 8];
    input.extend_from_slice(&body);
    assert_eq!(read_all(&input), Ok(vec![packet(6, &body)]));
    let mut input = vec![0xC6, 255, 0, 0, 0, 2, 1, 2];
    assert_eq!(read_all(&input), Ok(vec![packet(6, &[1, 2])]));
    input.truncate(6);
    assert_eq!(read_all(&input), Err(PgpError::PrematureEnd));
}

#[test]
fn new_format_partial_bodies() {
    // 0xE1 announces a partial block of 2 bytes; the final block has 1 byte.
    assert_eq!(
        read_all(&[0xC6, 0xE1, 1, 2, 1, 3]),
        Ok(vec![packet(6, &[1, 2, 3])])
    );
    assert_eq!(read_all(&[0xC6, 0xE1, 1]), Err(PgpError::PrematureEnd));
}

#[test]
fn new_format_truncated() {
    assert_eq!(read_all(&[0xC6]), Err(PgpError::PrematureEnd));
    assert_eq!(read_all(&[0xC6, 192]), Err(PgpError::PrematureEnd));
    assert_eq!(read_all(&[0xC6, 255, 0, 0]), Err(PgpError::PrematureEnd));
    assert_eq!(read_all(&[0xC6, 5, 1]), Err(PgpError::PrematureEnd));
}

#[test]
fn header_without_high_bit_is_invalid() {
    assert_eq!(read_all(&[0x06, 0]), Err(PgpError::InvalidHeader));
    assert_eq!(
        PacketReader::new(&[0x06]).next_tag(),
        Err(PgpError::InvalidHeader)
    );
    assert_eq!(
        PacketReader::new(&[0x06, 0]).read().map(|_| ()),
        Err(PgpError::InvalidHeader)
    );
}

#[test]
fn empty_input_has_no_packet() {
    assert_eq!(PacketReader::new(&[]).next_tag(), Ok(None));
    assert_eq!(PacketReader::new(&[]).read(), Err(PgpError::PrematureEnd));
}

#[test]
fn write_old_format_lengths() {
    let mut out = Vec::new();
    write_packet(&mut out, 6, &[1, 2, 3]);
    assert_eq!(out, [0x98, 3, 1, 2, 3]);

    let body = vec![7u8; 300];
    let mut out = Vec::new();
    write_packet(&mut out, 6, &body);
    assert_eq!(&out[..3], &[0x99, 0x01, 0x2C]);
    assert_eq!(read_all(&out), Ok(vec![packet(6, &body)]));

    let body = vec![7u8; 70_000];
    let mut out = Vec::new();
    write_packet(&mut out, 6, &body);
    assert_eq!(&out[..5], &[0x9A, 0x00, 0x01, 0x11, 0x70]);
    assert_eq!(read_all(&out), Ok(vec![packet(6, &body)]));
}

#[test]
fn write_new_format_lengths() {
    let mut out = Vec::new();
    write_packet(&mut out, 17, &[1, 2, 3]);
    assert_eq!(out, [0xD1, 3, 1, 2, 3]);

    for length in [191, 192, 1000, 8383, 8384, 9000] {
        let body = vec![5u8; length];
        let mut out = Vec::new();
        write_packet(&mut out, 17, &body);
        assert_eq!(
            read_all(&out),
            Ok(vec![packet(17, &body)]),
            "length {length}"
        );
    }

    let mut out = Vec::new();
    write_packet(&mut out, 17, &vec![0u8; 9000]);
    assert_eq!(&out[..6], &[0xD1, 0xFF, 0x00, 0x00, 0x23, 0x28]);
}

#[test]
fn peek_tag_stops_at_an_invalid_header() {
    assert_eq!(PacketReader::new(&[]).peek_tag(), None);
    assert_eq!(PacketReader::new(&[0x06]).peek_tag(), None);
    assert_eq!(PacketReader::new(&[0x98, 0]).peek_tag(), Some(6));
}
