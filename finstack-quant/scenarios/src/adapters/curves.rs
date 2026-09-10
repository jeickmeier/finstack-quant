//! Curve shock adapters (discount, forward, hazard, inflation, commodity, vol-index).
//!
//! Functions here translate curve-oriented [`OperationSpec`](crate::spec::OperationSpec)
//! variants into [`ScenarioEffect`]s. Curves are rebuilt rather than mutated
//! in place to preserve determinism and metadata such as identifiers and base
//! dates.

use crate::adapters::traits::ScenarioEffect;
use crate::engine::{ExecutionContext, HazardApplyEnv};
use crate::error::{Error, Result};
use crate::spec::{CurveKind, HazardBumpMode, OperationSpec, TenorMatchMode};
use crate::utils::calculate_interpolation_weights;
use crate::warning::Warning;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, Tenor};
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable, MarketBump,
};
use finstack_quant_core::market_data::context::{CurveStorage, MarketContext};
use finstack_quant_core::market_data::term_structures::{
    DiscountCurve, ForwardCurve, InflationCurve, PriceCurve, PriceCurveKind,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations::recalibration::{
    HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump,
};

/// Shared market snapshot for curve-effect generation without a mutable context.
struct CurveApplyCtx<'a> {
    market: &'a MarketContext,
    as_of: Date,
    env: &'a HazardApplyEnv<'a>,
}

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

/// Build the core triangular key-rate bump centered on one native knot.
fn key_rate_spec(knots: &[f64], index: usize, bump_bp: f64) -> Option<BumpSpec> {
    let &target = knots.get(index)?;
    Some(match (index.checked_sub(1), knots.get(index + 1)) {
        (None, Some(&next)) => BumpSpec::triangular_key_rate_first_bp(target, next, bump_bp),
        (Some(previous), None) => {
            BumpSpec::triangular_key_rate_last_bp(knots[previous], target, bump_bp)
        }
        (Some(previous), Some(&next)) => {
            BumpSpec::triangular_key_rate_bp(knots[previous], target, next, bump_bp)
        }
        (None, None) => BumpSpec::parallel_bp(bump_bp),
    })
}

/// Convert resolved curve-node shocks into core triangular key-rate bumps.
fn node_market_bump_effects(
    curve_id: &CurveId,
    knots: &[f64],
    indexed_targets: &[(usize, f64)],
    warnings: Vec<Warning>,
) -> Vec<ScenarioEffect> {
    let mut effects = Vec::with_capacity(indexed_targets.len() + warnings.len());
    for &(index, bump_bp) in indexed_targets {
        let Some(spec) = key_rate_spec(knots, index, bump_bp) else {
            continue;
        };
        effects.push(ScenarioEffect::MarketBump(MarketBump::Curve {
            id: curve_id.clone(),
            spec,
        }));
    }
    effects.extend(warnings.into_iter().map(ScenarioEffect::Warning));
    effects
}

/// Result of resolving bump targets, including any warnings.
struct BumpTargetResult {
    /// Resolved `(time, bump_value)` pairs for direct or quote-space delivery.
    targets: Vec<(f64, f64)>,
    /// Resolved (knot_index, bump_value) pairs for direct curve modification.
    indexed_targets: Vec<(usize, f64)>,
    /// Warnings generated during resolution (e.g., extrapolation).
    warnings: Vec<Warning>,
    /// Off-pillar interpolate requests that need native-interpolant calibration.
    interpolate_hits: Vec<InterpolateHit>,
}

/// One off-pillar interpolate request: target time, requested add, neighbor pillars.
struct InterpolateHit {
    t: f64,
    requested: f64,
}

/// Whether the bumped curve is rebuilt by solve-to-par recalibration
/// (ParCDS only) rather than a direct knot shift. Direct-shift curves
/// calibrate interpolated splits onto the live interpolant. Solve-to-par
/// splits emit a [`Warning::InterpolatedNodeBumpFirstOrder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BumpDelivery {
    /// Curve knots are shifted directly; interpolated splits are calibrated
    /// onto the curve's native interpolant.
    Direct,
    /// Shifted targets are snapped to calibration quotes and re-solved to par.
    SolveToPar,
}

