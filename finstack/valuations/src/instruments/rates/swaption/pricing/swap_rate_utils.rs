//! Shared utilities for swap rate calculation from Hull-White model.
//!
//! Provides reusable functions for computing forward swap rates and bond prices
//! from Hull-White short rate simulations. Used by both swaption and CMS pricing.

use super::monte_carlo_payoff::SwapSchedule;
use finstack_monte_carlo::process::ou::HullWhite1FParams;

/// Hull-White bond price calculation utilities.
///
/// Computes P(t, T) = A(t, T) * exp(-B(t, T) * r(t))
///
/// where:
/// - B(t, T) = (1 - exp(-κ(T-t))) / κ
/// - A(t, T) depends on model parameters
pub struct HullWhiteBondPrice;

impl HullWhiteBondPrice {
    /// Compute B(t, T) factor for bond price.
    ///
    /// B factor represents the sensitivity of bond price to short rate.
    #[allow(non_snake_case)]
    pub fn b_factor(kappa: f64, t: f64, maturity_time: f64) -> f64 {
        if kappa.abs() < 1e-10 {
            // Limit as κ → 0: B(t, T) = T - t
            maturity_time - t
        } else {
            (1.0 - (-kappa * (maturity_time - t)).exp()) / kappa
        }
    }

    /// Compute the exact curve-calibrated HW1F `A(t, T)` factor.
    ///
    /// This is the Hull-White affine bond reconstruction (Brigo & Mercurio
    /// 2006, *Interest Rate Models*, §3.3.1, eqs. 3.39–3.40): the time-`t`
    /// zero-coupon bond is `P(t, T) = A(t, T) · exp(−B(t, T) · r(t))` with
    ///
    /// ```text
    /// ln A(t,T) = ln(P_mkt(0,T)/P_mkt(0,t))
    ///           + B(t,T)·f(0,t)
    ///           − (σ²/4κ)·(1 − e^{−2κt})·B(t,T)²
    /// ```
    ///
    /// where `f(0,t) = −∂/∂t ln P_mkt(0,t)` is the market instantaneous
    /// forward. Built this way, `A(t,T)` is consistent with a θ(t)-calibrated
    /// HW1F process: at `r(t) = f(0,t)` the reconstruction reproduces the
    /// market term ratio `P_mkt(0,T)/P_mkt(0,t)` up to the HW1F log-bond
    /// variance `exp(−(σ²/4κ)(1−e^{−2κt})B²)` *for every* `t`, exactly at
    /// `t = 0` — unlike the old formula, which drifted away from `t = 0`.
    ///
    /// This replaces the former "simplified" Vasicek-style formula, which
    /// used the flat market forward `f(t,T)` as the drift over `[t, T]` —
    /// inconsistent with a curve-fitted HW1F and biased away from `t = 0`.
    ///
    /// # Arguments
    ///
    /// * `params` - Hull-White parameters (only κ, σ are used)
    /// * `t` - Current time
    /// * `maturity_time` - Maturity time (T)
    /// * `discount_curve_fn` - Market discount factors `P_mkt(0, ·)`
    #[allow(non_snake_case)]
    pub fn a_factor(
        params: &HullWhite1FParams,
        t: f64,
        maturity_time: f64,
        discount_curve_fn: impl Fn(f64) -> f64,
    ) -> f64 {
        ln_a_factor(params, t, maturity_time, &discount_curve_fn).exp()
    }

    /// Compute bond price P(t, T) from short rate r(t).
    ///
    /// # Arguments
    ///
    /// * `params` - Hull-White parameters
    /// * `r_t` - Current short rate
    /// * `t` - Current time
    /// * `maturity_time` - Maturity time (T)
    /// * `discount_curve_fn` - Function to get market discount factors
    #[allow(non_snake_case)]
    pub fn bond_price(
        params: &HullWhite1FParams,
        r_t: f64,
        t: f64,
        maturity_time: f64,
        discount_curve_fn: impl Fn(f64) -> f64,
    ) -> f64 {
        let B = Self::b_factor(params.kappa, t, maturity_time);
        let A = Self::a_factor(params, t, maturity_time, discount_curve_fn);
        A * (-B * r_t).exp()
    }
}

