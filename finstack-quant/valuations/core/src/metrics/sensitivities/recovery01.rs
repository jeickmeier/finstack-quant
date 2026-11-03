//! Recovery-rate bump helpers shared by every `recovery01` calculator.
//!
//! `recovery01` is the PV change per +1% (0.01 absolute) recovery-rate move, in
//! currency, by finite differences:
//!
//! - **Central difference** (interior): `(PV(R+h) - PV(R-h)) / (2h)`
//! - **Forward difference** (near R=0): `(PV(R+h) - PV(R)) / h`
//! - **Backward difference** (near the upper bound): `(PV(R) - PV(R-h)) / h`
//!
//! Bumped recoveries are clamped to `[0, 0.999]`: recovery must stay below 1
//! (zero LGD makes a protection leg degenerate and fails validation).
//!
//! ## Hazard curve recalibration
//!
//! When the hazard curve carries the par-spread quotes it was bootstrapped
//! from, the bumped recovery goes through a full re-bootstrap of the survival
//! curve so the observed CDS spreads stay consistent. This captures the
//! indirect `h ≈ S/(1-R)` effect that dominates the recovery sensitivity of a
//! distressed credit. A curve with no stored recipe (a hand-built knot curve)
//! takes a frozen-curve bump: the hazard rates are unchanged and only the
//! recovery moves. The two can differ greatly. A CDS's frozen sensitivity
//! typically understates the recalibrated one by 2-5x; a par bond's overstates
//! it, because at fixed spreads the higher hazard rate offsets most of the gain.

use crate::market::conventions::CdsValuationConvention;
use crate::market::conventions::ids::CdsDocClause;
use crate::metrics::MetricContext;
use crate::recalibration::{
    DealCdsQuoteOverride, HazardRecalibrationAction, HazardRecalibrationRequest,
};
use finstack_quant_core::Result;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;
use std::sync::Arc;

/// Standard recovery rate bump: 1% (0.01).
pub const RECOVERY_BUMP: f64 = 0.01;

/// Highest recovery a bumped instrument may carry; recovery must stay below 1.
const MAX_BUMPED_RECOVERY: f64 = 0.999;

/// Minimum bump size considered valid for finite differences.
/// Below this threshold, we treat the bump as ineffective.
const MIN_EFFECTIVE_BUMP: f64 = 1e-6;

/// `base + delta` clamped to the bumpable recovery range `[0, 0.999]`.
///
/// # Arguments
///
/// * `base` - Recovery rate before the bump, as a decimal fraction of the claim.
/// * `delta` - Signed absolute change in recovery (`0.01` is one point).
pub fn bumped_recovery(base: f64, delta: f64) -> f64 {
    (base + delta).clamp(0.0, MAX_BUMPED_RECOVERY)
}

/// Recovery01 by finite differences on `pv_at(recovery)`: central inside the
/// bumpable range, one-sided against `base_pv` at a bound, scaled to the PV
/// change per [`RECOVERY_BUMP`].
///
/// # Arguments
///
/// * `base_recovery` - Recovery rate the instrument prices at, as a decimal.
/// * `base_pv` - Current PV in currency, the anchor of a one-sided difference.
/// * `pv_at` - Full reprice in currency at a bumped recovery rate.
///
/// # Returns
///
/// PV change in currency for a +0.01 recovery move; zero only when neither bump
/// is possible.
///
/// # Errors
///
/// Returns the first error from `pv_at`.
pub fn recovery01_fd(
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
pub struct RecoveryReplayTerms {
    /// Discount curve used by the re-bootstrap.
    pub discount_curve_id: CurveId,
    /// Deal doc clause asserted against the stored recipe.
    pub doc_clause: Option<CdsDocClause>,
    /// Deal valuation convention of the replayed quotes.
    pub cds_valuation_convention: Option<CdsValuationConvention>,
    /// Optional deal-level clean-spread override.
    pub deal_quote_override: Option<DealCdsQuoteOverride>,
}

impl RecoveryReplayTerms {
    /// Terms of a non-CDS instrument: the recipe's own clause and convention.
    ///
    /// # Arguments
    ///
    /// * `discount_curve_id` - Instrument discount curve the re-bootstrap uses.
    pub fn on_discount_curve(discount_curve_id: CurveId) -> Self {
        Self {
            discount_curve_id,
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
        }
    }
}

/// `market` with hazard curve `hazard_id` re-expressed at `recovery`: a full
/// par-spread re-bootstrap when the curve carries its calibration recipe, a
/// frozen-curve recovery realignment (λ knots unchanged) otherwise.
///
/// # Arguments
///
/// * `context` - Metric context holding the recalibration provider.
/// * `market` - Market to copy, holding the hazard curve.
/// * `hazard_id` - Hazard curve to re-express.
/// * `recovery` - New curve recovery rate, as a decimal in `[0, 1)`.
/// * `terms` - Deal terms of the re-bootstrap; unused for a frozen curve.
/// * `label` - Prefix of the re-bootstrap error message.
///
/// # Errors
///
/// Returns not-found when the curve is missing, and a calibration error when
/// the re-bootstrap fails. There is no silent fallback to a frozen curve.
pub fn market_with_recovery(
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
        // recovery. Without this a pricer's recovery-consistency guard rejects
        // the (trade, curve) pair; the λ knots are untouched so the priced PV
        // is exactly the documented LGD-only sensitivity.
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
