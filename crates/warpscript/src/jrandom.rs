// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.
// Derived from Warp 10 (Copyright 2018-2026 SenX S.A.S., Apache-2.0), ported to Rust and modified.

//! `java.util.Random`, the 48-bit linear congruential generator behind the seeded WarpScript
//! functions (`PRNG` / `SRAND`…), implemented from the algorithm its specification publishes
//! (multiplier, increment, seed scrambling and the derivation of each `next*` method), so that
//! a given seed yields the exact Java sequence.

const MULTIPLIER: i64 = 0x5_DEEC_E66D;
const ADDEND: i64 = 0xB;
const MASK: i64 = (1 << 48) - 1;
/// `0x1.0p-53`, the unit in the last place of a double in `[0, 1)`.
const DOUBLE_UNIT: f64 = 1.0 / (1u64 << 53) as f64;

/// A `java.util.Random` instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaRandom {
    /// The 48-bit state (`seed` in the JDK).
    seed: i64,
}

impl JavaRandom {
    /// `new Random(seed)`.
    pub fn new(seed: i64) -> Self {
        JavaRandom {
            seed: (seed ^ MULTIPLIER) & MASK,
        }
    }

    /// Rebuild a generator from a state saved with [`JavaRandom::state`].
    pub fn from_state(state: i64) -> Self {
        JavaRandom { seed: state & MASK }
    }

    /// The internal state, to persist the generator between calls.
    pub fn state(&self) -> i64 {
        self.seed
    }

    /// `Random.next(bits)`: advance the generator and return the top `bits` (1..=32) bits of
    /// the new state.
    pub fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// `Random.nextInt()`.
    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// `Random.nextInt(bound)`: uniform in `[0, bound)`; `bound` must be positive.
    pub fn next_int_bounded(&mut self, bound: i32) -> i32 {
        let mut r = self.next(31);
        let m = bound - 1;
        if (bound & m) == 0 {
            // Power of two: take the high bits.
            ((i64::from(bound) * i64::from(r)) >> 31) as i32
        } else {
            // Reject the values of the last, incomplete, bucket.
            let mut u = r;
            loop {
                r = u % bound;
                if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                    return r;
                }
                u = self.next(31);
            }
        }
    }

    /// `Random.nextLong()`.
    pub fn next_long(&mut self) -> i64 {
        (i64::from(self.next(32)) << 32).wrapping_add(i64::from(self.next(32)))
    }

    /// `Random.nextBoolean()`.
    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    /// `Random.nextDouble()`: uniform in `[0, 1)`.
    pub fn next_double(&mut self) -> f64 {
        let high = i64::from(self.next(26)) << 27;
        let low = i64::from(self.next(27));
        (high + low) as f64 * DOUBLE_UNIT
    }
}
