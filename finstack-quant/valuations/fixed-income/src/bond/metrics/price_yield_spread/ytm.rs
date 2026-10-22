//! Yield-to-maturity calculators (Street and Moosmüller compounding).
//!
use super::quote_date_dirty_target;
use crate::bond::pricing::quote_conversions::YieldCompounding;
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::Bond;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Calculates yield to maturity (YTM) for bonds under one compounding
/// convention.
///
/// YTM is defined here as the internal rate of return that equates the present
/// value of **all projected future cashflows** to the current dirty market
/// price (quoted clean price plus accrued interest at the **quote date**).
/// The registry registers it twice: `Ytm` with
/// [`YieldCompounding::Street`] and `MoosmullerYtm` with
/// [`YieldCompounding::Moosmuller`], which solves the schedule-aware
///
/// ```text
/// PV = 1/(1 + y*w) * [CF_1 + Σ_{k≥2} CF_k / (1 + y/f)^{k-1}]
/// ```
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
pub(crate) struct YtmCalculator {
    /// Compounding convention the yield is solved under.
    pub(crate) compounding: YieldCompounding,
}

impl MetricCalculator for YtmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let notional = bond.notional;
        let day_count = bond.cashflow_spec.day_count();
        let discount_curve_id = bond.discount_curve_id.to_owned();
        // Only seeds the solver; non-fixed coupon shapes start from 0.
        let coupon = bond.cashflow_spec.plain_fixed_rate()?.unwrap_or(0.0);
        let frequency = bond.cashflow_spec.frequency();

        // Compute quote-date context (settlement date and accrued at settlement)
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        // Dirty price in currency at the quote date: the quoted clean price
        // plus accrued there, otherwise the model PV carried to the quote date
        // so the solved yield discounts from the same origin.
        let dirty = Money::new(
            quote_date_dirty_target(bond, context, &quote_ctx)?,
            notional.currency(),
        )?;

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
        let flows = context.require_cashflows()?;

        // Time origin is the quote_date (settlement date) to match market convention
        let bond: &Bond = context.instrument_as()?;
        crate::bond::pricing::ytm_solver::solve_bond_ytm(
            bond,
            flows,
            quote_ctx.quote_date,
            dirty,
            crate::bond::pricing::ytm_solver::YtmPricingSpec {
                day_count,
                notional,
                coupon_rate: coupon,
                compounding: self.compounding,
                frequency,
            },
        )
    }
}
