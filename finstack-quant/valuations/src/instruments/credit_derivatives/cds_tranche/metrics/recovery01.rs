//! Recovery01 calculator for CDS Tranche.
//!
//! Computes Recovery01 (recovery rate sensitivity) using finite differences.
//! Recovery01 measures the change in PV for a 1% (100bp) change in recovery rate.
//!
//! Note: Recovery rate is stored in the credit index, so we bump the recovery
//! rate inside the `CreditIndexData` while preserving any per-issuer recovery
//! and weight overrides (which represent independent quotes that should not
//! move with an index-level recovery shock).
//!
//! ## Hazard Curve Recalibration
//!
//! When the index hazard curve carries the par-spread quotes it was
//! bootstrapped from (`par_spread_points` non-empty), the bumped recovery
//! is propagated through a full re-bootstrap of the index survival curve
//! so the observed CDS index spreads remain consistent. This captures the
//! indirect `h ≈ S/(1-R)` effect that dominates the recovery sensitivity for
//! distressed indices.
//!
//! When the curve has no stored par spreads (e.g. a hand-built knot curve
//! used in tests), the calculator falls back to a frozen-curve bump: only
//! the index-level recovery rate is updated and the survival curve is
//! reused unchanged. This produces a *partial* recovery sensitivity that,
//! for spread-bootstrapped curves, typically understates the true value
//! by 2-5x.
//!
//! Per-issuer hazard curves with distinct curve IDs remain frozen because
//! they represent independent single-name quotes. Issuers referencing the
//! index hazard's curve ID share its recalibrated curve, consistent with the
//! market context's single canonical curve for each ID. Per-issuer recovery
//! agreements remain unchanged.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds_tranche::CdsTranche;
use crate::metrics::{MetricCalculator, MetricContext};
use crate::recalibration::{HazardRecalibrationAction, HazardRecalibrationRequest};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::Result;
use std::sync::Arc;

/// Standard recovery rate bump: 1% (0.01)
const RECOVERY_BUMP: f64 = 0.01;

/// Recovery01 calculator for CDS Tranche.
pub(crate) struct Recovery01Calculator;

/// Build a copy of `original` with `recovery_rate` replaced, preserving all
/// per-issuer overrides (curves, recovery rates, weights). Mirrors the
/// pricer's own rebuild logic so that heterogeneous bespoke tranches are
/// not silently downgraded to the homogeneous binomial path during bumping.
fn rebuild_with_recovery(original: &CreditIndexData, new_recovery: f64) -> Result<CreditIndexData> {
    let mut builder = CreditIndexData::builder()
        .num_constituents(original.num_constituents)
        .recovery_rate(new_recovery)
        .index_credit_curve(std::sync::Arc::clone(&original.index_credit_curve))
        .base_correlation_curve(std::sync::Arc::clone(&original.base_correlation_curve));

    if let Some(curves) = &original.issuer_credit_curves {
        builder = builder.issuer_curves(curves.clone());
    }
    if let Some(rates) = &original.issuer_recovery_rates {
        builder = builder.issuer_recovery_rates(rates.clone());
    }
    if let Some(weights) = &original.issuer_weights {
        builder = builder.issuer_weights(weights.clone());
    }

    builder.build()
}

/// Build a market with the index recovery bumped and the index hazard curve
/// **frozen** (reused unchanged).
fn frozen_market_at_bumped_recovery(
    base_market: &MarketContext,
    tranche: &CdsTranche,
    original_index: &CreditIndexData,
    new_recovery: f64,
) -> Result<MarketContext> {
    let bumped_index = rebuild_with_recovery(original_index, new_recovery)?;
    base_market
        .clone()
        .insert_credit_index(&tranche.credit_index_id, bumped_index)
}

