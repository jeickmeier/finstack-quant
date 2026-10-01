//! Checked forward-option prices and Greeks shared by Rust and host APIs.

use super::{
    bachelier_call, bachelier_delta_call, bachelier_delta_put, bachelier_gamma, bachelier_put,
    bachelier_vega, black_call, black_delta_call, black_delta_put, black_gamma, black_put,
    black_vega,
};
use crate::closed_form::vanilla::checked_closed_form_value;
use crate::types::OptionType;
use finstack_quant_core::{Error, Result};

/// Undiscounted option sensitivities on a unit annuity, with respect to the forward.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForwardGreeks {
    /// First derivative of the undiscounted option premium with respect to the forward.
    pub delta: f64,
    /// Second derivative with respect to the forward, in inverse forward-price units.
    pub gamma: f64,
    /// Derivative per unit (1.0) annual volatility change, not per percentage point.
    pub vega: f64,
}

fn validate_inputs(forward: f64, strike: f64, vol: f64, expiry: f64) -> Result<()> {
    for (name, value) in [
        ("forward", forward),
        ("strike", strike),
        ("vol", vol),
        ("expiry", expiry),
    ] {
        if !value.is_finite() {
            return Err(Error::Validation(format!(
                "option {name} must be finite, got {value}"
            )));
        }
    }
    if vol < 0.0 || expiry < 0.0 {
        return Err(Error::Validation(format!(
            "option volatility and expiry must be non-negative, got vol={vol}, expiry={expiry}"
        )));
    }
    Ok(())
}

fn validate_black(forward: f64, strike: f64, vol: f64, expiry: f64) -> Result<()> {
    validate_inputs(forward, strike, vol, expiry)?;
    if forward <= 0.0 || strike <= 0.0 {
        return Err(Error::Validation(format!(
            "Black requires positive forward and strike, got forward={forward}, strike={strike}"
        )));
    }
    Ok(())
}

fn checked_greeks(delta: f64, gamma: f64, vega: f64) -> Result<ForwardGreeks> {
    Ok(ForwardGreeks {
        delta: checked_closed_form_value(delta, "forward delta")?,
        gamma: checked_closed_form_value(gamma, "forward gamma")?,
        vega: checked_closed_form_value(vega, "forward vega")?,
    })
}

/// Discounted Black-76 option premium per unit of underlying.
///
/// Zero volatility or expiry returns discounted forward intrinsic value.
///
/// # Arguments
///
/// * `forward` - Positive finite forward price or rate at expiry, in strike units.
/// * `strike` - Positive finite exercise price or rate, in forward units.
/// * `df` - Positive finite discount factor from valuation to option payment.
/// * `expiry` - Finite non-negative remaining expiry in years.
/// * `vol` - Finite non-negative annual lognormal volatility as a decimal.
/// * `option_type` - Call or put payoff direction.
///
/// # Errors
///
/// Returns a validation error for an invalid input or non-finite premium.
pub fn black76_price(
    forward: f64,
    strike: f64,
    df: f64,
    expiry: f64,
    vol: f64,
    option_type: OptionType,
) -> Result<f64> {
    validate_black(forward, strike, vol, expiry)?;
    if !df.is_finite() || df <= 0.0 {
        return Err(Error::Validation(format!(
            "Black discount factor must be finite and positive, got {df}"
        )));
    }
    let value = match option_type {
        OptionType::Call => black_call(forward, strike, vol, expiry),
        OptionType::Put => black_put(forward, strike, vol, expiry),
    };
    checked_closed_form_value(df * value, "Black-76 price")
}

/// Undiscounted Black-76 forward delta, gamma, and vega on a unit annuity.
///
/// At zero expiry or volatility, gamma and vega are zero; call delta is one
/// when the forward is at or above the strike and zero otherwise. Put delta
/// is call delta minus one. These are explicit boundary conventions at the
/// payoff kink, where a two-sided derivative does not exist.
///
/// # Arguments
///
/// * `forward` - Positive finite forward price or rate at expiry, in strike units.
/// * `strike` - Positive finite exercise price or rate, in forward units.
/// * `expiry` - Finite non-negative remaining expiry in years.
/// * `vol` - Finite non-negative annual lognormal volatility as a decimal.
/// * `option_type` - Call or put payoff direction; gamma and vega are identical.
///
/// # Errors
///
/// Returns a validation error for an invalid input or non-finite sensitivity.
pub fn black76_greeks(
    forward: f64,
    strike: f64,
    expiry: f64,
    vol: f64,
    option_type: OptionType,
) -> Result<ForwardGreeks> {
    validate_black(forward, strike, vol, expiry)?;
    let delta = match option_type {
        OptionType::Call => black_delta_call(forward, strike, vol, expiry),
        OptionType::Put => black_delta_put(forward, strike, vol, expiry),
    };
    checked_greeks(
        delta,
        black_gamma(forward, strike, vol, expiry),
        black_vega(forward, strike, vol, expiry),
    )
}

