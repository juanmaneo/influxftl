// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Bouncy Castle OpenPGP (Copyright (c) 2000-2024 The Legion of the Bouncy Castle Inc., MIT licence), ported to Rust and modified.

//! The ASCII armor of OpenPGP, as BouncyCastle's `PGPUtil.getDecoderStream`,
//! `ArmoredInputStream` and `ArmoredOutputStream` read and write it.

use std::collections::VecDeque;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::error::pgp::ArmorError;

const CR: u8 = b'\r';
const LF: u8 = b'\n';
/// The bytes that `PGPUtil.getDecoderStream` peeks at, CR and LF included.
const PEEK_WINDOW: usize = 60;
const CRC24_INIT: u32 = 0x00B7_04CE;
const CRC24_POLY: u32 = 0x0186_4CFB;
const VERSION_HEADER: &str = "Version: BCPG v@RELEASE_NAME@";

/// The bytes of the input as OpenPGP packets, whatever the armor they come in.
///
/// Input whose first byte has bit 7 set, or that is empty, is returned unchanged.
pub fn decode(input: &[u8]) -> Result<Vec<u8>, ArmorError> {
    let has_headers = match kind(input)? {
        Kind::Raw => return Ok(input.to_vec()),
        Kind::Armored { has_headers } => has_headers,
    };
    let mut armored = Armored::new(input, has_headers)?;
    let mut out = Vec::new();
    loop {
        match armored.read()? {
            Some(b) => out.push(b),
            None if armored.at_eof => return Ok(out),
            None => {}
        }
    }
}

/// The ASCII armor of one encoded OpenPGP object, whose block type is that of its first byte.
pub fn encode(data: &[u8]) -> String {
    let typ = block_type(data.first());
    let mut out = format!("-----BEGIN PGP {typ}-----\n{VERSION_HEADER}\n\n");
    for group in data.chunks(48) {
        out.push_str(&STANDARD.encode(group));
        out.push('\n');
    }
    let [_, hi, mid, lo] = crc24(data).to_be_bytes();
    out.push('=');
    out.push_str(&STANDARD.encode([hi, mid, lo]));
    out.push('\n');
    out.push_str(&format!("-----END PGP {typ}-----\n"));
    out
}

enum Kind {
    Raw,
    Armored { has_headers: bool },
}

/// The decision of `PGPUtil.getDecoderStream`, made on the peek window.
fn kind(input: &[u8]) -> Result<Kind, ArmorError> {
    let Some(&first) = input.first() else {
        return Ok(Kind::Raw);
    };
    if first & 0x80 != 0 {
        return Ok(Kind::Raw);
    }
    let window: Vec<u8> = input.iter().take(PEEK_WINDOW).copied().collect();
    if window.len() < 4 || !window.iter().all(|&b| is_possibly_base64(b)) {
        return Ok(Kind::Armored { has_headers: true });
    }
    let stored: Vec<u8> = window
        .iter()
        .enumerate()
        .filter(|&(i, &b)| i == 0 || !matches!(b, CR | LF))
        .map(|(_, &b)| b)
        .take(8)
        .collect();
    Ok(Kind::Armored {
        has_headers: head_has_headers(first, &stored)?,
    })
}

/// Whether the armor has headers, given the first byte and the first (at most eight) bytes of
/// the window that are stored, as `getDecoderStream` decodes them.
fn head_has_headers(first: u8, stored: &[u8]) -> Result<bool, ArmorError> {
    let lead_ignored = matches!(first, CR | LF);
    let valid = stored.len().saturating_sub(usize::from(lead_ignored));
    if valid < 4 {
        return Err(ArmorError::Base64InvalidCharacters);
    }
    if lead_ignored {
        return Err(ArmorError::Base64IndexOutOfBounds);
    }
    if valid < 8 {
        return Err(ArmorError::Base64InvalidCharactersAtEnd);
    }
    Ok(sextet(first) & 0x20 == 0)
}

/// The block type of the armor, from the packet tag of the first byte, as `ArmoredOutputStream`
/// chooses it.
fn block_type(first: Option<&u8>) -> &'static str {
    let tag = match first {
        Some(&b) if b & 0x40 != 0 => b & 0x3F,
        Some(&b) => (b & 0x3F) >> 2,
        None => 0,
    };
    match tag {
        6 => "PUBLIC KEY BLOCK",
        5 => "PRIVATE KEY BLOCK",
        2 => "SIGNATURE",
        _ => "MESSAGE",
    }
}

