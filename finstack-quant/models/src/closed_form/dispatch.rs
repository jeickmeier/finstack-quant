//! Canonical string-keyed dispatch for the closed-form exotic pricers.
//!
//! Host bindings (Python, WASM) expose the barrier / Asian / lookback / quanto
//! closed forms behind compact selectors (`direction`/`knock` and
//! `strike_type` strings, the [`AveragingMethod`] enum) plus a call/put flag. This module is the single
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
//! use finstack_quant_models::types::{OptionMarketParams, OptionType};
//! use finstack_quant_models::closed_form::{
//!     asian_option_price, barrier_option_price,
//! };
//! use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
//!
//! let call = OptionMarketParams {
//!     spot: 100.0,
//!     strike: 100.0,
//!     rate: 0.05,
//!     volatility: 0.20,
//!     time_to_expiry: 1.0,
//!     dividend_yield: 0.02,
//!     option_type: OptionType::Call,
//! };
//! let barrier_call = barrier_option_price(&call, 90.0, "down", "out")?;
//! assert!(barrier_call > 0.0);
//! let put = OptionMarketParams { option_type: OptionType::Put, ..call.clone() };
//! let barrier_put = barrier_option_price(&put, 90.0, "down", "out")?;
//! assert!(barrier_put >= 0.0);
//!
//! let asian = asian_option_price(&call, 12, AveragingMethod::Arithmetic)?;
//! assert!(asian > 0.0);
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

use finstack_quant_core::{Error, Result};

use super::asian::{arithmetic_asian_tw_price, geometric_asian_price};
use super::barrier::{BarrierParams, barrier_price};
use super::lookback::{
    fixed_strike_lookback_call, fixed_strike_lookback_put, floating_strike_lookback_call,
    floating_strike_lookback_put,
};
use super::quanto::quanto_price;
use super::vanilla::{
    checked_closed_form_value, require_finite_positive, validate_spot_model_inputs,
};
use crate::monte_carlo::payoff::asian::AveragingMethod;
use crate::types::{OptionMarketParams, OptionType};
use finstack_quant_core::types::BarrierType;

/// Averaging convention host bindings use when `averaging` is omitted from
/// [`asian_option_price`]: arithmetic (Turnbull-Wakeman approximation).
pub const DEFAULT_ASIAN_AVERAGING: AveragingMethod = AveragingMethod::Arithmetic;

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

/// Reiner-Rubinstein continuous-monitoring barrier option, selected by strings.
///
/// Parses the `(direction, knock)` pair into a [`BarrierType`] and calls
/// [`barrier_price`].
///
/// # Arguments
///
/// * `market` - Spot and strike in the same price units, continuously
///   compounded risk-free `rate` and `dividend_yield` (foreign rate for FX) as
///   decimals, annualized decimal `volatility`, `time_to_expiry` in years and
///   the call/put `option_type`.
/// * `barrier` - Continuously monitored barrier level in the same units as `spot`.
/// * `direction` - `"up"` for an upper barrier or `"down"` for a lower barrier.
/// * `knock` - `"in"` for knock-in or `"out"` for knock-out.
///
/// # Errors
///
/// Returns `Error::Validation` if `(direction, knock)` is not a supported
/// pair, if any input is outside its valid domain, or if the resulting
/// barrier price is non-finite.
pub fn barrier_option_price(
    market: &OptionMarketParams,
    barrier: f64,
    direction: &str,
    knock: &str,
) -> Result<f64> {
    let &OptionMarketParams {
        spot,
        strike,
        rate,
        volatility: vol,
        time_to_expiry: expiry,
        dividend_yield: div_yield,
        option_type,
    } = market;
    let barrier_type = parse_barrier_type(direction, knock)?;
    validate_spot_model_inputs("barrier", spot, rate, div_yield, vol, expiry)?;
    require_finite_positive("barrier", "strike", strike)?;
    require_finite_positive("barrier", "barrier", barrier)?;
    let params = BarrierParams::new(spot, strike, barrier, expiry, rate, div_yield, vol);
    let value = barrier_price(&params, barrier_type, option_type);
    checked_closed_form_value(value, "barrier price")
}

