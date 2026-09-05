//! CDS Index CS01 metric calculators.
//!
//! Both calculators report CS01 against the [canonical convention][canonical]:
//! a parallel 1 bp shock to credit spreads with a symmetric (central) finite
//! difference `(PV(s + 1bp) − PV(s − 1bp)) / 2`. They differ only in *which*
//! spread is shocked and how the index aggregates per-name sensitivity:
//!
//! - [`Cs01Calculator`]: parallel CS01 derived from per-name finite differences
//!   summed over surviving constituents (or computed on the synthetic CDS in
//!   `SingleCurve` mode). Routed through [`CDSIndex::cs01`]; treats each
//!   constituent's bump as a parallel par-spread shock.
//! - [`CdsIndexBucketedCs01Calculator`]: quote-bucketed par-spread CS01 — the
//!   bucketed counterpart of [`Cs01Calculator`]. Applies one exact atomic
//!   `spread_risk_inputs` shock at a time to each mode-aware credit curve,
//!   reprices end-to-end, and stores per-curve series whose sum reconciles to
//!   parallel `Cs01`.
//!
//! Sign convention (per canonical reference):
//! - Long index protection (sell protection) → CS01 negative.
//! - Short index protection (buy protection) → CS01 positive.
//!
//! [canonical]: crate::metrics::sensitivities::cs01
//! [`CDSIndex::cs01`]: crate::instruments::credit_derivatives::cds_index::CDSIndex::cs01

use crate::instruments::credit_derivatives::cds_index::{CDSIndex, IndexPricing};
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::{
    compute_key_rate_cs01_series_with_context_raw, cs01_reval, Cs01Request,
};
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::math::NeumaierAccumulator;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;

/// Parallel CS01 calculator for CDS Index (per-name finite difference).
pub(crate) struct Cs01Calculator;

impl MetricCalculator for Cs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let provider = context.recalibration_provider("cs01")?;
        let idx: &CDSIndex = context.instrument_as()?;
        idx.cs01(&context.curves, context.as_of, provider.as_ref())
    }
}

fn index_credit_curve_ids(index: &CDSIndex) -> Result<Vec<CurveId>> {
    match index.pricing {
        IndexPricing::SingleCurve => Ok(vec![index.protection.credit_curve_id.clone()]),
        IndexPricing::Constituents => {
            let mut curve_ids = Vec::new();
            for constituent in index
                .constituents
                .iter()
                .filter(|constituent| !constituent.defaulted)
            {
                let curve_id = &constituent.credit.credit_curve_id;
                if !curve_ids.contains(curve_id) {
                    curve_ids.push(curve_id.clone());
                }
            }
            Ok(curve_ids)
        }
    }
}

/// Quote-bucketed par-spread CS01 calculator for CDS Index.
///
/// Uses the shared exact replay-binding decomposition for each relevant curve:
/// `SingleCurve` processes the synthetic index curve, while `Constituents`
/// processes each distinct surviving constituent curve. Every series is stored
/// under `bucketed_cs01::{curve_id}` with collision-safe quote labels.
pub(crate) struct CdsIndexBucketedCs01Calculator;

impl MetricCalculator for CdsIndexBucketedCs01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let index: CDSIndex = context.instrument_as::<CDSIndex>()?.clone();

        // Expired → zero, no series (mirrors the parallel aggregation path).
        if context.as_of >= index.premium.end {
            return Ok(0.0);
        }

        let defaults = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_overrides(),
        )?;
        let bump_bp = defaults.credit_spread_bump_bp;

        let credit_ids = index_credit_curve_ids(&index)?;
        if credit_ids.is_empty() {
            return Ok(0.0);
        }
        let discount_id = index.premium.discount_curve_id;
        let mut total = NeumaierAccumulator::new();
        for credit_id in credit_ids {
            let series_id = MetricId::custom(format!("bucketed_cs01::{}", credit_id.as_str()));
            let reval = cs01_reval(context);
            let request = Cs01Request::generic(bump_bp, discount_id.clone());
            total.add(compute_key_rate_cs01_series_with_context_raw(
                context, &credit_id, series_id, &request, reval,
            )?);
        }
        Ok(total.total())
    }
}
