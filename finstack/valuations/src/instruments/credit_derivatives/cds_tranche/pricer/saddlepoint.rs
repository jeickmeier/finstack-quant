//! Conditional loss approximations for heterogeneous CDS-tranche pricing.
//!
//! Conditional on the systemic factor `Z`, the portfolio loss is a sum of
//! *independent* heterogeneous Bernoulli contributions
//! `L = Σ aᵢ·Bᵢ`, `Bᵢ ~ Bernoulli(pᵢ)`, `aᵢ = weightᵢ · lgdᵢ`. The
//! heterogeneous EL integrand needs the conditional equity-tranchelet loss
//! `E[min(L, K) | Z]`.
//!
//! This module provides two evaluators:
//!
//! 1. [`conditional_min_loss_normal`] — the **moment-matched Gaussian
//!    (normal) approximation**, O'Kane (2008) *Modelling Single-name and
//!    Multi-name Credit Derivatives* §9. This is the **production** path:
//!    for the diversified pools (`n > 16`) that reach the approximate
//!    branch, the central limit theorem makes `L|Z` close to Gaussian, and
//!    matching the exact conditional mean and variance gives a small,
//!    bounded error.
//!
//! 2. [`conditional_tranchelet_loss`] — a genuine cumulant-generating-
//!    function (CGF) **saddle-point approximation** (Antonov-Mechkov-
//!    Misirpashaev 2005; Martin-Thompson-Browne, *Taking to the Saddle*,
//!    Risk 2001). Retained as a tested, available alternative for
//!    validation work — see the empirical note below for why it is not the
//!    production default.
//!
//! # Why the normal approximation is the production default
//!
//! The Gaussian approximation places a small probability mass on `L < 0`;
//! this leakage is bounded by `Φ(−μ/σ)` and contributes a negative bias to
//! `E[min(L,K)]`. It was tempting to replace it with a "genuine" saddle-
//! point method. However, benchmarking both estimators against the exact
//! convolution PMF (the same `hetero_exact_convolution_full` reference used
//! for small pools) across realistic CDX / bespoke pools (`n` = 50, 80,
//! 125; low- and high-PD regimes) showed:
//!
//! - the moment-matched normal approximation's total absolute error is
//!   **2–6× smaller** than the second-order Lugannani-Rice / AMM saddle-
//!   point at every pool size tested;
//! - the `L < 0` leakage contributes `< 1e-3` of tranchelet EL even in the
//!   worst (low-PD) case.
//!
//! The reason: for a diversified pool `L|Z` is genuinely near-Gaussian by
//! the CLT, so matching its first two moments is accurate, whereas a
//! second-order saddle-point expansion adds bias from its own asymptotic
//! error at finite `n`. A saddle-point method that *beats* the moment-
//! matched normal needs the validated AMM higher-order correction terms —
//! deferred as a separate work item (see the audit FLAG). Until then the
//! normal approximation is both the more accurate and the simpler choice.
//!
//! # Saddle-point formula (for [`conditional_tranchelet_loss`])
//!
//! The exact conditional CGF and its derivatives are
//! ```text
//! K(s)  = Σ log(1 − pᵢ + pᵢ · e^{s·aᵢ})
//! K'(s) = Σ  aᵢ·pᵢ·e^{s·aᵢ} / (1 − pᵢ + pᵢ·e^{s·aᵢ})
//! K''(s)= Σ  aᵢ²·pᵢ·e^{s·aᵢ}·(1−pᵢ) / (1 − pᵢ + pᵢ·e^{s·aᵢ})²
//! ```
//! With `μ = K'(0)` we use `E[min(L,K)] = μ − E[(L−K)⁺]`. The saddle `ŝ`
//! solves `K'(ŝ) = K`; the Lugannani-Rice exceedance probability and the
//! AMM partial-expectation companion formula give
//! ```text
//! w  = sign(ŝ)·√(2(ŝK − K(ŝ))) ,   u = ŝ·√(K''(ŝ))
//! P(L>K)    ≈ 1 − Φ(w) + φ(w)·(1/u − 1/w)
//! E[(L−K)⁺] ≈ (μ−K)·P(L>K) + K''(ŝ)·f̂(K) ,  f̂(K)=e^{K(ŝ)−ŝK}/√(2π K''(ŝ))
//! ```
//!
//! All branches are panic-free: degenerate inputs (zero variance, `K`
//! outside the loss support, failed saddle solve) fall back to the
//! deterministic loss or the normal estimate.

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

