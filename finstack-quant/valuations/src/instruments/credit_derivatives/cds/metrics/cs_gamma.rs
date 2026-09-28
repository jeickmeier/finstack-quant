//! CDS CS-Gamma metric calculator.
//!
//! Calculates the second derivative of the CDS value with respect to parallel
//! credit spread shifts. CS-Gamma measures how CS01 changes as spreads move.
//!
//! ## Methodology
//!
//! CS-Gamma is computed as the central second finite difference over **par-spread
//! re-bootstrapped** hazard curves — using the exact same infrastructure as the
//! CS01 calculator:
//!
//! ```text
//! CS-Gamma ≈ (PV(s+Δ) - 2·PV(s) + PV(s-Δ)) / Δ²
//! ```
//!
//! where each `PV(s±Δ)` and `PV(s)` is computed after **re-bootstrapping the
//! hazard curve from the same spread-risk quote set**, shifted by ±Δ or left
//! at its quote-space center, under the CDS's doc clause and valuation
//! convention.
//!
//! CS01 is reported in currency per basis point, while CS-Gamma is reported in
//! currency per decimal-spread squared. For a move of `Δb` basis points, with
//! `Δs = Δb / 10_000`, the consistent Taylor approximation is:
//!
//! ```text
//! ΔPV ≈ CS01 × Δb + ½ × CS-Gamma × Δs²
//! ```
//!
//! The bump size `Δ` is read from the same `credit_spread_bump_bp` config field
//! as CS01 (default 1bp). A smaller bump reduces the Taylor approximation error
//! but amplifies floating-point noise in the second difference; 1bp is the
//! workspace standard and gives acceptable noise on $10M notional CDS.
//!
//! ## Consistency with CS01
//!
//! An equivalent identity (useful for testing) is:
//!
//! ```text
//! CS-Gamma ≈ 10_000 × (CS01(s+Δs) - CS01(s-Δs)) / (2Δs)
//! ```
//!
//! The factor `10_000` converts the per-basis-point CS01 output to the
//! per-decimal first derivative before differentiating again.
//! Under the old hazard-rate bump implementation this identity failed because
//! the two metrics bumped different objects; this implementation satisfies it.
//!
//! ## `hazard_with_deal_quote` handling
//!
//! When a CDS carries a `cds_quote_bp` market-quote override, the hazard curve
//! retains its full replay recipe while the exactly matching contractual
//! spread-risk pillar is replaced. The up, down, and center replays therefore
//! use the same overridden quote set as standard CS01.

use crate::constants::BASIS_POINTS_PER_UNIT;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::with_prepared_cds_risk_context;
use crate::metrics::{MetricCalculator, MetricContext};
use crate::recalibration::{HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump};

/// Calculates CS-Gamma for credit default swaps.
///
/// CS-Gamma is the second derivative of PV w.r.t. a parallel par-spread shift,
/// consistent with CS01 = first derivative. Both use par-spread re-bootstrapping.
pub(crate) struct CsGammaCalculator;

impl MetricCalculator for CsGammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let as_of = context.as_of;

        // Read bump size from config — same field as CS01 so the Taylor
        // expansion ΔPV ≈ CS01·Δb + ½·CS-Gamma·Δs² uses consistent shocks.
        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_pricing_overrides(),
        )?
        .credit_spread_bump_bp;

        if bump_bp.abs() <= 1e-10 {
            return Ok(0.0);
        }

        with_prepared_cds_risk_context::<CreditDefaultSwap>(
            context,
            Some(0.0),
            "CDS CS-Gamma",
            |context, prepared| {
                let base_ctx = context.curves.as_ref();
                let hazard = base_ctx.get_hazard(prepared.hazard_id.as_str())?;
                let hazard_ref = hazard.as_ref();
                crate::metrics::sensitivities::cs01::require_hazard_replay(
                    hazard_ref,
                    "CDS CS-Gamma",
                )?;

                let rebuild = |action: HazardRecalibrationAction| {
                    context.rebuild_hazard_curve(
                        HazardRecalibrationRequest {
                            hazard: std::sync::Arc::clone(&hazard),
                            source_market: std::sync::Arc::clone(&context.curves),
                            target_market: std::sync::Arc::clone(&context.curves),
                            discount_curve_id: prepared.discount_id.clone(),
                            doc_clause: Some(prepared.doc_clause),
                            cds_valuation_convention: Some(prepared.valuation_convention),
                            deal_quote_override: prepared.deal_quote_override,
                            action,
                        },
                        "cs_gamma",
                    )
                };
                let bumped_hazard_up = rebuild(HazardRecalibrationAction::SpreadBump(
                    QuoteBump::ParallelBp(bump_bp),
                ))?;
                let bumped_hazard_dn = rebuild(HazardRecalibrationAction::SpreadBump(
                    QuoteBump::ParallelBp(-bump_bp),
                ))?;
                let bumped_hazard_0 = rebuild(HazardRecalibrationAction::SpreadRiskCenterReplay)?;

                let (pv_up, pv_0, pv_dn) = context.with_market_scratch(|ctx, scratch| {
                    // PV at s + Δ
                    scratch.insert_mut(bumped_hazard_up);
                    let pv_up = ctx.reprice_raw(scratch, as_of)?;
                    scratch.insert_mut(std::sync::Arc::clone(&hazard));

                    // PV at s (re-bootstrapped base, so the base-effect is zero)
                    scratch.insert_mut(bumped_hazard_0);
                    let pv_0 = ctx.reprice_raw(scratch, as_of)?;
                    scratch.insert_mut(std::sync::Arc::clone(&hazard));

                    // PV at s - Δ
                    scratch.insert_mut(bumped_hazard_dn);
                    let pv_dn = ctx.reprice_raw(scratch, as_of)?;
                    scratch.insert_mut(std::sync::Arc::clone(&hazard));

                    Ok((pv_up, pv_0, pv_dn))
                })?;

                // Central second difference, normalised to per (decimal spread)²
                // (dividing by the DECIMAL bump squared — 1bp bump ⇒ divisor 1e-8).
                // CS-Gamma = (PV(s+Δ) + PV(s-Δ) - 2·PV(s)) / Δ²
                // where Δ is in decimal (1bp = 0.0001).
                let bump_decimal = bump_bp / BASIS_POINTS_PER_UNIT;
                let cs_gamma = (pv_up + pv_dn - 2.0 * pv_0) / (bump_decimal * bump_decimal);
                Ok(cs_gamma)
            },
        )
    }
}
