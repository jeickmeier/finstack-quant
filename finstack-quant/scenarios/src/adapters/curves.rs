//! Curve shock adapters (discount, forward, hazard, inflation, commodity, vol-index).
//!
//! Functions here translate curve-oriented [`OperationSpec`](crate::spec::OperationSpec)
//! variants into [`ScenarioEffect`]s. Curves are rebuilt rather than mutated
//! in place to preserve determinism and metadata such as identifiers and base
//! dates.

use crate::adapters::traits::ScenarioEffect;
use crate::engine::ExecutionContext;
use crate::error::{Error, Result};
use crate::spec::{CurveKind, TenorMatchMode};
use crate::utils::{calculate_interpolation_weights, parse_tenor_to_years_with_context};
use crate::warning::Warning;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount};
use finstack_quant_core::market_data::bumps::{BumpSpec, MarketBump};
use finstack_quant_core::market_data::context::CurveStorage;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::calibration::bumps::{
    bump_discount_curve_synthetic, bump_hazard_spreads, bump_inflation_rates,
    infer_currency_from_curve_id, infer_currency_from_discount_curve_id,
    observation_lag_from_curve, BumpRequest,
};

/// Construct the `MarketDataNotFound` error for a curve that failed to fetch.
fn missing_market_err(curve_id: &str) -> Error {
    Error::MarketDataNotFound {
        id: curve_id.to_string(),
    }
}

/// Build the default effect vector for a curve shock: `UpdateCurve` followed by
/// any warnings accumulated during bump resolution.
fn update_effects<C>(new_curve: C, warnings: Vec<Warning>) -> Vec<ScenarioEffect>
where
    CurveStorage: From<C>,
{
    let mut effects = vec![ScenarioEffect::UpdateCurve(CurveStorage::from(new_curve))];
    effects.extend(warnings.into_iter().map(ScenarioEffect::Warning));
    effects
}

/// Result of resolving bump targets, including any warnings.
struct BumpTargetResult {
    /// Resolved (time, bump_value) pairs for curve knots (used by BumpRequest::Tenors).
    targets: Vec<(f64, f64)>,
    /// Resolved (knot_index, bump_value) pairs for direct curve modification.
    indexed_targets: Vec<(usize, f64)>,
    /// Warnings generated during resolution (e.g., extrapolation).
    warnings: Vec<Warning>,
}

fn resolve_bump_targets(
    curve_id: &str,
    nodes: &[(String, f64)],
    knots: &[f64],
    match_mode: TenorMatchMode,
    as_of: finstack_quant_core::dates::Date,
    day_count: DayCount,
) -> Result<BumpTargetResult> {
    let mut targets = Vec::new();
    let mut indexed_targets = Vec::new();
    let mut warnings = Vec::new();

    let min_knot = knots.first().copied().unwrap_or(0.0);
    let max_knot = knots.last().copied().unwrap_or(0.0);

    for (tenor_str, bp) in nodes {
        let tenor_years_ctx = parse_tenor_to_years_with_context(
            tenor_str,
            as_of,
            None,
            BusinessDayConvention::Unadjusted,
            day_count,
        )?;
        let tenor_years_simple = crate::utils::parse_tenor_to_years(tenor_str)?;

        let add = *bp;

        match match_mode {
            TenorMatchMode::Exact => {
                let match_ctx = knots
                    .iter()
                    .enumerate()
                    .find(|(_, t)| (**t - tenor_years_ctx).abs() < 1e-6);
                let match_simple = knots
                    .iter()
                    .enumerate()
                    .find(|(_, t)| (**t - tenor_years_simple).abs() < 1e-6);

                let (idx, target_years) = match (match_ctx, match_simple) {
                    (Some((i, _)), _) => (i, tenor_years_ctx),
                    (None, Some((i, _))) => (i, tenor_years_simple),
                    (None, None) => {
                        return Err(Error::TenorNotFound {
                            tenor: tenor_str.clone(),
                            curve_id: curve_id.to_string(),
                        })
                    }
                };

                targets.push((target_years, add));
                indexed_targets.push((idx, add));
            }
            TenorMatchMode::Interpolate => {
                let has_exact_ctx = knots.iter().any(|&t| (t - tenor_years_ctx).abs() < 1e-6);
                let has_exact_simple = knots.iter().any(|&t| (t - tenor_years_simple).abs() < 1e-6);

                let use_years = if !has_exact_ctx && has_exact_simple {
                    tenor_years_simple
                } else {
                    tenor_years_ctx
                };

                let result = calculate_interpolation_weights(use_years, knots);

                if result.is_extrapolation {
                    let distance = result.extrapolation_distance.unwrap_or(0.0);
                    warnings.push(Warning::TenorExtrapolated {
                        curve_id: curve_id.to_string(),
                        detail: format!(
                            "Tenor '{tenor_str}' ({use_years:.2}Y) on curve '{curve_id}' extrapolates outside curve range \
                             [{min_knot:.2}Y, {max_knot:.2}Y] by {distance:.2}Y. Using flat extrapolation to nearest pillar."
                        ),
                    });
                }

                // Delivery correction: distributing `add` with raw linear
                // weights (w0 + w1 = 1) realizes only add·(w0² + w1²) at the
                // requested tenor under linear interpolation, since
                // rate(t) = w0·r0 + w1·r1 — half the requested shock at a
                // segment midpoint. Rescale by 1/Σw² (the minimum-norm pillar
                // perturbation) so the interpolated curve moves by exactly
                // `add` at the requested tenor. On-pillar requests have
                // Σw² = 1 and are unchanged; the adjacent pillar can move by
                // at most ~1.21·add (at w ≈ 0.72). Exact for directly-bumped
                // curves (forward, commodity, vol-index, hazard direct shift);
                // first-order for solve-to-par recalibration paths.
                let norm: f64 = result.weights.iter().map(|(_, w)| w * w).sum();
                let scale = if norm > 1e-12 { 1.0 / norm } else { 1.0 };
                for (idx, weight) in result.weights {
                    targets.push((knots[idx], add * weight * scale));
                    indexed_targets.push((idx, add * weight * scale));
                }
            }
        }
    }
    Ok(BumpTargetResult {
        targets,
        indexed_targets,
        warnings,
    })
}