/// `ln A(t, T)` for the exact curve-calibrated HW1F affine bond price.
///
/// See [`HullWhiteBondPrice::a_factor`] for the formula and references.
#[allow(non_snake_case)]
fn ln_a_factor(
    params: &HullWhite1FParams,
    t: f64,
    maturity_time: f64,
    discount_curve_fn: &impl Fn(f64) -> f64,
) -> f64 {
    let kappa = params.kappa;
    let sigma = params.sigma;
    let B = HullWhiteBondPrice::b_factor(kappa, t, maturity_time);

    let p0_t = discount_curve_fn(t);
    let p0_T = discount_curve_fn(maturity_time);
    let f0t = instantaneous_forward(discount_curve_fn, t);

    // Variance term (σ²/4κ)·(1 − e^{−2κt})·B², with the κ→0 Taylor limit
    // (1 − e^{−2κt})/2κ → t.
    let var_term = if kappa.abs() < 1e-10 {
        sigma * sigma * t * B * B / 2.0
    } else {
        sigma * sigma / (4.0 * kappa) * (1.0 - (-2.0 * kappa * t).exp()) * B * B
    };

    // P_mkt(0,t), P_mkt(0,T) are positive on a well-formed curve; guard the
    // degenerate extrapolation case so the result is always finite.
    if p0_t > 0.0 && p0_T > 0.0 {
        (p0_T / p0_t).ln() + B * f0t - var_term
    } else {
        // Degenerate curve: collapse to the driftless P = exp(−B·r).
        B * f0t - var_term
    }
}

/// Market instantaneous forward `f(0,t) = −d/dt ln P_mkt(0,t)`.
///
/// Central finite difference where there is room, one-sided forward
/// difference against `P(0,0) = 1` near `t = 0`.
fn instantaneous_forward(discount_curve_fn: &impl Fn(f64) -> f64, t: f64) -> f64 {
    let h = (t * 1e-3).clamp(1e-6, 1e-3);
    if t > h {
        let dfp = discount_curve_fn(t + h);
        let dfm = discount_curve_fn(t - h);
        if dfp > 0.0 && dfm > 0.0 {
            -(dfp.ln() - dfm.ln()) / (2.0 * h)
        } else {
            0.0
        }
    } else {
        let dfh = discount_curve_fn(h);
        if dfh > 0.0 {
            -dfh.ln() / h
        } else {
            0.0
        }
    }
}

/// Forward swap rate calculation from Hull-White model.
///
/// Computes S(t) = [P(t, T_0) - P(t, T_N)] / A(t)
///
/// where:
/// - P(t, T_i) are bond prices
/// - A(t) is the annuity (sum of accrual-weighted bond prices)
pub struct ForwardSwapRate;

impl ForwardSwapRate {
    /// Compute forward swap rate at time t from short rate r(t).
    ///
    /// # Arguments
    ///
    /// * `params` - Hull-White parameters
    /// * `r_t` - Current short rate
    /// * `t` - Current time
    /// * `schedule` - Swap schedule
    /// * `discount_curve_fn` - Function to get market discount factors
    pub fn compute(
        params: &HullWhite1FParams,
        r_t: f64,
        t: f64,
        schedule: &SwapSchedule,
        discount_curve_fn: impl Fn(f64) -> f64,
    ) -> f64 {
        // Only compute if t < swap start
        if t >= schedule.end_date {
            return 0.0; // Swap has expired
        }

        // Compute bond prices for swap start and end
        let p_start = if t <= schedule.start_date {
            HullWhiteBondPrice::bond_price(params, r_t, t, schedule.start_date, &discount_curve_fn)
        } else {
            // After swap start, use current time as start
            1.0
        };

        let p_end =
            HullWhiteBondPrice::bond_price(params, r_t, t, schedule.end_date, &discount_curve_fn);

        // Compute annuity: A(t) = Σ τ_i * P(t, T_i)
        let mut annuity = 0.0;
        for (i, &payment_time) in schedule.payment_dates.iter().enumerate() {
            if payment_time > t {
                let p_i = HullWhiteBondPrice::bond_price(
                    params,
                    r_t,
                    t,
                    payment_time,
                    &discount_curve_fn,
                );
                let tau_i = schedule.accrual_fractions[i];
                annuity += tau_i * p_i;
            }
        }

        // Forward swap rate
        if annuity > 1e-10 {
            (p_start - p_end) / annuity
        } else {
            0.0
        }
    }

