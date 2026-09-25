//! Market FX pair conventions: CCY1 order, USD direct/indirect, pip size, spot lag.
//!
//! These helpers make the Bloomberg/Reuters quote hierarchy **queryable**. They
//! do not change [`FxMatrix`](super::FxMatrix) storage orientation and do not
//! reject inverted `FxSpot` / `FxForward` construction. Distinct from NDF
//! quote conventions in `finstack-quant-valuations`.
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::money::fx::{
//!     fx_market_pair, fx_pair_convention, fx_pip_size, invert_fx_rate, FxQuoteConvention,
//! };
//!
//! assert_eq!(
//!     fx_market_pair(Currency::USD, Currency::EUR),
//!     (Currency::EUR, Currency::USD)
//! );
//! let conv = fx_pair_convention(Currency::USD, Currency::JPY);
//! assert_eq!(conv.base, Currency::USD);
//! assert_eq!(conv.usd_quotation, FxQuoteConvention::Indirect);
//! assert_eq!(fx_pip_size(Currency::USD, Currency::JPY), 0.01);
//! assert!((invert_fx_rate(1.10).expect("positive rate") - 1.0 / 1.10).abs() < 1e-12);
//! ```

use std::cmp::Ordering;

use crate::currency::Currency;
use crate::dates::fx::fx_standard_settlement_days;
use crate::Result;

use super::reciprocal_rate_or_err;

/// USD quotation style for a market FX pair.
///
/// **Direct** means USD is the quote currency (EURUSD, GBPUSD). **Indirect**
/// means USD is the base (USDJPY, USDCAD). Non-USD crosses inherit the USD
/// quotation of the market CCY1 versus USD (EURGBP is Direct because EURUSD
/// is Direct).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FxQuoteConvention {
    /// USD is the quote currency (units of USD per one unit of CCY1).
    Direct,
    /// USD is the base currency (units of CCY2 per one USD).
    Indirect,
}

impl std::fmt::Display for FxQuoteConvention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => write!(f, "direct"),
            Self::Indirect => write!(f, "indirect"),
        }
    }
}

impl std::str::FromStr for FxQuoteConvention {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "direct" => Ok(Self::Direct),
            "indirect" => Ok(Self::Indirect),
            _ => Err(crate::error::InputError::Invalid.into()),
        }
    }
}

/// Market convention for one FX pair after Bloomberg/Reuters CCY1 ordering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxPairConvention {
    /// Market CCY1 (base): the currency that is one unit in the screen pair.
    pub base: Currency,
    /// Market CCY2 (quote): units of this currency per one unit of [`Self::base`].
    pub quote: Currency,
    /// Direct if the USD leg of this market pair quotes USD as CCY2; Indirect
    /// if USD is CCY1. Crosses inherit CCY1's USD-leg convention.
    pub usd_quotation: FxQuoteConvention,
    /// Pip size in rate units: `0.01` for JPY, KRW, and HUF pairs, else `0.0001`.
    pub pip_size: f64,
    /// Standard T+N settlement (spot) lag in business days (T+1 or T+2) for this pair.
    pub settlement_days: u32,
}

/// Bloomberg/Reuters CCY1 rank. Lower is stronger (more likely to be CCY1).
fn market_ccy1_rank(ccy: Currency) -> u8 {
    match ccy {
        Currency::EUR => 0,
        Currency::GBP => 1,
        Currency::AUD => 2,
        Currency::NZD => 3,
        Currency::USD => 4,
        _ => 5,
    }
}

/// USD quotation of a pair that is already in market CCY1/CCY2 order.
fn usd_quotation_for_market_pair(base: Currency, quote: Currency) -> FxQuoteConvention {
    if base == Currency::USD {
        FxQuoteConvention::Indirect
    } else if quote == Currency::USD {
        FxQuoteConvention::Direct
    } else {
        let (usd_base, usd_quote) = fx_market_pair(base, Currency::USD);
        usd_quotation_for_market_pair(usd_base, usd_quote)
    }
}

/// Order two currencies into the market CCY1/CCY2 pair.
///
/// Priority is EUR > GBP > AUD > NZD > USD > other, with a stable ISO-4217
/// alphabetic tie-break when both sides share the same rank.
///
/// # Arguments
///
/// * `a` - First currency of the unordered pair. Need not be market CCY1.
/// * `b` - Second currency of the unordered pair. Need not be market CCY2.
///
/// # Returns
///
/// `(CCY1, CCY2)` in market order. `fx_market_pair(USD, EUR)` is `(EUR, USD)`.
pub fn fx_market_pair(a: Currency, b: Currency) -> (Currency, Currency) {
    match market_ccy1_rank(a).cmp(&market_ccy1_rank(b)) {
        Ordering::Less => (a, b),
        Ordering::Greater => (b, a),
        Ordering::Equal => {
            if a.as_ref() <= b.as_ref() {
                (a, b)
            } else {
                (b, a)
            }
        }
    }
}

/// Market convention for an unordered currency pair.
///
/// Returned [`FxPairConvention::base`] / [`FxPairConvention::quote`] are always
/// the market CCY1/CCY2, even when the arguments are inverted.
///
/// # Arguments
///
/// * `base` - One currency of the pair. Orientation is ignored; the helper
///   reorders into market CCY1/CCY2.
/// * `quote` - The other currency of the pair. Orientation is ignored.
///
/// # Returns
///
/// Market CCY1/CCY2, USD quotation, pip size, and standard spot lag.
pub fn fx_pair_convention(base: Currency, quote: Currency) -> FxPairConvention {
    let (ccy1, ccy2) = fx_market_pair(base, quote);
    FxPairConvention {
        base: ccy1,
        quote: ccy2,
        usd_quotation: usd_quotation_for_market_pair(ccy1, ccy2),
        pip_size: fx_pip_size(ccy1, ccy2),
        settlement_days: fx_standard_settlement_days(ccy1, ccy2),
    }
}

