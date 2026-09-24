//! Moosmüller yield-to-maturity calculator.
//!
use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::money::Money;

/// Calculates Moosmüller YTM by solving the schedule-aware compounding
///
/// ```text
/// PV = 1/(1 + y*w) * [CF_1 + Σ_{k≥2} CF_k / (1 + y/f)^{k-1}]
/// ```
///
/// to the quote-date dirty price. Street [`super::ytm::YtmCalculator`] stays
/// hardcoded to [`crate::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding::Street`].
///
/// # Dependencies
///
/// None (accrued is computed internally at quote_date).
pub(crate) struct MoosmullerYtmCalculator;

impl MetricCalculator for MoosmullerYtmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let maybe_clean_px = bond
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price;
        let notional = bond.notional;
        let day_count = bond.cashflow_spec.day_count();
        let discount_curve_id = bond.discount_curve_id.to_owned();
        let coupon = bond.cashflow_spec.plain_fixed_rate()?.unwrap_or(0.0);
        let frequency = bond.cashflow_spec.frequency();
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;

        let dirty: Money = if let Some(clean_px) = maybe_clean_px {
            Money::new(
                quote_ctx.dirty_from_clean_pct(clean_px, notional.amount()),
                notional.currency(),
            )?
        } else {
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

        crate::instruments::fixed_income::bond::pricing::ytm_solver::solve_ytm(
            flows,
            quote_ctx.quote_date,
            dirty,
            crate::instruments::fixed_income::bond::pricing::ytm_solver::YtmPricingSpec {
                day_count,
                notional,
                coupon_rate: coupon,
                compounding:
                    crate::instruments::fixed_income::bond::pricing::quote_conversions::YieldCompounding::Moosmuller,
                frequency,
            },
        )
    }
}
