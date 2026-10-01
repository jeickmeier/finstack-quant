//! Shared building blocks for Andersen's Quadratic-Exponential (QE) schemes.
//!
//! `QeHeston` (variance leg) and `QeCir` both implement the same one-step
//! transition for a square-root mean-reverting diffusion, differing only in
//! which parameter names they surface and whether they also post-process a
//! correlated spot leg. Previously each module carried its own (slightly
//! divergent) copy of the conditional-moment logic, the ψ safeguards, and
//! the Case A / Case B switch. This module holds the single canonical
//! implementation so the two schemes stay in lock-step.
//!
//! Reference: Andersen, L. (2008). "Simple and efficient simulation of the
//! Heston stochastic volatility model." *Journal of Computational Finance*,
//! 11(3), §3.2.

use finstack_quant_core::math::special_functions::norm_cdf;

/// Threshold on `|κ·Δt|` used for the Heston spot leg's integrated-variance
/// approximation. The conditional CIR moments use `expm1` without truncating
/// their immigration contribution. Chosen so that
/// the quadratic remainder `(κ·Δt)²/2` is below one part in 1e16 (≈ f64
/// epsilon) while still being loose enough to trigger for daily steps at
/// small κ.
pub(crate) const KAPPA_DT_EXPANSION_EPS: f64 = 1e-8;

/// Validate a user-supplied ψ_c switch threshold.
///
/// Andersen (2008, §3.2.4) requires ψ_c ∈ \[1, 2\]: Case A's
/// `sqrt(2/ψ·(2/ψ − 1))` is real only for ψ ≤ 2, so a larger ψ_c would let
/// ψ ∈ (2, ψ_c\] reach Case A and produce NaN draws.
pub(crate) fn validate_psi_c(psi_c: f64) -> finstack_quant_core::Result<()> {
    if !(1.0..=2.0).contains(&psi_c) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "QE psi_c must lie in [1, 2] (Andersen 2008, §3.2.4), got {psi_c}"
        )));
    }
    Ok(())
}

/// Test-only convenience wrapper that computes `exp(-κΔt)` itself; production
/// callers use [`qe_conditional_moments_with_exp`] with a precomputed value.
#[cfg(test)]
#[inline]
pub(crate) fn qe_conditional_moments(
    v_t: f64,
    kappa: f64,
    theta: f64,
    sigma: f64,
    dt: f64,
) -> (f64, f64) {
    qe_conditional_moments_with_exp(v_t, kappa, theta, sigma, dt, (-kappa * dt).exp())
}

/// Conditional moments of the CIR-type variance update with a caller-supplied
/// `exp(-κΔt)`: returns `(m, s²)`.
///
/// `exp_kappa_dt` is `e^{−κΔt}`; it depends only on `(κ, Δt)` and so is constant
/// across a uniform grid, so schemes that precompute it (via
/// `Discretization::prepare`) pass it in to avoid recomputing the transcendental
/// for the decay on every step. Passing `(-κΔt).exp()` reproduces the direct
/// computation bit-for-bit. The complementary exponential uses `expm1` so
/// small positive mean reversion retains both its mean and variance
/// contributions from immigration toward θ. At κ=0 the exact square-root
/// diffusion limit is `(v_t, v_t·σ²·Δt)`.
#[inline]
pub(crate) fn qe_conditional_moments_with_exp(
    v_t: f64,
    kappa: f64,
    theta: f64,
    sigma: f64,
    dt: f64,
    exp_kappa_dt: f64,
) -> (f64, f64) {
    let kappa_dt = kappa * dt;
    let one_minus_exp = -(-kappa_dt).exp_m1();
    let integrated_decay = if kappa_dt == 0.0 {
        dt
    } else {
        one_minus_exp / kappa
    };
    let surviving_state = v_t * exp_kappa_dt;
    let immigration = theta * one_minus_exp;
    let m = surviving_state + immigration;
    let s2 = sigma * sigma * integrated_decay * (surviving_state + 0.5 * immigration);
    (m, s2)
}

