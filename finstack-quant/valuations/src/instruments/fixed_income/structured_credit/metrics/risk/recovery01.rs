//! Recovery01 calculator for StructuredCredit.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) change in recovery rate.

use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard recovery rate bump: 1% (0.01)
const RECOVERY_BUMP: f64 = 0.01;

/// Recovery01 calculator for StructuredCredit.
pub(crate) struct Recovery01Calculator;

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument = context.instrument_as::<StructuredCredit>()?.clone();
        let as_of = context.as_of;

        use crate::cashflow::builder::RecoveryModelSpec;

        // Get current recovery spec and create bumped versions
        let recovery_up = RecoveryModelSpec {
            rate: (instrument.credit_model.recovery_spec.rate + RECOVERY_BUMP).clamp(0.0, 1.0),
            recovery_lag: instrument.credit_model.recovery_spec.recovery_lag,
        };

        let recovery_down = RecoveryModelSpec {
            rate: (instrument.credit_model.recovery_spec.rate - RECOVERY_BUMP).clamp(0.0, 1.0),
            recovery_lag: instrument.credit_model.recovery_spec.recovery_lag,
        };

        // Actual symmetric bump width after clamping to [0, 1]. Using the nominal
        // 2·bump would halve/bias the sensitivity whenever the recovery rate sits
        // within one bump of 0 or 1 (distressed-recovery or near-boundary deals),
        // where one side clamps and the move becomes one-sided.
        let achieved_bump = recovery_up.rate - recovery_down.rate;

        // Calculate up scenario
        let mut inst_up = instrument.clone();
        inst_up.credit_model.recovery_spec = recovery_up;
        let pv_up = context.reprice_instrument_raw(&inst_up, context.curves.as_ref(), as_of)?;

        // Calculate down scenario
        let mut inst_down = instrument;
        inst_down.credit_model.recovery_spec = recovery_down;
        let pv_down = context.reprice_instrument_raw(&inst_down, context.curves.as_ref(), as_of)?;

        // RECOVERY01 = slope × 1% — dollars per 1% (0.01) recovery move,
        // matching the documented convention AND the CDS-side Recovery01
        // producers (`slope * RECOVERY_BUMP`); the former per-unit figure was
        // 100× larger, giving the same MetricId two units across producers
        // (prior fix). Pairs with `measure_recovery_shift` (pct-pt).
        let recovery01 = if achieved_bump > 0.0 {
            (pv_up - pv_down) / achieved_bump * 0.01
        } else {
            0.0
        };

        Ok(recovery01)
    }
}
