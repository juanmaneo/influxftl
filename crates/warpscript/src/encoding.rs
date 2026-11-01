//! Binary-to-text encodings used by WarpScript: order preserving base64 (`OPB64`), hex.

use crate::error::{Error, Result};

/// Alphabet of the order preserving base64 encoding (ASCII order is preserved).
pub(crate) const OPB64_ALPHABET: &[u8; 64] =
    b".0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";

/// Value of an OPB64 digit, `None` for characters outside the alphabet.
pub(crate) fn opb64_value(c: u8) -> Option<u8> {
    match c {
        b'.' => Some(0),
        b'0'..=b'9' => Some(c - b'0' + 1),
        b'A'..=b'Z' => Some(c - b'A' + 11),
        b'_' => Some(37),
        b'a'..=b'z' => Some(c - b'a' + 38),
        _ => None,
    }
}

/// `OrderPreservingBase64.encode`: base64 bit layout, no padding, order preserving alphabet.
pub fn opb64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let (chunks, rest) = data.as_chunks::<3>();
    for chunk in chunks {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(chunk[1]) << 8) | u32::from(chunk[2]);
        out.push(OPB64_ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(OPB64_ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(OPB64_ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        out.push(OPB64_ALPHABET[(n & 0x3F) as usize] as char);
    }
    match rest {
        [a] => {
            out.push(OPB64_ALPHABET[(a >> 2) as usize] as char);
            out.push(OPB64_ALPHABET[((a << 4) & 0x30) as usize] as char);
        }
        [a, b] => {
            out.push(OPB64_ALPHABET[(a >> 2) as usize] as char);
            out.push(OPB64_ALPHABET[(((a & 0x3) << 4) | (b >> 4)) as usize] as char);
            out.push(OPB64_ALPHABET[((b << 2) & 0x3C) as usize] as char);
        }
        _ => {}
    }
    out
}

/// `OrderPreservingBase64.decode`. Invalid characters are errors.
pub fn opb64_decode(text: &str) -> Result<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut nbits = 0;
    for (idx, &c) in bytes.iter().enumerate() {
        let v = opb64_value(c).ok_or_else(|| {
            Error::new(format!(
                "Invalid OPB64 character '{}' at offset {idx}.",
                c as char
            ))
        })?;
        acc = (acc << 6) | u32::from(v);
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push(((acc >> nbits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

/// Lowercase hexadecimal rendering of `data`.
pub fn hex_encode(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(data.len() * 2);
    for &b in data {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xF) as usize] as char);
    }
    out
}

/// `WarpHexDecoder.decode`: decode a hexadecimal string (either case, no separators). An odd
/// number of characters (UTF-16 units, like Java) or an invalid digit are errors with the
/// Java messages.
pub fn hex_decode(text: &str) -> Result<Vec<u8>> {
    if !text.encode_utf16().count().is_multiple_of(2) {
        return Err(Error::new("Odd number of hexadecimal digits."));
    }
    let mut out = Vec::with_capacity(text.len() / 2);
    let mut digits = text.chars().map(|c| c.to_digit(16));
    while let Some(hi) = digits.next() {
        match (hi, digits.next().flatten()) {
            (Some(hi), Some(lo)) => out.push((hi * 16 + lo) as u8),
            _ => return Err(Error::new("Invalid hex character.")),
        }
    }
    Ok(out)
}
