//! Metrics for [`LeveredRealEstateEquity`].

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::real_estate::LeveredRealEstateEquity;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::cashflow::xirr_with_daycount;
use finstack_quant_core::Error as CoreError;

/// Levered equity IRR (XIRR-style) from the levered equity cashflow schedule.
#[derive(Debug, Default)]
pub(super) struct LeveredIrr;

impl MetricCalculator for LeveredIrr {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let inst = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
            .ok_or_else(|| CoreError::Validation("LeveredIrr: instrument type mismatch".into()))?;

        let flows = inst.equity_cashflows(&context.curves, context.as_of)?;
        xirr_with_daycount(flows.as_slice(), inst.irr_day_count(), None)
    }
}

/// Equity multiple (MOIC-like): total inflows / total outflows (absolute).
#[derive(Debug, Default)]
pub(super) struct EquityMultiple;

impl MetricCalculator for EquityMultiple {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let inst = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
            .ok_or_else(|| {
                CoreError::Validation("EquityMultiple: instrument type mismatch".into())
            })?;

        let flows = inst.equity_cashflows(&context.curves, context.as_of)?;
        let mut inflows = 0.0;
        let mut outflows = 0.0;
        for (_d, a) in flows {
            if a >= 0.0 {
                inflows += a;
            } else {
                outflows += -a;
            }
        }
        if outflows <= 0.0 {
            return Err(CoreError::Validation(
                "EquityMultiple: total outflows must be positive".into(),
            ));
        }
        Ok(inflows / outflows)
    }
}

/// Loan-to-value at `as_of`: financing PV / asset PV.
///
/// Uses present values (not face/outstanding) to keep the metric consistent with
/// the library's valuation approach.
#[derive(Debug, Default)]
pub(super) struct LoanToValue;

impl MetricCalculator for LoanToValue {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let inst = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
            .ok_or_else(|| CoreError::Validation("LoanToValue: type mismatch".into()))?;

        let asset_pv = inst.asset.value(&context.curves, context.as_of)?;
        let denom = asset_pv.amount();
        if denom <= 0.0 {
            return Err(CoreError::Validation(
                "LoanToValue: asset PV must be positive".into(),
            ));
        }
        let financing_pv =
            super::super::levered_pricer::financing_pv(inst, &context.curves, context.as_of)?;
        Ok(financing_pv.abs() / denom)
    }
}

/// True when a cashflow kind counts as *scheduled* debt service.
///
/// Market DSCR uses NOI over scheduled debt service: cash interest, fees, and
/// (unless interest-only) scheduled amortization. Balloon/funding `Notional`
/// legs, PIK accruals, prepayments, and revolver movements are excluded.
fn is_scheduled_debt_service(
    kind: finstack_quant_core::cashflow::CFKind,
    interest_only: bool,
) -> bool {
    use finstack_quant_core::cashflow::CFKind;
    match kind {
        CFKind::Notional
        | CFKind::Pik
        | CFKind::PrePayment
        | CFKind::RevolvingDraw
        | CFKind::RevolvingRepayment => false,
        CFKind::Amortization => !interest_only,
        _ => true,
    }
}

fn min_dscr(
    context: &MetricContext,
    interest_only: bool,
    metric_name: &str,
) -> finstack_quant_core::Result<f64> {
    let inst = context
        .instrument
        .as_any()
        .downcast_ref::<LeveredRealEstateEquity>()
        .ok_or_else(|| CoreError::Validation(format!("{metric_name}: type mismatch")))?;

    let as_of = context.as_of;
    let exit = inst.resolve_exit_date(as_of)?;

    let noi = inst.asset.noi_flows(as_of)?;
    if noi.is_empty() {
        return Err(CoreError::Validation(format!(
            "{metric_name}: missing NOI flows"
        )));
    }

    let schedules = inst.financing_schedules_supported(&context.curves, as_of)?;

    let mut min_dscr = f64::INFINITY;
    let mut prev = as_of;
    for (d, noi_amt) in noi {
        if d > exit {
            break;
        }
        let mut debt_service = 0.0;
        for sched in &schedules {
            debt_service += sched
                .get_flows()
                .iter()
                .filter(|cf| cf.date > prev && cf.date <= d)
                // Lender outflows (e.g., funding legs, revolver draws) are
                // borrower receipts, never debt service.
                .filter(|cf| cf.amount.amount() > 0.0)
                .filter(|cf| is_scheduled_debt_service(cf.kind, interest_only))
                .map(|cf| cf.amount.amount())
                .sum::<f64>();
        }

        if debt_service > 0.0 {
            min_dscr = min_dscr.min(noi_amt / debt_service);
        }
        prev = d;
    }

    if !min_dscr.is_finite() {
        return Err(CoreError::Validation(format!(
            "{metric_name}: could not compute (no qualifying debt service)"
        )));
    }
    Ok(min_dscr)
}