/// Build a market with both the index recovery bumped and the index hazard
/// curve re-bootstrapped from par spreads at the new recovery.
fn recalibrated_market_at_bumped_recovery(
    context: &MetricContext,
    base_market: &MarketContext,
    tranche: &CdsTranche,
    original_index: &CreditIndexData,
    new_recovery: f64,
) -> Result<MarketContext> {
    let mut bumped_index = rebuild_with_recovery(original_index, new_recovery)?;
    let recalibrated = context
        .rebuild_hazard_curve(
            HazardRecalibrationRequest {
                hazard: Arc::clone(&original_index.index_credit_curve),
                source_market: Arc::new(base_market.clone()),
                target_market: Arc::new(base_market.clone()),
                discount_curve_id: tranche.discount_curve_id.clone(),
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
                action: HazardRecalibrationAction::RecoveryRateReplay {
                    recovery_rate: new_recovery,
                },
            },
            "recovery01",
        )
        .map_err(|error| finstack_quant_core::Error::Calibration {
            message: format!(
                "CDS tranche '{}' Recovery01 failed to re-bootstrap index hazard curve '{}' \
             at recovery {new_recovery}: {error}",
                tranche.id,
                original_index.index_credit_curve.id()
            ),
            category: "recovery01_rebootstrap".to_string(),
        })?;
    bumped_index.index_credit_curve = Arc::clone(&recalibrated);
    base_market
        .clone()
        .insert(recalibrated)
        .insert_credit_index(&tranche.credit_index_id, bumped_index)
}

