use std::collections::{BTreeMap, BTreeSet};

use finstack_quant_analytics::correlation::{
    nearest_correlation_matrix, validate_correlation_matrix, NearestCorrelationOpts,
};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::BumpUnits;
use finstack_quant_core::types::IssuerId;
use finstack_quant_core::Result;

use super::config::{CovarianceStrategy, PanelSpace, VolModelChoice};
use super::statistics::{
    d_rho_d, flat_to_row_major, ledoit_wolf_cov_and_corr, sample_correlation_flat,
};
use super::validation::validation_err;
use crate::factor::credit::hierarchy::{
    CalibrationDiagnostics, CreditHierarchySpec, FactorCorrelationMatrix, FactorHistories,
    FactorVolModel, FitQuality, FoldUpRecord, IdiosyncraticVolModel, IssuerBetaMode, IssuerBetaRow,
    IssuerBetas, IssuerTags, LevelAnchor, LevelsAtAnchor, VolState,
};
use crate::factor::matching::{CreditHierarchicalConfig, CREDIT_GENERIC_FACTOR_ID};
use crate::factor::{
    FactorCovarianceMatrix, FactorDefinition, FactorId, FactorModelConfig, FactorType,
    MarketMapping, MatchingConfig, PricingMode,
};

/// Anchor-step output: anchor levels + per-issuer adder values at as_of.
pub(super) struct AnchorOutcome {
    pub(super) levels: LevelsAtAnchor,
    pub(super) adder: BTreeMap<IssuerId, f64>,
}

/// Step 7: anchor levels at as_of (level space, not return space).
///
/// Implements the same peel-the-onion math as
/// [`crate::factor::credit::decomposition::decompose_levels`] but uses
/// the calibrated betas from step 4-5. We don't have a complete
/// `CreditFactorModel` yet, so this is a self-contained re-implementation.
pub(super) fn anchor_levels(
    hierarchy: &CreditHierarchySpec,
    as_of_spreads: &BTreeMap<IssuerId, f64>,
    tags: &BTreeMap<IssuerId, IssuerTags>,
    generic_at_asof: f64,
    betas: &BTreeMap<IssuerId, IssuerBetas>,
    folded: &BTreeMap<IssuerId, Vec<bool>>,
    weights: &BTreeMap<IssuerId, f64>,
) -> Result<AnchorOutcome> {
    let num_levels = hierarchy.levels.len();
    // Resolve issuer → tags + bucket_paths.
    let mut bucket_paths: BTreeMap<IssuerId, Vec<String>> = BTreeMap::new();
    for issuer in as_of_spreads.keys() {
        let issuer_tags = tags.get(issuer).cloned().unwrap_or_default();
        let paths = hierarchy.bucket_paths(&issuer_tags).map_err(|missing| {
            validation_err(format!(
                "CreditCalibrator anchor: issuer {:?} missing tag {:?}",
                issuer.as_str(),
                missing
            ))
        })?;
        bucket_paths.insert(issuer.clone(), paths);
    }

    let betas_ref: BTreeMap<&IssuerId, &IssuerBetas> = betas.iter().collect();
    let peel = crate::factor::credit::peel::peel_single_observation(
        crate::factor::credit::peel::PeelSingleObservation {
            observed_spreads: as_of_spreads,
            observed_generic: generic_at_asof,
            betas: &betas_ref,
            bucket_paths: &bucket_paths,
            folded,
            num_levels,
            weights,
        },
    );
    let by_level: Vec<LevelAnchor> = peel
        .by_level
        .into_iter()
        .enumerate()
        .map(|(k, values)| LevelAnchor {
            level_index: k,
            dimension: hierarchy.levels[k].clone(),
            values,
        })
        .collect();

    Ok(AnchorOutcome {
        levels: LevelsAtAnchor {
            pc: generic_at_asof,
            by_level,
        },
        adder: peel.adder,
    })
}

/// Canonical factor ID order: PC first, then bucket factors sorted lexicographically.
pub(super) fn build_factor_id_order(
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
) -> Vec<FactorId> {
    let pc = FactorId::new(CREDIT_GENERIC_FACTOR_ID);
    let mut buckets: Vec<FactorId> = factor_returns
        .keys()
        .filter(|f| f.as_str() != CREDIT_GENERIC_FACTOR_ID)
        .cloned()
        .collect();
    buckets.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    let mut order = Vec::with_capacity(1 + buckets.len());
    order.push(pc);
    order.extend(buckets);
    order
}

