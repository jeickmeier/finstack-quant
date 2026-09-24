//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::money::Money;

/// Calculates yield to maturity (YTM) for bonds.
///
/// YTM is defined here as the internal rate of return that equates the present
/// value of **all projected future cashflows** to the current dirty market
/// price (quoted clean price plus accrued interest at the **quote date**).
///
/// # Quote-Date Convention
///
/// YTM is computed relative to the **quote date** (settlement date when
/// `settlement_days` is set, otherwise `as_of`):
/// - Accrued interest is computed at the quote date
/// - Cashflows before the quote date are excluded
/// - Time to each cashflow is measured from the quote date
///
/// This matches market convention where bond quotes are settlement-date quotes.
///
/// # Applicability
///
/// - **Primary use**: plain-vanilla **fixed-rate bullet bonds**, where YTM has
///   the usual market interpretation (coupon-like yield for comparison).
/// - **Other cashflow specs**: for floating-rate, amortizing, or custom
///   cashflow structures, this calculator still solves a well-defined IRR off
///   the full discounted cashflow schedule. The resulting YTM is a
///   **cashflow-implied yield**, but it is **not** the market-standard quote
///   for FRNs (where **discount margin** is preferred) and may have less direct
///   interpretation for exotic structures.
///
/// Implementation detail: the `coupon_rate` field in `YtmPricingSpec` is used
/// only as a **solver hint / initial guess**. For non-fixed `CashflowSpec`
/// variants this is set to `0.0`, but the solved YTM is fully determined by
/// the explicit projected cashflows and the target price, not by this hint.
///
/// # Dependencies
///
/// None (accrued is computed internally at quote_date).
pub(crate) struct YtmCalculator;

impl MetricCalculator for YtmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let maybe_clean_px = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price;
        let notional = bond.notional;
        let day_count = bond.cashflow_spec.day_count();
        let discount_curve_id = bond.discount_curve_id.to_owned();
        // Only seeds the solver; non-fixed coupon shapes start from 0.
        let coupon = bond.cashflow_spec.plain_fixed_rate()?.unwrap_or(0.0);
        let frequency = bond.cashflow_spec.frequency();

        // Compute quote-date context (settlement date and accrued at settlement)
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        // Determine dirty price in currency at the quote date.
        //
        // Preferred path: use quoted clean price (market quote) plus accrued
        // interest at the quote date to build the dirty market price.
        // When no quoted clean price is available, fall back to the model PV
        // adjusted for time value between as_of and quote_date.
        let dirty: Money = if let Some(clean_px) = maybe_clean_px {
            // Compute dirty price at quote_date: clean% × notional + accrued_at_quote
            let dirty_amt = quote_ctx.dirty_from_clean_pct(clean_px, notional.amount());
            Money::new(dirty_amt, notional.currency())?
        } else {
            // Fallback: forward-value the model PV (computed at `as_of`) to the
            // quote/settlement date so the solved YTM discounts cashflows from
            // the same origin (`quote_date`) at which the dirty price is
            // expressed (see `model_dirty_at_quote_date` for the carry
            // rationale shared with the Z-spread and DM fallbacks).
            let pv_at_quote =
                crate::instruments::fixed_income::bond::pricing::settlement::model_dirty_at_quote_date(
                    bond,
                    &context.curves,
                    context.as_of,
                    quote_ctx.quote_date,
                    context.base_value.amount(),
                )?;
            Money::new(pv_at_quote, notional.currency())?
        };

        // Build and cache flows and hints if not already present. Coupon
        // entitlement is tested at the quote/settlement date so a trade whose
        // settlement lands inside an ex-coupon window drops the imminent
        // coupon, consistent with the (negative) accrued at the quote date.
        if context.cashflows.is_none() {
            let bond: &Bond = context.instrument_as()?;
            let flows = quote_ctx.entitled_flows(bond, &context.curves, context.as_of)?;
            context.cashflows = Some(flows);
            context.discount_curve_id = Some(discount_curve_id);
            context.day_count = Some(day_count);
        }
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "cashflows".to_string(),
            })
        })?;

        // Solve for YTM using shared solver with Street compounding (default)
        // Time origin is the quote_date (settlement date) to match market convention
        let ytm = crate::instruments::fixed_income::bond::pricing::ytm_solver::solve_ytm(
            flows,
            quote_ctx.quote_date,
            dirty,
            crate::instruments::fixed_income::bond::pricing::ytm_solver::YtmPricingSpec {
                day_count,
                notional,
                coupon_rate: coupon,
                compounding:
                    crate::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding::Street,
                frequency,
            },
        )?;

        Ok(ytm)
    }
}