/// Pip size in outright-rate units for a currency pair.
///
/// Returns `0.01` when either side is JPY, KRW, or HUF (0-decimal FX pairs);
/// otherwise `0.0001` (the standard G10 pip). Argument order does not matter.
///
/// # Arguments
///
/// * `base` - One currency of the pair. Order is not significant.
/// * `quote` - The other currency of the pair. Order is not significant.
///
/// # Returns
///
/// Pip size as a decimal increment of the outright FX rate (`0.01` or `0.0001`).
pub fn fx_pip_size(base: Currency, quote: Currency) -> f64 {
    if matches!(base, Currency::JPY | Currency::KRW | Currency::HUF)
        || matches!(quote, Currency::JPY | Currency::KRW | Currency::HUF)
    {
        0.01
    } else {
        0.0001
    }
}

/// Reciprocal of a strictly positive finite FX rate.
///
/// Non-finite inputs are rejected, zero is rejected, and the reciprocal itself
/// must be a valid FX rate (finite and strictly positive). The pair stamped on
/// currency-bearing errors is USD/USD because this helper is pair-agnostic.
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
    reciprocal_rate_or_err(rate, Currency::USD, Currency::USD)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx_market_pair_usd_eur_is_eurusd() {
        assert_eq!(
            fx_market_pair(Currency::USD, Currency::EUR),
            (Currency::EUR, Currency::USD)
        );
        assert_eq!(
            fx_market_pair(Currency::EUR, Currency::USD),
            (Currency::EUR, Currency::USD)
        );
    }

    #[test]
    fn fx_pair_convention_eurusd_is_direct() {
        let conv = fx_pair_convention(Currency::EUR, Currency::USD);
        assert_eq!(conv.base, Currency::EUR);
        assert_eq!(conv.quote, Currency::USD);
        assert_eq!(conv.usd_quotation, FxQuoteConvention::Direct);
        assert_eq!(conv.pip_size, 0.0001);
        assert_eq!(conv.settlement_days, 2);
        let inverted = fx_pair_convention(Currency::USD, Currency::EUR);
        assert_eq!(inverted, conv);
    }

    #[test]
    fn fx_pair_convention_usdjpy_is_indirect() {
        let conv = fx_pair_convention(Currency::USD, Currency::JPY);
        assert_eq!(conv.base, Currency::USD);
        assert_eq!(conv.quote, Currency::JPY);
        assert_eq!(conv.usd_quotation, FxQuoteConvention::Indirect);
        assert_eq!(conv.pip_size, 0.01);
        assert_eq!(conv.settlement_days, 2);
    }

    #[test]
    fn fx_pair_convention_usdcad_is_t1() {
        let conv = fx_pair_convention(Currency::USD, Currency::CAD);
        assert_eq!(conv.usd_quotation, FxQuoteConvention::Indirect);
        assert_eq!(conv.settlement_days, 1);
        assert_eq!(conv.pip_size, 0.0001);
    }

    #[test]
    fn fx_pair_convention_eurgbp_inherits_direct_usd_leg() {
        let conv = fx_pair_convention(Currency::EUR, Currency::GBP);
        assert_eq!(conv.base, Currency::EUR);
        assert_eq!(conv.quote, Currency::GBP);
        assert_eq!(conv.usd_quotation, FxQuoteConvention::Direct);
        assert_eq!(conv.pip_size, 0.0001);
    }

    #[test]
    fn fx_market_pair_iso_tie_break() {
        assert_eq!(
            fx_market_pair(Currency::CAD, Currency::JPY),
            (Currency::CAD, Currency::JPY)
        );
        assert_eq!(
            fx_pair_convention(Currency::CAD, Currency::JPY).usd_quotation,
            FxQuoteConvention::Indirect
        );
    }

    #[test]
    fn fx_pip_size_jpy_krw_huf() {
        assert_eq!(fx_pip_size(Currency::USD, Currency::JPY), 0.01);
        assert_eq!(fx_pip_size(Currency::JPY, Currency::USD), 0.01);
        assert_eq!(fx_pip_size(Currency::USD, Currency::KRW), 0.01);
        assert_eq!(fx_pip_size(Currency::EUR, Currency::HUF), 0.01);
        assert_eq!(fx_pip_size(Currency::EUR, Currency::USD), 0.0001);
        assert_eq!(fx_pip_size(Currency::USD, Currency::CAD), 0.0001);
    }

    #[test]
    fn invert_fx_rate_reciprocal() {
        let inverted = invert_fx_rate(1.10).expect("positive rate");
        assert!((inverted - 1.0 / 1.10).abs() < 1e-12);
        assert!((inverted - 0.90909).abs() < 1e-5);
    }

    #[test]
    fn invert_fx_rate_rejects_non_positive() {
        assert!(invert_fx_rate(0.0).is_err());
        assert!(invert_fx_rate(-1.10).is_err());
        assert!(invert_fx_rate(f64::NAN).is_err());
        assert!(invert_fx_rate(f64::INFINITY).is_err());
    }

    #[test]
    fn fx_quote_convention_fromstr_display_roundtrip() {
        assert_eq!(
            "direct"
                .parse::<FxQuoteConvention>()
                .expect("direct parses"),
            FxQuoteConvention::Direct
        );
        assert_eq!(
            "indirect"
                .parse::<FxQuoteConvention>()
                .expect("indirect parses"),
            FxQuoteConvention::Indirect
        );
        assert_eq!(FxQuoteConvention::Direct.to_string(), "direct");
        assert!("Direct".parse::<FxQuoteConvention>().is_err());
    }
}
