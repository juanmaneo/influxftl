// Copyright 2026 The influxftl Authors. Licensed under the Apache License, Version 2.0.

//! Java's `java.math.BigDecimal` (and the `BigInteger` operations the `BD*` functions use) on
//! top of `num-bigint`: an arbitrary precision unscaled integer and a 32-bit scale, the value
//! being `unscaled × 10^-scale`. The semantics (scales of the results, rounding, exceptions and
//! their messages, `toString`) are those of the JDK.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::error::fault::{ArithmeticError, NumberFormatError};
use crate::format::decimal_value;

/// The result of a decimal or integer operation.
type ArithResult<T> = Result<T, ArithmeticError>;
/// The result of reading a decimal or an integer.
type ParseResult<T> = Result<T, NumberFormatError>;

/// Rounding modes of `setScale` (besides `UNNECESSARY`, see [`BigDecimal::set_scale`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundingMode {
    /// Towards positive infinity.
    Ceiling,
    /// Towards negative infinity.
    Floor,
    /// To the nearest neighbour, ties away from zero.
    HalfUp,
}

/// `10^n` for `n >= 0`.
fn ten_pow(n: u32) -> BigInt {
    num_traits::pow(BigInt::from(10u8), n as usize)
}

/// A Java `BigDecimal`.
#[derive(Clone, Debug)]
#[must_use]
pub struct BigDecimal {
    /// The unscaled value.
    pub unscaled: BigInt,
    /// The scale.
    pub scale: i32,
}

impl PartialEq for BigDecimal {
    /// `equals`: same value and same scale (`2.0` is not `2.00`).
    fn eq(&self, other: &Self) -> bool {
        self.scale == other.scale && self.unscaled == other.unscaled
    }
}

impl Eq for BigDecimal {}

impl Hash for BigDecimal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.unscaled.hash(state);
        self.scale.hash(state);
    }
}

impl BigDecimal {
    /// `new BigDecimal(unscaled, scale)`.
    pub fn new(unscaled: BigInt, scale: i32) -> BigDecimal {
        BigDecimal { unscaled, scale }
    }

    /// `BigDecimal.valueOf(long)`.
    pub fn from_i64(value: i64) -> BigDecimal {
        BigDecimal::new(BigInt::from(value), 0)
    }

    /// `new BigDecimal(BigInteger)`.
    pub fn from_bigint(value: BigInt) -> BigDecimal {
        BigDecimal::new(value, 0)
    }

    /// `BigDecimal.valueOf(double)`: `new BigDecimal(Double.toString(d))`.
    pub fn from_f64(value: f64) -> ParseResult<BigDecimal> {
        if !value.is_finite() {
            return Err(NumberFormatError::NotFinite);
        }
        Ok(BigDecimal::from_finite_f64(value))
    }

    /// [`BigDecimal::from_f64`] of a finite double (whose `Double.toString` always parses).
    pub fn from_finite_f64(value: f64) -> BigDecimal {
        BigDecimal::parse(&crate::format::java_double_to_string(value))
            .unwrap_or(BigDecimal::from_i64(0))
    }

    /// `new BigDecimal(double)`: the exact binary value of a finite double.
    pub fn from_f64_exact(value: f64) -> ParseResult<BigDecimal> {
        if !value.is_finite() {
            return Err(NumberFormatError::NotFinite);
        }
        if value == 0.0 {
            return Ok(BigDecimal::from_i64(0));
        }
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let mut mantissa = (bits & ((1u64 << 52) - 1)) as i64;
        let mut exponent = if exponent == 0 {
            -1074
        } else {
            mantissa |= 1i64 << 52;
            exponent - 1075
        };
        while mantissa & 1 == 0 {
            mantissa >>= 1;
            exponent += 1;
        }
        if value < 0.0 {
            mantissa = -mantissa;
        }
        let mantissa = BigInt::from(mantissa);
        Ok(if exponent >= 0 {
            BigDecimal::new(mantissa << exponent as usize, 0)
        } else {
            let five = num_traits::pow(BigInt::from(5u8), exponent.unsigned_abs() as usize);
            BigDecimal::new(mantissa * five, -exponent)
        })
    }