/// Typical parallel-bump stress range for commodity curves.
const COMMODITY_LARGE_SHOCK_MIN_BP: f64 = -5_000.0;
const COMMODITY_LARGE_SHOCK_MAX_BP: f64 = 10_000.0;

fn commodity_shock_warning(curve_id: &CurveId, bp: f64) -> Option<Warning> {
    let range = COMMODITY_LARGE_SHOCK_MIN_BP..=COMMODITY_LARGE_SHOCK_MAX_BP;
    (!range.contains(&bp)).then(|| Warning::CommodityShockOutsideRange {
        curve_id: curve_id.as_str().to_string(),
        detail: format!(
            "Commodity curve '{curve_id}' parallel bump {bp:+.0} bp is outside the typical \
             stress range [{COMMODITY_LARGE_SHOCK_MIN_BP:+.0}, {COMMODITY_LARGE_SHOCK_MAX_BP:+.0}] bp; \
             verify convenience-yield semantics (bp means 1e-4 on the zero rate, not a price shift)."
        ),
    })
}

fn commodity_node_shock_warning(curve_id: &CurveId, nodes: &[(String, f64)]) -> Option<Warning> {
    let range = COMMODITY_LARGE_SHOCK_MIN_BP..=COMMODITY_LARGE_SHOCK_MAX_BP;
    let extreme: Vec<String> = nodes
        .iter()
        .filter(|(_, bp)| !range.contains(bp))
        .map(|(tenor, bp)| format!("{tenor}={bp:+.0}bp"))
        .collect();
    (!extreme.is_empty()).then(|| Warning::CommodityShockOutsideRange {
        curve_id: curve_id.as_str().to_string(),
        detail: format!(
            "Commodity curve '{curve_id}' node shocks outside typical stress range \
             [{COMMODITY_LARGE_SHOCK_MIN_BP:+.0}, {COMMODITY_LARGE_SHOCK_MAX_BP:+.0}] bp: [{}]; \
             verify convenience-yield semantics.",
            extreme.join(", ")
        ),
    })
}

