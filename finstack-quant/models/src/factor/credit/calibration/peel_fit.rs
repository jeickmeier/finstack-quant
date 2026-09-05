use std::collections::BTreeMap;

use finstack_quant_core::types::IssuerId;

use super::config::{BetaShrinkage, CreditCalibrationConfig};
use super::panel::WorkingPanel;
use crate::factor::credit::hierarchy::{
    dimension_key, CreditHierarchySpec, FitQuality, IssuerBetaMode, IssuerBetas, IssuerTags,
};
use crate::factor::matching::{bucket_factor_id, CREDIT_GENERIC_FACTOR_ID};
use crate::factor::FactorId;

/// Outcome of the PC + per-level peel (steps 4 + 5).
pub(super) struct PeelOutcome {
    /// Calibrated betas per issuer.
    pub(super) betas: BTreeMap<IssuerId, IssuerBetas>,
    /// Adder return series per issuer (length = working panel length).
    pub(super) adder_series: BTreeMap<IssuerId, Vec<Option<f64>>>,
    /// PC-fit quality stats for IssuerBeta issuers.
    pub(super) fit_quality: BTreeMap<IssuerId, FitQuality>,
    /// Per-level fit quality for IssuerBeta issuers, aligned with
    /// `betas.levels` (`None` where no fit ran: folded level or degenerate
    /// regressor). BucketOnly issuers have no entry.
    pub(super) level_fit_quality: BTreeMap<IssuerId, Vec<Option<FitQuality>>>,
    /// Per-factor return series (sparse), keyed by FactorId.
    /// Includes the generic factor (always `Some`) and every surviving bucket
    /// factor. Bucket factors carry `None` at dates where all bucket members
    /// were absent ("empty-bucket" sentinel). Use
    /// [`factor_variances`][super::statistics::factor_variances] to compute
    /// variance over only the observed entries, and flatten to
    /// `0.0`-substituted `Vec<f64>` when writing to
    /// [`FactorHistories`][crate::factor::credit::hierarchy::FactorHistories].
    pub(super) factor_returns: BTreeMap<FactorId, Vec<Option<f64>>>,
}