/// Assemble `(FactorCorrelationMatrix, FactorModelConfig)` for a given strategy.
///
/// Returns the static correlation matrix and the covariance-embedded config.
pub(crate) fn assemble_factor_model_config(
    factor_id_order: &[FactorId],
    factor_variances: &BTreeMap<FactorId, f64>,
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
    hierarchy: &CreditHierarchySpec,
    issuer_betas: &[IssuerBetaRow],
    strategy: CovarianceStrategy,
    annualization_factor: f64,
) -> Result<(FactorCorrelationMatrix, FactorModelConfig)> {
    let mut factors = Vec::with_capacity(factor_id_order.len());
    for fid in factor_id_order {
        // Empty curve_ids are an honest no-op for hierarchy-derived factors;
        // downstream attribution uses the credit hierarchical matcher rather
        // than these placeholder curve mappings.
        factors.push(FactorDefinition {
            id: fid.clone(),
            factor_type: FactorType::Credit,
            market_mapping: MarketMapping::CurveParallel {
                curve_ids: vec![],
                units: BumpUnits::RateBp,
            },
            description: None,
        });
    }

    let n = factor_id_order.len();

    // Per-factor standard deviations (sqrt of annualized variances).
    let stds: Vec<f64> = factor_id_order
        .iter()
        .map(|fid| {
            let var = factor_variances.get(fid).copied().unwrap_or(0.0).max(0.0);
            var.sqrt()
        })
        .collect();

    let (static_correlation, cov_data) = match strategy {
        CovarianceStrategy::Diagonal => {
            // Identity correlation; Σ = diag(σ²).
            let corr = FactorCorrelationMatrix::identity(factor_id_order.to_vec());
            let mut data = vec![0.0_f64; n * n];
            for (i, fid) in factor_id_order.iter().enumerate() {
                let var = factor_variances.get(fid).copied().unwrap_or(0.0);
                data[i * n + i] = var.max(0.0);
            }
            (corr, data)
        }
        CovarianceStrategy::Ridge { alpha } => {
            if alpha < 0.0 {
                return Err(validation_err(format!(
                    "Ridge: alpha must be >= 0.0; got {alpha}"
                )));
            }
            // Sample correlation ρ (PSD-repaired if needed); Σ = D·ρ·D + α·I.
            let rho_flat = sample_correlation_flat(factor_id_order, factor_returns);
            // Repair ρ to PSD if needed (e.g. when n_factors > n_obs).
            let rho_flat = if validate_correlation_matrix(&rho_flat, n).is_ok() {
                rho_flat
            } else {
                nearest_correlation_matrix(&rho_flat, n, NearestCorrelationOpts::default())
                    .map_err(|e| {
                        validation_err(format!("Ridge: nearest_correlation_matrix failed: {e}"))
                    })?
            };
            let corr_data = flat_to_row_major(&rho_flat, n);
            let corr =
                FactorCorrelationMatrix::new(factor_id_order.to_vec(), corr_data).map_err(|e| {
                    validation_err(format!(
                        "Ridge: repaired correlation is not a valid correlation matrix: {e}"
                    ))
                })?;
            // Σ = D·ρ_repaired·D + α·I (row-major flat).
            let mut data = d_rho_d(&stds, &rho_flat, n);
            for i in 0..n {
                data[i * n + i] += alpha;
            }
            (corr, data)
        }
        CovarianceStrategy::FullSampleRepaired => {
            // Sample correlation ρ; repair if not PSD; Σ = D·ρ_repaired·D.
            let rho_flat = sample_correlation_flat(factor_id_order, factor_returns);
            let rho_repaired = if validate_correlation_matrix(&rho_flat, n).is_ok() {
                rho_flat
            } else {
                nearest_correlation_matrix(&rho_flat, n, NearestCorrelationOpts::default())
                    .map_err(|e| {
                        validation_err(format!(
                            "FullSampleRepaired: nearest_correlation_matrix failed: {e}"
                        ))
                    })?
            };
            let corr_data = flat_to_row_major(&rho_repaired, n);
            let corr =
                FactorCorrelationMatrix::new(factor_id_order.to_vec(), corr_data).map_err(|e| {
                    validation_err(format!(
                        "FullSampleRepaired: repaired correlation is not valid: {e}"
                    ))
                })?;
            let data = d_rho_d(&stds, &rho_repaired, n);
            (corr, data)
        }
        CovarianceStrategy::LedoitWolf => {
            // Ledoit-Wolf shrinkage over complete-case observations. Unlike
            // Ridge/FullSampleRepaired, both ρ and the covariance diagonal
            // come from the shrunk estimator; `vol_state` keeps the
            // vol-model variances (precedent: Ridge already stores a Σ whose
            // diagonal differs from vol_state by α).
            let (corr_rows, cov_ann) =
                ledoit_wolf_cov_and_corr(factor_id_order, factor_returns, annualization_factor)?;
            let corr =
                FactorCorrelationMatrix::new(factor_id_order.to_vec(), corr_rows).map_err(|e| {
                    validation_err(format!(
                        "LedoitWolf: derived correlation is not a valid correlation matrix: {e}"
                    ))
                })?;
            (corr, cov_ann)
        }
    };

    let covariance = FactorCovarianceMatrix::new(factor_id_order.to_vec(), cov_data)
        .map_err(|e| validation_err(format!("FactorCovarianceMatrix::new failed: {e}")))?;

    let matching = MatchingConfig::CreditHierarchical(CreditHierarchicalConfig {
        dependency_filter: Default::default(),
        hierarchy: hierarchy.clone(),
        issuer_betas: issuer_betas.to_vec(),
        // Calibrated artifacts know their issuer universe; a missing
        // issuer-id meta key on a credit position is a data gap that must
        // surface rather than silently proxy to PC-only.
        require_issuer_id: true,
    });

    let config = FactorModelConfig {
        factors,
        covariance,
        matching,
        pricing_mode: PricingMode::DeltaBased,
        risk_measure: Default::default(),
        bump_size: None,
        // Warn rather than the silent Residual default: a calibrated
        // artifact knows its factor universe, so a runtime issuer matching
        // a bucket outside it is a data gap worth surfacing.
        unmatched_policy: Some(crate::factor::UnmatchedPolicy::Warn),
    };

    Ok((static_correlation, config))
}

