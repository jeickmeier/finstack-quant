//! Recovery01 calculator for CDS, and the recovery-bump helpers shared by the
//! CDS, CDS-option, CDS-index and CDS-tranche Recovery01 calculators.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) change in recovery rate.
//!
//! ## Methodology
//!
//! Uses central differences when possible, with automatic fallback to one-sided
//! differences at recovery rate boundaries:
//!
//! - **Central difference** (interior): `(PV(R+h) - PV(R-h)) / (2h)`
//! - **Forward difference** (near R=0): `(PV(R+h) - PV(R)) / h`
//! - **Backward difference** (near the upper bound): `(PV(R) - PV(R-h)) / h`
//!
//! Bumped recoveries are clamped to `[0, 0.999]`: recovery must stay below 1
//! (zero LGD makes the protection leg degenerate and fails validation).
//!
//! ## Hazard Curve Recalibration
//!
//! When the hazard curve carries the par-spread quotes it was bootstrapped
//! from (`par_spread_points` non-empty), the bumped recovery is propagated
//! through a full re-bootstrap of the survival curve so the observed CDS
//! spreads remain consistent. This captures the indirect `h ≈ S/(1-R)` effect
//! that dominates the recovery sensitivity for distressed credits.
//!
//! When the curve has no stored par spreads (e.g. a hand-built knot curve
//! used in tests or a curve loaded without preserving its calibration
//! quotes), the calculator falls back to a "frozen-curve" bump: the recovery
//! is bumped on the instrument only and the survival curve is reused
//! unchanged. This produces a *partial* recovery sensitivity that, for
//! spread-bootstrapped curves, typically understates the true value by 2-5x.
//!
//! ## Note
//!
//! Recovery rate changes affect both the protection leg (LGD = 1 - recovery)
//! and the premium leg (accrued on default settlement). This metric captures
//! the full direct sensitivity across both legs.

use super::{deal_quote_override, market_doc_clause};
use crate::cds::CreditDefaultSwap;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations_core::market::conventions::CdsValuationConvention;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use finstack_quant_valuations_core::recalibration::{
    DealCdsQuoteOverride, HazardRecalibrationAction, HazardRecalibrationRequest,
};
use std::sync::Arc;

/// Standard recovery rate bump: 1% (0.01)
pub(crate) const RECOVERY_BUMP: f64 = 0.01;

/// Highest recovery a bumped instrument may carry; recovery must stay below 1.
const MAX_BUMPED_RECOVERY: f64 = 0.999;

/// Minimum bump size considered valid for finite differences.
/// Below this threshold, we treat the bump as ineffective.
const MIN_EFFECTIVE_BUMP: f64 = 1e-6;

/// `base + delta` clamped to the bumpable recovery range `[0, 0.999]`.
pub(crate) fn bumped_recovery(base: f64, delta: f64) -> f64 {
    (base + delta).clamp(0.0, MAX_BUMPED_RECOVERY)
}

/// Recovery01 by finite differences on `pv_at(recovery)`: central inside the
/// bumpable range, one-sided against `base_pv` at a bound, scaled to the PV
/// change per [`RECOVERY_BUMP`].
pub(crate) fn recovery01_fd(
    base_recovery: f64,
    base_pv: f64,
    mut pv_at: impl FnMut(f64) -> Result<f64>,
) -> Result<f64> {
    let bumped_up = bumped_recovery(base_recovery, RECOVERY_BUMP);
    let bumped_down = bumped_recovery(base_recovery, -RECOVERY_BUMP);
    let up_delta = bumped_up - base_recovery;
    let down_delta = base_recovery - bumped_down;

    let can_bump_up = up_delta > MIN_EFFECTIVE_BUMP;
    let can_bump_down = down_delta > MIN_EFFECTIVE_BUMP;

    let slope = match (can_bump_up, can_bump_down) {
        (true, true) => {
            let pv_up = pv_at(bumped_up)?;
            let pv_down = pv_at(bumped_down)?;
            (pv_up - pv_down) / (up_delta + down_delta)
        }
        (true, false) => (pv_at(bumped_up)? - base_pv) / up_delta,
        (false, true) => (base_pv - pv_at(bumped_down)?) / down_delta,
        (false, false) => 0.0,
    };

    Ok(slope * RECOVERY_BUMP)
}

