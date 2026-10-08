use std::collections::BTreeMap;

use finstack_quant_core::types::IssuerId;
use finstack_quant_core::Result;

use super::config::VolModelChoice;
use super::validation::validation_err;
use crate::factor::credit::hierarchy::AdderVolSource;
use crate::factor::FactorId;

/// Step 6 (part A): per-issuer adder annualized **vol** (std dev) computed
/// from the residual series after the last level, for every issuer with at
/// least 2 valid residual observations (any
/// [`IssuerBetaMode`][crate::factor::credit::hierarchy::IssuerBetaMode]).
///
/// Returns the annualized standard deviation `sqrt(var * annualization_factor)`.
/// Issuers with fewer than 2 valid residuals are excluded — they receive their
/// vol via the cascade in [`assign_adder_vol`].
///
/// `BucketOnly` issuers are included deliberately: under
/// [`IssuerBetaPolicy::GloballyOff`][crate::factor::credit::hierarchy::IssuerBetaPolicy]
/// every issuer is `BucketOnly`, and skipping
/// them here would leave the whole model with 0.0 idiosyncratic vol.
///
/// A **numerically-zero** residual series is excluded. Exact IEEE `0.0` is
/// the signature of an issuer alone in its deepest bucket (the residual
/// minus its own bucket mean is exactly `0.0`). Leave-one-out peel on a
/// near-linear panel can leave ~`1e-18` bp of float dust, which is still
/// not a real idiosyncratic series. Recording either as a `FromHistory`
/// estimate of ~0 would silently zero the issuer's specific risk. Excluded
/// issuers fall through the [`assign_adder_vol`] cascade to the bucket peer
/// proxy or the global default instead. Genuine residuals from multi-member
/// buckets carry noise well above `1e-12` bp and are unaffected.
///
/// Variance uses the unbiased sample estimator (`n − 1`, Bessel's correction),
/// matching [`factor_variances`]; sparse adders can have short effective
/// histories where the `n` vs `n − 1` distinction is material. Under
/// [`VolModelChoice::Ewma`] the same EWMA recursion replaces the sample
/// estimator; the `< 2` observation gate is unchanged.
pub(super) fn adder_vols_from_history(
    adder_series: &BTreeMap<IssuerId, Vec<Option<f64>>>,
    vol_model: VolModelChoice,
    annualization_factor: f64,
) -> BTreeMap<IssuerId, f64> {
    let mut out = BTreeMap::new();
    for (issuer, series) in adder_series {
        let n_valid = series.iter().filter(|v| v.is_some()).count();
        if n_valid < 2 {
            continue;
        }
        // Degenerate residual: every observed residual is numerically zero.
        // Exact IEEE 0.0 is the singleton-bucket signature; leave-one-out
        // peel on a near-linear synthetic panel can leave ~1e-18 bp of
        // float noise, which is still not a real idiosyncratic series
        // (genuine residual noise is ~1e-16 bp and above). Treating that
        // dust as `FromHistory` would persist a ~1e-18 annualized vol that
        // is not a JSON f64 fixed point.
        const DEGENERATE_RESIDUAL_BP: f64 = 1e-12;
        if series
            .iter()
            .flatten()
            .all(|v| v.abs() < DEGENERATE_RESIDUAL_BP)
        {
            continue;
        }
        let ann_var = match vol_model {
            VolModelChoice::Sample => sample_variance_annualized(series, annualization_factor),
            VolModelChoice::Ewma { lambda } => ewma_variance(series, lambda, annualization_factor),
        };
        out.insert(issuer.clone(), ann_var.sqrt());
    }
    out
}