/// Minimum DSCR over the NOI dates in the asset schedule:
/// NOI / (cash interest + fees + scheduled amortization).
///
/// Balloon principal at maturity, prepayments, and revolver movements are
/// excluded — DSCR measures coverage of *scheduled* debt service only.
/// This is a simplified DSCR proxy computed on NOI dates. It is intended for
/// screening and covenant-like reporting, not legal covenant calculation.
#[derive(Debug, Default)]
pub(super) struct DscrMin;

impl MetricCalculator for DscrMin {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        min_dscr(context, false, "DscrMin")
    }
}

/// Minimum interest-only DSCR over the NOI dates in the asset schedule:
/// NOI / (cash interest + fees), excluding scheduled principal/amortization.
#[derive(Debug, Default)]
pub(super) struct DscrMinInterestOnly;

impl MetricCalculator for DscrMinInterestOnly {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        min_dscr(context, true, "DscrMinInterestOnly")
    }
}

/// Loan-to-value at origination: initial debt drawn / purchase price.
#[derive(Debug, Default)]
pub(super) struct LoanToValueAtOrigination;

impl MetricCalculator for LoanToValueAtOrigination {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let inst = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
            .ok_or_else(|| {
                CoreError::Validation("LoanToValueAtOrigination: type mismatch".into())
            })?;

        let purchase = inst.asset.purchase_price.ok_or_else(|| {
            CoreError::Validation("LoanToValueAtOrigination: purchase_price is required".into())
        })?;
        if purchase.currency() != inst.currency {
            return Err(CoreError::Validation(
                "LoanToValueAtOrigination: purchase_price currency mismatch".into(),
            ));
        }
        if purchase.amount() <= 0.0 {
            return Err(CoreError::Validation(
                "LoanToValueAtOrigination: purchase_price must be positive".into(),
            ));
        }

        let schedules = inst.financing_schedules_supported(&context.curves, context.as_of)?;
        // Origination draw = lender outflows (negative Notional/RevolvingDraw)
        // on the first draw date on/after as_of, per financing instrument.
        // Matching only flows dated exactly at as_of would silently return 0
        // LTV whenever funding settles after the valuation date.
        let mut drawn = 0.0;
        for sched in &schedules {
            let draws: Vec<_> = sched
                .get_flows()
                .iter()
                .filter(|cf| cf.date >= context.as_of)
                .filter(|cf| {
                    matches!(
                        cf.kind,
                        finstack_quant_core::cashflow::CFKind::Notional
                            | finstack_quant_core::cashflow::CFKind::RevolvingDraw
                    ) && cf.amount.amount() < 0.0
                })
                .collect();
            if let Some(first_date) = draws.iter().map(|cf| cf.date).min() {
                drawn += draws
                    .iter()
                    .filter(|cf| cf.date == first_date)
                    .map(|cf| -cf.amount.amount())
                    .sum::<f64>();
            }
        }
        if drawn <= 0.0 {
            return Err(CoreError::Validation(
                "LoanToValueAtOrigination: no financing draw found on/after as_of".into(),
            ));
        }

        Ok(drawn / purchase.amount())
    }
}

/// Debt payoff at exit (absolute amount).
#[derive(Debug, Default)]
pub(super) struct DebtPayoffAtExit;

impl MetricCalculator for DebtPayoffAtExit {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let inst = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
            .ok_or_else(|| CoreError::Validation("DebtPayoffAtExit: type mismatch".into()))?;

        let payoff = inst.financing_payoff_at_exit(&context.curves, context.as_of)?;
        Ok(payoff.amount())
    }
}