/// Runs the PC + per-level peel over the working panel.
///
/// `weights` are **per-date** bucket weights aligned to the working panel
/// (see [`super::panel::issuer_bucket_weight_series`]): under DTS weighting
/// each date's bucket mean uses that date's contemporaneous DTS, so no
/// as-of information leaks into historical factor construction.
pub(super) fn run_peel(
    config: &CreditCalibrationConfig,
    panel: &WorkingPanel,
    modes: &BTreeMap<IssuerId, IssuerBetaMode>,
    bucket_paths: &BTreeMap<IssuerId, Vec<String>>,
    folded: &BTreeMap<IssuerId, Vec<bool>>,
    weights: &BTreeMap<IssuerId, Vec<f64>>,
) -> PeelOutcome {
    let n = panel.generic.len();
    let num_levels = config.hierarchy.levels.len();

    let mut betas: BTreeMap<IssuerId, IssuerBetas> = BTreeMap::new();
    let mut residuals: BTreeMap<IssuerId, Vec<Option<f64>>> = BTreeMap::new();
    let mut fit_quality: BTreeMap<IssuerId, FitQuality> = BTreeMap::new();
    let mut level_fit_quality: BTreeMap<IssuerId, Vec<Option<FitQuality>>> = BTreeMap::new();
    let mut factor_returns: BTreeMap<FactorId, Vec<Option<f64>>> = BTreeMap::new();

    // Generic factor is always fully observed; wrap in Some for type uniformity.
    factor_returns.insert(
        FactorId::new(CREDIT_GENERIC_FACTOR_ID),
        panel.generic.iter().map(|v| Some(*v)).collect(),
    );

    // Reused across every OLS / fit-quality gather and every LOO series.
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut loo_series = Vec::new();

    // Step 4 — PC peel.
    for (issuer, series) in &panel.issuers {
        let mode = modes
            .get(issuer)
            .copied()
            .unwrap_or(IssuerBetaMode::BucketOnly);
        let beta_pc = match mode {
            IssuerBetaMode::BucketOnly => 1.0,
            IssuerBetaMode::IssuerBeta => {
                gather_aligned_dense(series, &panel.generic, &mut xs, &mut ys);
                let raw = ols_slope_pairs(&ys, &xs).unwrap_or(1.0);
                let shrunk = apply_shrinkage(&config.beta_shrinkage, raw);
                // Diagnostics on the through-origin peel residual y − β g,
                // not the intercept-form residual used by the OLS slope.
                if let Some(fq) = peel_fit_quality_from_pairs(&ys, &xs, shrunk) {
                    fit_quality.insert(issuer.clone(), fq);
                }
                level_fit_quality.insert(issuer.clone(), vec![None; num_levels]);
                shrunk
            }
        };
        let res_pc: Vec<Option<f64>> = series
            .iter()
            .enumerate()
            .map(|(t, v)| v.as_ref().map(|s| s - beta_pc * panel.generic[t]))
            .collect();
        residuals.insert(issuer.clone(), res_pc);
        // Initialize beta row with per-level betas at 0.0; the per-level peel
        // below overwrites entries for non-folded buckets. Folded buckets stay
        // at 0.0 (the contractual sentinel for "skip this level").
        betas.insert(
            issuer.clone(),
            IssuerBetas {
                pc: beta_pc,
                levels: vec![0.0; num_levels],
            },
        );
    }

    // Step 5 — per-level peel.
    // Range-based loop over the hierarchy level index `k`. `k` indexes into
    // multiple parallel structures (`bucket_paths[issuer][k]`, `folded[i][k]`,
    // `betas[issuer].levels[k]`); enumerate-iterating any one of them would
    // not eliminate indexing into the others.
    let mut prev_bucket_members: BTreeMap<&str, Vec<&IssuerId>> = BTreeMap::new();
    #[allow(clippy::needless_range_loop)]
    for k in 0..num_levels {
        // 5a. For each surviving (non-folded) bucket, compute factor return series.
        // Folded issuers contribute β=0 at this level and DO NOT participate
        // in computing the bucket factor return; they simply propagate
        // residuals unchanged.
        let mut bucket_members: BTreeMap<&str, Vec<&IssuerId>> = BTreeMap::new();
        for issuer in panel.issuers.keys() {
            let folded_at_k = folded
                .get(issuer)
                .map(|f| f.get(k).copied().unwrap_or(false))
                .unwrap_or(false);
            if folded_at_k {
                continue;
            }
            let path = bucket_paths[issuer][k].as_str();
            bucket_members.entry(path).or_default().push(issuer);
        }

        // Full-bucket weighted mean plus the (sum, weight-sum) totals that
        // make leave-one-out O(T) per issuer instead of O(members · T).
        // Totals are taken from residuals *before* any issuer at this level
        // is peeled, so LOO never sees already-updated peers.
        let mut bucket_stats: BTreeMap<&str, BucketStats> = BTreeMap::new();
        for (bucket, members) in &bucket_members {
            bucket_stats.insert(
                *bucket,
                bucket_weighted_stats(members, weights, &residuals, n),
            );
        }

        // 5b. For each member, fit / set its level-k beta and update its residual.
        for (bucket, members) in &bucket_members {
            let stats = &bucket_stats[bucket];
            let factor_series = &stats.means;
            // A child that does not refine its parent has the same members;
            // LOO would regress on a peer leftover, not a new common factor.
            let non_refining = k > 0
                && parent_bucket_path(bucket)
                    .and_then(|parent| prev_bucket_members.get(parent))
                    .is_some_and(|parent_members| same_member_set(parent_members, members));
            static EMPTY_WEIGHTS: &[f64] = &[];
            for issuer in members {
                let mode = modes
                    .get(*issuer)
                    .copied()
                    .unwrap_or(IssuerBetaMode::BucketOnly);
                let exclude_w = weights
                    .get(*issuer)
                    .map(Vec::as_slice)
                    .unwrap_or(EMPTY_WEIGHTS);
                let beta_k = {
                    let r_series = residuals.get(*issuer).map(Vec::as_slice).unwrap_or(&[]);
                    match mode {
                        IssuerBetaMode::BucketOnly => 1.0,
                        IssuerBetaMode::IssuerBeta => {
                            // Gate on the *stored* full-bucket factor: a
                            // non-refining child level has a ~0 full mean even
                            // when the LOO series (the other names) still varies.
                            // Singleton / degenerate LOO then falls back to unit β.
                            gather_aligned_sparse(r_series, factor_series, &mut xs, &mut ys);
                            let fitted = if non_refining || ols_slope_pairs(&ys, &xs).is_none() {
                                None
                            } else {
                                fill_loo_series(r_series, exclude_w, stats, &mut loo_series);
                                gather_aligned_sparse(r_series, &loo_series, &mut xs, &mut ys);
                                ols_slope_pairs(&ys, &xs)
                            };
                            let shrunk =
                                apply_shrinkage(&config.beta_shrinkage, fitted.unwrap_or(1.0));
                            // Diagnostics on the actual peel residual y − β x_full.
                            if fitted.is_some() {
                                gather_aligned_sparse(r_series, factor_series, &mut xs, &mut ys);
                                if let Some(slots) = level_fit_quality.get_mut(*issuer) {
                                    slots[k] = peel_fit_quality_from_pairs(&ys, &xs, shrunk);
                                }
                            }
                            shrunk
                        }
                    }
                };
                if let Some(b) = betas.get_mut(*issuer) {
                    b.levels[k] = beta_k;
                }
                // Propagate `None` when the factor is unavailable at date t.
                if let Some(r) = residuals.get_mut(*issuer) {
                    for (t, slot) in r.iter_mut().enumerate() {
                        *slot = match (*slot, factor_series.get(t).copied().flatten()) {
                            (Some(x), Some(f)) => Some(x - beta_k * f),
                            _ => None,
                        };
                    }
                }
            }
        }

        // Folded issuers: beta_k stays at 0.0 (already initialized) and
        // residual is unchanged (no subtraction). We've simply skipped them.

        // Record bucket factor return series in the canonical FactorId form.
        // The sparse `Vec<Option<f64>>` is stored directly in `factor_returns`;
        // flattening to a dense `Vec<f64>` for `FactorHistories` happens in
        // `build_factor_histories` (missing dates are a validation error).
        // `factor_variances` computes variance over only the `Some` entries.
        for (bucket, stats) in bucket_stats {
            // Reconstruct an IssuerTags for path → use a synthetic helper:
            // We need bucket_factor_id. The existing helper requires IssuerTags;
            // we don't have them here, but we can build a minimal tag map by
            // splitting the path on '.'.
            let tags = synth_tags_from_path(&config.hierarchy, bucket);
            // bucket_factor_id is `Some(_)` whenever every dimension key in
            // `levels[0..=k]` appears in `tags` — which `synth_tags_from_path`
            // guarantees by construction. Fall through with `continue` defensively
            // rather than panic via `.expect()`.
            let Some(factor_id) = bucket_factor_id(&config.hierarchy, &tags, k) else {
                continue;
            };
            factor_returns.insert(factor_id, stats.means);
        }
        prev_bucket_members = bucket_members;
    }

    PeelOutcome {
        betas,
        adder_series: residuals,
        fit_quality,
        level_fit_quality,
        factor_returns,
    }
}

