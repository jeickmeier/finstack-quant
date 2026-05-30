//! Recovery01 calculator for StructuredCredit.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) change in recovery rate.

use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Standard recovery rate bump: 1% (0.01)
const RECOVERY_BUMP: f64 = 0.01;

/// Recovery01 calculator for StructuredCredit.
pub(crate) struct Recovery01Calculator;

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &StructuredCredit = context.instrument_as()?;
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
        let pv_up = inst_up.price(context.curves.as_ref(), as_of)?.amount();

        // Calculate down scenario
        let mut inst_down = instrument.clone();
        inst_down.credit_model.recovery_spec = recovery_down;
        let pv_down = inst_down.price(context.curves.as_ref(), as_of)?.amount();

        // RECOVERY01 = (PV_up - PV_down) / achieved_bump
        let recovery01 = if achieved_bump > 0.0 {
            (pv_up - pv_down) / achieved_bump
        } else {
            0.0
        };

        Ok(recovery01)
    }
}