    /// `new BigDecimal(String)`: an optional sign, digits with an optional decimal point and an
    /// optional exponent (`NumberFormatException` without message for an empty string or
    /// exponent).
    pub fn parse(text: &str) -> ParseResult<BigDecimal> {
        let bare = || NumberFormatError::Bare;
        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return Err(bare());
        }
        let mut i = 0;
        let negative = match chars[0] {
            '-' => {
                i = 1;
                true
            }
            '+' => {
                i = 1;
                false
            }
            _ => false,
        };
        let mut digits = String::new();
        let mut scale: i64 = 0;
        let mut dot = false;
        while i < chars.len() {
            let c = chars[i];
            if c.is_ascii_digit() {
                digits.push(c);
                if dot {
                    scale += 1;
                }
            } else if c == '.' {
                if dot {
                    return Err(NumberFormatError::MultipleDecimalPoints);
                }
                dot = true;
            } else if c == 'e' || c == 'E' {
                break;
            } else {
                return Err(NumberFormatError::InvalidDecimalCharacter(c));
            }
            i += 1;
        }
        if digits.is_empty() {
            return Err(NumberFormatError::NoDigits);
        }
        if i < chars.len() {
            // `parseExp`: a sign, then digits (leading zeros ignored, ten at most).
            let mut exponent = &chars[i + 1..];
            let negative_exponent = exponent.first() == Some(&'-');
            if matches!(exponent.first(), Some('-' | '+')) {
                exponent = &exponent[1..];
            }
            if exponent.is_empty() {
                return Err(bare());
            }
            if exponent.iter().any(|c| !c.is_ascii_digit()) {
                return Err(NumberFormatError::NotADigit);
            }
            let significant: String = exponent.iter().skip_while(|c| **c == '0').collect();
            if significant.len() > 10 {
                return Err(NumberFormatError::ExponentTooLong);
            }
            let mut value = decimal_value(&significant);
            if negative_exponent {
                value = -value;
            }
            scale -= value;
        }
        let scale = i32::try_from(scale).map_err(|_| NumberFormatError::ScaleOutOfRange)?;
        let mut unscaled: BigInt = digits.parse().unwrap_or_default();
        if negative {
            unscaled = -unscaled;
        }
        Ok(BigDecimal::new(unscaled, scale))
    }

    /// `signum`.
    pub fn signum(&self) -> i32 {
        match self.unscaled.sign() {
            Sign::Minus => -1,
            Sign::NoSign => 0,
            Sign::Plus => 1,
        }
    }

    /// The unscaled value at scale `scale` (not smaller than the current one).
    fn unscaled_at(&self, scale: i32) -> BigInt {
        &self.unscaled * ten_pow((i64::from(scale) - i64::from(self.scale)) as u32)
    }

    /// `add`: the scale is the larger one.
    pub fn add(&self, other: &BigDecimal) -> BigDecimal {
        let scale = self.scale.max(other.scale);
        BigDecimal::new(self.unscaled_at(scale) + other.unscaled_at(scale), scale)
    }

    /// `subtract`.
    pub fn subtract(&self, other: &BigDecimal) -> BigDecimal {
        self.add(&other.negate())
    }

    /// `multiply`: the scales add up.
    pub fn multiply(&self, other: &BigDecimal) -> ArithResult<BigDecimal> {
        let scale = self.check_scale(i64::from(self.scale) + i64::from(other.scale))?;
        Ok(BigDecimal::new(&self.unscaled * &other.unscaled, scale))
    }

    /// `multiply` of operands whose scales add up within an `int`.
    pub fn times(&self, other: &BigDecimal) -> BigDecimal {
        BigDecimal::new(
            &self.unscaled * &other.unscaled,
            saturate(i64::from(self.scale) + i64::from(other.scale)),
        )
    }

    /// `checkScale`: a scale computed in `long`, saturated for zero, `ArithmeticException`
    /// otherwise when it does not fit an `int`.
    fn check_scale(&self, scale: i64) -> ArithResult<i32> {
        match i32::try_from(scale) {
            Ok(scale) => Ok(scale),
            Err(_) if self.unscaled.is_zero() => Ok(saturate(scale)),
            Err(_) if scale > 0 => Err(ArithmeticError::Underflow),
            Err(_) => Err(ArithmeticError::Overflow),
        }
    }

    /// `negate`.
    pub fn negate(&self) -> BigDecimal {
        BigDecimal::new(-&self.unscaled, self.scale)
    }

    /// `abs`.
    pub fn abs(&self) -> BigDecimal {
        BigDecimal::new(self.unscaled.abs(), self.scale)
    }

    /// `compareTo`: numerical order (`2.0` equals `2.00`).
    pub fn compare(&self, other: &BigDecimal) -> Ordering {
        let scale = self.scale.max(other.scale);
        self.unscaled_at(scale).cmp(&other.unscaled_at(scale))
    }

    /// `ulp`: one unit in the last place.
    pub fn ulp(&self) -> BigDecimal {
        BigDecimal::new(BigInt::one(), self.scale)
    }

    /// `stripTrailingZeros` (zero is `0` with scale 0).
    pub fn strip_trailing_zeros(&self) -> BigDecimal {
        if self.unscaled.is_zero() {
            return BigDecimal::from_i64(0);
        }
        let mut unscaled = self.unscaled.clone();
        let mut scale = i64::from(self.scale);
        let ten = BigInt::from(10u8);
        loop {
            let (q, r) = unscaled.div_rem(&ten);
            if !r.is_zero() || scale <= i64::from(i32::MIN) {
                break;
            }
            unscaled = q;
            scale -= 1;
        }
        BigDecimal::new(unscaled, scale as i32)
    }

    /// `setScale(newScale, mode)`.
    pub fn round(&self, scale: i32, mode: RoundingMode) -> BigDecimal {
        if scale >= self.scale {
            return BigDecimal::new(self.unscaled_at(scale), scale);
        }
        let divisor = ten_pow((i64::from(self.scale) - i64::from(scale)) as u32);
        BigDecimal::new(divide_rounded(&self.unscaled, &divisor, mode), scale)
    }

    /// `setScale(newScale)` (`UNNECESSARY`): `ArithmeticException` when digits are lost.
    pub fn set_scale(&self, scale: i32) -> ArithResult<BigDecimal> {
        if scale >= self.scale {
            return Ok(BigDecimal::new(self.unscaled_at(scale), scale));
        }
        let divisor = ten_pow((i64::from(self.scale) - i64::from(scale)) as u32);
        let (q, r) = self.unscaled.div_rem(&divisor);
        if r.is_zero() {
            Ok(BigDecimal::new(q, scale))
        } else {
            Err(ArithmeticError::RoundingNecessary)
        }
    }

    /// `toBigInteger`: the integer part.
    pub fn to_bigint(&self) -> BigInt {
        if self.scale <= 0 {
            self.unscaled_at(0)
        } else {
            &self.unscaled / ten_pow(self.scale as u32)
        }
    }

    /// `toBigIntegerExact`, `None` with a fractional part.
    pub fn to_bigint_exact(&self) -> Option<BigInt> {
        if self.scale <= 0 {
            return Some(self.unscaled_at(0));
        }
        let (q, r) = self.unscaled.div_rem(&ten_pow(self.scale as u32));
        r.is_zero().then_some(q)
    }

    /// `intValueExact`.
    pub fn int_value_exact(&self) -> ArithResult<i32> {
        let integer = self
            .to_bigint_exact()
            .ok_or(ArithmeticError::RoundingNecessary)?;
        integer.to_i32().ok_or(ArithmeticError::Overflow)
    }

    /// `longValue`: the low 64 bits of the integer part.
    pub fn long_value(&self) -> i64 {
        let integer = self.to_bigint();
        let (sign, digits) = integer.to_u64_digits();
        let low = digits.first().copied().unwrap_or_default();
        let value = low as i64;
        if sign == Sign::Minus {
            value.wrapping_neg()
        } else {
            value
        }
    }

    /// `doubleValue`, correctly rounded.
    pub fn double_value(&self) -> f64 {
        format!("{}E{}", self.unscaled, -i64::from(self.scale))
            .parse()
            .unwrap_or(f64::NAN)
    }

    /// `pow(n)` for `0 <= n <= 999999999`.
    pub fn pow(&self, n: i32) -> ArithResult<BigDecimal> {
        if !(0..=999_999_999).contains(&n) {
            return Err(ArithmeticError::InvalidOperation);
        }
        let scale = self.check_scale(i64::from(self.scale) * i64::from(n))?;
        Ok(BigDecimal::new(
            num_traits::pow(self.unscaled.clone(), n as usize),
            scale,
        ))
    }

    /// `divide(divisor)`: the exact quotient, at the preferred scale `this.scale -
    /// divisor.scale` when that represents it, `ArithmeticException` when it has no
    /// terminating decimal expansion within the precision Java allows.
    pub fn divide(&self, divisor: &BigDecimal) -> ArithResult<BigDecimal> {
        let preferred = saturate(i64::from(self.scale) - i64::from(divisor.scale));
        if divisor.unscaled.is_zero() {
            return Err(if self.unscaled.is_zero() {
                ArithmeticError::DivisionUndefined
            } else {
                ArithmeticError::DecimalDivisionByZero
            });
        }
        if self.unscaled.is_zero() {
            return Ok(BigDecimal::new(BigInt::zero(), preferred));
        }
        let non_terminating = || ArithmeticError::NonTerminatingExpansion;
        // `self / divisor = (a / b) × 10^(divisor.scale - self.scale)` with `a / b` reduced:
        // exact iff `b` has no prime factor but 2 and 5.
        let gcd = self.unscaled.gcd(&divisor.unscaled);
        let mut numerator = &self.unscaled / &gcd;
        let mut denominator = &divisor.unscaled / &gcd;
        if denominator.is_negative() {
            numerator = -numerator;
            denominator = -denominator;
        }
        let (mut twos, mut fives) = (0u32, 0u32);
        let two = BigInt::from(2u8);
        let five = BigInt::from(5u8);
        while (&denominator % &two).is_zero() {
            denominator /= &two;
            twos += 1;
        }
        while (&denominator % &five).is_zero() {
            denominator /= &five;
            fives += 1;
        }
        if !denominator.is_one() {
            return Err(non_terminating());
        }
        // `numerator / (2^twos × 5^fives) = numerator × 2^(k - twos) × 5^(k - fives) / 10^k`.
        let k = twos.max(fives);
        let unscaled = numerator
            * num_traits::pow(two, (k - twos) as usize)
            * num_traits::pow(five, (k - fives) as usize);
        // A terminating quotient never has more digits than the precision Java divides with
        // (`this.precision() + ceil(10 × divisor.precision() / 3)`).
        let scale = saturate(i64::from(preferred) + i64::from(k));
        let quotient = BigDecimal::new(unscaled, scale).strip_trailing_zeros();
        if quotient.scale < preferred {
            quotient.set_scale(preferred)
        } else {
            Ok(quotient)
        }
    }

    /// `divide(divisor, roundingMode)`: the quotient at the scale of the dividend.
    pub fn divide_rounded(
        &self,
        divisor: &BigDecimal,
        mode: RoundingMode,
    ) -> ArithResult<BigDecimal> {
        // JDK 27 divides the compact (long) values: a zero divisor fails like `long` division.
        if divisor.unscaled.is_zero() {
            return Err(ArithmeticError::DivisionByZero);
        }
        Ok(self.divide_nonzero(divisor, mode))
    }

    /// [`BigDecimal::divide_rounded`] by a non zero divisor.
    pub fn divide_nonzero(&self, divisor: &BigDecimal, mode: RoundingMode) -> BigDecimal {
        // `self.unscaled × 10^(scale - self.scale + divisor.scale) / divisor.unscaled`.
        let shift = i64::from(divisor.scale);
        let (mut numerator, mut denominator) = if shift >= 0 {
            (
                &self.unscaled * ten_pow(shift as u32),
                divisor.unscaled.clone(),
            )
        } else {
            (
                self.unscaled.clone(),
                &divisor.unscaled * ten_pow(shift.unsigned_abs() as u32),
            )
        };
        if denominator.is_negative() {
            numerator = -numerator;
            denominator = -denominator;
        }
        BigDecimal::new(divide_rounded(&numerator, &denominator, mode), self.scale)
    }

    /// `divideToIntegralValue(divisor)`: the integer part of the quotient, its trailing zeros
    /// stripped down to (or padded up to) the preferred scale.
    pub fn divide_to_integral_value(&self, divisor: &BigDecimal) -> ArithResult<BigDecimal> {
        let preferred = saturate(i64::from(self.scale) - i64::from(divisor.scale));
        // A zero dividend with a non zero divisor is smaller in magnitude.
        if self.abs().compare(&divisor.abs()) == Ordering::Less {
            return Ok(BigDecimal::new(BigInt::zero(), preferred));
        }
        if divisor.unscaled.is_zero() {
            return Err(if self.unscaled.is_zero() {
                ArithmeticError::DivisionUndefined
            } else {
                ArithmeticError::DecimalDivisionByZero
            });
        }
        let scale = self.scale.max(divisor.scale);
        let integer = self.unscaled_at(scale) / divisor.unscaled_at(scale);
        let mut unscaled = integer;
        let mut scale = 0i32;
        let ten = BigInt::from(10u8);
        while scale > preferred && !unscaled.is_zero() {
            let (q, r) = unscaled.div_rem(&ten);
            if !r.is_zero() {
                break;
            }
            unscaled = q;
            scale -= 1;
        }
        let quotient = BigDecimal::new(unscaled, scale);
        if quotient.scale < preferred {
            quotient.set_scale(preferred)
        } else {
            Ok(quotient)
        }
    }

    /// `remainder(divisor)`: `this - this.divideToIntegralValue(divisor) × divisor`.
    pub fn remainder(&self, divisor: &BigDecimal) -> ArithResult<BigDecimal> {
        Ok(self.divide_and_remainder(divisor)?.1)
    }

    /// `divideAndRemainder(divisor)`: `divideToIntegralValue` and `remainder`.
    pub fn divide_and_remainder(
        &self,
        divisor: &BigDecimal,
    ) -> ArithResult<(BigDecimal, BigDecimal)> {
        let quotient = self.divide_to_integral_value(divisor)?;
        // `quotient.multiply(divisor)`: the scales add up (and fit, their sum being at most
        // the scale of the dividend).
        let product = BigDecimal::new(
            &quotient.unscaled * &divisor.unscaled,
            saturate(i64::from(quotient.scale) + i64::from(divisor.scale)),
        );
        let remainder = self.subtract(&product);
        Ok((quotient, remainder))
    }

    /// `toString`: plain notation for a non negative scale and an adjusted exponent of at
    /// least -6, scientific notation otherwise.
    fn java_string(&self) -> String {
        let digits = self.unscaled.abs().to_string();
        let sign = if self.unscaled.is_negative() { "-" } else { "" };
        let len = digits.len() as i64;
        let scale = i64::from(self.scale);
        let adjusted = len - 1 - scale;
        if scale == 0 {
            return format!("{sign}{digits}");
        }
        if scale > 0 && adjusted >= -6 {
            return if scale >= len {
                format!("{sign}0.{}{digits}", "0".repeat((scale - len) as usize))
            } else {
                let (int, frac) = digits.split_at((len - scale) as usize);
                format!("{sign}{int}.{frac}")
            };
        }
        let mantissa = if len > 1 {
            format!("{}.{}", &digits[..1], &digits[1..])
        } else {
            digits
        };
        let exponent = if adjusted >= 0 {
            format!("+{adjusted}")
        } else {
            adjusted.to_string()
        };
        format!("{sign}{mantissa}E{exponent}")
    }

    /// `toPlainString`.
    pub fn to_plain_string(&self) -> String {
        let digits = self.unscaled.abs().to_string();
        let sign = if self.unscaled.is_negative() { "-" } else { "" };
        if self.scale <= 0 {
            let zeros = if self.unscaled.is_zero() {
                0
            } else {
                -i64::from(self.scale)
            };
            return format!("{sign}{digits}{}", "0".repeat(zeros as usize));
        }
        let len = digits.len() as i64;
        let scale = i64::from(self.scale);
        if scale >= len {
            format!("{sign}0.{}{digits}", "0".repeat((scale - len) as usize))
        } else {
            let (int, frac) = digits.split_at((len - scale) as usize);
            format!("{sign}{int}.{frac}")
        }
    }
}

