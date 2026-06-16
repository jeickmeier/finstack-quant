//! Prepayment01 calculator for StructuredCredit.
//!
//! Computes Prepayment01 (prepayment rate sensitivity) using finite differences.
//! Prepayment01 measures the change in PV for a 1bp (0.0001) change in the
//! prepayment rate (CPR).
//!
//! # Formula
//! ```text
//! Prepayment01 = (PV(CPR + 1bp) - PV(CPR - 1bp)) / achieved_bump
//! ```
//! Where the nominal bump is 1bp (0.0001) of annual CPR and `achieved_bump`
//! is the realized two-sided width after clamping at zero.
//!
//! For curve-shaped specs the bump targets the parameter the curve actually
//! reads: `Psa` ignores `cpr` entirely (the rate is `speed_multiplier × ramp`),
//! so the multiplier is bumped such that the peak CPR shifts by 1bp (matching
//! the attribution layer's PSA ≈ 6% terminal-CPR convention). `CmbsLockout`
//! and `Constant` read `cpr` directly.

use crate::cashflow::builder::specs::PrepaymentCurve;
use crate::cashflow::builder::PrepaymentModelSpec;
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard prepayment bump: 1bp (0.0001) of annual CPR
const PREPAYMENT_BUMP_CPR: f64 = 0.0001;

/// PSA terminal CPR at 100% speed (peak of the seasoning ramp).
const PSA_TERMINAL_CPR: f64 = 0.06;

/// Build up/down bumped specs and the achieved bump width in annual-CPR terms.
fn bumped_prepayment_specs(
    spec: &PrepaymentModelSpec,
) -> (PrepaymentModelSpec, PrepaymentModelSpec, f64) {
    match &spec.curve {
        Some(PrepaymentCurve::Psa { speed_multiplier }) => {
            // The PSA curve derives CPR from `speed_multiplier` alone; bump the
            // multiplier so the peak CPR moves by 1bp: Δmult = bump / 0.06.
            let mult_bump = PREPAYMENT_BUMP_CPR / PSA_TERMINAL_CPR;
            let mult_up = speed_multiplier + mult_bump;
            let mult_down = (speed_multiplier - mult_bump).max(0.0);
            let up = PrepaymentModelSpec {
                cpr: spec.cpr,
                curve: Some(PrepaymentCurve::Psa {
                    speed_multiplier: mult_up,
                }),
            };
            let down = PrepaymentModelSpec {
                cpr: spec.cpr,
                curve: Some(PrepaymentCurve::Psa {
                    speed_multiplier: mult_down,
                }),
            };
            let achieved = (mult_up - mult_down) * PSA_TERMINAL_CPR;
            (up, down, achieved)
        }
        // Constant / CmbsLockout / no curve all read `cpr` directly.
        _ => {
            let cpr_up = (spec.cpr + PREPAYMENT_BUMP_CPR).max(0.0);
            let cpr_down = (spec.cpr - PREPAYMENT_BUMP_CPR).max(0.0);
            let up = PrepaymentModelSpec {
                cpr: cpr_up,
                curve: spec.curve.clone(),
            };
            let down = PrepaymentModelSpec {
                cpr: cpr_down,
                curve: spec.curve.clone(),
            };
            (up, down, cpr_up - cpr_down)
        }
    }
}

/// Prepayment01 calculator for StructuredCredit.
pub(crate) struct Prepayment01Calculator;

impl MetricCalculator for Prepayment01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &StructuredCredit = context.instrument_as()?;
        let as_of = context.as_of;

        let (prepayment_up, prepayment_down, achieved_bump) =
            bumped_prepayment_specs(&instrument.credit_model.prepayment_spec);

        // Calculate up scenario
        let mut inst_up = instrument.clone();
        inst_up.credit_model.prepayment_spec = prepayment_up;
        let pv_up = inst_up.price(context.curves.as_ref(), as_of)?.amount();

        // Calculate down scenario
        let mut inst_down = instrument.clone();
        inst_down.credit_model.prepayment_spec = prepayment_down;
        let pv_down = inst_down.price(context.curves.as_ref(), as_of)?.amount();

        // Near a 0 rate the down bump clamps and the move becomes one-sided,
        // so divide by the achieved width rather than the nominal 2·bump.
        //
        // Quant review Note: the slope `(ΔPV / achieved_bump)` is in dollars
        // per UNIT of CPR; multiply by 1bp (0.0001) so the metric matches its
        // documented `$ per 1bp` convention — the unit the attribution layer's
        // `measure_prepayment_shift` (bp) pairs with directly.
        let prepayment01 = if achieved_bump > 0.0 {
            (pv_up - pv_down) / achieved_bump * 0.0001
        } else {
            0.0
        };

        Ok(prepayment01)
    }
}
