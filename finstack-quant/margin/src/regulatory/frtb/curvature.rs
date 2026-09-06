//! FRTB curvature risk charge computation.
//!
//! Per MAR21.5 / MAR21.89 the curvature charge is built from signed
//! per-factor shifts `CVR+` and `CVR-` and aggregated with a sign-aware
//! `psi` indicator that prevents gains on one side from offsetting
//! losses on the other.
//!
//! # Algorithm
//!
//! For each risk factor `k`:
//!
//! * `CVR+_k` = loss under an upward shock of the factor, already
//!   adjusted for the delta-hedged component, sign convention "loss is
//!   positive".
//! * `CVR-_k` = loss under a downward shock, same convention.
//!
//! For each bucket `b`:
//!
//! ```text
//! K_b^+ = sqrt( max(0, sum_k max(CVR+_k, 0)^2
//!                        + sum_{k != l} rho_kl * CVR+_k * CVR+_l * psi(CVR+_k, CVR+_l)) )
//! K_b^- = sqrt( ... same with CVR-_k )
//! K_b   = max(K_b^+, K_b^-)
//! ```
//!
//! Whichever direction (+/-) gave the larger `K_b` also dictates the
//! bucket's signed sum `S_b = sum_k CVR_k`, capped by the bucket's own
//! aggregate: `S_b_capped = max(min(S_b, K_b), -K_b)`.
//!
//! Inter-bucket:
//!
//! ```text
//! Curvature = sqrt( max(0, sum_b K_b^2
//!                   + sum_{b != c} gamma_bc^2 * S_b * S_c * psi(S_b, S_c)) )
//! ```
//!
//! The `psi` function is `0` when *both* arguments are strictly
//! negative and `1` otherwise, so a pair of bucket-level gains
//! cannot reduce the charge from a separate pair of losses.

use super::aggregation::{inter_bucket_pairwise, inter_bucket_plus_undiversified};
use super::params::{self, fx, girr};
use super::types::{CorrelationScenario, FrtbRiskClass, FrtbSensitivities};
use finstack_quant_core::HashMap;
use std::collections::BTreeMap;

/// Compute the curvature risk charge for a single risk class.
///
/// # Arguments
///
/// * `risk_class` - FRTB risk class to calculate.
/// * `sensitivities` - Curvature inputs as `(CVR_up, CVR_down)` pairs using
///   the loss-positive convention documented in [`super::types::FrtbSensitivities`].
/// * `scenario` - Low, medium, or high correlation scenario applied to the
///   prescribed curvature correlations.
///
/// # Returns
///
/// The non-negative curvature risk charge for `risk_class` under `scenario`.
///
/// # References
///
/// - BCBS FRTB Minimum Capital Requirements: `docs/REFERENCES.md#bcbs-frtb-minimum-capital-requirements`
///
pub fn curvature_charge(
    risk_class: FrtbRiskClass,
    sensitivities: &FrtbSensitivities,
    scenario: CorrelationScenario,
) -> f64 {
    match risk_class {
        FrtbRiskClass::Girr => girr_curvature(sensitivities, scenario),
        FrtbRiskClass::CsrNonSec => csr_nonsec_curvature(sensitivities, scenario),
        FrtbRiskClass::CsrSecCtp => csr_sec_ctp_curvature(sensitivities, scenario),
        FrtbRiskClass::CsrSecNonCtp => csr_sec_nonctp_curvature(sensitivities, scenario),
        FrtbRiskClass::Equity => equity_curvature(sensitivities, scenario),
        FrtbRiskClass::Commodity => commodity_curvature(sensitivities, scenario),
        FrtbRiskClass::Fx => fx_curvature(sensitivities, scenario),
    }
}

// Per-risk-class drivers

fn girr_curvature(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.girr_curvature.is_empty() {
        return 0.0;
    }

    // GIRR curvature: one risk factor per currency, so each bucket holds
    // exactly one `(up, down)` pair. `intra_rho` is irrelevant for a
    // single-factor bucket — the diagonal K_b is max(CVR_plus, 0) or
    // max(CVR_minus, 0), and the direction picks the larger.
    let bucket_results: Vec<(f64, f64)> = sens
        .girr_curvature
        .values()
        .map(|&(up, down)| bucket_k_and_s(&[(up, down)], 0.0))
        .collect();

    let gamma = scenario.scale_correlation(girr::GIRR_INTER_BUCKET_CORRELATION.powi(2));
    curvature_inter_bucket(&bucket_results, gamma)
}

fn csr_nonsec_curvature(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_curvature_bucketed(
        &sens.csr_nonsec_curvature,
        FrtbRiskClass::CsrNonSec,
        scenario,
    )
}

