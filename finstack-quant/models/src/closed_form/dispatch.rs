//! Canonical string-keyed dispatch for the closed-form exotic pricers.
//!
//! Host bindings (Python, WASM) expose the barrier / Asian / lookback / quanto
//! closed forms behind compact string selectors (`direction`/`knock`,
//! `averaging`, `strike_type`) plus a call/put flag. This module is the single
//! home for that selector parsing and routing so both hosts share one match
//! table and one error message; the leaf formulas in [`super::barrier`],
//! [`super::asian`], [`super::lookback`], and [`super::quanto`] are unchanged.
//!
//! Every dispatcher finishes through [`checked_closed_form_value`], so a
//! non-finite price from a degenerate input surfaces as a validation error
//! instead of silently crossing a host boundary.
//!
//! # Examples
//! ```rust
//! use finstack_quant_models::OptionType;
//! use finstack_quant_models::closed_form::dispatch::{
//!     asian_option_price, barrier_call, barrier_put,
//! };
//!
//! let barrier = barrier_call(100.0, 100.0, 90.0, 0.05, 0.02, 0.20, 1.0, "down", "out")?;
//! assert!(barrier > 0.0);
//! let barrier_put = barrier_put(100.0, 100.0, 90.0, 0.05, 0.02, 0.20, 1.0, "down", "out")?;
//! assert!(barrier_put >= 0.0);
//!
//! let asian = asian_option_price(
//!     100.0, 100.0, 0.05, 0.02, 0.20, 1.0, 12, "arithmetic", OptionType::Call,
//! )?;
//! assert!(asian > 0.0);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

use finstack_quant_core::{Error, Result};

use super::asian::{
    arithmetic_asian_call_tw, arithmetic_asian_put_tw, geometric_asian_call, geometric_asian_put,
};
use super::barrier::{barrier_price, BarrierParams};
use super::lookback::{
    fixed_strike_lookback_call, fixed_strike_lookback_put, floating_strike_lookback_call,
    floating_strike_lookback_put,
};
use super::quanto::{quanto_call, quanto_put};
use super::vanilla::checked_closed_form_value;
use crate::types::OptionType;
use finstack_quant_core::types::BarrierType;

/// Averaging convention host bindings use when `averaging` is omitted from
/// [`asian_option_price`]: `"arithmetic"` (Turnbull-Wakeman approximation).
pub const DEFAULT_ASIAN_AVERAGING: &str = "arithmetic";

/// Strike convention host bindings use when `strike_type` is omitted from
/// [`lookback_option_price`]: `"fixed"`.
pub const DEFAULT_LOOKBACK_STRIKE_TYPE: &str = "fixed";

/// Parse a `(direction, knock)` string pair into a [`BarrierType`].
fn parse_barrier_type(direction: &str, knock: &str) -> Result<BarrierType> {
    match (direction, knock) {
        ("up", "in") => Ok(BarrierType::UpAndIn),
        ("up", "out") => Ok(BarrierType::UpAndOut),
        ("down", "in") => Ok(BarrierType::DownAndIn),
        ("down", "out") => Ok(BarrierType::DownAndOut),
        _ => Err(Error::Validation(format!(
            "unknown barrier spec: direction='{direction}' knock='{knock}'; \
             expected direction in {{'up','down'}} and knock in {{'in','out'}}"
        ))),
    }
}

/// Reiner-Rubinstein continuous-monitoring barrier call, selected by strings.
///
/// Parses the `(direction, knock)` pair into a [`BarrierType`] and calls
/// [`barrier_price`] with [`OptionType::Call`].
///
/// # Arguments
///
/// * `spot` - Current spot price of the underlying.
/// * `strike` - Option strike price in the same units as `spot`.
/// * `barrier` - Continuously monitored barrier level in the same units as `spot`.
/// * `rate` - Risk-free rate, continuously compounded decimal.
/// * `div_yield` - Continuous dividend yield (or foreign rate for FX), decimal.
/// * `vol` - Annualized volatility, decimal.
/// * `expiry` - Time to expiry in years.
/// * `direction` - `"up"` for an upper barrier or `"down"` for a lower barrier.
/// * `knock` - `"in"` for knock-in or `"out"` for knock-out.
///
/// # Errors
///
/// Returns `Error::Validation` if `(direction, knock)` is not a supported
/// pair, or if the resulting barrier price is non-finite.
#[allow(clippy::too_many_arguments)]
pub fn barrier_call(
    spot: f64,
    strike: f64,
    barrier: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    direction: &str,
    knock: &str,
) -> Result<f64> {
    let params = BarrierParams::new(spot, strike, barrier, expiry, rate, div_yield, vol);
    let value = barrier_price(
        &params,
        parse_barrier_type(direction, knock)?,
        OptionType::Call,
    );
    checked_closed_form_value(value, "barrier price")
}

