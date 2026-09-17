//! Recovery01 calculator for StructuredCredit.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) change in recovery rate.

use super::effective_recovery;
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard recovery rate bump: 1% (0.01)
const RECOVERY_BUMP: f64 = 0.01;

/// Recovery01 calculator for StructuredCredit.
pub(crate) struct Recovery01Calculator;

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument = context
            .instrument_as::<StructuredCredit>()?
            .resolved_for_pricing()?;
        let as_of = context.as_of;

        use crate::cashflow::builder::RecoveryModelSpec;

        // Get current recovery spec and create bumped versions; a severity
        // vector by month of default is bumped entry by entry.
        let base = &instrument.credit_model.recovery_spec;
        let bumped = |delta: f64| -> RecoveryModelSpec {
            let mut spec = base.clone();
            spec.rate = (base.rate + delta).clamp(0.0, 1.0);
            if let Some(severities) = &base.severity_vector {
                spec.severity_vector = Some(
                    severities
                        .iter()
                        .map(|severity| (severity - delta).clamp(0.0, 1.0))
                        .collect(),
                );
            }
            spec
        };
        let recovery_up = bumped(RECOVERY_BUMP);
        let recovery_down = bumped(-RECOVERY_BUMP);

        // Actual symmetric bump width after clamping to [0, 1], measured on the
        // effective (vector-averaged) recovery. Using the nominal 2·bump would
        // halve/bias the sensitivity whenever the recovery rate sits within one
        // bump of 0 or 1 (distressed-recovery or near-boundary deals), where
        // one side clamps and the move becomes one-sided.
        let achieved_bump = effective_recovery(&recovery_up) - effective_recovery(&recovery_down);

        let mut inst_up = instrument.clone();
        inst_up.credit_model.recovery_spec = recovery_up;
        let pv_up = context.reprice_instrument_raw(&inst_up, context.curves.as_ref(), as_of)?;

        let mut inst_down = instrument;
        inst_down.credit_model.recovery_spec = recovery_down;
        let pv_down = context.reprice_instrument_raw(&inst_down, context.curves.as_ref(), as_of)?;

        // RECOVERY01 = slope × 1% — dollars per 1% (0.01) recovery move,
        // matching the documented convention AND the CDS-side Recovery01
        // producers (`slope * RECOVERY_BUMP`); the former per-unit figure was
        // 100× larger, giving the same MetricId two units across producers
        // (prior fix). Model-parameter attribution consumes this `$ per 1%`
        // convention.
        let recovery01 = if achieved_bump > 0.0 {
            (pv_up - pv_down) / achieved_bump * 0.01
        } else {
            0.0
        };

        Ok(recovery01)
    }
}