fn csr_sec_ctp_curvature(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_curvature_bucketed(
        &sens.csr_sec_ctp_curvature,
        FrtbRiskClass::CsrSecCtp,
        scenario,
    )
}

fn csr_sec_nonctp_curvature(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_curvature_bucketed(
        &sens.csr_sec_nonctp_curvature,
        FrtbRiskClass::CsrSecNonCtp,
        scenario,
    )
}

fn equity_curvature(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_curvature_bucketed(&sens.equity_curvature, FrtbRiskClass::Equity, scenario)
}

fn commodity_curvature(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    generic_curvature_bucketed(
        &sens.commodity_curvature,
        FrtbRiskClass::Commodity,
        scenario,
    )
}

fn fx_curvature(sens: &FrtbSensitivities, scenario: CorrelationScenario) -> f64 {
    if sens.fx_curvature.is_empty() {
        return 0.0;
    }
    // FX curvature factors live in a single (implicit) bucket; inter-pair
    // correlation drives the intra-bucket aggregation.
    let pairs: Vec<(f64, f64)> = sens
        .fx_curvature
        .values()
        .map(|&(up, down)| (up, down))
        .collect();
    let rho = scenario.scale_correlation(fx::FX_INTER_PAIR_CORRELATION.powi(2));
    let (k_b, s_b) = bucket_k_and_s(&pairs, rho);
    curvature_inter_bucket(&[(k_b, s_b)], rho)
}

// Generic bucketed curvature

fn generic_curvature_bucketed(
    curvatures: &HashMap<(String, u8), (f64, f64)>,
    class: FrtbRiskClass,
    scenario: CorrelationScenario,
) -> f64 {
    let mut buckets = BTreeMap::<u8, Vec<(f64, f64)>>::new();
    for ((_, bucket), pair) in curvatures {
        buckets.entry(*bucket).or_default().push(*pair);
    }
    let ids: Vec<_> = buckets.keys().copied().collect();
    let results: Vec<_> = buckets
        .iter()
        .map(|(b, pairs)| {
            if params::other_bucket(class, *b) {
                let up: f64 = pairs.iter().map(|p| p.0.max(0.0)).sum();
                let down: f64 = pairs.iter().map(|p| p.1.max(0.0)).sum();
                return (up.max(down), up.max(down));
            }
            bucket_k_and_s(
                pairs,
                scenario.scale_correlation(params::name_correlation(class, *b).powi(2)),
            )
        })
        .collect();
    if class == FrtbRiskClass::CsrSecNonCtp {
        return inter_bucket_plus_undiversified(&ids, &results, |b| params::other_bucket(class, b));
    }
    inter_bucket_pairwise(&results, |i, j| {
        scenario.scale_correlation(params::bucket_correlation(class, ids[i], ids[j]).powi(2))
            * psi(results[i].1, results[j].1)
    })
}

// MAR21.5 helpers

/// Psi indicator per MAR21.5.
///
/// Returns `0` when *both* arguments are strictly negative; returns `1`
/// otherwise. Used to prevent a pair of net-negative CVRs from
/// contributing negative off-diagonal mass that would reduce the charge.
#[inline]
fn psi(x: f64, y: f64) -> f64 {
    if x < 0.0 && y < 0.0 {
        0.0
    } else {
        1.0
    }
}

/// Compute `(K_b, S_b)` for a curvature bucket.
///
/// `pairs` is the list of `(CVR+, CVR-)` per risk factor. Computes
/// `K_b^+` and `K_b^-` separately, picks the larger, and returns the
/// corresponding direction's uncapped `sum CVR`. On a tie, select the
/// direction with the larger sum, as required by MAR21.5.
fn bucket_k_and_s(pairs: &[(f64, f64)], intra_rho: f64) -> (f64, f64) {
    if pairs.is_empty() {
        return (0.0, 0.0);
    }

    let (k_plus, s_plus_raw) =
        one_side_k(&pairs.iter().map(|p| p.0).collect::<Vec<_>>(), intra_rho);
    let (k_minus, s_minus_raw) =
        one_side_k(&pairs.iter().map(|p| p.1).collect::<Vec<_>>(), intra_rho);

    if k_plus
        .total_cmp(&k_minus)
        .then_with(|| s_plus_raw.total_cmp(&s_minus_raw))
        .is_gt()
    {
        (k_plus, s_plus_raw)
    } else {
        (k_minus, s_minus_raw)
    }
}