/// Full-bucket weighted means plus the totals needed for O(T) leave-one-out.
struct BucketStats {
    means: Vec<Option<f64>>,
    sums: Vec<f64>,
    wsums: Vec<f64>,
}

/// Weighted mean (and running totals) of observed member residuals.
///
/// `weights` are per-date series aligned to the working panel; the weight
/// applied to a member's residual at date `t` is that member's weight at `t`.
fn bucket_weighted_stats(
    members: &[&IssuerId],
    weights: &BTreeMap<IssuerId, Vec<f64>>,
    residuals: &BTreeMap<IssuerId, Vec<Option<f64>>>,
    n: usize,
) -> BucketStats {
    let mut means = Vec::with_capacity(n);
    let mut sums = vec![0.0; n];
    let mut wsums = vec![0.0; n];
    for t in 0..n {
        let mut vsum = 0.0;
        let mut wsum = 0.0;
        for issuer in members {
            let Some(v) = residuals
                .get(*issuer)
                .and_then(|r| r.get(t).copied().flatten())
            else {
                continue;
            };
            let Some(w) = weights
                .get(*issuer)
                .and_then(|series| series.get(t))
                .copied()
            else {
                continue;
            };
            if w <= 0.0 {
                continue;
            }
            vsum += w * v;
            wsum += w;
        }
        sums[t] = vsum;
        wsums[t] = wsum;
        means.push((wsum > 0.0).then_some(vsum / wsum));
    }
    BucketStats { means, sums, wsums }
}

