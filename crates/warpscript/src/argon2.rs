// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from BouncyCastle `Argon2BytesGenerator` (Copyright (c) 2000-2023 The Legion of the Bouncy Castle Inc., MIT-style license), ported to Rust and modified; BLAKE2b implemented from RFC 7693.

//! Argon2 (RFC 9106) as implemented by BouncyCastle's `Argon2BytesGenerator`, the engine
//! behind WarpLib's `ARGON2`.
//!
//! The port follows the Java class line by line: same parameter validation (and error texts),
//! same memory alignment (`memory` rounded down to a multiple of `4 * lanes` blocks, at least
//! `8 * lanes`), same `H'` variable length hash and BlaMka round function. The BLAKE2b hash it
//! relies on is implemented here as well (RFC 7693, unkeyed), because Argon2 needs output
//! lengths that are only known at run time.
//!
//! Unlike most Argon2 libraries (which refuse salts shorter than 8 bytes or less than 8 KiB of
//! memory), BouncyCastle accepts everything the specification allows, and so does this port.

use crate::error::Result;
use crate::error::crypto::CryptoError;
use crate::error::fault::ResourceError;

/// `Argon2Parameters.ARGON2_d`: data-dependent addressing.
pub const ARGON2_D: u32 = 0;
/// `Argon2Parameters.ARGON2_i`: data-independent addressing.
pub const ARGON2_I: u32 = 1;
/// `Argon2Parameters.ARGON2_id`: hybrid addressing.
pub const ARGON2_ID: u32 = 2;
/// `Argon2Parameters.ARGON2_VERSION_10`.
pub const VERSION_10: u32 = 0x10;
/// `Argon2Parameters.ARGON2_VERSION_13`.
pub const VERSION_13: u32 = 0x13;

const BLOCK_SIZE: usize = 1024;
const QWORDS_IN_BLOCK: usize = BLOCK_SIZE / 8;
const ADDRESSES_IN_BLOCK: usize = 128;
const PREHASH_DIGEST_LENGTH: usize = 64;
const PREHASH_SEED_LENGTH: usize = 72;
const SYNC_POINTS: usize = 4;
const MIN_PARALLELISM: i32 = 1;
const MAX_PARALLELISM: i32 = 16_777_216;
const MIN_OUTLEN: usize = 4;
const MIN_ITERATIONS: i32 = 1;

/// The parameters of a derivation (`Argon2Parameters`). The integers are Java `int`s: the
/// Warp 10 function truncates the `LONG`s of its parameter map to 32 bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Params {
    /// [`ARGON2_D`], [`ARGON2_I`] or [`ARGON2_ID`].
    pub kind: u32,
    /// [`VERSION_10`] or [`VERSION_13`].
    pub version: u32,
    /// Memory in KiB (one block each).
    pub memory: i32,
    /// Number of passes.
    pub iterations: i32,
    /// Degree of parallelism.
    pub lanes: i32,
    /// Salt (`None` and empty are equivalent).
    pub salt: Option<Vec<u8>>,
    /// Secret value (`K`).
    pub secret: Option<Vec<u8>>,
    /// Associated data (`X`).
    pub additional: Option<Vec<u8>>,
}

impl Default for Params {
    /// `new Argon2Parameters.Builder(ARGON2_id).withVersion(ARGON2_VERSION_13)` as built by
    /// Warp 10: 1 lane, 4096 KiB, 3 iterations, no salt.
    fn default() -> Self {
        Params {
            kind: ARGON2_ID,
            version: VERSION_13,
            memory: 1 << 12,
            iterations: 3,
            lanes: 1,
            salt: None,
            secret: None,
            additional: None,
        }
    }
}

/// One 1 KiB memory block of 128 little-endian 64-bit words.
#[derive(Clone)]
struct Block([u64; QWORDS_IN_BLOCK]);

impl Block {
    const ZERO: Block = Block([0; QWORDS_IN_BLOCK]);

    fn from_bytes(bytes: &[u8]) -> Block {
        let mut block = Block::ZERO;
        for (word, chunk) in block.0.iter_mut().zip(bytes.as_chunks::<8>().0) {
            *word = u64::from_le_bytes(*chunk);
        }
        block
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.0.iter().flat_map(|w| w.to_le_bytes()).collect()
    }

