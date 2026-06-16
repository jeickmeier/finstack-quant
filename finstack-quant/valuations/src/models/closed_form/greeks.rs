//! Black-Scholes-Merton analytical Greeks with full derivations.
//!
//! Provides closed-form formulas for option sensitivities (Greeks) derived from
//! the Black-Scholes-Merton model. These analytical formulas serve as:
//! 1. **Production risk metrics** for vanilla options
//! 2. **Validation benchmarks** for numerical Greek computations
//!
//! # Relationship to [`super::vanilla`]
//!
//! [`super::vanilla::bs_greeks`] computes all first-order Greeks in a single pass
//! using consistent scaling conventions (vega per 1%, rho per 1%). Both modules
//! return **annualized** theta; `vanilla::bs_greeks` additionally exposes a
//! `theta_days_per_year` parameter that converts to per-day theta for reporting.
//! Use that function when you need all Greeks at once with an `OptionType`
//! discriminant. The individual functions in *this* module (`bs_call_delta`,
//! `bs_gamma`, etc.) are useful when only a subset of Greeks is needed.
//!
//! # Mathematical Foundation
//!
//! The Black-Scholes-Merton model assumes:
//! - Geometric Brownian motion for the underlying asset
//! - Constant volatility σ
//! - Constant risk-free rate r
//! - Continuous dividend yield q
//! - No arbitrage and frictionless markets
//!
//! ## Pricing Formulas
//!
//! **European Call:**
//! ```text
//! C = S·e^(-qT)·N(d₁) - K·e^(-rT)·N(d₂)
//! ```
//!
//! **European Put:**
//! ```text
//! P = K·e^(-rT)·N(-d₂) - S·e^(-qT)·N(-d₁)
//! ```
//!
//! where:
//! ```text
//! d₁ = [ln(S/K) + (r - q + σ²/2)T] / (σ√T)
//! d₂ = d₁ - σ√T
//! ```
//!
//! and N(·) is the cumulative standard normal distribution.
//!
//! # Greeks Definitions
//!
//! Greeks measure the sensitivity of option prices to various parameters:
//!
//! - **Delta (Δ)**: ∂V/∂S - Sensitivity to spot price changes
//! - **Gamma (Γ)**: ∂²V/∂S² - Rate of change of delta
//! - **Vega (ν)**: ∂V/∂σ - Sensitivity to volatility changes
//! - **Theta (Θ)**: ∂V/∂t - Time decay (negative of ∂V/∂T)
//! - **Rho (ρ)**: ∂V/∂r - Sensitivity to interest rate changes
//!
//! # References
//!
//! - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate Liabilities."
//!   *Journal of Political Economy*, 81(3), 637-654.
//! - Merton, R. C. (1973). "Theory of Rational Option Pricing."
//!   *Bell Journal of Economics and Management Science*, 4(1), 141-183.
//! - Hull, J. C. (2018). *Options, Futures, and Other Derivatives* (10th ed.).
//!   Pearson. Chapter 19: The Greek Letters.
//! - Haug, E. G. (2007). *The Complete Guide to Option Pricing Formulas* (2nd ed.).
//!   McGraw-Hill. Chapter 1.
//!
//! # Examples
//!
//! ## Computing All Greeks
//!
//! ```ignore
//! use finstack_quant_valuations::models::closed_form::greeks::{
//!     bs_call_greeks, BsGreeks
//! };
//!
//! let spot = 100.0;
//! let strike = 100.0;
//! let time = 1.0;
//! let rate = 0.05;
//! let div_yield = 0.02;
//! let vol = 0.20;
//!
//! let greeks: BsGreeks = bs_call_greeks(spot, strike, time, rate, div_yield, vol);
//! assert!(greeks.is_valid());
//! ```
//!
//! ## Individual Greek Calculations
//!
//! ```ignore
//! use finstack_quant_valuations::models::closed_form::greeks::{
//!     bs_call_delta, bs_gamma, bs_vega
//! };
//!
//! let spot = 100.0;
//! let strike = 100.0;
//! let time = 0.25;
//! let rate = 0.05;
//! let div_yield = 0.0;
//! let vol = 0.30;
//!
//! let delta = bs_call_delta(spot, strike, time, rate, div_yield, vol);
//! assert!((delta - 0.5).abs() < 0.1);
//!
//! let gamma = bs_gamma(spot, strike, time, rate, div_yield, vol);
//! assert!(gamma > 0.0);
//!
//! let vega = bs_vega(spot, strike, time, rate, div_yield, vol);
//! assert!(vega > 0.0);
//! ```

