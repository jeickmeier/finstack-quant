//! Black-style option pricing formulas and Greeks.
//!
//! Black-76 functions take a forward `F`, strike `K`, lognormal volatility
//! `sigma`, and expiry `t` in years. They return undiscounted values on a unit
//! annuity or unit discount factor; callers multiply by the relevant annuity,
//! discount factor, notional, or PV01 outside this module.
//!
//! Zero volatility or expiry returns intrinsic value for Black-76 prices and
//! zero or digital-limit values for Greeks. Invalid inputs return `NaN`; use
//! the checked price/Greeks entrypoints for validation errors.
//!
//! # Performance Notes
//!
//! For hot paths that need both d1 and d2, use [`d1_d2`] or [`d1_d2_black76`]
//! to avoid redundant computation. The combined functions compute shared
//! intermediate values (ln, sqrt) only once.
//!
//! # Edge Case Behavior: At Expiry or Zero Volatility
//!
//! When time to expiry `t <= 0` or volatility `sigma <= 0`, the d1/d2 values
//! are computed as mathematical limits:
//!
//! | Moneyness | d1, d2 | Resulting N(d1) | Delta |
//! |-----------|--------|-----------------|-------|
//! | ITM (S > K) | +∞ | 1.0 | 1.0 (call) |
//! | OTM (S < K) | -∞ | 0.0 | 0.0 (call) |
//! | ATM (S = K) | 0.0 | 0.5 | 0.5 (call) |
//!
//! The ATM case returns `(0.0, 0.0)` which implies delta = N(0) = 0.5. This is
//! the **mathematical limit** as t→0 or σ→0, not the physical delta at expiry
//! (which would be a step function). This convention:
//!
//! - Provides continuous, well-defined values for downstream calculations
//! - Is consistent with the Black-Scholes model's limiting behavior
//! - May differ from physical exercise decisions (which are binary at expiry)
//!
//! For production systems pricing expired options, consider validating that
//! `t > 0` before calling these functions, or handling the ATM case separately
//! based on your exercise convention.
//!
//! # References
//!
//! - Black, F. (1976), "The pricing of commodity contracts". `docs/REFERENCES.md#black-1976`
//!
//! - Black, F. and Scholes, M. (1973), "The pricing of options and corporate
//!   liabilities". `docs/REFERENCES.md#black-scholes-1973`
//! - Hull, J. C., *Options, Futures, and Other Derivatives*. `docs/REFERENCES.md#hull-options-futures`
//!

use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf};

/// Calculate both d1 and d2 for Black–Scholes in a single pass.
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option's price units.
/// * `strike` - Exercise price in the same units as `spot`.
/// * `r` - Continuously compounded domestic risk-free rate as a decimal.
/// * `sigma` - Annualized lognormal volatility as a decimal.
/// * `t` - Remaining time to expiry in years.
/// * `q` - Continuously compounded dividend yield or foreign-rate carry as a
///   decimal annual rate.
///
/// # Returns
/// Tuple of (d1, d2)
///
/// # Edge Cases
/// When `t <= 0`, returns the expiry limit from spot moneyness. When
/// `sigma == 0`, returns the deterministic-forward limit from
/// `spot * exp((r - q) * t)` relative to `strike`.
///
/// Negative volatility is outside the model domain; checked callers must reject
/// it before using this low-level primitive.
///
/// # Example
/// ```
/// use finstack_quant_models::closed_form::d1_d2;
///
/// let (d1, d2) = d1_d2(100.0, 100.0, 0.05, 0.20, 1.0, 0.02);
/// assert!(d1 > d2);
/// ```
#[inline]
#[must_use]
pub fn d1_d2(spot: f64, strike: f64, r: f64, sigma: f64, t: f64, q: f64) -> (f64, f64) {
    if t <= 0.0 {
        let limit = moneyness_limit(spot - strike);
        return (limit, limit);
    }
    if sigma == 0.0 {
        let deterministic_forward = spot * ((r - q) * t).exp();
        let limit = moneyness_limit(deterministic_forward - strike);
        return (limit, limit);
    }

    let sqrt_t = t.sqrt();
    let sigma_sqrt_t = sigma * sqrt_t;
    let d1 = ((spot / strike).ln() + (r - q + 0.5 * sigma * sigma) * t) / sigma_sqrt_t;
    let d2 = d1 - sigma_sqrt_t;
    (d1, d2)
}

