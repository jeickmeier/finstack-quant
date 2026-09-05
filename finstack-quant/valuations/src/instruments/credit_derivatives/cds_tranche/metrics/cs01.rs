//! CDS tranche–specific CS01 calculators.
//!
//! Implements the [canonical CS01 convention][canonical]: a parallel 1 bp
//! shock to the par CDS curve underlying the credit index, re-bootstrapped,
//! with a symmetric (central) finite difference
//! `(PV(s + 1bp) − PV(s − 1bp)) / 2`. The bucketed variant applies the same
//! shock one tenor at a time.
//!
//! These calculators differ from the workspace generics only in how they
//! resolve the credit curve. The generic CS01 calculator assumes
//! `MarketDependencies::curves.credit_curves` contains direct hazard curve IDs;
//! for CDS tranches, however, the credit dependency is a **credit index ID**
//! (e.g. `"CDX.NA.IG.HAZARD"`), and the actual hazard curve sits inside the
//! `CreditIndexData` under a different ID (e.g. `"CDX-HAZ"`). These
//! calculators resolve the index → hazard mapping before delegating to the
//! shared CS01 bump helpers.
//!
//! Sign convention (per canonical reference):
//! - Long tranche / sell tranche protection → CS01 negative.
//! - Short tranche / buy tranche protection → CS01 positive.
//!
//! [canonical]: crate::metrics::sensitivities::cs01

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::credit_derivatives::cds_tranche::CDSTranche;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::{
    compute_key_rate_cs01_series_with_context_raw, compute_parallel_cs01_with_context_raw,
    Cs01Request,
};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::types::CurveId;
use std::sync::Arc;

/// Resolve the hazard curve ID and discount curve ID for a CDS tranche.
///
/// Fetches the `CreditIndexData` from the market context, extracts the
/// actual hazard curve ID from `index_credit_curve`, and returns it
/// alongside the discount curve ID.
fn resolve_tranche_cs01_curves(
    tranche: &CDSTranche,
    market: &finstack_quant_core::market_data::context::MarketContext,
) -> finstack_quant_core::Result<(CurveId, Option<CurveId>)> {
    let index_data = market
        .get_credit_index(&tranche.credit_index_id)
        .map_err(|_| {
            finstack_quant_core::Error::Validation(format!(
                "Credit index '{}' not found for tranche '{}' CS01 calculation",
                tranche.credit_index_id,
                tranche.id()
            ))
        })?;

    let hazard_id = CurveId::from(index_data.index_credit_curve.id().as_str());
    let discount_id = Some(tranche.discount_curve_id.clone());
    Ok((hazard_id, discount_id))
}

/// CDS tranche parallel CS01 that resolves the credit index → hazard curve mapping.
pub(crate) struct CdsTrancheCs01Calculator;

impl MetricCalculator for CdsTrancheCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let tranche: CDSTranche = context.instrument_as::<CDSTranche>()?.clone();
        let (hazard_id, discount_id) =
            resolve_tranche_cs01_curves(&tranche, context.curves.as_ref())?;

        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?
        .credit_spread_bump_bp;

        let index_data = context.curves.get_credit_index(&tranche.credit_index_id)?;
        let credit_index_id = tranche.credit_index_id;
        let hazard_id_for_reval = hazard_id.clone();
        let inst_arc = Arc::clone(&context.instrument);
        let dispatch = context.clone_pricer_dispatch();
        let as_of = context.as_of;

        let reval = move |temp_ctx: &finstack_quant_core::market_data::context::MarketContext| {
            let bumped_hazard = temp_ctx.get_hazard(hazard_id_for_reval.as_str())?;
            let indexed_ctx = temp_ctx.clone().insert_credit_index(
                credit_index_id.as_str(),
                rebuild_index(index_data.as_ref(), bumped_hazard)?,
            );
            dispatch.price_raw(inst_arc.as_ref(), &indexed_ctx, as_of)
        };

        let request = Cs01Request::generic(
            bump_bp,
            discount_id.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "CDS tranche CS01 requires a discount curve".to_string(),
                )
            })?,
        );
        let cs01 = compute_parallel_cs01_with_context_raw(context, &hazard_id, &request, reval)?;

        context.computed.insert(
            MetricId::custom(format!("cs01::{}", hazard_id.as_str())),
            cs01,
        );

        Ok(cs01)
    }
}

fn rebuild_index(
    original_index: &CreditIndexData,
    hazard: Arc<finstack_quant_core::market_data::term_structures::HazardCurve>,
) -> finstack_quant_core::Result<CreditIndexData> {
    let mut builder = CreditIndexData::builder()
        .num_constituents(original_index.num_constituents)
        .recovery_rate(original_index.recovery_rate)
        .index_credit_curve(hazard)
        .base_correlation_curve(Arc::clone(&original_index.base_correlation_curve));
    if let Some(curves) = &original_index.issuer_credit_curves {
        builder = builder.issuer_curves(curves.clone());
    }
    if let Some(rates) = &original_index.issuer_recovery_rates {
        builder = builder.issuer_recovery_rates(rates.clone());
    }
    if let Some(weights) = &original_index.issuer_weights {
        builder = builder.issuer_weights(weights.clone());
    }
    builder.build()
}

/// CDS tranche bucketed CS01 that resolves the credit index → hazard curve mapping.
pub(crate) struct CdsTrancheBucketedCs01Calculator;

impl MetricCalculator for CdsTrancheBucketedCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let tranche: CDSTranche = context.instrument_as::<CDSTranche>()?.clone();
        let (hazard_id, discount_id) =
            resolve_tranche_cs01_curves(&tranche, context.curves.as_ref())?;

        let defaults = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?;
        let bump_bp = defaults.credit_spread_bump_bp;

        let index_data = context.curves.get_credit_index(&tranche.credit_index_id)?;
        let credit_index_id = tranche.credit_index_id;
        let hazard_id_for_reval = hazard_id.clone();
        let inst_arc = Arc::clone(&context.instrument);
        let dispatch = context.clone_pricer_dispatch();
        let as_of = context.as_of;

        let reval = move |temp_ctx: &finstack_quant_core::market_data::context::MarketContext| {
            let bumped_hazard = temp_ctx.get_hazard(hazard_id_for_reval.as_str())?;
            let indexed_ctx = temp_ctx.clone().insert_credit_index(
                credit_index_id.as_str(),
                rebuild_index(index_data.as_ref(), bumped_hazard)?,
            );
            match &dispatch {
                crate::pricer::PricingDispatch::Registered { model, registry } => registry
                    .price_raw(inst_arc.as_ref(), *model, &indexed_ctx, as_of)
                    .map_err(Into::into),
                crate::pricer::PricingDispatch::InstrumentDefault => {
                    inst_arc.value_raw(&indexed_ctx, as_of)
                }
            }
        };

        let series_id = MetricId::custom(format!("bucketed_cs01::{}", hazard_id.as_str()));

        let request = Cs01Request::generic(
            bump_bp,
            discount_id.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "CDS tranche bucketed CS01 requires a discount curve".to_string(),
                )
            })?,
        );
        compute_key_rate_cs01_series_with_context_raw(
            context, &hazard_id, series_id, &request, reval,
        )
    }
}