use crate::models::volatility::black::{d1, d1_d2, d2};
use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf};

/// Black-Scholes call option delta.
///
/// Delta measures the sensitivity of the option price to changes in the
/// underlying asset price. For a call option, delta ranges from 0 to 1.
///
/// # Formula
///
/// ```text
/// Δ_call = e^(-qT) · N(d₁)
/// ```
///
/// where q is the continuous dividend yield and N(·) is the cumulative
/// standard normal distribution.
///
/// # Interpretation
///
/// - **Hedge ratio**: Number of shares to hold to delta-hedge the option
/// - **Probability proxy**: Approximates risk-neutral probability of expiring ITM
/// - **Range**: [0, 1] with ATM call delta typically near 0.5
///
/// # Arguments
///
/// * `spot` - Current spot price S
/// * `strike` - Strike price K
/// * `time` - Time to expiration T (in years)
/// * `rate` - Risk-free rate r (continuously compounded)
/// * `div_yield` - Dividend yield q (continuously compounded)
/// * `vol` - Volatility σ (annualized)
///
/// # Returns
///
/// Call delta value. Returns 1.0 if deeply ITM at expiration, 0.0 if OTM.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::models::closed_form::greeks::bs_call_delta;
///
/// let spot = 100.0;
/// let strike = 100.0;    // ATM
/// let time = 1.0;
/// let rate = 0.05;
/// let div_yield = 0.02;
/// let vol = 0.20;
///
/// let delta = bs_call_delta(spot, strike, time, rate, div_yield, vol);
/// // ATM call delta typically near 0.5
/// assert!((delta - 0.5).abs() < 0.1);
/// ```
#[must_use]
pub fn bs_call_delta(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
) -> f64 {
    if time <= 0.0 {
        return if spot > strike { 1.0 } else { 0.0 };
    }

    let d1_val = d1(spot, strike, rate, vol, time, div_yield);
    (-div_yield * time).exp() * norm_cdf(d1_val)
}

/// Black-Scholes put option delta.
///
/// Delta measures the sensitivity of the option price to changes in the
/// underlying asset price. For a put option, delta ranges from -1 to 0.
///
/// # Formula
///
/// ```text
/// Δ_put = -e^(-qT) · N(-d₁) = e^(-qT) · [N(d₁) - 1]
/// ```
///
/// # Interpretation
///
/// - **Hedge ratio**: Negative delta means short position in underlying for hedge
/// - **Range**: [-1, 0] with ATM put delta typically near -0.5
/// - **Put-call parity**: Δ_put = Δ_call - e^(-qT)
///
/// # Arguments
///
/// * `spot` - Current spot price S
/// * `strike` - Strike price K
/// * `time` - Time to expiration T (in years)
/// * `rate` - Risk-free rate r (continuously compounded)
/// * `div_yield` - Dividend yield q (continuously compounded)
/// * `vol` - Volatility σ (annualized)
///
/// # Returns
///
/// Put delta value. Returns -1.0 if deeply ITM at expiration, 0.0 if OTM.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::models::closed_form::greeks::bs_put_delta;
///
/// let spot = 100.0;
/// let strike = 100.0;    // ATM
/// let time = 1.0;
/// let rate = 0.05;
/// let div_yield = 0.02;
/// let vol = 0.20;
///
/// let delta = bs_put_delta(spot, strike, time, rate, div_yield, vol);
/// // ATM put delta typically near -0.5 (accounting for dividend yield adjustment)
/// assert!(delta < 0.0 && delta > -1.0);
/// ```
#[must_use]
pub fn bs_put_delta(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    if time <= 0.0 {
        return if spot < strike { -1.0 } else { 0.0 };
    }

    let d1_val = d1(spot, strike, rate, vol, time, div_yield);
    -(-div_yield * time).exp() * norm_cdf(-d1_val)
}