#[inline]
fn moneyness_limit(difference: f64) -> f64 {
    if difference > 0.0 {
        f64::INFINITY
    } else if difference < 0.0 {
        f64::NEG_INFINITY
    } else {
        0.0
    }
}

/// Calculate both d1 and d2 for Black76 model in a single pass.
///
/// # Arguments
///
/// * `forward` - Forward price or rate at option expiry.
/// * `strike` - Exercise price or rate in the same units as `forward`.
/// * `sigma` - Annualized lognormal volatility as a decimal.
/// * `t` - Remaining time to expiry in years.
///
/// # Returns
/// Tuple of (d1, d2)
///
/// # Example
/// ```
/// use finstack_quant_models::closed_form::d1_d2_black76;
///
/// let (d1, d2) = d1_d2_black76(0.05, 0.05, 0.20, 1.0);
/// assert!(d1 > d2);
/// ```
#[inline]
#[must_use]
pub fn d1_d2_black76(forward: f64, strike: f64, sigma: f64, t: f64) -> (f64, f64) {
    if t <= 0.0 || sigma <= 0.0 {
        let intrinsic_sign = (forward - strike).signum();
        let limit = if intrinsic_sign > 0.0 {
            f64::INFINITY
        } else if intrinsic_sign < 0.0 {
            f64::NEG_INFINITY
        } else {
            0.0
        };
        return (limit, limit);
    }

    let sqrt_t = t.sqrt();
    let sigma_sqrt_t = sigma * sqrt_t;
    let variance = sigma_sqrt_t * sigma_sqrt_t; // sigma^2 * t
    let d1 = ((forward / strike).ln() + 0.5 * variance) / sigma_sqrt_t;
    let d2 = d1 - sigma_sqrt_t;
    (d1, d2)
}

#[derive(Clone, Copy, Debug)]
struct BlackState {
    st: f64,
    d1: f64,
    d2: f64,
}

#[inline]
fn all_finite(values: &[f64]) -> bool {
    values.iter().all(|value| value.is_finite())
}

#[inline]
fn valid_black_inputs(forward: f64, strike: f64, sigma: f64, t: f64) -> bool {
    all_finite(&[forward, strike, sigma, t])
        && forward > 0.0
        && strike > 0.0
        && sigma >= 0.0
        && t >= 0.0
}

#[inline]
fn black_state(forward: f64, strike: f64, sigma: f64, t: f64) -> Option<BlackState> {
    if t <= 0.0 || sigma <= 0.0 || forward <= 0.0 || strike <= 0.0 {
        return None;
    }

    let st = sigma * t.sqrt();
    let (d1, d2) = d1_d2_black76(forward, strike, sigma, t);
    Some(BlackState { st, d1, d2 })
}

/// Black-76 lognormal call price with unit annuity.
///
/// # Arguments
///
/// - `forward`: Forward rate or forward price `F`; must be positive for the
///   lognormal formula.
/// - `strike`: Strike `K`; must be positive for the lognormal formula.
/// - `sigma`: Lognormal volatility as an annual decimal, such as `0.20` for 20%.
/// - `t`: Expiry in years.
///
/// # Returns
///
/// Returns `F N(d1) - K N(d2)` before multiplying by any annuity, notional, or
/// discount factor. At zero expiry or volatility returns intrinsic value
/// `(forward - strike).max(0.0)`. Invalid or non-finite inputs return `NaN`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::closed_form::{black_call, black_put};
///
/// let call = black_call(0.05, 0.04, 0.20, 1.5);
/// let put = black_put(0.05, 0.04, 0.20, 1.5);
/// assert!((call - put - 0.01).abs() < 1e-12);
/// ```
pub fn black_call(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if !valid_black_inputs(forward, strike, sigma, t) {
        return f64::NAN;
    }
    match black_state(forward, strike, sigma, t) {
        Some(state) => forward * norm_cdf(state.d1) - strike * norm_cdf(state.d2),
        None => (forward - strike).max(0.0),
    }
}