/// Reiner-Rubinstein continuous-monitoring barrier put, selected by strings.
///
/// Parses the `(direction, knock)` pair into a [`BarrierType`] and calls
/// [`barrier_price`] with [`OptionType::Put`], mirroring [`barrier_call`].
///
/// # Arguments
///
/// * `spot` - Current spot price of the underlying.
/// * `strike` - Option strike price in the same units as `spot`.
/// * `barrier` - Continuously monitored barrier level in the same units as `spot`.
/// * `rate` - Risk-free rate, continuously compounded decimal.
/// * `div_yield` - Continuous dividend yield (or foreign rate for FX), decimal.
/// * `vol` - Annualized volatility, decimal.
/// * `expiry` - Time to expiry in years.
/// * `direction` - `"up"` for an upper barrier or `"down"` for a lower barrier.
/// * `knock` - `"in"` for knock-in or `"out"` for knock-out.
///
/// # Errors
///
/// Returns `Error::Validation` if `(direction, knock)` is not a supported
/// pair, or if the resulting barrier price is non-finite.
#[allow(clippy::too_many_arguments)]
pub fn barrier_put(
    spot: f64,
    strike: f64,
    barrier: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    direction: &str,
    knock: &str,
) -> Result<f64> {
    let params = BarrierParams::new(spot, strike, barrier, expiry, rate, div_yield, vol);
    let value = barrier_price(
        &params,
        parse_barrier_type(direction, knock)?,
        OptionType::Put,
    );
    checked_closed_form_value(value, "barrier price")
}

/// Asian option price, selected by averaging convention.
///
/// Routes to the Turnbull-Wakeman arithmetic approximation
/// ([`arithmetic_asian_call_tw`] / [`arithmetic_asian_put_tw`]) or the exact
/// Kemna-Vorst geometric closed form ([`geometric_asian_call`] /
/// [`geometric_asian_put`]).
///
/// # Arguments
///
/// * `spot` - Current spot price of the underlying.
/// * `strike` - Option strike price in the same units as `spot`.
/// * `rate` - Risk-free rate, continuously compounded decimal.
/// * `div_yield` - Continuous dividend yield, decimal.
/// * `vol` - Annualized volatility, decimal.
/// * `expiry` - Time to expiry in years.
/// * `num_fixings` - Positive number of equally spaced averaging observations.
/// * `averaging` - `"arithmetic"` (Turnbull-Wakeman) or `"geometric"` (Kemna-Vorst).
/// * `option_type` - Call or put payoff convention.
///
/// # Errors
///
/// Returns `Error::Validation` if `num_fixings` is zero, `averaging` is not a
/// supported convention, or the resulting option price is non-finite.
#[allow(clippy::too_many_arguments)]
pub fn asian_option_price(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    num_fixings: usize,
    averaging: &str,
    option_type: OptionType,
) -> Result<f64> {
    if num_fixings == 0 {
        return Err(Error::Validation(
            "asian option num_fixings must be positive".to_string(),
        ));
    }
    let value = match (averaging, option_type) {
        ("arithmetic", OptionType::Call) => {
            arithmetic_asian_call_tw(spot, strike, expiry, rate, div_yield, vol, num_fixings)
        }
        ("arithmetic", OptionType::Put) => {
            arithmetic_asian_put_tw(spot, strike, expiry, rate, div_yield, vol, num_fixings)
        }
        ("geometric", OptionType::Call) => {
            geometric_asian_call(spot, strike, expiry, rate, div_yield, vol, num_fixings)
        }
        ("geometric", OptionType::Put) => {
            geometric_asian_put(spot, strike, expiry, rate, div_yield, vol, num_fixings)
        }
        _ => {
            return Err(Error::Validation(format!(
                "unknown averaging '{averaging}'; expected 'arithmetic' or 'geometric'"
            )));
        }
    };
    checked_closed_form_value(value, "asian option price")
}

