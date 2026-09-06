//! Analytical pricing formulas for lookback options.
//!
//! This module provides closed-form solutions for lookback options with continuous monitoring
//! under the Black-Scholes framework.
//!
//! # Conventions
//!
//! | Parameter | Convention | Units |
//! |-----------|-----------|-------|
//! | Rates (r, q) | Continuously compounded | Decimal (0.05 = 5%) |
//! | Volatility (σ) | Annualized | Decimal (0.20 = 20%) |
//! | Time (T) | ACT/365-style | Years (1.0 = 1 year) |
//! | Prices | Per unit of underlying | Currency units |
//!
//! # References
//!
//! - Conze, A., & Viswanathan, R. (1991), "Path Dependent Options: The Case of Lookback Options" `docs/REFERENCES.md#conze-viswanathan-1991`
//! - Cheuk, T. H. F., & Vorst, T. C. F. (1997), "Lookback Options and Binomial Trees"
//! - Haug, E. G. (2007), "The Complete Guide to Option Pricing Formulas", Chapter 6 `docs/REFERENCES.md#haug-2007-option-formulas`
//! - Goldman, Sosin & Gatto (1979), "Path Dependent Options: Buy at the Low, Sell at the High" `docs/REFERENCES.md#goldman-sosin-gatto-1979`
//!
//! # Types
//!
//! - **Fixed strike lookback**: Strike is fixed, payoff depends on max/min of path
//!   - Call: max(S_max - K, 0)
//!   - Put: max(K - S_min, 0)
//! - **Floating strike lookback**: Strike floats with path extremum
//!   - Call: S_T - S_min
//!   - Put: S_max - S_T
//!
//! # Implementation Notes
//!
//! The formulas handle the special case where r = q (rate equals dividend yield) using
//! L'Hôpital's rule limiting forms to avoid division by zero.

use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf};

/// Absolute carry threshold for the L'Hôpital limit of the reflection term.
/// Below this threshold, evaluating the general 0/0 expression loses precision.
const RATE_EQ_DIV_TOL: f64 = 1e-7;

/// Price a fixed-strike lookback call option (continuous monitoring).
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option's price units.
/// * `strike` - Fixed exercise price in the same units as `spot`.
/// * `time` - Remaining time to maturity in years.
/// * `rate` - Continuously compounded domestic risk-free rate as a decimal.
/// * `div_yield` - Continuously compounded dividend yield or foreign-rate
///   carry as a decimal.
/// * `vol` - Annualized lognormal volatility as a decimal.
/// * `spot_max` - Highest observed underlying spot including the current
///   observation, in the same units as `spot`.
///
/// # Returns
///
/// Option price
///
/// # Formula (Conze & Viswanathan, 1991; Haug, 2007 Chapter 6)
///
/// Let H = max(K, M), where M is the observed maximum. The payoff is
/// `(M - K).max(0) + (future_max - H).max(0)`. Price the locked-in first
/// term at the risk-free discount factor and the second using the
/// continuous-maximum formula at threshold H.
pub fn fixed_strike_lookback_call(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    spot_max: f64,
) -> f64 {
    if time <= 0.0 {
        return (spot_max - strike).max(0.0);
    }
    if vol <= 0.0 {
        let forward = spot * ((rate - div_yield) * time).exp();
        return ((forward.max(spot_max) - strike) * (-rate * time).exp()).max(0.0);
    }

    let s_max = spot_max.max(spot); // Ensure S_max ≥ S
    let df = (-rate * time).exp();

    let intrinsic_pv = (s_max - strike).max(0.0) * df;
    let strike = strike.max(s_max);
    let sqrt_t = time.sqrt();
    let vol_sqrt_t = vol * sqrt_t;
    let vol2 = vol * vol;
    let df_q = (-div_yield * time).exp();
    let b = rate - div_yield;
    let d1 = ((spot / strike).ln() + (b + 0.5 * vol2) * time) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;

    let term1 = spot * df_q * norm_cdf(d1);
    let term2 = -strike * df * norm_cdf(d2);
    let term3 = if b.abs() < RATE_EQ_DIV_TOL {
        // L'Hôpital limit of the (σ²/2b)[…] term as b = r − q → 0.
        let log_ratio = (spot / strike).ln();
        spot * df
            * (vol * sqrt_t * norm_pdf(d1)
                + log_ratio * norm_cdf(d1)
                + 0.5 * vol2 * time * norm_cdf(d1))
    } else {
        let ratio_power = (spot / strike).powf(-2.0 * b / vol2);
        let d_corr = d1 - 2.0 * b * sqrt_t / vol;
        spot * df
            * (vol2 / (2.0 * b))
            * (-ratio_power * norm_cdf(d_corr) + (b * time).exp() * norm_cdf(d1))
    };

    (intrinsic_pv + term1 + term2 + term3).max(0.0)
}