fn resolve_bump_targets(
    curve_id: &str,
    nodes: &[(String, f64)],
    knots: &[f64],
    match_mode: TenorMatchMode,
    as_of: finstack_quant_core::dates::Date,
    day_count: DayCount,
    delivery: BumpDelivery,
) -> Result<BumpTargetResult> {
    let mut targets = Vec::new();
    let mut indexed_targets = Vec::new();
    let mut warnings = Vec::new();
    let mut interpolate_hits = Vec::new();

    let min_knot = knots.first().copied().unwrap_or(0.0);
    let max_knot = knots.last().copied().unwrap_or(0.0);

    for (tenor_str, bp) in nodes {
        // `Tenor::parse` rejects a zero count, but node shocks need a way to
        // address the t=0 front knot (vol-index / commodity spot sync).
        let (tenor_years_ctx, tenor_years_simple) = if tenor_str == "0Y" {
            (0.0, 0.0)
        } else {
            let tenor = Tenor::parse(tenor_str).map_err(|e| Error::InvalidTenor(e.to_string()))?;
            let tenor_years_ctx = tenor
                .to_years_with_context(as_of, None, BusinessDayConvention::Unadjusted, day_count)
                .map_err(|e| Error::Internal(e.to_string()))?;
            (tenor_years_ctx, tenor.to_years())
        };

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

                // Initial guess: 1/Σw² minimum-norm split so a linear-on-rate
                // interpolant would already hit `add`. Direct-shift curves then
                // calibrate these pillar deltas onto the live interpolant.
                // Solve-to-par (ParCDS) keeps the first-order split and warns.
                let norm: f64 = result.weights.iter().map(|(_, w)| w * w).sum();
                let scale = if norm > 1e-12 { 1.0 / norm } else { 1.0 };
                if result.weights.len() > 1 && delivery == BumpDelivery::SolveToPar {
                    let pillars: Vec<String> = result
                        .weights
                        .iter()
                        .map(|(idx, _)| format!("{:.2}Y", knots[*idx]))
                        .collect();
                    warnings.push(Warning::InterpolatedNodeBumpFirstOrder {
                        curve_id: curve_id.to_string(),
                        detail: format!(
                            "Tenor '{tenor_str}' on curve '{curve_id}' falls between pillars \
                             [{}] and the curve is rebuilt by par-CDS solve-to-par \
                             recalibration: the interpolated-split delivery correction is only \
                             first-order and the split targets are snapped to the nearest \
                             calibration quotes, so the realized shock at '{tenor_str}' may \
                             differ from the requested size. Use TenorMatchMode::Exact at \
                             pillar tenors for pillar-accurate bucket risk.",
                            pillars.join(", ")
                        ),
                    });
                }
                if delivery == BumpDelivery::Direct {
                    interpolate_hits.push(InterpolateHit {
                        t: use_years,
                        requested: add,
                    });
                }
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
        interpolate_hits,
    })
}

const INTERPOLANT_CALIBRATE_TOL: f64 = 1e-12;

fn indexed_to_dense(indexed: &[(usize, f64)], n: usize) -> Vec<f64> {
    let mut deltas = vec![0.0; n];
    for &(idx, bp) in indexed {
        if idx < n {
            deltas[idx] += bp;
        }
    }
    deltas
}

fn write_dense_targets(result: &mut BumpTargetResult, deltas: &[f64], knots: &[f64]) {
    result.indexed_targets.clear();
    result.targets.clear();
    for (idx, &bp) in deltas.iter().enumerate() {
        if bp.abs() > 0.0 {
            result.indexed_targets.push((idx, bp));
            result.targets.push((knots[idx], bp));
        }
    }
}

/// Calibrate Direct interpolate splits so the live interpolant hits each request.
fn calibrate_native_interpolant<Eval, Target>(
    result: &mut BumpTargetResult,
    knots: &[f64],
    curve_id: &str,
    quantity_at: Eval,
    target_at: Target,
) -> Result<()>
where
    Eval: Fn(&[f64], f64) -> Result<f64>,
    Target: Fn(f64, f64) -> Result<f64>,
{
    if result.interpolate_hits.is_empty() {
        return Ok(());
    }

    // Solve all requests together: correcting one shared pillar must not
    // invalidate a previously accepted tenor. Duplicate requests are additive.
    let mut targets: Vec<(f64, f64)> = Vec::new();
    for hit in &result.interpolate_hits {
        if let Some((_, bump)) = targets.iter_mut().find(|(t, _)| (*t - hit.t).abs() < 1e-12) {
            *bump += hit.requested;
        } else {
            targets.push((hit.t, hit.requested));
        }
    }
    let targets = targets
        .into_iter()
        .map(|(t, bump)| Ok((t, target_at(t, bump)?)))
        .collect::<Result<Vec<_>>>()?;
    let initial = indexed_to_dense(&result.indexed_targets, knots.len());
    let mut active: Vec<usize> = result.indexed_targets.iter().map(|(i, _)| *i).collect();
    active.sort_unstable();
    active.dedup();
    let expand = |params: &[f64]| {
        let mut deltas = vec![0.0; knots.len()];
        for (&index, &value) in active.iter().zip(params) {
            deltas[index] = value;
        }
        deltas
    };
    let residuals = |params: &[f64], out: &mut [f64]| {
        let deltas = expand(params);
        for ((t, target), residual) in targets.iter().zip(out) {
            *residual = quantity_at(&deltas, *t).map_or(f64::NAN, |value| value - target);
        }
    };
    let initial: Vec<f64> = active.iter().map(|&i| initial[i]).collect();
    let solution = finstack_quant_core::math::solver_multi::LevenbergMarquardtSolver::new()
        .with_tolerance(INTERPOLANT_CALIBRATE_TOL * 0.1)
        .solve_system_with_dim_stats(residuals, &initial, targets.len())?;
    let deltas = expand(&solution.params);
    // Solver termination alone is not acceptance: check every requested point
    // against the final rebuilt native interpolant, including exact pillars.
    for (t, target) in targets {
        let realized = quantity_at(&deltas, t)?;
        if !realized.is_finite() || (realized - target).abs() > INTERPOLANT_CALIBRATE_TOL {
            return Err(Error::Validation(format!(
                "Native interpolant delivery on '{curve_id}' at t={t:.6} did not converge \
                 (target {target:.12}, realized {realized:.12})"
            )));
        }
    }

    write_dense_targets(result, &deltas, knots);
    Ok(())
}

