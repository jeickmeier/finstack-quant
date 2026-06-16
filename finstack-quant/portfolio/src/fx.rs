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
//! Callers that need specialized behaviour (e.g. the implied-rate optimization
//! used by metrics aggregation, or the far-future warning emitted by cashflow
//! collapsing) should wrap this helper rather than re-implementing the matrix
//! lookup.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::fx::FxQuery;
use finstack_quant_core::money::Money;

use crate::error::{Error, Result};

/// Convert a monetary amount into `base_ccy` using the market FX matrix.
///
/// Returns the input unchanged when it is already denominated in `base_ccy`.
///
/// # Arguments
///
/// * `amount` - Monetary amount to convert.
/// * `as_of` - Date used for the FX rate lookup.
/// * `market` - Market context supplying the FX matrix.
/// * `base_ccy` - Target reporting currency.
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
    base_ccy: Currency,
) -> Result<Money> {
    if amount.currency() == base_ccy {
        return Ok(amount);
    }

    let fx_matrix = market
        .fx()
        .ok_or_else(|| Error::MissingMarketData("FX matrix not available".to_string()))?;

    let query = FxQuery::new(amount.currency(), base_ccy, as_of);
    let rate_result = fx_matrix
        .rate(query)
        .map_err(|_| Error::FxConversionFailed {
            from: amount.currency(),
            to: base_ccy,
        })?;

    Ok(Money::new(amount.amount() * rate_result.rate, base_ccy))
}