/// Step 6 (part B): build the peer proxy index for the bucket-peer cascade.
///
/// Returns a `Vec` (indexed by hierarchy level `k`) of `BTreeMap` from
/// `bucket_path_at_level_k` to the list of `FromHistory` adder vols of all
/// peers in that bucket.
///
/// Only issuers present in `from_history_vols` (i.e., successful `FromHistory`
/// fits) contribute to the index; issuers with insufficient history are
/// implicitly excluded because they have no entry in `from_history_vols`.
///
/// The returned structure is deterministic: `BTreeMap` key order and `Vec`
/// element order both follow `BTreeMap` iteration (lexicographic).
pub(super) fn build_peer_proxy_index(
    from_history_vols: &BTreeMap<IssuerId, f64>,
    bucket_paths: &BTreeMap<IssuerId, Vec<String>>,
    num_levels: usize,
) -> Vec<BTreeMap<String, Vec<f64>>> {
    // One BTreeMap<bucket_path, vols> per hierarchy level.
    let mut index: Vec<BTreeMap<String, Vec<f64>>> = vec![BTreeMap::new(); num_levels];

    // Iterate in BTreeMap order (sorted by issuer_id) for determinism.
    for (issuer, vol) in from_history_vols {
        if let Some(paths) = bucket_paths.get(issuer) {
            for (k, path) in paths.iter().enumerate() {
                if k < num_levels {
                    index[k].entry(path.clone()).or_default().push(*vol);
                }
            }
        }
    }

    // Sort each bucket's vol list for fully deterministic mean computation.
    for level_map in &mut index {
        for vols in level_map.values_mut() {
            vols.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    index
}

/// Step 6 (part C): assign an `(adder_vol, AdderVolSource)` pair to a single
/// issuer using the full caller → history → peer-proxy → global → zero cascade.
///
/// # Cascade order
///
/// 1. **Caller override** (present in `idiosyncratic_overrides`): use it with
///    `AdderVolSource::CallerSupplied`. Wins for both `IssuerBeta` and `BucketOnly`.
/// 2. **Successful `FromHistory` fit** (any mode): use the history-derived
///    vol with `AdderVolSource::FromHistory`.
/// 3. **Bucket-peer proxy** (deepest level first, then walking up): find the
///    deepest level `k` such that the issuer has a path at level `k` and there
///    is at least one peer with a `FromHistory` vol in that bucket.
///    Use the mean of those peers' vols with
///    `AdderVolSource::BucketPeerProxy { peer_bucket }`.
/// 4. **Global default**: if no peers exist anywhere up the hierarchy, use the
///    mean of *all* `FromHistory` vols across the entire model.
///    `AdderVolSource::Default`.
/// 5. **No data at all**: `(0.0, AdderVolSource::Default)` — applies when
///    `from_history_vols` is completely empty (e.g. single-observation panel).
pub(super) fn assign_adder_vol(
    issuer_id: &IssuerId,
    from_history_vols: &BTreeMap<IssuerId, f64>,
    peer_proxy_index: &[BTreeMap<String, Vec<f64>>],
    bucket_paths: &BTreeMap<IssuerId, Vec<String>>,
    idiosyncratic_overrides: &BTreeMap<IssuerId, f64>,
    num_levels: usize,
) -> (f64, AdderVolSource) {
    // 1. Caller override wins for any mode.
    if let Some(&override_vol) = idiosyncratic_overrides.get(issuer_id) {
        return (override_vol, AdderVolSource::CallerSupplied);
    }

    // 2. Successful FromHistory fit (any mode).
    if let Some(&vol) = from_history_vols.get(issuer_id) {
        return (vol, AdderVolSource::FromHistory);
    }

    // 3. Bucket-peer proxy cascade: walk from deepest level to broadest.
    if let Some(paths) = bucket_paths.get(issuer_id) {
        for k in (0..num_levels).rev() {
            if k < paths.len() && k < peer_proxy_index.len() {
                let bucket = &paths[k];
                if let Some(peer_vols) = peer_proxy_index[k].get(bucket) {
                    if !peer_vols.is_empty() {
                        return (
                            mean_of(peer_vols).unwrap_or(0.0),
                            AdderVolSource::BucketPeerProxy {
                                peer_bucket: bucket.clone(),
                            },
                        );
                    }
                }
            }
        }
    }

    // 4. Global mean of all IssuerBeta FromHistory vols.
    if !from_history_vols.is_empty() {
        let global_vols: Vec<f64> = from_history_vols.values().copied().collect();
        return (
            mean_of(&global_vols).unwrap_or(0.0),
            AdderVolSource::Default,
        );
    }

    // 5. No IssuerBeta data anywhere: hardcoded 0.0.
    (0.0, AdderVolSource::Default)
}

/// Compute the arithmetic mean of a slice. Returns `None` for an empty slice.
fn mean_of(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / (values.len() as f64))
}

/// Annualized unbiased sample variance over the `Some` entries of a sparse
/// series. Returns `0.0` when fewer than 2 valid observations exist.
///
/// Extracted from the previous inline bodies of [`factor_variances`] and
/// [`adder_vols_from_history`] so both estimators share one implementation.
fn sample_variance_annualized(series: &[Option<f64>], annualization_factor: f64) -> f64 {
    let valid: Vec<f64> = series.iter().filter_map(|v| *v).collect();
    let n = valid.len();
    if n < 2 {
        return 0.0;
    }
    let nf = n as f64;
    let mean = valid.iter().sum::<f64>() / nf;
    let var = valid.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (nf - 1.0);
    (var * annualization_factor).max(0.0)
}

/// Annualized RiskMetrics EWMA variance over the `Some` entries of a sparse
/// series.
///
/// Implements the finite-window normalized exponentially weighted variance
/// with the RiskMetrics zero-mean convention (squared returns, no demeaning):
///
/// ```text
/// σ² = (1 − λ) · Σ_{t=0}^{T−1} λ^{T−1−t} · r_t²  /  (1 − λ^T)
/// ```
///
/// computed by the serial recursion `acc ← λ·acc + (1 − λ)·r²` (oldest →
/// newest) followed by the `1 − λ^T` normalization, then multiplied by
/// `annualization_factor`. Missing (`None`) entries are skipped; the recency
/// ordering is taken over the observed entries only. Returns `0.0` when fewer
/// than 2 valid observations exist (mirrors [`sample_variance_annualized`]).
///
/// # References
///
/// - Longerstaey, J., & Spencer, M. (1996). *RiskMetrics — Technical
///   Document* (4th ed.). J.P. Morgan/Reuters. §5.2 (recommends λ = 0.94 for
///   daily data, λ = 0.97 for monthly). `docs/REFERENCES.md#jpmorgan1996RiskMetrics`
pub(super) fn ewma_variance(series: &[Option<f64>], lambda: f64, annualization_factor: f64) -> f64 {
    let valid: Vec<f64> = series.iter().filter_map(|v| *v).collect();
    let n = valid.len();
    if n < 2 {
        return 0.0;
    }
    let mut acc = 0.0_f64;
    for r in &valid {
        acc = lambda * acc + (1.0 - lambda) * r * r;
    }
    // λ ∈ (0, 1) (validated in `validate_calibration_config`) and n ≥ 2, so
    // the normalizer is strictly positive.
    let norm = 1.0 - lambda.powf(n as f64);
    ((acc / norm) * annualization_factor).max(0.0)
}

/// Step 8: per-factor annualized variance under the configured vol model.
///
/// Returns annualized factor variances (already squared) suitable for placing
/// on the diagonal of `Σ`. Values are **not** std devs; callers must not take
/// a square root before inserting into the covariance matrix. Sparse entries
/// (`None`, empty-bucket dates) are skipped by both estimators.
///
/// - [`VolModelChoice::Sample`]: unbiased (Bessel-corrected) sample variance;
///   see [`sample_variance_annualized`].
/// - [`VolModelChoice::Ewma`]: RiskMetrics exponentially weighted variance;
///   see [`ewma_variance`].
pub(crate) fn factor_variances(
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
    vol_model: VolModelChoice,
    annualization_factor: f64,
) -> BTreeMap<FactorId, f64> {
    let mut out = BTreeMap::new();
    for (fid, series) in factor_returns {
        let var = match vol_model {
            VolModelChoice::Sample => sample_variance_annualized(series, annualization_factor),
            VolModelChoice::Ewma { lambda } => ewma_variance(series, lambda, annualization_factor),
        };
        out.insert(fid.clone(), var);
    }
    out
}

/// Compute the flat (row-major) sample correlation matrix over the factor return
/// series. For each pair `(i, j)` only dates where BOTH factors have `Some(_)`
/// are used (pairwise complete observation).
///
/// If a factor has fewer than 2 valid observations, all off-diagonal entries in
/// its row/column are set to 0.0 (i.e. the factor is treated as uncorrelated).
///
/// Means, covariance, and both variances are all formed over the same
/// pairwise-overlap window (see the inline comment in the loop body), so each
/// entry is a proper sample correlation in `[-1, 1]`.
///
/// # PSD guarantee
///
/// Off-diagonal entries are clamped to `[-1, 1]` but the resulting matrix is
/// **not guaranteed to be positive semi-definite** (e.g. when the number of
/// factors exceeds the number of observations). Callers that require a PSD
/// matrix must use [`CovarianceStrategy::Ridge`][super::CovarianceStrategy] or
/// [`CovarianceStrategy::FullSampleRepaired`][super::CovarianceStrategy], both
/// of which apply nearest-correlation repair when the raw sample matrix is not
/// PSD.
pub(super) fn sample_correlation_flat(
    factor_id_order: &[FactorId],
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
) -> Vec<f64> {
    let n = factor_id_order.len();
    if n == 0 {
        return vec![];
    }

    // Materialise each factor's series (missing factors get an empty series).
    let empty: Vec<Option<f64>> = vec![];
    let series: Vec<&Vec<Option<f64>>> = factor_id_order
        .iter()
        .map(|fid| factor_returns.get(fid).unwrap_or(&empty))
        .collect();

    // Flat row-major result — diagonal = 1.0, off-diagonal filled below.
    let mut rho = vec![0.0_f64; n * n];
    for i in 0..n {
        rho[i * n + i] = 1.0;
    }

    for i in 0..n {
        for j in (i + 1)..n {
            // Pairwise: use only dates where both factors are observed.
            //
            // The covariance, both variances, and the means MUST all be formed
            // over the same pairwise-overlap window. A factor's marginal mean
            // (over its full valid history) generally differs from its mean on
            // the subset of dates where the *other* factor is also observed; on
            // a sparse panel the two windows can be wildly different. Demeaning
            // with the marginal mean while summing over the overlap yields a
            // ratio that is not a Pearson correlation (it can even exceed 1
            // before the clamp). Compute the overlap mean for each factor here
            // so the assembled entry is a proper sample correlation.
            // Two passes over the same slices (no per-pair Vec materialisation):
            // pass 1 accumulates the overlap means, pass 2 the demeaned cross-
            // and self-products. This keeps the exact two-pass demeaning math
            // while avoiding O(n²) heap allocations across the factor pairs.
            let mut sum_i = 0.0_f64;
            let mut sum_j = 0.0_f64;
            let mut count = 0usize;
            for (vi_opt, vj_opt) in series[i].iter().zip(series[j].iter()) {
                if let (Some(vi), Some(vj)) = (*vi_opt, *vj_opt) {
                    sum_i += vi;
                    sum_j += vj;
                    count += 1;
                }
            }
            let corr = if count >= 2 {
                let nf = count as f64;
                let mean_i = sum_i / nf;
                let mean_j = sum_j / nf;
                let mut cov_ij = 0.0_f64;
                let mut var_i = 0.0_f64;
                let mut var_j = 0.0_f64;
                for (vi_opt, vj_opt) in series[i].iter().zip(series[j].iter()) {
                    if let (Some(vi), Some(vj)) = (*vi_opt, *vj_opt) {
                        let di = vi - mean_i;
                        let dj = vj - mean_j;
                        cov_ij += di * dj;
                        var_i += di * di;
                        var_j += dj * dj;
                    }
                }
                // `cov_ij`, `var_i`, `var_j` share the same denominator
                // (n − 1), so it cancels in the ratio and need not be applied.
                if var_i > 0.0 && var_j > 0.0 {
                    (cov_ij / (var_i * var_j).sqrt()).clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };
            rho[i * n + j] = corr;
            rho[j * n + i] = corr;
        }
    }

    rho
}

/// Compute `Σ = D · ρ · D` from standard deviations and flat correlation matrix.
///
/// Returns flat row-major `n×n` covariance matrix.
pub(super) fn d_rho_d(stds: &[f64], rho_flat: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..n {
            out[i * n + j] = stds[i] * rho_flat[i * n + j] * stds[j];
        }
    }
    out
}

/// Ledoit-Wolf covariance/correlation assembly over the complete-case rows of
/// the sparse factor-return panel.
///
/// Builds the row-major `T_c × n` observation matrix from dates where every
/// factor in `factor_id_order` is observed, delegates to
/// [`finstack_quant_core::math::linalg::ledoit_wolf_shrinkage`], annualizes
/// the shrunk covariance, and derives the (scale-invariant) correlation
/// `ρ_ij = Σ*_ij / √(Σ*_ii · Σ*_jj)`.
///
/// Returns `(correlation_rows, annualized_covariance_flat)`.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when fewer than 2
/// complete observations exist or the core shrinkage routine rejects the
/// panel.
pub(super) fn ledoit_wolf_cov_and_corr(
    factor_id_order: &[FactorId],
    factor_returns: &BTreeMap<FactorId, Vec<Option<f64>>>,
    annualization_factor: f64,
) -> Result<(Vec<Vec<f64>>, Vec<f64>)> {
    let n = factor_id_order.len();
    let empty: Vec<Option<f64>> = vec![];
    let series: Vec<&Vec<Option<f64>>> = factor_id_order
        .iter()
        .map(|fid| factor_returns.get(fid).unwrap_or(&empty))
        .collect();
    let t_max = series.iter().map(|s| s.len()).max().unwrap_or(0);

    // Complete-case rows: dates where every factor is observed.
    let mut rows: Vec<f64> = Vec::new();
    let mut t_complete = 0usize;
    for date_idx in 0..t_max {
        let mut row = Vec::with_capacity(n);
        let mut complete = true;
        for s in &series {
            match s.get(date_idx).copied().flatten() {
                Some(v) => row.push(v),
                None => {
                    complete = false;
                    break;
                }
            }
        }
        if complete {
            rows.extend_from_slice(&row);
            t_complete += 1;
        }
    }
    if t_complete < 2 {
        return Err(validation_err(format!(
            "LedoitWolf: only {t_complete} complete observation(s) across all {n} factor(s); \
             need at least 2. Use Ridge or FullSampleRepaired for sparse panels."
        )));
    }

    let lw = finstack_quant_core::math::linalg::ledoit_wolf_shrinkage(&rows, t_complete, n)
        .map_err(|e| validation_err(format!("LedoitWolf: shrinkage failed: {e}")))?;

    let mut cov_ann = lw.covariance.clone();
    for v in &mut cov_ann {
        *v *= annualization_factor;
    }

    let mut corr_rows: Vec<Vec<f64>> = Vec::with_capacity(n);
    for i in 0..n {
        let mut row = Vec::with_capacity(n);
        for j in 0..n {
            if i == j {
                row.push(1.0);
                continue;
            }
            let var_i = lw.covariance[i * n + i];
            let var_j = lw.covariance[j * n + j];
            if var_i > 0.0 && var_j > 0.0 {
                row.push((lw.covariance[i * n + j] / (var_i * var_j).sqrt()).clamp(-1.0, 1.0));
            } else {
                row.push(0.0);
            }
        }
        corr_rows.push(row);
    }

    Ok((corr_rows, cov_ann))
}
