//! Per-issuer credit-factor cascade for waterfall and parallel.
//!
//! Builds an ordered cascade `(generic / level_0 / ... / level_{L-1} / adder)`
//! of incremental synthetic spread shifts (in bp) that, applied per-issuer to
//! the instrument's hazard curves, decomposes the credit P&L into hierarchy
//! components.
//!
//! Single-instrument scope mirrors the linear CS01 path: the instrument's
//! issuer is read from `attributes().get_meta("credit::issuer_id")`, and the
//! per-issuer ΔS_i is synthesized by feeding `S_t0=0, S_t1=ΔS_i` to
//! `decompose_levels`, with `Δgeneric=0`.  Because `Σ_step (β·ΔF) + Δadder ≡
//! ΔS_i` by linearity of `decompose_period`, the cascade telescopes back to a
//! parallel ΔS_i shift on every credit curve.
//!
//! To preserve `credit_curves_pnl` *byte-identically* against the no-model
//! single-step in the presence of non-parallel hazard curve moves, the final
//! `Adder` step swaps the running hazard curves for the T1 hazard curves
//! wholesale (instead of merely bumping by `Δadder_i` bp). The step is still
//! labelled "Adder" — the bp-bump portion exactly equals `Δadder_i` for
//! parallel moves; for non-parallel moves the residual tenor structure is
//! absorbed into Adder, so all credit
//! roll-down / curve-shape effects flow into the per-issuer adder.

use std::collections::BTreeMap;
use std::sync::Arc;

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::diff::{measure_par_spread_shift, TenorSamplingMethod};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, IssuerId};
use finstack_quant_core::Result;
use finstack_quant_models::factor::credit::hierarchy::{
    dimension_key, CreditFactorModel, HierarchyDimension, IssuerBetaRow,
};
use finstack_quant_models::factor::matching::{
    bucket_factor_id, CREDIT_GENERIC_FACTOR_ID, ISSUER_ID_META_KEY,
};

use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::recalibration::{
    provider_missing, HazardRecalibrationAction, HazardRecalibrationRequest, QuoteBump,
    RecalibrationProvider,
};

/// Threshold above which an adder step's absolute P&L is considered large
/// enough to warrant a `tracing::warn!`. Expressed as a fraction of the
/// total credit P&L (sum of |generic| + Σ|level| + |adder|).
///
/// The adder step absorbs whatever residual hazard-curve shape remains after
/// the parallel cascade steps (see module-level docs). A large adder
/// magnitude indicates significant non-parallel curve moves that the
/// hierarchy decomposition could not explain.
pub(crate) const ADDER_MAGNITUDE_WARN_RATIO: f64 = 0.05;

/// Minimum factor-series/par-spread scale ratio treated as a unit
/// mismatch. Scalar factor series ([`MarketScalar::Unitless`]) are consumed
/// directly as basis points; a percent-quoted series (e.g. `3.25` → `3.50`
/// meaning 25bp) under-reads its move 100× and the idiosyncratic adder
/// silently absorbs the difference. When a nonzero factor move is at least
/// this many times smaller than the nonzero par-spread move it is meant to
/// explain, a loud diagnostic is surfaced.
///
/// This is a warning, not a hard `Error::Validation`: a genuine idiosyncratic
/// blowout on a quiet index day (series Δ ≈ 0.2bp while the issuer gaps 25bp)
/// is numerically indistinguishable from a unit mismatch, so refusing to run
/// would reject correct attributions. The note lands in the cascade's
/// `warnings` (routed to `meta.notes` on the linear wire) alongside a
/// `tracing::warn!`.
pub(crate) const FACTOR_UNIT_MISMATCH_RATIO: f64 = 99.0;

/// What kind of cascade step a single bump represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreditStepKind {
    /// Generic / PC factor: bp = β_PC × ΔF_PC.
    Generic,
    /// Hierarchy level k: bp = β_level_k × ΔF_level_k(g_i^k).
    Level(usize),
    /// Per-issuer adder — the **parallel** issuer-idiosyncratic move. The bump
    /// value is `Δadder_i` bp and the step applies a parallel hazard-curve
    /// bump, exactly like Generic / Level.
    Adder,
    /// Curve-shape / term-structure residual. Captures the **non-parallel**
    /// part of the hazard-curve move (steepening, twist) — everything the
    /// parallel Generic / Level / Adder bumps cannot explain. The step snaps
    /// the running hazard curves to their T1 state, so the cascade end-state
    /// matches the no-model single Credit step exactly and `Σ steps ≡ total`
    /// still holds. Previously this residual was absorbed into `Adder`,
    /// mislabeling curve-shape risk as issuer-idiosyncratic.
    CurveShape,
}

/// One step in the credit cascade.
#[derive(Debug, Clone)]
pub(crate) struct CreditCascadeStep {
    /// Step kind.
    pub kind: CreditStepKind,
    /// Human-readable label, e.g. `"credit::generic"`, `"credit::rating"`,
    /// `"credit::adder"`.
    pub label: String,
    /// Per-issuer synthetic spread shift in basis points to apply at this step.
    /// For the `Adder` step this is the Δadder bp; the running market is also
    /// snapped to T1 hazard at the end of that step.
    pub delta_bp: f64,
}