/// One-sided `K` aggregate:
///
/// ```text
/// K = sqrt( max(0, sum_k max(cvr_k, 0)^2
/// + sum_{k != l} rho * cvr_k * cvr_l * psi(cvr_k, cvr_l)) )
/// ```
///
/// Also returns the raw (uncapped) sum of cvr_k for this side.
fn one_side_k(cvrs: &[f64], rho: f64) -> (f64, f64) {
    let per_factor: Vec<(f64, f64)> = cvrs.iter().map(|&c| (c.max(0.0), c)).collect();
    let k = inter_bucket_pairwise(&per_factor, |i, j| rho * psi(cvrs[i], cvrs[j]));
    let s: f64 = cvrs.iter().sum();
    (k, s)
}

/// Inter-bucket curvature aggregation:
///
/// ```text
/// Curvature = sqrt( max(0, sum_b K_b^2
/// + sum_{b != c} gamma^2 * S_b * S_c * psi(S_b, S_c)) )
/// ```
fn curvature_inter_bucket(bucket_results: &[(f64, f64)], gamma: f64) -> f64 {
    inter_bucket_pairwise(bucket_results, |i, j| {
        gamma * psi(bucket_results[i].1, bucket_results[j].1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psi_zero_only_when_both_negative() {
        assert_eq!(psi(-1.0, -2.0), 0.0);
        assert_eq!(psi(-1.0, 2.0), 1.0);
        assert_eq!(psi(1.0, -2.0), 1.0);
        assert_eq!(psi(1.0, 2.0), 1.0);
        // Boundary: one value exactly zero -> psi returns 1 (not both strictly negative)
        assert_eq!(psi(0.0, -2.0), 1.0);
        assert_eq!(psi(-1.0, 0.0), 1.0);
    }

    #[test]
    fn single_factor_bucket_picks_worse_side() {
        // CVR+ = 10 loss, CVR- = 3 loss -> K_b = 10 from + side, S_b = 10.
        let (k, s) = bucket_k_and_s(&[(10.0, 3.0)], 0.0);
        assert!((k - 10.0).abs() < 1e-12);
        assert!((s - 10.0).abs() < 1e-12);

        // Reverse: CVR- is larger.
        let (k, s) = bucket_k_and_s(&[(2.0, 15.0)], 0.0);
        assert!((k - 15.0).abs() < 1e-12);
        assert!((s - 15.0).abs() < 1e-12);
    }

    #[test]
    fn s_b_preserves_raw_sum() {
        let pairs = vec![(10.0, 0.0), (10.0, 0.0)];
        let (k, s) = bucket_k_and_s(&pairs, 0.5);
        let k_expected = 300_f64.sqrt();
        assert!((k - k_expected).abs() < 1e-10);
        assert_eq!(s, 20.0);
    }

    #[test]
    fn psi_blocks_negative_off_diagonal() {
        // Two factors with CVR = -5 each. Under the plain quadratic the
        // off-diagonal would give 2*rho*25 = 25 (for rho=0.5) which
        // would increase the charge. With psi=0 for both-negative
        // pairs, the off-diagonal drops out and the diagonal
        // max(CVR, 0)^2 = 0 for each, so K = 0.
        let (k, _s) = bucket_k_and_s(&[(-5.0, 0.0), (-5.0, 0.0)], 0.5);
        assert!(
            k.abs() < 1e-12,
            "both-negative CVR+ should contribute 0 to K_b, got {k}"
        );
    }

    #[test]
    fn bucket_returns_sign_of_winning_direction() {
        // Two factors; CVR+ = (8, 6) all positive loss; CVR- = (-1, -2)
        // (gains on down shock). K+ dominates, S_b must be the positive
        // sum (14), not the down-side sum (-3). This is what the
        // inter-bucket psi-blocking relies on.
        let (k, s) = bucket_k_and_s(&[(8.0, -1.0), (6.0, -2.0)], 0.5);
        let k_plus_expected = (8.0f64.powi(2) + 6.0_f64.powi(2) + 2.0 * 0.5 * 8.0 * 6.0).sqrt();
        assert!(
            (k - k_plus_expected).abs() < 1e-10,
            "K_b should equal K+ = {k_plus_expected}, got {k}"
        );
        assert_eq!(s, 14.0);
    }

    #[test]
    fn inter_bucket_blocks_both_negative_offsets() {
        // Two buckets with negative S. Under a plain quadratic form they
        // would contribute +gamma^2 * |S1|*|S2| to the sum; under
        // MAR21.5 psi=0 for both-negative, so no contribution.
        let buckets = vec![(5.0, -3.0), (5.0, -4.0)];
        let total = curvature_inter_bucket(&buckets, 0.5);
        // Expected: sqrt(K1^2 + K2^2) because the off-diagonal vanishes.
        let expected = (25.0 + 25.0_f64).sqrt();
        assert!(
            (total - expected).abs() < 1e-10,
            "inter-bucket total = {total}, expected {expected}"
        );
    }
}
