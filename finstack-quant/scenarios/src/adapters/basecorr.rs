//! Convert scenario detachment coordinates to the core curve's percentage units.

use crate::engine::ScenarioEffect;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::types::CurveId;

/// Construct a decimal correlation shock for core to validate and apply atomically.
pub(crate) fn base_corr_effects(
    surface_id: &CurveId,
    detachment_bp: Option<&[i32]>,
    points: f64,
) -> Vec<ScenarioEffect> {
    let detachments =
        detachment_bp.map(|values| values.iter().map(|bp| f64::from(*bp) / 100.0).collect());
    vec![ScenarioEffect::MarketBump(MarketBump::BaseCorrBucketPts {
        surface_id: surface_id.clone(),
        detachments,
        points,
    })]
}
