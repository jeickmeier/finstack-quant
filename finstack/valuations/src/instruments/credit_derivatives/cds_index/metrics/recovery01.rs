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

use crate::calibration::bumps::hazard::recalibrate_hazard_with_recovery_and_doc_clause_and_valuation_convention;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds::metrics::market_doc_clause;
use crate::instruments::credit_derivatives::cds_index::CDSIndex;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::Result;

/// Standard recovery rate bump: 1% (0.01)
const RECOVERY_BUMP: f64 = 0.01;

/// Replace `curve_id` in `market` with the same hazard curve re-expressed at
/// `new_recovery`: a full par-spread re-bootstrap when the curve carries its
/// calibration quotes, a frozen-curve recovery realignment otherwise.
fn market_with_recovery(
    index: &CDSIndex,
    market: &MarketContext,
    curve_id: &str,
    new_recovery: f64,
) -> Result<MarketContext> {
    let hazard = market.get_hazard(curve_id)?;
    let discount_id = index.premium.discount_curve_id.clone();
    let synthetic = index.to_synthetic_cds();

    let frozen_curve_market = || -> Result<MarketContext> {
        Ok(market
            .clone()
            .insert(hazard.with_recovery_rate(new_recovery)?))
    };

    if hazard.par_spread_points().next().is_some() {
        match recalibrate_hazard_with_recovery_and_doc_clause_and_valuation_convention(
            hazard.as_ref(),
            new_recovery,
            market,
            Some(&discount_id),
            Some(market_doc_clause(&synthetic)),
            Some(synthetic.valuation_convention),
        ) {
            Ok(recalibrated) => Ok(market.clone().insert(recalibrated)),
            // Recalibration failure (e.g. degenerate spreads under the new
            // recovery) is non-fatal: fall back to the frozen-curve bump so
            // the metric still produces a number.
            Err(_) => frozen_curve_market(),
        }
    } else {
        frozen_curve_market()
    }
}

/// Recovery01 calculator for CDS Index.
pub(crate) struct Recovery01Calculator;

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let index: &CDSIndex = context.instrument_as()?;
        let as_of = context.as_of;

        let bump = |idx: &CDSIndex, delta: f64| -> CDSIndex {
            let mut bumped = idx.clone();
            if bumped.constituents.is_empty() {
                let base = bumped.protection.recovery_rate;
                bumped.protection.recovery_rate = (base + delta).clamp(0.0, 1.0);
            } else {
                for con in &mut bumped.constituents {
                    let base = con.credit.recovery_rate;
                    con.credit.recovery_rate = (base + delta).clamp(0.0, 1.0);
                }
            }
            bumped
        };

        let effective_delta = |idx: &CDSIndex, delta: f64| -> f64 {
            if idx.constituents.is_empty() {
                let base = idx.protection.recovery_rate;
                (base + delta).clamp(0.0, 1.0) - base
            } else {
                let sum: f64 = idx
                    .constituents
                    .iter()
                    .map(|con| {
                        let base = con.credit.recovery_rate;
                        (base + delta).clamp(0.0, 1.0) - base
                    })
                    .sum();
                sum / idx.constituents.len() as f64
            }
        };

        // Re-express every hazard curve the bumped index will price against
        // at its bumped recovery (re-bootstrap when par quotes are stored).
        let market_for = |bumped: &CDSIndex| -> Result<MarketContext> {
            let mut market = context.curves.as_ref().clone();
            if bumped.constituents.is_empty() {
                market = market_with_recovery(
                    bumped,
                    &market,
                    bumped.protection.credit_curve_id.as_str(),
                    bumped.protection.recovery_rate,
                )?;
            } else {
                for con in &bumped.constituents {
                    market = market_with_recovery(
                        bumped,
                        &market,
                        con.credit.credit_curve_id.as_str(),
                        con.credit.recovery_rate,
                    )?;
                }
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
