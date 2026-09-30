//! Black-style option pricing formulas and Greeks.
//!
//! Black-76 functions take a forward `F`, strike `K`, lognormal volatility
//! `sigma`, and expiry `t` in years. They return undiscounted values on a unit
//! annuity or unit discount factor; callers multiply by the relevant annuity,
//! discount factor, notional, or PV01 outside this module.
//!
//! Spot Black-Scholes-Merton functions take spot, continuously compounded
//! risk-free rate, continuous dividend yield, volatility, and expiry. Those
//! functions include the continuous discount factors in the returned spot price.
//!
//! Degenerate Black-76 and shifted-Black inputs return intrinsic value for
//! prices and zero or digital-limit values for Greeks. Spot Black-Scholes
//! functions return `NaN` when any input is non-finite and otherwise return
//! intrinsic value at zero expiry or zero volatility.
//!
//! # References
//!
//! - Black, F. (1976), "The pricing of commodity contracts". `docs/REFERENCES.md#black-1976`
//!
//! - Black, F. and Scholes, M. (1973), "The pricing of options and corporate
//!   liabilities". `docs/REFERENCES.md#black-scholes-1973`
//! - Hull, J. C., *Options, Futures, and Other Derivatives*. `docs/REFERENCES.md#hull-options-futures`
//!