/// Planned cascade for a single instrument's credit P&L.
#[derive(Debug, Clone)]
pub(crate) struct CreditCascade {
    /// Resolved issuer id (from instrument attributes).
    pub issuer_id: IssuerId,
    /// Hazard curve ids the instrument depends on.
    pub hazard_curve_ids: Vec<CurveId>,
    /// Discount curve id used to re-bootstrap the hazard curves when applying a
    /// par-spread cascade step (`shift_credit_curves_par_spread`). `None` when
    /// the instrument declares no discount curve — the par-spread bump then
    /// degrades to a direct hazard-rate shift.
    pub discount_curve_id: Option<CurveId>,
    /// Ordered cascade steps: generic, then one per hierarchy level, then adder.
    pub steps: Vec<CreditCascadeStep>,
    /// Level names (one per hierarchy dimension), used to build
    /// `LevelPnl.level_name` and as parallel-factor labels.
    pub level_names: Vec<String>,
    /// Diagnostics collected while planning (currently the unit-coherence
    /// guard, [`FACTOR_UNIT_MISMATCH_RATIO`]). Callers on the linear wire
    /// merge these into `meta.notes`.
    pub warnings: Vec<String>,
}

/// Human-readable name for a credit hierarchy dimension.
pub(crate) fn hierarchy_level_name(dim: &HierarchyDimension) -> String {
    match dim {
        HierarchyDimension::Custom(s) => s.clone(),
        _ => dimension_key(dim).to_owned(),
    }
}

/// Human-readable names for every level in model order.
pub(crate) fn hierarchy_level_names(model: &CreditFactorModel) -> Vec<String> {
    model
        .hierarchy
        .levels
        .iter()
        .map(hierarchy_level_name)
        .collect()
}

/// Single-instrument bucket map for one issuer/level.
pub(crate) fn single_issuer_by_bucket(
    model: &CreditFactorModel,
    row: &IssuerBetaRow,
    level_index: usize,
    total: Money,
    include: bool,
) -> BTreeMap<String, Money> {
    if !include {
        return BTreeMap::new();
    }
    model
        .hierarchy
        .bucket_path(&row.tags, level_index)
        .map(|bucket| BTreeMap::from([(bucket, total)]))
        .unwrap_or_default()
}

/// Optional single-issuer adder map used by single-instrument detail paths.
pub(crate) fn optional_single_issuer_adder(
    issuer_id: &IssuerId,
    adder: Money,
    include: bool,
) -> Option<BTreeMap<IssuerId, Money>> {
    include.then(|| BTreeMap::from([(issuer_id.clone(), adder)]))
}

/// Fill the `CurveShape` step with the residual needed for exact reconciliation.
pub(crate) fn apply_curve_shape_residual(
    step_pnls: &mut [Money],
    steps: &[CreditCascadeStep],
    credit_total: Money,
) -> finstack_quant_core::Result<()> {
    let _: () = {
        debug_assert_eq!(step_pnls.len(), steps.len());
        let parallel_sum: f64 = step_pnls
            .iter()
            .zip(steps.iter())
            .filter(|(_, step)| !matches!(step.kind, CreditStepKind::CurveShape))
            .map(|(pnl, _)| pnl.amount())
            .sum();
        let curve_shape = Money::new(
            credit_total.amount() - parallel_sum,
            credit_total.currency(),
        )?;
        for (pnl, step) in step_pnls.iter_mut().zip(steps.iter()) {
            if matches!(step.kind, CreditStepKind::CurveShape) {
                *pnl = curve_shape;
            }
        }
    };
    Ok(())
}

