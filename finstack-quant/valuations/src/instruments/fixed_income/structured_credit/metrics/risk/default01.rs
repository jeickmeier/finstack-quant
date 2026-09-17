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
use crate::instruments::fixed_income::structured_credit::assumptions::embedded_registry_or_panic;
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Standard default bump: 1bp (0.0001) of annual CDR
const DEFAULT_BUMP_CDR: f64 = 0.0001;

/// Build up/down bumped specs and the achieved bump width in annual-CDR terms.
fn bumped_default_specs(spec: &DefaultModelSpec) -> (DefaultModelSpec, DefaultModelSpec, f64) {
    match &spec.curve {
        Some(DefaultCurve::Sda { speed_multiplier }) => {
            // The SDA curve derives CDR from `speed_multiplier` alone; bump the
            // multiplier so the peak CDR moves by 1bp: Δmult = bump / peak.
            // Peak CDR comes from the same registry the curve itself uses
            // (0.60% at 100% SDA with the standard values), so the bump
            // scaling cannot drift from the simulated curve.
            let sda_peak_cdr = embedded_registry_or_panic().sda_curve().peak_cdr;
            let mult_bump = DEFAULT_BUMP_CDR / sda_peak_cdr;
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
            let achieved = (mult_up - mult_down) * sda_peak_cdr;
            (up, down, achieved)
        }
        // Explicit vectors are bumped additively, entry by entry.
        Some(DefaultCurve::Vector { monthly_cdr }) => {
            let shifted = |delta: f64| -> Vec<f64> {
                monthly_cdr
                    .iter()
                    .map(|cdr| (cdr + delta).clamp(0.0, 1.0))
                    .collect()
            };
            let up_vec = shifted(DEFAULT_BUMP_CDR);
            let down_vec = shifted(-DEFAULT_BUMP_CDR);
            let achieved =
                up_vec.first().copied().unwrap_or(0.0) - down_vec.first().copied().unwrap_or(0.0);
            let up = DefaultModelSpec {
                cdr: spec.cdr,
                curve: Some(DefaultCurve::Vector {
                    monthly_cdr: up_vec,
                }),
            };
            let down = DefaultModelSpec {
                cdr: spec.cdr,
                curve: Some(DefaultCurve::Vector {
                    monthly_cdr: down_vec,
                }),
            };
            (up, down, achieved)
        }
        // Lifetime curves state defaults as a share of the original balance,
        // so the bump is 1bp of *lifetime* cumulative defaults: the loss
        // curve is scaled so its terminal default fraction moves by the bump.
        Some(DefaultCurve::CumulativeLoss {
            cumulative_net_loss_pct,
            severity,
        }) => {
            let terminal = cumulative_net_loss_pct.last().copied().unwrap_or(0.0);
            let delta_pct = DEFAULT_BUMP_CDR * severity * 100.0;
            let scaled = |delta: f64| -> Vec<f64> {
                if terminal > 0.0 {
                    let factor = (terminal + delta).max(0.0) / terminal;
                    cumulative_net_loss_pct
                        .iter()
                        .map(|loss| loss * factor)
                        .collect()
                } else {
                    cumulative_net_loss_pct
                        .iter()
                        .map(|loss| (loss + delta).max(0.0))
                        .collect()
                }
            };
            let up_vec = scaled(delta_pct);
            let down_vec = scaled(-delta_pct);
            let achieved = (up_vec.last().copied().unwrap_or(0.0)
                - down_vec.last().copied().unwrap_or(0.0))
                / 100.0
                / severity;
            let up = DefaultModelSpec::cumulative_loss(up_vec, *severity);
            let down = DefaultModelSpec::cumulative_loss(down_vec, *severity);
            (up, down, achieved)
        }
        Some(DefaultCurve::Timing {
            cumulative_default_rate,
            annual_pct,
        }) => {
            let rate_up = (cumulative_default_rate + DEFAULT_BUMP_CDR).min(1.0);
            let rate_down = (cumulative_default_rate - DEFAULT_BUMP_CDR).max(0.0);
            let up = DefaultModelSpec::timing(rate_up, annual_pct.clone());
            let down = DefaultModelSpec::timing(rate_down, annual_pct.clone());
            (up, down, rate_up - rate_down)
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
        let instrument = context
            .instrument_as::<StructuredCredit>()?
            .resolved_for_pricing()?;
        let as_of = context.as_of;

        let (default_up, default_down, achieved_bump) =
            bumped_default_specs(&instrument.credit_model.default_spec);

        let mut inst_up = instrument.clone();
        inst_up.credit_model.default_spec = default_up;
        let pv_up = context.reprice_instrument_raw(&inst_up, context.curves.as_ref(), as_of)?;

        let mut inst_down = instrument;
        inst_down.credit_model.default_spec = default_down;
        let pv_down = context.reprice_instrument_raw(&inst_down, context.curves.as_ref(), as_of)?;

        // Near CDR ≈ 0 the down bump clamps and the move becomes one-sided,
        // so divide by the achieved width rather than the nominal 2·bump.
        // Quant review Note: rescale the per-unit slope to the documented
        // `$ per 1bp` convention used by model-parameter attribution.
        let default01 = if achieved_bump > 0.0 {
            (pv_up - pv_down) / achieved_bump * 0.0001
        } else {
            0.0
        };

        Ok(default01)
    }
}
