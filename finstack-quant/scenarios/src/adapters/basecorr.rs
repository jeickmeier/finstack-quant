//! Base correlation shock adapter.
//!
//! Contains helpers for both parallel and bucketed base correlation shocks.
//!
//! # Clamping Behavior
//!
//! Correlation values are clamped to the valid range [0, 1] after applying shocks.
//! When clamping occurs, warnings are returned to alert the caller that the
//! requested shock could not be fully applied.

use crate::adapters::traits::ScenarioEffect;
use crate::engine::ExecutionContext;
use crate::error::{Error, Result};
use crate::warning::Warning;
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::market_data::term_structures::BASE_CORR_DETACHMENT_MATCH_TOLERANCE;
use finstack_quant_core::types::CurveId;

fn detachment_matches(requested: f64, detachment: f64) -> bool {
    (requested - detachment).abs() <= BASE_CORR_DETACHMENT_MATCH_TOLERANCE
}

fn bump_warnings(
    ctx: &ExecutionContext,
    surface_id: &CurveId,
    detachments: Option<&[f64]>,
    points: f64,
) -> Vec<Warning> {
    let Ok(curve) = ctx.market.get_base_correlation(surface_id.as_str()) else {
        return Vec::new();
    };

    let matching: Vec<(f64, f64)> = curve
        .detachment_points()
        .iter()
        .copied()
        .zip(curve.correlations().iter().copied())
        .filter(|(detachment, _)| {
            detachments
                .map(|requested| {
                    requested
                        .iter()
                        .any(|target| detachment_matches(*target, *detachment))
                })
                .unwrap_or(true)
        })
        .collect();

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
                    "BaseCorrBucketPts on {surface_id} detachment {detachment:.10} clamped \
                     correlation from requested {bumped:.10} to {clamped:.10}",
                ),
            })
        }
    }));

    warnings
}

/// Generate effects for a parallel base-correlation shock.
pub(crate) fn base_corr_parallel_effects(
    surface_id: &CurveId,
    points: f64,
    ctx: &ExecutionContext,
) -> Vec<ScenarioEffect> {
    let bump = MarketBump::Curve {
        id: surface_id.clone(),
        spec: BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: points,
            bump_type: BumpType::Parallel,
        },
    };
    let mut effects = vec![ScenarioEffect::MarketBump(bump)];
    effects.extend(
        bump_warnings(ctx, surface_id, None, points)
            .into_iter()
            .map(ScenarioEffect::Warning),
    );
    effects
}

/// Generate effects for a bucketed base-correlation shock.
pub(crate) fn base_corr_bucket_effects(
    surface_id: &CurveId,
    detachment_bps: Option<&[i32]>,
    maturities: Option<&[String]>,
    points: f64,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    if let Some(mats) = maturities {
        if !mats.is_empty() {
            return Err(Error::Validation(
                "BaseCorrBucketPts maturity filtering is not supported: \
                 BaseCorrelationCurve is 1D (detachment only, no term structure). \
                 Remove the maturities field or use detachment_bps alone."
                    .to_string(),
            ));
        }
    }

    let dets = detachment_bps.map(crate::utils::bps_to_fractions);

    let bump = MarketBump::BaseCorrBucketPts {
        surface_id: surface_id.clone(),
        detachments: dets.clone(),
        points,
    };

    let mut effects = vec![ScenarioEffect::MarketBump(bump)];
    effects.extend(
        bump_warnings(ctx, surface_id, dets.as_deref(), points)
            .into_iter()
            .map(ScenarioEffect::Warning),
    );

    Ok(effects)
}
