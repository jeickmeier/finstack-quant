//! Shared utilities for swap rate calculation from Hull-White model.
//!
//! Provides reusable functions for computing forward swap rates and bond prices
//! from Hull-White short rate simulations. Used by both swaption and CMS pricing.

use finstack_quant_models::monte_carlo::process::ou::HullWhite1FParams;
use finstack_quant_models::rates::hull_white::{hw_b, hw_ln_a};

/// Hull-White bond price calculation utilities.
///
/// Computes P(t, T) = A(t, T) * exp(-B(t, T) * r(t))
///
/// where:
/// - B(t, T) = (1 - exp(-κ(T-t))) / κ
/// - A(t, T) depends on model parameters
pub struct HullWhiteBondPrice;

impl HullWhiteBondPrice {
    /// Compute bond price P(t, T) from short rate r(t).
    ///
    /// Uses the curve-calibrated affine reconstruction
    /// `P(t,T) = A(t,T) · exp(−B(t,T) · r(t))` with `B` from
    /// [`hw_b`] and `ln A` from [`hw_ln_a`] (Brigo & Mercurio 2006,
    /// §3.3.1, eqs. 3.39–3.40), evaluated at the process volatility
    /// `σ(t)`.
    ///
    /// # Arguments
    ///
    /// * `params` - Hull-White parameters (κ and σ(t) are used)
    /// * `r_t` - Current short rate
    /// * `t` - Current time
    /// * `maturity_time` - Maturity time (T)
    /// * `discount_curve_fn` - Market discount factors `P_mkt(0, ·)`
    pub fn bond_price(
        params: &HullWhite1FParams,
        r_t: f64,
        t: f64,
        maturity_time: f64,
        discount_curve_fn: impl Fn(f64) -> f64,
    ) -> f64 {
        let b = hw_b(params.kappa, t, maturity_time);
        let ln_a = hw_ln_a(
            params.kappa,
            params.sigma_at_time(t),
            t,
            maturity_time,
            &discount_curve_fn,
        );
        (ln_a - b * r_t).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_models::rates::hull_white::fd_instantaneous_forward;

    /// The pre-W-15 "simplified" Vasicek-style `bond_price`, kept here purely
    /// so the regression test can prove the defect existed and is now fixed.
    /// Uses the flat market forward `f(t,T)` as the drift over `[t, T]`.
    #[allow(non_snake_case)]
    fn old_bond_price(
        params: &HullWhite1FParams,
        r_t: f64,
        t: f64,
        maturity_time: f64,
        discount_curve_fn: impl Fn(f64) -> f64,
    ) -> f64 {
        let kappa = params.kappa;
        let sigma = params.sigma_at_time(t);
        let B = hw_b(kappa, t, maturity_time);
        let tau = maturity_time - t;
        let theta_mid = params.theta_at_time((t + maturity_time) / 2.0);
        let df_T = discount_curve_fn(maturity_time);
        let df_t = discount_curve_fn(t);
        let forward_rate = if tau > 1e-10 {
            -(df_T / df_t).ln() / tau
        } else {
            theta_mid
        };
        let term1 = forward_rate * tau;
        let term2 = forward_rate * B;
        let term3 = (sigma * sigma) / (2.0 * kappa * kappa) * (B - tau);
        let term4 = (sigma * sigma) / (4.0 * kappa) * B * B;
        let A = (term1 - term2 + term3 + term4).exp();
        A * (-B * r_t).exp()
    }

    #[test]
    fn test_hw_bond_price_b_factor() {
        let kappa = 0.1;
        let t = 0.0;
        let t_maturity = 1.0;
        let b = hw_b(kappa, t, t_maturity);

        // B(0,1) with κ=0.1 should be approximately (1 - exp(-0.1)) / 0.1 ≈ 0.9516
        let expected = (1.0 - (-0.1_f64).exp()) / 0.1;
        assert!((b - expected).abs() < 1e-10);
    }

    /// Exact-A property (W-15): the curve-calibrated HW1F bond reconstruction
    /// must satisfy, for *every* `t` (not only near `t = 0`),
    ///
    /// ```text
    /// P(t,T; r = f(0,t)) = (P_mkt(0,T)/P_mkt(0,t)) · exp(−V(t,T))
    /// V(t,T) = (σ²/4κ)·(1 − e^{−2κt})·B(t,T)²
    /// ```
    ///
    /// i.e. at the market instantaneous forward `r(t) = f(0,t)` the `B·f(0,t)`
    /// drift terms cancel and the only residual is the HW1F log-bond variance
    /// `V` (which vanishes at `t = 0`, recovering exact repricing). The former
    /// "simplified" Vasicek-style `a_factor` used the flat forward `f(t,T)` as
    /// drift, so it did not reproduce this — drifting away from `t = 0`.
    #[test]
    #[allow(non_snake_case)]
    fn test_bond_price_exact_a_reproduces_curve_non_flat() {
        let params = HullWhite1FParams::new(0.2, 0.015, 0.03).expect("valid Hull-White parameters");
        let kappa = params.kappa;
        let sigma = params.sigma_at_time(0.0);

        // Non-flat (humped) discount curve: instantaneous forward varies in t,
        // so f(t,T) (flat) ≠ f(0,t) — the case the old formula got wrong.
        let discount_fn = |t: f64| {
            // -ln DF(t) = ∫₀ᵗ f(0,s) ds with f(0,s) = 0.02 + 0.03·s·e^{-0.4 s}
            // (a smooth humped forward). Closed form of the integral:
            let a = 0.4_f64;
            let integral_hump = 0.03 / (a * a) * (1.0 - (-a * t).exp() * (1.0 + a * t));
            (-(0.02 * t + integral_hump)).exp()
        };

        // Market instantaneous forward at t. Use the *same* finite-difference
        // estimator the reconstruction itself uses, so the test isolates the
        // formula (not FD-step mismatch).
        let f0 = |t: f64| fd_instantaneous_forward(&discount_fn, t).unwrap_or(0.0);

        // At several t > 0, evaluate P(t,T) with r = f(0,t) and check it
        // reproduces the forward term ratio modulo the HW1F variance term.
        let mut old_max_err = 0.0_f64;
        for &(t, big_t) in &[(0.5, 2.0), (1.0, 3.0), (2.5, 5.0), (4.0, 7.0)] {
            let r_t = f0(t);
            let recon = HullWhiteBondPrice::bond_price(&params, r_t, t, big_t, discount_fn);

            let B = hw_b(kappa, t, big_t);
            let var = sigma * sigma / (4.0 * kappa) * (1.0 - (-2.0 * kappa * t).exp()) * B * B;
            let expected = discount_fn(big_t) / discount_fn(t) * (-var).exp();
            assert!(
                (recon - expected).abs() < 1e-9,
                "t={t}, T={big_t}: reconstructed P(t,T)={recon:.12} should \
                 match curve ratio · exp(−V) = {expected:.12}"
            );

            // Regression guard: the old "simplified" formula misses this
            // away from t = 0 (it uses the flat forward as drift).
            let old = old_bond_price(&params, r_t, t, big_t, discount_fn);
            old_max_err = old_max_err.max((old - expected).abs());
        }
        assert!(
            old_max_err > 1e-4,
            "regression guard: the pre-W-15 formula should mis-price a \
             non-flat curve away from t=0 (max err {old_max_err:.2e})"
        );
    }

    /// At `t = 0` both the old and the new formula reprice the curve exactly
    /// (`P(0,T) = P_mkt(0,T)` when `r(0) = f(0,0)`); this anchors the fix.
    #[test]
    fn test_bond_price_reprices_curve_at_t0() {
        let params = HullWhite1FParams::new(0.15, 0.01, 0.03).expect("valid Hull-White parameters");
        let discount_fn = |t: f64| (-(0.025 * t + 0.005 * t * t)).exp();
        // f(0,0) = 0.025.
        let r0 = 0.025;
        for &big_t in &[0.25, 1.0, 5.0, 10.0] {
            let recon = HullWhiteBondPrice::bond_price(&params, r0, 0.0, big_t, discount_fn);
            let market = discount_fn(big_t);
            assert!(
                (recon - market).abs() < 1e-6,
                "T={big_t}: P(0,T)={recon:.10} should match market {market:.10}"
            );
        }
    }
}