impl fmt::Display for BigDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.java_string())
    }
}

/// `saturateLong`.
fn saturate(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// `a / b` (`b > 0`) rounded with `mode`.
fn divide_rounded(a: &BigInt, b: &BigInt, mode: RoundingMode) -> BigInt {
    let (q, r) = a.div_rem(b);
    if r.is_zero() {
        return q;
    }
    let away = if a.is_negative() {
        &q - BigInt::one()
    } else {
        &q + BigInt::one()
    };
    let up = match mode {
        RoundingMode::Ceiling => !a.is_negative(),
        RoundingMode::Floor => a.is_negative(),
        RoundingMode::HalfUp => (r.abs() * 2u8).cmp(b) != Ordering::Less,
    };
    if up { away } else { q }
}

// -----------------------------------------------------------------------------------------
// BigInteger
// -----------------------------------------------------------------------------------------

/// `BigInteger.bitLength`: the bits of the two's complement form, sign bit excluded.
pub fn bit_length(value: &BigInt) -> u64 {
    if value.is_negative() {
        (-value - BigInt::one()).bits()
    } else {
        value.bits()
    }
}

/// `BigInteger.bitCount`: the bits that differ from the sign bit.
pub fn bit_count(value: &BigInt) -> u64 {
    let magnitude = if value.is_negative() {
        -value - BigInt::one()
    } else {
        value.clone()
    };
    magnitude
        .to_u64_digits()
        .1
        .iter()
        .map(|d| u64::from(d.count_ones()))
        .sum()
}

/// `BigInteger.getLowestSetBit` (-1 for zero).
pub fn lowest_set_bit(value: &BigInt) -> i64 {
    value.trailing_zeros().map_or(-1, |n| n as i64)
}

/// `BigInteger.testBit`.
pub fn test_bit(value: &BigInt, n: i32) -> ArithResult<bool> {
    if n < 0 {
        return Err(ArithmeticError::NegativeBitAddress);
    }
    Ok(!((value >> n as usize) & BigInt::one()).is_zero())
}

/// `BigInteger.setBit` / `clearBit` / `flipBit`.
pub fn change_bit(
    value: &BigInt,
    n: i32,
    op: fn(&BigInt, &BigInt) -> BigInt,
) -> ArithResult<BigInt> {
    if n < 0 {
        return Err(ArithmeticError::NegativeBitAddress);
    }
    Ok(op(value, &(BigInt::one() << n as usize)))
}

/// `BigInteger.shiftLeft(n)` (a negative `n` shifts right, rounding down).
pub fn shift_left(value: &BigInt, n: i32) -> BigInt {
    if n >= 0 {
        value << n as usize
    } else {
        value >> n.unsigned_abs() as usize
    }
}

/// `BigInteger.mod(m)`.
pub fn modulo(value: &BigInt, m: &BigInt) -> ArithResult<BigInt> {
    if !m.is_positive() {
        return Err(ArithmeticError::ModulusNotPositive);
    }
    Ok(value.mod_floor(m))
}

/// `BigInteger.modInverse(m)`.
pub fn mod_inverse(value: &BigInt, m: &BigInt) -> ArithResult<BigInt> {
    if !m.is_positive() {
        return Err(ArithmeticError::ModulusNotPositive);
    }
    if m.is_one() {
        return Ok(BigInt::zero());
    }
    let a = value.mod_floor(m);
    let extended = a.extended_gcd(m);
    if !extended.gcd.is_one() {
        return Err(ArithmeticError::NotInvertible);
    }
    Ok(extended.x.mod_floor(m))
}

/// `BigInteger.modPow(exponent, m)` (a negative exponent inverts the base first).
pub fn mod_pow(value: &BigInt, exponent: &BigInt, m: &BigInt) -> ArithResult<BigInt> {
    if !m.is_positive() {
        return Err(ArithmeticError::ModulusNotPositive);
    }
    let base = if exponent.is_negative() {
        mod_inverse(value, m)?
    } else {
        value.mod_floor(m)
    };
    Ok(base.modpow(&exponent.abs(), m))
}

/// Deterministic Miller-Rabin rounds on the first prime bases: exact for every `n` below
/// 3.3 × 10^24, and a probable prime test beyond (Java tests with random bases).
fn miller_rabin(n: &BigInt) -> bool {
    const BASES: [u32; 13] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41];
    let one = BigInt::one();
    let two = BigInt::from(2u8);
    if n < &two {
        return false;
    }
    for &p in &BASES {
        let p = BigInt::from(p);
        if n == &p {
            return true;
        }
        if (n % &p).is_zero() {
            return false;
        }
    }
    let n_minus_one = n - &one;
    let s = n_minus_one.trailing_zeros().unwrap_or_default();
    let d = &n_minus_one >> s as usize;
    'bases: for &a in &BASES {
        let mut x = BigInt::from(a).modpow(&d, n);
        if x == one || x == n_minus_one {
            continue;
        }
        for _ in 1..s {
            x = x.modpow(&two, n);
            if x == n_minus_one {
                continue 'bases;
            }
        }
        return false;
    }
    true
}

