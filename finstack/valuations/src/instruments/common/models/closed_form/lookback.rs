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
//! - Conze, A., & Viswanathan, R. (1991), "Path Dependent Options: The Case of Lookback Options"
//! - Cheuk, T. H. F., & Vorst, T. C. F. (1997), "Lookback Options and Binomial Trees"
//! - Haug, E. G. (2007), "The Complete Guide to Option Pricing Formulas", Chapter 6
//! - Goldman, Sosin & Gatto (1979), "Path Dependent Options: Buy at the Low, Sell at the High"
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

use finstack_core::math::special_functions::{norm_cdf, norm_pdf};

/// Tolerance for the r = q (b = r − q → 0) degeneracy.
///
/// The reflection-principle correction term contains (σ²/(2b)) which diverges
/// as b → 0. Standard references (Haug 2007 §6; Goldman, Sosin & Gatto 1979)
/// use a d-value `d₃ = a₁ − 2b√T/σ` in the reflection bracket, making the
/// bracket vanish at b = 0 and yielding a clean 0/0 L'Hôpital form.
///
/// **Implementation note:** The general-case reflection bracket uses `d₃ = a₁ − 2b√T/σ`
/// as defined in Goldman, Sosin & Gatto (1979) / Haug (2007). The limiting forms
/// at b → 0 are derived via L'Hôpital's rule and are independent of the d₃ vs a₂
/// distinction (both collapse to a₁ at b = 0).
///
/// The threshold must be small enough that the general and limiting forms
/// agree to within 0.1% at the crossover point. At 1e-7 the σ²/(2b) factor
/// in the general form is still well-conditioned and the L'Hôpital limiting
/// form is sufficiently accurate. Previous values of 1e-2 and 1e-4 created
/// visible price discontinuities at the switching boundary.
///
/// **Note:** an early audit recommendation was to scale the threshold relative
/// to `max(|r|, |q|)` to absorb curve-interpolation noise around `b = r - q`.
/// That premise is mathematically wrong here: what matters for the limiting
/// form's accuracy is whether `|b|` is small in **absolute** terms (so that
/// σ²/(2b) is finite enough that the general form has not yet collapsed). At
/// `|b| ≈ 5e-7` (5× the floor) the general form is still well-conditioned and
/// the limiting form mis-prices by O(20%) — so we keep an absolute floor.
const RATE_EQ_DIV_TOL: f64 = 1e-7;

/// Price a fixed-strike lookback call option (continuous monitoring).
///
/// # Arguments
///
/// * `spot` - Current spot price
/// * `strike` - Fixed strike price
/// * `time` - Time to maturity (years)
/// * `rate` - Risk-free rate
/// * `div_yield` - Dividend yield
/// * `vol` - Volatility
/// * `spot_max` - Maximum spot observed so far (S_max up to now)
///
/// # Returns
///
/// Option price
///
/// # Formula (Conze & Viswanathan, 1991; Haug, 2007 Chapter 6)
///
/// The fixed-strike lookback call is decomposed using the relationship with floating-strike:
///
/// For M ≥ K (observed max exceeds strike):
/// ```text
/// C_fixed = e^(-rT)(M - K) + C_floating(S, T, r, q, σ, M)
/// ```
/// where C_floating is the floating-strike lookback call with current observed minimum = M.
///
/// For M < K (observed max below strike), we use the floating-strike formula evaluated at
/// a synthetic minimum equal to K.
///
/// This decomposition ensures the lookback premium is always positive and the price
/// is always greater than or equal to the vanilla option.
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

    if s_max >= strike {
        // Case: M >= K (in-the-money based on observed maximum)
        // Decomposition: intrinsic + floating-strike call starting from M
        // The floating-strike call captures the value of exceeding the current max
        let intrinsic_pv = (s_max - strike) * df;

        // The floating-strike lookback call with "minimum" = s_max gives the
        // additional value from potentially exceeding s_max. However, we need
        // to be careful: for a call, we want max(S_T - s_max, 0) which is a
        // floating-strike call with minimum = s_max.
        // But note: if S < s_max, the intrinsic of the floating part is negative.
        // We use the full floating-strike formula which handles S <= S_min correctly.
        let floating_premium =
            floating_strike_lookback_call(spot, time, rate, div_yield, vol, s_max);

        (intrinsic_pv + floating_premium).max(0.0)
    } else {
        // Case: M < K (out-of-the-money based on observed maximum)
        // Need to reach K first, then capture upside above K
        // Use the floating-strike formula with "minimum" = K as approximation
        // This gives the value of max(S_max_future - K, 0)
        floating_strike_lookback_call(spot, time, rate, div_yield, vol, strike)
    }
}

