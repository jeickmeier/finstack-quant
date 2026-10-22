//! Bachelier normal option pricing formulas and Greeks.
//!
//! Functions take a forward `F`, strike `K`, normal volatility `sigma_n`, and
//! expiry `t` in years. They return undiscounted values on a unit annuity or
//! unit discount factor; callers multiply by the relevant annuity, discount
//! factor, notional, or PV01 outside this module.
//!
//! Boundary inputs with `t == 0` or `sigma_n == 0` return intrinsic value for
//! prices and zero or digital-limit values for Greeks. Unlike Black-76, the
//! forward and strike may be negative because the normal model supports
//! negative rates and prices. Non-finite inputs, negative volatility, or
//! negative expiry return `NaN`; checked entrypoints return validation errors.
//!
//! # References
//!
//! - Bachelier, L. (1900), "Theorie de la speculation". `docs/REFERENCES.md#bachelier-1900`
//!
//! - Hull, J. C., *Options, Futures, and Other Derivatives*. `docs/REFERENCES.md#hull-options-futures`
//!

use finstack_quant_core::math::special_functions::{norm_cdf, norm_pdf};

#[derive(Clone, Copy, Debug)]
struct BachelierState {
    st: f64,
    d: f64,
}

#[inline]
fn valid_bachelier_inputs(forward: f64, strike: f64, sigma_n: f64, t: f64) -> bool {
    [forward, strike, sigma_n, t].iter().all(|x| x.is_finite()) && sigma_n >= 0.0 && t >= 0.0
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
/// annuity, discount factor, or notional. At zero expiry or volatility returns
/// intrinsic value `(forward - strike).max(0.0)`. Invalid inputs return `NaN`.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::closed_form::{bachelier_call, bachelier_put};
///
/// let call = bachelier_call(0.03, 0.025, 0.005, 2.0);
/// let put = bachelier_put(0.03, 0.025, 0.005, 2.0);
/// assert!((call - put - 0.005).abs() < 1e-12);
/// ```
pub fn bachelier_call(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    if !valid_bachelier_inputs(forward, strike, sigma_n, t) {
        return f64::NAN;
    }
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
/// annuity, discount factor, or notional. At zero expiry or volatility returns
/// intrinsic value `(strike - forward).max(0.0)`. Invalid inputs return `NaN`.
pub fn bachelier_put(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    if !valid_bachelier_inputs(forward, strike, sigma_n, t) {
        return f64::NAN;
    }
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
/// - `strike`: Exercise rate or price, expressed in the same units as
///   `forward`.
/// - `sigma_n`: Normal volatility.
/// - `t`: Expiry in years.
///
/// # Returns
///
/// Returns `sqrt(T) n(d)` on the same unit-annuity scale as
/// [`bachelier_call`]. Returns `0.0` at zero expiry or volatility and `NaN`
/// for invalid inputs.
pub fn bachelier_vega(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    if !valid_bachelier_inputs(forward, strike, sigma_n, t) {
        return f64::NAN;
    }
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
/// `forward >= strike`, otherwise `0.0`. Invalid inputs return `NaN`.
///
/// # Arguments
///
/// * `forward` - Forward rate or forward price on the unit-annuity pricing
///   scale.
/// * `strike` - Option strike in the same units as `forward`.
/// * `sigma_n` - Annualized normal volatility in forward-price units, not a
///   lognormal percentage volatility.
/// * `t` - Time to expiry in years.
pub fn bachelier_delta_call(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    if !valid_bachelier_inputs(forward, strike, sigma_n, t) {
        return f64::NAN;
    }
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
///
/// # Arguments
///
/// * `forward` - Forward rate or forward price on the unit-annuity pricing
///   scale.
/// * `strike` - Option strike in the same units as `forward`.
/// * `sigma_n` - Annualized normal volatility in forward-price units, not a
///   lognormal percentage volatility.
/// * `t` - Time to expiry in years.
pub fn bachelier_delta_put(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    if !valid_bachelier_inputs(forward, strike, sigma_n, t) {
        return f64::NAN;
    }
    match bachelier_state(forward, strike, sigma_n, t) {
        Some(state) => -norm_cdf(-state.d),
        None => bachelier_delta_call(forward, strike, sigma_n, t) - 1.0,
    }
}

/// Bachelier gamma with respect to the forward.
///
/// # Returns
///
/// Returns `n(d) / (sigma_n sqrt(T))` in the valid normal-model domain and
/// `0.0` at zero expiry or volatility. Invalid inputs return `NaN`.
///
/// # Arguments
///
/// * `forward` - Forward rate or forward price on the unit-annuity pricing
///   scale.
/// * `strike` - Option strike in the same units as `forward`.
/// * `sigma_n` - Annualized normal volatility in forward-price units, not a
///   lognormal percentage volatility.
/// * `t` - Time to expiry in years.
pub fn bachelier_gamma(forward: f64, strike: f64, sigma_n: f64, t: f64) -> f64 {
    if !valid_bachelier_inputs(forward, strike, sigma_n, t) {
        return f64::NAN;
    }
    match bachelier_state(forward, strike, sigma_n, t) {
        Some(state) => norm_pdf(state.d) / state.st,
        None => 0.0,
    }
}

/// Bachelier (Normal) model price scaled by an annuity (unchecked).
///
/// The canonical checked unit-annuity price the host bindings expose is
/// [`crate::closed_form::bachelier_price`]; this variant multiplies by a
/// caller-supplied annuity and collapses degenerate inputs to intrinsic value.
///
/// # Arguments
/// * `option_type` - Call (payer) or Put (receiver)
/// * `forward` - Forward rate or price at expiry, in the same units as the
///   strike and normal volatility.
/// * `strike` - Exercise rate or price in the same units as `forward`.
/// * `sigma` - Normal volatility (in rate terms, not percentage)
/// * `t` - Time to expiry in years
/// * `annuity` - Present value of 1bp running (sum of discount factors × accrual fractions)
///
/// # Returns
/// Option premium in the same units as annuity (typically currency units)
#[inline]
#[must_use]
pub fn bachelier_price_with_annuity(
    option_type: crate::types::OptionType,
    forward: f64,
    strike: f64,
    sigma: f64,
    t: f64,
    annuity: f64,
) -> f64 {
    // Degenerate inputs (`t <= 0` or `sigma <= 0`) collapse to intrinsic value
    // inside the unit-annuity kernels.
    let unit_price = match option_type {
        crate::types::OptionType::Call => bachelier_call(forward, strike, sigma, t),
        crate::types::OptionType::Put => bachelier_put(forward, strike, sigma, t),
    };
    annuity * unit_price
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn test_bachelier_put_call_parity() {
        // Put-Call parity: Call - Put = F - K
        let forward = 0.03;
        let strike = 0.025;
        let sigma = 0.005;
        let t = 2.0;

        let call = bachelier_call(forward, strike, sigma, t);
        let put = bachelier_put(forward, strike, sigma, t);

        let parity_diff = call - put - (forward - strike);
        assert!(
            parity_diff.abs() < EPSILON,
            "Bachelier put-call parity violated: diff = {}",
            parity_diff
        );
    }

    #[test]
    fn test_bachelier_atm_symmetry() {
        // ATM: Call = Put when F = K
        let forward = 0.03;
        let strike = 0.03; // ATM
        let sigma = 0.005;
        let t = 1.0;

        let call = bachelier_call(forward, strike, sigma, t);
        let put = bachelier_put(forward, strike, sigma, t);

        assert!(
            (call - put).abs() < EPSILON,
            "Bachelier ATM call != put: {} vs {}",
            call,
            put
        );
    }

    #[test]
    fn test_bachelier_vega_positive() {
        let forward = 0.03;
        let strike = 0.025;
        let sigma = 0.005;
        let t = 1.0;

        let vega = bachelier_vega(forward, strike, sigma, t);
        assert!(vega > 0.0, "Bachelier vega should be positive");
    }

    #[test]
    fn test_bachelier_delta_bounds() {
        let forward = 0.03;
        let sigma = 0.005;
        let t = 1.0;

        let delta_itm = bachelier_delta_call(forward, 0.01, sigma, t);
        assert!(delta_itm > 0.9, "ITM call delta should be close to 1");

        let delta_otm = bachelier_delta_call(forward, 0.05, sigma, t);
        assert!(delta_otm < 0.1, "OTM call delta should be close to 0");

        let delta_atm = bachelier_delta_call(forward, forward, sigma, t);
        assert!(
            (delta_atm - 0.5).abs() < 0.01,
            "ATM call delta should be ~0.5"
        );
    }

    #[test]
    fn test_bachelier_gamma_positive() {
        let forward = 0.03;
        let strike = 0.025;
        let sigma = 0.005;
        let t = 1.0;

        let gamma = bachelier_gamma(forward, strike, sigma, t);
        assert!(gamma > 0.0, "Bachelier gamma should be positive");

        let gamma_atm = bachelier_gamma(forward, forward, sigma, t);
        let gamma_otm = bachelier_gamma(forward, forward + 0.02, sigma, t);
        assert!(
            gamma_atm > gamma_otm,
            "ATM gamma should be higher than OTM gamma"
        );
    }
}