/// Leave-one-out weighted mean from precomputed bucket totals.
///
/// Algebraically identical to summing every other member; the last bits
/// may differ from a fresh sum because `(Σ − wᵢrᵢ) / (W − wᵢ)` is not
/// bitwise-associative with a direct residual walk.
///
/// Dates where `exclude` is the only observed member (or the remaining
/// members are all missing / zero-weight) emit `None` so OLS falls back
/// to unit β. If `exclude` did not contribute to the full-bucket mean at
/// date `t` (missing residual or non-positive weight), LOO equals the
/// full mean. `exclude_weights` is the excluded member's per-date weight
/// series; a date beyond its length is treated as non-contributing.
fn fill_loo_series(
    exclude_residual: &[Option<f64>],
    exclude_weights: &[f64],
    stats: &BucketStats,
    out: &mut Vec<Option<f64>>,
) {
    out.clear();
    out.reserve(stats.means.len());
    for t in 0..stats.means.len() {
        let mean = stats.means.get(t).copied().flatten();
        let sum = stats.sums.get(t).copied().unwrap_or(0.0);
        let wsum = stats.wsums.get(t).copied().unwrap_or(0.0);
        let exclude_val = exclude_residual.get(t).copied().flatten();
        let exclude_w = exclude_weights.get(t).copied().unwrap_or(0.0);
        out.push(loo_at(exclude_val, exclude_w, sum, wsum, mean));
    }
}

fn loo_at(
    exclude_val: Option<f64>,
    exclude_w: f64,
    sum: f64,
    wsum: f64,
    mean: Option<f64>,
) -> Option<f64> {
    let m = mean?;
    match exclude_val {
        Some(v) if exclude_w > 0.0 => {
            let loo_w = wsum - exclude_w;
            (loo_w > 0.0).then_some((sum - exclude_w * v) / loo_w)
        }
        _ => Some(m),
    }
}

/// Parent path of a dotted bucket path (`"IG.TECH"` → `"IG"`).
fn parent_bucket_path(path: &str) -> Option<&str> {
    path.rfind('.').map(|i| &path[..i])
}

/// True when both member lists name the same issuers (order-independent).
fn same_member_set(left: &[&IssuerId], right: &[&IssuerId]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut ids: BTreeMap<&IssuerId, usize> = BTreeMap::new();
    for issuer in left {
        *ids.entry(*issuer).or_insert(0) += 1;
    }
    for issuer in right {
        match ids.get_mut(*issuer) {
            Some(count) if *count > 0 => *count -= 1,
            _ => return false,
        }
    }
    ids.values().all(|c| *c == 0)
}

/// Reconstruct an [`IssuerTags`] from a dotted bucket path so that callers
/// can re-use [`bucket_factor_id`].
///
/// The path has `k+1` segments aligned with `hierarchy.levels[0..=k]`.
fn synth_tags_from_path(hierarchy: &CreditHierarchySpec, path: &str) -> IssuerTags {
    let segments: Vec<&str> = path.split('.').collect();
    let mut map = BTreeMap::new();
    for (i, seg) in segments.iter().enumerate() {
        if let Some(dim) = hierarchy.levels.get(i) {
            map.insert(dimension_key(dim).to_owned(), (*seg).to_owned());
        }
    }
    IssuerTags(map)
}

/// Apply shrinkage rule to an OLS β estimate.
fn apply_shrinkage(rule: &BetaShrinkage, beta_fit: f64) -> f64 {
    match rule {
        BetaShrinkage::None => beta_fit,
        BetaShrinkage::TowardOne { alpha } => (1.0 - *alpha) * beta_fit + *alpha * 1.0,
    }
}