    /// `this = a ^ b`.
    fn xor(&mut self, a: &Block, b: &Block) {
        for ((out, x), y) in self.0.iter_mut().zip(&a.0).zip(&b.0) {
            *out = x ^ y;
        }
    }

    /// `this ^= other`.
    fn xor_with(&mut self, other: &Block) {
        for (out, x) in self.0.iter_mut().zip(&other.0) {
            *out ^= x;
        }
    }

    /// `this ^= a ^ b`.
    fn xor_with_both(&mut self, a: &Block, b: &Block) {
        for ((out, x), y) in self.0.iter_mut().zip(&a.0).zip(&b.0) {
            *out ^= x ^ y;
        }
    }
}

// -----------------------------------------------------------------------------------------
// BLAKE2b (RFC 7693)
// -----------------------------------------------------------------------------------------

const BLAKE2B_IV: [u64; 8] = [
    0x6a09_e667_f3bc_c908,
    0xbb67_ae85_84ca_a73b,
    0x3c6e_f372_fe94_f82b,
    0xa54f_f53a_5f1d_36f1,
    0x510e_527f_ade6_82d1,
    0x9b05_688c_2b3e_6c1f,
    0x1f83_d9ab_fb41_bd6b,
    0x5be0_cd19_137e_2179,
];

const BLAKE2B_SIGMA: [[usize; 16]; 12] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
];

/// An unkeyed BLAKE2b hash with an output of 1 to 64 bytes (`Blake2bDigest`).
pub(crate) struct Blake2b {
    h: [u64; 8],
    buffer: Vec<u8>,
    counter: u128,
    out_len: usize,
}

impl Blake2b {
    /// A digest producing `out_len` bytes (clamped to 1..=64, the sizes Argon2 uses).
    pub(crate) fn new(out_len: usize) -> Blake2b {
        let out_len = out_len.clamp(1, 64);
        let mut h = BLAKE2B_IV;
        h[0] ^= 0x0101_0000 ^ out_len as u64;
        Blake2b {
            h,
            buffer: Vec::with_capacity(128),
            counter: 0,
            out_len,
        }
    }

    pub(crate) fn update(&mut self, data: &[u8]) {
        for &byte in data {
            if self.buffer.len() == 128 {
                self.counter += 128;
                let block = std::mem::take(&mut self.buffer);
                self.compress(&block, false);
            }
            self.buffer.push(byte);
        }
    }

    pub(crate) fn finalize(mut self) -> Vec<u8> {
        self.counter += self.buffer.len() as u128;
        let mut last = std::mem::take(&mut self.buffer);
        last.resize(128, 0);
        self.compress(&last, true);
        self.h
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .take(self.out_len)
            .collect()
    }

    fn compress(&mut self, block: &[u8], last: bool) {
        let mut m = [0u64; 16];
        for (word, chunk) in m.iter_mut().zip(block.as_chunks::<8>().0) {
            *word = u64::from_le_bytes(*chunk);
        }
        let mut v = [0u64; 16];
        v[..8].copy_from_slice(&self.h);
        v[8..].copy_from_slice(&BLAKE2B_IV);
        v[12] ^= self.counter as u64;
        v[13] ^= (self.counter >> 64) as u64;
        if last {
            v[14] = !v[14];
        }
        for sigma in &BLAKE2B_SIGMA {
            blake2b_g(&mut v, 0, 4, 8, 12, m[sigma[0]], m[sigma[1]]);
            blake2b_g(&mut v, 1, 5, 9, 13, m[sigma[2]], m[sigma[3]]);
            blake2b_g(&mut v, 2, 6, 10, 14, m[sigma[4]], m[sigma[5]]);
            blake2b_g(&mut v, 3, 7, 11, 15, m[sigma[6]], m[sigma[7]]);
            blake2b_g(&mut v, 0, 5, 10, 15, m[sigma[8]], m[sigma[9]]);
            blake2b_g(&mut v, 1, 6, 11, 12, m[sigma[10]], m[sigma[11]]);
            blake2b_g(&mut v, 2, 7, 8, 13, m[sigma[12]], m[sigma[13]]);
            blake2b_g(&mut v, 3, 4, 9, 14, m[sigma[14]], m[sigma[15]]);
        }
        for (i, h) in self.h.iter_mut().enumerate() {
            *h ^= v[i] ^ v[i + 8];
        }
    }
}