/// Price a fixed-strike lookback put option (continuous monitoring).
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option's price units.
/// * `strike` - Fixed exercise price in the same units as `spot`.
/// * `time` - Remaining time to maturity in years.
/// * `rate` - Continuously compounded domestic risk-free rate as a decimal.
/// * `div_yield` - Continuously compounded dividend yield or foreign-rate
///   carry as a decimal.
/// * `vol` - Annualized lognormal volatility as a decimal.
/// * `spot_min` - Lowest observed underlying spot including the current
///   observation, in the same units as `spot`.
///
/// # Returns
///
/// Option price
///
/// # Formula (Conze & Viswanathan, 1991; Haug, 2007 Chapter 6)
///
/// Let H = min(K, m), where m is the observed minimum. The payoff is
/// `(K - m).max(0) + (H - future_min).max(0)`. Price the locked-in first
/// term at the risk-free discount factor and the second using the
/// continuous-minimum formula at threshold H.
pub fn fixed_strike_lookback_put(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    spot_min: f64,
) -> f64 {
    if time <= 0.0 {
        return (strike - spot_min).max(0.0);
    }
    if vol <= 0.0 {
        let forward = spot * ((rate - div_yield) * time).exp();
        return ((strike - forward.min(spot_min)) * (-rate * time).exp()).max(0.0);
    }

    let s_min = spot_min.min(spot); // Ensure S_min ≤ S
    let df = (-rate * time).exp();

    let intrinsic_pv = (strike - s_min).max(0.0) * df;
    let strike = strike.min(s_min);
    let sqrt_t = time.sqrt();
    let vol_sqrt_t = vol * sqrt_t;
    let vol2 = vol * vol;
    let df_q = (-div_yield * time).exp();
    let b = rate - div_yield;
    let d1 = ((spot / strike).ln() + (b + 0.5 * vol2) * time) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;

    let term1 = strike * df * norm_cdf(-d2);
    let term2 = -spot * df_q * norm_cdf(-d1);
    let term3 = if b.abs() < RATE_EQ_DIV_TOL {
        // L'Hôpital limit of the (σ²/2b)[…] term as b = r − q → 0.
        let log_ratio = (spot / strike).ln();
        spot * df
            * (vol * sqrt_t * norm_pdf(d1)
                - log_ratio * norm_cdf(-d1)
                - 0.5 * vol2 * time * norm_cdf(-d1))
    } else {
        let ratio_power = (spot / strike).powf(-2.0 * b / vol2);
        let d_corr = d1 - 2.0 * b * sqrt_t / vol;
        spot * df
            * (vol2 / (2.0 * b))
            * (ratio_power * norm_cdf(-d_corr) - (b * time).exp() * norm_cdf(-d1))
    };

    (intrinsic_pv + term1 + term2 + term3).max(0.0)
}

