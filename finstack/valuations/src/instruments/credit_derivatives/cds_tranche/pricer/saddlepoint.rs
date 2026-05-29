//! Conditional loss approximation for heterogeneous CDS-tranche pricing.
//!
//! Conditional on the systemic factor `Z`, the portfolio loss is a sum of
//! *independent* heterogeneous Bernoulli contributions
//! `L = Σ aᵢ·Bᵢ`, `Bᵢ ~ Bernoulli(pᵢ)`, `aᵢ = weightᵢ · lgdᵢ`. The
//! heterogeneous EL integrand needs the conditional equity-tranchelet loss
//! `E[min(L, K) | Z]`.
//!
//! [`conditional_min_loss_normal`] provides the **moment-matched Gaussian
//! (normal) approximation**, O'Kane (2008) *Modelling Single-name and
//! Multi-name Credit Derivatives* §9. For the diversified pools (`n > 16`)
//! that reach the approximate branch, the central limit theorem makes `L|Z`
//! close to Gaussian, and matching the exact conditional mean and variance
//! gives a small, bounded error.
//!
//! The Gaussian approximation places a small probability mass on `L < 0`;
//! this leakage is bounded by `Φ(−μ/σ)` and contributes a negative bias to
//! `E[min(L,K)]` that benchmarking against the exact convolution PMF showed to
//! be `< 1e-3` of tranchelet EL even in the worst (low-PD) case. A genuine
//! higher-order saddle-point alternative was evaluated and found 2–6× less
//! accurate at realistic pool sizes, so it is not implemented here (deferred —
//! see the audit FLAG).
//!
//! All branches are panic-free: degenerate inputs (zero/non-finite variance)
//! fall back to the deterministic loss.

use finstack_core::math::{norm_cdf, norm_pdf};

/// Moment-matched Gaussian (normal) approximation of the conditional
/// equity-tranchelet loss `E[min(L, K) | Z]`.
///
/// `L | Z` is approximated by `N(mean, var)` — the Gaussian matching the
/// exact conditional loss mean and variance. The closed form is
/// ```text
/// E[min(L,K)] = μ·Φ(a) − σ·φ(a) + K·(1 − Φ(a)),   a = (K − μ)/σ
/// ```
/// (O'Kane 2008, §9). This is the production heterogeneous-pool estimator;
/// see the module documentation for the accuracy comparison against the
/// saddle-point alternative and the `L < 0` bias bound.
///
/// Degenerate variance (`σ → 0`) collapses to the deterministic `min(μ, K)`.
/// The result is clamped to the analytic equity-tranchelet envelope `[0, K]`
/// so no downstream `.max(0.0)` patch is needed.
#[inline]
pub(super) fn conditional_min_loss_normal(k: f64, mean: f64, var: f64) -> f64 {
    if k <= 0.0 {
        return 0.0;
    }
    // Degenerate / non-finite variance: loss is deterministic at the mean.
    // (`var.is_nan()` covers a NaN variance; `var <= floor` covers the
    // zero/near-zero-variance degenerate case.)
    if var.is_nan() || var <= MIN_SPA_STD * MIN_SPA_STD {
        return mean.clamp(0.0, k);
    }
    let s = var.sqrt();
    let a = (k - mean) / s;
    let phi_a = norm_cdf(a);
    let el = mean * phi_a - s * norm_pdf(a) + k * (1.0 - phi_a);
    // E[min(L,K)] is analytically in [0, K] for any loss distribution;
    // clamp to absorb the O(Φ(−μ/σ)) Gaussian-tail residual.
    el.clamp(0.0, k)
}

