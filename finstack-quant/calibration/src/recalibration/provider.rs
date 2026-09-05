use super::hazard::{
    bump_hazard_spread_risk_input_cached, hazard_spread_risk_buckets,
    hazard_with_deal_quote_override, recalibrate_hazard_with_recovery,
    replay_hazard_on_dependency_market, replay_hazard_spread_risk_center, HazardRecalibrationCache,
};
use super::rates::{
    bump_discount_curve_from_rate_calibration_cached, bump_market_via_rate_quote_shock_cached,
    bump_single_ois_market_via_rate_quote_shock_cached, RateRecalibrationCache,
};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_valuations::recalibration::{
    DiscountCurveRecalibrationRequest, HazardRecalibrationAction, HazardRecalibrationRequest,
    HazardSpreadRiskBucket, RateMarketRecalibrationRequest, RecalibrationProvider,
};
use std::sync::Arc;

/// Batch-local quote-recalibration provider with concurrent result caches.
#[derive(Default)]
pub struct CachedRecalibrationProvider {
    rate: RateRecalibrationCache,
    hazard: HazardRecalibrationCache,
}

impl CachedRecalibrationProvider {
    /// Create an empty provider for one immutable pricing or scenario batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl RecalibrationProvider for CachedRecalibrationProvider {
    fn rebuild_rate_market(
        &self,
        request: &RateMarketRecalibrationRequest,
    ) -> finstack_quant_core::Result<Arc<MarketContext>> {
        match request {
            RateMarketRecalibrationRequest::LinkedDiscountForward {
                market,
                discount_curve_id,
                forward_curve_id,
                bump,
            } => bump_market_via_rate_quote_shock_cached(
                Some(&self.rate),
                market,
                discount_curve_id,
                forward_curve_id,
                bump,
            ),
            RateMarketRecalibrationRequest::SingleOis {
                market,
                curve_id,
                bump,
            } => bump_single_ois_market_via_rate_quote_shock_cached(
                Some(&self.rate),
                market,
                curve_id,
                bump,
            ),
        }
    }

    fn rebuild_discount_curve(
        &self,
        request: &DiscountCurveRecalibrationRequest,
    ) -> finstack_quant_core::Result<Arc<DiscountCurve>> {
        request.bump.validate()?;
        bump_discount_curve_from_rate_calibration_cached(
            Some(&self.rate),
            request.curve.as_ref(),
            &request.recipe,
            request.market.as_ref(),
            &request.bump,
        )
    }

    fn rebuild_hazard_curve(
        &self,
        request: &HazardRecalibrationRequest,
    ) -> finstack_quant_core::Result<Arc<HazardCurve>> {
        let hazard = match request.deal_quote_override {
            Some(deal_override) => Arc::new(hazard_with_deal_quote_override(
                &request.hazard,
                deal_override,
            )?),
            None => Arc::clone(&request.hazard),
        };
        let discount_id = Some(&request.discount_curve_id);
        let doc_clause = request.doc_clause;
        let convention = request.cds_valuation_convention;
        match &request.action {
            HazardRecalibrationAction::TimeRollReplay => {
                super::hazard::replay_hazard_at_horizon(request).map(Arc::new)
            }
            HazardRecalibrationAction::SpreadBump(bump) => {
                bump.validate()?;
                super::hazard::bump_hazard_on_target_market(
                    &self.hazard,
                    request,
                    hazard.as_ref(),
                    bump,
                )
            }
            HazardRecalibrationAction::ExactQuoteIndexBump {
                quote_index,
                bump_bp,
            } => {
                if !bump_bp.is_finite() {
                    return Err(finstack_quant_core::Error::Validation(
                        "exact quote bump must be finite".to_string(),
                    ));
                }
                bump_hazard_spread_risk_input_cached(
                    Some(&self.hazard),
                    hazard.as_ref(),
                    request.target_market.as_ref(),
                    (*quote_index, *bump_bp),
                    discount_id,
                    doc_clause,
                    convention,
                )
            }
            HazardRecalibrationAction::SpreadRiskCenterReplay => replay_hazard_spread_risk_center(
                hazard.as_ref(),
                request.target_market.as_ref(),
                discount_id,
                doc_clause,
                convention,
            )
            .map(Arc::new),
            HazardRecalibrationAction::DependencyMarketReplay => {
                replay_hazard_on_dependency_market(
                    hazard.as_ref(),
                    request.source_market.as_ref(),
                    request.target_market.as_ref(),
                    discount_id,
                    doc_clause,
                    convention,
                )
                .map(Arc::new)
            }
            HazardRecalibrationAction::RecoveryRateReplay { recovery_rate } => {
                if !recovery_rate.is_finite() || !(0.0..1.0).contains(recovery_rate) {
                    return Err(finstack_quant_core::Error::Validation(
                        "hazard recovery replay requires a finite recovery rate in [0, 1)"
                            .to_string(),
                    ));
                }
                recalibrate_hazard_with_recovery(
                    hazard.as_ref(),
                    *recovery_rate,
                    request.target_market.as_ref(),
                    discount_id,
                    doc_clause,
                    convention,
                )
                .map(Arc::new)
            }
        }
    }

    fn hazard_spread_risk_buckets(
        &self,
        hazard: &HazardCurve,
    ) -> finstack_quant_core::Result<Vec<HazardSpreadRiskBucket>> {
        hazard_spread_risk_buckets(hazard).map(|buckets| {
            buckets
                .into_iter()
                .map(|bucket| HazardSpreadRiskBucket {
                    quote_index: bucket.index,
                    quote_id: bucket.quote_id,
                    pillar_date: bucket.pillar_date,
                    pillar_time: bucket.pillar_time,
                })
                .collect()
        })
    }
}
