//! Base correlation shock adapter.
//!
//! One generator serves both parallel and bucketed base correlation shocks:
//! a parallel shock is a bucket shock over every detachment point.
//!
//! # Clamping Behavior
//!
//! Correlation values are clamped to the valid range [0, 1] after applying shocks.
//! When clamping occurs, warnings are returned to alert the caller that the
//! requested shock could not be fully applied.

use crate::engine::ExecutionContext;
use crate::engine::ScenarioEffect;
use crate::warning::Warning;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::types::CurveId;

/// Clamp and no-match warnings for a base-correlation shock; `op` names the
/// scenario operation in the warning detail.
fn bump_warnings(
    op: &str,
    ctx: &ExecutionContext,
    surface_id: &CurveId,
    detachments: Option<&[f64]>,
    points: f64,
) -> Vec<Warning> {
    let Ok(curve) = ctx.market.get_base_correlation(surface_id.as_str()) else {
        return Vec::new();
    };

    let matching: Vec<(f64, f64)> = curve.bucket_knots(detachments).collect();

    let mut warnings = Vec::new();
    if detachments.is_some_and(|requested| !requested.is_empty()) && matching.is_empty() {
        warnings.push(Warning::BaseCorrBucketNoMatch {
            surface_id: surface_id.as_str().to_string(),
        });
    }

    warnings.extend(matching.into_iter().filter_map(|(detachment, corr)| {
        let bumped = corr + points;
        if (0.0..=1.0).contains(&bumped) {
            None
        } else {
            let clamped = bumped.clamp(0.0, 1.0);
            Some(Warning::CorrelationClamped {
                instrument_id: surface_id.as_str().to_string(),
                detail: format!(
                    "{op} on {surface_id} detachment {detachment:.10} clamped \
                     correlation from requested {bumped:.10} to {clamped:.10}",
                ),
            })
        }
    }));

    warnings
}

/// Generate effects for a base-correlation shock.
///
/// `detachment_bp` restricts the shock to those detachment points (basis
/// points, `300` = 3%); `None` shocks every bucket, which is the parallel
/// shock. `op` is the scenario operation name reported in warnings
/// (`"BaseCorrParallelPts"` or `"BaseCorrBucketPts"`).
pub(crate) fn base_corr_effects(
    op: &str,
    surface_id: &CurveId,
    detachment_bp: Option<&[i32]>,
    points: f64,
    ctx: &ExecutionContext,
) -> Vec<ScenarioEffect> {
    let dets = detachment_bp.map(|bp| {
        bp.iter()
            .map(|bp| f64::from(*bp) / 10_000.0)
            .collect::<Vec<f64>>()
    });

    let bump = MarketBump::BaseCorrBucketPts {
        surface_id: surface_id.clone(),
        detachments: dets.clone(),
        points,
    };

    let mut effects = vec![ScenarioEffect::MarketBump(bump)];
    effects.extend(
        bump_warnings(op, ctx, surface_id, dets.as_deref(), points)
            .into_iter()
            .map(ScenarioEffect::Warning),
    );

    effects
}
