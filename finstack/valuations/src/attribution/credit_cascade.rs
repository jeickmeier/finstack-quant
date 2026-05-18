//! PR-8a: per-issuer credit-factor cascade for waterfall and parallel.
//!
//! Builds an ordered cascade `(generic / level_0 / ... / level_{L-1} / adder)`
//! of incremental synthetic spread shifts (in bp) that, applied per-issuer to
//! the instrument's hazard curves, decomposes the credit P&L into hierarchy
//! components.
//!
//! Single-instrument scope mirrors the linear PR-7 wire: the instrument's
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
//! absorbed into Adder, matching the PR-plan's intent that all credit
//! roll-down / curve-shape effects flow into the per-issuer adder.

use std::collections::BTreeMap;
use std::sync::Arc;

use finstack_core::dates::Date;
use finstack_core::factor_model::credit_hierarchy::{
    dimension_key, CreditFactorModel, HierarchyDimension, IssuerTags,
};
use finstack_core::factor_model::matching::{
    bucket_factor_id, CREDIT_GENERIC_FACTOR_ID, ISSUER_ID_META_KEY,
};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::diff::{measure_hazard_curve_shift, TenorSamplingMethod};
use finstack_core::market_data::scalars::MarketScalar;
use finstack_core::types::{CurveId, IssuerId};
use finstack_core::Result;

use crate::calibration::bumps::{bump_hazard_shift, BumpRequest};
use crate::factor_model::{decompose_levels, decompose_period};
use crate::instruments::common_impl::traits::Instrument;