/// The BLAKE2b mixing function `G`.
#[allow(clippy::many_single_char_names)]
fn blake2b_g(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

/// BLAKE2b of the concatenation of `parts`.
pub(crate) fn blake2b(out_len: usize, parts: &[&[u8]]) -> Vec<u8> {
    let mut digest = Blake2b::new(out_len);
    for part in parts {
        digest.update(part);
    }
    digest.finalize()
}

/// `H'`, the variable length hash of Argon2 (`Argon2BytesGenerator.hash`).
fn hash_prime(input: &[u8], out_len: usize) -> Vec<u8> {
    let len_bytes = (out_len as u32).to_le_bytes();
    if out_len <= 64 {
        return blake2b(out_len, &[&len_bytes, input]);
    }
    let mut out = Vec::with_capacity(out_len);
    let mut buffer = blake2b(64, &[&len_bytes, input]);
    out.extend_from_slice(&buffer[..32]);
    let r = out_len.div_ceil(32) - 2;
    for _ in 2..=r {
        buffer = blake2b(64, &[&buffer]);
        out.extend_from_slice(&buffer[..32]);
    }
    let last_length = out_len - 32 * r;
    out.extend(blake2b(last_length, &[&buffer]));
    out
}

// -----------------------------------------------------------------------------------------
// Argon2
// -----------------------------------------------------------------------------------------

/// The BlaMka quarter round: `a += b + 2 * low32(a) * low32(b); c = rotr(c ^ a, s)`.
#[allow(clippy::many_single_char_names)]
fn quarter_round(v: &mut [u64; QWORDS_IN_BLOCK], x: usize, y: usize, z: usize, s: u32) {
    let (a, b, c) = (v[x], v[y], v[z]);
    let a = a
        .wrapping_add(b)
        .wrapping_add(2u64.wrapping_mul((a & 0xFFFF_FFFF).wrapping_mul(b & 0xFFFF_FFFF)));
    v[x] = a;
    v[z] = (c ^ a).rotate_right(s);
}

#[allow(clippy::many_single_char_names)]
fn mix(v: &mut [u64; QWORDS_IN_BLOCK], a: usize, b: usize, c: usize, d: usize) {
    quarter_round(v, a, b, d, 32);
    quarter_round(v, c, d, b, 24);
    quarter_round(v, a, b, d, 16);
    quarter_round(v, c, d, b, 63);
}

/// The Argon2 permutation `P` on sixteen words of a block.
fn round_function(block: &mut Block, idx: [usize; 16]) {
    let v = &mut block.0;
    mix(v, idx[0], idx[4], idx[8], idx[12]);
    mix(v, idx[1], idx[5], idx[9], idx[13]);
    mix(v, idx[2], idx[6], idx[10], idx[14]);
    mix(v, idx[3], idx[7], idx[11], idx[15]);
    mix(v, idx[0], idx[5], idx[10], idx[15]);
    mix(v, idx[1], idx[6], idx[11], idx[12]);
    mix(v, idx[2], idx[7], idx[8], idx[13]);
    mix(v, idx[3], idx[4], idx[9], idx[14]);
}

/// `FillBlock.applyBlake`: the permutation on the columns then on the rows of a block.
fn apply_blake(z: &mut Block) {
    for i in 0..8 {
        let i16 = 16 * i;
        round_function(z, std::array::from_fn(|k| i16 + k));
    }
    for i in 0..8 {
        let i2 = 2 * i;
        round_function(z, std::array::from_fn(|k| i2 + 16 * (k / 2) + (k % 2)));
    }
}

/// `G(X, Y)`: the compression function; `xor` selects `fillBlockWithXor` (the result is
/// XORed into the destination instead of replacing it).
fn fill_block(x: &Block, y: &Block, dest: &mut Block, xor: bool) {
    let mut r = Block::ZERO;
    r.xor(x, y);
    let mut z = r.clone();
    apply_blake(&mut z);
    if xor {
        dest.xor_with_both(&r, &z);
    } else {
        dest.xor(&r, &z);
    }
}

struct Position {
    pass: i32,
    lane: usize,
    slice: usize,
}

/// The state of one derivation (`Argon2BytesGenerator` after `init`).
struct Generator<'a> {
    params: &'a Params,
    memory: Vec<Block>,
    segment_length: usize,
    lane_length: usize,
    lanes: usize,
}

