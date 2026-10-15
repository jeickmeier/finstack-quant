//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::instruments::fixed_income::bond::pricing::settlement::settlement_date;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates settlement-date accrued interest for bonds.
///
/// Accrued interest is always anchored at the bond's quote/settlement date so
/// `accrued`, `clean_price`, `dirty_price`, YTM, and spread metrics share one
/// market convention regardless of whether the bond carries a quote override.
///
/// The calculation uses the bond's accrual method (linear, compounded, or indexed)
/// and respects ex-coupon conventions: inside the ex-coupon window accrued
/// interest is **negative** (UK gilt / DMO convention:
/// `AI = −C × days-to-coupon / days-in-period` for the linear method), since
/// the seller retains the imminent coupon and compensates the buyer for the
/// remaining stub.
///
/// # See Also
///
/// - [`Bond::accrual_config`] for accrual configuration
/// - [`finstack_quant_cashflows::accrual`] for the accrual engine
pub(crate) struct AccruedInterestCalculator;

impl MetricCalculator for AccruedInterestCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        // Borrow bond once to compute accrued and optionally cache flows/hints
        let (accrued_amt, discount_curve_id, day_count, maybe_flows) = {
            let bond: &Bond = context.instrument_as()?;

            // Build full schedule with market context (supports FRNs, amortization, custom schedules)
            let schedule = bond.full_cashflow_schedule(&context.curves)?;

            let accrual_date = settlement_date(bond, context.as_of)?;

            // Use generic cashflow accrual engine with bond's config.
            let accrued_amt = finstack_quant_cashflows::accrual::accrued_interest_amount(
                &schedule,
                accrual_date,
                &bond.accrual_config(),
            )?;

            // Prepare potential flows for caching (build now, assign later).
            // Coupon entitlement is tested at the same date the accrued is
            // anchored to (quote/settlement date for market quotes), so the
            // cached flows and the accrued stay on one convention basis.
            let maybe_flows = if context.cashflows.is_none() {
                Some(bond.pricing_dated_cashflows_at(
                    &context.curves,
                    context.as_of,
                    accrual_date,
                )?)
            } else {
                None
            };

            (
                accrued_amt,
                bond.discount_curve_id.to_owned(),
                bond.cashflow_spec.day_count(),
                maybe_flows,
            )
        };

        // Cache basic context hints for downstream metrics
        context.discount_curve_id = Some(discount_curve_id);
        context.day_count = Some(day_count);
        // Also cache full holder cashflows for downstream risk metrics
        if context.cashflows.is_none() {
            if let Some(flows) = maybe_flows {
                context.cashflows = Some(flows);
            }
        }

        Ok(accrued_amt)
    }
}