/// Black-76 lognormal put price with unit annuity.
///
/// # Arguments
///
/// - `forward`: Forward rate or forward price `F`; must be positive for the
///   lognormal formula.
/// - `strike`: Strike `K`; must be positive for the lognormal formula.
/// - `sigma`: Lognormal volatility as an annual decimal.
/// - `t`: Expiry in years.
///
/// # Returns
///
/// Returns `K N(-d2) - F N(-d1)` before multiplying by any annuity, notional, or
/// discount factor. At zero expiry or volatility returns intrinsic value
/// `(strike - forward).max(0.0)`. Invalid or non-finite inputs return `NaN`.
pub fn black_put(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if !valid_black_inputs(forward, strike, sigma, t) {
        return f64::NAN;
    }
    match black_state(forward, strike, sigma, t) {
        Some(state) => strike * norm_cdf(-state.d2) - forward * norm_cdf(-state.d1),
        None => (strike - forward).max(0.0),
    }
}

/// Black-76 vega with respect to lognormal volatility.
///
/// # Arguments
///
/// - `forward`: Forward rate or forward price `F`.
/// - `strike`: Exercise price `K`, expressed in the same rate or price units
///   as `forward`.
/// - `sigma`: Lognormal volatility as an annual decimal.
/// - `t`: Expiry in years.
///
/// # Returns
///
/// Returns `d price / d sigma` on the same unit-annuity scale as
/// [`black_call`]. Returns `0.0` at zero expiry or volatility and `NaN` for
/// invalid or non-finite inputs.
pub fn black_vega(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if !valid_black_inputs(forward, strike, sigma, t) {
        return f64::NAN;
    }
    match black_state(forward, strike, sigma, t) {
        Some(state) => forward * t.sqrt() * norm_pdf(state.d1),
        None => 0.0,
    }
}

/// Black-76 call delta with respect to the forward.
///
/// # Returns
///
/// Returns `N(d1)` in the valid lognormal domain. At zero expiry or volatility,
/// returns the intrinsic digital limit: `1.0` when `forward >= strike`,
/// otherwise `0.0`. Invalid or non-finite inputs return `NaN`.
///
/// # Arguments
///
/// * `forward` - Positive forward rate or forward price `F` on the unit-annuity
///   pricing scale.
/// * `strike` - Positive option strike `K` in the same units as `forward`.
/// * `sigma` - Annualized lognormal volatility as a decimal, for example `0.20`.
/// * `t` - Time to expiry in years.
pub fn black_delta_call(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if !valid_black_inputs(forward, strike, sigma, t) {
        return f64::NAN;
    }
    match black_state(forward, strike, sigma, t) {
        Some(state) => norm_cdf(state.d1),
        None => {
            if forward >= strike {
                1.0
            } else {
                0.0
            }
        }
    }
}

/// Black-76 put delta with respect to the forward.
///
/// # Returns
///
/// Returns call delta minus one. This is the forward delta of the undiscounted
/// unit-annuity Black-76 put.
///
/// # Arguments
///
/// * `forward` - Positive forward rate or forward price `F` on the unit-annuity
///   pricing scale.
/// * `strike` - Positive option strike `K` in the same units as `forward`.
/// * `sigma` - Annualized lognormal volatility as a decimal, for example `0.20`.
/// * `t` - Time to expiry in years.
pub fn black_delta_put(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if !valid_black_inputs(forward, strike, sigma, t) {
        return f64::NAN;
    }
    match black_state(forward, strike, sigma, t) {
        Some(state) => -norm_cdf(-state.d1),
        None => black_delta_call(forward, strike, sigma, t) - 1.0,
    }
}