/// Deal terms of the hazard re-bootstrap at a bumped recovery.
pub(crate) struct RecoveryReplayTerms {
    /// Discount curve used by the re-bootstrap.
    pub(crate) discount_curve_id: CurveId,
    /// Deal doc clause asserted against the stored recipe.
    pub(crate) doc_clause: Option<CdsDocClause>,
    /// Deal valuation convention of the replayed quotes.
    pub(crate) cds_valuation_convention: Option<CdsValuationConvention>,
    /// Optional deal-level clean-spread override.
    pub(crate) deal_quote_override: Option<DealCdsQuoteOverride>,
}

/// `market` with hazard curve `hazard_id` re-expressed at `recovery`: a full
/// par-spread re-bootstrap when the curve carries its calibration recipe, a
/// frozen-curve recovery realignment (λ knots unchanged) otherwise. `label`
/// prefixes the re-bootstrap error.
pub(crate) fn market_with_recovery(
    context: &MetricContext,
    market: &Arc<MarketContext>,
    hazard_id: &str,
    recovery: f64,
    terms: &RecoveryReplayTerms,
    label: &str,
) -> Result<MarketContext> {
    let hazard = market.get_hazard(hazard_id)?;
    if hazard.hazard_calibration().is_none() {
        // Frozen-curve fallback: reuse the hazard curve's λ knots unchanged
        // but realign its `recovery_rate` metadata with the bumped trade
        // recovery. Without this the recovery-consistency guard in the
        // pricer rejects the (trade, curve) pair; the λ knots are untouched
        // so the priced PV is exactly the documented LGD-only sensitivity.
        let frozen = hazard.with_recovery_rate(recovery)?;
        return Ok(market.as_ref().clone().insert(frozen));
    }
    let recalibrated = context
        .rebuild_hazard_curve(
            HazardRecalibrationRequest {
                hazard,
                source_market: Arc::clone(market),
                target_market: Arc::clone(market),
                discount_curve_id: terms.discount_curve_id.clone(),
                doc_clause: terms.doc_clause,
                cds_valuation_convention: terms.cds_valuation_convention,
                deal_quote_override: terms.deal_quote_override,
                action: HazardRecalibrationAction::RecoveryRateReplay {
                    recovery_rate: recovery,
                },
            },
            "recovery01",
        )
        .map_err(|e| finstack_quant_core::Error::Calibration {
            message: format!(
                "{label}: recovery re-bootstrap failed for curve '{hazard_id}' ({e}); refusing silent frozen-curve fallback"
            ),
            category: "recovery01_rebootstrap".to_string(),
        })?;
    Ok(market
        .as_ref()
        .clone()
        .insert(recalibrated.as_ref().clone()))
}

/// Recovery01 calculator for CDS.
pub(crate) struct Recovery01Calculator;

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: &CreditDefaultSwap = context.instrument_as()?;
        let as_of = context.as_of;
        let terms = RecoveryReplayTerms {
            discount_curve_id: cds.premium_leg.discount_curve_id.clone(),
            doc_clause: Some(market_doc_clause(cds)),
            cds_valuation_convention: Some(cds.valuation_convention),
            deal_quote_override: deal_quote_override(cds),
        };
        recovery01_fd(
            cds.protection_leg.recovery_rate,
            context.base_value.amount(),
            |recovery| {
                let mut bumped_cds = cds.clone();
                bumped_cds.protection_leg.recovery_rate = recovery;
                let market = market_with_recovery(
                    context,
                    &context.curves,
                    cds.protection_leg.credit_curve_id.as_str(),
                    recovery,
                    &terms,
                    "CDS Recovery01",
                )?;
                Ok(bumped_cds.value(&market, as_of)?.amount())
            },
        )
    }
}
