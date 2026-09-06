//! Shared FRTB SBA aggregation primitives.
//!
//! The FRTB Sensitivity-Based Approach repeatedly applies two aggregation
//! shapes — intra-bucket (combine weighted sensitivities inside one bucket
//! into `(K_b, S_b)`) and inter-bucket (combine per-bucket `(K_b, S_b)` into
//! the risk-class charge with the MAR21.4-21.6 alternative fallback). These
//! helpers encapsulate that logic so delta, vega, and curvature do not drift.
//!
//! Cross-risk-class aggregation ([`aggregate_sba`]) is also simple addition —
//! the SBA, unlike SIMM, has no cross-risk-class correlation matrix.

use super::types::FrtbRiskClass;
use finstack_quant_core::HashMap;

/// One bucket's intra-bucket result: `(K_b, S_b_uncapped)`.
///
/// `K_b` is the non-negative square-root of the intra-bucket quadratic form
/// (capped at zero). `S_b` is the **uncapped** sum of weighted sensitivities
/// — the MAR21.6 alternative formula caps `S_b` to `[-K_b, K_b]` on-the-fly,
/// so we must preserve the raw sum here.
pub(crate) type BucketResult = (f64, f64);

/// Correlated quadratic-form norm `sqrt(max(0, Σ_i Σ_j ρ(i, j) · ws_i · ws_j))`.
///
/// The diagonal is always taken as `1`; `rho` is only consulted for `i != j`.
/// The reduction runs in index order (`i` outer, `j` inner, diagonal
/// included in sequence), so callers that need bit-reproducible numbers
/// must pass `ws` in a canonical order.
///
/// This is the single "Σ ρ_ij ws_i ws_j" shape shared by every SIMM risk
/// class and by the FRTB intra-bucket aggregations.
///
/// # Arguments
///
/// * `ws` - Weighted sensitivities (already risk-weighted and, where
///   applicable, concentration-scaled) in the caller's canonical order.
/// * `rho` - Correlation between positions `i` and `j` (`i != j`), already
///   scaled for the active correlation scenario.
pub(crate) fn correlated_norm(ws: &[f64], rho: impl Fn(usize, usize) -> f64) -> f64 {
    let mut sum = 0.0;
    for (i, &ws_i) in ws.iter().enumerate() {
        for (j, &ws_j) in ws.iter().enumerate() {
            let r = if i == j { 1.0 } else { rho(i, j) };
            sum += r * ws_i * ws_j;
        }
    }
    sum.max(0.0).sqrt()
}

/// Intra-bucket aggregation with a pairwise correlation over factor labels.
///
/// Returns `(K_b, S_b_uncapped)` where `K_b` is [`correlated_norm`] of the
/// weighted sensitivities and `S_b` is their raw sum.
///
/// # Arguments
///
/// * `entries` - `(weighted_sensitivity, factor)` pairs for one bucket.
/// * `rho` - Correlation between two distinct factors, already scaled for
///   the active correlation scenario.
pub(super) fn intra_bucket_pairwise<T>(
    entries: &[(f64, T)],
    rho: impl Fn(&T, &T) -> f64,
) -> BucketResult {
    let ws: Vec<f64> = entries.iter().map(|(ws, _)| *ws).collect();
    let k_b = correlated_norm(&ws, |i, j| rho(&entries[i].1, &entries[j].1));
    let s_b: f64 = ws.iter().sum();
    (k_b, s_b)
}

/// Inter-bucket quadratic form `sqrt(max(0, Σ_b K_b² + Σ_{b≠c} γ(b, c) · S_b · S_c))`.
///
/// Shared by the FRTB MAR21.4 standard form (uniform `γ`), the FRTB
/// curvature form (`γ²·ψ`) and the SIMM credit-qualifying sector
/// aggregation (per-pair `γ_bc`). The reduction runs in index order.
///
/// # Arguments
///
/// * `bucket_results` - `(K_b, S_b)` per bucket in the caller's canonical order.
/// * `gamma` - Inter-bucket correlation (or any pairwise weight) for buckets
///   `b != c`.
pub(crate) fn inter_bucket_pairwise(
    bucket_results: &[BucketResult],
    gamma: impl Fn(usize, usize) -> f64,
) -> f64 {
    inter_bucket_quadratic(bucket_results, gamma)
        .max(0.0)
        .sqrt()
}