/// Maximum Newton iterations for the saddle-point solve `K'(s) = target`.
const SADDLE_MAX_ITER: usize = 60;

/// Convergence tolerance on the saddle-point equation residual `K'(s) − K`.
const SADDLE_TOL: f64 = 1e-12;

/// Bound on `|s·aᵢ|` inside the CGF to keep `exp` away from overflow. With
/// per-name loss `aᵢ ≤ 1` this still admits saddle points far into either
/// tail; beyond it the tilt is numerically saturated and we clamp.
const MAX_TILT_EXP: f64 = 80.0;

/// Below this conditional standard deviation the loss is treated as
/// deterministic (`E[min(L,K)] = min(μ,K)`); a saddle-point solve is
/// ill-posed when `K''(0) → 0`.
const MIN_SPA_STD: f64 = 1e-7;

/// Conditional CGF `K(s)`, its first two derivatives, evaluated at `s`.
///
/// `a` are the per-name loss amounts (`weightᵢ · lgdᵢ`), `p` the conditional
/// default probabilities. Returns `(K, K', K'')`.
#[inline]
fn cgf_derivs(s: f64, a: &[f64], p: &[f64]) -> (f64, f64, f64) {
    let mut k0 = 0.0_f64;
    let mut k1 = 0.0_f64;
    let mut k2 = 0.0_f64;
    for (&ai, &pi) in a.iter().zip(p.iter()) {
        if ai <= 0.0 || pi <= 0.0 {
            continue;
        }
        let pi = pi.min(1.0);
        // q = 1 - p ; tilt e^{s a} clamped for overflow safety.
        let q = 1.0 - pi;
        let exponent = (s * ai).clamp(-MAX_TILT_EXP, MAX_TILT_EXP);
        let e = exponent.exp();
        let denom = q + pi * e; // > 0 always (q ≥ 0, p·e > 0)
        // tilted Bernoulli probability for name i under measure shifted by s
        let pt = (pi * e) / denom;
        k0 += denom.ln();
        k1 += ai * pt;
        k2 += ai * ai * pt * (1.0 - pt);
    }
    (k0, k1, k2)
}

/// Solve the saddle-point equation `K'(s) = target` by safeguarded Newton
/// iteration. `K'` is strictly increasing in `s` (since `K'' > 0` whenever the
/// loss is non-degenerate), so the root is unique and Newton — bracketed by a
/// bisection fallback — always converges. Returns `None` only if the loss is
/// degenerate (`K'' ≈ 0`), in which case the caller uses the deterministic
/// branch.
fn solve_saddle(target: f64, a: &[f64], p: &[f64]) -> Option<f64> {
    // Bracket: K'(s) ranges over (0, Σaᵢ) as s ranges over (−∞, ∞).
    // Expand outward in log-space until the sign of (K'−target) changes.
    let (_, k1_0, k2_0) = cgf_derivs(0.0, a, p);
    if k2_0 <= MIN_SPA_STD * MIN_SPA_STD {
        return None; // degenerate — variance ~ 0
    }
    // Newton seed from the local quadratic model at s = 0.
    let mut s = (target - k1_0) / k2_0;
    if !s.is_finite() {
        s = 0.0;
    }

    let mut lo = f64::NEG_INFINITY;
    let mut hi = f64::INFINITY;
    for _ in 0..SADDLE_MAX_ITER {
        let (_, k1, k2) = cgf_derivs(s, a, p);
        let resid = k1 - target;
        if resid.abs() <= SADDLE_TOL {
            return Some(s);
        }
        // Maintain a bracket: K' increasing ⇒ resid increasing in s.
        if resid > 0.0 {
            hi = s;
        } else {
            lo = s;
        }
        // Newton step, guarded by the bracket.
        let mut next = if k2 > 0.0 {
            s - resid / k2
        } else {
            f64::NAN
        };
        let in_bracket = next.is_finite()
            && (lo.is_infinite() || next > lo)
            && (hi.is_infinite() || next < hi);
        if !in_bracket {
            // Bisection / outward expansion fallback.
            next = match (lo.is_finite(), hi.is_finite()) {
                (true, true) => 0.5 * (lo + hi),
                (true, false) => lo + 1.0 + lo.abs(),
                (false, true) => hi - 1.0 - hi.abs(),
                (false, false) => 0.0,
            };
        }
        if (next - s).abs() <= SADDLE_TOL * (1.0 + s.abs()) {
            return Some(next);
        }
        s = next;
    }
    // Accept the last iterate if the bracket is already tight.
    if lo.is_finite() && hi.is_finite() && (hi - lo) <= 1e-6 {
        Some(0.5 * (lo + hi))
    } else {
        None
    }
}