/// Reject parallel VolIndex shocks that would drive any knot — or the spot
/// level — to a non-positive value. Spot is checked with the same hard-error
/// policy as the knots: silently clamping spot to zero would leave the curve
/// internally inconsistent with its term structure.
fn check_vol_index_post_shock_positivity(
    curve_id: &CurveId,
    levels: &[f64],
    spot_level: f64,
    pts: f64,
) -> Result<()> {
    let base_min = levels.iter().copied().fold(spot_level, f64::min);
    if base_min.is_finite() && base_min + pts <= 0.0 {
        return Err(Error::Validation(format!(
            "VolIndex '{curve_id}' parallel shock would produce non-positive level \
             (min of spot/knots {base_min:.4} + shift {pts:+.4} = {:.4}); volatility must stay \
             positive",
            base_min + pts
        )));
    }
    Ok(())
}

/// Resolve the discount curve ID used by recalibration-based curve bumps.
///
/// Walks the live `MarketContext` instead of materialising a serializable
/// snapshot, so this is cheap to call repeatedly.
///
/// # Naming assumption
///
/// The currency heuristic matches on the **first three characters of the hint
/// curve id, uppercase** (e.g. `USD_SOFR` → discount curves starting with
/// `USD`). Curve ids that do not lead with an uppercase ISO currency code
/// (e.g. `sofr-usd`, `OIS-USD`) bypass the heuristic and fall through to the
/// single-curve fallback or an explicit-resolution error. Pass
/// `discount_curve_id` explicitly when curve naming does not follow the
/// `CCY...` convention.
fn resolve_discount_curve_id(
    market: &finstack_quant_core::market_data::context::MarketContext,
    explicit_discount_curve_id: Option<&CurveId>,
    hint_curve_id: Option<&CurveId>,
) -> Result<(CurveId, Option<Warning>)> {
    if let Some(explicit) = explicit_discount_curve_id {
        market
            .get_discount(explicit.as_str())
            .map_err(|_| missing_market_err(explicit.as_str()))?;
        return Ok((explicit.clone(), None));
    }

    let discount_curves: Vec<(CurveId, _)> = market
        .iter_discount_curves()
        .map(|(id, curve)| (id.clone(), curve))
        .collect();

    if discount_curves.is_empty() {
        return Err(Error::Validation(
            "No discount curves are available for recalibration-based scenario bump".into(),
        ));
    }

    if let Some(hint) = hint_curve_id {
        let hint_str = hint.as_str();
        let ccy_prefix = hint_str.get(..3).unwrap_or("");
        if ccy_prefix.len() == 3 && ccy_prefix.chars().all(|c| c.is_ascii_uppercase()) {
            let prefix_matches: Vec<&CurveId> = discount_curves
                .iter()
                .filter_map(|(id, _)| id.as_str().starts_with(ccy_prefix).then_some(id))
                .collect();

            if prefix_matches.len() > 1 {
                return Err(Error::Validation(format!(
                    "Ambiguous discount curve resolution for '{hint}': multiple '{ccy_prefix}' discount curves found",
                )));
            }

            if let Some(discount_id) = prefix_matches.first() {
                let chosen = (*discount_id).clone();
                let reason = format!(
                    "Using heuristic discount curve '{chosen}' for '{hint}'",
                    chosen = chosen.as_str()
                );
                return Ok((
                    chosen.clone(),
                    Some(Warning::DiscountCurveHeuristic {
                        for_curve: hint_str.to_string(),
                        chosen_discount: chosen.as_str().to_string(),
                        reason,
                    }),
                ));
            }
        }
    }

    if discount_curves.len() == 1 {
        let chosen = discount_curves[0].0.clone();
        let reason = format!(
            "Using only available discount curve '{chosen}' as fallback",
            chosen = chosen.as_str()
        );
        let for_curve = hint_curve_id
            .map(|h| h.as_str().to_string())
            .unwrap_or_default();
        return Ok((
            chosen.clone(),
            Some(Warning::DiscountCurveHeuristic {
                for_curve,
                chosen_discount: chosen.as_str().to_string(),
                reason,
            }),
        ));
    }

    let hint_str = hint_curve_id.map(|h| h.as_str()).unwrap_or("curve bump");
    Err(Error::Validation(format!(
        "Unable to resolve discount curve for '{hint_str}' without an explicit discount_curve_id",
    )))
}

