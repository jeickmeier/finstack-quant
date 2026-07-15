//! Interest rate convexity and cross-gamma calculators for interest rate swaps.
//!
//! # IR Convexity (Parallel Gamma)
//!
//! Second derivative of PV with respect to parallel rate curve shifts:
//! ```text
//! IR Convexity = d²PV / dr² ≈ (PV(+h) + PV(-h) - 2×PV_base) / h²
//! ```
//!
//! # Cross-Gamma (Discount vs Forward)
//!
//! Mixed second derivative measuring how DV01 changes when the other curve moves:
//! ```text
//! CrossGamma = d²PV / (dr_disc × dr_fwd)
//!            ≈ (PV(d+,f+) - PV(d+,f-) - PV(d-,f+) + PV(d-,f-)) / (4 × h_d × h_f)
//! ```
//!
//! # References
//!
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives*. Chapter 7.
//! - Tuckman, B., & Serrat, A. (2011). *Fixed Income Securities*. Chapter 5.

use crate::instruments::common_impl::dependencies::RatesCurveKind;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::InterestRateSwap;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::Result;

/// Parallel IR convexity (second-order rate sensitivity).
pub(crate) struct IrConvexityCalculator;

impl MetricCalculator for IrConvexityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let irs: &InterestRateSwap = context.instrument_as()?;
        let as_of = context.as_of;
        let bump_bp =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?
                .rate_bump_bp;

        let base_pv = irs.value_raw(context.curves.as_ref(), as_of)?;

        let deps = irs.market_dependencies()?.curves;
        let mut bumps_up: Vec<MarketBump> = Vec::new();
        let mut bumps_down: Vec<MarketBump> = Vec::new();

        for (curve_id, kind) in deps.all_with_kind() {
            let exists = match kind {
                RatesCurveKind::Discount => context.curves.get_discount(curve_id.as_str()).is_ok(),
                RatesCurveKind::Forward => context.curves.get_forward(curve_id.as_str()).is_ok(),
                RatesCurveKind::Credit | RatesCurveKind::Inflation => false,
            };
            if !exists {
                continue;
            }
            bumps_up.push(MarketBump::Curve {
                id: curve_id.clone(),
                spec: BumpSpec::parallel_bp(bump_bp),
            });
            bumps_down.push(MarketBump::Curve {
                id: curve_id.clone(),
                spec: BumpSpec::parallel_bp(-bump_bp),
            });
        }

        if bumps_up.is_empty() {
            return Ok(0.0);
        }

        // Single scratch clone, reused for up and down bumps via in-place bump + revert.
        let mut scratch = context.curves.as_ref().clone();
        let spec_up = BumpSpec::parallel_bp(bump_bp);
        let spec_down = BumpSpec::parallel_bp(-bump_bp);

        let mut tokens_up = Vec::with_capacity(bumps_up.len());
        for bump in &bumps_up {
            if let MarketBump::Curve { id, .. } = bump {
                tokens_up.push(scratch.apply_curve_bump_in_place(id, spec_up)?);
            }
        }
        let pv_up = irs.value_raw(&scratch, as_of)?;
        for token in tokens_up.into_iter().rev() {
            scratch.revert_scratch_bump(token)?;
        }

        let mut tokens_down = Vec::with_capacity(bumps_down.len());
        for bump in &bumps_down {
            if let MarketBump::Curve { id, .. } = bump {
                tokens_down.push(scratch.apply_curve_bump_in_place(id, spec_down)?);
            }
        }
        let pv_down = irs.value_raw(&scratch, as_of)?;
        for token in tokens_down.into_iter().rev() {
            scratch.revert_scratch_bump(token)?;
        }

        let h = bump_bp * 1e-4;
        let convexity = (pv_up + pv_down - 2.0 * base_pv) / (h * h);

        Ok(convexity)
    }
}

/// Cross-gamma between discount and forward curves.
///
/// Uses central mixed finite differences:
/// `d²PV/(dr_disc × dr_fwd) ≈ (PV(d+,f+) - PV(d+,f-) - PV(d-,f+) + PV(d-,f-)) / (4hk)`
///
/// Returns 0.0 when the swap is single-curve (discount == forward) since the
/// mixed derivative is not meaningful in that case.
pub(crate) struct CrossGammaCalculator;

impl MetricCalculator for CrossGammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let irs: &InterestRateSwap = context.instrument_as()?;
        let as_of = context.as_of;
        let bump_bp =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?
                .rate_bump_bp;

        if irs.fixed.discount_curve_id == irs.float.forward_curve_id {
            return Ok(0.0);
        }

        let disc_id = &irs.fixed.discount_curve_id;
        let fwd_id = &irs.float.forward_curve_id;

        let disc_exists = context.curves.get_discount(disc_id.as_str()).is_ok();
        let fwd_exists = context.curves.get_forward(fwd_id.as_str()).is_ok();
        if !disc_exists || !fwd_exists {
            return Ok(0.0);
        }

        let bump_disc_up = vec![MarketBump::Curve {
            id: disc_id.clone(),
            spec: BumpSpec::parallel_bp(bump_bp),
        }];
        let bump_disc_down = vec![MarketBump::Curve {
            id: disc_id.clone(),
            spec: BumpSpec::parallel_bp(-bump_bp),
        }];
        let bump_fwd_up = MarketBump::Curve {
            id: fwd_id.clone(),
            spec: BumpSpec::parallel_bp(bump_bp),
        };
        let bump_fwd_down = MarketBump::Curve {
            id: fwd_id.clone(),
            spec: BumpSpec::parallel_bp(-bump_bp),
        };

        let ctx_disc_up = context.curves.bump(bump_disc_up)?;
        let ctx_disc_down = context.curves.bump(bump_disc_down)?;

        let pv_du_fu = irs.value_raw(&ctx_disc_up.bump(vec![bump_fwd_up.clone()])?, as_of)?;
        let pv_du_fd = irs.value_raw(&ctx_disc_up.bump(vec![bump_fwd_down.clone()])?, as_of)?;
        let pv_dd_fu = irs.value_raw(&ctx_disc_down.bump(vec![bump_fwd_up])?, as_of)?;
        let pv_dd_fd = irs.value_raw(&ctx_disc_down.bump(vec![bump_fwd_down])?, as_of)?;

        let h = bump_bp * 1e-4;
        let cross_gamma = (pv_du_fu - pv_du_fd - pv_dd_fu + pv_dd_fd) / (4.0 * h * h);

        Ok(cross_gamma)
    }
}