/// Plan a credit cascade for one instrument.
///
/// Returns `Ok(None)` when no cascade can be planned: instrument has no
/// `credit::issuer_id` attribute, the issuer is not in the model's
/// `issuer_betas`, the instrument has no hazard curve dependencies, or none of
/// the hazard curves can be measured (missing curves on either side).
///
/// # Multi-curve averaging
///
/// When the instrument has multiple hazard curves for the same issuer, the
/// cascade uses the simple average ΔS across curves (in bp). This is exact for
/// single-curve issuers and an approximation otherwise; all curves are shifted
/// by the same bp at each step. The Adder step's snap-to-T1 absorbs any
/// residual curve-shape differences so reconciliation remains exact, but the
/// split between level-k and Adder for multi-curve divergent moves is
/// approximate.
pub(crate) fn plan_credit_cascade(
    model: &CreditFactorModel,
    instrument: &Arc<dyn Instrument>,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Result<Option<CreditCascade>> {
    let issuer_id_str = match instrument.attributes().get_meta(ISSUER_ID_META_KEY) {
        Some(s) => s.to_string(),
        None => return Ok(None),
    };
    let issuer_id = IssuerId::new(issuer_id_str.as_str());

    let Some(issuer_row) = model.issuer_betas.iter().find(|r| r.issuer_id == issuer_id) else {
        tracing::warn!(
            instrument_id = %instrument.id(),
            issuer_id = %issuer_id_str,
            "Credit cascade skipped: issuer is not mapped in the credit factor model"
        );
        return Ok(None);
    };

    // Resolve hazard + discount curves from the instrument's market dependencies.
    let market_deps = instrument.market_dependencies()?;
    let credit_curves: Vec<CurveId> = market_deps.curves.credit_curves.to_vec();
    if credit_curves.is_empty() {
        return Ok(None);
    }
    let discount_curve_id: Option<CurveId> = market_deps.curves.discount_curves.first().cloned();

    // Measure the average **par CDS spread** ΔS_i across the issuer's hazard
    // curves (in bp). The cascade is par-spread-consistent: this
    // `measure_par_spread_shift` pairs with `shift_credit_curves_par_spread`,
    // which the cascade reprice path and the credit-detail CS01 both apply — so
    // step `delta_bp` and CS01 share units (par CDS spread bp) and reconcile to
    // the par-spread `credit_curves_pnl`.
    let mut total_shift_bp = 0.0;
    let mut count = 0usize;
    for curve_id in &credit_curves {
        if let Ok(shift) = measure_par_spread_shift(
            curve_id.as_str(),
            market_t0,
            market_t1,
            TenorSamplingMethod::Standard,
        ) {
            total_shift_bp += shift;
            count += 1;
        }
    }
    if count == 0 {
        return Ok(None);
    }
    let ds_i = total_shift_bp / count as f64;

    let level_names = hierarchy_level_names(model);
    let mut scalar_level_moves: Vec<(String, Option<f64>)> =
        Vec::with_capacity(model.hierarchy.levels.len());
    for k in 0..model.hierarchy.levels.len() {
        let factor_id = bucket_factor_id(&model.hierarchy, &issuer_row.tags, k)
            .map(|factor_id| factor_id.to_string())
            .unwrap_or_default();
        let move_bp = factor_move_bp(&factor_id, market_t0, market_t1);
        scalar_level_moves.push((factor_id, move_bp));
    }

    let generic_move = factor_move_bp(&model.generic_factor.series_id, market_t0, market_t1)
        .or_else(|| factor_move_bp(CREDIT_GENERIC_FACTOR_ID, market_t0, market_t1));
    let has_scalar_factor_moves =
        generic_move.is_some() || scalar_level_moves.iter().any(|(_, m)| m.is_some());

    // Unit-coherence guard: scalar series are consumed as bp with
    // no unit tag, so a percent-quoted series silently under-reads its factor
    // move ~100× and the adder absorbs the residual. Flag any observed
    // nonzero factor move that is ≥ FACTOR_UNIT_MISMATCH_RATIO× smaller than
    // the nonzero par-spread move it should explain. Surfaced as a loud note
    // + tracing::warn rather than a hard error — see the constant's docs for
    // why refusing outright would reject genuine idiosyncratic blowouts.
    let mut warnings: Vec<String> = Vec::new();
    {
        let mut check_unit_coherence = |factor_id: &str, move_bp: Option<f64>| {
            let Some(mv) = move_bp else { return };
            if mv != 0.0 && ds_i != 0.0 && mv.abs() * FACTOR_UNIT_MISMATCH_RATIO <= ds_i.abs() {
                let msg = format!(
                    "credit factor series '{factor_id}' moved {mv:.6} while issuer \
                     '{issuer_id}' par spread moved {ds_i:.4}bp — a ≥{FACTOR_UNIT_MISMATCH_RATIO:.0}× \
                     scale mismatch. Factor series are consumed as basis points; a percent- \
                     or decimal-quoted series under-reads its move and the idiosyncratic \
                     adder silently absorbs the difference. Verify the series' units \
                     (or accept if the issuer move is genuinely idiosyncratic)."
                );
                tracing::warn!(
                    factor_id,
                    factor_move = mv,
                    par_spread_move_bp = ds_i,
                    threshold = FACTOR_UNIT_MISMATCH_RATIO,
                    "credit cascade factor-series/par-spread unit scale mismatch"
                );
                warnings.push(msg);
            }
        };
        check_unit_coherence(&model.generic_factor.series_id, generic_move);
        for (factor_id, move_bp) in &scalar_level_moves {
            check_unit_coherence(factor_id, *move_bp);
        }
    }
    if has_scalar_factor_moves {
        let mut steps: Vec<CreditCascadeStep> =
            Vec::with_capacity(model.hierarchy.levels.len() + 2);
        let mut explained_bp = 0.0;
        // the calibrated model identity is
        // `S_i = β_PC·F_PC + Σ_k β_k·F_level_k + adder_i`, so each scalar
        // factor move must be scaled by the ISSUER's beta before it explains
        // any of ΔS_i (matching `CreditStepKind`'s documented `bp = β × ΔF`
        // and the synthesized non-scalar path below). The raw moves were
        // previously used unscaled, mislabeling the β-residual as
        // idiosyncratic adder for any non-unit-beta issuer.
        let beta_pc = issuer_row.betas.pc;
        let level_betas = issuer_row.betas.levels.clone();
        let mut append_factor = |factor_id: &str, steps: &mut Vec<CreditCascadeStep>| {
            if factor_id == model.generic_factor.series_id || factor_id == CREDIT_GENERIC_FACTOR_ID
            {
                let generic_bp = beta_pc * generic_move.unwrap_or(0.0);
                explained_bp += generic_bp;
                steps.push(CreditCascadeStep {
                    kind: CreditStepKind::Generic,
                    label: "credit::generic".to_string(),
                    delta_bp: generic_bp,
                });
                return true;
            }
            for (k, (level_factor_id, move_bp)) in scalar_level_moves.iter().enumerate() {
                if factor_id == level_factor_id {
                    let beta_k = level_betas.get(k).copied().unwrap_or(0.0);
                    let level_bp = beta_k * move_bp.unwrap_or(0.0);
                    explained_bp += level_bp;
                    steps.push(CreditCascadeStep {
                        kind: CreditStepKind::Level(k),
                        label: format!("credit::{}", level_names[k]),
                        delta_bp: level_bp,
                    });
                    return true;
                }
            }
            false
        };

        let mut matched_config_factor = false;
        // Dedupe config factor ids: a duplicated id would
        // append two cascade steps for the same factor; the cumulative-bump
        // executor would double-apply the move and the detail builder's
        // last-wins assignment would silently break the detail-sum invariant.
        let mut seen_factor_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for factor in &model.config.factors {
            if !seen_factor_ids.insert(factor.id.as_str()) {
                tracing::warn!(
                    factor_id = factor.id.as_str(),
                    "duplicate factor id in CreditFactorModel.config.factors ignored"
                );
                continue;
            }
            matched_config_factor |= append_factor(factor.id.as_str(), &mut steps);
        }
        if !matched_config_factor {
            append_factor(CREDIT_GENERIC_FACTOR_ID, &mut steps);
            for (factor_id, _) in &scalar_level_moves {
                append_factor(factor_id, &mut steps);
            }
        }
        steps.push(CreditCascadeStep {
            kind: CreditStepKind::Adder,
            label: "credit::adder".to_string(),
            delta_bp: ds_i - explained_bp,
        });
        // Curve-shape residual: snap-to-T1 catches the non-parallel hazard
        // move. `delta_bp` is not a bp value (it is a snap), kept 0.0.
        steps.push(CreditCascadeStep {
            kind: CreditStepKind::CurveShape,
            label: "credit::curve_shape".to_string(),
            delta_bp: 0.0,
        });
        return Ok(Some(CreditCascade {
            issuer_id,
            hazard_curve_ids: credit_curves,
            discount_curve_id,
            steps,
            level_names,
            warnings,
        }));
    }

    // No market scalar series for any credit factor (prior fix):
    // without observed factor moves, NOTHING about the issuer's spread move
    // is identifiably systematic, so the entire ΔS_i is routed to the
    // idiosyncratic adder. The former approach synthesized a single-issuer
    // period decomposition (S_t0 = 0 → ΔS_i), which estimated each "level
    // factor move" from the one issuer's own residual: with unit betas the
    // whole ΔS landed in level 0 mislabeled as systematic rating risk, and
    // with calibrated betas the components oscillated (e.g. 3ΔS / −8ΔS /
    // +6ΔS) — reconciling exactly but economically meaningless, and the
    // adder-magnitude warning could never fire for genuinely idiosyncratic
    // moves.
    let mut steps: Vec<CreditCascadeStep> = Vec::with_capacity(model.hierarchy.levels.len() + 2);
    steps.push(CreditCascadeStep {
        kind: CreditStepKind::Generic,
        label: "credit::generic".to_string(),
        delta_bp: 0.0,
    });
    for (k, level_name) in level_names.iter().enumerate() {
        steps.push(CreditCascadeStep {
            kind: CreditStepKind::Level(k),
            label: format!("credit::{level_name}"),
            delta_bp: 0.0,
        });
    }
    steps.push(CreditCascadeStep {
        kind: CreditStepKind::Adder,
        label: "credit::adder".to_string(),
        delta_bp: ds_i,
    });
    // Curve-shape residual: snap-to-T1 catches the non-parallel hazard move.
    steps.push(CreditCascadeStep {
        kind: CreditStepKind::CurveShape,
        label: "credit::curve_shape".to_string(),
        delta_bp: 0.0,
    });

    Ok(Some(CreditCascade {
        issuer_id,
        hazard_curve_ids: credit_curves,
        discount_curve_id,
        steps,
        level_names,
        warnings,
    }))
}

fn scalar_to_bp(scalar: &MarketScalar) -> f64 {
    match scalar {
        MarketScalar::Unitless(value) => *value,
        MarketScalar::Price(money) => money.amount(),
    }
}

fn factor_move_bp(
    factor_id: &str,
    market_t0: &MarketContext,
    market_t1: &MarketContext,
) -> Option<f64> {
    let t0 = market_t0.get_price(factor_id).ok().map(scalar_to_bp)?;
    let t1 = market_t1.get_price(factor_id).ok().map(scalar_to_bp)?;
    Some(t1 - t0)
}

/// Apply a parallel **par CDS spread** shift (in bp) to every hazard curve in
/// `curve_ids` from `source_market` into `target_market`, returning a new
/// market with the shifted curves.
///
/// Every curve is first replayed unchanged against the target dependency
/// market and then bumped from that replayed center. Direct hazard-intensity
/// shifts are intentionally not a fallback for this quote-space attribution
/// operation.
///
/// Used by the credit-factor cascade so the per-issuer step `delta_bp`, the
/// credit-detail CS01, and `credit_curves_pnl` are all expressed in the same
/// par CDS spread basis.
pub(crate) fn shift_credit_curves_par_spread(
    source_market: &MarketContext,
    target_market: &MarketContext,
    curve_ids: &[CurveId],
    discount_id: Option<&CurveId>,
    delta_bp: f64,
    provider: &dyn RecalibrationProvider,
) -> Result<MarketContext> {
    let mut new_market = target_market.clone();
    let discount_id = discount_id
        .cloned()
        .ok_or_else(|| provider_missing("attribution credit replay"))?;
    let source_market = Arc::new(source_market.clone());
    for curve_id in curve_ids {
        let Ok(source_hazard) = source_market.get_hazard(curve_id.as_str()) else {
            continue;
        };
        let target = Arc::new(new_market.clone());
        let replayed = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
            hazard: source_hazard,
            source_market: Arc::clone(&source_market),
            target_market: Arc::clone(&target),
            discount_curve_id: discount_id.clone(),
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
            action: HazardRecalibrationAction::DependencyMarketReplay,
        })?;
        new_market = new_market.insert(replayed.as_ref().clone());
        if delta_bp == 0.0 {
            continue;
        }
        let replay_market = Arc::new(new_market.clone());
        let bumped = provider.rebuild_hazard_curve(&HazardRecalibrationRequest {
            hazard: replayed,
            source_market: Arc::clone(&replay_market),
            target_market: replay_market,
            discount_curve_id: discount_id.clone(),
            doc_clause: None,
            cds_valuation_convention: None,
            deal_quote_override: None,
            action: HazardRecalibrationAction::SpreadBump(QuoteBump::ParallelBp(delta_bp)),
        })?;
        new_market = new_market.insert(bumped.as_ref().clone());
    }
    Ok(new_market)
}