/// Black-Scholes gamma (same for both calls and puts).
///
/// Gamma measures the rate of change of delta with respect to the underlying
/// asset price. It represents the curvature of the option value function.
///
/// # Formula
///
/// ```text
/// Γ = e^(-qT) · φ(d₁) / (S · σ · √T)
/// ```
///
/// where φ(·) is the standard normal probability density function.
///
/// # Interpretation
///
/// - **Delta hedging cost**: High gamma means frequent rebalancing needed
/// - **Convexity**: Measures how quickly delta changes
/// - **Always positive**: Both calls and puts have positive gamma
/// - **Peaks at ATM**: Maximum gamma occurs when spot ≈ strike
///
/// # Arguments
///
/// * `spot` - Current spot price S
/// * `strike` - Strike price K
/// * `time` - Time to expiration T (in years)
/// * `rate` - Risk-free rate r (continuously compounded)
/// * `div_yield` - Dividend yield q (continuously compounded)
/// * `vol` - Volatility σ (annualized)
///
/// # Returns
///
/// Gamma value. Returns 0.0 at expiration or for zero volatility.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::models::closed_form::greeks::bs_gamma;
///
/// let spot = 100.0;
/// let strike = 100.0;    // ATM has highest gamma
/// let time = 0.25;       // 3 months
/// let rate = 0.05;
/// let div_yield = 0.0;
/// let vol = 0.20;
///
/// let gamma = bs_gamma(spot, strike, time, rate, div_yield, vol);
/// assert!(gamma > 0.0); // Always positive
///
/// // OTM option has lower gamma
/// let gamma_otm = bs_gamma(spot, 110.0, time, rate, div_yield, vol);
/// assert!(gamma > gamma_otm);
/// ```
#[must_use]
pub fn bs_gamma(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    if time <= 0.0 || spot <= 0.0 || vol <= 0.0 {
        return 0.0;
    }

    let d1_val = d1(spot, strike, rate, vol, time, div_yield);
    (-div_yield * time).exp() * norm_pdf(d1_val) / (spot * vol * time.sqrt())
}

/// Black-Scholes vega (same for both calls and puts).
///
/// Vega measures the sensitivity of the option price to changes in implied
/// volatility. Not technically a "Greek" letter, but universally called vega.
///
/// # Formula
///
/// ```text
/// ν = S · e^(-qT) · √T · φ(d₁)
/// ```
///
/// # Interpretation
///
/// - **Volatility exposure**: Change in option value per 1% change in volatility
/// - **Always positive**: Long options have positive vega (benefit from vol increases)
/// - **Time decay**: Vega decreases as expiration approaches
/// - **Peaks at ATM**: Maximum vega when spot ≈ strike
///
/// # Convention
///
/// Vega is typically quoted per 1% (0.01) change in volatility. Some systems
/// quote per 1bp (0.0001) change, so verify conventions.
///
/// # Arguments
///
/// * `spot` - Current spot price S
/// * `strike` - Strike price K
/// * `time` - Time to expiration T (in years)
/// * `rate` - Risk-free rate r (continuously compounded)
/// * `div_yield` - Dividend yield q (continuously compounded)
/// * `vol` - Volatility σ (annualized)
///
/// # Returns
///
/// Vega value (per 1% change in volatility). Returns 0.0 at expiration.
///
/// # Examples
///
/// ```ignore
/// use finstack_quant_valuations::models::closed_form::greeks::bs_vega;
///
/// let spot = 100.0;
/// let strike = 100.0;
/// let time = 1.0;
/// let rate = 0.05;
/// let div_yield = 0.0;
/// let vol = 0.20;
///
/// let vega = bs_vega(spot, strike, time, rate, div_yield, vol);
/// assert!(vega > 0.0); // Always positive for long options
///
/// // Vega decreases as expiration approaches
/// let vega_short = bs_vega(spot, strike, 0.25, rate, div_yield, vol);
/// assert!(vega > vega_short);
/// ```
#[must_use]
pub fn bs_vega(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    // At expiry, or at zero/negative volatility, the option value is the
    // deterministic intrinsic — it carries no volatility sensitivity, so vega
    // is exactly 0. The `vol <= 0` guard is required because `d1_d2` returns a
    // finite `d1 = 0` for an ATM option at `σ = 0`, and `norm_pdf(0) ≈ 0.399`
    // would otherwise yield a spurious non-zero vega. This mirrors the
    // `vol <= 0` guard already present in `bs_gamma`.
    if time <= 0.0 || vol <= 0.0 {
        return 0.0;
    }

    let d1_val = d1(spot, strike, rate, vol, time, div_yield);
    // Scale by 0.01 to represent sensitivity per 1% vol change
    0.01 * spot * (-div_yield * time).exp() * time.sqrt() * norm_pdf(d1_val)
}