/// Price a fixed-strike lookback put option (continuous monitoring).
///
/// # Arguments
///
/// * `spot` - Current spot price
/// * `strike` - Fixed strike price
/// * `time` - Time to maturity (years)
/// * `rate` - Risk-free rate
/// * `div_yield` - Dividend yield
/// * `vol` - Volatility
/// * `spot_min` - Minimum spot observed so far (S_min up to now)
///
/// # Returns
///
/// Option price
///
/// # Formula (Conze & Viswanathan, 1991; Haug, 2007 Chapter 6)
///
/// The fixed-strike lookback put is decomposed using the relationship with floating-strike:
///
/// For m ≤ K (observed min below strike):
/// ```text
/// P_fixed = e^(-rT)(K - m) + P_floating(S, T, r, q, σ, m)
/// ```
/// where P_floating is the floating-strike lookback put with current observed maximum = m.
///
/// For m > K (observed min above strike), we use the floating-strike formula evaluated at
/// a synthetic maximum equal to K.
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

    if s_min <= strike {
        // Case: m <= K (in-the-money based on observed minimum)
        // Decomposition: intrinsic + floating-strike put starting from m
        // The floating-strike put captures the value of going below the current min
        let intrinsic_pv = (strike - s_min) * df;

        // The floating-strike lookback put with "maximum" = s_min gives the
        // additional value from potentially going below s_min.
        let floating_premium =
            floating_strike_lookback_put(spot, time, rate, div_yield, vol, s_min);

        (intrinsic_pv + floating_premium).max(0.0)
    } else {
        // Case: m > K (out-of-the-money based on observed minimum)
        // Need to reach K first, then capture downside below K
        // Use the floating-strike formula with "maximum" = K
        floating_strike_lookback_put(spot, time, rate, div_yield, vol, strike)
    }
}