/// Signed (un-floored) inter-bucket quadratic form `Σ_b K_b² + Σ_{b≠c} γ(b, c) · S_b · S_c`.
fn inter_bucket_quadratic(
    bucket_results: &[BucketResult],
    gamma: impl Fn(usize, usize) -> f64,
) -> f64 {
    let mut total = 0.0;
    for (i, &(k_i, s_i)) in bucket_results.iter().enumerate() {
        total += k_i * k_i;
        for (j, &(_, s_j)) in bucket_results.iter().enumerate() {
            if i != j {
                total += gamma(i, j) * s_i * s_j;
            }
        }
    }
    total
}

/// Inter-bucket aggregation per MAR21.4-21.6.
///
/// Tries the standard quadratic form first with uncapped
/// `S_b = sum_k WS_k`:
///
/// ```text
/// Delta² = sum_b K_b² + sum_{b != c} gamma * S_b * S_c
/// ```
///
/// If `Delta²` is non-negative, the risk-class charge is `sqrt(Delta²)`.
/// Otherwise the alternative formula (MAR21.6) fires, replacing every `S_b`
/// with `S_b_capped = max(-K_b, min(S_b, K_b))`. The capped form is
/// provably non-negative.
///
/// `gamma` must already be scaled for the active correlation scenario.
pub(super) fn inter_bucket(bucket_results: &[BucketResult], gamma: f64) -> f64 {
    inter_bucket_with(bucket_results, |_, _| gamma)
}

/// Standard and alternative aggregation for bucket-specific correlations.
pub(super) fn inter_bucket_with(
    bucket_results: &[BucketResult],
    gamma: impl Fn(usize, usize) -> f64,
) -> f64 {
    let standard = inter_bucket_quadratic(bucket_results, &gamma);
    if standard >= 0.0 {
        standard.sqrt()
    } else {
        let capped: Vec<BucketResult> = bucket_results
            .iter()
            .map(|&(k, s)| (k, s.clamp(-k, k)))
            .collect();
        inter_bucket_pairwise(&capped, gamma)
    }
}

/// MAR21.71: add undiversified buckets as a sum of `K_b` after a
/// zero-correlation quadratic over the remaining buckets.
pub(super) fn inter_bucket_plus_undiversified(
    ids: &[u8],
    results: &[BucketResult],
    undiversified: impl Fn(u8) -> bool,
) -> f64 {
    let extra: f64 = ids
        .iter()
        .zip(results)
        .filter(|(b, _)| undiversified(**b))
        .map(|(_, r)| r.0)
        .sum();
    extra
        + ids
            .iter()
            .zip(results)
            .filter(|(b, _)| !undiversified(**b))
            .map(|(_, r)| r.0 * r.0)
            .sum::<f64>()
            .sqrt()
}

/// Aggregate delta+vega+curvature across risk classes for one correlation scenario.
///
/// `SBA_agg = sum_rc [ Delta_rc + Vega_rc + Curvature_rc ]`
///
/// The final capital charge picks the maximum across scenarios:
///   `Capital = max(SBA_agg_low, SBA_agg_medium, SBA_agg_high) + DRC + RRAO`
///
/// # Arguments
///
/// * `delta_charges` - Per-risk-class delta charges for one correlation scenario.
/// * `vega_charges` - Per-risk-class vega charges for the same scenario.
/// * `curvature_charges` - Per-risk-class curvature charges for the same scenario.
///
/// # Returns
///
/// The total SBA charge for one scenario before adding DRC/RRAO and before
/// taking the maximum across correlation scenarios.
///
/// # References
///
/// - BCBS FRTB Minimum Capital Requirements: `docs/REFERENCES.md#bcbs-frtb-minimum-capital-requirements`
///
pub fn aggregate_sba(
    delta_charges: &HashMap<FrtbRiskClass, f64>,
    vega_charges: &HashMap<FrtbRiskClass, f64>,
    curvature_charges: &HashMap<FrtbRiskClass, f64>,
) -> f64 {
    let sum_delta: f64 = delta_charges.values().sum();
    let sum_vega: f64 = vega_charges.values().sum();
    let sum_curvature: f64 = curvature_charges.values().sum();
    sum_delta + sum_vega + sum_curvature
}