/// `BigInteger.isProbablePrime(certainty)`: always `true` for a certainty of at most 0.
pub fn is_probable_prime(value: &BigInt, certainty: i32) -> bool {
    certainty <= 0 || miller_rabin(&value.abs())
}

/// `BigInteger.nextProbablePrime` (`ArithmeticException` for a negative value).
pub fn next_probable_prime(value: &BigInt) -> ArithResult<BigInt> {
    if value.is_negative() {
        return Err(ArithmeticError::NegativeStart(value.clone()));
    }
    let mut candidate = value + BigInt::one();
    if candidate <= BigInt::from(2u8) {
        return Ok(BigInt::from(2u8));
    }
    if candidate.is_even() {
        candidate += BigInt::one();
    }
    while !miller_rabin(&candidate) {
        candidate += BigInt::from(2u8);
    }
    Ok(candidate)
}

/// `BigInteger.toByteArray`: the minimal big-endian two's complement bytes.
pub fn to_byte_array(value: &BigInt) -> Vec<u8> {
    value.to_signed_bytes_be()
}

/// `new BigInteger(byte[])`: big-endian two's complement (`NumberFormatException` for no
/// byte).
pub fn from_byte_array(bytes: &[u8]) -> ParseResult<BigInt> {
    if bytes.is_empty() {
        return Err(NumberFormatError::ZeroLengthBigInteger);
    }
    Ok(BigInt::from_signed_bytes_be(bytes))
}

