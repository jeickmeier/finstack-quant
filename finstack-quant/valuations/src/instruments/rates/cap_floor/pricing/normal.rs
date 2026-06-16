//! Bachelier (normal) model helpers for interest rate caplets/floorlets.
//!
//! This pricing path is used when cap/floor volatility is quoted in normal terms
//! (absolute rate units), which is common in low/negative-rate environments.

use super::payoff::CapletFloorletInputs;
use crate::instruments::common_impl::parameters::OptionType;
use crate::models::volatility::normal::{bachelier_price, d_bachelier};
use finstack_quant_core::math::{norm_cdf, norm_pdf};
use finstack_quant_core::money::Money;

/// Price a caplet/floorlet using Bachelier's normal model.
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

    if !forward.is_finite()
        || !strike.is_finite()
        || !sigma.is_finite()
        || !t_fix.is_finite()
        || !tau.is_finite()
        || !df.is_finite()
        || !notional.is_finite()
    {
        return Err(finstack_quant_core::Error::Validation(
            "Normal cap/floor pricing received non-finite input".to_string(),
        ));
    }

    if tau < 0.0 || df < 0.0 || notional < 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "Normal cap/floor pricing requires non-negative notional, discount factor, and accrual"
                .to_string(),
        ));
    }

    let annuity = notional * tau * df;
    let option_type = if is_cap {
        OptionType::Call
    } else {
        OptionType::Put
    };

    // For near-zero/invalid vol, return intrinsic to avoid unstable Bachelier numerics.
    const MIN_NORMAL_VOL: f64 = 1e-8;
    let effective_sigma = if sigma.is_finite() { sigma } else { 0.0 };
    if effective_sigma < MIN_NORMAL_VOL {
        let intrinsic = if is_cap {
            (forward - strike).max(0.0)
        } else {
            (strike - forward).max(0.0)
        };
        return Ok(Money::new(intrinsic * annuity, ccy));
    }

    let pv = bachelier_price(
        option_type,
        forward,
        strike,
        effective_sigma.max(MIN_NORMAL_VOL),
        t_fix.max(0.0),
        annuity,
    );

    if !pv.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Normal cap/floor pricing produced non-finite PV: forward={}, strike={}, sigma={}, t={}",
            forward, strike, sigma, t_fix
        )));
    }

    Ok(Money::new(pv, ccy))
}

/// Bachelier forward delta (per unit forward).
///
/// Returns the sensitivity of the option price to changes in the forward rate.
///
/// # Sign Convention
///
/// - **Caplet**: Returns `N(d)`, positive in \[0, 1\].
/// - **Floorlet**: Returns `N(d) - 1 = -N(-d)`, negative in \[-1, 0\].
///
/// At expiry or with zero volatility, returns the intrinsic delta:
/// - Caplet: 1 if ITM (F > K), 0 if OTM
/// - Floorlet: -1 if ITM (F < K), 0 if OTM
///
/// # References
///
/// - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models*, Ch. 1.
pub(crate) fn delta(is_cap: bool, strike: f64, forward: f64, sigma: f64, t_fix: f64) -> f64 {
    if t_fix <= 0.0 || sigma <= 0.0 {
        if is_cap {
            return if forward > strike { 1.0 } else { 0.0 };
        } else {
            return if forward < strike { -1.0 } else { 0.0 };
        }
    }
    let d = d_bachelier(forward, strike, sigma, t_fix);
    if is_cap {
        norm_cdf(d)
    } else {
        -norm_cdf(-d)
    }
}

/// Bachelier forward gamma (per unit forward).
///
/// Returns the second derivative of option price with respect to forward rate.
/// Gamma is always non-negative for long options.
///
/// Gamma = n(d) / (σ√T)
pub(crate) fn gamma(strike: f64, forward: f64, sigma: f64, t_fix: f64) -> f64 {
    if t_fix <= 0.0 || sigma <= 0.0 {
        return 0.0;
    }
    let d = d_bachelier(forward, strike, sigma, t_fix);
    let denom = (sigma * t_fix.sqrt()).max(1e-12);
    norm_pdf(d) / denom
}

/// Bachelier vega per 1% normal vol.
///
/// Returns the sensitivity of option price to a 1% (absolute) change in normal volatility.
/// Vega is always non-negative for long options.
///
/// Vega = √T · n(d) / 100
pub(crate) fn vega_per_pct(strike: f64, forward: f64, sigma: f64, t_fix: f64) -> f64 {
    // A degenerate (extrapolated) zero/negative vol reports zero vega rather
    // than the `n(0)` value an ATM-equivalent `d` would give.
    if t_fix <= 0.0 || sigma <= 0.0 {
        return 0.0;
    }
    let d = d_bachelier(forward, strike, sigma, t_fix);
    norm_pdf(d) * t_fix.sqrt() / 100.0
}