/// `Argon2BytesGenerator.init` + `generateBytes`: derive `out_len` bytes from `password`.
///
/// The errors carry the texts of the `IllegalStateException`s (and of the
/// `NegativeArraySizeException` for a negative `out_len`) of the Java implementation.
pub fn generate(params: &Params, password: &[u8], out_len: i64) -> Result<Vec<u8>> {
    let out_len =
        usize::try_from(out_len).map_err(|_| CryptoError::NegativeOutputLength(out_len))?;
    if params.lanes < MIN_PARALLELISM {
        return Err(CryptoError::LanesTooFew {
            min: MIN_PARALLELISM,
        }
        .into());
    }
    if params.lanes > MAX_PARALLELISM {
        return Err(CryptoError::LanesTooMany {
            max: MAX_PARALLELISM,
        }
        .into());
    }
    if params.memory < 2 * params.lanes {
        return Err(CryptoError::MemoryTooSmall {
            min: 2 * params.lanes,
        }
        .into());
    }
    if params.iterations < MIN_ITERATIONS {
        return Err(CryptoError::IterationsTooFew {
            min: MIN_ITERATIONS,
        }
        .into());
    }
    if out_len < MIN_OUTLEN {
        return Err(CryptoError::OutputTooShort { min: MIN_OUTLEN }.into());
    }
    let lanes = params.lanes as usize;
    let memory_blocks = (params.memory as usize).max(2 * SYNC_POINTS * lanes);
    let segment_length = memory_blocks / (lanes * SYNC_POINTS);
    let lane_length = segment_length * SYNC_POINTS;
    let memory_blocks = segment_length * lanes * SYNC_POINTS;
    let mut generator = Generator {
        params,
        memory: allocate(memory_blocks)?,
        segment_length,
        lane_length,
        lanes,
    };
    generator.initialize(password, out_len);
    generator.fill_memory_blocks();
    Ok(generator.digest(out_len))
}

/// `memory_blocks` zeroed blocks, or an error when the allocation fails (Java: an
/// `OutOfMemoryError`).
fn allocate(memory_blocks: usize) -> Result<Vec<Block>> {
    let mut memory: Vec<Block> = Vec::new();
    memory
        .try_reserve_exact(memory_blocks)
        .map_err(|_| ResourceError::OutOfMemory)?;
    memory.resize(memory_blocks, Block::ZERO);
    Ok(memory)
}

