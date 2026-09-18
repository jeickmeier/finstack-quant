//! Effective-interest-rate (EIR) amortization reporting for revolving credit
//! facilities: the origination effective rate including the upfront fee and,
//! by default, every running fee.

use crate::cashflow::traits::CashflowScheduleSource;
use crate::instruments::fixed_income::loan_quotes::oid_eir_schedule_from_flows;
use crate::instruments::RevolvingCredit;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

/// Computes the EIR amortization series from the origination date and
/// returns the total amortization; stores `oid_eir_rate` and the dated
/// `oid_eir_amortization` / `oid_eir_carrying_value` series on the context.
///
/// The schedule is built as of the commitment date (the origination view an
/// effective rate belongs to; a floating facility therefore needs the same
/// contractual fixing pricing on that date needs), with the anchor drawn
/// balance as the opening funding leg and the upfront fee as a receipt on
/// the commitment date.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct OidEirAmortizationCalculator;

impl MetricCalculator for OidEirAmortizationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;
        let origination = facility.commitment_date;
        let schedule = facility.raw_cashflow_schedule(&context.curves, origination)?;
        let spec = facility.oid_eir.clone().unwrap_or_default();
        let mut extra = Vec::new();
        let upfront = facility.upfront_fee_amount();
        if upfront.amount() > 0.0 {
            extra.push((origination, upfront.amount()));
        }
        let opening_funding = (facility.drawn_amount.amount() > 0.0)
            .then(|| (origination, -facility.drawn_amount.amount()));
        let eir = oid_eir_schedule_from_flows(
            &schedule,
            &extra,
            opening_funding,
            facility.day_count,
            facility.commitment_amount.currency(),
            spec.include_fees,
        )?;

        context
            .computed
            .insert(MetricId::custom("oid_eir_rate"), eir.effective_rate);
        if !eir.periods.is_empty() {
            context.store_bucketed_series(
                MetricId::custom("oid_eir_amortization"),
                eir.periods
                    .iter()
                    .map(|p| (p.date.to_string(), p.oid_amortization.amount())),
            );
            context.store_bucketed_series(
                MetricId::custom("oid_eir_carrying_value"),
                eir.periods
                    .iter()
                    .map(|p| (p.date.to_string(), p.closing_balance.amount())),
            );
        }
        Ok(eir
            .periods
            .iter()
            .map(|p| p.oid_amortization.amount())
            .sum())
    }
}
