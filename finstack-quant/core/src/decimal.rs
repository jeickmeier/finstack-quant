//! Decimal conversion utilities.
//!
//! Canonical `f64 ↔ Decimal` helpers that propagate errors on non-finite or
//! unrepresentable values rather than silently collapsing to zero. All
//! production code that converts raw `f64` user input to `Decimal` (or back)
//! should use these helpers; trusted literals in tests and examples can use
//! `Decimal::from_f64_retain(...)` or the `dec!` macro.

use crate::{Error, InputError, NonFiniteKind, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Convert an `f64` to [`Decimal`], returning an error for non-finite values.
///
/// This prevents silent masking of `NaN`/`Infinity` values as zero, which would
/// result in zero rates, strikes, or spreads that materially misprice
/// instruments.
///
/// # Errors
///
/// Returns [`InputError::NonFiniteValue`] for `NaN`, `+inf`, or `-inf`.
/// Returns [`InputError::ConversionOverflow`] when the finite value cannot be
/// represented as `Decimal` (extremely large `f64` magnitude).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::decimal::f64_to_decimal;
///
/// assert!(f64_to_decimal(0.05).is_ok());
/// assert!(f64_to_decimal(f64::NAN).is_err());
/// assert!(f64_to_decimal(f64::INFINITY).is_err());
/// ```
pub fn f64_to_decimal(value: f64) -> Result<Decimal> {
    if value.is_nan() {
        return Err(InputError::NonFiniteValue {
            kind: NonFiniteKind::NaN,
        }
        .into());
    }
    if value.is_infinite() {
        let kind = if value.is_sign_positive() {
            NonFiniteKind::PosInfinity
        } else {
            NonFiniteKind::NegInfinity
        };
        return Err(InputError::NonFiniteValue { kind }.into());
    }
    Decimal::try_from(value).map_err(|_| Error::from(InputError::ConversionOverflow))
}

/// Convert a [`Decimal`] to `f64`, returning an error if conversion fails.
///
/// While `Decimal` values are always finite, conversion to `f64` can fail for
/// very large magnitudes that exceed `f64`'s representable range
/// (~1.8 × 10^308).
///
/// # Errors
///
/// Returns [`InputError::ConversionOverflow`] when the value cannot be
/// represented as `f64`.
pub fn decimal_to_f64(value: Decimal) -> Result<f64> {
    value
        .to_f64()
        .ok_or_else(|| Error::from(InputError::ConversionOverflow))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_to_decimal_accepts_typical_financial_values() {
        let decimal = f64_to_decimal(0.0525).expect("finite rate should convert");
        assert!(decimal > Decimal::ZERO);
    }

    #[test]
    fn f64_to_decimal_rejects_nan() {
        let err = f64_to_decimal(f64::NAN);
        assert!(matches!(
            err,
            Err(Error::Input(InputError::NonFiniteValue {
                kind: NonFiniteKind::NaN
            }))
        ));
    }

    #[test]
    fn f64_to_decimal_rejects_positive_and_negative_infinity() {
        assert!(matches!(
            f64_to_decimal(f64::INFINITY),
            Err(Error::Input(InputError::NonFiniteValue {
                kind: NonFiniteKind::PosInfinity
            }))
        ));
        assert!(matches!(
            f64_to_decimal(f64::NEG_INFINITY),
            Err(Error::Input(InputError::NonFiniteValue {
                kind: NonFiniteKind::NegInfinity
            }))
        ));
    }

    #[test]
    fn f64_to_decimal_rejects_unrepresentable_magnitude() {
        assert!(matches!(
            f64_to_decimal(1e100),
            Err(Error::Input(InputError::ConversionOverflow))
        ));
    }

    #[test]
    fn decimal_to_f64_roundtrips_typical_values() {
        let d = f64_to_decimal(0.0525).expect("convert");
        let f = decimal_to_f64(d).expect("convert back");
        assert!((f - 0.0525).abs() < 1e-12);
    }
}