/// Generate effects for a parallel curve bump.
pub(crate) fn curve_parallel_effects(
    curve_kind: CurveKind,
    curve_id: &CurveId,
    discount_curve_id: Option<&CurveId>,
    bp: f64,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    let bump_req = BumpRequest::Parallel(bp);
    let as_of = ctx.as_of;

    match curve_kind {
        CurveKind::Discount => {
            let base_curve = ctx
                .market
                .get_discount(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let currency = infer_currency_from_discount_curve_id(&base_curve);
            let new_curve =
                bump_discount_curve_synthetic(&base_curve, ctx.market, &bump_req, as_of, currency)?;

            Ok(vec![ScenarioEffect::UpdateCurve(CurveStorage::from(
                new_curve,
            ))])
        }
        CurveKind::Forward => {
            // Forward curve parallel bump uses direct additive rate shifts.
            // Discount curves use solve-to-par; forward curves apply additive
            // shifts directly because they represent forward rates rather than
            // derived discount factors.
            let _base_curve = ctx
                .market
                .get_forward(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let spec = BumpSpec::parallel_bp(bp);
            let bump = MarketBump::Curve {
                id: curve_id.clone(),
                spec,
            };
            Ok(vec![ScenarioEffect::MarketBump(bump)])
        }
        CurveKind::ParCDS => {
            let base_curve = ctx
                .market
                .get_hazard(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;
            let (discount_id, warning) =
                resolve_discount_curve_id(ctx.market, discount_curve_id, Some(curve_id))?;
            let new_curve =
                bump_hazard_spreads(&base_curve, ctx.market, &bump_req, Some(&discount_id))?;

            let mut effects = vec![ScenarioEffect::UpdateCurve(CurveStorage::from(new_curve))];
            if let Some(w) = warning {
                effects.push(ScenarioEffect::Warning(w));
            }
            Ok(effects)
        }
        CurveKind::Inflation => {
            let base_curve = ctx
                .market
                .get_inflation_curve(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let (discount_id, warning) =
                resolve_discount_curve_id(ctx.market, discount_curve_id, Some(curve_id))?;

            let currency = infer_currency_from_curve_id(&base_curve);
            let lag = observation_lag_from_curve(&base_curve);
            let new_curve = bump_inflation_rates(
                &base_curve,
                ctx.market,
                &bump_req,
                &discount_id,
                as_of,
                currency,
                &lag,
            )?;

            let mut effects = vec![ScenarioEffect::UpdateCurve(CurveStorage::from(new_curve))];
            if let Some(w) = warning {
                effects.push(ScenarioEffect::Warning(w));
            }
            Ok(effects)
        }
        CurveKind::Commodity => {
            let _base_curve = ctx
                .market
                .get_discount(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let spec = BumpSpec::parallel_bp(bp);
            let bump = MarketBump::Curve {
                id: curve_id.clone(),
                spec,
            };
            let mut effects = vec![ScenarioEffect::MarketBump(bump)];
            if let Some(w) = commodity_shock_warning(curve_id, bp) {
                effects.push(ScenarioEffect::Warning(w));
            }
            Ok(effects)
        }
    }
}

/// Generate effects for a node-specific curve bump.
pub(crate) fn curve_node_effects(
    curve_kind: CurveKind,
    curve_id: &CurveId,
    discount_curve_id: Option<&CurveId>,
    nodes: &[(String, f64)],
    match_mode: TenorMatchMode,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    let as_of = ctx.as_of;

    match curve_kind {
        CurveKind::Discount => {
            let base_curve = ctx
                .market
                .get_discount(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots: Vec<f64> = base_curve.knots().to_vec();
            let result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                base_curve.day_count(),
            )?;
            let bump_req = BumpRequest::Tenors(result.targets);

            let currency = infer_currency_from_discount_curve_id(&base_curve);
            let new_curve =
                bump_discount_curve_synthetic(&base_curve, ctx.market, &bump_req, as_of, currency)?;

            Ok(update_effects(new_curve, result.warnings))
        }
        CurveKind::Forward => {
            let base_curve = ctx
                .market
                .get_forward(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots = base_curve.knots().to_vec();
            let mut forwards = base_curve.forwards().to_vec();

            let result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                base_curve.day_count(),
            )?;

            for &(idx, bp) in &result.indexed_targets {
                forwards[idx] += bp * 1e-4;
            }

            let bumped_points: Vec<(f64, f64)> = knots.into_iter().zip(forwards).collect();
            let new_curve =
                finstack_quant_core::market_data::term_structures::ForwardCurve::builder(
                    base_curve.id().as_str(),
                    base_curve.tenor(),
                )
                .base_date(base_curve.base_date())
                .reset_lag(base_curve.reset_lag())
                .day_count(base_curve.day_count())
                .interp(base_curve.interp_style())
                .extrapolation(base_curve.extrapolation())
                .rate_calibration_opt(base_curve.rate_calibration().cloned())
                .fx_policy_opt(base_curve.fx_policy().map(ToOwned::to_owned))
                .knots(bumped_points)
                .build()?;

            Ok(update_effects(new_curve, result.warnings))
        }
        CurveKind::ParCDS => {
            let base_curve = ctx
                .market
                .get_hazard(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots: Vec<f64> = base_curve.knot_points().map(|(t, _)| t).collect();
            let result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                base_curve.day_count(),
            )?;
            let bump_req = BumpRequest::Tenors(result.targets);

            let (discount_id, warning) =
                resolve_discount_curve_id(ctx.market, discount_curve_id, Some(curve_id))?;

            let new_curve =
                bump_hazard_spreads(&base_curve, ctx.market, &bump_req, Some(&discount_id))?;

            let mut effects = vec![ScenarioEffect::UpdateCurve(CurveStorage::from(new_curve))];
            effects.extend(result.warnings.into_iter().map(ScenarioEffect::Warning));
            if let Some(w) = warning {
                effects.push(ScenarioEffect::Warning(w));
            }
            Ok(effects)
        }
        CurveKind::Inflation => {
            let base_curve = ctx
                .market
                .get_inflation_curve(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots: Vec<f64> = base_curve.knots().to_vec();

            let (discount_id, warning) =
                resolve_discount_curve_id(ctx.market, discount_curve_id, Some(curve_id))?;
            let tenor_day_count = ctx
                .market
                .get_discount(discount_id.as_str())
                .map(|dc| dc.day_count())
                .unwrap_or(DayCount::Act365F);

            let result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                tenor_day_count,
            )?;
            let bump_req = BumpRequest::Tenors(result.targets);

            let currency = infer_currency_from_curve_id(&base_curve);
            let lag = observation_lag_from_curve(&base_curve);
            let new_curve = bump_inflation_rates(
                &base_curve,
                ctx.market,
                &bump_req,
                &discount_id,
                as_of,
                currency,
                &lag,
            )?;

            let mut effects = vec![ScenarioEffect::UpdateCurve(CurveStorage::from(new_curve))];
            effects.extend(result.warnings.into_iter().map(ScenarioEffect::Warning));
            if let Some(w) = warning {
                effects.push(ScenarioEffect::Warning(w));
            }
            Ok(effects)
        }
        CurveKind::Commodity => {
            let base_curve = ctx
                .market
                .get_discount(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots: Vec<f64> = base_curve.knots().to_vec();
            let result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                base_curve.day_count(),
            )?;

            let mut dfs: Vec<f64> = base_curve.dfs().to_vec();
            for &(idx, bp_shift) in &result.indexed_targets {
                let t = knots[idx];
                if t > 1e-12 {
                    if dfs[idx] <= 0.0 {
                        tracing::warn!(
                            idx,
                            df = dfs[idx],
                            "Non-positive discount factor in commodity curve bump; skipping node"
                        );
                        continue;
                    }
                    let zero = -(dfs[idx].ln()) / t;
                    let shifted = zero + bp_shift * 1e-4;
                    dfs[idx] = (-shifted * t).exp();
                }
            }

            let bumped_points: Vec<(f64, f64)> = knots.into_iter().zip(dfs).collect();
            let new_curve =
                finstack_quant_core::market_data::term_structures::DiscountCurve::builder(
                    base_curve.id().as_str(),
                )
                .base_date(base_curve.base_date())
                .day_count(base_curve.day_count())
                .interp(base_curve.interp_style())
                .extrapolation(base_curve.extrapolation())
                .validation(
                    finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                        allow_non_monotonic: true,
                        forward_floor: None,
                    },
                )
                .knots(bumped_points)
                .build()?;

            let mut warnings = result.warnings;
            if let Some(w) = commodity_node_shock_warning(curve_id, nodes) {
                warnings.push(w);
            }
            Ok(update_effects(new_curve, warnings))
        }
    }
}

/// Generate effects for a parallel vol-index curve shock (absolute index points).
pub(crate) fn vol_index_parallel_effects(
    curve_id: &CurveId,
    points: f64,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    let base_curve = ctx
        .market
        .get_vol_index_curve(curve_id.as_str())
        .map_err(|_| missing_market_err(curve_id.as_str()))?;

    check_vol_index_post_shock_positivity(
        curve_id,
        base_curve.levels(),
        base_curve.spot_level(),
        points,
    )?;

    // Rebuild with the original ID so `MarketContext::insert` replaces the
    // existing entry rather than adding a parallel "VIX+...bp" copy.
    let knots: Vec<f64> = base_curve.knots().to_vec();
    let bumped_levels: Vec<f64> = base_curve.levels().iter().map(|l| l + points).collect();
    let bumped_points: Vec<(f64, f64)> = knots.into_iter().zip(bumped_levels).collect();
    let new_curve =
        finstack_quant_core::market_data::term_structures::VolatilityIndexCurve::builder(
            base_curve.id().as_str(),
        )
        .base_date(base_curve.base_date())
        .day_count(base_curve.day_count())
        .spot_level(base_curve.spot_level() + points)
        .interp(base_curve.interp_style())
        .extrapolation(base_curve.extrapolation())
        .knots(bumped_points)
        .build()?;

    Ok(vec![ScenarioEffect::UpdateCurve(CurveStorage::from(
        new_curve,
    ))])
}

/// Generate effects for a node-specific vol-index curve shock (absolute index points).
pub(crate) fn vol_index_node_effects(
    curve_id: &CurveId,
    nodes: &[(String, f64)],
    match_mode: TenorMatchMode,
    ctx: &ExecutionContext,
) -> Result<Vec<ScenarioEffect>> {
    let as_of = ctx.as_of;
    let base_curve = ctx
        .market
        .get_vol_index_curve(curve_id.as_str())
        .map_err(|_| missing_market_err(curve_id.as_str()))?;

    let knots: Vec<f64> = base_curve.knots().to_vec();
    let result = resolve_bump_targets(
        curve_id.as_str(),
        nodes,
        &knots,
        match_mode,
        as_of,
        base_curve.day_count(),
    )?;

    let mut levels: Vec<f64> = base_curve.levels().to_vec();

    for &(idx, pts) in &result.indexed_targets {
        let proposed = levels[idx] + pts;
        if proposed <= 0.0 {
            return Err(Error::Validation(format!(
                "VolIndex '{curve_id}' node shock at knot[{idx}] would \
                 produce non-positive level (base {:.4} + shift {:+.4} = \
                 {:.4}); volatility must stay positive",
                levels[idx], pts, proposed,
            )));
        }
        levels[idx] = proposed;
    }

    let bumped_points: Vec<(f64, f64)> = knots.into_iter().zip(levels).collect();
    let new_curve =
        finstack_quant_core::market_data::term_structures::VolatilityIndexCurve::builder(
            base_curve.id().as_str(),
        )
        .base_date(base_curve.base_date())
        .day_count(base_curve.day_count())
        .spot_level(base_curve.spot_level())
        .interp(base_curve.interp_style())
        .extrapolation(base_curve.extrapolation())
        .knots(bumped_points)
        .build()?;

    Ok(update_effects(new_curve, result.warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ScenarioEngine;
    use crate::spec::{OperationSpec, ScenarioSpec};
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::VolatilityIndexCurve;
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use finstack_quant_statements::FinancialModelSpec;
    use time::macros::date;

    #[test]
    fn vol_index_parallel_uses_absolute_index_points() {
        let as_of = date!(2025 - 01 - 01);
        let vol_curve = VolatilityIndexCurve::builder("VIX")
            .base_date(as_of)
            .spot_level(18.5)
            .knots([(0.0, 18.5), (0.25, 20.0), (0.5, 21.5)])
            .build()
            .expect("vol index curve should build");
        let mut market = MarketContext::new().insert(vol_curve);
        let mut model = FinancialModelSpec::new("demo", vec![]);

        let scenario = ScenarioSpec {
            id: "vol".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::VolIndexParallelPts {
                curve_id: "VIX".into(),
                points: 1.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        let engine = ScenarioEngine::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        engine.apply(&scenario, &mut ctx).expect("should apply");

        let updated = market
            .get_vol_index_curve("VIX")
            .expect("vol index should exist");
        assert!((updated.spot_level() - 19.5).abs() < 1.0e-12);
        assert!((updated.forward_level(0.25) - 21.0).abs() < 1.0e-12);
    }

    #[test]
    fn curve_shocks_preserve_forward_and_vol_index_metadata() {
        use finstack_quant_core::market_data::term_structures::ForwardCurve;

        let as_of = date!(2025 - 01 - 01);
        let forward = ForwardCurve::builder("USD-SOFR", 0.25)
            .base_date(as_of)
            .reset_lag(5)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::CubicHermite)
            .extrapolation(ExtrapolationPolicy::None)
            .fx_policy("preserve-me")
            .knots([(0.0, 0.02), (0.25, 0.021), (0.5, 0.022)])
            .build()
            .expect("forward curve should build");
        let vol = VolatilityIndexCurve::builder("VIX")
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .spot_level(18.0)
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .knots([(0.0, 18.0), (0.25, 20.0), (0.5, 22.0)])
            .build()
            .expect("vol-index curve should build");
        let mut market = MarketContext::new().insert(forward).insert(vol);
        let mut model = FinancialModelSpec::new("demo", vec![]);
        let scenario = ScenarioSpec {
            id: "metadata".into(),
            name: None,
            description: None,
            operations: vec![
                OperationSpec::CurveNodeBp {
                    curve_kind: CurveKind::Forward,
                    curve_id: "USD-SOFR".into(),
                    discount_curve_id: None,
                    nodes: vec![("3M".into(), 1.0)],
                    match_mode: TenorMatchMode::Exact,
                },
                OperationSpec::VolIndexNodePts {
                    curve_id: "VIX".into(),
                    nodes: vec![("3M".into(), 1.0)],
                    match_mode: TenorMatchMode::Exact,
                },
            ],
            priority: 0,
            resolution_mode: Default::default(),
        };
        let engine = ScenarioEngine::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        engine.apply(&scenario, &mut ctx).expect("should apply");

        let updated_forward = market.get_forward("USD-SOFR").expect("forward exists");
        assert_eq!(updated_forward.reset_lag(), 5);
        assert_eq!(updated_forward.day_count(), DayCount::Act365F);
        assert_eq!(updated_forward.interp_style(), InterpStyle::CubicHermite);
        assert_eq!(updated_forward.extrapolation(), ExtrapolationPolicy::None);
        assert_eq!(updated_forward.fx_policy(), Some("preserve-me"));

        let updated_vol = market.get_vol_index_curve("VIX").expect("vol exists");
        assert_eq!(updated_vol.day_count(), DayCount::Act360);
        assert_eq!(updated_vol.interp_style(), InterpStyle::LogLinear);
        assert_eq!(
            updated_vol.extrapolation(),
            ExtrapolationPolicy::FlatForward
        );
    }

    #[test]
    fn vol_index_parallel_rejects_non_positive_floor() {
        let as_of = date!(2025 - 01 - 01);
        let vol_curve = VolatilityIndexCurve::builder("VIX")
            .base_date(as_of)
            .spot_level(15.0)
            .knots([(0.0, 15.0), (0.25, 16.0), (0.5, 18.0)])
            .build()
            .expect("vol index curve should build");
        let mut market = MarketContext::new().insert(vol_curve);
        let mut model = FinancialModelSpec::new("demo", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };

        let curve_id = CurveId::from("VIX");
        let err = vol_index_parallel_effects(&curve_id, -15.0, &ctx)
            .expect_err("shock to zero must be rejected");
        assert!(err.to_string().contains("non-positive level"));
    }

    #[test]
    fn commodity_parallel_large_shock_emits_warning() {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;

        let as_of = date!(2025 - 01 - 01);
        let curve = DiscountCurve::builder("WTI")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, 0.97), (5.0, 0.85)])
            .build()
            .expect("commodity curve should build");

        let mut market = MarketContext::new().insert(curve);
        let mut model = FinancialModelSpec::new("demo", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };

        let curve_id = CurveId::from("WTI");
        let effects = curve_parallel_effects(CurveKind::Commodity, &curve_id, None, 50_000.0, &ctx)
            .expect("commodity shock should be handled");

        let has_warning = effects.iter().any(|e| {
            matches!(
                e,
                ScenarioEffect::Warning(Warning::CommodityShockOutsideRange { .. })
            )
        });
        assert!(has_warning, "expected large-shock warning");
    }
}