/// Black-Scholes call theta (**annualized**).
///
/// Θ_call = -S * φ(d1) * σ / (2√T) * exp(-qT) - r*K*exp(-rT)*N(d2) + q*S*exp(-qT)*N(d1)
///
/// Returns theta per year. For **per-day** theta (divided by a day-count basis),
/// use [`super::vanilla::bs_greeks`] which accepts a `theta_days_per_year` parameter.
#[must_use]
pub fn bs_call_theta(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
) -> f64 {
    if time <= 0.0 {
        return 0.0;
    }

    let d1_val = d1(spot, strike, rate, vol, time, div_yield);
    let d2_val = d2(spot, strike, rate, vol, time, div_yield);

    let term1 = -spot * norm_pdf(d1_val) * vol * (-div_yield * time).exp() / (2.0 * time.sqrt());
    let term2 = -rate * strike * (-rate * time).exp() * norm_cdf(d2_val);
    let term3 = div_yield * spot * (-div_yield * time).exp() * norm_cdf(d1_val);

    term1 + term2 + term3
}

/// Black-Scholes put theta (**annualized**).
///
/// Θ_put = -S * φ(d1) * σ / (2√T) * exp(-qT) + r*K*exp(-rT)*N(-d2) - q*S*exp(-qT)*N(-d1)
///
/// Returns theta per year. For **per-day** theta (divided by a day-count basis),
/// use [`super::vanilla::bs_greeks`] which accepts a `theta_days_per_year` parameter.
#[must_use]
pub fn bs_put_theta(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    if time <= 0.0 {
        return 0.0;
    }

    let d1_val = d1(spot, strike, rate, vol, time, div_yield);
    let d2_val = d2(spot, strike, rate, vol, time, div_yield);

    let term1 = -spot * norm_pdf(d1_val) * vol * (-div_yield * time).exp() / (2.0 * time.sqrt());
    let term2 = rate * strike * (-rate * time).exp() * norm_cdf(-d2_val);
    let term3 = -div_yield * spot * (-div_yield * time).exp() * norm_cdf(-d1_val);

    term1 + term2 + term3
}

/// Black-Scholes call rho (per 1% rate change).
///
/// ```text
/// ρ_call = K · T · e^(-rT) · N(d₂) · 0.01
/// ```
///
/// Returns the PV change per 1% (100bp) parallel shift in the domestic rate,
/// consistent with `BsGreeks::rho_r` and `vanilla::bs_greeks`.
#[must_use]
pub fn bs_call_rho(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    if time <= 0.0 {
        return 0.0;
    }

    let d2_val = d2(spot, strike, rate, vol, time, div_yield);
    strike * time * (-rate * time).exp() * norm_cdf(d2_val) * 0.01
}

/// Black-Scholes put rho (per 1% rate change).
///
/// ```text
/// ρ_put = -K · T · e^(-rT) · N(-d₂) · 0.01
/// ```
///
/// Returns the PV change per 1% (100bp) parallel shift in the domestic rate,
/// consistent with `BsGreeks::rho_r` and `vanilla::bs_greeks`.
#[must_use]
pub fn bs_put_rho(spot: f64, strike: f64, time: f64, rate: f64, div_yield: f64, vol: f64) -> f64 {
    if time <= 0.0 {
        return 0.0;
    }

    let d2_val = d2(spot, strike, rate, vol, time, div_yield);
    -strike * time * (-rate * time).exp() * norm_cdf(-d2_val) * 0.01
}

// Re-export the canonical BsGreeks struct from vanilla module
pub use super::vanilla::BsGreeks;