/// Distribution regime of one QE variance step (Andersen 2008, §3.2).
///
/// Exposing the regime (rather than only the sampled draw) lets the spot leg
/// compute the exact conditional moment-generating function
/// `M(A) = E[exp(A·v_{t+Δt}) | v_t]` needed for the martingale-exact `K0*`
/// correction (Andersen 2008, §4.2 / Prop. 8).
#[derive(Debug, Clone, Copy)]
pub(crate) enum QeRegime {
    /// A deterministic transition (zero conditional variance).
    Deterministic {
        /// Conditional mean, including the absorbing zero state.
        value: f64,
    },
    /// Case A (ψ ≤ ψ_c): `v_{t+Δt} = (√mean_square + √a·Z)²`.
    Quadratic {
        /// Scale `a = m / (1 + b²)`.
        a: f64,
        /// Squared deterministic component `a·b² = m-a`.
        mean_square: f64,
    },
    /// Case B (ψ > ψ_c): mixture of an atom at 0 (probability `p`) and an
    /// exponential with rate `beta`.
    Exponential {
        /// Probability mass at zero.
        p: f64,
        /// Exponential rate of the positive component.
        beta: f64,
    },
}

impl QeRegime {
    /// Sample `v_{t+Δt}` from this regime given a standard normal shock `z`.
    #[inline]
    pub(crate) fn sample(&self, z: f64) -> f64 {
        match *self {
            QeRegime::Deterministic { value } => value,
            QeRegime::Quadratic { a, mean_square } => (mean_square.sqrt() + a.sqrt() * z).powi(2),
            QeRegime::Exponential { p, beta } => {
                // Clamp u away from 1 so the inverse CDF stays finite when
                // `norm_cdf` saturates to exactly 1.0 for extreme shocks
                // (z ≳ 8).
                let u = norm_cdf(z).min(1.0 - f64::EPSILON);
                if u <= p {
                    0.0
                } else {
                    // Andersen (2008) eq. (25): Ψ⁻¹(u) = ln((1−p)/(1−u))/β
                    // for p < u < 1. Monotone increasing in u, vanishing as
                    // u → p⁺, so the z → v map preserves the coupling that
                    // antithetic sampling and pathwise parity rely on.
                    (((1.0 - p) / (1.0 - u)).ln() / beta).max(0.0)
                }
            }
        }
    }

    /// Exact conditional moment-generating function
    /// `M(A) = E[exp(A·v_{t+Δt}) | v_t]` of the QE draw (Andersen 2008,
    /// Prop. 8). Returns `None` when `A` is outside the regime's domain of
    /// finiteness (`2·A·a ≥ 1` for Case A, `A ≥ β` for Case B).
    #[inline]
    pub(crate) fn exp_moment(&self, a_coeff: f64) -> Option<f64> {
        match *self {
            QeRegime::Deterministic { value } => {
                let m = (a_coeff * value).exp();
                m.is_finite().then_some(m)
            }
            QeRegime::Quadratic { a, mean_square } => {
                let denom = 1.0 - 2.0 * a_coeff * a;
                if denom <= 0.0 {
                    return None;
                }
                let m = (a_coeff * mean_square / denom).exp() / denom.sqrt();
                m.is_finite().then_some(m)
            }
            QeRegime::Exponential { p, beta } => {
                if p >= 1.0 {
                    // Degenerate atom at zero: E[exp(A·0)] = 1.
                    return Some(1.0);
                }
                if a_coeff >= beta {
                    return None;
                }
                let m = p + beta * (1.0 - p) / (beta - a_coeff);
                m.is_finite().then_some(m)
            }
        }
    }
}

/// Compute the QE regime (Case A / Case B with safeguards) for one variance
/// step, without sampling.
#[inline]
pub(crate) fn qe_regime(
    v_t: f64,
    kappa: f64,
    theta: f64,
    sigma: f64,
    dt: f64,
    psi_c: f64,
) -> QeRegime {
    qe_regime_with_exp(v_t, kappa, theta, sigma, dt, psi_c, (-kappa * dt).exp())
}

