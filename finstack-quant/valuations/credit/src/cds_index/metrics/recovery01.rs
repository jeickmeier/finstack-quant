//! Recovery01 calculator for CDS Index.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) absolute change in recovery rate.
//!
//! ## Hazard Curve Recalibration
//!
//! Mirrors the single-name CDS Recovery01 methodology: when a hazard curve
//! carries the par-spread quotes it was bootstrapped from
//! (`par_spread_points` non-empty), the bumped recovery is propagated through
//! a full re-bootstrap of the survival curve so the observed spreads remain
//! consistent. This captures the indirect `h ≈ S/(1-R)` effect that dominates
//! the recovery sensitivity.
//!
//! When a curve has no stored par spreads (e.g. a hand-built knot curve), the
//! calculator falls back to a "frozen-curve" bump: the recovery is bumped on
//! the instrument and the curve's recovery metadata is realigned, but the λ
//! knots are reused unchanged. That partial sensitivity typically understates
//! the true value by 2-5x for spread-bootstrapped curves.

use crate::cds::metrics::market_doc_clause;
use crate::cds_index::{CdsIndex, IndexPricing};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use finstack_quant_valuations_core::recalibration::{
    HazardRecalibrationAction, HazardRecalibrationRequest,
};

/// Standard recovery rate bump: 1% (0.01)
const RECOVERY_BUMP: f64 = 0.01;

/// Replace `curve_id` in `market` with the same hazard curve re-expressed at
/// `new_recovery`: a full par-spread re-bootstrap when the curve carries its
/// calibration quotes, a frozen-curve recovery realignment otherwise.
fn market_with_recovery(
    context: &MetricContext,
    index: &CdsIndex,
    market: &MarketContext,
    curve_id: &str,
    new_recovery: f64,
) -> Result<MarketContext> {
    let hazard = market.get_hazard(curve_id)?;
    let discount_id = index.premium_leg.discount_curve_id.clone();
    let synthetic = index.to_synthetic_cds()?;

    let frozen_curve_market = || -> Result<MarketContext> {
        Ok(market
            .clone()
            .insert(hazard.with_recovery_rate(new_recovery)?))
    };

    if hazard.hazard_calibration().is_some() {
        let recalibrated = context
            .rebuild_hazard_curve(
                HazardRecalibrationRequest {
                    hazard,
                    source_market: std::sync::Arc::new(market.clone()),
                    target_market: std::sync::Arc::new(market.clone()),
                    discount_curve_id: discount_id,
                    doc_clause: Some(market_doc_clause(&synthetic)),
                    cds_valuation_convention: Some(synthetic.valuation_convention),
                    deal_quote_override: None,
                    action: HazardRecalibrationAction::RecoveryRateReplay {
                        recovery_rate: new_recovery,
                    },
                },
                "recovery01",
            )
        .map_err(|e| finstack_quant_core::Error::Calibration {
            message: format!(
                "CDS index Recovery01: recovery re-bootstrap failed for curve '{curve_id}' ({e}); refusing silent frozen-curve fallback"
            ),
            category: "recovery01_rebootstrap".to_string(),
        })?;
        Ok(market.clone().insert(recalibrated.as_ref().clone()))
    } else {
        frozen_curve_market()
    }
}

/// Recovery01 calculator for CDS Index.
pub(crate) struct Recovery01Calculator;

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let index: &CdsIndex = context.instrument_as()?;
        let as_of = context.as_of;

        // Recovery the bumped index prices with: the index leg in
        // `SingleCurve` mode, the surviving constituents otherwise. Defaulted
        // names are settled and outside both PV and market dependencies.
        let by_constituent = index.pricing == IndexPricing::Constituents;
        let bump = |idx: &CdsIndex, delta: f64| -> CdsIndex {
            let mut bumped = idx.clone();
            if by_constituent {
                for con in bumped.constituents.iter_mut().filter(|c| !c.defaulted) {
                    let base = con.credit.recovery_rate;
                    con.credit.recovery_rate = (base + delta).clamp(0.0, 1.0);
                }
            } else {
                let base = bumped.protection_leg.recovery_rate;
                bumped.protection_leg.recovery_rate = (base + delta).clamp(0.0, 1.0);
            }
            bumped
        };

        let effective_delta = |idx: &CdsIndex, delta: f64| -> f64 {
            if by_constituent {
                let deltas: Vec<f64> = idx
                    .constituents
                    .iter()
                    .filter(|con| !con.defaulted)
                    .map(|con| {
                        let base = con.credit.recovery_rate;
                        (base + delta).clamp(0.0, 1.0) - base
                    })
                    .collect();
                if deltas.is_empty() {
                    0.0
                } else {
                    deltas.iter().sum::<f64>() / deltas.len() as f64
                }
            } else {
                let base = idx.protection_leg.recovery_rate;
                (base + delta).clamp(0.0, 1.0) - base
            }
        };

        // Re-express every hazard curve the bumped index will price against
        // at its bumped recovery (re-bootstrap when par quotes are stored).
        let market_for = |bumped: &CdsIndex| -> Result<MarketContext> {
            let mut market = context.curves.as_ref().clone();
            if by_constituent {
                for con in bumped.constituents.iter().filter(|c| !c.defaulted) {
                    market = market_with_recovery(
                        context,
                        bumped,
                        &market,
                        con.credit.credit_curve_id.as_str(),
                        con.credit.recovery_rate,
                    )?;
                }
            } else {
                market = market_with_recovery(
                    context,
                    bumped,
                    &market,
                    bumped.protection_leg.credit_curve_id.as_str(),
                    bumped.protection_leg.recovery_rate,
                )?;
            }
            Ok(market)
        };

        let up_delta = effective_delta(index, RECOVERY_BUMP);
        let down_delta = -effective_delta(index, -RECOVERY_BUMP);

        let index_up = bump(index, RECOVERY_BUMP);
        let pv_up = index_up.value(&market_for(&index_up)?, as_of)?.amount();

        let index_down = bump(index, -RECOVERY_BUMP);
        let pv_down = index_down.value(&market_for(&index_down)?, as_of)?.amount();

        let span = up_delta + down_delta;
        if span <= 0.0 {
            return Ok(0.0);
        }
        let recovery01 = (pv_up - pv_down) / span * RECOVERY_BUMP;

        Ok(recovery01)
    }
}