impl Generator<'_> {
    /// `H0` and the first two blocks of every lane.
    fn initialize(&mut self, password: &[u8], out_len: usize) {
        let params = self.params;
        let mut h0 = Blake2b::new(PREHASH_DIGEST_LENGTH);
        for value in [
            params.lanes as u32,
            out_len as u32,
            params.memory as u32,
            params.iterations as u32,
            params.version,
            params.kind,
        ] {
            h0.update(&value.to_le_bytes());
        }
        for bytes in [
            Some(password),
            params.salt.as_deref(),
            params.secret.as_deref(),
            params.additional.as_deref(),
        ] {
            let bytes = bytes.unwrap_or_default();
            h0.update(&(bytes.len() as u32).to_le_bytes());
            h0.update(bytes);
        }
        let mut seed = h0.finalize();
        seed.resize(PREHASH_SEED_LENGTH, 0);
        for lane in 0..self.lanes {
            for (block_index, tag) in [(0usize, 0u8), (1, 1)] {
                seed[PREHASH_DIGEST_LENGTH] = tag;
                seed[PREHASH_DIGEST_LENGTH + 4..PREHASH_DIGEST_LENGTH + 8]
                    .copy_from_slice(&(lane as u32).to_le_bytes());
                let bytes = hash_prime(&seed, BLOCK_SIZE);
                self.memory[lane * self.lane_length + block_index] = Block::from_bytes(&bytes);
            }
        }
    }

    fn fill_memory_blocks(&mut self) {
        for pass in 0..self.params.iterations {
            for slice in 0..SYNC_POINTS {
                for lane in 0..self.lanes {
                    self.fill_segment(&Position { pass, lane, slice });
                }
            }
        }
    }

    fn data_independent_addressing(&self, position: &Position) -> bool {
        self.params.kind == ARGON2_I
            || (self.params.kind == ARGON2_ID
                && position.pass == 0
                && position.slice < SYNC_POINTS / 2)
    }

    fn fill_segment(&mut self, position: &Position) {
        let data_independent = self.data_independent_addressing(position);
        let starting_index = usize::from(position.pass == 0 && position.slice == 0) * 2;
        let segment_start = position.lane * self.lane_length + position.slice * self.segment_length;
        let first_offset = segment_start + starting_index;
        // The block before the first one of the segment, wrapping around the lane.
        let mut prev_offset = if first_offset.is_multiple_of(self.lane_length) {
            first_offset + self.lane_length - 1
        } else {
            first_offset - 1
        };
        let mut address_block = Block::ZERO;
        let mut input_block = Block::ZERO;
        if data_independent {
            input_block.0[0] = u64::from(position.pass as u32);
            input_block.0[1] = position.lane as u64;
            input_block.0[2] = position.slice as u64;
            input_block.0[3] = self.memory.len() as u64;
            input_block.0[4] = u64::from(self.params.iterations as u32);
            input_block.0[5] = u64::from(self.params.kind);
            if position.pass == 0 && position.slice == 0 {
                next_addresses(&mut input_block, &mut address_block);
            }
        }
        let with_xor = !(position.pass == 0 || self.params.version == VERSION_10);
        for index in starting_index..self.segment_length {
            let pseudo_random = if data_independent {
                let address_index = index % ADDRESSES_IN_BLOCK;
                if address_index == 0 {
                    next_addresses(&mut input_block, &mut address_block);
                }
                address_block.0[address_index]
            } else {
                self.memory[prev_offset].0[0]
            };
            let ref_lane = if position.pass == 0 && position.slice == 0 {
                position.lane
            } else {
                ((pseudo_random >> 32) % self.lanes as u64) as usize
            };
            let ref_column =
                self.ref_column(position, index, pseudo_random, ref_lane == position.lane);
            let current_offset = segment_start + index;
            let prev_block = self.memory[prev_offset].clone();
            let ref_block = self.memory[self.lane_length * ref_lane + ref_column].clone();
            fill_block(
                &prev_block,
                &ref_block,
                &mut self.memory[current_offset],
                with_xor,
            );
            prev_offset = current_offset;
        }
    }

    /// `getRefColumn`: the index of the reference block within its lane.
    fn ref_column(
        &self,
        position: &Position,
        index: usize,
        pseudo_random: u64,
        same_lane: bool,
    ) -> usize {
        let (reference_area_size, start_position): (i64, usize) = if position.pass == 0 {
            let base = (position.slice * self.segment_length) as i64;
            if same_lane {
                (base + index as i64 - 1, 0)
            } else {
                (base - i64::from(index == 0), 0)
            }
        } else {
            let base = (self.lane_length - self.segment_length) as i64;
            let start = ((position.slice + 1) * self.segment_length) % self.lane_length;
            if same_lane {
                (base + index as i64 - 1, start)
            } else {
                (base - i64::from(index == 0), start)
            }
        };
        let relative = pseudo_random & 0xFFFF_FFFF;
        let relative = (relative * relative) >> 32;
        let relative = reference_area_size - 1 - ((reference_area_size * relative as i64) >> 32);
        ((start_position as i64 + relative) % self.lane_length as i64) as usize
    }

    /// XOR the last block of every lane and apply `H'`.
    fn digest(&mut self, out_len: usize) -> Vec<u8> {
        let mut final_block = self.memory[self.lane_length - 1].clone();
        for lane in 1..self.lanes {
            let last = &self.memory[lane * self.lane_length + self.lane_length - 1];
            final_block.xor_with(last);
        }
        hash_prime(&final_block.to_bytes(), out_len)
    }
}

/// `nextAddresses`: the next block of 128 pseudo-random addresses.
fn next_addresses(input_block: &mut Block, address_block: &mut Block) {
    input_block.0[6] += 1;
    let zero = Block::ZERO;
    fill_block(input_block, &zero, address_block, false);
    let copy = address_block.clone();
    fill_block(&copy, &zero, address_block, false);
}