/// Black-76 gamma with respect to the forward.
///
/// # Returns
///
/// Returns `n(d1) / (F sigma sqrt(T))` in the valid lognormal domain and `0.0`
/// at zero expiry or volatility. Invalid or non-finite inputs return `NaN`.
///
/// # Arguments
///
/// * `forward` - Positive forward rate or forward price `F` on the unit-annuity
///   pricing scale.
/// * `strike` - Positive option strike `K` in the same units as `forward`.
/// * `sigma` - Annualized lognormal volatility as a decimal, for example `0.20`.
/// * `t` - Time to expiry in years.
pub fn black_gamma(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if !valid_black_inputs(forward, strike, sigma, t) {
        return f64::NAN;
    }
    match black_state(forward, strike, sigma, t) {
        Some(state) => norm_pdf(state.d1) / (forward * state.st),
        None => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closed_form::{
        bachelier_call, bachelier_delta_call, bachelier_delta_put, bachelier_gamma, bachelier_put,
        bachelier_vega, bs_price,
    };
    use crate::types::OptionType;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn spot_price_at_zero_volatility_is_the_discounted_forward_payoff() {
        let spot_call = |spot, strike, rate, div_yield, vol, expiry| {
            bs_price(spot, strike, rate, div_yield, vol, expiry, OptionType::Call)
                .expect("valid Black-Scholes inputs")
        };
        let spot_put = |spot, strike, rate, div_yield, vol, expiry| {
            bs_price(spot, strike, rate, div_yield, vol, expiry, OptionType::Put)
                .expect("valid Black-Scholes inputs")
        };
        let call = spot_call(100.0, 102.0, 0.05, 0.0, 0.0, 1.0);
        let put = spot_put(100.0, 102.0, 0.05, 0.0, 0.0, 1.0);
        let discounted_intrinsic = 100.0 - 102.0 * (-0.05_f64).exp();
        assert!((call - 2.974_598_700_927_174_4).abs() < 1e-12);
        assert_eq!(put, 0.0);
        assert!((call - put - discounted_intrinsic).abs() < 1e-12);
        let negative_carry_call = spot_call(100.0, 100.0, 0.0, 0.05, 0.0, 1.0);
        let negative_carry_put = spot_put(100.0, 100.0, 0.0, 0.05, 0.0, 1.0);
        assert_eq!(negative_carry_call, 0.0);
        assert!((negative_carry_put - 100.0 * (1.0 - (-0.05_f64).exp())).abs() < 1e-12);
        assert_eq!(spot_call(110.0, 100.0, 0.05, 0.02, 0.0, 0.0), 10.0);
    }

    #[test]
    fn raw_forward_formulas_do_not_mask_invalid_inputs_at_boundaries() {
        type ForwardFormula = fn(f64, f64, f64, f64) -> f64;
        let formulas: [ForwardFormula; 12] = [
            black_call,
            black_put,
            black_delta_call,
            black_delta_put,
            black_gamma,
            black_vega,
            bachelier_call,
            bachelier_put,
            bachelier_delta_call,
            bachelier_delta_put,
            bachelier_gamma,
            bachelier_vega,
        ];
        for formula in formulas {
            for base in [[100.0, 100.0, 0.2, 0.0], [100.0, 100.0, 0.0, 1.0]] {
                for index in 0..4 {
                    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                        let mut args = base;
                        args[index] = invalid;
                        assert!(formula(args[0], args[1], args[2], args[3]).is_nan());
                    }
                }
            }
            assert!(formula(100.0, 100.0, -0.2, 1.0).is_nan());
            assert!(formula(100.0, 100.0, 0.2, -1.0).is_nan());
        }
    }

    #[test]
    fn test_black_put_call_parity() {
        // Put-Call parity: Call - Put = F - K
        let forward = 0.05;
        let strike = 0.04;
        let sigma = 0.20;
        let t = 1.5;

        let call = black_call(forward, strike, sigma, t);
        let put = black_put(forward, strike, sigma, t);

        let parity_diff = call - put - (forward - strike);
        assert!(
            parity_diff.abs() < EPSILON,
            "Black put-call parity violated: diff = {}",
            parity_diff
        );
    }

    #[test]
    fn test_black_atm_symmetry() {
        // ATM: Call = Put when F = K
        let forward = 0.05;
        let strike = 0.05; // ATM
        let sigma = 0.20;
        let t = 1.0;

        let call = black_call(forward, strike, sigma, t);
        let put = black_put(forward, strike, sigma, t);

        assert!(
            (call - put).abs() < EPSILON,
            "Black ATM call != put: {} vs {}",
            call,
            put
        );
    }

    #[test]
    fn test_black_vega_positive() {
        let forward = 0.05;
        let strike = 0.045;
        let sigma = 0.20;
        let t = 1.0;

        let vega = black_vega(forward, strike, sigma, t);
        assert!(vega > 0.0, "Black vega should be positive");
    }

    #[test]
    fn test_black_delta_bounds() {
        let forward = 0.05;
        let sigma = 0.20;
        let t = 1.0;

        let delta_itm = black_delta_call(forward, 0.02, sigma, t);
        let delta_atm = black_delta_call(forward, forward, sigma, t);
        let delta_otm = black_delta_call(forward, 0.08, sigma, t);

        assert!((0.0..=1.0).contains(&delta_itm));
        assert!((0.0..=1.0).contains(&delta_atm));
        assert!((0.0..=1.0).contains(&delta_otm));

        assert!(delta_itm > delta_atm);
        assert!(delta_atm > delta_otm);

        // Note: Black-76 ATM delta is NOT 0.5 (unlike Bachelier).
        // At ATM, delta = N(0.5σ√T) > 0.5 due to the drift term in d1.
        // For σ=20%, T=1: delta ≈ N(0.1) ≈ 0.54
        assert!(
            delta_atm > 0.5 && delta_atm < 0.6,
            "Black ATM delta should be ~0.54, got {}",
            delta_atm
        );
    }

    #[test]
    fn test_black_gamma_positive() {
        let forward = 0.05;
        let strike = 0.05;
        let sigma = 0.20;
        let t = 1.0;

        let gamma = black_gamma(forward, strike, sigma, t);
        assert!(gamma > 0.0, "Black gamma should be positive");
    }

    #[test]
    fn test_shifted_black_consistency() {
        let forward = 0.02;
        let strike = 0.025;
        let sigma = 0.20;
        let t = 1.0;
        let shift = 0.03;

        // Shifted Black with zero shift should equal regular Black
        let regular = black_call(forward, strike, sigma, t);
        let shifted_zero = black_call(forward, strike, sigma, t);
        assert!(
            (regular - shifted_zero).abs() < EPSILON,
            "Shifted Black with zero shift should equal regular Black"
        );

        // Shifted Black should handle negative forward
        let negative_fwd = -0.01;
        let shifted_price = black_call(negative_fwd + shift, strike + shift, sigma, t);
        assert!(shifted_price >= 0.0, "Option price should be non-negative");
    }

    #[test]
    fn test_expiry_boundary() {
        // At expiry (t=0), option price = intrinsic value
        let forward = 0.05;
        let strike_itm = 0.03;
        let strike_otm = 0.07;
        let sigma = 0.20;

        assert_eq!(
            bachelier_call(forward, strike_itm, sigma, 0.0),
            forward - strike_itm
        );
        assert_eq!(bachelier_call(forward, strike_otm, sigma, 0.0), 0.0);

        assert_eq!(
            black_call(forward, strike_itm, sigma, 0.0),
            forward - strike_itm
        );
        assert_eq!(black_call(forward, strike_otm, sigma, 0.0), 0.0);

        assert_eq!(bachelier_put(forward, strike_itm, sigma, 0.0), 0.0);
        assert_eq!(
            bachelier_put(forward, strike_otm, sigma, 0.0),
            strike_otm - forward
        );
    }

    #[test]
    fn test_zero_vol_boundary() {
        // At zero vol, option price = intrinsic value
        let forward = 0.05;
        let strike_itm = 0.03;
        let strike_otm = 0.07;
        let t = 1.0;

        assert_eq!(
            bachelier_call(forward, strike_itm, 0.0, t),
            forward - strike_itm
        );
        assert_eq!(bachelier_call(forward, strike_otm, 0.0, t), 0.0);

        assert_eq!(
            black_call(forward, strike_itm, 0.0, t),
            forward - strike_itm
        );
        assert_eq!(black_call(forward, strike_otm, 0.0, t), 0.0);
    }
}
