//! Quote-space market recalibration contract used by valuation and risk code.
//!
//! This module deliberately contains contracts only. Implementations live in
//! `finstack-quant-calibration`, keeping the permanent dependency direction
//! `calibration -> valuations`.

use crate::instruments::credit_derivatives::cds::CdsValuationConvention;
use crate::market::conventions::ids::CdsDocClause;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, HazardCurve, RateCalibrationRecipe,
};
use finstack_quant_core::types::CurveId;
use std::sync::Arc;

/// Additive quote-space shock expressed in basis points.
#[derive(Clone, Debug, PartialEq)]
pub enum QuoteBump {
    /// Apply the same basis-point shock to every replay quote.
    ParallelBp(f64),
    /// Apply additive basis-point shocks to matching tenor buckets.
    ///
    /// Each tuple is `(tenor_years, bump_bp)`. Repeated matching targets are
    /// additive and retain their input order.
    TenorsBp(Vec<(f64, f64)>),
}

impl QuoteBump {
    /// Validate that every tenor and shock is finite.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when a parallel
    /// shock, tenor, or tenor shock is not finite.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        match self {
            Self::ParallelBp(bp) if !bp.is_finite() => Err(finstack_quant_core::Error::Validation(
                "parallel quote bump must be finite".to_string(),
            )),
            Self::TenorsBp(targets) => {
                for (tenor, bp) in targets {
                    if !tenor.is_finite() || !bp.is_finite() {
                        return Err(finstack_quant_core::Error::Validation(
                            "quote-bump tenor and basis-point shock must be finite".to_string(),
                        ));
                    }
                }
                Ok(())
            }
            Self::ParallelBp(_) => Ok(()),
        }
    }

    /// Return whether the requested quote shock is exactly zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        match self {
            Self::ParallelBp(bp) => *bp == 0.0,
            Self::TenorsBp(targets) => targets.iter().all(|(_, bp)| *bp == 0.0),
        }
    }
}

/// Request to rebuild a rate market from stored quote recipes.
#[derive(Clone)]
pub enum RateMarketRecalibrationRequest {
    /// Rebuild linked discount and forward curves.
    LinkedDiscountForward {
        /// Immutable source market containing both curves and their recipes.
        market: Arc<MarketContext>,
        /// Discount curve identifier.
        discount_curve_id: CurveId,
        /// Forward curve identifier.
        forward_curve_id: CurveId,
        /// Quote-space shock applied to both stored recipes.
        bump: QuoteBump,
    },
    /// Rebuild one OIS curve used for both discounting and projection.
    SingleOis {
        /// Immutable source market containing the curve and its recipe.
        market: Arc<MarketContext>,
        /// OIS discount curve identifier.
        curve_id: CurveId,
        /// Quote-space shock applied to the stored recipe.
        bump: QuoteBump,
    },
}

/// Request to rebuild one discount curve from its stored core recipe.
#[derive(Clone)]
pub struct DiscountCurveRecalibrationRequest {
    /// Stored curve whose shape and metadata are preserved.
    pub curve: Arc<DiscountCurve>,
    /// Lossless core replay recipe attached when the curve was built.
    pub recipe: RateCalibrationRecipe,
    /// Immutable market supplying calibration dependencies.
    pub market: Arc<MarketContext>,
    /// Quote-space shock to apply before replay.
    pub bump: QuoteBump,
}

/// Optional deal-level par-spread replacement applied to hazard replay inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DealCdsQuoteOverride {
    /// Contractual CDS end date used to select the exact replay pillar.
    pub contract_end: Date,
    /// Replacement clean par spread in basis points.
    pub spread_bp: f64,
}

/// Hazard-curve replay operation.
#[derive(Clone, Debug, PartialEq)]
pub enum HazardRecalibrationAction {
    /// Apply a parallel or tenor quote shock to spread-risk inputs.
    SpreadBump(QuoteBump),
    /// Bump exactly one ordered spread-risk quote.
    ExactQuoteIndexBump {
        /// Zero-based index in the stored spread-risk replay inputs.
        quote_index: usize,
        /// Additive par-spread shock in basis points.
        bump_bp: f64,
    },
    /// Replay the stored spread-risk center without a shock.
    SpreadRiskCenterReplay,
    /// Derive horizon-date par quotes from conditional survival on the rolled
    /// target market and calibrate a fresh recipe with surviving dated pillars.
    /// The source recipe is verified before rolling; expired quotes are removed.
    /// Deal-level quote overrides are rejected for this action.
    TimeRollReplay,
    /// Replay unchanged quotes against the target dependency market.
    DependencyMarketReplay,
    /// Replay unchanged quotes under a new recovery assumption.
    RecoveryRateReplay {
        /// Recovery rate represented as a decimal in `[0, 1)`.
        recovery_rate: f64,
    },
}

/// Request to rebuild a hazard curve from stored quote recipes.
#[derive(Clone)]
pub struct HazardRecalibrationRequest {
    /// Source hazard curve carrying the lossless replay recipe.
    pub hazard: Arc<HazardCurve>,
    /// Market used to verify zero-shock identity and supply source dependencies.
    pub source_market: Arc<MarketContext>,
    /// Market supplying dependencies for the requested replay.
    pub target_market: Arc<MarketContext>,
    /// Discount curve identifier required by the stored recipe.
    pub discount_curve_id: CurveId,
    /// Optional documentation-clause assertion.
    pub doc_clause: Option<CdsDocClause>,
    /// Optional CDS valuation-convention assertion.
    pub cds_valuation_convention: Option<CdsValuationConvention>,
    /// Optional deal-level clean-spread replacement.
    pub deal_quote_override: Option<DealCdsQuoteOverride>,
    /// Replay action to execute.
    pub action: HazardRecalibrationAction,
}