/// Compute all call Greeks at once.
///
/// Uses a single `d1_d2()` call to compute shared intermediates, avoiding the
/// redundant recomputation that would occur from calling individual Greek functions.
///
/// Returns [`BsGreeks`] with `rho_r` set to the domestic rate sensitivity
/// and `rho_q` set to the dividend yield sensitivity.
#[must_use]
pub fn bs_call_greeks(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
) -> BsGreeks {
    if time <= 0.0 {
        return BsGreeks {
            delta: if spot > strike { 1.0 } else { 0.0 },
            ..BsGreeks::default()
        };
    }

    let (d1_val, d2_val) = d1_d2(spot, strike, rate, vol, time, div_yield);

    let exp_q_t = (-div_yield * time).exp();
    let exp_r_t = (-rate * time).exp();
    let sqrt_t = time.sqrt();
    let pdf_d1 = norm_pdf(d1_val);
    let cdf_d1 = norm_cdf(d1_val);
    let cdf_d2 = norm_cdf(d2_val);

    let delta = exp_q_t * cdf_d1;
    let gamma = if spot <= 0.0 || vol <= 0.0 || sqrt_t <= 0.0 {
        0.0
    } else {
        exp_q_t * pdf_d1 / (spot * vol * sqrt_t)
    };
    // `vol <= 0` guard: at zero vol the value is deterministic intrinsic with no
    // volatility sensitivity, and `norm_pdf(d1)` at the ATM `d1 = 0` limit is
    // non-zero, so vega must be explicitly zeroed (consistent with `bs_vega`).
    let vega = if vol <= 0.0 {
        0.0
    } else {
        0.01 * spot * exp_q_t * sqrt_t * pdf_d1
    };
    let theta = {
        let term1 = -spot * pdf_d1 * vol * exp_q_t / (2.0 * sqrt_t);
        let term2 = -rate * strike * exp_r_t * cdf_d2;
        let term3 = div_yield * spot * exp_q_t * cdf_d1;
        term1 + term2 + term3
    };
    let rho_r = strike * time * exp_r_t * cdf_d2 * 0.01;
    let rho_q = -spot * time * exp_q_t * cdf_d1 * 0.01;

    BsGreeks {
        delta,
        gamma,
        vega,
        theta,
        rho_r,
        rho_q,
    }
}