/// Replace the running market's hazard curves (for `curve_ids`) with the T1
/// hazard curves from `market_t1`. Used at the Adder step so end-state matches
/// the no-model single-Credit-step result.
pub(crate) fn snap_hazard_to_t1(
    base_market: &MarketContext,
    market_t1: &MarketContext,
    curve_ids: &[CurveId],
) -> MarketContext {
    let mut new_market = base_market.clone();
    for curve_id in curve_ids {
        if let Ok(curve_t1) = market_t1.get_hazard(curve_id.as_str()) {
            new_market = new_market.insert((*curve_t1).clone());
        }
    }
    new_market
}

/// Build a `CreditFactorAttribution` from per-step P&L amounts captured during
/// the cascade. `step_pnls` must align with `cascade.steps`.
pub(crate) fn build_credit_factor_attribution(
    model: &CreditFactorModel,
    cascade: &CreditCascade,
    options: &super::credit_factor::CreditFactorDetailOptions,
    step_pnls: &[finstack_quant_core::money::Money],
) -> finstack_quant_core::Result<super::types::CreditFactorAttribution> {
    Ok({
        use super::credit_factor::credit_factor_model_id;
        use super::types::{CreditFactorAttribution, LevelPnl};

        // Release-safe shape check: a silent zip truncation
        // would break the detail-sum invariant with no signal in release builds.
        if step_pnls.len() != cascade.steps.len() {
            tracing::warn!(
                issuer_id = %cascade.issuer_id,
                step_pnls = step_pnls.len(),
                steps = cascade.steps.len(),
                "credit cascade step P&L count does not match planned steps; \
                 the shorter length is used and the detail sum may not reconcile"
            );
        }
        debug_assert_eq!(step_pnls.len(), cascade.steps.len());
        let ccy = step_pnls
            .first()
            .map(|m| m.currency())
            .unwrap_or(finstack_quant_core::currency::Currency::USD);
        if step_pnls.is_empty() {
            tracing::warn!(
                issuer_id = %cascade.issuer_id,
                "credit cascade produced no step P&Ls; emitting an all-zero detail (USD)"
            );
        }

        // Resolve issuer's bucket path for per-bucket detail (single-instrument
        // scope: each level has at most one populated bucket).
        let issuer_row = model
            .issuer_betas
            .iter()
            .find(|r| r.issuer_id == cascade.issuer_id);

        let mut generic_pnl = finstack_quant_core::money::Money::from((0_i64, ccy));
        let mut adder_pnl = finstack_quant_core::money::Money::from((0_i64, ccy));
        let mut curve_shape_pnl = finstack_quant_core::money::Money::from((0_i64, ccy));
        let mut level_pnls: BTreeMap<usize, finstack_quant_core::money::Money> = BTreeMap::new();

        for (step, pnl) in cascade.steps.iter().zip(step_pnls.iter()) {
            match step.kind {
                CreditStepKind::Generic => generic_pnl = *pnl,
                CreditStepKind::Adder => adder_pnl = *pnl,
                CreditStepKind::CurveShape => curve_shape_pnl = *pnl,
                CreditStepKind::Level(k) => {
                    level_pnls.insert(k, *pnl);
                }
            }
        }

        let mut levels: Vec<LevelPnl> = Vec::with_capacity(cascade.level_names.len());
        for (k, level_name) in cascade.level_names.iter().enumerate() {
            let total = level_pnls
                .get(&k)
                .copied()
                .unwrap_or_else(|| finstack_quant_core::money::Money::from((0_i64, ccy)));
            let by_bucket = issuer_row
                .map(|row| {
                    single_issuer_by_bucket(
                        model,
                        row,
                        k,
                        total,
                        options.include_per_bucket_breakdown,
                    )
                })
                .unwrap_or_default();
            levels.push(LevelPnl {
                level_name: level_name.clone(),
                total,
                by_bucket,
            });
        }

        let adder_pnl_by_issuer = optional_single_issuer_adder(
            &cascade.issuer_id,
            adder_pnl,
            options.include_per_issuer_adder,
        );

        // Diagnostic: surface the adder magnitude and warn when it dominates the
        // credit P&L. The adder is now the *parallel* issuer-idiosyncratic move
        // only — non-parallel curve-shape risk lands in `curve_shape_pnl` — so a
        // large |adder| genuinely signals a large idiosyncratic spread move.
        let adder_abs = adder_pnl.amount().abs();
        let curve_shape_abs = curve_shape_pnl.amount().abs();
        let total_credit_abs = generic_pnl.amount().abs()
            + levels.iter().map(|l| l.total.amount().abs()).sum::<f64>()
            + adder_abs
            + curve_shape_abs;
        if total_credit_abs > 0.0 && adder_abs > ADDER_MAGNITUDE_WARN_RATIO * total_credit_abs {
            tracing::warn!(
                issuer_id = %cascade.issuer_id,
                adder_pnl = adder_pnl.amount(),
                adder_abs = adder_abs,
                total_credit_abs = total_credit_abs,
                ratio = adder_abs / total_credit_abs,
                threshold = ADDER_MAGNITUDE_WARN_RATIO,
                "credit cascade per-issuer adder magnitude exceeds {:.0}% of total \
                 credit P&L — the issuer's idiosyncratic parallel spread move is large",
                ADDER_MAGNITUDE_WARN_RATIO * 100.0
            );
        }
        // A large curve-shape component means the hazard curve moved
        // non-parallel (steepening / twist) — surfaced as its own signal.
        if total_credit_abs > 0.0 && curve_shape_abs > ADDER_MAGNITUDE_WARN_RATIO * total_credit_abs
        {
            tracing::warn!(
                issuer_id = %cascade.issuer_id,
                curve_shape_pnl = curve_shape_pnl.amount(),
                curve_shape_abs = curve_shape_abs,
                total_credit_abs = total_credit_abs,
                ratio = curve_shape_abs / total_credit_abs,
                threshold = ADDER_MAGNITUDE_WARN_RATIO,
                "credit cascade curve-shape magnitude exceeds {:.0}% of total credit \
                 P&L — non-parallel hazard move and/or higher-order (spread \
                 convexity) effects the parallel CS01 steps cannot capture; on the \
                 linear (metrics-based / Taylor) wire this fires for large parallel \
                 moves too",
                ADDER_MAGNITUDE_WARN_RATIO * 100.0
            );
        }

        CreditFactorAttribution {
            model_id: credit_factor_model_id(model),
            generic_pnl,
            levels,
            adder_pnl_total: adder_pnl,
            curve_shape_pnl,
            adder_pnl_by_issuer,
            adder_magnitude: Some(finstack_quant_core::money::Money::new(adder_abs, ccy)?),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::create_date;
    use finstack_quant_core::market_data::bumps::BumpUnits;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::term_structures::HazardCurve;
    use finstack_quant_core::money::Money;
    use finstack_quant_models::factor::credit::hierarchy::{
        AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
        FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
        IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelsAtAnchor, VolState,
    };
    use finstack_quant_models::factor::{
        FactorCovarianceMatrix, FactorDefinition, FactorId, FactorModelConfig, FactorType,
        MarketMapping, MatchingConfig, PricingMode,
    };
    use finstack_quant_valuations::instruments::Instrument;
    use finstack_quant_valuations::instruments::{Attributes, Bond};
    use time::Month;

    fn empty_factor_config() -> FactorModelConfig {
        FactorModelConfig {
            factors: vec![],
            covariance: FactorCovarianceMatrix::new(vec![], vec![]).unwrap(),
            matching: MatchingConfig::MappingTable(vec![]),
            pricing_mode: PricingMode::DeltaBased,
            risk_measure: Default::default(),
            bump_size: None,
            unmatched_policy: None,
        }
    }

    fn make_model() -> CreditFactorModel {
        let mut tags = std::collections::BTreeMap::new();
        tags.insert("rating".to_string(), "B".to_string());
        tags.insert("region".to_string(), "US".to_string());

        CreditFactorModel {
            schema:
                finstack_quant_models::factor::credit::hierarchy::CreditFactorModelSchema::CURRENT,
            as_of: create_date(2024, Month::March, 29).unwrap(),
            calibration_window: DateRange {
                start: create_date(2022, Month::March, 29).unwrap(),
                end: create_date(2024, Month::March, 29).unwrap(),
            },
            policy: IssuerBetaPolicy::GloballyOff,
            generic_factor: GenericFactorSpec {
                name: "CDX HY".into(),
                series_id: "cdx.hy.5y".into(),
            },
            hierarchy: CreditHierarchySpec {
                levels: vec![HierarchyDimension::Rating, HierarchyDimension::Region],
            },
            panel_frequency:
                finstack_quant_models::factor::credit::calibration::PanelFrequency::Monthly,
            use_returns_or_levels:
                finstack_quant_models::factor::credit::calibration::PanelSpace::Returns,
            bucket_weighting:
                finstack_quant_models::factor::credit::calibration::BucketWeighting::Equal,
            config: empty_factor_config(),
            issuer_betas: vec![IssuerBetaRow {
                issuer_id: IssuerId::new("ISSUER-B"),
                tags: IssuerTags(tags),
                mode: IssuerBetaMode::IssuerBeta,
                betas: IssuerBetas {
                    pc: 2.0,
                    levels: vec![3.0, 4.0],
                },
                adder_at_anchor: 0.0,
                adder_vol_annualized: 0.0,
                adder_vol_source: AdderVolSource::Default,
                fit_quality: None,
                level_fit_quality: vec![],
                spread_duration: 1.0,
            }],
            anchor_state: LevelsAtAnchor {
                pc: 0.0,
                by_level: vec![],
            },
            static_correlation: FactorCorrelationMatrix::identity(vec![]),
            vol_state: VolState {
                factors: std::collections::BTreeMap::new(),
                idiosyncratic: std::collections::BTreeMap::new(),
            },
            factor_histories: None,
            diagnostics: CalibrationDiagnostics {
                mode_counts: std::collections::BTreeMap::new(),
                bucket_sizes_per_level: vec![],
                fold_ups: vec![],
                r_squared_histogram: None,
                tag_taxonomy: std::collections::BTreeMap::new(),
            },
        }
    }

    fn with_factor_order(mut model: CreditFactorModel, ids: &[&str]) -> CreditFactorModel {
        model.config.factors = ids
            .iter()
            .map(|id| FactorDefinition {
                id: FactorId::new(*id),
                factor_type: FactorType::Credit,
                market_mapping: MarketMapping::CurveParallel {
                    curve_ids: vec![],
                    units: BumpUnits::RateBp,
                },
                description: None,
            })
            .collect();
        model.config.covariance = FactorCovarianceMatrix::new(
            model.config.factors.iter().map(|f| f.id.clone()).collect(),
            vec![0.0; ids.len() * ids.len()],
        )
        .unwrap();
        model
    }

    fn canonical_credit_bond(curve_id: CurveId) -> Arc<dyn Instrument> {
        let mut bond = Bond::fixed(
            "BOND-ISSUER-B",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            create_date(2024, Month::January, 1).unwrap(),
            create_date(2030, Month::January, 1).unwrap(),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("bond construction");
        bond.credit_curve_id = Some(curve_id);
        bond.attributes = Attributes::new().with_meta(ISSUER_ID_META_KEY, "ISSUER-B");
        Arc::new(bond)
    }

    // Recovery 0 → LGD = 1, so the par CDS spread move (`measure_par_spread_shift`)
    // equals the hazard-rate move. These tests assert on the cascade's bp
    // arithmetic (factor ordering, adder reconciliation), so keeping par-spread
    // and hazard units coincident isolates that structure from the LGD scaling.
    fn hazard(id: &str, as_of: finstack_quant_core::dates::Date, rate: f64) -> HazardCurve {
        HazardCurve::builder(id)
            .base_date(as_of)
            .recovery_rate(0.0)
            .knots([(1.0, rate), (5.0, rate)])
            .build()
            .unwrap()
    }

    #[test]
    fn credit_cascade_scales_market_scalar_factor_moves_by_issuer_betas() {
        let as_of_t0 = create_date(2025, Month::January, 1).unwrap();
        let as_of_t1 = create_date(2025, Month::January, 2).unwrap();
        let curve_id = CurveId::new("ISSUER-B-HAZ");
        let model = make_model();
        let instrument = canonical_credit_bond(curve_id.clone());
        let market_t0 = MarketContext::new()
            .insert(hazard(curve_id.as_str(), as_of_t0, 0.0100))
            .insert_price("cdx.hy.5y", MarketScalar::Unitless(100.0))
            .insert_price("credit::level0::Rating::B", MarketScalar::Unitless(0.0))
            .insert_price(
                "credit::level1::Rating.Region::B.US",
                MarketScalar::Unitless(0.0),
            );
        let market_t1 = MarketContext::new()
            .insert(hazard(curve_id.as_str(), as_of_t1, 0.0130))
            .insert_price("cdx.hy.5y", MarketScalar::Unitless(125.0))
            .insert_price("credit::level0::Rating::B", MarketScalar::Unitless(7.0))
            .insert_price(
                "credit::level1::Rating.Region::B.US",
                MarketScalar::Unitless(-2.0),
            );

        let cascade = plan_credit_cascade(&model, &instrument, &market_t0, &market_t1)
            .unwrap()
            .expect("cascade");

        // Cascade: generic, rating, region, adder, curve_shape
        // (for a flat hazard move, curve_shape is a 0bp snap).
        let labels: Vec<&str> = cascade.steps.iter().map(|s| s.label.as_str()).collect();
        let deltas: Vec<f64> = cascade.steps.iter().map(|step| step.delta_bp).collect();
        assert_eq!(
            labels,
            vec![
                "credit::generic",
                "credit::rating",
                "credit::region",
                "credit::adder",
                "credit::curve_shape",
            ]
        );
        assert_eq!(deltas.len(), 5);
        // Each factor step is β_issuer × ΔF : the calibrated
        // identity is S_i = β_PC·F_PC + Σ β_k·L_k + adder_i, so with
        // β_pc = 2, β_levels = [3, 4]:
        //   generic = 2 × 25bp = 50bp; rating = 3 × 7bp = 21bp;
        //   region = 4 × (−2bp) = −8bp; explained = 63bp.
        // ΔS_i = 30bp hazard move (recovery 0 ⇒ par spread ≡ hazard), so the
        // idiosyncratic adder reconciles to 30 − 63 = −33bp.
        assert!(
            (deltas[0] - 50.0).abs() < 1e-10,
            "generic should be β_pc×25bp"
        );
        assert!((deltas[1] - 21.0).abs() < 1e-10, "rating should be β_0×7bp");
        assert!(
            (deltas[2] - (-8.0)).abs() < 1e-10,
            "region should be β_1×(−2bp)"
        );
        assert!(
            (deltas[3] - (-33.0)).abs() < 1e-9,
            "adder should reconcile ΔS − Σβ·ΔF, got {}",
            deltas[3]
        );
        assert!(
            (deltas[4]).abs() < 1e-10,
            "curve_shape carries no bp value (it is a snap step)"
        );
    }

    /// A percent-quoted factor series (3.25 → 3.50, i.e. Δ = 0.25
    /// "percent" intended as 25bp) consumed as bp explains only 0.25bp of a
    /// 25bp spread move — a 100× unit under-read the adder silently absorbs.
    /// The cascade must surface a loud diagnostic when the factor series'
    /// scale is ≥ ~100× smaller than the par-spread move it explains.
    #[test]
    fn credit_cascade_flags_unit_scale_mismatch_for_percent_quoted_factor() {
        let as_of_t0 = create_date(2025, Month::January, 1).unwrap();
        let as_of_t1 = create_date(2025, Month::January, 2).unwrap();
        let curve_id = CurveId::new("ISSUER-B-HAZ");
        let model = make_model();
        let instrument = canonical_credit_bond(curve_id.clone());
        // Hazard 1.00% → 1.25% at recovery 0 → ΔS = 25bp par spread.
        let market_t0 = MarketContext::new()
            .insert(hazard(curve_id.as_str(), as_of_t0, 0.0100))
            .insert_price("cdx.hy.5y", MarketScalar::Unitless(3.25));
        let market_t1 = MarketContext::new()
            .insert(hazard(curve_id.as_str(), as_of_t1, 0.0125))
            .insert_price("cdx.hy.5y", MarketScalar::Unitless(3.50));

        let cascade = plan_credit_cascade(&model, &instrument, &market_t0, &market_t1)
            .unwrap()
            .expect("cascade");

        assert!(
            cascade
                .warnings
                .iter()
                .any(|w| w.contains("scale mismatch")),
            "a ≥100× factor-series/spread scale mismatch must produce a loud \
             warning, got warnings = {:?}",
            cascade.warnings
        );
    }

    /// The unit guard must NOT fire when the factor series is genuinely
    /// bp-quoted and of comparable magnitude to the spread move.
    #[test]
    fn credit_cascade_does_not_flag_bp_quoted_factor_series() {
        let as_of_t0 = create_date(2025, Month::January, 1).unwrap();
        let as_of_t1 = create_date(2025, Month::January, 2).unwrap();
        let curve_id = CurveId::new("ISSUER-B-HAZ");
        let model = make_model();
        let instrument = canonical_credit_bond(curve_id.clone());
        let market_t0 = MarketContext::new()
            .insert(hazard(curve_id.as_str(), as_of_t0, 0.0100))
            .insert_price("cdx.hy.5y", MarketScalar::Unitless(100.0));
        let market_t1 = MarketContext::new()
            .insert(hazard(curve_id.as_str(), as_of_t1, 0.0125))
            .insert_price("cdx.hy.5y", MarketScalar::Unitless(125.0));

        let cascade = plan_credit_cascade(&model, &instrument, &market_t0, &market_t1)
            .unwrap()
            .expect("cascade");

        assert!(
            cascade.warnings.is_empty(),
            "bp-quoted series of comparable magnitude must not be flagged, \
             got warnings = {:?}",
            cascade.warnings
        );
    }

    #[test]
    fn credit_cascade_applies_fixed_bp_factors_in_config_order_then_residual() {
        let as_of_t0 = create_date(2025, Month::January, 1).unwrap();
        let as_of_t1 = create_date(2025, Month::January, 2).unwrap();
        let curve_id = CurveId::new("ISSUER-B-HAZ");
        let model = with_factor_order(
            make_model(),
            &[
                "credit::level0::Rating::B",
                "cdx.hy.5y",
                "credit::level1::Rating.Region::B.US",
            ],
        );
        let instrument = canonical_credit_bond(curve_id.clone());
        let market_t0 = MarketContext::new()
            .insert(hazard(curve_id.as_str(), as_of_t0, 0.0100))
            .insert_price("cdx.hy.5y", MarketScalar::Unitless(100.0))
            .insert_price("credit::level0::Rating::B", MarketScalar::Unitless(0.0))
            .insert_price(
                "credit::level1::Rating.Region::B.US",
                MarketScalar::Unitless(0.0),
            );
        let market_t1 = MarketContext::new()
            .insert(hazard(curve_id.as_str(), as_of_t1, 0.0130))
            .insert_price("cdx.hy.5y", MarketScalar::Unitless(105.0))
            .insert_price("credit::level0::Rating::B", MarketScalar::Unitless(25.0))
            .insert_price(
                "credit::level1::Rating.Region::B.US",
                MarketScalar::Unitless(-2.0),
            );

        let cascade = plan_credit_cascade(&model, &instrument, &market_t0, &market_t1)
            .unwrap()
            .expect("cascade");

        let labels: Vec<&str> = cascade.steps.iter().map(|s| s.label.as_str()).collect();
        let deltas: Vec<f64> = cascade.steps.iter().map(|s| s.delta_bp).collect();
        // Config-ordered factors, then adder, then the curve-shape snap step.
        assert_eq!(
            labels,
            vec![
                "credit::rating",
                "credit::generic",
                "credit::region",
                "credit::adder",
                "credit::curve_shape",
            ]
        );
        assert_eq!(deltas.len(), 5);
        // β-scaled steps  with β_pc = 2, β_levels = [3, 4]:
        // rating = 3 × 25 = 75bp, generic = 2 × 5 = 10bp,
        // region = 4 × (−2) = −8bp; adder = 30 − 77 = −47bp.
        assert!((deltas[0] - 75.0).abs() < 1e-10);
        assert!((deltas[1] - 10.0).abs() < 1e-10);
        assert!((deltas[2] - (-8.0)).abs() < 1e-10);
        assert!(
            (deltas[3] - (-47.0)).abs() < 1e-9,
            "adder should reconcile ΔS − Σβ·ΔF, got {}",
            deltas[3]
        );
        assert!(
            (deltas[4]).abs() < 1e-10,
            "curve_shape carries no bp value (it is a snap step)"
        );
    }
}
