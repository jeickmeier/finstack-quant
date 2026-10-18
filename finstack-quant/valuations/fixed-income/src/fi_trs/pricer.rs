//! Fixed Income Index TRS pricing - carry/yield analytics model.
//!
//! This module implements **carry-oriented analytics** for the total return leg of fixed income index TRS
//! using a carry-based model where the index yield drives the expected total return.
//!
//! # Model Choice
//!
//! For fixed income index TRS, two approaches exist:
//!
//! 1. **No-arbitrage forward model**: `F_t = S_0 * e^{(r-y)*t}`, total return equals
//!    the risk-free rate. Mathematically correct but has no yield sensitivity since
//!    the price return and income terms cancel exactly under the multiplicative formula.
//!
//! 2. **Carry model** (used here): Total return per period = `e^{y * dt} - 1`.
//!    This models the index as earning its yield continuously, which is the primary
//!    economic driver of FI index TRS. Rate sensitivity comes from discounting future
//!    payments and is captured separately by the DV01/DurationDv01 metrics.
//!
//! We use the carry model because:
//! - FI index TRS are fundamentally carry products (yield vs financing cost)
//! - Yield sensitivity is essential for what-if analysis and hedge ratio computation
//! - It matches how dealers think about TRS economics: par spread ≈ yield - financing rate
//!
//! This is **not** a full production mark-to-market model for fixed income index TRS.
//! It intentionally omits roll-down and mark-to-market from underlying rate/spread moves.

use super::types::FiIndexTotalReturnSwap;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::common_impl::pricing::{
    PeriodReturnInputs, TotalReturnLegParams, TrsEngine, TrsReturnModel,
};

/// Extracts index yield from market data.
///
/// # Yield Convention
///
/// The scalar must represent a **continuously compounded** annualized yield.
/// If your source provides a bond-equivalent yield (semiannual compounding),
/// convert before populating the market context:
///
/// ```text
/// y_continuous = 2 * ln(1 + y_bey / 2)
/// ```
///
/// Using a semiannual BEY directly will introduce a systematic carry
/// overestimate of ~1-3 bp/quarter for typical IG yields.
///
/// # Errors
///
/// Returns an error if `yield_id` is configured but the corresponding market data
/// is missing. This prevents silent zero-yield assumptions that would materially
/// affect carry calculations.
fn extract_index_yield(trs: &FiIndexTotalReturnSwap, context: &MarketContext) -> Result<f64> {
    match &trs.underlying.yield_id {
        Some(id) => {
            let scalar = context.get_price(id.as_str()).map_err(|_| {
                finstack_quant_core::Error::Validation(format!(
                    "Index yield data '{}' is configured but not found in market context. \
                     Provide the yield scalar or remove yield_id to use zero yield.",
                    id
                ))
            })?;
            Ok(require_unitless_scalar(scalar, "yield", id)?)
        }
        None => Ok(0.0), // No yield configured — intentional zero carry
    }
}

/// Extracts a unitless f64 from a [`MarketScalar`], returning an error if the
/// variant is `Price` (which indicates a likely configuration mistake — yield
/// and duration are dimensionless quantities, not monetary amounts).
fn require_unitless_scalar(scalar: &MarketScalar, kind: &str, id: &str) -> Result<f64> {
    match scalar {
        MarketScalar::Unitless(v) => Ok(*v),
        MarketScalar::Price(_) => Err(finstack_quant_core::Error::Validation(format!(
            "Market scalar '{}' for index {} has type Price, but {} is a unitless quantity. \
             Use MarketScalar::Unitless instead.",
            id, kind, kind
        ))),
    }
}

/// Reads the live index level used to value the period in progress.
///
/// The scalar keyed by `underlying.index_id` is a price level in the index's
/// own units (unitless, or a price in the base currency).
///
/// # Errors
///
/// Returns an error if the level is missing, has the wrong currency, or is
/// not positive and finite.
fn extract_index_level(trs: &FiIndexTotalReturnSwap, context: &MarketContext) -> Result<f64> {
    let id = trs.underlying.index_id.as_str();
    let scalar = context.get_price(id).map_err(|_| {
        finstack_quant_core::Error::Validation(format!(
            "FiIndexTotalReturnSwap '{}': index level '{}' is required to value the \
             period in progress but is not in the market context",
            trs.id.as_str(),
            id
        ))
    })?;
    let level =
        finstack_quant_valuations_core::instruments::common_impl::helpers::scalar_price_amount(
            scalar,
            trs.underlying.currency,
        )?;
    if !level.is_finite() || level <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FiIndexTotalReturnSwap '{}': index level '{}' must be positive and finite, got {}",
            trs.id.as_str(),
            id,
            level
        )));
    }
    Ok(level)
}

/// Fixed income index return model using the carry/yield approach.
///
/// Models the total return per period as:
///
/// ```text
/// Total Return = e^{y * dt} - 1
/// ```
///
/// where `y` is the continuous index yield and `dt` is the accrual period year fraction.
///
/// For the period in progress on the valuation date the return is
/// `S · e^{y·τ} / L − 1`, where `S` is the live index level
/// (`underlying.index_id`), `L` is the reset level (`initial_level`) and `τ`
/// is the schedule year fraction from `as_of` to the period end.
///
/// This is the exponential (multiplicative) form of the income-based return model.
/// For typical quarterly periods and HY yields (5-6%), the difference between
/// `e^{y*dt} - 1` and the linear approximation `y*dt` is about 1bp/quarter.
///
/// # Rate sensitivity
///
/// Rate sensitivity in the total return leg comes from discounting future payments
/// (handled by `TrsEngine`). Direct rate sensitivity of the underlying index is
/// captured by the [`DurationDv01`](super::metrics::DurationDv01Calculator) metric.
struct FiIndexReturnModel<'a> {
    trs: &'a FiIndexTotalReturnSwap,
    index_yield: f64,
}