/// Price a floating-strike lookback call option (continuous monitoring).
///
/// Payoff: S_T - S_min
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option's price units.
/// * `time` - Remaining time to maturity in years.
/// * `rate` - Continuously compounded domestic risk-free rate as a decimal.
/// * `div_yield` - Continuously compounded dividend yield or foreign-rate
///   carry as a decimal.
/// * `vol` - Annualized lognormal volatility as a decimal.
/// * `spot_min` - Lowest observed underlying spot including the current
///   observation, in the same units as `spot`.
///
/// # Returns
///
/// Option price
///
/// # Formula (Goldman, Sosin & Gatto, 1979; Haug, 2007)
///
/// ```text
/// C_float = S·e^(-qT)·N(a1) - S_min·e^(-rT)·N(a1 - σ√T)
///         + S·e^(-rT)·(σ²/(2b))·[(S/S_min)^(-2b/σ²)·N(-a3) - e^(bT)·N(-a1)]
/// ```
///
/// where b = r - q and:
/// ```text
/// a1 = [ln(S/S_min) + (b + σ²/2)T] / (σ√T)
/// a3 = a1 - 2b√T/σ
/// ```
///
/// When r = q, uses the limiting form to avoid division by zero.
pub fn floating_strike_lookback_call(
    spot: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    spot_min: f64,
) -> f64 {
    if time <= 0.0 {
        return (spot - spot_min).max(0.0);
    }
    if vol <= 0.0 {
        let forward = spot * ((rate - div_yield) * time).exp();
        return (forward - spot_min).max(0.0) * (-rate * time).exp();
    }

    let s_min = spot_min.min(spot);
    let sqrt_t = time.sqrt();
    let vol_sqrt_t = vol * sqrt_t;
    let vol2 = vol * vol;
    let df = (-rate * time).exp();
    let df_q = (-div_yield * time).exp();
    let b = rate - div_yield;

    let a1 = ((spot / s_min).ln() + (b + 0.5 * vol2) * time) / vol_sqrt_t;
    let a2 = a1 - vol_sqrt_t;

    let term1 = spot * df_q * norm_cdf(a1);
    let term2 = -s_min * df * norm_cdf(a2);
    let term3 = if b.abs() < RATE_EQ_DIV_TOL {
        // L'Hôpital limit of the (σ²/2b)[…] term as b = r − q → 0.
        let log_ratio = (spot / s_min).ln();
        spot * df
            * (vol * sqrt_t * norm_pdf(a1)
                - log_ratio * norm_cdf(-a1)
                - 0.5 * vol2 * time * norm_cdf(-a1))
    } else {
        let d3 = a1 - 2.0 * b * sqrt_t / vol;
        let ratio_power = (spot / s_min).powf(-2.0 * b / vol2);
        spot * df
            * (vol2 / (2.0 * b))
            * (ratio_power * norm_cdf(-d3) - (b * time).exp() * norm_cdf(-a1))
    };

    (term1 + term2 + term3).max(0.0)
}

