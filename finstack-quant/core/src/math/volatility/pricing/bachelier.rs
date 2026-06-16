//! Bachelier normal option pricing formulas and Greeks.
//!
//! Functions take a forward `F`, strike `K`, normal volatility `sigma_n`, and
//! expiry `t` in years. They return undiscounted values on a unit annuity or
//! unit discount factor; callers multiply by the relevant annuity, discount
//! factor, notional, or PV01 outside this module.
//!
//! Degenerate inputs with `t <= 0` or `sigma_n <= 0` return intrinsic value for
//! prices and zero or digital-limit values for Greeks. Unlike Black-76, the
//! forward and strike may be negative because the normal model supports
//! negative rates and prices.
//!
//! # References
//!
//! - Bachelier, L. (1900), "Theorie de la speculation".
//! - Hull, J. C., *Options, Futures, and Other Derivatives*.

use crate::math::{norm_cdf, norm_pdf};

#[derive(Clone, Copy, Debug)]
struct BachelierState {
    st: f64,
    d: f64,
}

#[inline]
fn bachelier_state(forward: f64, strike: f64, sigma_n: f64, t: f64) -> Option<BachelierState> {
    if t <= 0.0 || sigma_n <= 0.0 {
        return None;
    }

    let st = sigma_n * t.sqrt();
    Some(BachelierState {
        st,
        d: (forward - strike) / st,
    })
}

/// Bachelier normal call price with unit annuity.
///
/// # Arguments
///
/// - `forward`: Forward rate or forward price `F`; may be negative.
/// - `strike`: Strike `K`; may be negative.
/// - `sigma_n`: Normal volatility in price or rate units per square-root year.
/// - `t`: Expiry in years.
///
/// # Returns
///
/// Returns `(F - K) N(d) + sigma_n sqrt(T) n(d)` before multiplying by any
/// annuity, discount factor, or notional. If `t <= 0` or `sigma_n <= 0`, returns
/// intrinsic value `(forward - strike).max(0.0)`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::volatility::{bachelier_call, bachelier_put};
///
/// let call = bachelier_call(0.03, 0.025, 0.005, 2.0);
/// let put = bachelier_put(0.03, 0.025, 0.005, 2.0);
/// assert!((call - put - 0.005).abs() < 1e-12);
/// ```
pub fn bachelier_call(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    match bachelier_state(forward, strike, sigma_n, t) {
        Some(state) => (forward - strike) * norm_cdf(state.d) + state.st * norm_pdf(state.d),
        None => (forward - strike).max(0.0),
    }
}

/// Bachelier normal put price with unit annuity.
///
/// # Arguments
///
/// - `forward`: Forward rate or forward price `F`; may be negative.
/// - `strike`: Strike `K`; may be negative.
/// - `sigma_n`: Normal volatility in price or rate units per square-root year.
/// - `t`: Expiry in years.
///
/// # Returns
///
/// Returns `(K - F) N(-d) + sigma_n sqrt(T) n(d)` before multiplying by any
/// annuity, discount factor, or notional. If `t <= 0` or `sigma_n <= 0`, returns
/// intrinsic value `(strike - forward).max(0.0)`.
pub fn bachelier_put(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    match bachelier_state(forward, strike, sigma_n, t) {
        Some(state) => (strike - forward) * norm_cdf(-state.d) + state.st * norm_pdf(state.d),
        None => (strike - forward).max(0.0),
    }
}

/// Bachelier vega with respect to normal volatility.
///
/// # Arguments
///
/// - `forward`: Forward rate or forward price.
/// - `strike`: Strike.
/// - `sigma_n`: Normal volatility.
/// - `t`: Expiry in years.
///
/// # Returns
///
/// Returns `sqrt(T) n(d)` on the same unit-annuity scale as
/// [`bachelier_call`]. Returns `0.0` for degenerate expiry or volatility.
pub fn bachelier_vega(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    match bachelier_state(forward, strike, sigma_n, t) {
        Some(state) => t.sqrt() * norm_pdf(state.d),
        None => 0.0,
    }
}

/// Bachelier call delta with respect to the forward.
///
/// # Returns
///
/// Returns `N(d)` in the valid normal-model domain. At zero expiry or zero
/// normal volatility, returns the intrinsic digital limit: `1.0` when
/// `forward >= strike`, otherwise `0.0`.
pub fn bachelier_delta_call(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    match bachelier_state(forward, strike, sigma_n, t) {
        Some(state) => norm_cdf(state.d),
        None => {
            if forward >= strike {
                1.0
            } else {
                0.0
            }
        }
    }
}

/// Bachelier put delta with respect to the forward.
///
/// # Returns
///
/// Returns call delta minus one. This is the forward delta of the undiscounted
/// unit-annuity Bachelier put.
pub fn bachelier_delta_put(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    bachelier_delta_call(forward, strike, sigma_n, t) - 1.0
}

/// Bachelier gamma with respect to the forward.
///
/// # Returns
///
/// Returns `n(d) / (sigma_n sqrt(T))` in the valid normal-model domain and
/// `0.0` for degenerate expiry or volatility.
pub fn bachelier_gamma(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    match bachelier_state(forward, strike, sigma_n, t) {
        Some(state) => norm_pdf(state.d) / state.st,
        None => 0.0,
    }
}