impl TrsReturnModel for FiIndexReturnModel<'_> {
    fn period_return(&self, inputs: &PeriodReturnInputs, context: &MarketContext) -> Result<f64> {
        let PeriodReturnInputs {
            period_start,
            period_end,
            t_start,
            t_end,
            ..
        } = *inputs;
        let fixing = |date: Date| -> Result<f64> {
            let level = if let Some(id) = &self.trs.fixing_series_id {
                context.get_series(id)?.value_on_exact(date)?
            } else if date == self.trs.schedule.start {
                self.trs.initial_level.ok_or_else(|| {
                    finstack_quant_core::Error::Validation(format!(
                        "FI TRS requires initial_level or a fixing series for {date}"
                    ))
                })?
            } else {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FI TRS requires a fixing series with an exact endpoint level on {date}"
                )));
            };
            if !level.is_finite() || level <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "FI TRS endpoint level on {date} must be positive and finite"
                )));
            }
            Ok(level)
        };
        if period_end <= inputs.as_of {
            return Ok(fixing(period_end)? / fixing(period_start)? - 1.0);
        }
        if t_start < 0.0 {
            let reset_level = fixing(period_start)?;
            let index_level = extract_index_level(self.trs, context)?;
            return Ok(index_level * (self.index_yield * t_end).exp() / reset_level - 1.0);
        }
        // Year fraction computed using the schedule's day count convention
        // (same convention used for the financing leg accrual).
        let ctx = finstack_quant_core::dates::DayCountContext::default();
        let dt = self
            .trs
            .schedule
            .params
            .day_count
            .year_fraction(period_start, period_end, ctx)?;

        // Multiplicative income return: e^{y * dt} - 1
        // This avoids the ~1bp/quarter linearization error of `y * dt`.
        Ok((self.index_yield * dt).exp() - 1.0)
    }
}

/// Calculates the present value of the total return leg for a fixed income index TRS.
///
/// Uses a carry/yield model where the expected total return per period is `e^{y * dt} - 1`.
/// Each period's return is discounted back to the valuation date using the discount curve.
///
/// # Arguments
/// * `trs` — The fixed income index TRS instrument
/// * `context` — Market context containing curves and market data
/// * `as_of` — Valuation date
///
/// # Returns
/// Present value of the total return leg in the instrument's currency.
///
/// # Errors
///
/// Returns an error if:
/// - The TRS has already matured (`end <= as_of`)
/// - The yield_id is configured but missing from the market context
/// - A period is in progress and `initial_level` or the index level is missing
/// - The discount curve is not found
pub(crate) fn pv_total_return_leg(
    trs: &FiIndexTotalReturnSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    // Warn once per process, not on every valuation call (batch pricing and
    // FD sensitivities call this hundreds of times).
    static CARRY_MODEL_WARNING: std::sync::Once = std::sync::Once::new();
    CARRY_MODEL_WARNING.call_once(|| {
        tracing::warn!(
            "FiIndexTotalReturnSwap total-return leg uses a carry-only analytic model; \
             it is not a full mark-to-market index return model"
        );
    });
    let index_yield = extract_index_yield(trs, context)?;

    let params = TotalReturnLegParams {
        schedule: &trs.schedule,
        notional: trs.notional,
        discount_curve_id: trs.financing_leg.discount_curve_id.as_str(),
        // The notional alone sizes both legs of an FI index TRS.
        initial_level: trs.initial_level,
    };

    let model = FiIndexReturnModel { trs, index_yield };
    TrsEngine::pv_total_return_leg_with_model(params, context, as_of, &model)
}

/// Project both legs using the same period calculations as present value.
pub(crate) fn net_cashflows(
    trs: &FiIndexTotalReturnSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Vec<(Date, Money)>> {
    trs.validate()?;
    let model = FiIndexReturnModel {
        trs,
        index_yield: extract_index_yield(trs, context)?,
    };
    let params = TotalReturnLegParams {
        schedule: &trs.schedule,
        notional: trs.notional,
        discount_curve_id: trs.financing_leg.discount_curve_id.as_str(),
        initial_level: trs.initial_level,
    };
    let mut flows = std::collections::BTreeMap::<Date, f64>::new();
    let sign = if trs.side == finstack_quant_valuations_core::instruments::common_impl::parameters::legs::PayReceive::Receive
    {
        1.0
    } else {
        -1.0
    };
    for (date, amount) in
        TrsEngine::cashflows_total_return_leg_with_model(params, context, as_of, &model)?
    {
        *flows.entry(date).or_default() += sign * amount.amount();
    }
    for (date, amount) in TrsEngine::cashflows_financing_leg(
        &trs.financing_leg,
        &trs.schedule,
        trs.notional,
        context,
        as_of,
    )? {
        *flows.entry(date).or_default() -= sign * amount.amount();
    }
    flows
        .into_iter()
        .map(|(date, amount)| Ok((date, Money::new(amount, trs.notional.currency())?)))
        .collect()
}
