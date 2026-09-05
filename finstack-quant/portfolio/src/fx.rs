//! Portfolio-level FX conversion helpers.
//!
//! This module centralizes the "convert a `Money` amount into the portfolio
//! base currency using the FX matrix from a `MarketContext`" pattern that is
//! otherwise duplicated across valuation, metrics, attribution, margin, and
//! cashflow aggregation. Callers that need the same behaviour should call
//! [`convert_to_base`] rather than re-implementing the FxMatrix lookup and
//! error mapping.
//!
//! The implementation intentionally stays narrow:
//!
//! - Same-currency inputs short-circuit without consulting the FX matrix.
//! - Missing FX matrices surface as [`Error::MissingMarketData`].
//! - Missing rates for a specific pair surface as [`Error::FxConversionFailed`].
//!
//! [`convert_to_base`] is the spot path (NAV, metrics, attribution, factor
//! endpoints). [`convert_to_base_forward`] wraps that spot lookup and applies
//! covered-interest-parity forwards for cashflows with `payment_date > as_of`.
//! Do not re-implement matrix lookup in those wrappers.

use std::collections::HashMap;

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::CurveId;

use crate::error::{Error, Result};

/// Discount factors at or below this magnitude are treated as missing/zero.
const DF_NEAR_ZERO: f64 = 1e-14;

/// Convert a monetary amount into `base_currency` using the market FX matrix.
///
/// Returns the input unchanged when it is already denominated in `base_currency`.
///
/// # Arguments
///
/// * `amount` - Monetary amount to convert.
/// * `as_of` - Date used for the FX rate lookup.
/// * `market` - Market context supplying the FX matrix.
/// * `base_currency` - Target reporting currency.
///
/// # Errors
///
/// * [`Error::MissingMarketData`] - The market context has no FX matrix.
/// * [`Error::FxConversionFailed`] - The requested currency pair is not
///   available in the FX matrix.
pub fn convert_to_base(
    amount: Money,
    as_of: Date,
    market: &MarketContext,
    base_currency: Currency,
) -> Result<Money> {
    let rate = spot_rate_to_base(amount.currency(), as_of, market, base_currency)?;
    Ok(Money::new(amount.amount() * rate, base_currency)?)
}

/// Resolve the spot FX factor from `from_currency` into `base_currency`.
///
/// Returns `1.0` for a same-currency conversion without consulting the market.
///
/// # Arguments
///
/// * `from_currency` - Native currency whose units the factor converts.
/// * `as_of` - Date used for the FX matrix lookup.
/// * `market` - Market context supplying the FX matrix.
/// * `base_currency` - Target reporting currency.
///
/// # Errors
///
/// * [`Error::MissingMarketData`] - The market context has no FX matrix.
/// * [`Error::FxConversionFailed`] - The requested currency pair is not
///   available in the FX matrix.
pub fn spot_rate_to_base(
    from_currency: Currency,
    as_of: Date,
    market: &MarketContext,
    base_currency: Currency,
) -> Result<f64> {
    if from_currency == base_currency {
        return Ok(1.0);
    }

    let fx_matrix = market
        .fx()
        .ok_or_else(|| Error::MissingMarketData("FX matrix not available".to_string()))?;
    fx_matrix
        .rate(FxQuery::new(from_currency, base_currency, as_of))
        .map(|result| result.rate)
        .map_err(|_| Error::FxConversionFailed {
            from: from_currency,
            to: base_currency,
        })
}

/// Convert a dated cashflow into `base_currency` using spot or a CIP forward.
///
/// Same-currency amounts are returned unchanged. Payments on or before `as_of`
/// use [`convert_to_base`] at `as_of` (spot). Later payments use the covered
/// interest-rate parity forward
/// `F(T) = S × DF_from(T) / DF_base(T)`, matching
/// [`finstack_quant_valuations::instruments::fx::fx_forward::FxForward::market_forward_rate`].
/// `from` is the cashflow currency.
///
/// Discount curves are resolved from `discount_curves` when that map contains
/// the currency; otherwise from `market.get_discount(currency.to_string())`.
/// Missing curves or missing/zero discount factors fail closed.
///
/// # Arguments
///
/// * `amount` - Cashflow amount in its native currency.
/// * `as_of` - Valuation date: spot FX is observed here, and discount factors
///   are measured from this date to `payment_date`.
/// * `payment_date` - Cashflow payment date. On or before `as_of` selects
///   spot; after `as_of` selects the CIP forward.
/// * `market` - Market context supplying the FX matrix and discount curves.
/// * `base_currency` - Target reporting currency (`DF_base` in the CIP formula).
/// * `discount_curves` - Optional `Currency → CurveId` overrides. A missing
///   map or missing currency entry falls back to the ISO currency code as
///   the discount-curve identifier. There is no silent fallback to another
///   curve in the same currency (for example `USD-OIS`).
///
/// # Errors
///
/// * [`Error::MissingMarketData`] - FX matrix, discount curve, or a usable
///   discount factor is missing.
/// * [`Error::FxConversionFailed`] - The spot pair is not in the FX matrix.
/// * [`Error::Core`] - Discount-factor evaluation failed for a resolved curve.
pub fn convert_to_base_forward(
    amount: Money,
    as_of: Date,
    payment_date: Date,
    market: &MarketContext,
    base_currency: Currency,
    discount_curves: Option<&HashMap<Currency, CurveId>>,
) -> Result<Money> {
    if amount.currency() == base_currency {
        return Ok(amount);
    }

    if payment_date <= as_of {
        return convert_to_base(amount, as_of, market, base_currency);
    }

    let spot_converted = convert_to_base(amount, as_of, market, base_currency)?;
    let df_from = discount_factor(
        market,
        amount.currency(),
        as_of,
        payment_date,
        discount_curves,
    )?;
    let df_base = discount_factor(market, base_currency, as_of, payment_date, discount_curves)?;

    Ok(Money::new(
        spot_converted.amount() * (df_from / df_base),
        base_currency,
    )?)
}