fn preview_discount_zero(base: &DiscountCurve, deltas_bp: &[f64], t: f64) -> Result<f64> {
    let mut preview = base.clone();
    for (index, bump_bp) in deltas_bp.iter().copied().enumerate() {
        if bump_bp.abs() <= f64::EPSILON {
            continue;
        }
        let Some(spec) = key_rate_spec(base.knots(), index, bump_bp) else {
            continue;
        };
        preview = preview.apply_bump(spec)?;
    }
    Ok(preview.zero(t))
}

fn implied_inflation_rate(curve: &InflationCurve, t: f64) -> Result<f64> {
    if t <= 0.0 {
        return Ok(0.0);
    }
    Ok(curve.inflation_rate(0.0, t)?)
}

fn preview_inflation_implied(base: &InflationCurve, deltas_bp: &[f64], t: f64) -> Result<f64> {
    let mut preview = base.clone();
    for (index, bump_bp) in deltas_bp.iter().copied().enumerate() {
        if bump_bp.abs() <= f64::EPSILON {
            continue;
        }
        let Some(spec) = key_rate_spec(base.knots(), index, bump_bp) else {
            continue;
        };
        preview = preview.apply_bump(spec)?;
    }
    implied_inflation_rate(&preview, t)
}

fn rebuild_forward_curve(base: &ForwardCurve, bumped: Vec<(f64, f64)>) -> Result<ForwardCurve> {
    Ok(ForwardCurve::builder(base.id().as_str(), base.tenor())
        .base_date(base.base_date())
        .reset_lag(base.reset_lag())
        .day_count(base.day_count())
        .interp(base.interp_style())
        .extrapolation(base.extrapolation())
        .rate_calibration_opt(base.rate_calibration().cloned())
        .fx_policy_opt(base.fx_policy().map(ToOwned::to_owned))
        .knots(bumped)
        .build()?)
}

fn preview_forward_rate(base: &ForwardCurve, deltas_bp: &[f64], t: f64) -> Result<f64> {
    let knots = base.knots();
    let bumped: Vec<(f64, f64)> = knots
        .iter()
        .zip(base.forwards().iter())
        .zip(deltas_bp.iter())
        .map(|((&tk, &fwd), &bp)| (tk, fwd + bp * 1e-4))
        .collect();
    Ok(rebuild_forward_curve(base, bumped)?.rate(t))
}

fn rebuild_price_curve(
    base: &PriceCurve,
    bumped: Vec<(f64, f64)>,
    spot: f64,
) -> Result<PriceCurve> {
    Ok(PriceCurve::builder(base.id().as_str())
        .base_date(base.base_date())
        .day_count(base.day_count())
        .spot_price(spot)
        .interp(base.interp_style())
        .extrapolation(base.extrapolation())
        .knots(bumped)
        .build()?)
}

fn preview_commodity_price(base: &PriceCurve, deltas_pct: &[f64], t: f64) -> Result<f64> {
    let knots = base.knots();
    let bumped: Vec<(f64, f64)> = knots
        .iter()
        .zip(base.prices().iter())
        .zip(deltas_pct.iter())
        .map(|((&tk, &px), &pct)| (tk, px * (1.0 + pct / 100.0)))
        .collect();
    let spot = if knots.first().is_some_and(|k| k.abs() < 1e-12) {
        bumped[0].1
    } else {
        base.spot_price()
    };
    Ok(rebuild_price_curve(base, bumped, spot)?.price(t))
}

/// Typical percent-of-forward stress range for commodity price curves.
const COMMODITY_LARGE_SHOCK_MIN_PCT: f64 = -80.0;
const COMMODITY_LARGE_SHOCK_MAX_PCT: f64 = 200.0;

fn commodity_shock_warning(curve_id: &CurveId, pct: f64) -> Option<Warning> {
    let range = COMMODITY_LARGE_SHOCK_MIN_PCT..=COMMODITY_LARGE_SHOCK_MAX_PCT;
    (!range.contains(&pct)).then(|| Warning::CommodityShockOutsideRange {
        curve_id: curve_id.as_str().to_string(),
        detail: format!(
            "Commodity curve '{curve_id}' parallel bump {pct:+.1} percent of the price-curve \
             forward is outside the typical stress range \
             [{COMMODITY_LARGE_SHOCK_MIN_PCT:+.0}, {COMMODITY_LARGE_SHOCK_MAX_PCT:+.0}] percent."
        ),
    })
}