/// Threshold above which an adder step's absolute P&L is considered large
/// enough to warrant a `tracing::warn!`. Expressed as a fraction of the
/// total credit P&L (sum of |generic| + Σ|level| + |adder|).
///
/// The adder step absorbs whatever residual hazard-curve shape remains after
/// the parallel cascade steps (see module-level docs). A large adder
/// magnitude indicates significant non-parallel curve moves that the
/// hierarchy decomposition could not explain.
pub(crate) const ADDER_MAGNITUDE_WARN_RATIO: f64 = 0.05;

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
    /// still holds. Audit item #1: previously this residual was absorbed into
    /// `Adder`, mislabeling curve-shape risk as issuer-idiosyncratic.
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
    /// Ordered cascade steps: generic, then one per hierarchy level, then adder.
    pub steps: Vec<CreditCascadeStep>,
    /// Level names (one per hierarchy dimension), used to build
    /// `LevelPnl.level_name` and as parallel-factor labels.
    pub level_names: Vec<String>,
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
    as_of_t0: Date,
    as_of_t1: Date,
) -> Result<Option<CreditCascade>> {
    // Resolve issuer id from instrument attributes.
    let issuer_id_str = match instrument.attributes().get_meta(ISSUER_ID_META_KEY) {
        Some(s) => s.to_string(),
        None => return Ok(None),
    };
    let issuer_id = IssuerId::new(issuer_id_str.as_str());

    // Find issuer in model.
    let issuer_row = match model.issuer_betas.iter().find(|r| r.issuer_id == issuer_id) {
        Some(row) => row,
        None => {
            tracing::warn!(
                instrument_id = %instrument.id(),
                issuer_id = %issuer_id_str,
                "Credit cascade skipped: issuer is not mapped in the credit factor model"
            );
            return Ok(None);
        }
    };
    let tags = issuer_row.tags.clone();

    // Resolve hazard curves from the instrument's market dependencies.
    let market_deps = instrument.market_dependencies()?;
    let credit_curves: Vec<CurveId> = market_deps.curve_dependencies().credit_curves.to_vec();
    if credit_curves.is_empty() {
        return Ok(None);
    }

    // Measure average ΔS_i across the issuer's hazard curves (in bp).
    let mut total_shift_bp = 0.0;
    let mut count = 0usize;
    for curve_id in &credit_curves {
        if let Ok(shift) = measure_hazard_curve_shift(
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

    let mut level_names: Vec<String> = Vec::with_capacity(model.hierarchy.levels.len());
    let mut scalar_level_moves: Vec<(String, Option<f64>)> =
        Vec::with_capacity(model.hierarchy.levels.len());
    for (k, dim) in model.hierarchy.levels.iter().enumerate() {
        let level_name = match dim {
            HierarchyDimension::Custom(s) => s.clone(),
            _ => dimension_key(dim),
        };
        let factor_id = bucket_factor_id(&model.hierarchy, &issuer_row.tags, k)
            .map(|factor_id| factor_id.to_string())
            .unwrap_or_default();
        let move_bp = factor_move_bp(&factor_id, market_t0, market_t1);
        level_names.push(level_name);
        scalar_level_moves.push((factor_id, move_bp));
    }

    let generic_move = factor_move_bp(&model.generic_factor.series_id, market_t0, market_t1)
        .or_else(|| factor_move_bp(CREDIT_GENERIC_FACTOR_ID, market_t0, market_t1));
    let has_scalar_factor_moves =
        generic_move.is_some() || scalar_level_moves.iter().any(|(_, m)| m.is_some());
    if has_scalar_factor_moves {
        let mut steps: Vec<CreditCascadeStep> =
            Vec::with_capacity(model.hierarchy.levels.len() + 2);
        let mut explained_bp = 0.0;
        let mut append_factor = |factor_id: &str, steps: &mut Vec<CreditCascadeStep>| {
            if factor_id == model.generic_factor.series_id || factor_id == CREDIT_GENERIC_FACTOR_ID
            {
                let generic_bp = generic_move.unwrap_or(0.0);
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
                    let level_bp = move_bp.unwrap_or(0.0);
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
        for factor in &model.config.factors {
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
            steps,
            level_names,
        }));
    }

    // Synthesize a single-issuer period decomposition: feed S_t0=0, S_t1=ΔS_i,
    // generic=0 to mirror the PR-7 linear wire. Δgeneric will be 0; the level-0
    // bucket carries ΔS_i; remaining levels and the adder are zero (modulo
    // calibrated betas).
    let mut s_t0: BTreeMap<IssuerId, f64> = BTreeMap::new();
    let mut s_t1: BTreeMap<IssuerId, f64> = BTreeMap::new();
    s_t0.insert(issuer_id.clone(), 0.0);
    s_t1.insert(issuer_id.clone(), ds_i);

    let mut runtime_tags: BTreeMap<IssuerId, IssuerTags> = BTreeMap::new();
    runtime_tags.insert(issuer_id.clone(), tags);

    let from = decompose_levels(model, &s_t0, 0.0, as_of_t0, Some(&runtime_tags))
        .map_err(|e| finstack_core::Error::Validation(format!("decompose_levels(t0): {e}")))?;
    let to = decompose_levels(model, &s_t1, 0.0, as_of_t1, Some(&runtime_tags))
        .map_err(|e| finstack_core::Error::Validation(format!("decompose_levels(t1): {e}")))?;
    let period = decompose_period(&from, &to)
        .map_err(|e| finstack_core::Error::Validation(format!("decompose_period: {e}")))?;

    // Build cascade steps.
    let beta_pc = issuer_row.betas.pc;
    let generic_bp = beta_pc * period.d_generic;

    let mut steps: Vec<CreditCascadeStep> = Vec::with_capacity(model.hierarchy.levels.len() + 2);

    steps.push(CreditCascadeStep {
        kind: CreditStepKind::Generic,
        label: "credit::generic".to_string(),
        delta_bp: generic_bp,
    });

    for (k, dim) in model.hierarchy.levels.iter().enumerate() {
        let level_name = match dim {
            HierarchyDimension::Custom(s) => s.clone(),
            _ => dimension_key(dim),
        };
        let bucket = model
            .hierarchy
            .bucket_path(&issuer_row.tags, k)
            .unwrap_or_default();
        let d_level = period.by_level[k]
            .deltas
            .get(&bucket)
            .copied()
            .unwrap_or(0.0);
        let beta_k = issuer_row.betas.levels.get(k).copied().unwrap_or(0.0);
        let level_bp = beta_k * d_level;
        steps.push(CreditCascadeStep {
            kind: CreditStepKind::Level(k),
            label: format!("credit::{}", level_name),
            delta_bp: level_bp,
        });
    }

    let adder_bp = period.d_adder.get(&issuer_id).copied().unwrap_or(0.0);
    steps.push(CreditCascadeStep {
        kind: CreditStepKind::Adder,
        label: "credit::adder".to_string(),
        delta_bp: adder_bp,
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
        steps,
        level_names,
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

/// Apply an additive parallel bp-shift to every hazard curve in `curve_ids`
/// from `base_market`, returning a new MarketContext with the shifted curves.
/// Non-hazard families on `base_market` are preserved.
pub(crate) fn shift_hazard_curves(
    base_market: &MarketContext,
    curve_ids: &[CurveId],
    delta_bp: f64,
) -> Result<MarketContext> {
    // For each curve, take its current state from base_market, bump in-place,
    // and re-insert into a clone of base_market.
    let mut new_market = base_market.clone();
    if delta_bp == 0.0 {
        return Ok(new_market);
    }
    for curve_id in curve_ids {
        let cur = match base_market.get_hazard(curve_id.as_str()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let bumped = bump_hazard_shift(cur.as_ref(), &BumpRequest::Parallel(delta_bp))?;
        new_market = new_market.insert(bumped);
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
    step_pnls: &[finstack_core::money::Money],
) -> super::types::CreditFactorAttribution {
    use super::credit_factor::credit_factor_model_id;
    use super::types::{CreditFactorAttribution, LevelPnl};

    debug_assert_eq!(step_pnls.len(), cascade.steps.len());
    let ccy = step_pnls
        .first()
        .map(|m| m.currency())
        .unwrap_or(finstack_core::currency::Currency::USD);

    // Resolve issuer's bucket path for per-bucket detail (single-instrument
    // scope: each level has at most one populated bucket).
    let issuer_row = model
        .issuer_betas
        .iter()
        .find(|r| r.issuer_id == cascade.issuer_id);

    let mut generic_pnl = finstack_core::money::Money::new(0.0, ccy);
    let mut adder_pnl = finstack_core::money::Money::new(0.0, ccy);
    let mut curve_shape_pnl = finstack_core::money::Money::new(0.0, ccy);
    let mut level_pnls: BTreeMap<usize, finstack_core::money::Money> = BTreeMap::new();

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
            .unwrap_or_else(|| finstack_core::money::Money::new(0.0, ccy));
        let mut by_bucket: BTreeMap<String, finstack_core::money::Money> = BTreeMap::new();
        if options.include_per_bucket_breakdown {
            if let Some(row) = issuer_row {
                if let Some(bucket) = model.hierarchy.bucket_path(&row.tags, k) {
                    by_bucket.insert(bucket, total);
                }
            }
        }
        levels.push(LevelPnl {
            level_name: level_name.clone(),
            total,
            by_bucket,
        });
    }

    let adder_pnl_by_issuer = if options.include_per_issuer_adder {
        let mut m = BTreeMap::new();
        m.insert(cascade.issuer_id.clone(), adder_pnl);
        Some(m)
    } else {
        None
    };

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
    if total_credit_abs > 0.0
        && curve_shape_abs > ADDER_MAGNITUDE_WARN_RATIO * total_credit_abs
    {
        tracing::warn!(
            issuer_id = %cascade.issuer_id,
            curve_shape_pnl = curve_shape_pnl.amount(),
            curve_shape_abs = curve_shape_abs,
            total_credit_abs = total_credit_abs,
            ratio = curve_shape_abs / total_credit_abs,
            threshold = ADDER_MAGNITUDE_WARN_RATIO,
            "credit cascade curve-shape magnitude exceeds {:.0}% of total credit \
             P&L — the hazard curve experienced a significant non-parallel move",
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
        adder_magnitude: Some(finstack_core::money::Money::new(adder_abs, ccy)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::{Attributes, Bond};
    use finstack_core::currency::Currency;
    use finstack_core::dates::create_date;
    use finstack_core::factor_model::credit_hierarchy::{
        AdderVolSource, CalibrationDiagnostics, CreditFactorModel, CreditHierarchySpec, DateRange,
        FactorCorrelationMatrix, GenericFactorSpec, HierarchyDimension, IssuerBetaMode,
        IssuerBetaPolicy, IssuerBetaRow, IssuerBetas, IssuerTags, LevelsAtAnchor, VolState,
    };
    use finstack_core::factor_model::{
        FactorCovarianceMatrix, FactorDefinition, FactorId, FactorModelConfig, FactorType,
        MarketMapping, MatchingConfig, PricingMode,
    };
    use finstack_core::market_data::bumps::BumpUnits;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::term_structures::HazardCurve;
    use finstack_core::money::Money;
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
            schema_version: CreditFactorModel::SCHEMA_VERSION.into(),
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
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            create_date(2024, Month::January, 1).unwrap(),
            create_date(2030, Month::January, 1).unwrap(),
            "USD-OIS",
        )
        .expect("bond construction");
        bond.credit_curve_id = Some(curve_id);
        bond.attributes = Attributes::new().with_meta(ISSUER_ID_META_KEY, "ISSUER-B");
        Arc::new(bond)
    }

    fn hazard(id: &str, as_of: finstack_core::dates::Date, rate: f64) -> HazardCurve {
        HazardCurve::builder(id)
            .base_date(as_of)
            .knots([(1.0, rate), (5.0, rate)])
            .build()
            .unwrap()
    }

    #[test]
    fn credit_cascade_uses_fixed_bp_factor_moves_from_market_scalars() {
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

        let cascade = plan_credit_cascade(
            &model,
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
        )
        .unwrap()
        .expect("cascade");

        // Cascade: generic, rating, region, adder, curve_shape (audit item #1
        // adds the curve-shape step — for a flat hazard move it is a 0bp snap).
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
        assert!((deltas[0] - 25.0).abs() < 1e-10, "generic should be +25bp");
        assert!((deltas[1] - 7.0).abs() < 1e-10, "rating should be +7bp");
        assert!((deltas[2] - (-2.0)).abs() < 1e-10, "region should be -2bp");
        assert!((deltas[3]).abs() < 1e-10, "adder should reconcile to zero");
        assert!(
            (deltas[4]).abs() < 1e-10,
            "curve_shape carries no bp value (it is a snap step)"
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

        let cascade = plan_credit_cascade(
            &model,
            &instrument,
            &market_t0,
            &market_t1,
            as_of_t0,
            as_of_t1,
        )
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
        assert!((deltas[0] - 25.0).abs() < 1e-10);
        assert!((deltas[1] - 5.0).abs() < 1e-10);
        assert!((deltas[2] - (-2.0)).abs() < 1e-10);
        assert!((deltas[3] - 2.0).abs() < 1e-10);
        assert!(
            (deltas[4]).abs() < 1e-10,
            "curve_shape carries no bp value (it is a snap step)"
        );
    }
}
