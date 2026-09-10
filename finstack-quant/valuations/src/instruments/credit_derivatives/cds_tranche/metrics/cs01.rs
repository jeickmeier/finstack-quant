//! Quote-space CS01 to the index or complete issuer pool actually consumed by pricing.

use super::super::credit_risk::{active_hazards, parallel_cs01, replace_hazard};
use super::super::CDSTranche;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::{compute_key_rate_cs01_series_for_hazard, Cs01Request};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use std::sync::Arc;

pub(crate) struct CdsTrancheCs01Calculator;

impl MetricCalculator for CdsTrancheCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let tranche = context.instrument_as::<CDSTranche>()?.clone();
        let index = context.curves.get_credit_index(&tranche.credit_index_id)?;
        let hazards = active_hazards(&index, true);
        let bump = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?
        .credit_spread_bump_bp;
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
        let tranche = context.instrument_as::<CDSTranche>()?.clone();
        let index = context.curves.get_credit_index(&tranche.credit_index_id)?;
        let hazards = active_hazards(&index, true);
        let bump = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?
        .credit_spread_bump_bp;
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
                    let mut bumped = index.as_ref().clone();
                    replace_hazard(&mut bumped, market.get_hazard(hazard_id.as_str())?);
                    let market = market
                        .clone()
                        .insert_credit_index(tranche.credit_index_id.as_str(), bumped);
                    dispatch.price_raw(instrument.as_ref(), &market, as_of)
                },
            )?;
        }
        Ok(total)
    }
}
