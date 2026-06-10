//! Default01 calculator for StructuredCredit.
//!
//! Computes Default01 (default rate sensitivity) using finite differences.
//! Default01 measures the change in PV for a 1bp (0.0001) change in the
//! default rate (CDR).
//!
//! # Formula
//! ```text
//! Default01 = (PV(CDR + 1bp) - PV(CDR - 1bp)) / achieved_bump
//! ```
//! Where the nominal bump is 1bp (0.0001) of annual CDR and `achieved_bump`
//! is the realized two-sided width after clamping at zero.
//!
//! For the `Sda` curve the bump targets `speed_multiplier` (the curve ignores
//! `cdr`): the multiplier is bumped such that the peak CDR (0.60% at 100% SDA)
//! shifts by 1bp. `Constant`/no-curve specs bump `cdr` directly.

use crate::cashflow::builder::specs::DefaultCurve;
use crate::cashflow::builder::DefaultModelSpec;
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Standard default bump: 1bp (0.0001) of annual CDR
const DEFAULT_BUMP_CDR: f64 = 0.0001;

/// SDA peak annual CDR at 100% speed (month 30–60 plateau).
const SDA_PEAK_CDR: f64 = 0.006;

/// Build up/down bumped specs and the achieved bump width in annual-CDR terms.
fn bumped_default_specs(spec: &DefaultModelSpec) -> (DefaultModelSpec, DefaultModelSpec, f64) {
    match &spec.curve {
        Some(DefaultCurve::Sda { speed_multiplier }) => {
            // The SDA curve derives CDR from `speed_multiplier` alone; bump the
            // multiplier so the peak CDR moves by 1bp: Δmult = bump / 0.006.
            let mult_bump = DEFAULT_BUMP_CDR / SDA_PEAK_CDR;
            let mult_up = speed_multiplier + mult_bump;
            let mult_down = (speed_multiplier - mult_bump).max(0.0);
            let up = DefaultModelSpec {
                cdr: spec.cdr,
                curve: Some(DefaultCurve::Sda {
                    speed_multiplier: mult_up,
                }),
            };
            let down = DefaultModelSpec {
                cdr: spec.cdr,
                curve: Some(DefaultCurve::Sda {
                    speed_multiplier: mult_down,
                }),
            };
            let achieved = (mult_up - mult_down) * SDA_PEAK_CDR;
            (up, down, achieved)
        }
        // Constant / no curve read `cdr` directly.
        _ => {
            let cdr_up = (spec.cdr + DEFAULT_BUMP_CDR).max(0.0);
            let cdr_down = (spec.cdr - DEFAULT_BUMP_CDR).max(0.0);
            let up = DefaultModelSpec {
                cdr: cdr_up,
                curve: spec.curve.clone(),
            };
            let down = DefaultModelSpec {
                cdr: cdr_down,
                curve: spec.curve.clone(),
            };
            (up, down, cdr_up - cdr_down)
        }
    }
}

/// Default01 calculator for StructuredCredit.
pub(crate) struct Default01Calculator;

impl MetricCalculator for Default01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let instrument: &StructuredCredit = context.instrument_as()?;
        let as_of = context.as_of;

        let (default_up, default_down, achieved_bump) =
            bumped_default_specs(&instrument.credit_model.default_spec);

        // Calculate up scenario
        let mut inst_up = instrument.clone();
        inst_up.credit_model.default_spec = default_up;
        let pv_up = inst_up.price(context.curves.as_ref(), as_of)?.amount();

        // Calculate down scenario
        let mut inst_down = instrument.clone();
        inst_down.credit_model.default_spec = default_down;
        let pv_down = inst_down.price(context.curves.as_ref(), as_of)?.amount();

        // Near CDR ≈ 0 the down bump clamps and the move becomes one-sided,
        // so divide by the achieved width rather than the nominal 2·bump.
        let default01 = if achieved_bump > 0.0 {
            (pv_up - pv_down) / achieved_bump
        } else {
            0.0
        };

        Ok(default01)
    }
}