/// The reader of `ArmoredInputStream`: the decoded bytes of the armored blocks of `input`.
struct Armored<'a> {
    input: &'a [u8],
    pos: usize,
    has_headers: bool,
    /// The CRC-24 of the bytes decoded so far in the current block.
    crc: u32,
    /// The decoded bytes of the current quad that are not read yet.
    pending: VecDeque<u8>,
    /// The next read starts a block, so its headers are parsed and its CRC is reset.
    start: bool,
    /// The block starts after an END line, so its BEGIN line is not searched for.
    restart: bool,
    at_eof: bool,
}

impl<'a> Armored<'a> {
    fn new(input: &'a [u8], has_headers: bool) -> Result<Self, ArmorError> {
        let mut armored = Self {
            input,
            pos: 0,
            has_headers,
            crc: CRC24_INIT,
            pending: VecDeque::new(),
            start: false,
            restart: false,
            at_eof: false,
        };
        if has_headers {
            armored.parse_headers()?;
        }
        Ok(armored)
    }

    /// The next decoded byte, or `None` at the end of a block; `at_eof` tells whether the input
    /// is over.
    fn read(&mut self) -> Result<Option<u8>, ArmorError> {
        if self.start {
            self.start = false;
            self.crc = CRC24_INIT;
            if self.has_headers {
                self.parse_headers()?;
            }
        }
        loop {
            if let Some(b) = self.pending.pop_front() {
                return Ok(Some(b));
            }
            if !self.advance()? {
                return Ok(None);
            }
        }
    }

    fn raw_read(&mut self) -> Option<u8> {
        let b = self.input.get(self.pos).copied()?;
        self.pos += 1;
        Some(b)
    }

    /// Skips the spaces, tabs, form feeds and vertical tabs that the armor ignores.
    fn read_ignore_space(&mut self) -> Result<Option<u8>, ArmorError> {
        loop {
            match self.raw_read() {
                Some(b' ' | b'\t' | 0x0B | 0x0C) => {}
                Some(b) if b >= 0x80 => return Err(ArmorError::InvalidArmor),
                other => return Ok(other),
            }
        }
    }

    /// Reads the next quad, a CRC line or an END line. Returns `false` when the block ends, at
    /// an END line or at the end of the input.
    fn advance(&mut self) -> Result<bool, ArmorError> {
        let first = self.read_ignore_space()?;
        if !matches!(first, Some(CR | LF)) {
            return self.quad(first);
        }
        let mut next = self.read_ignore_space()?;
        while matches!(next, Some(CR | LF)) {
            next = self.read_ignore_space()?;
        }
        match next {
            Some(b'=') => {
                self.read_crc()?;
                Ok(true)
            }
            Some(b'-') => {
                self.read_end();
                Ok(false)
            }
            _ => self.quad(next),
        }
    }

    /// Decodes the quad that starts with `first`; the input ends when `first` is `None`.
    fn quad(&mut self, first: Option<u8>) -> Result<bool, ArmorError> {
        if first.is_none() {
            self.at_eof = true;
            return Ok(false);
        }
        let rest = [
            first,
            self.read_ignore_space()?,
            self.read_ignore_space()?,
            self.read_ignore_space()?,
        ];
        let bytes = decode_quad(rest)?;
        for &b in &bytes {
            self.crc = crc24_update(self.crc, b);
        }
        self.pending.extend(bytes);
        Ok(true)
    }

    /// Reads the CRC line after its `=` and checks it against the CRC-24 of the block.
    fn read_crc(&mut self) -> Result<(), ArmorError> {
        let quad = [
            self.read_ignore_space()?,
            self.read_ignore_space()?,
            self.read_ignore_space()?,
            self.read_ignore_space()?,
        ];
        let bytes = decode_quad(quad)?;
        let received = match bytes.as_slice() {
            &[a, b, c] => u32::from(a) << 16 | u32::from(b) << 8 | u32::from(c),
            _ => return Err(ArmorError::MalformedCrc),
        };
        if received != self.crc {
            return Err(ArmorError::CrcCheckFailed);
        }
        Ok(())
    }