/// Relative variance floor below which a regressor is treated as degenerate.
///
/// A hierarchy level that does not refine its parent partition produces a
/// bucket factor that is identically zero *in exact arithmetic* but carries
/// float noise around `1e-15` in practice. `finstack_quant_analytics::beta`
/// only rejects an *exactly* zero-variance regressor, so the OLS slope
/// `Cov(y, x) / Var(x)` divides residual-scale noise by squared rounding
/// noise and returns finite garbage on the order of `1e13`. Requiring
/// `Var(x) > tol · Var(y)` rejects that case while keeping any regressor
/// whose variation is economically meaningful relative to the response.
const DEGENERATE_REGRESSOR_REL_TOL: f64 = 1e-12;

/// Collect aligned `(x, y)` pairs where `y` is observed and `x` is dense.
fn gather_aligned_dense(y: &[Option<f64>], x: &[f64], xs: &mut Vec<f64>, ys: &mut Vec<f64>) {
    xs.clear();
    ys.clear();
    let cap = y.len().min(x.len());
    xs.reserve(cap);
    ys.reserve(cap);
    for (yi, xi) in y.iter().zip(x) {
        if let Some(v) = yi {
            xs.push(*xi);
            ys.push(*v);
        }
    }
}

/// Collect aligned `(x, y)` pairs where both series are observed.
fn gather_aligned_sparse(
    y: &[Option<f64>],
    x: &[Option<f64>],
    xs: &mut Vec<f64>,
    ys: &mut Vec<f64>,
) {
    xs.clear();
    ys.clear();
    let cap = y.len().min(x.len());
    xs.reserve(cap);
    ys.reserve(cap);
    for (yi, xi) in y.iter().zip(x) {
        if let (Some(yv), Some(xv)) = (yi, xi) {
            ys.push(*yv);
            xs.push(*xv);
        }
    }
}

/// Shared OLS core over aligned pairs: degenerate-regressor gate, then
/// delegate to `finstack_quant_analytics::beta`.
fn ols_slope_pairs(ys: &[f64], xs: &[f64]) -> Option<f64> {
    let n = xs.len();
    if n >= 2 {
        let nf = n as f64;
        let mean_x = xs.iter().sum::<f64>() / nf;
        let mean_y = ys.iter().sum::<f64>() / nf;
        let var_x = xs.iter().map(|v| (v - mean_x).powi(2)).sum::<f64>();
        let var_y = ys.iter().map(|v| (v - mean_y).powi(2)).sum::<f64>();
        // Shared (n − 1) denominator cancels in the ratio test.
        if var_x <= DEGENERATE_REGRESSOR_REL_TOL * var_y {
            return None;
        }
    }
    let result = finstack_quant_analytics::beta(ys, xs);
    if result.beta.is_nan() {
        None
    } else {
        Some(result.beta)
    }
}

/// R² and residual std on the through-origin peel residual `y − β x`.
///
/// The OLS slope itself is the with-intercept estimator from
/// `finstack_quant_analytics::beta`. Peel subtraction has no intercept, so
/// diagnostics use `y − β x` (not `y − α − β x`). `residual_std` is a
/// population std (`√(rss / n)`); calibration windows are required to be
/// long enough (`min_history` default 24) that the bias is negligible.
fn peel_fit_quality_from_pairs(ys: &[f64], xs: &[f64], beta: f64) -> Option<FitQuality> {
    let n = xs.len();
    if n < 3 {
        return None;
    }
    let nf = n as f64;
    let mean_y = ys.iter().sum::<f64>() / nf;
    let mut tss = 0.0;
    let mut rss = 0.0;
    for i in 0..n {
        let resid = ys[i] - beta * xs[i];
        rss += resid * resid;
        let dy = ys[i] - mean_y;
        tss += dy * dy;
    }
    let r_squared = if tss > 0.0 { 1.0 - rss / tss } else { 0.0 };
    let residual_std = (rss / nf).sqrt();
    Some(FitQuality {
        r_squared,
        residual_std,
        n_obs: n,
    })
}
