use crate::instruments::fixed_income::bond::pricing::settlement::settlement_date;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates accrued interest for bonds.
///
/// Computes the accrued interest since the last coupon payment. For market
/// quoted bonds, accrued is anchored at the quote/settlement date; otherwise it
/// is anchored at the valuation date. This keeps `accrued`, `clean_price`, and
/// `dirty_price` on the same market convention basis.
///
/// The calculation uses the bond's accrual method (linear, compounded, or indexed)
/// and respects ex-coupon conventions: inside the ex-coupon window accrued
/// interest is **negative** (UK gilt / DMO convention:
/// `AI = −C × days-to-coupon / days-in-period` for the linear method), since
/// the seller retains the imminent coupon and compensates the buyer for the
/// remaining stub.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
/// use finstack_quant_valuations::metrics::{MetricRegistry, MetricId, MetricContext};
/// use finstack_quant_core::market_data::context::MarketContext;
/// use finstack_quant_core::dates::Date;
///
/// # let bond = Bond::example().unwrap();
/// # let market = MarketContext::new();
/// # let as_of = Date::from_calendar_date(2024, time::Month::January, 15).unwrap();
/// // Accrued interest is computed automatically when requesting bond metrics
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # See Also
///
/// - [`Bond::accrual_config`] for accrual configuration
/// - [`crate::cashflow::accrual`] for the accrual engine
pub(crate) struct AccruedInterestCalculator;

impl MetricCalculator for AccruedInterestCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        // Borrow bond once to compute accrued and optionally cache flows/hints
        let (accrued_amt, discount_curve_id, dc, maybe_flows) = {
            let bond: &Bond = context.instrument_as()?;

            // Build full schedule with market context (supports FRNs, amortization, custom schedules)
            let schedule = bond.full_cashflow_schedule(&context.curves)?;

            let accrual_date = if bond
                .instrument_pricing_overrides
                .market_quotes
                .has_price_driver()
            {
                settlement_date(bond, context.as_of)?
            } else {
                context.as_of
            };

            // Use generic cashflow accrual engine with bond's config.
            let accrued_amt = crate::cashflow::accrual::accrued_interest_amount(
                &schedule,
                accrual_date,
                &bond.accrual_config(),
            )?;

            // Prepare potential flows for caching (build now, assign later)
            let maybe_flows = if context.cashflows.is_none() {
                Some(bond.pricing_dated_cashflows(&context.curves, context.as_of)?)
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
        context.day_count = Some(dc);
        // Also cache full holder cashflows for downstream risk metrics
        if context.cashflows.is_none() {
            if let Some(flows) = maybe_flows {
                context.cashflows = Some(flows);
            }
        }

        Ok(accrued_amt)
    }
}