    /// Consumes the rest of the END line; the next read starts another block.
    fn read_end(&mut self) {
        while let Some(c) = self.raw_read() {
            if matches!(c, CR | LF) {
                break;
            }
        }
        self.start = true;
        self.restart = true;
    }

    /// Parses the armor headers at the start of a block, the BEGIN line included, and consumes
    /// the blank line that ends them.
    fn parse_headers(&mut self) -> Result<(), ArmorError> {
        if !self.restart && !self.find_begin() {
            return Ok(());
        }
        let mut line = vec![b'-'];
        if self.restart {
            line.push(b'-');
        }
        let mut first = true;
        let mut crlf = false;
        let mut last = None;
        while let Some(c) = self.raw_read() {
            let after_cr = last == Some(CR);
            last = Some(c);
            if c == LF && after_cr {
                crlf = true;
            } else if !matches!(c, CR | LF) {
                line.push(c);
            } else {
                if line.iter().all(|&b| b <= b' ') {
                    break;
                }
                if !first && !line.contains(&b':') {
                    return Err(ArmorError::InvalidArmorHeader);
                }
                first = false;
                line.clear();
            }
        }
        if crlf && self.raw_read() != Some(LF) {
            return Err(ArmorError::InconsistentLineEndings);
        }
        Ok(())
    }

    /// Skips the input up to a `-` that starts a line, and consumes it.
    fn find_begin(&mut self) -> bool {
        let mut last = 0;
        while let Some(c) = self.raw_read() {
            if c == b'-' && matches!(last, 0 | LF | CR) {
                return true;
            }
            last = c;
        }
        false
    }
}

/// Decodes a quad of base64 characters into its bytes, the last ones missing after a `=`.
fn decode_quad(quad: [Option<u8>; 4]) -> Result<Vec<u8>, ArmorError> {
    let [Some(c0), Some(c1), Some(c2), Some(c3)] = quad else {
        return Err(ArmorError::UnexpectedEnd);
    };
    let [v0, v1, v2, v3] = [c0, c1, c2, c3].map(sextet);
    if c2 == b'=' {
        // The masked tail of BouncyCastle: its characters are not checked.
        let (v0, v1) = (v0 & 0xFF, v1 & 0xFF);
        return Ok(vec![((v0 << 2) | (v1 >> 4)) as u8]);
    }
    if c3 == b'=' {
        if [v0, v1, v2].iter().any(|&v| v < 0) {
            return Err(ArmorError::InvalidArmor);
        }
        return Ok(vec![
            ((v0 << 2) | (v1 >> 4)) as u8,
            (((v1 << 4) | (v2 >> 2)) & 0xFF) as u8,
        ]);
    }
    if [v0, v1, v2, v3].iter().any(|&v| v < 0) {
        return Err(ArmorError::InvalidArmor);
    }
    Ok(vec![
        ((v0 << 2) | (v1 >> 4)) as u8,
        (((v1 << 4) | (v2 >> 2)) & 0xFF) as u8,
        (((v2 << 6) | v3) & 0xFF) as u8,
    ])
}

/// The value of a base64 character, or -1 for any other byte.
fn sextet(c: u8) -> i32 {
    match c {
        b'A'..=b'Z' => i32::from(c - b'A'),
        b'a'..=b'z' => i32::from(c - b'a') + 26,
        b'0'..=b'9' => i32::from(c - b'0') + 52,
        b'+' => 62,
        b'/' => 63,
        _ => -1,
    }
}

fn is_possibly_base64(b: u8) -> bool {
    sextet(b) >= 0 || matches!(b, CR | LF)
}

fn crc24_update(crc: u32, b: u8) -> u32 {
    let mut crc = crc ^ (u32::from(b) << 16);
    for _ in 0..8 {
        crc <<= 1;
        if crc & 0x0100_0000 != 0 {
            crc ^= CRC24_POLY;
        }
    }
    crc
}

fn crc24(data: &[u8]) -> u32 {
    data.iter().fold(CRC24_INIT, |crc, &b| crc24_update(crc, b))
}
