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
use finstack_quant_core::Result;
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::market::conventions::CdsValuationConvention;
use finstack_quant_valuations_core::market::conventions::ids::CdsDocClause;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, sensitivity_central_diff,
};
use finstack_quant_valuations_core::recalibration::{
    DealCdsQuoteOverride, DiscountCurveRecalibrationRequest, HazardRecalibrationAction,
    HazardRecalibrationRequest, QuoteBump,
};
use std::sync::Arc;

pub(crate) enum DiscountShock {
    ParallelBp(f64),
    Quote(QuoteBump),
    Direct(BumpSpec),
}

fn discount_bumped_market(
    context: &MetricContext,
    discount_id: &CurveId,
    shock: DiscountShock,
) -> Result<MarketContext> {
    let base_discount = context.curves.get_discount(discount_id.as_str())?;
    let bump = match shock {
        DiscountShock::Direct(spec) => {
            let mut market = context.curves.condition(context.as_of)?;
            market.apply_curve_bump_in_place(discount_id, spec)?;
            return Ok(market);
        }
        DiscountShock::ParallelBp(bp) if base_discount.rate_calibration().is_none() => {
            let mut market = context.curves.as_ref().clone();
            market.apply_curve_bump_in_place(discount_id, BumpSpec::parallel_bp(bp))?;
            return Ok(market);
        }
        DiscountShock::ParallelBp(bp) => QuoteBump::ParallelBp(bp),
        DiscountShock::Quote(bump) => bump,
    };
    let recipe = base_discount.rate_calibration().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "rate quote replay requires calibration for {discount_id}"
        ))
    })?;
    let bumped = context
        .recalibration_provider_ref("dv01")?
        .rebuild_discount_curve(&DiscountCurveRecalibrationRequest {
            curve: Arc::clone(&base_discount),
            recipe: recipe.clone(),
            market: Arc::clone(&context.curves),
            bump,
        })?;
    context.curves.as_ref().clone().try_insert(bumped)
}

pub(crate) fn calculate_credit_dv01(
    context: &mut MetricContext,
    discount_id: &CurveId,
    bucketed: bool,
    mut reprice: impl FnMut(&MetricContext, DiscountShock) -> Result<f64>,
) -> Result<f64> {
    use finstack_quant_valuations_core::metrics::sensitivities::dv01::rate_quote_dv01_series;
    use finstack_quant_valuations_core::metrics::{
        MetricId, format_key_rate_label, validate_buckets_strictly_increasing,
    };

    let settings = sens_config::resolve(context)?;
    let bp = settings.rate_bump_bp;
    if bp.abs() <= 1e-10 {
        return Ok(0.0);
    }
    if !bucketed {
        return sensitivity_central_diff(
            reprice(context, DiscountShock::ParallelBp(bp))?,
            reprice(context, DiscountShock::ParallelBp(-bp))?,
            bp,
        );
    }

    let curve = context.curves.get_discount(discount_id)?;
    let series = if let Some(recipe) = curve.rate_calibration() {
        rate_quote_dv01_series(discount_id, recipe, bp, |shock| {
            reprice(context, DiscountShock::Quote(shock))
        })?
    } else {
        let buckets = &settings.dv01_buckets_years;
        validate_buckets_strictly_increasing(buckets)?;
        let mut series = Vec::with_capacity(buckets.len());
        for (index, &target) in buckets.iter().enumerate() {
            let shock = |bp| {
                DiscountShock::Direct(BumpSpec::triangular_key_rate_bucket_bp(
                    buckets[..index].last().copied(),
                    target,
                    buckets.get(index + 1).copied(),
                    bp,
                ))
            };
            let up = reprice(context, shock(bp))?;
            let down = reprice(context, shock(-bp))?;
            series.push((
                format_key_rate_label(target),
                sensitivity_central_diff(up, down, bp)?,
            ));
        }
        series
    };
    let total =
        finstack_quant_core::math::summation::neumaier_sum(series.iter().map(|(_, value)| *value))?;
    context.store_bucketed_series(
        MetricId::composite(&MetricId::BucketedDv01, &[discount_id.as_str()]),
        series,
    );
    Ok(total)
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

/// Market with discount curve `discount_id` moved by the selected rate shock
/// (see [`discount_bumped_market`]) and every calibrated hazard curve in
/// `replays` re-bootstrapped from its unchanged par spreads against that
/// bumped discount curve. Hazards without a calibration recipe stay as they
/// are. Returns the rebuilt curves too, so callers that also hold the hazards
/// elsewhere (a credit index) can swap them in.
pub(crate) fn rate_bumped_market_with_hazard_replay(
    context: &MetricContext,
    discount_id: &CurveId,
    shock: DiscountShock,
    replays: impl IntoIterator<Item = HazardReplay>,
) -> Result<(MarketContext, Vec<Arc<HazardCurve>>)> {
    let mut bumped_market = discount_bumped_market(context, discount_id, shock)?;
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
        bumped_market.try_insert_mut(Arc::clone(&recalibrated))?;
        rebuilt.push(recalibrated);
    }
    Ok((bumped_market, rebuilt))
}

/// CDS DV01 calculator with par-spread hazard re-bootstrap when possible.
pub(crate) struct CdsDv01Calculator(pub(crate) bool);

impl MetricCalculator for CdsDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cds: CreditDefaultSwap = context.instrument_as::<CreditDefaultSwap>()?.clone();
        let hazard = context
            .curves
            .get_hazard(cds.protection_leg.credit_curve_id.as_str())?;

        let reprice = |context: &MetricContext, shock: DiscountShock| -> Result<f64> {
            let replay = HazardReplay {
                hazard: Arc::clone(&hazard),
                doc_clause: Some(market_doc_clause(&cds)),
                cds_valuation_convention: Some(cds.valuation_convention),
                deal_quote_override: deal_quote_override(&cds),
            };
            let (bumped_market, _) = rate_bumped_market_with_hazard_replay(
                context,
                &cds.premium_leg.discount_curve_id,
                shock,
                [replay],
            )?;
            context.reprice_raw(&bumped_market, context.as_of)
        };

        calculate_credit_dv01(context, &cds.premium_leg.discount_curve_id, self.0, reprice)
    }
}
