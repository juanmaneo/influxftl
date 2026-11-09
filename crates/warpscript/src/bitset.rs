// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! An immutable `java.util.BitSet`, the value type produced by `BYTESTOBITS`.
//!
//! The set is stored as the little-endian bytes of `BitSet.toByteArray()`: bit `i` lives in
//! byte `i / 8` at position `i % 8`, and trailing zero bytes are never kept, so two sets with
//! the same bits have the same bytes (`BitSet.equals` semantics come for free).

use std::fmt;

use crate::error::Result;
use crate::error::fault::BoundsError;

/// A set of bits indexed from 0 (`java.util.BitSet`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct BitSet {
    bytes: Vec<u8>,
}

impl BitSet {
    /// `BitSet.valueOf(byte[])`: bit `8 * k + j` is bit `j` of byte `k`.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let used = bytes.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
        BitSet {
            bytes: bytes[..used].to_vec(),
        }
    }

    /// `BitSet.toByteArray()`: the shortest little-endian byte array holding every set bit.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// `BitSet.get(int)`: `true` when the bit at `index` is set. A negative index is an error
    /// (`IndexOutOfBoundsException`).
    pub fn get(&self, index: i32) -> Result<bool> {
        if index < 0 {
            return Err(BoundsError::NegativeBitIndex(i64::from(index)).into());
        }
        let index = index as usize;
        Ok(self
            .bytes
            .get(index / 8)
            .is_some_and(|byte| byte & (1 << (index % 8)) != 0))
    }

    /// `BitSet.length()`: the index of the highest set bit plus one, 0 for an empty set.
    pub fn length(&self) -> i64 {
        match self.bytes.last() {
            Some(last) => (self.bytes.len() as i64 - 1) * 8 + i64::from(8 - last.leading_zeros()),
            None => 0,
        }
    }

    /// `BitSet.cardinality()`: the number of set bits.
    pub fn cardinality(&self) -> i64 {
        self.bytes.iter().map(|b| i64::from(b.count_ones())).sum()
    }

    /// Indices of the set bits, ascending.
    pub fn set_bits(&self) -> impl Iterator<Item = usize> + '_ {
        self.bytes.iter().enumerate().flat_map(|(k, &b)| {
            (0..8)
                .filter(move |j| b & (1 << j) != 0)
                .map(move |j| 8 * k + j)
        })
    }
}

impl fmt::Display for BitSet {
    /// `BitSet.toString()`: the set bits between braces, e.g. `{0, 2, 23}`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{")?;
        for (n, bit) in self.set_bits().enumerate() {
            if n > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{bit}")?;
        }
        f.write_str("}")
    }
}
