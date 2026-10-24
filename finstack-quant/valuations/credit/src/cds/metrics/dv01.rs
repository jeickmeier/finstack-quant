//! CDS-family DV01: discount-curve bump with hazard-curve replay.
//!
//! CDS rate risk is a cross-curve sensitivity when the credit curve is stored
//! with the market par spreads used to build it: after a rate-curve bump, the
//! hazard curve must be re-bootstrapped from unchanged CDS spreads. This matches
//! Bloomberg-style IR DV01 for CDS screens. The CDS index and CDS tranche DV01
//! calculators reuse [`rate_bumped_market_with_hazard_replay`] for the same
//! bump-and-replay step.

use super::{deal_quote_override, market_doc_clause};
use crate::cds::CreditDefaultSwap;
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations_core::market::conventions::CdsValuationConvention;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::sensitivities::cs01::sensitivity_central_diff;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use finstack_quant_valuations_core::recalibration::{
    DealCdsQuoteOverride, DiscountCurveRecalibrationRequest, HazardRecalibrationAction,
    HazardRecalibrationRequest, QuoteBump,
};
use std::sync::Arc;

/// Market with discount curve `discount_id` shifted by a parallel `bump_bp`
/// (basis points): a swap-quote bump and rebuild when the curve records its
/// rate calibration, a parallel bump of the discount factors otherwise. Shared
/// by every CDS-family DV01 calculator (CDS, CDS option, CDS index, CDS
/// tranche) so all report one unit and sign.
pub(crate) fn discount_bumped_market(
    context: &MetricContext,
    discount_id: &CurveId,
    bump_bp: f64,
) -> Result<MarketContext> {
    let mut bumped_market: MarketContext = context.curves.as_ref().clone();
    let base_discount = context.curves.get_discount(discount_id.as_str())?;
    if let Some(calibration) = base_discount.rate_calibration() {
        let bumped_discount = context.rebuild_discount_curve(
            DiscountCurveRecalibrationRequest {
                curve: Arc::clone(&base_discount),
                recipe: calibration.clone(),
                market: Arc::clone(&context.curves),
                bump: QuoteBump::ParallelBp(bump_bp),
            },
            "dv01",
        )?;
        bumped_market = bumped_market.insert(bumped_discount.as_ref().clone());
    } else {
        bumped_market.apply_curve_bump_in_place(discount_id, BumpSpec::parallel_bp(bump_bp))?;
    }
    Ok(bumped_market)
}

/// A hazard curve to re-bootstrap onto a rate-bumped market, with the CDS
/// conventions its stored par-spread recipe must be replayed under.
pub(crate) struct HazardReplay {
    /// Hazard curve consumed by pricing; skipped when it has no calibration recipe.
    pub hazard: Arc<HazardCurve>,
    /// Documentation clause asserted against the recipe, if the instrument fixes one.
    pub doc_clause: Option<CdsDocClause>,
    /// CDS valuation convention asserted against the recipe, if the instrument fixes one.
    pub cds_valuation_convention: Option<CdsValuationConvention>,
    /// Deal-level clean quote replacing the recipe's quote at the deal maturity.
    pub deal_quote_override: Option<DealCdsQuoteOverride>,
}

/// Market with discount curve `discount_id` bumped by `bump_bp` basis points
/// (see [`discount_bumped_market`]) and every calibrated hazard curve in
/// `replays` re-bootstrapped from its unchanged par spreads against that
/// bumped discount curve. Hazards without a calibration recipe stay as they
/// are. Returns the rebuilt curves too, so callers that also hold the hazards
/// elsewhere (a credit index) can swap them in.
pub(crate) fn rate_bumped_market_with_hazard_replay(
    context: &MetricContext,
    discount_id: &CurveId,
    bump_bp: f64,
    replays: impl IntoIterator<Item = HazardReplay>,
) -> Result<(MarketContext, Vec<Arc<HazardCurve>>)> {
    let mut bumped_market = discount_bumped_market(context, discount_id, bump_bp)?;
    let target_market = Arc::new(bumped_market.clone());
    let mut rebuilt = Vec::new();
    for replay in replays {
        if replay.hazard.hazard_calibration().is_none() {
            continue;
        }
        let recalibrated = context.rebuild_hazard_curve(
            HazardRecalibrationRequest {
                hazard: replay.hazard,
                source_market: Arc::clone(&context.curves),
                target_market: Arc::clone(&target_market),
                discount_curve_id: discount_id.clone(),
                doc_clause: replay.doc_clause,
                cds_valuation_convention: replay.cds_valuation_convention,
                deal_quote_override: replay.deal_quote_override,
                action: HazardRecalibrationAction::DependencyMarketReplay,
            },
            "dv01",
        )?;
        bumped_market.insert_mut(Arc::clone(&recalibrated));
        rebuilt.push(recalibrated);
    }
    Ok((bumped_market, rebuilt))
}

/// CDS DV01 calculator with par-spread hazard re-bootstrap when possible.
pub(crate) struct CdsDv01Calculator;

impl MetricCalculator for CdsDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: CreditDefaultSwap = context.instrument_as::<CreditDefaultSwap>()?.clone();
        let bump_bp = sens_config::resolve(context)?.rate_bump_bp;
        if bump_bp.abs() <= 1e-10 {
            return Ok(0.0);
        }
        let hazard = context
            .curves
            .get_hazard(cds.protection_leg.credit_curve_id.as_str())?;

        let reprice = |bp: f64| -> Result<f64> {
            let replay = HazardReplay {
                hazard: Arc::clone(&hazard),
                doc_clause: Some(market_doc_clause(&cds)),
                cds_valuation_convention: Some(cds.valuation_convention),
                deal_quote_override: deal_quote_override(&cds),
            };
            let (bumped_market, _) = rate_bumped_market_with_hazard_replay(
                context,
                &cds.premium_leg.discount_curve_id,
                bp,
                [replay],
            )?;
            context.reprice_raw(&bumped_market, context.as_of)
        };

        Ok(sensitivity_central_diff(
            reprice(bump_bp)?,
            reprice(-bump_bp)?,
            bump_bp,
        ))
    }
}
