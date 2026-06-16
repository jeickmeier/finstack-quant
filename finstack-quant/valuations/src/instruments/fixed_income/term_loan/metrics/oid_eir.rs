//! Effective interest rate (EIR) amortization reporting for term loans.

use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};

/// Computes EIR amortization series and returns total amortization.
pub(crate) struct OidEirAmortizationCalculator;

impl MetricCalculator for OidEirAmortizationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let loan: &TermLoan = context.instrument_as()?;
        let market = &context.curves;
        let as_of = context.as_of;

        let schedule =
            crate::instruments::fixed_income::term_loan::cashflows::build_oid_eir_schedule(
                loan, market, as_of,
            )?;

        context
            .computed
            .insert(MetricId::custom("oid_eir_rate"), schedule.effective_rate);

        if !schedule.periods.is_empty() {
            let amort_series = schedule
                .periods
                .iter()
                .map(|p| (p.date.to_string(), p.oid_amortization.amount()));
            context.store_bucketed_series(MetricId::custom("oid_eir_amortization"), amort_series);

            let carrying_series = schedule
                .periods
                .iter()
                .map(|p| (p.date.to_string(), p.closing_balance.amount()));
            context
                .store_bucketed_series(MetricId::custom("oid_eir_carrying_value"), carrying_series);
        }

        let total = schedule
            .periods
            .iter()
            .map(|p| p.oid_amortization.amount())
            .sum();
        Ok(total)
    }
}
