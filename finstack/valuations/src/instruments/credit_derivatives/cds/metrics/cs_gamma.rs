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
//! hazard curve from par-spread quotes shifted by ±Δ (or 0)**, under the
//! CDS's doc clause and valuation convention — exactly as CS01 does.
//!
//! This guarantees that CS-Gamma is the second derivative of the same PV function
//! as CS01 (the first derivative), making the two consistent for Taylor P&L:
//!
//! ```text
//! ΔPV ≈ CS01 × Δs + ½ × CS-Gamma × Δs²
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
//! CS-Gamma ≈ (CS01(s+Δ) - CS01(s-Δ)) / (2Δ)
//! ```
//!
//! i.e. CS-Gamma equals the numerical derivative of CS01 w.r.t. the spread.
//! Under the old hazard-rate bump implementation this identity failed because
//! the two metrics bumped different objects; this implementation satisfies it.
//!
//! ## `hazard_with_deal_quote` handling
//!
//! When a CDS carries a `cds_quote_bp` market-quote override, the hazard curve
//! is rebuilt from that single point before the bump grid is applied — identical
//! to the CS01 `cs01_curve_override` path.

use crate::calibration::bumps::hazard::{
    bump_hazard_shift, bump_hazard_spreads_with_doc_clause_and_valuation_convention,
};
use crate::calibration::bumps::BumpRequest;
use crate::constants::BASIS_POINTS_PER_UNIT;
use crate::instruments::common_impl::traits::CurveDependencies;
use crate::instruments::credit_derivatives::cds::CreditDefaultSwap;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;

/// Calculates CS-Gamma for credit default swaps.
///
/// CS-Gamma is the second derivative of PV w.r.t. a parallel par-spread shift,
/// consistent with CS01 = first derivative. Both use par-spread re-bootstrapping.
pub(crate) struct CsGammaCalculator;

impl MetricCalculator for CsGammaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let as_of = context.as_of;

        // Read bump size from config — same field as CS01 so the Taylor
        // expansion ΔPV ≈ CS01·Δs + ½·CS-Gamma·Δs² uses consistent Δ.
        let bump_bp =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?
                .credit_spread_bump_bp;

        if bump_bp.abs() <= 1e-10 {
            return Ok(0.0);
        }

        // Resolve curve IDs.
        let curve_deps = cds.curve_dependencies()?;
        let Some(hazard_id) = curve_deps.credit_curves.first().cloned() else {
            return Ok(0.0);
        };
        let discount_id = curve_deps.discount_curves.first().cloned();

        // Per-deal bootstrap convention (doc clause + valuation convention) —
        // mirrors CreditParallelCs01::calculate.
        let doc_clause = super::market_doc_clause(cds);
        let valuation_convention = cds.valuation_convention;

        // Apply hazard_with_deal_quote override — same as cs01_curve_override.
        let original_curves = Arc::clone(&context.curves);
        {
            let hazard = original_curves.get_hazard(hazard_id.as_str())?;
            if let Some(quote_hazard) =
                super::hazard_with_deal_quote(cds, hazard.as_ref())?
            {
                context.curves = Arc::new(original_curves.as_ref().clone().insert(quote_hazard));
            }
        }

        let result = (|| {
            let base_ctx = context.curves.as_ref();
            let hazard = base_ctx.get_hazard(hazard_id.as_str())?;
            let hazard_ref = hazard.as_ref();

            // Determine whether par-spread re-bootstrapping is available by
            // attempting the 0-shift bootstrap probe — exactly as
            // `compute_parallel_cs01_with_context_raw` does.  A weak check
            // (par-spread points exist) would allow a deal whose bootstrap
            // *fails* to silently fall back to a hazard-rate shift while CS01
            // surfaces a hard error — precisely the CS01/CS-Gamma inconsistency
            // this file was written to eliminate.
            let has_par_points = hazard_ref.par_spread_points().next().is_some();
            let used_rebootstrap = if discount_id.is_some() && has_par_points {
                bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                    hazard_ref,
                    base_ctx,
                    &BumpRequest::Parallel(0.0),
                    discount_id.as_ref(),
                    Some(doc_clause),
                    Some(valuation_convention),
                )
                .map_err(|e| finstack_core::Error::Calibration {
                    message: format!(
                        "CS-Gamma hazard curve re-calibration failed for '{}': {} \
                         (cannot compute CS-Gamma under market-standard par spread bump \
                         methodology)",
                        hazard_id.as_str(),
                        e
                    ),
                    category: "cs_gamma_rebootstrap".to_string(),
                })?;
                true
            } else {
                false
            };

            // Helper: build the bumped hazard curve for a given shift.
            let make_bumped = |shift_bp: f64| -> finstack_core::Result<_> {
                let req = BumpRequest::Parallel(shift_bp);
                if used_rebootstrap {
                    bump_hazard_spreads_with_doc_clause_and_valuation_convention(
                        hazard_ref,
                        base_ctx,
                        &req,
                        discount_id.as_ref(),
                        Some(doc_clause),
                        Some(valuation_convention),
                    )
                } else {
                    bump_hazard_shift(hazard_ref, &req)
                }
            };

            let bumped_hazard_up = make_bumped(bump_bp)?;
            let bumped_hazard_dn = make_bumped(-bump_bp)?;
            let bumped_hazard_0 = make_bumped(0.0)?;

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

            // Central second difference, normalised to per (basis point)².
            // CS-Gamma = (PV(s+Δ) + PV(s-Δ) - 2·PV(s)) / Δ²
            // where Δ is in decimal (1bp = 0.0001).
            let bump_decimal = bump_bp / BASIS_POINTS_PER_UNIT;
            let cs_gamma = (pv_up + pv_dn - 2.0 * pv_0) / (bump_decimal * bump_decimal);
            Ok(cs_gamma)
        })();

        context.curves = original_curves;
        result
    }
}