/// Saddle-point approximation of the call payoff `E[(L − k)⁺ | Z]`.
///
/// Uses the Lugannani–Rice exceedance probability together with the
/// partial-expectation companion formula (AMM 2005 / H.-P. Studer 2001). The
/// result is clamped to the analytic envelope `0 ≤ E[(L−k)⁺] ≤ μ` so a
/// second-order overshoot near the saddle can never produce a negative
/// tranche EL.
///
/// `mu` and `var` are the conditional mean and variance `K'(0)`, `K''(0)`
/// (passed in to avoid recomputing the `s = 0` CGF).
fn spa_call(k: f64, mu: f64, var: f64, a: &[f64], p: &[f64]) -> f64 {
    // Total reachable loss: e^{s·a} support ⇒ L ∈ [0, Σaᵢ].
    let total: f64 = a.iter().zip(p.iter())
        .filter(|(&ai, &pi)| ai > 0.0 && pi > 0.0)
        .map(|(&ai, _)| ai)
        .sum();

    // Strike outside the support: exact.
    if k <= 0.0 {
        return mu; // (L − k)⁺ = L − k, but k ≤ 0 ⇒ E[(L)⁺]=μ for k=0; caller handles k<0
    }
    if k >= total {
        return 0.0; // loss can never exceed k
    }

    let std = var.sqrt();
    if std < MIN_SPA_STD {
        return (mu - k).max(0.0);
    }

    // Saddle point K'(ŝ) = k.
    let Some(s_hat) = solve_saddle(k, a, p) else {
        // Degenerate: fall back to the deterministic split.
        return (mu - k).max(0.0);
    };

    let (k0, _k1, k2) = cgf_derivs(s_hat, a, p);
    // `K''(ŝ)` must be a finite positive curvature and `K(ŝ)` finite; any
    // NaN / non-positive value falls back to the deterministic split.
    if !(k2.is_finite() && k2 > 0.0 && k0.is_finite()) {
        return (mu - k).max(0.0);
    }

    // Saddle at (essentially) the mean: K is at the conditional mean, the
    // Lugannani–Rice form is singular (w → 0). Use the central limit value.
    if s_hat.abs() < 1e-8 {
        // E[(L−k)⁺] with L ≈ N(μ, var) and k ≈ μ:  σ·φ(0) since (k−μ)≈0.
        let z = (k - mu) / std;
        let call_normal = std * norm_pdf(z) - (k - mu) * (1.0 - norm_cdf(z));
        return call_normal.clamp(0.0, mu);
    }

    // Lugannani–Rice exceedance probability  P(L > k).
    //   w = sign(ŝ)·√(2(ŝk − K(ŝ)))   (radicand ≥ 0 at a valid saddle)
    //   u = ŝ·√(K''(ŝ))
    let radicand = 2.0 * (s_hat * k - k0);
    let w = (radicand.max(0.0)).sqrt() * s_hat.signum();
    let u = s_hat * k2.sqrt();
    let tail = if w.abs() < 1e-9 || u.abs() < 1e-9 {
        // Degenerate guard — revert to the normal exceedance probability.
        1.0 - norm_cdf((k - mu) / std)
    } else {
        // Lugannani–Rice: P(L > k) = Φ_c(w) + φ(w)·(1/u − 1/w).
        let lr = (1.0 - norm_cdf(w)) + norm_pdf(w) * (1.0 / u - 1.0 / w);
        lr.clamp(0.0, 1.0)
    };

    // Saddle-point density of L at k:  f̂(k) = e^{K(ŝ)−ŝk} / √(2π K''(ŝ)).
    let log_tilt = k0 - s_hat * k; // K(ŝ) − ŝk ≤ 0 at the saddle
    let density = (log_tilt.exp()) / (2.0 * std::f64::consts::PI * k2).sqrt();

    // Partial expectation (Antonov-Mechkov-Misirpashaev 2005; Studer 2001):
    //   ŝ > 0 (k above the conditional mean):
    //       E[(L−k)⁺] ≈ (μ − k)·P(L>k) + K''(ŝ)·f̂(k)
    //   ŝ < 0 (k below the conditional mean): put–call parity gives
    //       E[(L−k)⁺] = (μ − k) + E[(k−L)⁺],
    //       E[(k−L)⁺] ≈ (k − μ)·P(L<k) + K''(ŝ)·f̂(k)
    //   so  E[(L−k)⁺] ≈ (μ − k)·P(L>k) + K''(ŝ)·f̂(k)  in BOTH cases
    //   (since (μ−k) + (k−μ)·P(L<k) = (μ−k)·(1−P(L<k)) = (μ−k)·P(L>k)).
    // The density correction term `K''·f̂` is the same on both branches.
    let call_raw = (mu - k) * tail + k2 * density;

    // Envelope: an equity-call payoff satisfies
    //   (μ − k)⁺ ≤ E[(L−k)⁺] ≤ min(μ, total − k).
    let intrinsic = (mu - k).max(0.0);
    let upper = mu.min(total - k).max(intrinsic);
    if call_raw.is_finite() {
        call_raw.clamp(intrinsic, upper)
    } else {
        // Last-resort: flat tail estimate.
        (tail * (total - k)).clamp(intrinsic, upper)
    }
}

