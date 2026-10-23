//! Bachelier (normal) model helpers for interest rate caplets/floorlets.
//!
//! This pricing path is used when cap/floor volatility is quoted in normal terms
//! (absolute rate units), which is common in low/negative-rate environments.

use super::CapletFloorletInputs;
use finstack_quant_core::money::Money;
use finstack_quant_models::closed_form::bachelier_price_with_annuity;
use finstack_quant_models::closed_form::{bachelier_delta_call, bachelier_delta_put};
use finstack_quant_models::types::OptionType;

/// Price a caplet/floorlet using Bachelier's normal model.
pub(crate) fn price_caplet_floorlet(
    inputs: CapletFloorletInputs,
) -> finstack_quant_core::Result<Money> {
    let is_cap = inputs.is_cap;
    let notional = inputs.notional;
    let strike = inputs.strike;
    let forward = inputs.forward;
    let df = inputs.discount_factor;
    let sigma =
        finstack_quant_valuations_core::instruments::common_impl::vol_resolution::validate_sigma(
            inputs.sigma,
        )?;
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

    // For near-zero vol, return intrinsic to avoid unstable Bachelier numerics.
    const MIN_NORMAL_VOL: f64 = 1e-8;
    if sigma < MIN_NORMAL_VOL {
        let intrinsic = if is_cap {
            (forward - strike).max(0.0)
        } else {
            (strike - forward).max(0.0)
        };
        return Money::new(intrinsic * annuity, ccy);
    }

    let pv =
        bachelier_price_with_annuity(option_type, forward, strike, sigma, t_fix.max(0.0), annuity);

    if !pv.is_finite() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Normal cap/floor pricing produced non-finite PV: forward={}, strike={}, sigma={}, t={}",
            forward, strike, sigma, t_fix
        )));
    }

    Money::new(pv, ccy)
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
/// - Brigo, D., & Mercurio, F. (2006). *Interest Rate Models*, Ch. 1. `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`
pub(crate) fn delta(is_cap: bool, strike: f64, forward: f64, sigma: f64, t_fix: f64) -> f64 {
    if is_cap {
        bachelier_delta_call(forward, strike, sigma, t_fix)
    } else {
        bachelier_delta_put(forward, strike, sigma, t_fix)
    }
}
