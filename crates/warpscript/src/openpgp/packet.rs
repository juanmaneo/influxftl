// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! The packets of OpenPGP (RFC 4880 §4): their headers, lengths and bodies, read and written
//! as BouncyCastle's `BCPGInputStream` and `BCPGOutputStream` do.

use crate::error::pgp::PgpError;

/// A packet: its tag and its body, which is kept as read so that it can be written back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// The tag, which tells the kind of the packet (6 public key, 13 user ID, …).
    pub tag: u8,
    /// The body, without its header.
    pub body: Vec<u8>,
}

/// The packets of a stream of bytes, read one after the other.
#[derive(Debug, Clone)]
pub struct PacketReader<'a> {
    input: &'a [u8],
}

impl<'a> PacketReader<'a> {
    /// A reader of the packets of `input`.
    pub fn new(input: &'a [u8]) -> Self {
        Self { input }
    }

    /// The tag of the next packet, `None` at the end of the input (`nextPacketTag`).
    pub fn next_tag(&self) -> Result<Option<u8>, PgpError> {
        match self.input.first() {
            None => Ok(None),
            Some(&header) => Ok(Some(tag_of(header)?)),
        }
    }

    /// The tag of the next packet when its header is valid, `None` otherwise. An invalid header
    /// is left for [`PacketReader::next_tag`] to report, which the reading of the rings ends with.
    pub fn peek_tag(&self) -> Option<u8> {
        self.input
            .first()
            .filter(|&&header| header & 0x80 != 0)
            .map(|&header| tag_bits(header))
    }

    /// Reads the next packet (`readPacket`).
    pub fn read(&mut self) -> Result<Packet, PgpError> {
        let header = self.take_byte()?;
        let tag = tag_of(header)?;
        let body = if header & 0x40 != 0 {
            self.new_format_body()?
        } else {
            self.old_format_body(header & 0x03)?
        };
        Ok(Packet { tag, body })
    }

    /// The body of a packet in the new format: a length, or partial lengths followed by one.
    fn new_format_body(&mut self) -> Result<Vec<u8>, PgpError> {
        let mut body = Vec::new();
        loop {
            let length = match self.take_byte()? {
                first @ 0..=191 => first as usize,
                first @ 192..=223 => {
                    ((first as usize - 192) << 8) + self.take_byte()? as usize + 192
                }
                first @ 224..=254 => {
                    body.extend_from_slice(self.take(1 << (first & 0x1F))?);
                    continue;
                }
                255 => u32::from_be_bytes(self.take_array()?) as usize,
            };
            body.extend_from_slice(self.take(length)?);
            return Ok(body);
        }
    }

    /// The body of a packet in the old format, whose length type is `length_type`.
    fn old_format_body(&mut self, length_type: u8) -> Result<Vec<u8>, PgpError> {
        let length = match length_type {
            0 => self.take_byte()? as usize,
            1 => u16::from_be_bytes(self.take_array()?) as usize,
            2 => u32::from_be_bytes(self.take_array()?) as usize,
            _ => self.input.len(),
        };
        Ok(self.take(length)?.to_vec())
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], PgpError> {
        let (head, rest) = self
            .input
            .split_at_checked(length)
            .ok_or(PgpError::PrematureEnd)?;
        self.input = rest;
        Ok(head)
    }

    fn take_byte(&mut self) -> Result<u8, PgpError> {
        let (&byte, rest) = self.input.split_first().ok_or(PgpError::PrematureEnd)?;
        self.input = rest;
        Ok(byte)
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N], PgpError> {
        let (&array, rest) = self
            .input
            .split_first_chunk::<N>()
            .ok_or(PgpError::PrematureEnd)?;
        self.input = rest;
        Ok(array)
    }
}

/// The tag of the packet whose header is `header` (`BCPGInputStream.nextPacketTag`).
fn tag_of(header: u8) -> Result<u8, PgpError> {
    if header & 0x80 == 0 {
        return Err(PgpError::InvalidHeader);
    }
    Ok(tag_bits(header))
}

/// The tag of a header whose high bit is set.
fn tag_bits(header: u8) -> u8 {
    if header & 0x40 != 0 {
        header & 0x3F
    } else {
        (header & 0x3C) >> 2
    }
}

/// Writes a packet with its body (`writePacket`). The tags up to 15 use the old format with the
/// shortest length; the others the new format.
pub fn write_packet(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    let length = body.len();
    if tag <= 15 {
        if length <= 0xFF {
            out.push(0x80 | (tag << 2));
            out.push(length as u8);
        } else if length <= 0xFFFF {
            out.push(0x80 | (tag << 2) | 1);
            out.extend_from_slice(&(length as u16).to_be_bytes());
        } else {
            out.push(0x80 | (tag << 2) | 2);
            out.extend_from_slice(&(length as u32).to_be_bytes());
        }
    } else {
        out.push(0xC0 | tag);
        write_new_length(out, length);
    }
    out.extend_from_slice(body);
}

/// The length of a packet in the new format (`writeNewPacketLength`).
fn write_new_length(out: &mut Vec<u8>, length: usize) {
    if length < 192 {
        out.push(length as u8);
    } else if length <= 8383 {
        let rest = length - 192;
        out.push((rest >> 8) as u8 + 192);
        out.push(rest as u8);
    } else {
        out.push(0xFF);
        out.extend_from_slice(&(length as u32).to_be_bytes());
    }
}