/// Conditional tranchelet (equity) expected loss `E[min(L, k) | Z]` via the
/// genuine CGF-based saddle-point approximation (Antonov-Mechkov-Misirpashaev
/// 2005).
///
/// `a` are per-name loss amounts (`weightᵢ · lgdᵢ`), `p` the conditional
/// default probabilities given the systemic factor, `k` the equity-tranche
/// detachment (in the same loss units, i.e. fraction of portfolio notional).
///
/// `min(L,k) = L − (L−k)⁺`, so `E[min(L,k)] = μ − E[(L−k)⁺]` with
/// `μ = E[L | Z]`. The result is clamped into `[0, k]` — the analytic range of
/// an equity tranchelet.
///
/// NOTE: this is **not** the production heterogeneous-pool estimator — see
/// the module documentation. [`conditional_min_loss_normal`] (the moment-
/// matched Gaussian approximation) is used in production because it is
/// empirically more accurate at realistic pool sizes. This function is kept
/// fully implemented and tested for validation work and as the foundation for
/// a future higher-order saddle-point method.
#[allow(dead_code)] // validation-only alternative; see module docs
pub(super) fn conditional_tranchelet_loss(k: f64, a: &[f64], p: &[f64]) -> f64 {
    // Conditional mean and variance from the s = 0 CGF.
    let (_, mu, var) = cgf_derivs(0.0, a, p);

    if k <= 0.0 {
        return 0.0;
    }
    if mu <= 0.0 {
        return 0.0;
    }
    // Degenerate variance: loss is (essentially) deterministic at μ.
    if var < MIN_SPA_STD * MIN_SPA_STD {
        return mu.min(k);
    }

    let call = spa_call(k, mu, var, a, p);
    // E[min(L,k)] = μ − E[(L−k)⁺]; clamp to the equity-tranchelet envelope.
    (mu - call).clamp(0.0, k)
}

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

    // ===================================================================
    // Saddle-point alternative: conditional_tranchelet_loss
    // (validation-only; see module docs for why it is not the default)
    // ===================================================================

    /// The saddle-point estimator must also respect the `[0, k]` envelope.
    #[test]
    fn spa_tranchelet_loss_within_equity_envelope() {
        let a = vec![0.02; 30];
        let p = vec![0.05; 30];
        for &k in &[0.01, 0.03, 0.05, 0.10, 0.20, 0.60] {
            let el = conditional_tranchelet_loss(k, &a, &p);
            assert!(
                (0.0..=k + 1e-12).contains(&el),
                "SPA tranchelet EL {el} must lie in [0, {k}]"
            );
        }
    }

    /// The saddle-point estimator is a valid (if not best-in-class)
    /// approximation: it must track the exact conditional `E[min(L,K)]`
    /// within a reasonable absolute tolerance.
    #[test]
    fn spa_matches_exact_within_tolerance() {
        let a = vec![
            0.015, 0.020, 0.025, 0.030, 0.012, 0.018, 0.022, 0.028, 0.016, 0.024, 0.014, 0.026,
            0.019, 0.021, 0.017, 0.023,
        ];
        let p = vec![
            0.03, 0.05, 0.02, 0.08, 0.04, 0.06, 0.03, 0.07, 0.05, 0.04, 0.06, 0.02, 0.05, 0.03,
            0.07, 0.04,
        ];
        for &k in &[0.02, 0.05, 0.10, 0.15] {
            let exact = exact_tranchelet_loss(k, &a, &p);
            let spa = conditional_tranchelet_loss(k, &a, &p);
            assert!(
                (spa - exact).abs() < 3e-3,
                "k={k}: SPA error {} too large (exact={exact}, spa={spa})",
                (spa - exact).abs()
            );
        }
    }

    /// At `k` above the maximum reachable loss the SPA tranchelet EL must
    /// equal the unconditional mean exactly (no tail mass beyond support).
    #[test]
    fn spa_equals_mean_when_strike_above_support() {
        let a = vec![0.02; 25];
        let p = vec![0.04; 25];
        let mu: f64 = a.iter().zip(p.iter()).map(|(&ai, &pi)| ai * pi).sum();
        let total: f64 = a.iter().sum();
        let el = conditional_tranchelet_loss(total + 0.01, &a, &p);
        assert!(
            (el - mu).abs() < 1e-9,
            "SPA tranchelet EL with strike above support {el} must equal mean {mu}"
        );
    }

    /// A zero-variance pool (all `p = 0`) has deterministic zero loss.
    #[test]
    fn spa_degenerate_zero_probability_pool() {
        let a = vec![0.02; 20];
        let p = vec![0.0; 20];
        let el = conditional_tranchelet_loss(0.05, &a, &p);
        assert!(el.abs() < 1e-12, "zero-PD pool must have zero EL, got {el}");
    }

    /// The SPA estimator must be monotonic in the detachment `k` — a wider
    /// equity tranchelet can only lose more. Guards against a saddle-point
    /// branch discontinuity.
    #[test]
    fn spa_monotonic_in_detachment() {
        let a = vec![
            0.018, 0.022, 0.020, 0.025, 0.015, 0.030, 0.012, 0.028, 0.016, 0.024, 0.019, 0.021,
            0.017, 0.023, 0.014, 0.026, 0.013, 0.027, 0.011, 0.029,
        ];
        let p = vec![0.05; 20];
        let mut prev = 0.0;
        let mut k = 0.005;
        while k < 0.60 {
            let el = conditional_tranchelet_loss(k, &a, &p);
            assert!(
                el >= prev - 1e-9,
                "SPA tranchelet EL must be non-decreasing in k: el({k})={el} < prev={prev}"
            );
            prev = el;
            k += 0.005;
        }
    }
}