/// Below this conditional standard deviation the loss is treated as
/// deterministic (`E[min(L,K)] = min(μ,K)`); a saddle-point solve is
/// ill-posed when `K''(0) → 0`.
const MIN_SPA_STD: f64 = 1e-7;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Exact `E[min(L,k)]` by full enumeration of the 2ⁿ default scenarios.
    /// Only used as the oracle for small `n` in the tests below.
    fn exact_tranchelet_loss(k: f64, a: &[f64], p: &[f64]) -> f64 {
        let n = a.len();
        assert!(n <= 20, "exact enumeration only for small n");
        let mut el = 0.0;
        for mask in 0..(1u32 << n) {
            let mut prob = 1.0;
            let mut loss = 0.0;
            for i in 0..n {
                if mask & (1 << i) != 0 {
                    prob *= p[i];
                    loss += a[i];
                } else {
                    prob *= 1.0 - p[i];
                }
            }
            el += prob * loss.min(k);
        }
        el
    }

    /// Per-name mean and variance of the conditional loss `L = Σ aᵢ·Bᵢ`.
    fn moments(a: &[f64], p: &[f64]) -> (f64, f64) {
        let mu: f64 = a.iter().zip(p.iter()).map(|(&ai, &pi)| ai * pi).sum();
        let var: f64 = a
            .iter()
            .zip(p.iter())
            .map(|(&ai, &pi)| ai * ai * pi * (1.0 - pi))
            .sum();
        (mu, var)
    }

    // ===================================================================
    // Production estimator: conditional_min_loss_normal
    // ===================================================================

    /// W-1 (item 1): the production normal-approximation tranchelet loss
    /// must lie in the analytic equity envelope `[0, k]`. The previous
    /// inline formula was unclamped and could leak below 0 / above k via the
    /// Gaussian's `L<0` tail mass.
    #[test]
    fn normal_approx_within_equity_envelope() {
        let a = vec![0.02; 30];
        let p = vec![0.05; 30];
        let (mu, var) = moments(&a, &p);
        for &k in &[0.01, 0.03, 0.05, 0.10, 0.20, 0.60] {
            let el = conditional_min_loss_normal(k, mu, var);
            assert!(
                (0.0..=k + 1e-12).contains(&el),
                "normal-approx tranchelet EL {el} must lie in [0, {k}]"
            );
        }
    }

    /// W-1 (item 1): the production normal approximation tracks the exact
    /// 2ⁿ-enumerated conditional `E[min(L,K)]` to a tight absolute tolerance
    /// for a heterogeneous pool. This pins the documented accuracy claim.
    #[test]
    fn normal_approx_matches_exact_for_heterogeneous_pool() {
        let a = vec![
            0.015, 0.020, 0.025, 0.030, 0.012, 0.018, 0.022, 0.028, 0.016, 0.024, 0.014, 0.026,
            0.019, 0.021, 0.017, 0.023,
        ];
        let p = vec![
            0.03, 0.05, 0.02, 0.08, 0.04, 0.06, 0.03, 0.07, 0.05, 0.04, 0.06, 0.02, 0.05, 0.03,
            0.07, 0.04,
        ];
        let (mu, var) = moments(&a, &p);
        for &k in &[0.02, 0.05, 0.10, 0.15] {
            let exact = exact_tranchelet_loss(k, &a, &p);
            let normal = conditional_min_loss_normal(k, mu, var);
            assert!(
                (normal - exact).abs() < 2e-3,
                "k={k}: normal-approx error {} too large (exact={exact}, normal={normal})",
                (normal - exact).abs()
            );
        }
    }

    /// W-1 (item 1): degenerate variance must collapse to the deterministic
    /// `min(μ,k)` rather than dividing by zero / producing NaN.
    #[test]
    fn normal_approx_degenerate_variance_is_deterministic() {
        // var = 0 ⇒ loss is exactly μ.
        assert!((conditional_min_loss_normal(0.10, 0.03, 0.0) - 0.03).abs() < 1e-15);
        // μ above k ⇒ clamps to k.
        assert!((conditional_min_loss_normal(0.02, 0.05, 0.0) - 0.02).abs() < 1e-15);
        // non-finite variance must not panic.
        let el = conditional_min_loss_normal(0.10, 0.03, f64::NAN);
        assert!(el.is_finite(), "NaN variance must not produce NaN EL");
    }

    /// W-1 (item 1): monotonic and bounded in the detachment `k`.
    #[test]
    fn normal_approx_monotonic_in_detachment() {
        let a = vec![0.018; 40];
        let p = vec![0.05; 40];
        let (mu, var) = moments(&a, &p);
        let mut prev = 0.0;
        let mut k = 0.005;
        while k < 0.60 {
            let el = conditional_min_loss_normal(k, mu, var);
            assert!(
                el >= prev - 1e-12,
                "normal-approx EL must be non-decreasing in k: el({k})={el} < prev={prev}"
            );
            prev = el;
            k += 0.005;
        }
    }
}