/// Exact ordered spread-risk quote binding used by bucketed CS01.
#[derive(Clone, Debug, PartialEq)]
pub struct HazardSpreadRiskBucket {
    /// Zero-based quote index in the stored replay recipe.
    pub quote_index: usize,
    /// Stable market quote identifier used in metric labels.
    pub quote_id: String,
    /// Resolved contractual pillar date.
    pub pillar_date: Date,
    /// Pillar time in the hazard curve's day-count convention.
    pub pillar_time: f64,
}

/// Quote-recalibration service injected into pricing and risk requests.
pub trait RecalibrationProvider: Send + Sync {
    /// Rebuild a rate market from stored quote recipes.
    ///
    /// # Arguments
    ///
    /// * `request` - Linked-curve or single-OIS replay request, including the
    ///   immutable source market and basis-point quote shock.
    fn rebuild_rate_market(
        &self,
        request: &RateMarketRecalibrationRequest,
    ) -> finstack_quant_core::Result<Arc<MarketContext>>;

    /// Rebuild one discount curve from its stored quote recipe.
    ///
    /// # Arguments
    ///
    /// * `request` - Curve, replay recipe, dependency market, and quote shock.
    fn rebuild_discount_curve(
        &self,
        request: &DiscountCurveRecalibrationRequest,
    ) -> finstack_quant_core::Result<Arc<DiscountCurve>>;

    /// Rebuild one hazard curve from its stored quote recipe.
    ///
    /// # Arguments
    ///
    /// * `request` - Source curve and markets, conventions, optional deal
    ///   override, and the exact replay action.
    fn rebuild_hazard_curve(
        &self,
        request: &HazardRecalibrationRequest,
    ) -> finstack_quant_core::Result<Arc<HazardCurve>>;

    /// Get the discount-curve dependency of a stored hazard calibration recipe.
    ///
    /// # Arguments
    ///
    /// * `hazard` - Source hazard curve carrying the original typed calibration
    ///   recipe. Its discount identifier is resolved by the provider that owns
    ///   that recipe's schema; missing or invalid recipes return an error.
    fn get_hazard_discount_curve_id(
        &self,
        hazard: &HazardCurve,
    ) -> finstack_quant_core::Result<CurveId>;

    /// Return exact ordered spread-risk bindings from a hazard replay recipe.
    ///
    /// # Arguments
    ///
    /// * `hazard` - Hazard curve carrying the lossless replay recipe.
    fn hazard_spread_risk_buckets(
        &self,
        hazard: &HazardCurve,
    ) -> finstack_quant_core::Result<Vec<HazardSpreadRiskBucket>>;
}

/// Build the canonical error for a quote-rebootstrap request without a provider.
///
/// # Arguments
///
/// * `metric` - Requested metric or operation name included in the diagnostic.
pub fn provider_missing(metric: &str) -> finstack_quant_core::Error {
    finstack_quant_core::Error::Calibration {
        message: format!(
            "quote recalibration required for '{metric}', but PricingOptions has no recalibration provider"
        ),
        category: "recalibration_provider_missing".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_object_safe_and_thread_safe(_: Arc<dyn RecalibrationProvider>) {}

    #[test]
    fn provider_trait_is_object_safe_send_and_sync() {
        struct Provider;
        impl RecalibrationProvider for Provider {
            fn rebuild_rate_market(
                &self,
                _request: &RateMarketRecalibrationRequest,
            ) -> finstack_quant_core::Result<Arc<MarketContext>> {
                unreachable!()
            }

            fn rebuild_discount_curve(
                &self,
                _request: &DiscountCurveRecalibrationRequest,
            ) -> finstack_quant_core::Result<Arc<DiscountCurve>> {
                unreachable!()
            }

            fn rebuild_hazard_curve(
                &self,
                _request: &HazardRecalibrationRequest,
            ) -> finstack_quant_core::Result<Arc<HazardCurve>> {
                unreachable!()
            }

            fn get_hazard_discount_curve_id(
                &self,
                _hazard: &HazardCurve,
            ) -> finstack_quant_core::Result<CurveId> {
                unreachable!()
            }

            fn hazard_spread_risk_buckets(
                &self,
                _hazard: &HazardCurve,
            ) -> finstack_quant_core::Result<Vec<HazardSpreadRiskBucket>> {
                unreachable!()
            }
        }

        assert_object_safe_and_thread_safe(Arc::new(Provider));
    }

    #[test]
    fn missing_provider_errors_name_every_quote_replay_operation() {
        for operation in [
            "rate_replay",
            "dv01",
            "cs01",
            "bucketed_cs01",
            "cs_gamma",
            "dependency_replay",
            "deal_quote_override",
            "recovery01",
        ] {
            let error = provider_missing(operation);
            match error {
                finstack_quant_core::Error::Calibration { message, category } => {
                    assert_eq!(category, "recalibration_provider_missing");
                    assert!(message.contains(operation));
                }
                other => panic!("unexpected provider error for {operation}: {other}"),
            }
        }
    }
}