/// Undiscounted Bachelier option premium on a unit annuity.
///
/// Negative forward prices and strikes are supported. Zero volatility or
/// expiry returns forward intrinsic value.
///
/// # Arguments
///
/// * `forward` - Finite forward price or rate at expiry, in strike units.
/// * `strike` - Finite exercise price or rate, in forward units.
/// * `normal_vol` - Finite non-negative annual normal volatility in absolute
///   forward units per square-root year, such as `0.0075` for 75 bp on decimal rates.
/// * `expiry` - Finite non-negative remaining expiry in years.
/// * `option_type` - Call or put payoff direction.
///
/// # Errors
///
/// Returns a validation error for an invalid input or non-finite premium.
pub fn bachelier_price(
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    option_type: OptionType,
) -> Result<f64> {
    validate_inputs(forward, strike, normal_vol, expiry)?;
    let value = match option_type {
        OptionType::Call => bachelier_call(forward, strike, normal_vol, expiry),
        OptionType::Put => bachelier_put(forward, strike, normal_vol, expiry),
    };
    checked_closed_form_value(value, "Bachelier price")
}

/// Undiscounted Bachelier forward delta, gamma, and vega on a unit annuity.
///
/// At zero expiry or volatility, gamma and vega are zero; call delta is one
/// when the forward is at or above the strike and zero otherwise. Put delta
/// is call delta minus one. The payoff kink has no two-sided derivative.
///
/// # Arguments
///
/// * `forward` - Finite forward price or rate at expiry; negative values are supported.
/// * `strike` - Finite exercise price or rate, in forward units; may be negative.
/// * `normal_vol` - Finite non-negative annual normal volatility in absolute
///   forward units per square-root year, not a lognormal percentage volatility.
/// * `expiry` - Finite non-negative remaining expiry in years.
/// * `option_type` - Call or put payoff direction; gamma and vega are identical.
///
/// # Errors
///
/// Returns a validation error for an invalid input or non-finite sensitivity.
pub fn bachelier_greeks(
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    option_type: OptionType,
) -> Result<ForwardGreeks> {
    validate_inputs(forward, strike, normal_vol, expiry)?;
    let delta = match option_type {
        OptionType::Call => bachelier_delta_call(forward, strike, normal_vol, expiry),
        OptionType::Put => bachelier_delta_put(forward, strike, normal_vol, expiry),
    };
    checked_greeks(
        delta,
        bachelier_gamma(forward, strike, normal_vol, expiry),
        bachelier_vega(forward, strike, normal_vol, expiry),
    )
}

fn shifted_coordinates(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
) -> Result<(f64, f64)> {
    validate_inputs(forward, strike, vol, expiry)?;
    if !shift.is_finite() {
        return Err(Error::Validation(format!(
            "Black shift must be finite, got {shift}"
        )));
    }
    let shifted = (forward + shift, strike + shift);
    validate_black(shifted.0, shifted.1, vol, expiry)?;
    Ok(shifted)
}

/// Undiscounted shifted-Black option premium on a unit annuity.
///
/// # Arguments
///
/// * `forward` - Finite unshifted forward price or rate; may be negative.
/// * `strike` - Finite exercise price or rate in the same units as `forward`.
/// * `vol` - Finite non-negative annual shifted-lognormal volatility as a decimal.
/// * `expiry` - Finite non-negative remaining expiry in years.
/// * `shift` - Finite additive displacement in forward units; both shifted
///   forward and strike must be finite and strictly positive.
/// * `option_type` - Call or put payoff direction.
///
/// # Errors
///
/// Returns a validation error for invalid original or shifted inputs or a
/// non-finite premium, including when expiry or volatility is zero.
pub fn black_shifted_price(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
    option_type: OptionType,
) -> Result<f64> {
    let (f, k) = shifted_coordinates(forward, strike, vol, expiry, shift)?;
    black76_price(f, k, 1.0, expiry, vol, option_type)
}

