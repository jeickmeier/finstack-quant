//! Effective-interest-rate (EIR) amortization reporting for revolving credit
//! facilities: the origination effective rate including the upfront fee and,
//! by default, every running fee.

use crate::loan_quotes::{OidEirSchedule, oid_eir_schedule_from_flows};
use crate::revolving_credit::RevolvingCredit;
use finstack_quant_cashflows::CashflowScheduleSource;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// Solve the origination EIR schedule of the facility in `context`.
///
/// The schedule is built as of the commitment date (the origination view an
/// effective rate belongs to; a floating facility therefore needs the same
/// contractual fixing pricing on that date needs), with the anchor drawn
/// balance as the opening funding leg and the upfront fee as a receipt on
/// the commitment date.
fn origination_schedule(context: &MetricContext) -> finstack_quant_core::Result<OidEirSchedule> {
    let facility: &RevolvingCredit = context.instrument_as()?;
    let origination = facility.issue_date;
    let schedule = facility.raw_cashflow_schedule(&context.curves, origination)?;
    let spec = facility.oid_eir.clone().unwrap_or_default();
    let mut extra = Vec::new();
    let upfront = facility.upfront_fee_amount()?;
    if upfront.amount() > 0.0 {
        extra.push((origination, upfront.amount()));
    }
    let opening_funding =
        (facility.drawn.amount() > 0.0).then(|| (origination, -facility.drawn.amount()));
    oid_eir_schedule_from_flows(
        &schedule,
        &extra,
        opening_funding,
        facility.day_count,
        facility.commitment.currency(),
        spec.include_fees,
    )
}

/// Computes the EIR amortization series from the origination date and
/// returns the total amortization; stores `oid_eir_rate` and the dated
/// `oid_eir_amortization` / `oid_eir_carrying_value` series on the context.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct OidEirAmortizationCalculator;

impl MetricCalculator for OidEirAmortizationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let eir = origination_schedule(context)?;
        eir.record(context);
        Ok(eir.total_amortization())
    }
}

/// Computes the origination effective interest rate (decimal XIRR).
///
/// Shares one solve with [`OidEirAmortizationCalculator`]: whichever of the two
/// runs first stores both results, and the registry then skips the other.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct OidEirRateCalculator;

impl MetricCalculator for OidEirRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let eir = origination_schedule(context)?;
        eir.record(context);
        Ok(eir.effective_rate)
    }
}