/// Conze-Viswanathan lookback option price, selected by strike convention.
///
/// Routes to [`fixed_strike_lookback_call`] / [`fixed_strike_lookback_put`]
/// or [`floating_strike_lookback_call`] / [`floating_strike_lookback_put`].
/// For `"floating"`, `strike` is ignored by the underlying formula.
///
/// # Arguments
///
/// * `spot` - Current spot price of the underlying.
/// * `strike` - Option strike price (ignored for `"floating"`).
/// * `rate` - Risk-free rate, continuously compounded decimal.
/// * `div_yield` - Continuous dividend yield, decimal.
/// * `vol` - Annualized volatility, decimal.
/// * `expiry` - Time to expiry in years.
/// * `extremum` - Observed running extremum to date, in `spot` units.
/// * `strike_type` - `"fixed"` or `"floating"`.
/// * `option_type` - Call or put payoff convention.
///
/// # Errors
///
/// Returns `Error::Validation` if `strike_type` is not a supported
/// convention, or if the resulting option price is non-finite.
#[allow(clippy::too_many_arguments)]
pub fn lookback_option_price(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    vol: f64,
    expiry: f64,
    extremum: f64,
    strike_type: &str,
    option_type: OptionType,
) -> Result<f64> {
    let value = match (strike_type, option_type) {
        ("fixed", OptionType::Call) => {
            fixed_strike_lookback_call(spot, strike, expiry, rate, div_yield, vol, extremum)
        }
        ("fixed", OptionType::Put) => {
            fixed_strike_lookback_put(spot, strike, expiry, rate, div_yield, vol, extremum)
        }
        ("floating", OptionType::Call) => {
            floating_strike_lookback_call(spot, expiry, rate, div_yield, vol, extremum)
        }
        ("floating", OptionType::Put) => {
            floating_strike_lookback_put(spot, expiry, rate, div_yield, vol, extremum)
        }
        _ => {
            return Err(Error::Validation(format!(
                "unknown strike_type '{strike_type}'; expected 'fixed' or 'floating'"
            )));
        }
    };
    checked_closed_form_value(value, "lookback option price")
}