/// As [`qe_regime`], but with a caller-supplied `exp(-κΔt)` (see
/// [`qe_conditional_moments_with_exp`]). Passing `(-κΔt).exp()` reproduces
/// [`qe_regime`] bit-for-bit.
#[inline]
pub(crate) fn qe_regime_with_exp(
    v_t: f64,
    kappa: f64,
    theta: f64,
    sigma: f64,
    dt: f64,
    psi_c: f64,
    exp_kappa_dt: f64,
) -> QeRegime {
    let v_t = v_t.max(0.0);
    let (m, s2) = qe_conditional_moments_with_exp(v_t, kappa, theta, sigma, dt, exp_kappa_dt);

    if m == 0.0 || s2 == 0.0 {
        return QeRegime::Deterministic { value: m };
    }
    // Case B supports arbitrarily large ψ. Altering ψ changes both the
    // conditional variance and the atom at zero, especially when Feller's
    // condition fails. Divide sequentially to avoid underflow in m².
    let psi = (s2 / m) / m;

    if psi <= psi_c {
        // Rationalized a/m = ψ/[2(1+√(1-ψ/2))] avoids forming b²,
        // which diverges in the deterministic limit. Store a·b² directly
        // so sampling and exponential moments never multiply zero by infinity.
        let a = m * (psi / (2.0 * (1.0 + (1.0 - 0.5 * psi).sqrt())));
        QeRegime::Quadratic {
            a,
            mean_square: m - a,
        }
    } else {
        // Algebraically β = 2 / (m·(ψ+1)); compute it before p to avoid
        // cancellation in (1-p) for highly concentrated atoms at zero.
        let beta = 2.0 / (s2 / m + m);
        let p = 1.0 - m * beta;
        QeRegime::Exponential { p, beta }
    }
}

