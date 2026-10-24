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
use crate::cds::metrics::recovery01::{
    bumped_recovery, market_with_recovery, RecoveryReplayTerms, RECOVERY_BUMP,
};
use crate::cds_index::{CdsIndex, IndexPricing};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;

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
        let synthetic = index.to_synthetic_cds()?;
        let terms = RecoveryReplayTerms {
            discount_curve_id: index.premium_leg.discount_curve_id.clone(),
            doc_clause: Some(market_doc_clause(&synthetic)),
            cds_valuation_convention: Some(synthetic.valuation_convention),
            deal_quote_override: None,
        };
        let bump = |idx: &CdsIndex, delta: f64| -> CdsIndex {
            let mut bumped = idx.clone();
            if by_constituent {
                for con in bumped.constituents.iter_mut().filter(|c| !c.defaulted) {
                    let base = con.credit.recovery_rate;
                    con.credit.recovery_rate = bumped_recovery(base, delta);
                }
            } else {
                let base = bumped.protection_leg.recovery_rate;
                bumped.protection_leg.recovery_rate = bumped_recovery(base, delta);
            }
            bumped
        };

        let effective_delta = |idx: &CdsIndex, delta: f64| -> f64 {
            if by_constituent {
                let mut weighted_delta = 0.0;
                let mut total_weight = 0.0;
                for con in idx.constituents.iter().filter(|con| !con.defaulted) {
                    let base = con.credit.recovery_rate;
                    weighted_delta += con.weight * (bumped_recovery(base, delta) - base);
                    total_weight += con.weight;
                }
                if total_weight <= 0.0 {
                    0.0
                } else {
                    weighted_delta / total_weight
                }
            } else {
                let base = idx.protection_leg.recovery_rate;
                bumped_recovery(base, delta) - base
            }
        };

        // Re-express every hazard curve the bumped index will price against
        // at its bumped recovery (re-bootstrap when par quotes are stored).
        let market_for = |bumped: &CdsIndex| -> Result<Arc<MarketContext>> {
            let mut market = Arc::clone(&context.curves);
            let mut rebuild = |curve_id: &str, recovery: f64| -> Result<()> {
                market = Arc::new(market_with_recovery(
                    context,
                    &market,
                    curve_id,
                    recovery,
                    &terms,
                    "CDS index Recovery01",
                )?);
                Ok(())
            };
            if by_constituent {
                for con in bumped.constituents.iter().filter(|c| !c.defaulted) {
                    rebuild(
                        con.credit.credit_curve_id.as_str(),
                        con.credit.recovery_rate,
                    )?;
                }
            } else {
                rebuild(
                    bumped.protection_leg.credit_curve_id.as_str(),
                    bumped.protection_leg.recovery_rate,
                )?;
            }
            Ok(market)
        };

        let up_delta = effective_delta(index, RECOVERY_BUMP);
        let down_delta = -effective_delta(index, -RECOVERY_BUMP);

        let index_up = bump(index, RECOVERY_BUMP);
        let pv_up = index_up
            .value(market_for(&index_up)?.as_ref(), as_of)?
            .amount();

        let index_down = bump(index, -RECOVERY_BUMP);
        let pv_down = index_down
            .value(market_for(&index_down)?.as_ref(), as_of)?
            .amount();

        let span = up_delta + down_delta;
        if span <= 0.0 {
            return Ok(0.0);
        }
        let recovery01 = (pv_up - pv_down) / span * RECOVERY_BUMP;

        Ok(recovery01)
    }
}