/// `new BigInteger(text, radix)` for the radixes 2, 10 and 16.
///
/// # Errors
///
/// A text that is empty, has a sign that is not the first character, or has a character that is
/// not a digit of the radix: the error tells the group of digits that `BigInteger` fails on.
pub fn parse_bigint(text: &str, radix: u32) -> ParseResult<BigInt> {
    let minus = text.rfind('-');
    let plus = text.rfind('+');
    if matches!(minus, Some(i) if i != 0)
        || matches!(plus, Some(i) if i != 0)
        || (minus.is_some() && plus.is_some())
    {
        return Err(NumberFormatError::EmbeddedSign);
    }
    let negative = minus.is_some();
    let digits = if minus.is_some() || plus.is_some() {
        &text[1..]
    } else {
        text
    };
    if digits.is_empty() {
        return Err(NumberFormatError::ZeroLengthBigInteger);
    }
    // `num-bigint` accepts `_` separators, Java does not.
    let parsed = if digits.contains('_') {
        None
    } else {
        BigInt::parse_bytes(digits.as_bytes(), radix)
    };
    let magnitude = parsed.ok_or_else(|| NumberFormatError::InvalidRadixInput {
        input: bad_group(digits, radix),
        radix,
    })?;
    Ok(if negative { -magnitude } else { magnitude })
}

/// The group of digits of a text that `new BigInteger(text, radix)` fails on. It skips the leading
/// zeroes and reads the digits by groups that fit an `int` (the first one is the short one), each
/// with `Integer.parseInt`, which tells the group it fails on. The whole text, when no group is bad.
pub(crate) fn bad_group(digits: &str, radix: u32) -> String {
    // The digits of an `int` in the radixes that are read.
    let per_int = match radix {
        2 => 30,
        16 => 7,
        _ => 9,
    };
    let significant: Vec<char> = digits
        .trim_start_matches(|c: char| c.to_digit(radix) == Some(0))
        .chars()
        .collect();
    let first = match significant.len() % per_int {
        0 => per_int,
        short => short,
    };
    let (head, tail) = significant.split_at(first.min(significant.len()));
    std::iter::once(head)
        .chain(tail.chunks(per_int))
        .find(|group| group.iter().any(|c| c.to_digit(radix).is_none()))
        .unwrap_or(&significant)
        .iter()
        .collect()
}
