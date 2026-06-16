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

use core::hash::Hash;

use super::types::FrtbRiskClass;
use finstack_quant_core::HashMap;

/// One bucket's intra-bucket result: `(K_b, S_b_uncapped)`.
///
/// `K_b` is the non-negative square-root of the intra-bucket quadratic form
/// (capped at zero). `S_b` is the **uncapped** sum of weighted sensitivities
/// — the MAR21.6 alternative formula caps `S_b` to `[-K_b, K_b]` on-the-fly,
/// so we must preserve the raw sum here.
pub(super) type BucketResult = (f64, f64);

/// Intra-bucket aggregation for a single bucket with a uniform correlation.
///
/// `rho_kl = 1` when `k == l`, otherwise `intra_rho`. Callers must pre-scale
/// `intra_rho` for the active correlation scenario.
///
/// Computed in `O(n)` using the closed-form identity for a uniform
/// off-diagonal correlation:
///
/// ```text
/// Σ_ij ρ_ij · ws_i · ws_j
///   = Σ_i ws_i² + ρ · Σ_{i≠j} ws_i · ws_j
///   = Σ_i ws_i² + ρ · [(Σ_i ws_i)² - Σ_i ws_i²]
///   = (1 - ρ) · Σ_i ws_i² + ρ · (Σ_i ws_i)²
/// ```
pub(super) fn intra_bucket_uniform(entries: &[f64], intra_rho: f64) -> BucketResult {
    let mut sum_ws = 0.0;
    let mut sum_ws_sq = 0.0;
    for &ws in entries {
        sum_ws += ws;
        sum_ws_sq += ws * ws;
    }
    let k_squared = (1.0 - intra_rho) * sum_ws_sq + intra_rho * sum_ws * sum_ws;
    let k_b = k_squared.max(0.0).sqrt();
    (k_b, sum_ws)
}

/// Apply [`intra_bucket_uniform`] across every bucket in a map.
///
/// Iteration order is driven by the input `HashMap` and is therefore not
/// deterministic — inter-bucket aggregation does not depend on order, so
/// that is fine.
pub(super) fn intra_bucket_uniform_map<B>(
    by_bucket: &HashMap<B, Vec<f64>>,
    intra_rho: f64,
) -> Vec<BucketResult>
where
    B: Eq + Hash,
{
    by_bucket
        .values()
        .map(|entries| intra_bucket_uniform(entries, intra_rho))
        .collect()
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
    let compute = |cap: bool| -> f64 {
        let mut total = 0.0;
        for (i, &(k_i, s_i_raw)) in bucket_results.iter().enumerate() {
            total += k_i * k_i;
            let s_i = if cap {
                s_i_raw.clamp(-k_i, k_i)
            } else {
                s_i_raw
            };
            for (j, &(k_j, s_j_raw)) in bucket_results.iter().enumerate() {
                if i != j {
                    let s_j = if cap {
                        s_j_raw.clamp(-k_j, k_j)
                    } else {
                        s_j_raw
                    };
                    total += gamma * s_i * s_j;
                }
            }
        }
        total
    };

    let standard = compute(false);
    if standard >= 0.0 {
        standard.sqrt()
    } else {
        compute(true).max(0.0).sqrt()
    }
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
/// - BCBS FRTB Minimum Capital Requirements:
///   `docs/REFERENCES.md#bcbs-frtb-minimum-capital-requirements`
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