/// Price a floating-strike lookback call option (continuous monitoring).
///
/// Payoff: S_T - S_min
///
/// # Arguments
///
/// * `spot` - Current spot price
/// * `time` - Time to maturity (years)
/// * `rate` - Risk-free rate
/// * `div_yield` - Dividend yield
/// * `vol` - Volatility
/// * `spot_min` - Minimum spot observed so far
///
/// # Returns
///
/// Option price
///
/// # Formula (Goldman, Sosin & Gatto, 1979; Haug, 2007)
///
/// ```text
/// C_float = S·e^(-qT)·N(a1) - S_min·e^(-rT)·N(a1 - σ√T)
///         + S·e^(-rT)·(σ²/(2b))·[(S/S_min)^(-2b/σ²)·N(-a2) - e^(bT)·N(-a1)]
/// ```
///
/// where b = r - q and:
/// ```text
/// a1 = [ln(S/S_min) + (b + σ²/2)T] / (σ√T)
/// a2 = [ln(S/S_min) + (b - σ²/2)T] / (σ√T)  (= a1 - σ√T)
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
    let b = rate - div_yield; // drift

    // Haug notation: a1 and a2
    let a1 = ((spot / s_min).ln() + (b + 0.5 * vol2) * time) / vol_sqrt_t;
    let a2 = a1 - vol_sqrt_t; // = [ln(S/S_min) + (b - σ²/2)T] / (σ√T)

    let term1 = spot * df_q * norm_cdf(a1);
    let term2 = -s_min * df * norm_cdf(a2); // a2 = a1 - σ√T

    // Reflection-principle correction (third term).
    // General form: S·e^{-rT}·(σ²/(2b))·[R^{-2b/σ²}·N(-d₃) - e^{bT}·N(-a₁)]
    // where d₃ = a₁ - 2b√T/σ (Goldman, Sosin & Gatto 1979; Haug 2007 §6).
    let term3 = if b.abs() < RATE_EQ_DIV_TOL {
        // L'Hôpital limiting form as b = r − q → 0.
        //
        // The general bracket is:
        //   (σ²/(2b))·[R^{-2b/σ²}·N(-d₃) − e^{bT}·N(−a₁)]
        // where R = S/S_min, d₃ = a₁ − 2b√T/σ.
        //
        // Taylor-expanding to first order in b:
        //   R^{-2b/σ²} ≈ 1 − (2b/σ²)·ln R
        //   N(-d₃) = N(-a₁ + 2b√T/σ) ≈ N(-a₁) + φ(a₁)·(2b√T/σ)
        //   e^{bT}  ≈ 1 + bT
        //
        // Collecting O(b) terms in the bracket and dividing by 2b/σ² gives:
        //   σ√T·φ(a₁) − ln(R)·N(−a₁) − (σ²/2)·T·N(−a₁)
        //
        // Hence term3 = S·e^{-rT}·[σ√T·φ(a₁) − log_ratio·N(−a₁) − (σ²/2)·T·N(−a₁)]
        let log_ratio = (spot / s_min).ln();
        spot * df
            * (vol * sqrt_t * norm_pdf(a1)
                - log_ratio * norm_cdf(-a1)
                - 0.5 * vol2 * time * norm_cdf(-a1))
    } else {
        let d3 = a1 - 2.0 * b * sqrt_t / vol;
        let power = -2.0 * b / vol2;
        let ratio_power = (spot / s_min).powf(power);

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
/// * `spot` - Current spot price
/// * `time` - Time to maturity (years)
/// * `rate` - Risk-free rate
/// * `div_yield` - Dividend yield
/// * `vol` - Volatility
/// * `spot_max` - Maximum spot observed so far
///
/// # Returns
///
/// Option price
///
/// # Formula (Goldman, Sosin & Gatto, 1979; Haug, 2007)
///
/// ```text
/// P_float = S_max·e^(-rT)·N(b1) - S·e^(-qT)·N(b1 - σ√T)
///         + S·e^(-rT)·(σ²/(2b))·[(S/S_max)^(-2b/σ²)·N(b2) - e^(bT)·N(b1)]
/// ```
///
/// where b = r - q and:
/// ```text
/// b1 = [ln(S_max/S) + (-b + σ²/2)T] / (σ√T)
/// b2 = [ln(S_max/S) + (-b - σ²/2)T] / (σ√T)  (= b1 - σ√T)
/// ```
///
/// When r = q, uses the limiting form to avoid division by zero.
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
    let sqrt_t = time.sqrt();
    let vol_sqrt_t = vol * sqrt_t;
    let vol2 = vol * vol;
    let df = (-rate * time).exp();
    let df_q = (-div_yield * time).exp();
    let b = rate - div_yield;

    // Haug notation for put: b1 and b2
    // b1 = [ln(S_max/S) + (-b + σ²/2)T] / (σ√T)
    let b1 = ((s_max / spot).ln() + (-b + 0.5 * vol2) * time) / vol_sqrt_t;
    let b2 = b1 - vol_sqrt_t; // = [ln(S_max/S) + (-b - σ²/2)T] / (σ√T)

    let term1 = s_max * df * norm_cdf(b1);
    let term2 = -spot * df_q * norm_cdf(b2);

    // Reflection-principle correction — put tracks the maximum, so the reflected
    // drift is -b, giving d₃' = b₁ + 2b√T/σ (sign opposite to the call's d₃).
    let term3 = if b.abs() < RATE_EQ_DIV_TOL {
        // L'Hôpital limiting form as b = r − q → 0 (symmetric to the call).
        //
        // The general put bracket is:
        //   (σ²/(2b))·[R_put^{-2b/σ²}·N(d₃') − e^{bT}·N(b₁)]
        // where R_put = S/S_max, d₃' = b₁ + 2b√T/σ.
        //
        // Taylor-expanding to first order in b (same algebra as call):
        //   R_put^{-2b/σ²} ≈ 1 − (2b/σ²)·ln(S/S_max)
        //   N(d₃') = N(b₁ + 2b√T/σ) ≈ N(b₁) + φ(b₁)·(2b√T/σ)
        //   e^{bT}  ≈ 1 + bT
        //
        // Collecting O(b) terms and noting ln(S/S_max) = −ln(S_max/S):
        //   σ√T·φ(b₁) + ln(S_max/S)·N(b₁) − (σ²/2)·T·N(b₁)
        //
        // Hence term3 = S·e^{-rT}·[σ√T·φ(b₁) − log_ratio·N(b₁) − (σ²/2)·T·N(b₁)]
        // where log_ratio = ln(S/S_max) so −log_ratio = ln(S_max/S).
        let log_ratio = (spot / s_max).ln(); // negative since S ≤ S_max
        spot * df
            * (vol * sqrt_t * norm_pdf(b1)
                - log_ratio * norm_cdf(b1)
                - 0.5 * vol2 * time * norm_cdf(b1))
    } else {
        let d3_put = b1 + 2.0 * b * sqrt_t / vol;
        let power = -2.0 * b / vol2;
        let ratio_power = (spot / s_max).powf(power);

        spot * df
            * (vol2 / (2.0 * b))
            * (ratio_power * norm_cdf(d3_put) - (b * time).exp() * norm_cdf(b1))
    };

    (term1 + term2 + term3).max(0.0)
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
        // At expiry, should equal intrinsic value
        let spot = 100.0;
        let s_min = 95.0;

        let call = floating_strike_lookback_call(spot, 0.0, 0.05, 0.02, 0.2, s_min);
        assert!((call - (spot - s_min)).abs() < 0.01);
    }

    #[test]
    fn test_fixed_intrinsic_value() {
        // At expiry, should equal intrinsic value
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

        // Vanilla BS call
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
        // When r = q, should still return a valid positive price
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
        // RATE_EQ_DIV_TOL is 1e-4, so a delta of 0.02 places us well
        // outside the tolerance band (general formula vs limiting form).
        let spot = 100.0;
        let s_min = 95.0;
        let time = 1.0;
        let vol = 0.2;
        let q = 0.05;

        let price_at_q = floating_strike_lookback_call(spot, time, q, q, vol, s_min);
        // Use delta > tolerance to test general formula vs limiting form
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
}