/// Undiscounted shifted-Black vega per unit (1.0) volatility change.
///
/// Returns zero at zero volatility or expiry, matching the forward-Greeks
/// boundary convention. No discount factor or contract scaling is applied.
///
/// # Arguments
///
/// * `forward` - Finite unshifted forward price or rate; may be negative.
/// * `strike` - Finite exercise price or rate in the same units as `forward`.
/// * `vol` - Finite non-negative annual shifted-lognormal volatility as a decimal.
/// * `expiry` - Finite non-negative remaining expiry in years.
/// * `shift` - Finite additive displacement in forward units; both shifted
///   forward and strike must be finite and strictly positive.
///
/// # Errors
///
/// Returns a validation error for invalid original or shifted inputs or a
/// non-finite vega, including when expiry or volatility is zero.
pub fn black_shifted_vega(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
) -> Result<f64> {
    let (f, k) = shifted_coordinates(forward, strike, vol, expiry, shift)?;
    checked_closed_form_value(black_vega(f, k, vol, expiry), "shifted Black vega")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_forward_models_reject_bad_data_before_intrinsic_branches() {
        for expiry in [0.0, 1.0] {
            for vol in [0.0, 0.2] {
                for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    assert!(
                        black76_price(invalid, 100.0, 1.0, expiry, vol, OptionType::Call).is_err()
                    );
                    assert!(
                        black76_price(100.0, invalid, 1.0, expiry, vol, OptionType::Call).is_err()
                    );
                    assert!(black76_greeks(invalid, 100.0, expiry, vol, OptionType::Call).is_err());
                    assert!(
                        bachelier_price(invalid, 100.0, vol, expiry, OptionType::Call).is_err()
                    );
                    assert!(
                        bachelier_greeks(100.0, invalid, vol, expiry, OptionType::Call).is_err()
                    );
                    assert!(black_shifted_price(
                        0.02,
                        0.03,
                        vol,
                        expiry,
                        invalid,
                        OptionType::Call
                    )
                    .is_err());
                    assert!(black_shifted_vega(0.02, 0.03, vol, expiry, invalid).is_err());
                }
            }
        }
        assert!(black76_price(100.0, 100.0, -1.0, 1.0, 0.2, OptionType::Call).is_err());
        assert!(black76_price(-0.01, 0.01, 1.0, 1.0, 0.2, OptionType::Put).is_err());
        assert!(black76_greeks(100.0, 100.0, 1.0, -0.2, OptionType::Call).is_err());
        assert!(bachelier_price(0.03, 0.03, -0.01, 1.0, OptionType::Call).is_err());
        assert!(bachelier_greeks(0.03, 0.03, 0.01, -1.0, OptionType::Call).is_err());
        assert!(black_shifted_price(-0.04, -0.02, 0.0, 0.0, 0.03, OptionType::Call).is_err());
        assert!(black_shifted_vega(f64::MAX, 1.0, 0.0, 0.0, f64::MAX).is_err());
    }

    #[test]
    fn checked_forward_models_preserve_valid_boundaries_and_negative_normal_rates() {
        assert_eq!(
            black76_price(110.0, 100.0, 0.95, 1.0, 0.0, OptionType::Call).unwrap(),
            9.5
        );
        assert_eq!(
            black76_price(110.0, 100.0, 0.95, 0.0, 0.2, OptionType::Put).unwrap(),
            0.0
        );
        let call = bachelier_price(-0.02, -0.03, 0.01, 2.0, OptionType::Call).unwrap();
        let put = bachelier_price(-0.02, -0.03, 0.01, 2.0, OptionType::Put).unwrap();
        assert!((call - put - 0.01).abs() < 1e-14);
        let greeks = black76_greeks(110.0, 100.0, 0.0, 0.2, OptionType::Call).unwrap();
        assert_eq!(
            greeks,
            ForwardGreeks {
                delta: 1.0,
                gamma: 0.0,
                vega: 0.0
            }
        );
        assert!(
            black_shifted_price(-0.01, -0.005, 0.2, 1.0, 0.03, OptionType::Call).unwrap() > 0.0
        );
    }
}