/// Asian option price, selected by averaging convention.
///
/// Routes to the Turnbull-Wakeman arithmetic approximation
/// ([`arithmetic_asian_tw_price`]) or the exact Kemna-Vorst geometric closed
/// form ([`geometric_asian_price`]).
///
/// # Arguments
///
/// * `market` - Spot and strike in the same price units, continuously
///   compounded risk-free `rate` and `dividend_yield` (foreign rate for FX) as
///   decimals, annualized decimal `volatility`, `time_to_expiry` in years and
///   the call/put `option_type`.
/// * `num_fixings` - Positive number of equally spaced averaging observations.
/// * `averaging` - [`AveragingMethod::Arithmetic`] (Turnbull-Wakeman) or
///   [`AveragingMethod::Geometric`] (Kemna-Vorst).
///
/// # Errors
///
/// Returns `Error::Validation` if `num_fixings` is zero, any input is outside
/// its valid domain, or the resulting option price is non-finite.
pub fn asian_option_price(
    market: &OptionMarketParams,
    num_fixings: usize,
    averaging: AveragingMethod,
) -> Result<f64> {
    let &OptionMarketParams {
        spot,
        strike,
        rate,
        volatility: vol,
        time_to_expiry: expiry,
        dividend_yield: div_yield,
        ..
    } = market;
    if num_fixings == 0 {
        return Err(Error::Validation(
            "asian option num_fixings must be positive".to_string(),
        ));
    }
    validate_spot_model_inputs("asian option", spot, rate, div_yield, vol, expiry)?;
    require_finite_positive("asian option", "strike", strike)?;
    let value = match averaging {
        AveragingMethod::Arithmetic => arithmetic_asian_tw_price(market, num_fixings),
        AveragingMethod::Geometric => geometric_asian_price(market, num_fixings),
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
/// * `market` - Spot and strike in the same price units, continuously
///   compounded risk-free `rate` and `dividend_yield` (foreign rate for FX) as
///   decimals, annualized decimal `volatility`, `time_to_expiry` in years and
///   the call/put `option_type`. The
///   `strike` is ignored (but must be finite) for `"floating"`.
/// * `extremum` - Observed running extremum to date, in `spot` units.
/// * `strike_type` - `"fixed"` or `"floating"`.
///
/// # Errors
///
/// Returns `Error::Validation` if `strike_type` is not a supported
/// convention, any input is outside its valid domain, or the resulting
/// option price is non-finite.
pub fn lookback_option_price(
    market: &OptionMarketParams,
    extremum: f64,
    strike_type: &str,
) -> Result<f64> {
    let &OptionMarketParams {
        spot,
        strike,
        rate,
        volatility: vol,
        time_to_expiry: expiry,
        dividend_yield: div_yield,
        option_type,
    } = market;
    if !matches!(strike_type, "fixed" | "floating") {
        return Err(Error::Validation(format!(
            "unknown strike_type '{strike_type}'; expected 'fixed' or 'floating'"
        )));
    }
    validate_spot_model_inputs("lookback option", spot, rate, div_yield, vol, expiry)?;
    require_finite_positive("lookback option", "extremum", extremum)?;
    if strike_type == "fixed" {
        require_finite_positive("lookback option", "strike", strike)?;
    } else if !strike.is_finite() {
        return Err(Error::Validation(format!(
            "lookback option strike must be finite, got {strike}"
        )));
    }
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
/// Validates the inputs, calls [`quanto_price`] and rejects non-finite
/// results, so host bindings share one guard.
///
/// # Arguments
///
/// * `market` - The foreign asset: `spot` and `strike` in foreign currency,
///   `rate` = the **domestic** risk-free rate and `dividend_yield` = the
///   asset's continuous dividend yield (continuously compounded decimals),
///   `volatility` = the annualized foreign-asset volatility (decimal),
///   `time_to_expiry` in years and the call/put `option_type`.
/// * `rate_foreign` - Foreign risk-free rate, continuously compounded decimal.
/// * `vol_fx` - Annualized FX-rate volatility, decimal.
/// * `correlation` - Correlation between asset and FX returns, in `[-1, 1]`.
///
/// # Errors
///
/// Returns `Error::Validation` if any input is outside its valid domain or
/// if the resulting option price is non-finite.
pub fn quanto_option_price(
    market: &OptionMarketParams,
    rate_foreign: f64,
    vol_fx: f64,
    correlation: f64,
) -> Result<f64> {
    let &OptionMarketParams {
        spot,
        strike,
        rate: rate_domestic,
        volatility: vol_asset,
        time_to_expiry: expiry,
        dividend_yield: div_yield,
        ..
    } = market;
    validate_spot_model_inputs(
        "quanto option",
        spot,
        rate_domestic,
        div_yield,
        vol_asset,
        expiry,
    )?;
    require_finite_positive("quanto option", "strike", strike)?;
    if !rate_foreign.is_finite() {
        return Err(Error::Validation(format!(
            "quanto option rate_foreign must be finite, got {rate_foreign}"
        )));
    }
    if !vol_fx.is_finite() || vol_fx < 0.0 {
        return Err(Error::Validation(format!(
            "quanto option vol_fx must be finite and non-negative, got {vol_fx}"
        )));
    }
    if !correlation.is_finite() || !(-1.0..=1.0).contains(&correlation) {
        return Err(Error::Validation(format!(
            "quanto option correlation must be in [-1, 1], got {correlation}"
        )));
    }
    let value = quanto_price(market, rate_foreign, vol_fx, correlation);
    checked_closed_form_value(value, "quanto option price")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barrier_dispatch_matches_leaf_functions() {
        let (s, k, b, t, r, q, sigma) = (100.0, 100.0, 90.0, 1.0, 0.05, 0.02, 0.20);
        assert_eq!(
            barrier_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Call
                },
                b,
                "down",
                "out"
            )
            .unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b, t, r, q, sigma),
                BarrierType::DownAndOut,
                OptionType::Call
            )
        );
        assert_eq!(
            barrier_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Call
                },
                b,
                "down",
                "in"
            )
            .unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b, t, r, q, sigma),
                BarrierType::DownAndIn,
                OptionType::Call
            )
        );
        let b_up = 120.0;
        assert_eq!(
            barrier_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Call
                },
                b_up,
                "up",
                "out"
            )
            .unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b_up, t, r, q, sigma),
                BarrierType::UpAndOut,
                OptionType::Call
            )
        );
        assert_eq!(
            barrier_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Call
                },
                b_up,
                "up",
                "in"
            )
            .unwrap(),
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
            barrier_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Put
                },
                b,
                "down",
                "out"
            )
            .unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b, t, r, q, sigma),
                BarrierType::DownAndOut,
                OptionType::Put
            )
        );
        assert_eq!(
            barrier_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Put
                },
                b,
                "down",
                "in"
            )
            .unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b, t, r, q, sigma),
                BarrierType::DownAndIn,
                OptionType::Put
            )
        );
        let b_up = 120.0;
        assert_eq!(
            barrier_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Put
                },
                b_up,
                "up",
                "out"
            )
            .unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b_up, t, r, q, sigma),
                BarrierType::UpAndOut,
                OptionType::Put
            )
        );
        assert_eq!(
            barrier_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Put
                },
                b_up,
                "up",
                "in"
            )
            .unwrap(),
            barrier_price(
                &BarrierParams::new(s, k, b_up, t, r, q, sigma),
                BarrierType::UpAndIn,
                OptionType::Put
            )
        );
        assert!(
            barrier_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Put
                },
                b,
                "sideways",
                "out"
            )
            .is_err()
        );
    }

    #[test]
    fn barrier_dispatch_rejects_unknown_selector() {
        let err = barrier_option_price(
            &OptionMarketParams {
                spot: 100.0,
                strike: 100.0,
                rate: 0.05,
                volatility: 0.20,
                time_to_expiry: 1.0,
                dividend_yield: 0.02,
                option_type: OptionType::Call,
            },
            90.0,
            "sideways",
            "out",
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown barrier spec"), "message: {msg}");
        assert!(msg.contains("direction='sideways'"), "message: {msg}");
    }

    #[test]
    fn asian_dispatch_matches_leaf_functions() {
        let (s, k, t, r, q, sigma, n) = (100.0, 100.0, 1.0, 0.05, 0.02, 0.20, 12);
        assert_eq!(
            asian_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Call
                },
                n,
                AveragingMethod::Arithmetic
            )
            .unwrap(),
            arithmetic_asian_tw_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Call
                },
                n
            )
        );
        assert_eq!(
            asian_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Put
                },
                n,
                AveragingMethod::Geometric
            )
            .unwrap(),
            geometric_asian_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Put
                },
                n
            )
        );
    }

    #[test]
    fn asian_dispatch_rejects_zero_fixings() {
        for averaging in [AveragingMethod::Arithmetic, AveragingMethod::Geometric] {
            let err = asian_option_price(
                &OptionMarketParams {
                    spot: 100.0,
                    strike: 100.0,
                    rate: 0.05,
                    volatility: 0.20,
                    time_to_expiry: 1.0,
                    dividend_yield: 0.02,
                    option_type: OptionType::Call,
                },
                0,
                averaging,
            )
            .unwrap_err();
            assert!(err.to_string().contains("num_fixings must be positive"));
        }
    }

    #[test]
    fn lookback_dispatch_matches_leaf_functions() {
        let (s, k, t, r, q, sigma, m) = (100.0, 100.0, 1.0, 0.05, 0.02, 0.20, 100.0);
        assert_eq!(
            lookback_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Call
                },
                m,
                "fixed"
            )
            .unwrap(),
            fixed_strike_lookback_call(s, k, t, r, q, sigma, m)
        );
        assert_eq!(
            lookback_option_price(
                &OptionMarketParams {
                    spot: s,
                    strike: k,
                    rate: r,
                    volatility: sigma,
                    time_to_expiry: t,
                    dividend_yield: q,
                    option_type: OptionType::Put
                },
                m,
                "floating"
            )
            .unwrap(),
            floating_strike_lookback_put(s, t, r, q, sigma, m)
        );
    }

    #[test]
    fn lookback_dispatch_rejects_unknown_strike_type() {
        let err = lookback_option_price(
            &OptionMarketParams {
                spot: 100.0,
                strike: 100.0,
                rate: 0.05,
                volatility: 0.20,
                time_to_expiry: 1.0,
                dividend_yield: 0.02,
                option_type: OptionType::Call,
            },
            100.0,
            "adaptive",
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown strike_type 'adaptive'"));
    }

    #[test]
    fn quanto_dispatch_matches_leaf_functions_and_checks_finiteness() {
        let price = quanto_option_price(
            &OptionMarketParams {
                spot: 100.0,
                strike: 100.0,
                rate: 0.03,
                volatility: 0.20,
                time_to_expiry: 1.0,
                dividend_yield: 0.0,
                option_type: OptionType::Call,
            },
            0.01,
            0.10,
            0.3,
        )
        .unwrap();
        assert_eq!(
            price,
            quanto_price(
                &OptionMarketParams {
                    spot: 100.0,
                    strike: 100.0,
                    rate: 0.03,
                    volatility: 0.20,
                    time_to_expiry: 1.0,
                    dividend_yield: 0.0,
                    option_type: OptionType::Call
                },
                0.01,
                0.10,
                0.3
            )
        );
        // Degenerate maturity with a negative domestic rate drives the price
        // non-finite; the dispatcher must reject it.
        assert!(
            quanto_option_price(
                &OptionMarketParams {
                    spot: 100.0,
                    strike: 100.0,
                    rate: -1.0,
                    volatility: 0.20,
                    time_to_expiry: 1.0e6,
                    dividend_yield: 0.0,
                    option_type: OptionType::Put
                },
                0.01,
                0.10,
                0.3
            )
            .is_err()
        );
    }
}