impl MetricCalculator for Recovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let tranche: &CdsTranche = context.instrument_as()?;
        let as_of = context.as_of;
        let market = context.curves.as_ref();

        let original_index = market.get_credit_index(&tranche.credit_index_id)?;
        let base_recovery = original_index.recovery_rate;

        let bumped_recovery_up = (base_recovery + RECOVERY_BUMP).clamp(0.0, 1.0);
        let up_delta = bumped_recovery_up - base_recovery;
        let bumped_recovery_down = (base_recovery - RECOVERY_BUMP).clamp(0.0, 1.0);
        let down_delta = base_recovery - bumped_recovery_down;

        let has_par_quotes = original_index
            .index_credit_curve
            .hazard_calibration()
            .is_some();

        let (curves_up, curves_down) = if has_par_quotes {
            (
                recalibrated_market_at_bumped_recovery(
                    context,
                    market,
                    tranche,
                    original_index.as_ref(),
                    bumped_recovery_up,
                )?,
                recalibrated_market_at_bumped_recovery(
                    context,
                    market,
                    tranche,
                    original_index.as_ref(),
                    bumped_recovery_down,
                )?,
            )
        } else {
            (
                frozen_market_at_bumped_recovery(
                    market,
                    tranche,
                    original_index.as_ref(),
                    bumped_recovery_up,
                )?,
                frozen_market_at_bumped_recovery(
                    market,
                    tranche,
                    original_index.as_ref(),
                    bumped_recovery_down,
                )?,
            )
        };

        let pv_up = tranche.value(&curves_up, as_of)?.amount();
        let pv_down = tranche.value(&curves_down, as_of)?.amount();

        let span = up_delta + down_delta;
        if span <= 0.0 {
            return Ok(0.0);
        }
        Ok((pv_up - pv_down) / span * RECOVERY_BUMP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::market_data::term_structures::{BaseCorrelationCurve, HazardCurve};
    use std::sync::Arc;

    fn sample_index(
        with_per_issuer_recovery: bool,
        with_per_issuer_weight: bool,
    ) -> CreditIndexData {
        let base = time::macros::date!(2024 - 01 - 01);
        let hz = Arc::new(
            HazardCurve::builder("HZ-IDX")
                .base_date(base)
                .recovery_rate(0.40)
                .knots([(1.0, 0.02), (5.0, 0.025)])
                .par_spreads([(1.0, 120.0), (5.0, 150.0)])
                .build()
                .expect("hazard curve"),
        );
        let bc = Arc::new(
            BaseCorrelationCurve::builder("BC-IDX")
                .knots([(3.0, 0.30), (7.0, 0.35), (15.0, 0.45)])
                .build()
                .expect("base correlation"),
        );

        let mut issuer_curves = finstack_quant_core::HashMap::default();
        for i in 0..3 {
            let id = format!("ISS-{i}");
            issuer_curves.insert(id, Arc::clone(&hz));
        }

        let mut builder = CreditIndexData::builder()
            .num_constituents(3)
            .recovery_rate(0.40)
            .index_credit_curve(Arc::clone(&hz))
            .base_correlation_curve(Arc::clone(&bc))
            .issuer_curves(issuer_curves);

        if with_per_issuer_recovery {
            let mut rates = finstack_quant_core::HashMap::default();
            rates.insert("ISS-0".to_string(), 0.10);
            rates.insert("ISS-1".to_string(), 0.50);
            rates.insert("ISS-2".to_string(), 0.70);
            builder = builder.issuer_recovery_rates(rates);
        }

        if with_per_issuer_weight {
            let mut weights = finstack_quant_core::HashMap::default();
            weights.insert("ISS-0".to_string(), 0.50);
            weights.insert("ISS-1".to_string(), 0.30);
            weights.insert("ISS-2".to_string(), 0.20);
            builder = builder.issuer_weights(weights);
        }

        builder.build().expect("credit index")
    }

    #[test]
    fn rebuild_with_recovery_preserves_per_issuer_recovery_rates() {
        let original = sample_index(true, false);
        let bumped = rebuild_with_recovery(&original, 0.41).expect("rebuild");

        assert!((bumped.recovery_rate - 0.41).abs() < 1e-12);

        // Per-issuer recovery overrides survive intact (this is the regression).
        assert!(
            bumped.issuer_recovery_rates.is_some(),
            "issuer_recovery_rates dropped"
        );
        let rates = bumped
            .issuer_recovery_rates
            .as_ref()
            .expect("rates present");
        assert!((rates.get("ISS-0").copied().unwrap_or(0.0) - 0.10).abs() < 1e-12);
        assert!((rates.get("ISS-1").copied().unwrap_or(0.0) - 0.50).abs() < 1e-12);
        assert!((rates.get("ISS-2").copied().unwrap_or(0.0) - 0.70).abs() < 1e-12);
    }

    #[test]
    fn rebuild_with_recovery_preserves_per_issuer_weights() {
        let original = sample_index(false, true);
        let bumped = rebuild_with_recovery(&original, 0.39).expect("rebuild");

        assert!((bumped.recovery_rate - 0.39).abs() < 1e-12);

        // Per-issuer weights survive intact (the second leg of the regression).
        assert!(bumped.issuer_weights.is_some(), "issuer_weights dropped");
        let weights = bumped.issuer_weights.as_ref().expect("weights present");
        assert!((weights.get("ISS-0").copied().unwrap_or(0.0) - 0.50).abs() < 1e-12);
        assert!((weights.get("ISS-1").copied().unwrap_or(0.0) - 0.30).abs() < 1e-12);
        assert!((weights.get("ISS-2").copied().unwrap_or(0.0) - 0.20).abs() < 1e-12);
    }

    #[test]
    fn rebuild_with_recovery_preserves_all_three_overrides_together() {
        let original = sample_index(true, true);
        let bumped = rebuild_with_recovery(&original, 0.42).expect("rebuild");

        assert!((bumped.recovery_rate - 0.42).abs() < 1e-12);
        assert!(
            bumped.issuer_credit_curves.is_some(),
            "issuer_curves dropped"
        );
        assert!(
            bumped.issuer_recovery_rates.is_some(),
            "issuer_recovery_rates dropped"
        );
        assert!(bumped.issuer_weights.is_some(), "issuer_weights dropped");
        assert_eq!(bumped.num_constituents, original.num_constituents);
    }

    #[test]
    fn rebuild_with_recovery_handles_homogeneous_index() {
        // Homogeneous (no per-issuer recovery/weight overrides) — must not
        // panic and must propagate the bumped recovery without inventing fields.
        let original = sample_index(false, false);
        let bumped = rebuild_with_recovery(&original, 0.45).expect("rebuild");

        assert!((bumped.recovery_rate - 0.45).abs() < 1e-12);
        assert!(bumped.issuer_credit_curves.is_some());
        assert!(bumped.issuer_recovery_rates.is_none());
        assert!(bumped.issuer_weights.is_none());
    }
}
