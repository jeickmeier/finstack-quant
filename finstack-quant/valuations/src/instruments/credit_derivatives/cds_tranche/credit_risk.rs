//! Resolve and rebootstrap the credit curves consumed by tranche pricing.

use crate::metrics::sensitivities::cs01::{require_hazard_replay, sensitivity_central_diff};
use crate::recalibration::{
    HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump, RecalibrationProvider,
};
use finstack_quant_core::market_data::{
    context::MarketContext,
    term_structures::{CreditIndexData, HazardCurve},
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::{Error, Result};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) fn active_hazards(index: &CreditIndexData, use_issuers: bool) -> Vec<Arc<HazardCurve>> {
    if let Some(issuers) = index.issuer_credit_curves.as_ref().filter(|_| use_issuers) {
        // One quote shock per curve, even if several names share its recipe.
        issuers
            .values()
            .map(|curve| (curve.id().clone(), Arc::clone(curve)))
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .collect()
    } else {
        vec![Arc::clone(&index.index_credit_curve)]
    }
}

pub(super) fn replace_hazard(index: &mut CreditIndexData, hazard: Arc<HazardCurve>) {
    if index.index_credit_curve.id() == hazard.id() {
        index.index_credit_curve = Arc::clone(&hazard);
    }
    if let Some(issuers) = &mut index.issuer_credit_curves {
        for curve in issuers.values_mut() {
            if curve.id() == hazard.id() {
                *curve = Arc::clone(&hazard);
            }
        }
    }
}

pub(super) fn parallel_cs01(
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
            replace_hazard(&mut bumped_index, Arc::clone(&bumped));
            bumped_market.insert_mut(bumped);
        }
        revalue(&bumped_market.insert_credit_index(index_id, bumped_index))
    };
    Ok(sensitivity_central_diff(
        reprice(bump_bp)?,
        reprice(-bump_bp)?,
        bump_bp,
    ))
}
