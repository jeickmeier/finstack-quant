//! Black model helpers for interest rate caplets/floorlets.
//!
//! Exposes pure functions for price and greeks to keep `types.rs` free of pricing logic.
//!
//! # Numerical Hardening
//!
//! This module handles edge cases that can cause numerical issues:
//! - **Zero/negative volatility**: Returns intrinsic value (same as expiry)
//! - **Zero/negative forward**: Returns error (Black model undefined)
//! - **Negative time to fixing**: Returns intrinsic value (option already fixed)
//!
//! # Delta Sign Convention
//!
//! The [`delta`] function returns the **forward delta** (sensitivity to forward rate changes):
//! - **Caplet delta**: Positive, in range \[0, 1\]. A caplet benefits from higher forwards.
//! - **Floorlet delta**: Negative, in range \[-1, 0\]. A floorlet benefits from lower forwards.
//!
//! The delta is computed as:
//! - Caplet: `N(d₁)` where N is the standard normal CDF
//! - Floorlet: `-N(-d₁) = N(d₁) - 1`
//!
//! This is the "per-unit forward delta" convention. When aggregated across periods in
//! `aggregate_over_caplets`,
//! the result is scaled by `notional × accrual × discount_factor`.
//!
//! # References
//!
//! - Black, F. (1976). "The pricing of commodity contracts."
//!   *Journal of Financial Economics*, 3(1-2), 167-179. `docs/REFERENCES.md#black-1976`

use super::payoff::CapletFloorletInputs;
use finstack_quant_core::money::Money;
use finstack_quant_models::closed_form::{
    black_call, black_delta_call, black_delta_put, black_gamma, black_put, black_vega,
};

/// Compute intrinsic value of a caplet/floorlet.
///
/// This is used when the option is at or past expiry, or when volatility is zero/negative.
#[inline]
fn intrinsic_value(inputs: &CapletFloorletInputs) -> f64 {
    let payoff = if inputs.is_cap {
        (inputs.forward - inputs.strike).max(0.0)
    } else {
        (inputs.strike - inputs.forward).max(0.0)
    };
    payoff * inputs.accrual_year_fraction * inputs.notional * inputs.discount_factor
}

/// Price a caplet/floorlet using Black's formula.
///
/// Returns PV in the instrument currency given forward, discount factor, vol, time to fixing and accrual.
///
/// # Edge Case Handling
///
/// - **`t_fix <= 0`**: Option is at or past fixing; returns intrinsic value.
/// - **`sigma <= 0 || !sigma.is_finite()`**: Volatility is invalid; returns intrinsic value.
/// - **`forward <= 0`**: Black model is undefined for non-positive forwards; returns error.
///   For negative rate environments, use a normal (Bachelier) model instead.
/// - **`strike <= 0`**: Technically valid but unusual; log warning and proceed.
///
/// # Returns
///
/// `Ok(Money)` with the caplet/floorlet PV, or `Err` if inputs are invalid.
pub(crate) fn price_caplet_floorlet(
    inputs: CapletFloorletInputs,
) -> finstack_quant_core::Result<Money> {
    let is_cap = inputs.is_cap;
    let notional = inputs.notional;
    let strike = inputs.strike;
    let forward = inputs.forward;
    let df = inputs.discount_factor;
    let sigma = inputs.volatility;
    let t_fix = inputs.time_to_fixing;
    let tau = inputs.accrual_year_fraction;
    let ccy = inputs.currency;

    // Edge case: Option is at or past fixing -> intrinsic value
    if t_fix <= 0.0 {
        return Money::new(intrinsic_value(&inputs), ccy);
    }

    // Edge case: Zero or negative volatility -> intrinsic value
    // This handles the case where vol surface returns 0 or negative due to extrapolation
    if sigma <= 0.0 || !sigma.is_finite() {
        return Money::new(intrinsic_value(&inputs), ccy);
    }

    // Edge case: Non-positive forward rate
    // Black (1976) model requires F > 0 since it uses log(F/K)
    // For negative rate environments, the normal (Bachelier) model should be used
    if forward <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Black model requires positive forward rate; got forward={:.6}. \
             For negative rate environments, use Normal (Bachelier) model.",
            forward
        )));
    }

    // Edge case: Non-positive strike - unusual but technically valid for deep ITM caps
    // The formula still works, but log warning
    if strike <= 0.0 {
        tracing::warn!(
            forward = forward,
            strike = strike,
            "Black caplet pricing with non-positive strike; result may be imprecise"
        );
    }

    // The closed forms return intrinsic value in any degenerate domain
    // (including a non-positive strike), so no separate NaN fallback is needed.
    let unit = if is_cap {
        black_call(forward, strike, sigma, t_fix)
    } else {
        black_put(forward, strike, sigma, t_fix)
    };
    let pv = df * tau * notional * unit;

    // Final sanity check
    if !pv.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Black caplet pricing produced non-finite PV: forward={}, strike={}, sigma={}, t={}",
            forward, strike, sigma, t_fix
        )));
    }

    Money::new(pv, ccy)
}

/// Black forward delta (per unit forward).
///
/// Returns the sensitivity of the option price to changes in the forward rate.
///
/// # Sign Convention
///
/// - **Caplet**: Returns `N(d₁)`, positive in \[0, 1\]. Higher forwards increase caplet value.
/// - **Floorlet**: Returns `-N(-d₁) = N(d₁) - 1`, negative in \[-1, 0\]. Lower forwards increase floorlet value.
///
/// At expiry or with zero volatility, returns the intrinsic delta:
/// - Caplet: 1 if ITM (F > K), 0 if OTM
/// - Floorlet: -1 if ITM (F < K), 0 if OTM
pub(crate) fn delta(is_cap: bool, strike: f64, forward: f64, sigma: f64, t_fix: f64) -> f64 {
    // `forward <= 0.0` makes `ln(F/K)` (hence d1) non-finite; return the
    // intrinsic delta rather than a NaN. Callers that want a finite-vol delta on
    // a non-positive forward should route through the Bachelier fallback (see
    // `common::lognormal_delta_with_fallback`).
    if is_cap {
        black_delta_call(forward, strike, sigma, t_fix)
    } else {
        black_delta_put(forward, strike, sigma, t_fix)
    }
}

/// Black forward gamma (per unit forward).
///
/// Returns the second derivative of option price with respect to forward rate.
/// Gamma is always non-negative for long options.
pub(crate) fn gamma(strike: f64, forward: f64, sigma: f64, t_fix: f64) -> f64 {
    black_gamma(forward, strike, sigma, t_fix)
}

/// Black vega per 1% vol.
///
/// Returns the sensitivity of option price to a 1% (absolute) change in volatility.
/// Vega is always non-negative for long options.
pub(crate) fn vega_per_pct(strike: f64, forward: f64, sigma: f64, t_fix: f64) -> f64 {
    // At `sigma <= 0` the true Black vega is zero away from the money and only
    // finite exactly at-the-money; a degenerate (extrapolated) zero vol should
    // report zero vega rather than the `n(0)` value an ATM-equivalent d1 would
    // give, which would overstate vega for ITM/OTM strikes.
    black_vega(forward, strike, sigma, t_fix) / 100.0
}