use crate::closed_form::vanilla::bs_price_unchecked;
use crate::types::OptionType;
use finstack_quant_core::math::{norm_cdf, norm_pdf};

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
fn black_state(forward: f64, strike: f64, sigma: f64, t: f64) -> Option<BlackState> {
    if t <= 0.0 || sigma <= 0.0 || forward <= 0.0 || strike <= 0.0 {
        return None;
    }

    let st = sigma * t.sqrt();
    let d1 = ((forward / strike).ln() + 0.5 * st * st) / st;
    Some(BlackState {
        st,
        d1,
        d2: d1 - st,
    })
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
/// discount factor. If `t <= 0`, `sigma <= 0`, `forward <= 0`, or `strike <= 0`,
/// returns intrinsic value `(forward - strike).max(0.0)`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::closed_form::volatility::{black_call, black_put};
///
/// let call = black_call(0.05, 0.04, 0.20, 1.5);
/// let put = black_put(0.05, 0.04, 0.20, 1.5);
/// assert!((call - put - 0.01).abs() < 1e-12);
/// ```
pub fn black_call(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
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
/// discount factor. If the lognormal domain is degenerate, returns intrinsic
/// value `(strike - forward).max(0.0)`.
pub fn black_put(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    match black_state(forward, strike, sigma, t) {
        Some(state) => strike * norm_cdf(-state.d2) - forward * norm_cdf(-state.d1),
        None => (strike - forward).max(0.0),
    }
}

/// Black-Scholes-Merton call price on spot with continuous carry.
///
/// # Arguments
///
/// - `spot`: Current spot price `S`.
/// - `strike`: Exercise price `K`, expressed in the same currency and price
///   units as `spot`.
/// - `rate`: Continuously compounded risk-free rate.
/// - `dividend_yield`: Continuously compounded dividend or convenience yield.
/// - `sigma`: Lognormal volatility as an annual decimal.
/// - `t`: Expiry in years.
///
/// # Returns
///
/// Returns `S exp(-qT) N(d1) - K exp(-rT) N(d2)`, including continuous discount
/// factors. Returns `NaN` if any input is non-finite. For finite degenerate
/// inputs (`t <= 0`, `sigma <= 0`, `spot <= 0`, or `strike <= 0`), returns
/// intrinsic value `(spot - strike).max(0.0)`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::closed_form::volatility::{
///     black_scholes_spot_call,
///     black_scholes_spot_put,
/// };
///
/// let call = black_scholes_spot_call(100.0, 95.0, 0.04, 0.01, 0.20, 1.0);
/// let put = black_scholes_spot_put(100.0, 95.0, 0.04, 0.01, 0.20, 1.0);
/// let parity = 100.0 * (-0.01_f64).exp() - 95.0 * (-0.04_f64).exp();
/// assert!((call - put - parity).abs() < 1e-10);
/// ```
#[must_use]
pub fn black_scholes_spot_call(
    spot: f64,
    strike: f64,
    rate: f64,
    dividend_yield: f64,
    sigma: f64,
    t: f64,
) -> f64 {
    black_scholes_spot(
        spot,
        strike,
        rate,
        dividend_yield,
        sigma,
        t,
        OptionType::Call,
    )
}

/// Black-Scholes-Merton put price on spot with continuous carry.
///
/// # Arguments
///
/// - `spot`: Current spot price `S`.
/// - `strike`: Exercise price `K`, expressed in the same currency and price
///   units as `spot`.
/// - `rate`: Continuously compounded risk-free rate.
/// - `dividend_yield`: Continuously compounded dividend or convenience yield.
/// - `sigma`: Lognormal volatility as an annual decimal.
/// - `t`: Expiry in years.
///
/// # Returns
///
/// Returns `K exp(-rT) N(-d2) - S exp(-qT) N(-d1)`, including continuous
/// discount factors. Returns `NaN` if any input is non-finite. For finite
/// degenerate inputs, returns intrinsic value `(strike - spot).max(0.0)`.
#[must_use]
pub fn black_scholes_spot_put(
    spot: f64,
    strike: f64,
    rate: f64,
    dividend_yield: f64,
    sigma: f64,
    t: f64,
) -> f64 {
    black_scholes_spot(
        spot,
        strike,
        rate,
        dividend_yield,
        sigma,
        t,
        OptionType::Put,
    )
}

/// Spot Black-Scholes-Merton price with this module's degenerate-input
/// conventions layered over the canonical [`bs_price_unchecked`] kernel.
#[inline]
fn black_scholes_spot(
    spot: f64,
    strike: f64,
    rate: f64,
    dividend_yield: f64,
    sigma: f64,
    t: f64,
    option_type: OptionType,
) -> f64 {
    if !all_finite(&[spot, strike, rate, dividend_yield, sigma, t]) {
        return f64::NAN;
    }
    if t <= 0.0 || sigma <= 0.0 || spot <= 0.0 || strike <= 0.0 {
        return match option_type {
            OptionType::Call => (spot - strike).max(0.0),
            OptionType::Put => (strike - spot).max(0.0),
        };
    }
    bs_price_unchecked(spot, strike, rate, dividend_yield, sigma, t, option_type)
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
/// [`black_call`]. Returns `0.0` for degenerate lognormal domains.
pub fn black_vega(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    match black_state(forward, strike, sigma, t) {
        Some(state) => forward * t.sqrt() * norm_pdf(state.d1),
        None => 0.0,
    }
}

/// Black-76 call delta with respect to the forward.
///
/// # Returns
///
/// Returns `N(d1)` in the valid lognormal domain. At zero expiry or another
/// degenerate domain, returns the intrinsic digital limit: `1.0` when
/// `forward >= strike`, otherwise `0.0`.
///
/// # Arguments
///
/// * `forward` - Positive forward rate or forward price `F` on the unit-annuity
///   pricing scale.
/// * `strike` - Positive option strike `K` in the same units as `forward`.
/// * `sigma` - Annualized lognormal volatility as a decimal, for example `0.20`.
/// * `t` - Time to expiry in years.
pub fn black_delta_call(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
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
    black_delta_call(forward, strike, sigma, t) - 1.0
}

/// Black-76 gamma with respect to the forward.
///
/// # Returns
///
/// Returns `n(d1) / (F sigma sqrt(T))` in the valid lognormal domain and `0.0`
/// for degenerate inputs.
///
/// # Arguments
///
/// * `forward` - Positive forward rate or forward price `F` on the unit-annuity
///   pricing scale.
/// * `strike` - Positive option strike `K` in the same units as `forward`.
/// * `sigma` - Annualized lognormal volatility as a decimal, for example `0.20`.
/// * `t` - Time to expiry in years.
pub fn black_gamma(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    match black_state(forward, strike, sigma, t) {
        Some(state) => norm_pdf(state.d1) / (forward * state.st),
        None => 0.0,
    }
}

/// Shifted Black call price with unit annuity.
///
/// Applies [`black_call`] to `forward + shift` and `strike + shift`. The shifted
/// forward and strike must be positive for the lognormal formula; otherwise the
/// underlying Black-76 degenerate-input intrinsic rule applies.
///
/// # Arguments
///
/// * `forward` - Unshifted forward rate or price in the option's native units.
/// * `strike` - Unshifted option strike in the same units as `forward`.
/// * `sigma` - Annualized lognormal volatility as a decimal, for example `0.20`.
/// * `t` - Time to expiry in years.
/// * `shift` - Additive displacement applied to both forward and strike before
///   Black-76 pricing.
#[inline]
pub fn black_shifted_call(forward: f64, strike: f64, sigma: f64, t: f64, shift: f64) -> f64 {
    black_call(forward + shift, strike + shift, sigma, t)
}

/// Shifted Black put price with unit annuity.
///
/// Applies [`black_put`] to `forward + shift` and `strike + shift` on the same
/// undiscounted unit-annuity scale as the unshifted Black-76 functions.
///
/// # Arguments
///
/// * `forward` - Unshifted forward rate or price in the option's native units.
/// * `strike` - Unshifted option strike in the same units as `forward`.
/// * `sigma` - Annualized lognormal volatility as a decimal, for example `0.20`.
/// * `t` - Time to expiry in years.
/// * `shift` - Additive displacement applied to both forward and strike before
///   Black-76 pricing.
#[inline]
pub fn black_shifted_put(forward: f64, strike: f64, sigma: f64, t: f64, shift: f64) -> f64 {
    black_put(forward + shift, strike + shift, sigma, t)
}
