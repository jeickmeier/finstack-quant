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
        let mut financing_pv = 0.0;
        for inst_json in &inst.financing {
            let boxed = inst_json.clone().into_boxed()?;
            let pv = boxed.value(&context.curves, context.as_of)?;
            financing_pv += pv.amount();
        }
        Ok(financing_pv.abs() / denom)
    }
}

/// Minimum DSCR over the NOI dates in the asset schedule: NOI / (cash interest + fees + principal).
///
/// This is a simplified DSCR proxy computed on NOI dates. It is intended for screening and
/// covenant-like reporting, not legal covenant calculation.
#[derive(Debug, Default)]
pub(super) struct DscrMin;

impl MetricCalculator for DscrMin {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let inst = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
            .ok_or_else(|| CoreError::Validation("DscrMin: type mismatch".into()))?;

        let as_of = context.as_of;
        let exit = inst.clone().resolve_exit_date(as_of)?;

        let noi = inst.asset.noi_flows(as_of)?;
        if noi.is_empty() {
            return Err(CoreError::Validation("DscrMin: missing NOI flows".into()));
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
                    .flows
                    .iter()
                    .filter(|cf| cf.date > prev && cf.date <= d)
                    .filter(|cf| {
                        // Exclude borrower funding legs (negative Notional from lender perspective).
                        if matches!(cf.kind, finstack_quant_core::cashflow::CFKind::Notional)
                            && cf.amount.amount() < 0.0
                        {
                            return false;
                        }
                        // Exclude PIK.
                        !matches!(cf.kind, finstack_quant_core::cashflow::CFKind::PIK)
                    })
                    // Borrower debt service is negative of lender inflows. We want a positive service amount.
                    .map(|cf| cf.amount.amount().abs())
                    .sum::<f64>();
            }

            if debt_service > 0.0 {
                min_dscr = min_dscr.min(noi_amt / debt_service);
            }
            prev = d;
        }

        if !min_dscr.is_finite() {
            return Err(CoreError::Validation(
                "DscrMin: could not compute (no debt service)".into(),
            ));
        }
        Ok(min_dscr)
    }
}

/// Minimum interest-only DSCR over the NOI dates in the asset schedule:
/// NOI / (cash interest + fees), excluding scheduled principal/amortization.
#[derive(Debug, Default)]
pub(super) struct DscrMinInterestOnly;

impl MetricCalculator for DscrMinInterestOnly {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let inst = context
            .instrument
            .as_any()
            .downcast_ref::<LeveredRealEstateEquity>()
            .ok_or_else(|| CoreError::Validation("DscrMinInterestOnly: type mismatch".into()))?;

        let as_of = context.as_of;
        let exit = inst.clone().resolve_exit_date(as_of)?;

        let noi = inst.asset.noi_flows(as_of)?;
        if noi.is_empty() {
            return Err(CoreError::Validation(
                "DscrMinInterestOnly: missing NOI flows".into(),
            ));
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
                    .flows
                    .iter()
                    .filter(|cf| cf.date > prev && cf.date <= d)
                    .filter(|cf| {
                        if matches!(cf.kind, finstack_quant_core::cashflow::CFKind::PIK) {
                            return false;
                        }
                        !matches!(
                            cf.kind,
                            finstack_quant_core::cashflow::CFKind::Notional
                                | finstack_quant_core::cashflow::CFKind::Amortization
                                | finstack_quant_core::cashflow::CFKind::PrePayment
                                | finstack_quant_core::cashflow::CFKind::RevolvingRepayment
                        )
                    })
                    // Borrower debt service is negative of lender inflows. We want a positive service amount.
                    .map(|cf| cf.amount.amount().abs())
                    .sum::<f64>();
            }

            if debt_service > 0.0 {
                min_dscr = min_dscr.min(noi_amt / debt_service);
            }
            prev = d;
        }

        if !min_dscr.is_finite() {
            return Err(CoreError::Validation(
                "DscrMinInterestOnly: could not compute (no qualifying debt service)".into(),
            ));
        }
        Ok(min_dscr)
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
        let mut drawn = 0.0;
        for sched in &schedules {
            for cf in &sched.flows {
                if cf.date != context.as_of {
                    continue;
                }
                let is_draw = matches!(
                    cf.kind,
                    finstack_quant_core::cashflow::CFKind::Notional
                        | finstack_quant_core::cashflow::CFKind::RevolvingDraw
                );
                if is_draw && cf.amount.amount() < 0.0 {
                    // Lender outflow is borrower draw.
                    drawn += -cf.amount.amount();
                }
            }
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