/// Quanto option price in domestic currency, finiteness-checked.
///
/// Routes to [`quanto_call`] or [`quanto_put`] and rejects non-finite
/// results, so host bindings share one call/put branch and one guard.
///
/// # Arguments
///
/// * `spot` - Spot price of the foreign asset in foreign currency.
/// * `strike` - Strike in foreign currency.
/// * `expiry` - Time to expiry in years.
/// * `rate_domestic` - Domestic risk-free rate, continuously compounded decimal.
/// * `rate_foreign` - Foreign risk-free rate, continuously compounded decimal.
/// * `div_yield` - Foreign asset continuous dividend yield, decimal.
/// * `vol_asset` - Annualized foreign-asset volatility, decimal.
/// * `vol_fx` - Annualized FX-rate volatility, decimal.
/// * `correlation` - Correlation between asset and FX returns, in `[-1, 1]`.
/// * `option_type` - Call or put payoff convention.
///
/// # Errors
///
/// Returns `Error::Validation` if the resulting option price is non-finite.
#[allow(clippy::too_many_arguments)]
pub fn quanto_option_price(
    spot: f64,
    strike: f64,
    expiry: f64,
    rate_domestic: f64,
    rate_foreign: f64,
    div_yield: f64,
    vol_asset: f64,
    vol_fx: f64,
    correlation: f64,
    option_type: OptionType,
) -> Result<f64> {
    let value = match option_type {
        OptionType::Call => quanto_call(
            spot,
            strike,
            expiry,
            rate_domestic,
            rate_foreign,
            div_yield,
            vol_asset,
            vol_fx,
            correlation,
        ),
        OptionType::Put => quanto_put(
            spot,
            strike,
            expiry,
            rate_domestic,
            rate_foreign,
            div_yield,
            vol_asset,
            vol_fx,
            correlation,
        ),
    };
    checked_closed_form_value(value, "quanto option price")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barrier_dispatch_matches_leaf_functions() {
        let (s, k, b, t, r, q, sigma) = (100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.20);
        assert_eq!(
            barrier_call(s, k, b, r, q, sigma, t, "down", "out").unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b, t, r, q, sigma),
                BarrierType::DownAndOut,
                OptionType::Call
            )
        );
        assert_eq!(
            barrier_call(s, k, b, r, q, sigma, t, "down", "in").unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b, t, r, q, sigma),
                BarrierType::DownAndIn,
                OptionType::Call
            )
        );
        let b_up = 120.0;
        assert_eq!(
            barrier_call(s, k, b_up, r, q, sigma, t, "up", "out").unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b_up, t, r, q, sigma),
                BarrierType::UpAndOut,
                OptionType::Call
            )
        );
        assert_eq!(
            barrier_call(s, k, b_up, r, q, sigma, t, "up", "in").unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b_up, t, r, q, sigma),
                BarrierType::UpAndIn,
                OptionType::Call
            )
        );
    }

    #[test]
    fn barrier_put_dispatch_matches_leaf_functions() {
        let (s, k, b, t, r, q, sigma) = (100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.20);
        assert_eq!(
            barrier_put(s, k, b, r, q, sigma, t, "down", "out").unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b, t, r, q, sigma),
                BarrierType::DownAndOut,
                OptionType::Put
            )
        );
        assert_eq!(
            barrier_put(s, k, b, r, q, sigma, t, "down", "in").unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b, t, r, q, sigma),
                BarrierType::DownAndIn,
                OptionType::Put
            )
        );
        let b_up = 120.0;
        assert_eq!(
            barrier_put(s, k, b_up, r, q, sigma, t, "up", "out").unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b_up, t, r, q, sigma),
                BarrierType::UpAndOut,
                OptionType::Put
            )
        );
        assert_eq!(
            barrier_put(s, k, b_up, r, q, sigma, t, "up", "in").unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b_up, t, r, q, sigma),
                BarrierType::UpAndIn,
                OptionType::Put
            )
        );
        assert!(barrier_put(s, k, b, r, q, sigma, t, "sideways", "out").is_err());
    }

    #[test]
    fn barrier_dispatch_rejects_unknown_selector() {
        let err =
            barrier_call(100.0, 100.0, 90.0, 0.05, 0.02, 0.20, 1.0, "sideways", "out").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown barrier spec"), "message: {msg}");
        assert!(msg.contains("direction='sideways'"), "message: {msg}");
    }

    #[test]
    fn asian_dispatch_matches_leaf_functions() {
        let (s, k, t, r, q, sigma, n) = (100.0, 100.0, 1.0, 0.05, 0.02, 0.20, 12);
        assert_eq!(
            asian_option_price(s, k, r, q, sigma, t, n, "arithmetic", OptionType::Call).unwrap(),
            arithmetic_asian_call_tw(s, k, t, r, q, sigma, n)
        );
        assert_eq!(
            asian_option_price(s, k, r, q, sigma, t, n, "geometric", OptionType::Put).unwrap(),
            geometric_asian_put(s, k, t, r, q, sigma, n)
        );
    }

    #[test]
    fn asian_dispatch_rejects_zero_fixings() {
        for averaging in ["arithmetic", "geometric"] {
            let err = asian_option_price(
                100.0,
                100.0,
                0.05,
                0.02,
                0.20,
                1.0,
                0,
                averaging,
                OptionType::Call,
            )
            .unwrap_err();
            assert!(err.to_string().contains("num_fixings must be positive"));
        }
    }

    #[test]
    fn asian_dispatch_rejects_unknown_averaging() {
        let err = asian_option_price(
            100.0,
            100.0,
            0.05,
            0.02,
            0.20,
            1.0,
            12,
            "median",
            OptionType::Call,
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown averaging 'median'"));
    }

    #[test]
    fn lookback_dispatch_matches_leaf_functions() {
        let (s, k, t, r, q, sigma, m) = (100.0, 100.0, 1.0, 0.05, 0.02, 0.20, 100.0);
        assert_eq!(
            lookback_option_price(s, k, r, q, sigma, t, m, "fixed", OptionType::Call).unwrap(),
            fixed_strike_lookback_call(s, k, t, r, q, sigma, m)
        );
        assert_eq!(
            lookback_option_price(s, k, r, q, sigma, t, m, "floating", OptionType::Put).unwrap(),
            floating_strike_lookback_put(s, t, r, q, sigma, m)
        );
    }

    #[test]
    fn lookback_dispatch_rejects_unknown_strike_type() {
        let err = lookback_option_price(
            100.0,
            100.0,
            0.05,
            0.02,
            0.20,
            1.0,
            100.0,
            "adaptive",
            OptionType::Call,
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown strike_type 'adaptive'"));
    }

    #[test]
    fn quanto_dispatch_matches_leaf_functions_and_checks_finiteness() {
        let price = quanto_option_price(
            100.0,
            100.0,
            1.0,
            0.03,
            0.01,
            0.0,
            0.20,
            0.10,
            0.3,
            OptionType::Call,
        )
        .unwrap();
        assert_eq!(
            price,
            quanto_call(100.0, 100.0, 1.0, 0.03, 0.01, 0.0, 0.20, 0.10, 0.3)
        );
        // Degenerate maturity with a negative domestic rate drives the price
        // non-finite; the dispatcher must reject it.
        assert!(quanto_option_price(
            100.0,
            100.0,
            1.0e6,
            -1.0,
            0.01,
            0.0,
            0.20,
            0.10,
            0.3,
            OptionType::Put,
        )
        .is_err());
    }
}