/// Compute all put Greeks at once.
///
/// Uses a single `d1_d2()` call to compute shared intermediates, avoiding the
/// redundant recomputation that would occur from calling individual Greek functions.
///
/// Returns [`BsGreeks`] with `rho_r` set to the domestic rate sensitivity
/// and `rho_q` set to the dividend yield sensitivity.
#[must_use]
pub fn bs_put_greeks(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
) -> BsGreeks {
    if time <= 0.0 {
        return BsGreeks {
            delta: if spot < strike { -1.0 } else { 0.0 },
            ..BsGreeks::default()
        };
    }

    let (d1_val, d2_val) = d1_d2(spot, strike, rate, vol, time, div_yield);

    let exp_q_t = (-div_yield * time).exp();
    let exp_r_t = (-rate * time).exp();
    let sqrt_t = time.sqrt();
    let pdf_d1 = norm_pdf(d1_val);
    let cdf_m_d1 = norm_cdf(-d1_val);
    let cdf_m_d2 = norm_cdf(-d2_val);

    let delta = -exp_q_t * cdf_m_d1;
    let gamma = if spot <= 0.0 || vol <= 0.0 || sqrt_t <= 0.0 {
        0.0
    } else {
        exp_q_t * pdf_d1 / (spot * vol * sqrt_t)
    };
    // `vol <= 0` guard: see `bs_call_greeks` — vega is zero at zero volatility.
    let vega = if vol <= 0.0 {
        0.0
    } else {
        0.01 * spot * exp_q_t * sqrt_t * pdf_d1
    };
    let theta = {
        let term1 = -spot * pdf_d1 * vol * exp_q_t / (2.0 * sqrt_t);
        let term2 = rate * strike * exp_r_t * cdf_m_d2;
        let term3 = -div_yield * spot * exp_q_t * cdf_m_d1;
        term1 + term2 + term3
    };
    let rho_r = -strike * time * exp_r_t * cdf_m_d2 * 0.01;
    let rho_q = spot * time * exp_q_t * cdf_m_d1 * 0.01;

    BsGreeks {
        delta,
        gamma,
        vega,
        theta,
        rho_r,
        rho_q,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_delta_atm() {
        let delta = bs_call_delta(100.0, 100.0, 1.0, 0.05, 0.02, 0.2);
        // ATM delta should be around 0.5-0.6
        assert!(delta > 0.4 && delta < 0.7);
    }

    #[test]
    fn test_put_delta_atm() {
        let delta = bs_put_delta(100.0, 100.0, 1.0, 0.05, 0.02, 0.2);
        // ATM put delta should be negative, around -0.4 to -0.5
        assert!(delta < 0.0 && delta > -0.7);
    }

    #[test]
    fn test_gamma_positive() {
        let gamma = bs_gamma(100.0, 100.0, 1.0, 0.05, 0.02, 0.2);
        // Gamma should always be positive
        assert!(gamma > 0.0);
    }

    #[test]
    fn test_vega_positive() {
        let vega = bs_vega(100.0, 100.0, 1.0, 0.05, 0.02, 0.2);
        // Vega should always be positive
        assert!(vega > 0.0);
    }

    #[test]
    fn test_put_call_delta_parity() {
        let call_delta = bs_call_delta(100.0, 100.0, 1.0, 0.05, 0.02, 0.2);
        let put_delta = bs_put_delta(100.0, 100.0, 1.0, 0.05, 0.02, 0.2);

        // Delta parity: Δ_call - Δ_put = exp(-qT)
        let lhs = call_delta - put_delta;
        let rhs = (-0.02_f64 * 1.0).exp();

        assert!(
            (lhs - rhs).abs() < 0.001,
            "Delta parity failed: {} vs {}",
            lhs,
            rhs
        );
    }

    #[test]
    fn expiry_and_invalid_inputs_return_documented_boundary_values() {
        assert_eq!(bs_call_delta(110.0, 100.0, 0.0, 0.05, 0.0, 0.2), 1.0);
        assert_eq!(bs_call_delta(90.0, 100.0, 0.0, 0.05, 0.0, 0.2), 0.0);
        assert_eq!(bs_put_delta(90.0, 100.0, 0.0, 0.05, 0.0, 0.2), -1.0);
        assert_eq!(bs_put_delta(110.0, 100.0, 0.0, 0.05, 0.0, 0.2), 0.0);

        assert_eq!(bs_gamma(100.0, 100.0, 0.0, 0.05, 0.0, 0.2), 0.0);
        assert_eq!(bs_gamma(0.0, 100.0, 1.0, 0.05, 0.0, 0.2), 0.0);
        assert_eq!(bs_gamma(100.0, 100.0, 1.0, 0.05, 0.0, 0.0), 0.0);
        assert_eq!(bs_vega(100.0, 100.0, 0.0, 0.05, 0.0, 0.2), 0.0);
        assert_eq!(bs_call_theta(100.0, 100.0, 0.0, 0.05, 0.0, 0.2), 0.0);
        assert_eq!(bs_put_theta(100.0, 100.0, 0.0, 0.05, 0.0, 0.2), 0.0);
        assert_eq!(bs_call_rho(100.0, 100.0, 0.0, 0.05, 0.0, 0.2), 0.0);
        assert_eq!(bs_put_rho(100.0, 100.0, 0.0, 0.05, 0.0, 0.2), 0.0);
    }

    #[test]
    fn put_and_call_share_gamma_and_vega_but_have_opposite_rho_signs() {
        let gamma = bs_gamma(100.0, 100.0, 1.0, 0.05, 0.01, 0.2);
        let vega = bs_vega(100.0, 100.0, 1.0, 0.05, 0.01, 0.2);
        let call = bs_call_greeks(100.0, 100.0, 1.0, 0.05, 0.01, 0.2);
        let put = bs_put_greeks(100.0, 100.0, 1.0, 0.05, 0.01, 0.2);

        assert!((call.gamma - gamma).abs() < 1e-12);
        assert!((put.gamma - gamma).abs() < 1e-12);
        assert!((call.vega - vega).abs() < 1e-12);
        assert!((put.vega - vega).abs() < 1e-12);
        assert!(call.rho_r > 0.0);
        assert!(put.rho_r < 0.0);
    }

    #[test]
    fn theta_and_vega_follow_expected_maturity_monotonicity() {
        let near_call_theta = bs_call_theta(100.0, 100.0, 0.25, 0.05, 0.0, 0.2);
        let far_call_theta = bs_call_theta(100.0, 100.0, 1.0, 0.05, 0.0, 0.2);
        let near_vega = bs_vega(100.0, 100.0, 0.25, 0.05, 0.0, 0.2);
        let far_vega = bs_vega(100.0, 100.0, 1.0, 0.05, 0.0, 0.2);

        assert!(near_call_theta < far_call_theta);
        assert!(far_vega > near_vega);
    }

    /// Audit item 8: `bs_vega` returned a spurious non-zero vega for an exactly
    /// `σ = 0` ATM option.
    ///
    /// Failure mode: with `σ = 0` the `d1_d2` helper returns `(0, 0)` for an ATM
    /// option (spot == strike), and `norm_pdf(0) ≈ 0.399` is non-zero, so
    /// `bs_vega` produced `0.01·S·e^(-qT)·√T·0.399 > 0`. At zero volatility the
    /// option value is deterministic intrinsic — it has no volatility
    /// sensitivity — so vega must be exactly `0`.
    #[test]
    fn bs_vega_is_zero_for_zero_sigma() {
        // Exactly-ATM, zero vol: the failure case from the audit.
        let v_atm = bs_vega(100.0, 100.0, 1.0, 0.05, 0.02, 0.0);
        assert_eq!(v_atm, 0.0, "σ=0 ATM vega must be 0, got {v_atm}");

        // The guard must be consistent across moneyness, not just ATM.
        assert_eq!(bs_vega(100.0, 90.0, 1.0, 0.05, 0.02, 0.0), 0.0);
        assert_eq!(bs_vega(100.0, 110.0, 1.0, 0.05, 0.02, 0.0), 0.0);
        // Negative vol is also non-physical and must be guarded.
        assert_eq!(bs_vega(100.0, 100.0, 1.0, 0.05, 0.02, -0.1), 0.0);

        // The aggregator vega must apply the same guard.
        let call = bs_call_greeks(100.0, 100.0, 1.0, 0.05, 0.02, 0.0);
        let put = bs_put_greeks(100.0, 100.0, 1.0, 0.05, 0.02, 0.0);
        assert_eq!(call.vega, 0.0, "σ=0 call aggregator vega must be 0");
        assert_eq!(put.vega, 0.0, "σ=0 put aggregator vega must be 0");

        // Positive vol still yields a strictly positive vega (no over-zealous guard).
        assert!(bs_vega(100.0, 100.0, 1.0, 0.05, 0.02, 0.2) > 0.0);
    }

    #[test]
    fn greek_aggregators_match_individual_components() {
        let call = bs_call_greeks(105.0, 100.0, 0.75, 0.04, 0.01, 0.25);
        let put = bs_put_greeks(105.0, 100.0, 0.75, 0.04, 0.01, 0.25);

        assert!((call.delta - bs_call_delta(105.0, 100.0, 0.75, 0.04, 0.01, 0.25)).abs() < 1e-12);
        assert!((call.theta - bs_call_theta(105.0, 100.0, 0.75, 0.04, 0.01, 0.25)).abs() < 1e-12);
        assert!((call.rho_r - bs_call_rho(105.0, 100.0, 0.75, 0.04, 0.01, 0.25)).abs() < 1e-12);
        assert!(call.rho_q < 0.0);

        assert!((put.delta - bs_put_delta(105.0, 100.0, 0.75, 0.04, 0.01, 0.25)).abs() < 1e-12);
        assert!((put.theta - bs_put_theta(105.0, 100.0, 0.75, 0.04, 0.01, 0.25)).abs() < 1e-12);
        assert!((put.rho_r - bs_put_rho(105.0, 100.0, 0.75, 0.04, 0.01, 0.25)).abs() < 1e-12);
        assert!(put.rho_q > 0.0);
    }
}
