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

use super::types::FIIndexTotalReturnSwap;
use crate::instruments::common_impl::pricing::{
    PeriodReturnInputs, TotalReturnLegParams, TrsEngine, TrsReturnModel,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

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
fn extract_index_yield(trs: &FIIndexTotalReturnSwap, context: &MarketContext) -> Result<f64> {
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
    trs: &'a FIIndexTotalReturnSwap,
    index_yield: f64,
}

impl TrsReturnModel for FiIndexReturnModel<'_> {
    fn period_return(&self, inputs: &PeriodReturnInputs, _context: &MarketContext) -> Result<f64> {
        let PeriodReturnInputs {
            period_start,
            period_end,
            t_start,
            ..
        } = *inputs;
        // Seasoned (in-progress) periods are not supported by the carry
        // model: the realized index move since the period start would be
        // silently replaced by projected carry. Fail loud (this preserves
        // the engine's previous behavior, which rejected past period starts).
        if t_start < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FIIndexTotalReturnSwap '{}': period starting {} is already in progress; \
                 seasoned FI index TRS pricing is not supported",
                self.trs.id.as_str(),
                period_start
            )));
        }
        // Carry model: the index earns its yield as total return.
        //
        // Year fraction computed using the schedule's day count convention
        // (same convention used for the financing leg accrual).
        let ctx = finstack_quant_core::dates::DayCountContext::default();
        let dt = self
            .trs
            .schedule
            .params
            .dc
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
/// - The discount curve is not found
pub(crate) fn pv_total_return_leg(
    trs: &FIIndexTotalReturnSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    // Warn once per process, not on every valuation call (batch pricing and
    // FD sensitivities call this hundreds of times).
    static CARRY_MODEL_WARNING: std::sync::Once = std::sync::Once::new();
    CARRY_MODEL_WARNING.call_once(|| {
        tracing::warn!(
            "FIIndexTotalReturnSwap total-return leg uses a carry-only analytic model; \
             it is not a full mark-to-market index return model"
        );
    });
    let index_yield = extract_index_yield(trs, context)?;

    let params = TotalReturnLegParams {
        schedule: &trs.schedule,
        notional: trs.notional,
        discount_curve_id: trs.financing.discount_curve_id.as_str(),
        contract_size: trs.underlying.contract_size,
        initial_level: trs.initial_level,
    };

    let model = FiIndexReturnModel { trs, index_yield };
    TrsEngine::pv_total_return_leg_with_model(params, context, as_of, &model)
}
