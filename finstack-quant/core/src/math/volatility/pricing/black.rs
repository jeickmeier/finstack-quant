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
//! - Black, F. (1976), "The pricing of commodity contracts".
//! - Black, F. and Scholes, M. (1973), "The pricing of options and corporate
//!   liabilities".
//! - Hull, J. C., *Options, Futures, and Other Derivatives*.

use crate::math::{norm_cdf, norm_pdf};

#[derive(Clone, Copy, Debug)]
struct BlackState {
    st: f64,
    d1: f64,
    d2: f64,
}

#[derive(Clone, Copy, Debug)]
struct SpotBlackState {
    d1: f64,
    d2: f64,
    df_r: f64,
    df_q: f64,
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

#[inline]
fn black_scholes_spot_state(
    spot: f64,
    strike: f64,
    rate: f64,
    dividend_yield: f64,
    sigma: f64,
    t: f64,
) -> Option<SpotBlackState> {
    if t <= 0.0 || sigma <= 0.0 || spot <= 0.0 || strike <= 0.0 {
        return None;
    }

    let st = sigma * t.sqrt();
    let ln_sk = (spot / strike).ln();
    let d1 = (ln_sk + (rate - dividend_yield + 0.5 * sigma * sigma) * t) / st;
    Some(SpotBlackState {
        d1,
        d2: d1 - st,
        df_r: (-rate * t).exp(),
        df_q: (-dividend_yield * t).exp(),
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
/// use finstack_quant_core::math::volatility::{black_call, black_put};
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
/// - `strike`: Strike `K`.
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
/// use finstack_quant_core::math::volatility::{
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
    if !all_finite(&[spot, strike, rate, dividend_yield, sigma, t]) {
        return f64::NAN;
    }

    match black_scholes_spot_state(spot, strike, rate, dividend_yield, sigma, t) {
        Some(state) => {
            spot * state.df_q * norm_cdf(state.d1) - strike * state.df_r * norm_cdf(state.d2)
        }
        None => (spot - strike).max(0.0),
    }
}

/// Black-Scholes-Merton put price on spot with continuous carry.
///
/// # Arguments
///
/// - `spot`: Current spot price `S`.
/// - `strike`: Strike `K`.
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
    if !all_finite(&[spot, strike, rate, dividend_yield, sigma, t]) {
        return f64::NAN;
    }

    match black_scholes_spot_state(spot, strike, rate, dividend_yield, sigma, t) {
        Some(state) => {
            strike * state.df_r * norm_cdf(-state.d2) - spot * state.df_q * norm_cdf(-state.d1)
        }
        None => (strike - spot).max(0.0),
    }
}

/// Geometric-average Asian call under GBM with discrete fixings.
///
/// The formula assumes equally spaced future fixings under geometric Brownian
/// motion and continuous rates. It prices only the option payoff; callers apply
/// external notionals or contract multipliers separately.
///
/// # Arguments
///
/// - `spot`: Current spot price.
/// - `strike`: Strike on the geometric average.
/// - `time`: Expiry in years.
/// - `rate`: Continuously compounded risk-free rate.
/// - `div_yield`: Continuously compounded dividend or convenience yield.
/// - `vol`: Annual lognormal volatility.
/// - `num_fixings`: Number of equally spaced fixings.
///
/// # Returns
///
/// Returns the discounted geometric-average Asian call price. Returns `NaN` if
/// any numeric input is non-finite. At zero expiry, returns intrinsic value. If
/// volatility, spot, strike, or fixing count is degenerate, returns discounted
/// forward intrinsic value.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::volatility::geometric_asian_call;
///
/// let price = geometric_asian_call(100.0, 100.0, 1.0, 0.03, 0.01, 0.20, 12);
/// assert!(price.is_finite() && price > 0.0);
/// ```
#[must_use]
pub fn geometric_asian_call(
    spot: f64,
    strike: f64,
    time: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    num_fixings: usize,
) -> f64 {
    if !all_finite(&[spot, strike, time, rate, div_yield, vol]) {
        return f64::NAN;
    }
    if time <= 0.0 {
        return (spot - strike).max(0.0);
    }
    if vol <= 0.0 || spot <= 0.0 || strike <= 0.0 || num_fixings == 0 {
        let fwd = spot * ((rate - div_yield) * time).exp();
        return (-rate * time).exp() * (fwd - strike).max(0.0);
    }

    let n = num_fixings as f64;
    let sigma_g = vol * ((n + 1.0) * (2.0 * n + 1.0) / (6.0 * n * n)).sqrt();
    let b_g = 0.5 * (rate - div_yield - 0.5 * vol * vol) * (n + 1.0) / n + 0.5 * sigma_g * sigma_g;
    let st = sigma_g * time.sqrt();
    if st <= 0.0 {
        return 0.0;
    }

    let ln_s_over_k = (spot / strike).ln();
    let d1 = (ln_s_over_k + (b_g + 0.5 * sigma_g * sigma_g) * time) / st;
    let d2 = d1 - st;
    let df_r = (-rate * time).exp();
    let growth = (b_g * time).exp();
    df_r * (spot * growth * norm_cdf(d1) - strike * norm_cdf(d2))
}

/// Black-76 vega with respect to lognormal volatility.
///
/// # Arguments
///
/// - `forward`: Forward rate or forward price `F`.
/// - `strike`: Strike `K`.
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
pub fn black_delta_put(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    black_delta_call(forward, strike, sigma, t) - 1.0
}

/// Black-76 gamma with respect to the forward.
///
/// # Returns
///
/// Returns `n(d1) / (F sigma sqrt(T))` in the valid lognormal domain and `0.0`
/// for degenerate inputs.
pub fn black_gamma(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    match black_state(forward, strike, sigma, t) {
        Some(state) => norm_pdf(state.d1) / (forward * state.st),
        None => 0.0,
    }
}

/// Black-76 d1: `(ln(F/K) + 0.5 * σ² * T) / (σ * √T)`.
///
/// # Degenerate-Input Convention
///
/// **For degenerate inputs (σ ≤ 0, T ≤ 0, F ≤ 0, or K ≤ 0) this function
/// returns the digital limit by moneyness: `+∞` when `forward >= strike`,
/// otherwise `−∞`.** This is consistent with [`black_delta_call`], which
/// returns delta `1.0` / `0.0` (i.e. `N(±∞)`) in the same degenerate domain.
/// The previous convention returned `0.0` (implying `N(d1) = 0.5`), which
/// disagreed with the delta functions' intrinsic digital limit.
///
/// # Arguments
///
/// - `forward`: Positive forward rate or forward price `F`.
/// - `strike`: Positive strike `K`.
/// - `sigma`: Positive lognormal volatility as an annual decimal.
/// - `t`: Positive expiry in years.
///
/// # Returns
///
/// Returns the Black-76 `d1` term, or `±∞` by moneyness for degenerate inputs
/// (see above).
#[inline]
pub fn d1_black76(forward: f64, strike: f64, sigma: f64, t: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 || forward <= 0.0 || strike <= 0.0 {
        return if forward >= strike {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
    }
    let sqrt_t = t.sqrt();
    ((forward / strike).ln() + 0.5 * sigma * sigma * t) / (sigma * sqrt_t)
}

/// Shifted Black call price with unit annuity.
///
/// Applies [`black_call`] to `forward + shift` and `strike + shift`. The shifted
/// forward and strike must be positive for the lognormal formula; otherwise the
/// underlying Black-76 degenerate-input intrinsic rule applies.
#[inline]
pub fn black_shifted_call(forward: f64, strike: f64, sigma: f64, t: f64, shift: f64) -> f64 {
    black_call(forward + shift, strike + shift, sigma, t)
}

/// Shifted Black put price with unit annuity.
///
/// Applies [`black_put`] to `forward + shift` and `strike + shift` on the same
/// undiscounted unit-annuity scale as the unshifted Black-76 functions.
#[inline]
pub fn black_shifted_put(forward: f64, strike: f64, sigma: f64, t: f64, shift: f64) -> f64 {
    black_put(forward + shift, strike + shift, sigma, t)
}

/// Shifted Black vega with unit annuity.
///
/// Applies [`black_vega`] to `forward + shift` and `strike + shift`.
#[inline]
pub fn black_shifted_vega(forward: f64, strike: f64, sigma: f64, t: f64, shift: f64) -> f64 {
    black_vega(forward + shift, strike + shift, sigma, t)
}
