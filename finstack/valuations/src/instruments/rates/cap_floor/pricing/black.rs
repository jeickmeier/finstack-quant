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
//! [`aggregate_over_caplets`](super::super::metrics::common::aggregate_over_caplets),
//! the result is scaled by `notional × accrual × discount_factor`.
//!
//! # References
//!
//! - Black, F. (1976). "The pricing of commodity contracts."
//!   *Journal of Financial Economics*, 3(1-2), 167-179.

use super::payoff::CapletFloorletInputs;
use crate::models::{d1_black76, d1_d2_black76};
use finstack_core::math::{norm_cdf, norm_pdf};
use finstack_core::money::Money;

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
pub(crate) fn price_caplet_floorlet(inputs: CapletFloorletInputs) -> finstack_core::Result<Money> {
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
        return Ok(Money::new(intrinsic_value(&inputs), ccy));
    }

    // Edge case: Zero or negative volatility -> intrinsic value
    // This handles the case where vol surface returns 0 or negative due to extrapolation
    if sigma <= 0.0 || !sigma.is_finite() {
        return Ok(Money::new(intrinsic_value(&inputs), ccy));
    }

    // Edge case: Non-positive forward rate
    // Black (1976) model requires F > 0 since it uses log(F/K)
    // For negative rate environments, the normal (Bachelier) model should be used
    if forward <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
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

    let (d1, d2) = d1_d2_black76(forward, strike, sigma, t_fix);

    // Check for NaN in d1/d2 which can happen with extreme inputs
    if !d1.is_finite() || !d2.is_finite() {
        // Fall back to intrinsic if Black formula produces NaN
        tracing::warn!(
            forward = forward,
            strike = strike,
            sigma = sigma,
            t_fix = t_fix,
            d1 = d1,
            d2 = d2,
            "Black caplet d1/d2 are non-finite; falling back to intrinsic value"
        );
        return Ok(Money::new(intrinsic_value(&inputs), ccy));
    }

    let pv = if is_cap {
        df * tau * notional * (forward * norm_cdf(d1) - strike * norm_cdf(d2))
    } else {
        df * tau * notional * (strike * norm_cdf(-d2) - forward * norm_cdf(-d1))
    };

    // Final sanity check
    if !pv.is_finite() {
        return Err(finstack_core::Error::Validation(format!(
            "Black caplet pricing produced non-finite PV: forward={}, strike={}, sigma={}, t={}",
            forward, strike, sigma, t_fix
        )));
    }

    Ok(Money::new(pv, ccy))
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
    if t_fix <= 0.0 || sigma <= 0.0 {
        if is_cap {
            return if forward > strike { 1.0 } else { 0.0 };
        } else {
            return if forward < strike { -1.0 } else { 0.0 };
        }
    }
    let d1 = d1_black76(forward, strike, sigma, t_fix);
    if is_cap {
        norm_cdf(d1)
    } else {
        -norm_cdf(-d1)
    }
}

/// Black forward gamma (per unit forward).
///
/// Returns the second derivative of option price with respect to forward rate.
/// Gamma is always non-negative for long options.
pub(crate) fn gamma(strike: f64, forward: f64, sigma: f64, t_fix: f64) -> f64 {
    if t_fix <= 0.0 || sigma <= 0.0 || forward <= 0.0 {
        return 0.0;
    }
    let d1 = d1_black76(forward, strike, sigma, t_fix);
    let denom = (forward * sigma * t_fix.sqrt()).max(1e-12);
    norm_pdf(d1) / denom
}

/// Black vega per 1% vol.
///
/// Returns the sensitivity of option price to a 1% (absolute) change in volatility.
/// Vega is always non-negative for long options.
pub(crate) fn vega_per_pct(strike: f64, forward: f64, sigma: f64, t_fix: f64) -> f64 {
    if t_fix <= 0.0 || forward <= 0.0 {
        return 0.0;
    }
    let d1 = if sigma > 0.0 {
        d1_black76(forward, strike, sigma, t_fix)
    } else {
        0.0
    };
    forward * norm_pdf(d1) * t_fix.sqrt() / 100.0
}