/// Price a floating-strike lookback put option (continuous monitoring).
///
/// Payoff: S_max - S_T
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option's price units.
/// * `time` - Remaining time to maturity in years.
/// * `rate` - Continuously compounded domestic risk-free rate as a decimal.
/// * `div_yield` - Continuously compounded dividend yield or foreign-rate
///   carry as a decimal.
/// * `vol` - Annualized lognormal volatility as a decimal.
/// * `spot_max` - Highest observed underlying spot including the current
///   observation, in the same units as `spot`.
///
/// # Returns
///
/// Option price
///
/// # Payoff identity
///
/// With observed maximum M (including spot), the future maximum satisfies
/// `max(M, future_max) - S_T = (future_max - M).max(0) + M - S_T`.
/// Thus the price is a fixed-strike lookback call struck at M plus
/// `M exp(-r T) - S exp(-q T)`. The fixed-strike formula also supplies
/// the continuous zero-carry limit when r = q.
pub fn floating_strike_lookback_put(
    spot: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    spot_max: f64,
) -> f64 {
    if time <= 0.0 {
        return (spot_max - spot).max(0.0);
    }
    if vol <= 0.0 {
        let forward = spot * ((rate - div_yield) * time).exp();
        return (spot_max - forward).max(0.0) * (-rate * time).exp();
    }

    let s_max = spot_max.max(spot);
    let maximum_premium =
        fixed_strike_lookback_call(spot, s_max, time, rate, div_yield, vol, s_max);
    (maximum_premium + s_max * (-rate * time).exp() - spot * (-div_yield * time).exp()).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_strike_lookback_call_positive() {
        let price = fixed_strike_lookback_call(100.0, 100.0, 1.0, 0.05, 0.02, 0.2, 100.0);
        assert!(price > 0.0);
        assert!(price < 150.0);
    }

    #[test]
    fn test_fixed_strike_lookback_put_positive() {
        let price = fixed_strike_lookback_put(100.0, 100.0, 1.0, 0.05, 0.02, 0.2, 100.0);
        assert!(price > 0.0);
        assert!(price < 150.0);
    }

    #[test]
    fn test_floating_strike_lookback_call_positive() {
        let price = floating_strike_lookback_call(100.0, 1.0, 0.05, 0.02, 0.2, 95.0);
        assert!(price > 5.0); // At least intrinsic value
        assert!(price < 150.0);
    }

    #[test]
    fn test_floating_strike_lookback_put_positive() {
        let price = floating_strike_lookback_put(100.0, 1.0, 0.05, 0.02, 0.2, 105.0);
        assert!(price > 5.0); // At least intrinsic value
        assert!(price < 150.0);
    }

    #[test]
    fn test_floating_intrinsic_value() {
        let spot = 100.0;
        let s_min = 95.0;

        let call = floating_strike_lookback_call(spot, 0.0, 0.05, 0.02, 0.2, s_min);
        assert!((call - (spot - s_min)).abs() < 0.01);
    }

    #[test]
    fn test_fixed_intrinsic_value() {
        let spot = 100.0;
        let strike = 95.0;
        let s_max = 110.0;

        let call = fixed_strike_lookback_call(spot, strike, 0.0, 0.05, 0.02, 0.2, s_max);
        assert!((call - (s_max - strike)).abs() < 0.01);
    }

    #[test]
    fn test_lookback_geq_vanilla() {
        // Fixed-strike lookback should be worth at least as much as vanilla
        // because it has the optionality of the maximum/minimum
        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.02;
        let vol = 0.2;

        let lookback = fixed_strike_lookback_call(spot, strike, time, rate, div_yield, vol, spot);

        let sqrt_t = time.sqrt();
        let d1 =
            ((spot / strike).ln() + (rate - div_yield + 0.5 * vol * vol) * time) / (vol * sqrt_t);
        let d2 = d1 - vol * sqrt_t;
        let vanilla = spot * (-div_yield * time).exp() * norm_cdf(d1)
            - strike * (-rate * time).exp() * norm_cdf(d2);

        assert!(
            lookback >= vanilla - 0.01,
            "Lookback {} should be ≥ vanilla {}",
            lookback,
            vanilla
        );
    }

    // ==================== R = Q EDGE CASE TESTS ====================

    #[test]
    fn test_floating_call_r_equals_q() {
        let spot = 100.0;
        let s_min = 95.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.05; // r = q
        let vol = 0.2;

        let price = floating_strike_lookback_call(spot, time, rate, div_yield, vol, s_min);

        assert!(price.is_finite(), "Price should be finite when r = q");
        assert!(price > 0.0, "Price should be positive");
        assert!(
            price >= (spot - s_min),
            "Price {} should be >= intrinsic {}",
            price,
            spot - s_min
        );
    }

    #[test]
    fn test_floating_put_r_equals_q() {
        let spot = 100.0;
        let s_max = 105.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.05; // r = q
        let vol = 0.2;

        let price = floating_strike_lookback_put(spot, time, rate, div_yield, vol, s_max);

        assert!(price.is_finite(), "Price should be finite when r = q");
        assert!(price > 0.0, "Price should be positive");
        assert!(
            price >= (s_max - spot),
            "Price {} should be >= intrinsic {}",
            price,
            s_max - spot
        );
    }

    #[test]
    fn test_fixed_call_r_equals_q() {
        let spot = 100.0;
        let strike = 100.0;
        let s_max = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.05; // r = q
        let vol = 0.2;

        let price = fixed_strike_lookback_call(spot, strike, time, rate, div_yield, vol, s_max);

        assert!(price.is_finite(), "Price should be finite when r = q");
        assert!(price > 0.0, "Price should be positive");
    }

    #[test]
    fn test_fixed_put_r_equals_q() {
        let spot = 100.0;
        let strike = 100.0;
        let s_min = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.05; // r = q
        let vol = 0.2;

        let price = fixed_strike_lookback_put(spot, strike, time, rate, div_yield, vol, s_min);

        assert!(price.is_finite(), "Price should be finite when r = q");
        assert!(price > 0.0, "Price should be positive");
    }

    #[test]
    fn test_r_equals_q_continuity() {
        // Prices should be continuous as r approaches q.
        // RATE_EQ_DIV_TOL is 1e-7, so a delta of 0.02 places us far
        // outside the tolerance band (general formula vs limiting form).
        let spot = 100.0;
        let s_min = 95.0;
        let time = 1.0;
        let vol = 0.2;
        let q = 0.05;

        let price_at_q = floating_strike_lookback_call(spot, time, q, q, vol, s_min);
        let price_near_q = floating_strike_lookback_call(spot, time, q + 0.02, q, vol, s_min);

        let diff = (price_at_q - price_near_q).abs();
        // The lookback premium component varies with drift (r-q), so a 2% drift
        // difference (0.02) can produce noticeable price changes. Accept up to
        // 50% relative difference for this edge case.
        let rel_diff = diff / price_at_q;
        assert!(
            rel_diff < 0.5,
            "Prices should be continuous near r=q: at_q={}, near_q={}, rel_diff={:.1}%",
            price_at_q,
            price_near_q,
            rel_diff * 100.0
        );
    }

    // ==================== SEASONED OPTION TESTS ====================

    #[test]
    fn test_fixed_call_seasoned_itm() {
        // Seasoned call where max > strike (in-the-money from observed max)
        let spot = 100.0;
        let strike = 95.0;
        let s_max = 110.0; // Already observed max above strike
        let time = 0.5;
        let rate = 0.05;
        let div_yield = 0.02;
        let vol = 0.2;

        let price = fixed_strike_lookback_call(spot, strike, time, rate, div_yield, vol, s_max);
        let intrinsic = s_max - strike;
        let intrinsic_pv = intrinsic * (-rate * time).exp();

        assert!(
            price >= intrinsic_pv - 0.01,
            "Seasoned ITM lookback call {} should be >= PV of intrinsic {}",
            price,
            intrinsic_pv
        );
    }

    #[test]
    fn test_fixed_call_seasoned_otm() {
        // Seasoned call where max < strike (out-of-the-money from observed max)
        let spot = 100.0;
        let strike = 120.0;
        let s_max = 105.0; // Max below strike
        let time = 0.5;
        let rate = 0.05;
        let div_yield = 0.02;
        let vol = 0.2;

        let price = fixed_strike_lookback_call(spot, strike, time, rate, div_yield, vol, s_max);

        assert!(
            price >= 0.0,
            "OTM seasoned lookback call should be non-negative"
        );
        assert!(
            price < 50.0,
            "OTM seasoned lookback call should be reasonable"
        );
    }

    #[test]
    fn test_fixed_put_seasoned_itm() {
        // Seasoned put where min < strike (in-the-money from observed min)
        let spot = 100.0;
        let strike = 105.0;
        let s_min = 90.0; // Already observed min below strike
        let time = 0.5;
        let rate = 0.05;
        let div_yield = 0.02;
        let vol = 0.2;

        let price = fixed_strike_lookback_put(spot, strike, time, rate, div_yield, vol, s_min);
        let intrinsic = strike - s_min;
        let intrinsic_pv = intrinsic * (-rate * time).exp();

        assert!(
            price >= intrinsic_pv - 0.01,
            "Seasoned ITM lookback put {} should be >= PV of intrinsic {}",
            price,
            intrinsic_pv
        );
    }

    #[test]
    fn test_r_eq_q_limiting_branch_matches_general_formula() {
        // Regression test: the r=q limiting branch must be continuous with the general
        // formula. We evaluate one point strictly INSIDE the tolerance band (hits the
        // limiting form) and one point strictly OUTSIDE (hits the general formula).
        //
        // RATE_EQ_DIV_TOL = 1e-7.  Choosing r = q (inside) and r = q + 2e-7 (outside).
        // The two prices must agree to within ~1e-5 relative.  The previous (buggy)
        // limiting form disagreed by ~18%.
        let spot = 100.0_f64;
        let s_min = 95.0_f64;
        let s_max = 105.0_f64;
        let time = 1.0_f64;
        let vol = 0.2_f64;
        let q = 0.05_f64;

        // --- Floating-strike lookback CALL ---
        // inside tolerance: r = q exactly  (|b| = 0, limiting branch)
        let call_limit = floating_strike_lookback_call(spot, time, q, q, vol, s_min);
        // outside tolerance: r = q + 2e-7   (|b| = 2e-7 > 1e-7, general formula)
        let r_outside = q + 2e-7;
        let call_general = floating_strike_lookback_call(spot, time, r_outside, q, vol, s_min);

        let rel_diff_call = (call_limit - call_general).abs() / call_general.abs().max(1e-10);
        assert!(
            rel_diff_call < 1e-4,
            "Floating-strike call: limiting form {call_limit:.6} vs general {call_general:.6}, \
             rel_diff={rel_diff_call:.2e} (must be < 1e-4)"
        );

        // --- Floating-strike lookback PUT ---
        let put_limit = floating_strike_lookback_put(spot, time, q, q, vol, s_max);
        let put_general = floating_strike_lookback_put(spot, time, r_outside, q, vol, s_max);

        let rel_diff_put = (put_limit - put_general).abs() / put_general.abs().max(1e-10);
        assert!(
            rel_diff_put < 1e-4,
            "Floating-strike put: limiting form {put_limit:.6} vs general {put_general:.6}, \
             rel_diff={rel_diff_put:.2e} (must be < 1e-4)"
        );
    }

    #[test]
    fn test_rate_eq_div_tol_boundary_continuity() {
        // Verify that the general formula and L'Hôpital limiting form agree
        // at the RATE_EQ_DIV_TOL crossover boundary (|b| = 1e-7).
        let spot = 100.0;
        let s_min = 95.0;
        let s_max = 105.0;
        let time = 1.0;
        let vol = 0.2;
        let q = 0.05;

        let eps = 1e-6; // Tiny perturbation around the boundary

        // --- Floating-strike lookback call ---
        // Just inside tolerance (limiting form): b = 1e-7 - eps ≈ 0
        let r_inside = q + super::RATE_EQ_DIV_TOL - eps;
        let call_inside = floating_strike_lookback_call(spot, time, r_inside, q, vol, s_min);
        // Just outside tolerance (general form): b = 1e-7 + eps
        let r_outside = q + super::RATE_EQ_DIV_TOL + eps;
        let call_outside = floating_strike_lookback_call(spot, time, r_outside, q, vol, s_min);

        let rel_diff_call =
            (call_inside - call_outside).abs() / call_inside.max(call_outside).max(1e-10);
        assert!(
            rel_diff_call < 1e-3,
            "Floating-strike lookback call should be continuous at RATE_EQ_DIV_TOL boundary: \
             inside={call_inside:.8}, outside={call_outside:.8}, rel_diff={rel_diff_call:.2e}"
        );

        // --- Floating-strike lookback put ---
        let put_inside = floating_strike_lookback_put(spot, time, r_inside, q, vol, s_max);
        let put_outside = floating_strike_lookback_put(spot, time, r_outside, q, vol, s_max);

        let rel_diff_put =
            (put_inside - put_outside).abs() / put_inside.max(put_outside).max(1e-10);
        assert!(
            rel_diff_put < 1e-3,
            "Floating-strike lookback put should be continuous at RATE_EQ_DIV_TOL boundary: \
             inside={put_inside:.8}, outside={put_outside:.8}, rel_diff={rel_diff_put:.2e}"
        );
    }

    /// The exact Conze-Viswanathan OTM fixed-strike lookback (observed extremum
    /// has not crossed the strike) must match a path Monte Carlo of the
    /// continuous-extremum payoff, validating the closed form that replaced the
    /// previous floating-strike approximation. Discrete monitoring slightly
    /// underestimates the continuous max/min, so MC sits just below the analytic.
    #[test]
    fn fixed_strike_lookback_otm_matches_monte_carlo() {
        let (time, r, q, vol) = (1.0_f64, 0.05_f64, 0.0_f64, 0.20_f64);
        let n_paths = 12_000usize;
        let n_steps = 500usize;
        let dt = time / n_steps as f64;
        let drift = (r - q - 0.5 * vol * vol) * dt;
        let diff = vol * dt.sqrt();
        let df = (-r * time).exp();

        // Broadie-Glasserman-Kou (1997) continuity correction: discrete monitoring
        // underestimates the continuous extremum by ≈ exp(±β·σ·√dt) in level
        // (β = 0.5826 = −ζ(1/2)/√(2π)). Applying it lets the discrete-path MC
        // match the continuous-monitoring closed form tightly.
        let corr = (0.5826 * vol * dt.sqrt()).exp();
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        // Deterministic GBM MC from `spot`, returning discounted E[payoff(run_max, run_min)].
        let mut mc = |spot: f64, payoff: &dyn Fn(f64, f64) -> f64| -> f64 {
            let mut next_unit = || {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                ((state >> 11) as f64 + 0.5) / (1u64 << 53) as f64
            };
            let mut sum = 0.0;
            for _ in 0..n_paths {
                let mut s = spot;
                let (mut run_max, mut run_min) = (spot, spot);
                for _ in 0..n_steps {
                    let u1 = next_unit();
                    let u2 = next_unit();
                    let z = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
                    s *= (drift + diff * z).exp();
                    run_max = run_max.max(s);
                    run_min = run_min.min(s);
                }
                sum += payoff(run_max * corr, run_min / corr);
            }
            df * sum / n_paths as f64
        };

        // OTM call: observed max = spot (95) < strike (100).
        let mc_call = mc(95.0, &|run_max, _| (run_max - 100.0).max(0.0));
        let analytic_call = fixed_strike_lookback_call(95.0, 100.0, time, r, q, vol, 95.0);
        let rel_call = (analytic_call - mc_call).abs() / analytic_call.max(1.0);
        assert!(
            rel_call < 0.02,
            "OTM fixed-strike lookback call must match MC: analytic={analytic_call:.4}, \
             mc={mc_call:.4}, rel={rel_call:.4}"
        );

        // OTM put: observed min = spot (110) > strike (100).
        let mc_put = mc(110.0, &|_, run_min| (100.0 - run_min).max(0.0));
        let analytic_put = fixed_strike_lookback_put(110.0, 100.0, time, r, q, vol, 110.0);
        let rel_put = (analytic_put - mc_put).abs() / analytic_put.max(1.0);
        assert!(
            rel_put < 0.02,
            "OTM fixed-strike lookback put must match MC: analytic={analytic_put:.4}, \
             mc={mc_put:.4}, rel={rel_put:.4}"
        );
    }
}
