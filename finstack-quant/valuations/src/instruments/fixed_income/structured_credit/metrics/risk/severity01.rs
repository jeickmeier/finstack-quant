//! Severity01 calculator for StructuredCredit.
//!
//! Computes Severity01 (loss severity sensitivity) using finite differences.
//! Severity01 measures the change in PV for a 1% (0.01) change in loss severity.
//!
//! # Formula
//! ```text
//! Severity01 = (PV(severity + 1%) - PV(severity - 1%)) / (2 * bump_size)
//! ```
//! Where bump_size is 1% (0.01).
//!
//! # Note
//! Loss Severity = 1 - Recovery Rate (LGD = Loss Given Default)
//! This metric is related to Recovery01 but measures sensitivity to loss severity
//! rather than recovery. For constant recovery, Severity01 ≈ -Recovery01.

use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard severity bump: 1% (0.01)
const SEVERITY_BUMP: f64 = 0.01;

/// Severity01 calculator for StructuredCredit.
pub(crate) struct Severity01Calculator;

impl MetricCalculator for Severity01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &StructuredCredit = context.instrument_as()?;
        let as_of = context.as_of;

        use crate::cashflow::builder::RecoveryModelSpec;

        // Loss Severity = 1 - Recovery Rate
        // So bumping severity up means bumping recovery down, and vice versa
        let recovery_up = RecoveryModelSpec {
            rate: (instrument.credit_model.recovery_spec.rate - SEVERITY_BUMP).clamp(0.0, 1.0),
            recovery_lag: instrument.credit_model.recovery_spec.recovery_lag,
        };

        let recovery_down = RecoveryModelSpec {
            rate: (instrument.credit_model.recovery_spec.rate + SEVERITY_BUMP).clamp(0.0, 1.0),
            recovery_lag: instrument.credit_model.recovery_spec.recovery_lag,
        };

        // Actual severity bump width after clamping recovery to [0, 1]. Severity
        // = 1 − recovery, so Δseverity = recovery_down.rate − recovery_up.rate.
        // Using the nominal 2·bump would bias the sensitivity when recovery sits
        // within one bump of 0 or 1 and one side clamps.
        let achieved_bump = recovery_down.rate - recovery_up.rate;

        // Calculate up scenario (lower recovery = higher severity)
        let mut inst_up = instrument.clone();
        inst_up.credit_model.recovery_spec = recovery_up;
        let pv_up = inst_up.price(context.curves.as_ref(), as_of)?.amount();

        // Calculate down scenario (higher recovery = lower severity)
        let mut inst_down = instrument.clone();
        inst_down.credit_model.recovery_spec = recovery_down;
        let pv_down = inst_down.price(context.curves.as_ref(), as_of)?.amount();

        // Severity01 = (PV_up - PV_down) / achieved_bump
        // PV_up is with lower recovery (higher severity)
        // PV_down is with higher recovery (lower severity)
        let severity01 = if achieved_bump > 0.0 {
            (pv_up - pv_down) / achieved_bump
        } else {
            0.0
        };

        Ok(severity01)
    }
}