fn discount_factor(
    market: &MarketContext,
    currency: Currency,
    as_of: Date,
    payment_date: Date,
    discount_curves: Option<&HashMap<Currency, CurveId>>,
) -> Result<f64> {
    let mapped = discount_curves.and_then(|map| map.get(&currency));
    let id = mapped.map_or(currency.as_ref(), CurveId::as_str);
    let curve = market.get_discount(id).map_err(|_| {
        Error::MissingMarketData(format!(
            "no discount curve for {currency} (looked up as '{id}')"
        ))
    })?;
    let df = curve
        .df_between_dates(as_of, payment_date)
        .map_err(Error::Core)?;
    if !df.is_finite() || df.abs() < DF_NEAR_ZERO {
        return Err(Error::MissingMarketData(format!(
            "discount factor for {currency} from {as_of} to {payment_date} is missing or zero (df={df})"
        )));
    }
    Ok(df)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use std::sync::Arc;
    use time::macros::date;

    fn flat_discount(id: &str, as_of: Date, df_1y: f64) -> DiscountCurve {
        DiscountCurve::builder(id)
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, df_1y)])
            .interp(InterpStyle::Linear)
            .build()
            .expect("test discount curve should build")
    }

    #[test]
    fn spot_rate_to_base_resolves_direct_and_inverse_pairs() {
        let as_of = date!(2025 - 01 - 01);
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.25)
            .expect("test FX quote should be valid");
        let market = MarketContext::new().insert_fx(FxMatrix::new(provider));

        let direct = spot_rate_to_base(Currency::EUR, as_of, &market, Currency::USD)
            .expect("direct quote should resolve");
        let inverse = spot_rate_to_base(Currency::USD, as_of, &market, Currency::EUR)
            .expect("inverse quote should resolve");

        assert!((direct - 1.25).abs() < 1e-12);
        assert!((inverse - 0.8).abs() < 1e-12);
    }

    #[test]
    fn spot_rate_to_base_preserves_missing_market_and_pair_errors() {
        let as_of = date!(2025 - 01 - 01);
        let missing_matrix =
            spot_rate_to_base(Currency::EUR, as_of, &MarketContext::new(), Currency::USD)
                .expect_err("missing matrix should fail");
        assert!(matches!(missing_matrix, Error::MissingMarketData(_)));

        let empty_market =
            MarketContext::new().insert_fx(FxMatrix::new(Arc::new(SimpleFxProvider::new())));
        let missing_pair = spot_rate_to_base(Currency::EUR, as_of, &empty_market, Currency::USD)
            .expect_err("missing pair should fail");
        assert!(matches!(
            missing_pair,
            Error::FxConversionFailed {
                from: Currency::EUR,
                to: Currency::USD
            }
        ));
    }

    #[test]
    fn forward_fx_uses_iso_code_without_curve_map() {
        let as_of = date!(2025 - 01 - 01);
        let payment = date!(2026 - 01 - 01);
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::JPY, Currency::USD, 0.0067)
            .expect("test FX quote should be valid");
        let market = MarketContext::new()
            .insert(flat_discount("JPY", as_of, 0.99))
            .insert(flat_discount("USD", as_of, 0.95))
            .insert_fx(FxMatrix::new(provider));

        let converted = convert_to_base_forward(
            Money::from((100_i64, Currency::JPY)),
            as_of,
            payment,
            &market,
            Currency::USD,
            None,
        )
        .expect("ISO curve ids should resolve without a map");

        let expected = 100.0 * 0.0067 * 0.99 / 0.95;
        assert!((converted.amount() - expected).abs() < 1e-12);
        assert_eq!(converted.currency(), Currency::USD);
    }

    #[test]
    fn forward_fx_uses_explicit_curve_id_without_allocating_iso_fallback() {
        let as_of = date!(2025 - 01 - 01);
        let payment = date!(2026 - 01 - 01);
        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::JPY, Currency::USD, 0.0067)
            .expect("test FX quote should be valid");
        let market = MarketContext::new()
            .insert(flat_discount("JPY-OIS", as_of, 0.99))
            .insert(flat_discount("USD-OIS", as_of, 0.95))
            .insert_fx(FxMatrix::new(provider));
        let mut curves = HashMap::new();
        curves.insert(Currency::JPY, CurveId::new("JPY-OIS"));
        curves.insert(Currency::USD, CurveId::new("USD-OIS"));

        let converted = convert_to_base_forward(
            Money::from((100_i64, Currency::JPY)),
            as_of,
            payment,
            &market,
            Currency::USD,
            Some(&curves),
        )
        .expect("explicit curve map should resolve JPY-OIS");

        let expected = 100.0 * 0.0067 * 0.99 / 0.95;
        assert!((converted.amount() - expected).abs() < 1e-12);
    }
}
