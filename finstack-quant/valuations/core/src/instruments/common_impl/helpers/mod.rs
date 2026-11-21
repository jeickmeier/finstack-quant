//! Utilities for instrument pricing and metrics assembly.
//!
//! Contains helpers shared across instrument implementations, notably metric
//! context construction, lifecycle validation, Monte Carlo path resolution, and
//! inflation reference resolution.

mod inflation;
mod lifecycle;
mod mc;

#[cfg(test)]
mod tests;

pub use inflation::{InflationReferenceValue, apply_inflation_lag, resolve_reference_inflation};
#[cfg(test)]
pub(crate) use lifecycle::build_with_metrics_dyn;
pub(crate) use lifecycle::{MetricBuildOptions, compute_metrics_dyn};
pub use lifecycle::{
    ValidatedPricingLifecycle, apply_scenario_raw_value, apply_scenario_value,
    attach_mc_diagnostics,
};
pub use mc::{merged_path_config, resolve_mc_paths};

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::{context::MarketContext, scalars::MarketScalar};
use finstack_quant_core::money::Money;

/// Read a scalar price amount without discarding an attached currency.
///
/// # Arguments
///
/// * `scalar` - Market scalar quote read from `MarketContext::get_price`.
/// * `expected_currency` - Settlement or instrument currency required when
///   `scalar` is currency-denominated.
pub fn scalar_price_amount(
    scalar: &MarketScalar,
    expected_currency: Currency,
) -> finstack_quant_core::Result<f64> {
    if let Some(value) = scalar.get_unitless() {
        return Ok(value);
    }
    if let Some(price) = scalar.get_price() {
        if price.currency() == expected_currency {
            return Ok(price.amount());
        }
        return Err(finstack_quant_core::Error::CurrencyMismatch {
            expected: expected_currency,
            actual: price.currency(),
        });
    }
    Err(finstack_quant_core::Error::Validation(
        "Unsupported market scalar variant".into(),
    ))
}

/// Convert a discount factor to an effective continuously-compounded zero rate.
///
/// Returns `r` such that `exp(-r * t) = df`. Returns `Ok(0.0)` at expiry
/// (`t <= 0`), which is the correct mathematical limit and matches the
/// behaviour required by callers that short-circuit on `t <= 0` before using
/// the returned rate.
///
/// # Arguments
///
/// * `df` - Finite positive unitless discount factor.
/// * `t` - Finite year fraction on the discount curve's clock.
/// * `context` - Pricing operation reported in input validation errors.
///
/// # Errors
///
/// Returns a `Validation` error for a non-finite time or a discount factor
/// that is non-finite or non-positive, including at expiry.
#[inline]
pub fn zero_rate_from_df(df: f64, t: f64, context: &str) -> finstack_quant_core::Result<f64> {
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: discount factor must be finite and > 0, got {df:.6e} for t={t:.6}"
        )));
    }
    if !t.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{context}: discount time must be finite, got {t}"
        )));
    }
    if t <= 0.0 {
        return Ok(0.0);
    }
    Ok(-df.ln() / t)
}

/// Schedule -> PV helper that uses the curve's own day count convention.
///
/// This variant ensures consistency between:
/// - Metric calculations (e.g., par rate using `df_on_date`)
/// - NPV calculations
///
/// Routes through `core::cashflow::npv` (record CORE-CONVENTIONS-016):
/// flows before `as_of` are excluded and a flow dated `as_of` counts at
/// factor 1 exactly when `include_today` is true.
/// An empty remaining schedule returns zero in the instrument's notional
/// currency; an instrument without a defined notional still returns an error.
///
/// # Arguments
///
/// * `instrument` - The instrument providing cashflows
/// * `market` - Market data context
/// * `as_of` - Valuation date
/// * `discount_curve_id` - ID of the discount curve to use
/// * `include_today` - Whether cash dated `as_of` is still owed; chosen by the
///   product, with no default.
pub fn schedule_pv<S>(
    instrument: &S,
    market: &MarketContext,
    as_of: Date,
    discount_curve_id: &finstack_quant_core::types::CurveId,
    include_today: bool,
) -> finstack_quant_core::Result<Money>
where
    S: finstack_quant_cashflows::CashflowProvider,
{
    use finstack_quant_core::cashflow::npv;

    let flows = S::dated_cashflows(instrument, market, as_of)?;
    if flows.is_empty() {
        let notional = instrument
            .notional()?
            .ok_or(finstack_quant_core::InputError::TooFewPoints)?;
        return Ok(Money::from((0_i64, notional.currency())));
    }
    let disc = market.get_discount(discount_curve_id.as_str())?;
    // Discount on the curve's own time axis, consistent with curve metrics.
    npv(disc.as_ref(), as_of, &flows, include_today)
}

/// Resolve an optional dividend-yield scalar from the market context.
///
/// Returns `0.0` only when no dividend yield ID is configured. If an ID is
/// configured, missing or wrongly-typed market data is treated as a validation
/// error rather than silently assuming zero carry.
///
/// # Arguments
///
/// * `market` - Market data context queried when `div_yield_id` is `Some`.
/// * `div_yield_id` - Optional identifier of the unitless dividend-yield scalar.
pub fn resolve_optional_dividend_yield(
    market: &MarketContext,
    div_yield_id: Option<&finstack_quant_core::types::PriceId>,
) -> finstack_quant_core::Result<f64> {
    let Some(div_id) = div_yield_id else {
        return Ok(0.0);
    };

    let scalar = market.get_price(div_id.as_str()).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "Failed to fetch dividend yield '{}': {}",
            div_id, e
        ))
    })?;

    if let Some(v) = scalar.get_unitless() {
        Ok(v)
    } else if let Some(m) = scalar.get_price() {
        Err(finstack_quant_core::Error::Validation(format!(
            "Dividend yield '{}' should be a unitless scalar, got Price({})",
            div_id,
            m.currency()
        )))
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "Dividend yield '{}' should be a unitless scalar",
            div_id
        )))
    }
}

/// Read a required unitless market scalar, erroring when it is missing or typed.
///
/// The `model` argument is purely diagnostic and appears in the error
/// message (e.g. `"Heston"`, `"rough Bergomi"`).
///
/// # Arguments
///
/// * `market` - Market context queried for `key`.
/// * `key` - Identifier of the required unitless market scalar.
/// * `model` - Model name surfaced in missing-input and type-mismatch diagnostics.
pub fn get_unitless_scalar_strict(
    market: &MarketContext,
    key: &str,
    model: &str,
) -> finstack_quant_core::Result<f64> {
    match market.get_price(key) {
        Ok(scalar) => scalar.get_unitless().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "{model} parameter '{key}' must be a unitless market scalar, got {scalar:?}"
            ))
        }),
        Err(_) => Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::NotFound {
                id: format!("{key} (required by {model})"),
            },
        )),
    }
}