    /// Test-only convexity-adjustment accessor.
    ///
    /// Delegates to the canonical [`crate::instruments::rates::cms_option::pricer::convexity_adjustment`]
    /// (first-order Hagan 2003 standard model) so the swaption-side tests
    /// exercise the same formula production CMS pricing uses.
    #[cfg(test)]
    fn convexity_adjustment(
        volatility: f64,
        time_to_fixing: f64,
        swap_tenor: f64,
        forward_rate: f64,
    ) -> f64 {
        crate::instruments::rates::cms_option::pricer::convexity_adjustment(
            volatility,
            time_to_fixing,
            swap_tenor,
            forward_rate,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let sigma = params.sigma;
        let B = HullWhiteBondPrice::b_factor(kappa, t, maturity_time);
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
        let b = HullWhiteBondPrice::b_factor(kappa, t, t_maturity);

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
        let params = HullWhite1FParams::new(0.2, 0.015, 0.03);
        let kappa = params.kappa;
        let sigma = params.sigma;

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
        let f0 = |t: f64| instantaneous_forward(&discount_fn, t);

        // At several t > 0, evaluate P(t,T) with r = f(0,t) and check it
        // reproduces the forward term ratio modulo the HW1F variance term.
        let mut old_max_err = 0.0_f64;
        for &(t, big_t) in &[(0.5, 2.0), (1.0, 3.0), (2.5, 5.0), (4.0, 7.0)] {
            let r_t = f0(t);
            let recon = HullWhiteBondPrice::bond_price(&params, r_t, t, big_t, discount_fn);

            let B = HullWhiteBondPrice::b_factor(kappa, t, big_t);
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
        let params = HullWhite1FParams::new(0.15, 0.01, 0.03);
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

    #[test]
    fn test_forward_swap_rate_simple() {
        let params = HullWhite1FParams::new(0.1, 0.01, 0.03);
        let r_t = 0.03;
        let t = 0.0;

        let payment_dates = vec![1.0, 1.25, 1.5, 1.75, 2.0];
        let accruals = vec![0.25, 0.25, 0.25, 0.25, 0.25];
        let schedule = SwapSchedule::new(1.0, 2.0, payment_dates, accruals)
            .expect("valid swap schedule inputs");

        // Simple discount curve: DF(t) = exp(-0.03 * t)
        let discount_fn = |t: f64| (-0.03 * t).exp();

        let swap_rate = ForwardSwapRate::compute(&params, r_t, t, &schedule, discount_fn);

        // Swap rate should be positive and reasonable
        assert!(swap_rate > 0.0);
        assert!(swap_rate < 1.0);
    }

    #[test]
    fn test_convexity_adjustment() {
        // Parameters: 20% vol, 1Y to fixing, 10Y swap tenor, 3% forward rate.
        let adj = ForwardSwapRate::convexity_adjustment(0.20, 1.0, 10.0, 0.03);

        // The convexity adjustment must be positive (it raises the CMS rate),
        // finite, and a small rate-scale quantity. The earlier formula was
        // dimensionally wrong and returned ~0.118 (1183 bp); a correct
        // first-order Hagan adjustment is single-to-low-double-digit bp.
        assert!(adj.is_finite(), "adjustment must be finite, got {adj}");
        assert!(adj > 0.0, "adjustment must be positive, got {adj}");
        assert!(
            adj < 0.01,
            "adjustment must be a sane rate-scale quantity (< 100 bp), got {adj}"
        );
    }

    #[test]
    fn test_convexity_adjustment_rate_sensitivity() {
        // The adjustment must respond to the forward-rate level and stay a
        // sane, positive, finite rate-scale quantity at both ends.
        let vol = 0.20;
        let time = 1.0;
        let swap_tenor = 10.0;

        let adj_low_rate = ForwardSwapRate::convexity_adjustment(vol, time, swap_tenor, 0.01);
        let adj_high_rate = ForwardSwapRate::convexity_adjustment(vol, time, swap_tenor, 0.05);

        for adj in [adj_low_rate, adj_high_rate] {
            assert!(adj.is_finite() && adj > 0.0 && adj < 0.05, "insane adj {adj}");
        }
        assert!(
            (adj_low_rate - adj_high_rate).abs() > 1e-9,
            "adjustment should depend on the forward-rate level"
        );
    }
}
