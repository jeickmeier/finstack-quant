//! Quote-space CS01 to the index or complete issuer pool actually consumed by pricing.

use crate::cds_tranche::CdsTranche;
use finstack_quant_core::market_data::{
    context::MarketContext,
    term_structures::{CreditIndexData, HazardCurve},
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::sensitivities::cs01::{
    compute_key_rate_cs01_series_for_hazard, require_hazard_replay, sensitivity_central_diff,
    Cs01Request,
};
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_valuations_core::recalibration::{
    HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump, RecalibrationProvider,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(crate) struct CdsTrancheCs01Calculator;

impl MetricCalculator for CdsTrancheCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let tranche = context.instrument_as::<CdsTranche>()?.clone();
        let index = context.curves.get_credit_index(&tranche.credit_index_id)?;
        let hazards = active_hazards(&index);
        let bump = sens_config::resolve(context)?.credit_spread_bump_bp;
        let provider = context.recalibration_provider("cs01")?;
        let dispatch = context.clone_pricer_dispatch();
        let revalue = |market: &finstack_quant_core::market_data::context::MarketContext| {
            dispatch.price_raw(context.instrument.as_ref(), market, context.as_of)
        };
        let total = parallel_cs01(
            provider.as_ref(),
            &context.curves,
            tranche.credit_index_id.as_str(),
            &tranche.discount_curve_id,
            &hazards,
            bump,
            revalue,
        )?;
        let components = hazards
            .iter()
            .map(|hazard| {
                let value = if hazards.len() == 1 {
                    total
                } else {
                    parallel_cs01(
                        provider.as_ref(),
                        &context.curves,
                        tranche.credit_index_id.as_str(),
                        &tranche.discount_curve_id,
                        std::slice::from_ref(hazard),
                        bump,
                        revalue,
                    )?
                };
                Ok((
                    MetricId::composite(&MetricId::Cs01, &[hazard.id().as_str()]),
                    value,
                ))
            })
            .collect::<finstack_quant_core::Result<Vec<_>>>()?;
        context.computed.extend(components);
        Ok(total)
    }
}

pub(crate) struct CdsTrancheBucketedCs01Calculator;

impl MetricCalculator for CdsTrancheBucketedCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let tranche = context.instrument_as::<CdsTranche>()?.clone();
        let index = context.curves.get_credit_index(&tranche.credit_index_id)?;
        let hazards = active_hazards(&index);
        let bump = sens_config::resolve(context)?.credit_spread_bump_bp;
        let request = Cs01Request::generic(bump, tranche.discount_curve_id.clone());
        let instrument = Arc::clone(&context.instrument);
        let dispatch = context.clone_pricer_dispatch();
        let as_of = context.as_of;
        let mut total = 0.0;
        for hazard in hazards {
            let hazard_id = hazard.id().clone();
            let series_id = MetricId::composite(&MetricId::BucketedCs01, &[hazard_id.as_str()]);
            total += compute_key_rate_cs01_series_for_hazard(
                context,
                hazard,
                series_id,
                &request,
                |market| {
                    let bumped = replace_hazard(&index, market.get_hazard(hazard_id.as_str())?)?;
                    let market = market
                        .clone()
                        .insert_credit_index(tranche.credit_index_id.as_str(), bumped)?;
                    dispatch.price_raw(instrument.as_ref(), &market, as_of)
                },
            )?;
        }
        Ok(total)
    }
}

// Resolve and rebootstrap the credit curves consumed by tranche pricing.

fn active_hazards(index: &CreditIndexData) -> Vec<Arc<HazardCurve>> {
    if let Some(issuers) = index.issuer_credit_curves() {
        // One quote shock per curve, even if several names share its recipe.
        issuers
            .values()
            .map(|curve| (curve.id().clone(), Arc::clone(curve)))
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .collect()
    } else {
        vec![Arc::clone(index.index_credit_curve())]
    }
}

fn replace_hazard(index: &CreditIndexData, hazard: Arc<HazardCurve>) -> Result<CreditIndexData> {
    let mut builder = index.to_builder();
    if index.index_credit_curve().id() == hazard.id() {
        builder = builder.index_credit_curve(Arc::clone(&hazard));
    }
    if let Some(issuers) = index.issuer_credit_curves() {
        let updated: finstack_quant_core::HashMap<_, _> = issuers
            .iter()
            .map(|(issuer, curve)| {
                let next = if curve.id() == hazard.id() {
                    Arc::clone(&hazard)
                } else {
                    Arc::clone(curve)
                };
                (issuer.clone(), next)
            })
            .collect();
        builder = builder.issuer_curves(updated);
    }
    builder.build()
}

fn parallel_cs01(
    provider: &dyn RecalibrationProvider,
    market: &MarketContext,
    index_id: &str,
    discount_id: &CurveId,
    hazards: &[Arc<HazardCurve>],
    bump_bp: f64,
    revalue: impl Fn(&MarketContext) -> Result<f64>,
) -> Result<f64> {
    if !bump_bp.is_finite() || bump_bp <= 0.0 {
        return Err(Error::Validation(
            "tranche CS01 bump must be finite and positive".to_owned(),
        ));
    }
    let original = market.get_credit_index(index_id)?;
    let source = Arc::new(market.clone());
    let reprice = |bp| {
        let mut bumped_index = original.as_ref().clone();
        let mut bumped_market = market.clone();
        for hazard in hazards {
            require_hazard_replay(hazard, "tranche quote-space CS01")?;
            let bumped = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
                hazard: Arc::clone(hazard),
                source_market: Arc::clone(&source),
                target_market: Arc::clone(&source),
                discount_curve_id: discount_id.clone(),
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
                action: HazardRecalibrationAction::SpreadBump(QuoteBump::ParallelBp(bp)),
            })?;
            bumped_index = replace_hazard(&bumped_index, Arc::clone(&bumped))?;
            bumped_market.insert_mut(bumped);
        }
        revalue(&bumped_market.insert_credit_index(index_id, bumped_index)?)
    };
    Ok(sensitivity_central_diff(
        reprice(bump_bp)?,
        reprice(-bump_bp)?,
        bump_bp,
    ))
}