fn commodity_node_shock_warning(curve_id: &CurveId, nodes: &[(String, f64)]) -> Option<Warning> {
    let range = COMMODITY_LARGE_SHOCK_MIN_PCT..=COMMODITY_LARGE_SHOCK_MAX_PCT;
    let extreme: Vec<String> = nodes
        .iter()
        .filter(|(_, pct)| !range.contains(pct))
        .map(|(tenor, pct)| format!("{tenor}={pct:+.1}%"))
        .collect();
    (!extreme.is_empty()).then(|| Warning::CommodityShockOutsideRange {
        curve_id: curve_id.as_str().to_string(),
        detail: format!(
            "Commodity curve '{curve_id}' node shocks outside typical stress range \
             [{COMMODITY_LARGE_SHOCK_MIN_PCT:+.0}, {COMMODITY_LARGE_SHOCK_MAX_PCT:+.0}] percent \
             of the price-curve forward: [{}].",
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
pub(crate) fn resolve_discount_curve_id(
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

fn par_cds_effects(
    curve_id: &CurveId,
    discount_curve_id: Option<&CurveId>,
    bump_req: &QuoteBump,
    market: &MarketContext,
    extra_warnings: Vec<Warning>,
    env: &HazardApplyEnv<'_>,
) -> Result<Vec<ScenarioEffect>> {
    let base_curve = market
        .get_hazard(curve_id.as_str())
        .map_err(|_| missing_market_err(curve_id.as_str()))?;
    match env.mode {
        HazardBumpMode::FirstOrderShift => {
            bump_req.validate()?;
            if bump_req.is_zero() {
                return Ok(update_effects(base_curve.as_ref().clone(), extra_warnings));
            }
            let loss_given_default = 1.0 - base_curve.recovery_rate();
            if loss_given_default <= 0.0 {
                return Err(Error::Validation(format!(
                    "first-order par-spread shock requires recovery below one for '{curve_id}'"
                )));
            }
            let new_curve = match bump_req {
                QuoteBump::ParallelBp(bp) => {
                    base_curve.with_parallel_hazard_rate_bump_bp(*bp / loss_given_default)?
                }
                QuoteBump::TenorsBp(targets) => {
                    let hazard_targets: Vec<_> = targets
                        .iter()
                        .map(|(tenor, bp)| (*tenor, *bp / loss_given_default))
                        .collect();
                    base_curve.with_tenor_hazard_rate_bumps_bp(&hazard_targets)?
                }
            };
            let mut warnings = extra_warnings;
            warnings.push(Warning::HazardSpreadFirstOrder {
                curve_id: curve_id.to_string(),
                recovery_rate: base_curve.recovery_rate(),
            });
            Ok(update_effects(new_curve, warnings))
        }
        HazardBumpMode::SolveToPar => {
            let provider = env.provider.ok_or_else(|| {
                Error::Core(finstack_quant_valuations::recalibration::provider_missing(
                    "par_cds_scenario",
                ))
            })?;
            let (discount_id, warning) =
                resolve_discount_curve_id(market, discount_curve_id, Some(curve_id))?;
            let target_market = std::sync::Arc::new(market.clone());
            let source_market = env
                .source_markets
                .and_then(|markets| markets.get(curve_id))
                .cloned()
                .unwrap_or_else(|| std::sync::Arc::clone(&target_market));
            let new_curve = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
                hazard: base_curve,
                source_market: std::sync::Arc::clone(&source_market),
                target_market,
                discount_curve_id: discount_id,
                doc_clause: None,
                cds_valuation_convention: None,
                deal_quote_override: None,
                action: HazardRecalibrationAction::SpreadBump(bump_req.clone()),
            })?;
            let mut effects = vec![ScenarioEffect::UpdateCurve(CurveStorage::from(
                new_curve.as_ref().clone(),
            ))];
            effects.extend(extra_warnings.into_iter().map(ScenarioEffect::Warning));
            if let Some(w) = warning {
                effects.push(ScenarioEffect::Warning(w));
            }
            Ok(effects)
        }
    }
}

/// Generate ParCDS / inflation replacement effects from a shared market snapshot.
pub(crate) fn generate_replace_curve_effects(
    op: &OperationSpec,
    market: &MarketContext,
    as_of: Date,
    env: &HazardApplyEnv<'_>,
) -> Result<Vec<ScenarioEffect>> {
    match op {
        OperationSpec::CurveParallelBp {
            curve_kind,
            curve_id,
            discount_curve_id,
            bp,
        } => curve_parallel_effects_on(
            *curve_kind,
            curve_id,
            discount_curve_id.as_ref(),
            *bp,
            &CurveApplyCtx { market, as_of, env },
        ),
        OperationSpec::CurveNodeBp {
            curve_kind,
            curve_id,
            discount_curve_id,
            nodes,
            match_mode,
        } => curve_node_effects_on(
            *curve_kind,
            curve_id,
            discount_curve_id.as_ref(),
            nodes,
            *match_mode,
            &CurveApplyCtx { market, as_of, env },
        ),
        _ => Err(Error::Internal(format!(
            "parallel replace-curve dispatch received a non-replace op: {op:?}"
        ))),
    }
}

/// Generate effects for a parallel curve bump.
pub(crate) fn curve_parallel_effects(
    curve_kind: CurveKind,
    curve_id: &CurveId,
    discount_curve_id: Option<&CurveId>,
    bp: f64,
    ctx: &ExecutionContext,
    env: &HazardApplyEnv<'_>,
) -> Result<Vec<ScenarioEffect>> {
    curve_parallel_effects_on(
        curve_kind,
        curve_id,
        discount_curve_id,
        bp,
        &CurveApplyCtx {
            market: ctx.market,
            as_of: ctx.as_of,
            env,
        },
    )
}

fn curve_parallel_effects_on(
    curve_kind: CurveKind,
    curve_id: &CurveId,
    discount_curve_id: Option<&CurveId>,
    bp: f64,
    ctx: &CurveApplyCtx<'_>,
) -> Result<Vec<ScenarioEffect>> {
    let market = ctx.market;
    let env = ctx.env;
    let bump_req = QuoteBump::ParallelBp(bp);

    match curve_kind {
        CurveKind::Discount => {
            let _base_curve = market
                .get_discount(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;
            Ok(vec![ScenarioEffect::MarketBump(MarketBump::Curve {
                id: curve_id.clone(),
                spec: BumpSpec::parallel_bp(bp),
            })])
        }
        CurveKind::Forward => {
            // Forward curve parallel bump uses direct additive rate shifts.
            // Discount parallel bumps are continuous-zero shifts
            // (`DF' = DF · exp(−δ t)`), not solve-to-par quote re-bootstraps.
            let _base_curve = market
                .get_forward(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let spec = BumpSpec::parallel_bp(bp);
            let bump = MarketBump::Curve {
                id: curve_id.clone(),
                spec,
            };
            Ok(vec![ScenarioEffect::MarketBump(bump)])
        }
        CurveKind::ParCDS => par_cds_effects(
            curve_id,
            discount_curve_id,
            &bump_req,
            market,
            Vec::new(),
            env,
        ),
        CurveKind::Inflation => {
            let _base_curve = market
                .get_inflation_curve(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;
            Ok(vec![ScenarioEffect::MarketBump(MarketBump::Curve {
                id: curve_id.clone(),
                spec: BumpSpec::parallel_bp(bp),
            })])
        }
        CurveKind::Commodity => {
            let _base_curve = market
                .get_price_curve(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let spec = BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: bp,
                bump_type: BumpType::Parallel,
            };
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
    env: &HazardApplyEnv<'_>,
) -> Result<Vec<ScenarioEffect>> {
    curve_node_effects_on(
        curve_kind,
        curve_id,
        discount_curve_id,
        nodes,
        match_mode,
        &CurveApplyCtx {
            market: ctx.market,
            as_of: ctx.as_of,
            env,
        },
    )
}

fn curve_node_effects_on(
    curve_kind: CurveKind,
    curve_id: &CurveId,
    discount_curve_id: Option<&CurveId>,
    nodes: &[(String, f64)],
    match_mode: TenorMatchMode,
    ctx: &CurveApplyCtx<'_>,
) -> Result<Vec<ScenarioEffect>> {
    let market = ctx.market;
    let as_of = ctx.as_of;
    let env = ctx.env;
    match curve_kind {
        CurveKind::Discount => {
            let base_curve = market
                .get_discount(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots: Vec<f64> = base_curve.knots().to_vec();
            let mut result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                base_curve.day_count(),
                BumpDelivery::Direct,
            )?;
            calibrate_native_interpolant(
                &mut result,
                &knots,
                curve_id.as_str(),
                |d, t| preview_discount_zero(&base_curve, d, t),
                |t, bp| Ok(base_curve.zero(t) + bp * 1e-4),
            )?;
            Ok(node_market_bump_effects(
                curve_id,
                &knots,
                &result.indexed_targets,
                result.warnings,
            ))
        }
        CurveKind::Forward => {
            let base_curve = market
                .get_forward(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots = base_curve.knots().to_vec();
            let mut forwards = base_curve.forwards().to_vec();

            let mut result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                base_curve.day_count(),
                BumpDelivery::Direct,
            )?;
            calibrate_native_interpolant(
                &mut result,
                &knots,
                curve_id.as_str(),
                |d, t| preview_forward_rate(&base_curve, d, t),
                |t, bp| Ok(base_curve.rate(t) + bp * 1e-4),
            )?;

            for &(idx, bp) in &result.indexed_targets {
                forwards[idx] += bp * 1e-4;
            }

            let bumped_points: Vec<(f64, f64)> = knots.into_iter().zip(forwards).collect();
            let new_curve = rebuild_forward_curve(&base_curve, bumped_points)?;

            Ok(update_effects(new_curve, result.warnings))
        }
        CurveKind::ParCDS => {
            let base_curve = market
                .get_hazard(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots: Vec<f64> = base_curve.knot_points().map(|(t, _)| t).collect();
            let delivery = match env.mode {
                HazardBumpMode::SolveToPar => BumpDelivery::SolveToPar,
                HazardBumpMode::FirstOrderShift => BumpDelivery::Direct,
            };
            let result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                base_curve.day_count(),
                delivery,
            )?;
            let bump_req = QuoteBump::TenorsBp(result.targets);
            par_cds_effects(
                curve_id,
                discount_curve_id,
                &bump_req,
                market,
                result.warnings,
                env,
            )
        }
        CurveKind::Inflation => {
            let base_curve = market
                .get_inflation_curve(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots: Vec<f64> = base_curve.knots().to_vec();

            let mut result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                base_curve.day_count(),
                BumpDelivery::Direct,
            )?;
            calibrate_native_interpolant(
                &mut result,
                &knots,
                curve_id.as_str(),
                |d, t| preview_inflation_implied(&base_curve, d, t),
                |t, bp| Ok(implied_inflation_rate(&base_curve, t)? + bp * 1e-4),
            )?;
            Ok(node_market_bump_effects(
                curve_id,
                &knots,
                &result.indexed_targets,
                result.warnings,
            ))
        }
        CurveKind::Commodity => {
            let base_curve = market
                .get_price_curve(curve_id.as_str())
                .map_err(|_| missing_market_err(curve_id.as_str()))?;

            let knots: Vec<f64> = base_curve.knots().to_vec();
            let mut result = resolve_bump_targets(
                curve_id.as_str(),
                nodes,
                &knots,
                match_mode,
                as_of,
                base_curve.day_count(),
                BumpDelivery::Direct,
            )?;
            calibrate_native_interpolant(
                &mut result,
                &knots,
                curve_id.as_str(),
                |d, t| preview_commodity_price(&base_curve, d, t),
                |t, pct| Ok(base_curve.price(t) * (1.0 + pct / 100.0)),
            )?;

            let mut prices: Vec<f64> = base_curve.prices().to_vec();
            for &(idx, pct) in &result.indexed_targets {
                prices[idx] *= 1.0 + pct / 100.0;
            }
            let mut spot = base_curve.spot_price();
            if knots.first().is_some_and(|k| k.abs() < 1e-12) {
                spot = prices[0];
            }

            let bumped_points: Vec<(f64, f64)> = knots.into_iter().zip(prices).collect();
            let new_curve = rebuild_price_curve(&base_curve, bumped_points, spot)?;

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
        base_curve.prices(),
        base_curve.spot_price(),
        points,
    )?;

    // Rebuild with the original ID so `MarketContext::insert` replaces the
    // existing entry rather than adding a parallel "VIX+...bp" copy.
    let knots: Vec<f64> = base_curve.knots().to_vec();
    let bumped_levels: Vec<f64> = base_curve.prices().iter().map(|l| l + points).collect();
    let bumped_points: Vec<(f64, f64)> = knots.into_iter().zip(bumped_levels).collect();
    let new_curve = finstack_quant_core::market_data::term_structures::PriceCurve::builder(
        base_curve.id().as_str(),
    )
    .kind(PriceCurveKind::VolIndex)
    .base_date(base_curve.base_date())
    .day_count(base_curve.day_count())
    .spot_price(base_curve.spot_price() + points)
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
        BumpDelivery::Direct,
    )?;

    let mut levels: Vec<f64> = base_curve.prices().to_vec();

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

    let mut spot_level = base_curve.spot_price();
    if knots.first().is_some_and(|k| k.abs() < 1e-12) {
        spot_level = levels[0];
    }

    let bumped_points: Vec<(f64, f64)> = knots.into_iter().zip(levels).collect();
    let new_curve = finstack_quant_core::market_data::term_structures::PriceCurve::builder(
        base_curve.id().as_str(),
    )
    .kind(PriceCurveKind::VolIndex)
    .base_date(base_curve.base_date())
    .day_count(base_curve.day_count())
    .spot_price(spot_level)
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
    use finstack_quant_calibration::recalibration::CachedRecalibrationProvider;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::{PriceCurve, PriceCurveKind};
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use finstack_quant_statements::FinancialModelSpec;
    use time::macros::date;

    fn solve_env(provider: &CachedRecalibrationProvider) -> HazardApplyEnv<'_> {
        HazardApplyEnv {
            mode: HazardBumpMode::SolveToPar,
            provider: Some(provider),
            source_markets: None,
        }
    }

    #[test]
    fn vol_index_parallel_uses_absolute_index_points() {
        let as_of = date!(2025 - 01 - 01);
        let vol_curve = PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(as_of)
            .spot_price(18.5)
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
            hazard_bump_mode: Default::default(),
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
        assert!((updated.spot_price() - 19.5).abs() < 1.0e-12);
        assert!((updated.price(0.25) - 21.0).abs() < 1.0e-12);
    }

    #[test]
    fn vol_index_node_front_knot_syncs_spot_level() {
        let as_of = date!(2025 - 01 - 01);
        let vol_curve = PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(as_of)
            .spot_price(18.5)
            .knots([(0.0, 18.5), (0.25, 20.0), (0.5, 21.5)])
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

        let effects = vol_index_node_effects(
            &CurveId::from("VIX"),
            &[("0Y".into(), 1.0)],
            TenorMatchMode::Exact,
            &ctx,
        )
        .expect("front-knot vol-index shock should apply");
        let bumped = effects
            .iter()
            .find_map(|e| match e {
                ScenarioEffect::UpdateCurve(storage) => storage.vol_index().map(|c| (**c).clone()),
                _ => None,
            })
            .expect("vol-index update");
        assert!(
            (bumped.spot_price() - 19.5).abs() < 1e-12,
            "front-knot +1.0 should move spot 18.5 → 19.5, got {}",
            bumped.spot_price()
        );
        assert!((bumped.price(0.0) - 19.5).abs() < 1e-12);
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
        let vol = PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(as_of)
            .day_count(DayCount::Act360)
            .spot_price(18.0)
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
            hazard_bump_mode: Default::default(),
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
        let vol_curve = PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(as_of)
            .spot_price(15.0)
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

    /// Off-pillar interpolated node bump on a log-linear discount curve
    /// delivers the requested zero shift at the request tenor under the
    /// native interpolant, with no first-order warning.
    #[test]
    fn interpolated_node_bump_on_discount_curve_hits_native_interpolant() {
        let as_of = date!(2025 - 01 - 01);
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .interp(InterpStyle::LogLinear)
            .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90), (10.0, 0.80)])
            .build()
            .expect("discount curve should build");
        let base_zero = curve.zero(3.0);
        let mut market = MarketContext::new().insert(curve);
        let mut model = FinancialModelSpec::new("test", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };

        let curve_id = CurveId::from("USD-OIS");
        let provider = CachedRecalibrationProvider::new();
        let env = solve_env(&provider);
        let effects = curve_node_effects(
            CurveKind::Discount,
            &curve_id,
            None,
            &[("3Y".into(), 25.0)],
            TenorMatchMode::Interpolate,
            &ctx,
            &env,
        )
        .expect("interpolated discount node bump should apply");

        assert!(
            !effects.iter().any(|e| matches!(
                e,
                ScenarioEffect::Warning(Warning::InterpolatedNodeBumpFirstOrder { .. })
            )),
            "direct-shift interpolated discount bump must not warn first-order, got {effects:?}"
        );

        let bumps = effects
            .iter()
            .filter_map(|effect| match effect {
                ScenarioEffect::MarketBump(bump) => Some(bump.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut bumped_market = market.clone();
        for bump in bumps {
            bumped_market = bumped_market
                .bump([bump])
                .expect("apply discount curve bump");
        }
        let bumped = bumped_market
            .get_discount("USD-OIS")
            .expect("discount curve update");
        let delta = bumped.zero(3.0) - base_zero;
        assert!(
            (delta - 0.0025).abs() < 1e-10,
            "native interpolant should deliver +25 bp at 3Y: got {delta}"
        );
    }

    /// Par-CDS solve-to-par rejects curves that do not carry a lossless replay
    /// recipe; stored display spreads are not sufficient quote provenance.
    #[test]
    fn interpolated_node_bump_on_par_cds_requires_lossless_recipe() {
        use finstack_quant_core::market_data::term_structures::HazardCurve;

        let as_of = date!(2025 - 01 - 01);
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.80), (10.0, 0.60)])
            .build()
            .expect("discount curve should build");
        let hazard = HazardCurve::builder("USD-CDS")
            .base_date(as_of)
            .recovery_rate(0.4)
            .knots(vec![(1.0, 0.01), (5.0, 0.02)])
            .par_spreads(vec![(1.0, 60.0), (5.0, 120.0)])
            .build()
            .expect("hazard curve should build");
        let mut market = MarketContext::new().insert(discount).insert(hazard);
        let mut model = FinancialModelSpec::new("test", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };

        let provider = CachedRecalibrationProvider::new();
        let env = solve_env(&provider);
        let error = curve_node_effects(
            CurveKind::ParCDS,
            &CurveId::from("USD-CDS"),
            None,
            &[("3Y".into(), 10.0)],
            TenorMatchMode::Interpolate,
            &ctx,
            &env,
        )
        .expect_err("display par spreads must not substitute for a replay recipe");

        assert!(error.to_string().contains("no lossless calibration recipe"));
    }

    /// The first-order warning must not fire when no approximation is in
    /// play: exact pillar matches (Σw² = 1) and direct-shift curve kinds
    /// (forward) deliver the shock exactly.
    #[test]
    fn exact_and_direct_shift_node_bumps_do_not_warn_first_order() {
        use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};

        let as_of = date!(2025 - 01 - 01);
        let discount = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots(vec![(0.0, 1.0), (1.0, 0.98), (5.0, 0.90), (10.0, 0.80)])
            .build()
            .expect("discount curve should build");
        let forward = ForwardCurve::builder("USD-SOFR", 0.25)
            .base_date(as_of)
            .knots([(0.0, 0.02), (1.0, 0.021), (5.0, 0.022)])
            .build()
            .expect("forward curve should build");
        let mut market = MarketContext::new().insert(discount).insert(forward);
        let mut model = FinancialModelSpec::new("test", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };

        let provider = CachedRecalibrationProvider::new();
        let env = solve_env(&provider);
        // Exact pillar hit on a direct-shift discount curve: no approximation.
        let effects = curve_node_effects(
            CurveKind::Discount,
            &CurveId::from("USD-OIS"),
            None,
            &[("5Y".into(), 25.0)],
            TenorMatchMode::Exact,
            &ctx,
            &env,
        )
        .expect("exact discount node bump should apply");
        assert!(
            !effects.iter().any(|e| matches!(
                e,
                ScenarioEffect::Warning(Warning::InterpolatedNodeBumpFirstOrder { .. })
            )),
            "exact pillar bump must not warn first-order"
        );

        // Off-pillar interpolated bump on a direct-shift (forward) curve: the
        // 1/Σw² correction is exact there, so no warning either.
        let effects = curve_node_effects(
            CurveKind::Forward,
            &CurveId::from("USD-SOFR"),
            None,
            &[("3Y".into(), 25.0)],
            TenorMatchMode::Interpolate,
            &ctx,
            &env,
        )
        .expect("interpolated forward node bump should apply");
        assert!(
            !effects.iter().any(|e| matches!(
                e,
                ScenarioEffect::Warning(Warning::InterpolatedNodeBumpFirstOrder { .. })
            )),
            "direct-shift interpolated bump must not warn first-order"
        );
    }

    fn wti_price_curve(as_of: finstack_quant_core::dates::Date) -> PriceCurve {
        PriceCurve::builder("WTI")
            .base_date(as_of)
            .spot_price(70.0)
            .knots([(0.0, 70.0), (1.0, 72.0), (5.0, 75.0)])
            .build()
            .expect("WTI price curve should build")
    }

    #[test]
    fn commodity_parallel_percent_moves_every_knot_and_spot() {
        let as_of = date!(2025 - 01 - 01);
        let mut market = MarketContext::new().insert(wti_price_curve(as_of));
        let mut model = FinancialModelSpec::new("test", vec![]);
        let engine = ScenarioEngine::new();
        let mut ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        engine
            .apply(
                &ScenarioSpec {
                    id: "commodity_parallel".into(),
                    name: None,
                    description: None,
                    operations: vec![OperationSpec::CurveParallelBp {
                        curve_kind: CurveKind::Commodity,
                        curve_id: "WTI".into(),
                        discount_curve_id: None,
                        bp: 10.0,
                    }],
                    priority: 0,
                    resolution_mode: Default::default(),
                    hazard_bump_mode: Default::default(),
                },
                &mut ctx,
            )
            .expect("parallel applies");

        let bumped = market.get_price_curve("WTI").expect("price curve");
        assert!((bumped.spot_price() - 77.0).abs() < 1e-12);
        for (t, expected) in [(0.0, 77.0), (1.0, 79.2), (5.0, 82.5)] {
            assert!(
                (bumped.price(t) - expected).abs() < 1e-12,
                "price({t}) should be {expected}, got {}",
                bumped.price(t)
            );
        }
    }

    #[test]
    fn commodity_exact_node_percent_moves_only_that_pillar() {
        let as_of = date!(2025 - 01 - 01);
        let mut market = MarketContext::new().insert(wti_price_curve(as_of));
        let mut model = FinancialModelSpec::new("test", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let provider = CachedRecalibrationProvider::new();
        let env = solve_env(&provider);
        let effects = curve_node_effects(
            CurveKind::Commodity,
            &CurveId::from("WTI"),
            None,
            &[("1Y".into(), 10.0)],
            TenorMatchMode::Exact,
            &ctx,
            &env,
        )
        .expect("exact commodity node bump should apply");
        let bumped = effects
            .iter()
            .find_map(|e| match e {
                ScenarioEffect::UpdateCurve(storage) => storage.price().map(|c| (**c).clone()),
                _ => None,
            })
            .expect("price curve update");
        assert!((bumped.price(1.0) - 79.2).abs() < 1e-12);
        assert!((bumped.spot_price() - 70.0).abs() < 1e-12);
        assert!((bumped.price(5.0) - 75.0).abs() < 1e-12);
    }

    #[test]
    fn commodity_interpolated_node_hits_native_price_percent() {
        let as_of = date!(2025 - 01 - 01);
        let curve = wti_price_curve(as_of);
        let base_px = curve.price(3.0);
        let mut market = MarketContext::new().insert(curve);
        let mut model = FinancialModelSpec::new("test", vec![]);
        let ctx = ExecutionContext {
            market: &mut market,
            model: Some(&mut model),
            instruments: None,
            rate_bindings: None,
            calendar: None,
            as_of,
        };
        let provider = CachedRecalibrationProvider::new();
        let env = solve_env(&provider);
        let effects = curve_node_effects(
            CurveKind::Commodity,
            &CurveId::from("WTI"),
            None,
            &[("3Y".into(), 10.0)],
            TenorMatchMode::Interpolate,
            &ctx,
            &env,
        )
        .expect("interpolated commodity node bump should apply");
        let bumped = effects
            .iter()
            .find_map(|e| match e {
                ScenarioEffect::UpdateCurve(storage) => storage.price().map(|c| (**c).clone()),
                _ => None,
            })
            .expect("price curve update");
        let expected = base_px * 1.10;
        assert!(
            (bumped.price(3.0) - expected).abs() < 1e-10,
            "price(3Y) should move +10%: got {} expected {expected}",
            bumped.price(3.0)
        );
    }

    #[test]
    fn commodity_parallel_large_shock_emits_warning() {
        let as_of = date!(2025 - 01 - 01);
        let mut market = MarketContext::new().insert(wti_price_curve(as_of));
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
        let provider = CachedRecalibrationProvider::new();
        let env = solve_env(&provider);
        let effects =
            curve_parallel_effects(CurveKind::Commodity, &curve_id, None, 250.0, &ctx, &env)
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
