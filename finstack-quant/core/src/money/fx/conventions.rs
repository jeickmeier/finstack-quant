//! Explicit FX quote metadata and the checked scalar reciprocal (record
//! CORE-CONVENTIONS-011 #1–#4).
//!
//! [`FxPairConvention`] keeps a supplied ordered pair (an outright rate is
//! quote-currency units per one base unit) and its caller-supplied pip size.
//! Nothing is inferred: no market ranking, no USD direct/indirect label, no
//! pip table and no spot lag.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::fx::{FxPairConvention, invert_fx_rate};
//!
//! let conv = FxPairConvention::new(Currency::USD, Currency::JPY, 0.01).expect("valid");
//! assert_eq!(conv.get_base(), Currency::USD);
//! assert_eq!(conv.get_pip_size(), 0.01);
//! assert!((conv.rate_points(50.0).expect("finite") - 0.5).abs() < 1e-12);
//! assert!((invert_fx_rate(1.10).expect("positive rate") - 1.0 / 1.10).abs() < 1e-12);
//! ```

use crate::Result;
use crate::currency::Currency;

/// Validated quote metadata for one ordered FX pair.
///
/// The order is preserved as supplied; the reversed pair is a different
/// quote that needs its own convention (its pip size is not inferred).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxPairConvention {
    base: Currency,
    quote: Currency,
    pip_size: f64,
}

impl FxPairConvention {
    /// Quote metadata for the ordered pair `base`/`quote`.
    ///
    /// # Arguments
    ///
    /// * `base` - Base currency: one unit of it is quoted.
    /// * `quote` - Quote currency: an outright rate is units of it per one
    ///   `base` unit. Must differ from `base`.
    /// * `pip_size` - Outright-rate increment per pip in this pair's quote
    ///   units (for example `0.0001` for EUR/USD, `0.01` for USD/JPY); not an
    ///   ISO minor unit, display precision or venue tick. Finite and strictly
    ///   positive.
    ///
    /// # Errors
    ///
    /// Returns a validation error when the currencies are equal or `pip_size`
    /// is not finite and strictly positive.
    pub fn new(base: Currency, quote: Currency, pip_size: f64) -> Result<Self> {
        if base == quote {
            return Err(crate::Error::Validation(format!(
                "FX quote convention needs two distinct currencies, got {base}/{quote}"
            )));
        }
        if !(pip_size.is_finite() && pip_size > 0.0) {
            return Err(crate::Error::Validation(format!(
                "FX pip size must be finite and positive, got {pip_size}"
            )));
        }
        Ok(Self {
            base,
            quote,
            pip_size,
        })
    }

    /// Base currency of the ordered pair.
    pub fn get_base(&self) -> Currency {
        self.base
    }

    /// Quote currency of the ordered pair.
    pub fn get_quote(&self) -> Currency {
        self.quote
    }

    /// Outright-rate increment per pip, in quote units per base unit.
    pub fn get_pip_size(&self) -> f64 {
        self.pip_size
    }

    /// Rate points for a signed number of forward pips: `pips * pip_size`.
    ///
    /// # Arguments
    ///
    /// * `pips` - Signed forward points in pips; must be finite.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `pips` or the product is not finite.
    pub fn rate_points(&self, pips: f64) -> Result<f64> {
        let points = pips * self.pip_size;
        if !(pips.is_finite() && points.is_finite()) {
            return Err(crate::Error::Validation(format!(
                "FX forward pips must be finite, got {pips}"
            )));
        }
        Ok(points)
    }
}

/// Reciprocal of a strictly positive finite FX rate.
///
/// Non-finite inputs are rejected, zero is rejected, and the reciprocal itself
/// must be a valid FX rate (finite and strictly positive). This pair-agnostic
/// helper reports validation errors without inventing a currency pair.
///
/// # Arguments
///
/// * `rate` - Outright FX rate to invert, in quote-per-base units. Must be
///   finite and strictly positive; the reciprocal must also be a valid rate.
///
/// # Errors
///
/// Returns an error when `rate` is non-finite, non-positive, or when `1 / rate`
/// is not a usable FX rate (overflow to infinity, zero, or a negative value).
///
/// # Returns
///
/// `1 / rate` when that reciprocal is a valid FX rate.
pub fn invert_fx_rate(rate: f64) -> Result<f64> {
    use super::provider::{ReciprocalError, checked_reciprocal};
    checked_reciprocal(rate).map_err(|failure| match failure {
        ReciprocalError::NonFiniteInput(kind) => crate::InputError::NonFiniteValue { kind }.into(),
        ReciprocalError::NonPositiveInput => crate::Error::Validation(format!(
            "Invalid FX rate: expected positive finite rate, got {rate}"
        )),
        ReciprocalError::InvalidReciprocal(reciprocal) => crate::Error::Validation(format!(
            "Invalid FX rate reciprocal: 1 / {rate} = {reciprocal}"
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conventions_keep_the_supplied_order() {
        let eurusd = FxPairConvention::new(Currency::EUR, Currency::USD, 0.0001).expect("valid");
        let usdeur = FxPairConvention::new(Currency::USD, Currency::EUR, 0.0001).expect("valid");
        assert_eq!(
            (eurusd.get_base(), eurusd.get_quote()),
            (Currency::EUR, Currency::USD)
        );
        assert_eq!(
            (usdeur.get_base(), usdeur.get_quote()),
            (Currency::USD, Currency::EUR)
        );
        assert_ne!(eurusd, usdeur);
    }

    #[test]
    fn conventions_reject_same_currency_and_bad_pip_sizes() {
        assert!(FxPairConvention::new(Currency::USD, Currency::USD, 0.0001).is_err());
        for pip in [0.0, -0.0001, f64::NAN, f64::INFINITY] {
            assert!(
                FxPairConvention::new(Currency::EUR, Currency::USD, pip).is_err(),
                "{pip}"
            );
        }
    }

    #[test]
    fn pips_convert_with_the_explicit_size() {
        let eurusd = FxPairConvention::new(Currency::EUR, Currency::USD, 0.0001).expect("valid");
        assert!((eurusd.rate_points(50.0).expect("finite") - 0.005).abs() < 1e-15);
        assert!(eurusd.rate_points(f64::NAN).is_err());
    }

    #[test]
    fn invert_fx_rate_reciprocal() {
        let inverted = invert_fx_rate(1.10).expect("positive rate");
        assert!((inverted - 1.0 / 1.10).abs() < 1e-12);
        assert!((inverted - 0.90909).abs() < 1e-5);
    }

    #[test]
    fn invert_fx_rate_rejects_non_positive() {
        let zero = invert_fx_rate(0.0).expect_err("a zero rate has no reciprocal");
        assert_eq!(zero.kind(), crate::ErrorKind::Validation, "{zero}");
        assert!(invert_fx_rate(-1.10).is_err());
        assert!(invert_fx_rate(f64::NAN).is_err());
        assert!(invert_fx_rate(f64::INFINITY).is_err());
        assert!(matches!(
            invert_fx_rate(1e-320),
            Err(crate::Error::Validation(_))
        ));
        assert!(matches!(
            invert_fx_rate(f64::NAN),
            Err(crate::Error::Input(
                crate::InputError::NonFiniteValue { .. }
            ))
        ));
    }
}