pub(super) fn build_factor_histories(
    dates: &[Date],
    space: &PanelSpace,
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
) -> Result<FactorHistories> {
    // Returns: histories align to dates[1..]. Levels: histories align to dates.
    let aligned_dates = match space {
        PanelSpace::Returns => dates.iter().skip(1).copied().collect::<Vec<_>>(),
        PanelSpace::Levels => dates.to_vec(),
    };
    // Histories are the official dense bp series. Task-1 completeness
    // guarantees every date is a real observation — do not 0-fill gaps.
    let mut values: BTreeMap<FactorId, Vec<f64>> = BTreeMap::new();
    for (fid, series) in factor_returns {
        let mut dense = Vec::with_capacity(series.len());
        for (t, value) in series.iter().enumerate() {
            let Some(v) = value else {
                return Err(validation_err(format!(
                    "CreditCalibrator: factor {} is missing an observation at \
                     history index {t}; factor histories must be a complete \
                     dense bp series",
                    fid.as_str()
                )));
            };
            dense.push(*v);
        }
        values.insert(fid.clone(), dense);
    }
    Ok(FactorHistories {
        dates: aligned_dates,
        values,
    })
}

pub(super) fn build_vol_state(
    factor_variances: &BTreeMap<FactorId, f64>,
    issuer_betas: &[IssuerBetaRow],
    vol_model: VolModelChoice,
) -> VolState {
    let mut factors = BTreeMap::new();
    for (fid, var) in factor_variances {
        let model = match vol_model {
            VolModelChoice::Sample => FactorVolModel::Sample { variance: *var },
            VolModelChoice::Ewma { lambda } => FactorVolModel::Ewma {
                lambda,
                variance: *var,
            },
        };
        factors.insert(fid.clone(), model);
    }
    let mut idiosyncratic = BTreeMap::new();
    for row in issuer_betas {
        let var = row.adder_vol_annualized.powi(2);
        let model = match vol_model {
            VolModelChoice::Sample => IdiosyncraticVolModel::Sample { variance: var },
            VolModelChoice::Ewma { lambda } => IdiosyncraticVolModel::Ewma {
                lambda,
                variance: var,
            },
        };
        idiosyncratic.insert(row.issuer_id.clone(), model);
    }
    VolState {
        factors,
        idiosyncratic,
    }
}

pub(super) fn build_diagnostics(
    modes: &BTreeMap<IssuerId, IssuerBetaMode>,
    bucket_sizes_per_level: Vec<BTreeMap<String, usize>>,
    fold_ups: Vec<FoldUpRecord>,
    fit_quality: &BTreeMap<IssuerId, FitQuality>,
    tag_taxonomy: BTreeMap<String, BTreeSet<String>>,
) -> CalibrationDiagnostics {
    let mut mode_counts: BTreeMap<String, usize> = BTreeMap::new();
    mode_counts.insert("issuer_beta".to_owned(), 0);
    mode_counts.insert("bucket_only".to_owned(), 0);
    for mode in modes.values() {
        let key = match mode {
            IssuerBetaMode::IssuerBeta => "issuer_beta",
            IssuerBetaMode::BucketOnly => "bucket_only",
        };
        *mode_counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    // R² histogram: 5 bins [0.0, 0.2, 0.4, 0.6, 0.8, 1.0]. Values < 0 fall into
    // the lowest bin; values > 1 (rare in OLS but possible if mean-shifted) fall
    // into the highest. Bin keys are stable strings for deterministic JSON.
    let r_squared_histogram = if fit_quality.is_empty() {
        None
    } else {
        let mut hist: BTreeMap<String, usize> = BTreeMap::new();
        for label in [
            "[0.0,0.2)",
            "[0.2,0.4)",
            "[0.4,0.6)",
            "[0.6,0.8)",
            "[0.8,1.0]",
        ] {
            hist.insert(label.to_owned(), 0);
        }
        for fq in fit_quality.values() {
            let r2 = fq.r_squared.clamp(0.0, 1.0);
            let key = if r2 < 0.2 {
                "[0.0,0.2)"
            } else if r2 < 0.4 {
                "[0.2,0.4)"
            } else if r2 < 0.6 {
                "[0.4,0.6)"
            } else if r2 < 0.8 {
                "[0.6,0.8)"
            } else {
                "[0.8,1.0]"
            };
            *hist.entry(key.to_owned()).or_insert(0) += 1;
        }
        Some(hist)
    };

    CalibrationDiagnostics {
        mode_counts,
        bucket_sizes_per_level,
        fold_ups,
        r_squared_histogram,
        tag_taxonomy,
    }
}
