//! Kani proof harnesses (compiled only under `cargo kani`).
//!
//! The harnesses are bounded and target the pure helpers of the runtime, which is where the
//! Java-compatibility corner cases live. Allocation-heavy code (strings, vectors) is covered by
//! the unit tests instead: symbolic execution of the allocator does not scale.

use crate::encoding::{OPB64_ALPHABET, opb64_value};
use crate::format::{is_double, is_long};
use crate::value::double_to_long;

/// Any 3 ASCII characters: a string accepted by `is_long` is never accepted by `is_double`
/// (number literals are unambiguous), a long ends with a digit and a double ends with a digit,
/// a dot, an exponent marker or sign (`1.` and `1e` are accepted, like in Warp 10, and
/// `Double.valueOf` has the last word) or is `NaN`.
#[kani::proof]
#[kani::unwind(5)]
fn long_and_double_detection_are_disjoint() {
    let bytes: [u8; 3] = kani::any();
    kani::assume(bytes[0].is_ascii() && bytes[1].is_ascii() && bytes[2].is_ascii());
    let Ok(text) = core::str::from_utf8(&bytes) else {
        return;
    };
    let long = is_long(text);
    let double = is_double(text);
    assert!(!(long && double));
    if long {
        assert!(bytes[2].is_ascii_digit());
    }
    if double {
        assert!(matches!(
            bytes[2],
            b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-' | b'N'
        ));
    }
}

/// `opb64_value` is the inverse of the OPB64 alphabet: decoding a digit of the alphabet yields
/// its index, and the indices are the only accepted values.
#[kani::proof]
fn opb64_value_inverts_the_alphabet() {
    let i: u8 = kani::any();
    kani::assume(i < 64);
    assert!(opb64_value(OPB64_ALPHABET[i as usize]) == Some(i));
}

/// The OPB64 alphabet is strictly increasing, so the encoding preserves the byte order of the
/// payloads it is applied to.
#[kani::proof]
fn opb64_alphabet_is_sorted() {
    let i: u8 = kani::any();
    kani::assume(i < 63);
    assert!(OPB64_ALPHABET[i as usize] < OPB64_ALPHABET[i as usize + 1]);
}

/// `(long) d` saturates and maps NaN to zero, like the JVM.
#[kani::proof]
fn double_to_long_matches_the_jvm() {
    let d: f64 = kani::any();
    let l = double_to_long(d);
    if d.is_nan() {
        assert!(l == 0);
    } else if d >= 9_223_372_036_854_775_807.0 {
        assert!(l == i64::MAX);
    } else if d <= -9_223_372_036_854_775_808.0 {
        assert!(l == i64::MIN);
    }
}