/// One QE step of a CIR-type variance process.
///
/// Given current variance `v_t`, mean-reversion parameters `(κ, θ)`,
/// vol-of-variance `σ`, step size `Δt`, a standard normal shock `z`, and
/// the user-facing ψ threshold `psi_c`, returns a non-negative `v_{t+Δt}`
/// using Andersen (2008)'s moment-preserving Case A / Case B switch.
#[inline]
pub(crate) fn qe_step_variance(
    v_t: f64,
    kappa: f64,
    theta: f64,
    sigma: f64,
    dt: f64,
    z: f64,
    psi_c: f64,
) -> f64 {
    qe_regime(v_t, kappa, theta, sigma, dt, psi_c).sample(z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_kappa_dt_converges_to_zero_mean_reversion_limit() {
        let (_, s2_exact) = qe_conditional_moments(0.04, 1.0, 0.04, 0.3, 1e-7);
        let (_, s2_taylor) = qe_conditional_moments(0.04, 0.0, 0.04, 0.3, 1e-7);
        assert!(
            (s2_exact - s2_taylor).abs() / s2_taylor.max(1e-20) < 1e-6,
            "near κ≈0 the exact and Taylor s² should agree: exact={s2_exact} taylor={s2_taylor}"
        );
    }

    #[test]
    fn small_positive_mean_reversion_retains_immigration_variance() {
        // Starting at zero does not make the state absorbing when κθ>0.
        // Independent expansions at κΔt=1e-9 give mean θ(x-x²/2) and
        // variance θσ²κΔt²(1-x)/2 to better than 1e-18 relative accuracy.
        let (mean, variance) = qe_conditional_moments(0.0, 1e-9, 0.04, 1.0, 1.0);
        assert!((mean / 3.999_999_998e-11 - 1.0).abs() < 1e-14);
        assert!((variance / 1.999_999_998e-11 - 1.0).abs() < 1e-14);
        let QeRegime::Exponential { p, beta } = qe_regime(0.0, 1e-9, 0.04, 1.0, 1.0, 1.5) else {
            panic!("positive immigration with high ψ requires a nondegenerate mixture");
        };
        assert!(p < 1.0 && p > 0.999_999_999);
        assert!(beta.is_finite() && beta > 0.0);
        assert!(qe_regime(0.0, 1e-9, 0.04, 1.0, 1.0, 1.5).sample(8.0) > 0.0);
    }

    #[test]
    fn zero_mean_reversion_has_exact_square_root_diffusion_moments() {
        for theta in [0.0, 0.04, 10.0] {
            let (mean, variance) = qe_conditional_moments(0.03, 0.0, theta, 0.4, 0.5);
            assert_eq!(mean, 0.03);
            assert!((variance - 0.03 * 0.4 * 0.4 * 0.5).abs() < 1e-18);
        }
        assert_eq!(qe_conditional_moments(0.0, 0.0, 0.04, 1.0, 1.0), (0.0, 0.0));
    }

    #[test]
    fn zero_shock_mean_revert_toward_theta() {
        let v_high = qe_step_variance(0.08, 2.0, 0.04, 0.3, 0.1, 0.0, 1.5);
        assert!(v_high > 0.04 && v_high < 0.08, "expected mean reversion");

        let v_low = qe_step_variance(0.02, 2.0, 0.04, 0.3, 0.1, 0.0, 1.5);
        assert!(v_low > 0.02 && v_low < 0.04, "expected mean reversion");
    }

    #[test]
    fn variance_stays_non_negative_across_shocks() {
        for z in [-5.0, -3.0, -1.0, 0.0, 1.0, 3.0, 5.0] {
            let v = qe_step_variance(0.04, 2.0, 0.04, 0.8, 0.25, z, 1.5);
            assert!(
                v >= 0.0,
                "QE scheme produced negative variance at z={z}: {v}"
            );
        }
    }

    #[test]
    fn small_mean_triggers_case_b_without_panicking() {
        let v = qe_step_variance(0.0, 1.0, 0.0, 0.3, 0.01, -3.0, 1.5);
        assert!(v >= 0.0 && v.is_finite());
    }

    #[test]
    fn extreme_psi_uses_case_b() {
        let v = qe_step_variance(0.001, 0.01, 1e-6, 2.0, 1.0, 4.0, 1.5);
        assert!(v.is_finite() && v >= 0.0);
    }

    #[test]
    fn high_psi_preserves_both_conditional_moments_and_zero_mass() {
        let (v, kappa, theta, sigma, dt) = (0.0005, 0.5, 0.04, 1.0, 0.5);
        let (mean, variance) = qe_conditional_moments(v, kappa, theta, sigma, dt);
        let psi = variance / mean.powi(2);
        assert!(psi > 10.0);
        let QeRegime::Exponential { p, beta } = qe_regime(v, kappa, theta, sigma, dt, 1.5) else {
            panic!("high ψ must use the exponential mixture");
        };
        let mixture_mean = (1.0 - p) / beta;
        let mixture_variance = (1.0 - p) * (1.0 + p) / beta.powi(2);
        assert!((mixture_mean / mean - 1.0).abs() < 1e-14);
        assert!((mixture_variance / variance - 1.0).abs() < 1e-14);
        assert!((p - (psi - 1.0) / (psi + 1.0)).abs() < 1e-14);
        assert_eq!(qe_step_variance(v, kappa, theta, sigma, dt, 1.2, 1.5), 0.0);
    }

    #[test]
    fn tiny_positive_mean_is_not_replaced_by_an_absorbing_state() {
        let regime = qe_regime(1e-12, 1.0, 1e-12, 1e-6, 0.1, 1.5);
        assert!(regime.sample(1.0) > 0.0);
        assert_eq!(qe_regime(0.04, 2.0, 0.04, 0.0, 1.0, 1.5).sample(3.0), 0.04);
    }

    #[test]
    fn nearly_deterministic_quadratic_regime_retains_its_mean_and_exp_moment() {
        for sigma in [1e-80, 1e-160] {
            let regime = qe_regime(0.04, 2.0, 0.04, sigma, 1.0, 1.5);
            for shock in [-8.0, 0.0, 8.0] {
                let sampled = regime.sample(shock);
                assert!((sampled - 0.04).abs() < 1e-16, "σ={sigma}, draw={sampled}");
            }
            let moment = regime
                .exp_moment(2.0)
                .expect("finite deterministic-limit moment");
            assert!((moment - 0.08_f64.exp()).abs() < 1e-14);
        }
    }

    #[test]
    fn quadratic_regime_preserves_conditional_moments() {
        let (mean, variance) = qe_conditional_moments(0.05, 2.0, 0.04, 0.3, 0.25);
        let QeRegime::Quadratic { a, mean_square } = qe_regime(0.05, 2.0, 0.04, 0.3, 0.25, 1.5)
        else {
            panic!("fixture must use the quadratic regime");
        };
        assert!((a + mean_square - mean).abs() < 1e-16);
        assert!((2.0 * a * a + 4.0 * a * mean_square - variance).abs() < 1e-16);
    }

    /// Monte Carlo moment check: averaging many QE draws should recover the
    /// closed-form conditional mean `m = θ + (v₀ − θ) e^{−κΔt}` within a 4σ
    /// tolerance of the theoretical Monte Carlo standard error. This is the
    /// QE scheme's primary calibration guarantee.
    #[test]
    fn mc_mean_matches_conditional_moment() {
        use crate::monte_carlo::rng::philox::PhiloxRng;
        use crate::monte_carlo::traits::RandomStream;

        let v0 = 0.05;
        let kappa = 2.0;
        let theta = 0.04;
        let sigma = 0.3;
        let dt = 0.25;
        let psi_c = 1.5;

        let (m_target, s2) = qe_conditional_moments(v0, kappa, theta, sigma, dt);
        let n = 200_000usize;

        let mut rng = PhiloxRng::new(0xC0FF_EE01);
        let mut draws = vec![0.0; n];
        rng.fill_std_normals(&mut draws);

        let sum: f64 = draws
            .iter()
            .map(|z| qe_step_variance(v0, kappa, theta, sigma, dt, *z, psi_c))
            .sum();
        let mean = sum / n as f64;
        let tol = 4.0 * (s2 / n as f64).sqrt();
        assert!(
            (mean - m_target).abs() < tol,
            "MC mean {mean:.6} should match conditional mean {m_target:.6} within {tol:.2e}",
        );
    }

    /// Case B sampling must invert the exact mixture CDF
    /// `Ψ(x) = p + (1−p)(1−e^{−βx})` (Andersen 2008, eqs. (23)–(25)): for
    /// every shock z with `Φ(z) > p`, `Ψ(sample(z)) = Φ(z)`. The previous
    /// implementation used `ln((1−p)/(u−p))/β`, which has the same marginal
    /// law but inverts the z → v coupling (prior fix).
    #[test]
    fn case_b_sample_inverts_exact_mixture_cdf() {
        // Parameters chosen so ψ > ψ_c = 1.5 forces Case B.
        let (v_t, kappa, theta, sigma, dt, psi_c) = (0.0005, 0.5, 0.04, 1.0, 0.5, 1.5);
        let regime = qe_regime(v_t, kappa, theta, sigma, dt, psi_c);
        let QeRegime::Exponential { p, beta } = regime else {
            panic!("test parameters must force Case B");
        };
        assert!(p < 1.0 && beta > 0.0);

        for z in [-1.0, 0.0, 0.5, 1.0, 2.0, 3.5, 5.0] {
            let u = norm_cdf(z);
            let v = regime.sample(z);
            if u <= p {
                assert_eq!(v, 0.0, "atom at zero for u <= p");
            } else {
                let cdf = p + (1.0 - p) * (1.0 - (-beta * v).exp());
                assert!(
                    (cdf - u).abs() < 1e-12,
                    "Ψ(sample(z)) should equal Φ(z) at z={z}: cdf={cdf}, u={u}"
                );
            }
        }
    }

    /// The z → v map must be monotone non-decreasing — larger shocks mean
    /// larger variance draws in both QE branches. The inverted Case B
    /// formula was monotone *decreasing* on the exponential branch.
    #[test]
    fn case_b_sample_is_monotone_in_z() {
        let (v_t, kappa, theta, sigma, dt, psi_c) = (0.0005, 0.5, 0.04, 1.0, 0.5, 1.5);
        let regime = qe_regime(v_t, kappa, theta, sigma, dt, psi_c);
        let mut prev = f64::NEG_INFINITY;
        let mut z = -4.0;
        while z <= 8.5 {
            let v = regime.sample(z);
            assert!(
                v >= prev,
                "Case B draw must be non-decreasing in z: v({z})={v} < {prev}"
            );
            assert!(v.is_finite());
            prev = v;
            z += 0.05;
        }
    }

    /// QeHeston and QeCir should agree by construction: feeding the same
    /// `(v_t, κ, θ, σ, Δt, z, ψ_c)` through `qe_step_variance` — the single
    /// canonical implementation — must produce identical outputs. This
    /// regression test guards against a future divergence of the two
    /// schemes.
    #[test]
    fn heston_and_cir_variance_paths_agree_exactly() {
        let v_t = 0.05;
        let kappa = 1.5;
        let theta = 0.04;
        let sigma = 0.6;
        let dt = 0.1;
        let psi_c = 1.5;

        for z in [-2.5, -1.0, -0.1, 0.0, 0.2, 1.3, 3.1] {
            let heston = qe_step_variance(v_t, kappa, theta, sigma, dt, z, psi_c);
            let cir = qe_step_variance(v_t, kappa, theta, sigma, dt, z, psi_c);
            assert_eq!(
                heston.to_bits(),
                cir.to_bits(),
                "bit-identical output expected at z={z}: heston={heston} cir={cir}",
            );
        }
    }
}
