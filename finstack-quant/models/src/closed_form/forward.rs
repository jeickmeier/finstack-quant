//! Checked forward-measure option entry points for host bindings.
//!
//! The leaf Black-76, Bachelier and shifted-Black formulas in
//! [`super::volatility`] are infallible unit-annuity kernels that collapse
//! degenerate inputs to intrinsic value. The functions here are the checked,
//! [`OptionType`]-dispatched twins the Python and WASM bindings call: each
//! validates its model's input domain, picks the call or put leg, applies any
//! discount factor, and finishes through [`checked_closed_form_value`], so a
//! bad input crosses a host boundary as a validation error rather than a
//! silent intrinsic value or a negative premium.
//!
//! | Function | Domain |
//! |---|---|
//! | [`black76_price`], [`black76_greeks`] | finite; `forward`, `strike` > 0; `vol`, `expiry` >= 0; `df` > 0 |
//! | [`bachelier_price`], [`bachelier_greeks`] | finite; `normal_vol`, `expiry` >= 0; any-sign forward and strike |
//! | [`black_shifted_price`], [`black_shifted_vega`] | finite; `forward + shift`, `strike + shift` > 0; `vol`, `expiry` >= 0 |
//!
//! Black-76 accepts the same `df` domain as its inverse
//! [`super::implied_vol::black76_implied_vol`] (finite and strictly positive).
//!
//! # References
//!
//! - Black, F. (1976), "The pricing of commodity contracts". `docs/REFERENCES.md#black-1976`
//! - Bachelier, L. (1900), "Theorie de la speculation". `docs/REFERENCES.md#bachelier-1900`
//!
//! # Examples
//!
//! ```rust
//! use finstack_quant_models::OptionType;
//! use finstack_quant_models::closed_form::{black76_greeks, black76_price};
//!
//! let price = black76_price(100.0, 100.0, 0.95, 1.0, 0.2, OptionType::Call)?;
//! assert!((price - 7.5673).abs() < 1e-4);
//! let greeks = black76_greeks(100.0, 100.0, 1.0, 0.2, OptionType::Call)?;
//! assert!((greeks.delta - 0.5398).abs() < 1e-4);
//! assert!(black76_price(100.0, 100.0, -0.95, 1.0, 0.2, OptionType::Call).is_err());
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

use finstack_quant_core::{Error, Result};

use super::vanilla::checked_closed_form_value;
use super::volatility::{
    bachelier_call, bachelier_delta_call, bachelier_delta_put, bachelier_gamma, bachelier_put,
    bachelier_vega, black_call, black_delta_call, black_delta_put, black_gamma, black_put,
    black_vega,
};
use crate::types::OptionType;

/// Undiscounted forward Greeks of a European option on a forward.
///
/// Returned by [`black76_greeks`] and [`bachelier_greeks`]. `delta` and
/// `gamma` are with respect to the forward; `vega` is per unit (1.0) change in
/// the model volatility (lognormal decimal for Black-76, absolute normal vol
/// for Bachelier). Multiply by the discount factor or annuity for present-value
/// sensitivities. Every field is finite when produced by those functions.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::OptionType;
/// use finstack_quant_models::closed_form::bachelier_greeks;
///
/// let g = bachelier_greeks(0.03, 0.03, 0.0075, 1.0, OptionType::Call)?;
/// assert!((g.delta - 0.5).abs() < 1e-12);
/// assert!(g.gamma > 0.0 && g.vega > 0.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ForwardGreeks {
    /// Sensitivity of the undiscounted premium to the forward (`dV/dF`).
    pub delta: f64,
    /// Second derivative of the undiscounted premium to the forward (`d2V/dF2`).
    pub gamma: f64,
    /// Sensitivity of the undiscounted premium to a unit (1.0) change in vol.
    pub vega: f64,
}

impl ForwardGreeks {
    /// Check each Greek is finite, labelling failures with `model`.
    fn checked(delta: f64, gamma: f64, vega: f64, model: &str) -> Result<Self> {
        Ok(Self {
            delta: checked_closed_form_value(delta, &format!("{model} delta"))?,
            gamma: checked_closed_form_value(gamma, &format!("{model} gamma"))?,
            vega: checked_closed_form_value(vega, &format!("{model} vega"))?,
        })
    }
}

fn require_finite(model: &str, inputs: &[(&str, f64)]) -> Result<()> {
    for &(name, value) in inputs {
        if !value.is_finite() {
            return Err(Error::Validation(format!(
                "{model} {name} must be finite, got {value}"
            )));
        }
    }
    Ok(())
}

fn require_positive(model: &str, name: &str, value: f64) -> Result<()> {
    if value <= 0.0 {
        return Err(Error::Validation(format!(
            "{model} {name} must be positive, got {value}"
        )));
    }
    Ok(())
}

fn require_non_negative(model: &str, name: &str, value: f64) -> Result<()> {
    if value < 0.0 {
        return Err(Error::Validation(format!(
            "{model} {name} must be non-negative, got {value}"
        )));
    }
    Ok(())
}

fn validate_black76(forward: f64, strike: f64, vol: f64, expiry: f64) -> Result<()> {
    const MODEL: &str = "Black-76";
    require_finite(
        MODEL,
        &[
            ("forward", forward),
            ("strike", strike),
            ("vol", vol),
            ("expiry", expiry),
        ],
    )?;
    require_positive(MODEL, "forward", forward)?;
    require_positive(MODEL, "strike", strike)?;
    require_non_negative(MODEL, "vol", vol)?;
    require_non_negative(MODEL, "expiry", expiry)
}

fn validate_bachelier(forward: f64, strike: f64, normal_vol: f64, expiry: f64) -> Result<()> {
    const MODEL: &str = "Bachelier";
    require_finite(
        MODEL,
        &[
            ("forward", forward),
            ("strike", strike),
            ("normal_vol", normal_vol),
            ("expiry", expiry),
        ],
    )?;
    require_non_negative(MODEL, "normal_vol", normal_vol)?;
    require_non_negative(MODEL, "expiry", expiry)
}

fn validate_black_shifted(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
) -> Result<()> {
    const MODEL: &str = "shifted Black";
    require_finite(
        MODEL,
        &[
            ("forward", forward),
            ("strike", strike),
            ("vol", vol),
            ("expiry", expiry),
            ("shift", shift),
        ],
    )?;
    require_positive(MODEL, "forward + shift", forward + shift)?;
    require_positive(MODEL, "strike + shift", strike + shift)?;
    require_non_negative(MODEL, "vol", vol)?;
    require_non_negative(MODEL, "expiry", expiry)
}

/// Discounted Black-76 price of a European option on a forward.
///
/// Returns `df * Black(forward, strike, vol, expiry)`; zero `vol` or `expiry`
/// gives the discounted intrinsic value.
///
/// # Arguments
///
/// * `forward` - Forward price or rate at expiry; finite and strictly positive.
/// * `strike` - Strike in the same units as `forward`; finite and strictly positive.
/// * `df` - Discount factor from valuation to expiry (or settlement), a finite
///   decimal strictly greater than zero; values above 1 (negative rates) are
///   accepted. Same domain as [`super::implied_vol::black76_implied_vol`].
/// * `expiry` - Time to expiry in years; finite and non-negative.
/// * `vol` - Annualized lognormal (Black) volatility as a decimal; finite and
///   non-negative.
/// * `option_type` - Call or put payoff.
///
/// # Returns
///
/// Discounted per-unit premium in the units of `forward`.
///
/// # Errors
///
/// Returns `Error::Validation` when an input is non-finite, `forward`,
/// `strike` or `df` is not positive, `vol` or `expiry` is negative, or the
/// premium is non-finite.
pub fn black76_price(
    forward: f64,
    strike: f64,
    df: f64,
    expiry: f64,
    vol: f64,
    option_type: OptionType,
) -> Result<f64> {
    validate_black76(forward, strike, vol, expiry)?;
    require_finite("Black-76", &[("df", df)])?;
    require_positive("Black-76", "df", df)?;
    let undiscounted = match option_type {
        OptionType::Call => black_call(forward, strike, vol, expiry),
        OptionType::Put => black_put(forward, strike, vol, expiry),
    };
    checked_closed_form_value(df * undiscounted, "Black-76 price")
}

/// Undiscounted Black-76 forward Greeks.
///
/// # Arguments
///
/// * `forward` - Forward price or rate at expiry; finite and strictly positive.
/// * `strike` - Strike in the same units as `forward`; finite and strictly positive.
/// * `expiry` - Time to expiry in years; finite and non-negative.
/// * `vol` - Annualized lognormal (Black) volatility as a decimal; finite and
///   non-negative. `vega` is per unit (1.0) change in this vol.
/// * `option_type` - Call or put; only `delta` differs between the two.
///
/// # Returns
///
/// [`ForwardGreeks`] with delta and gamma to the forward and vega per unit vol,
/// all undiscounted.
///
/// # Errors
///
/// Returns `Error::Validation` when an input is non-finite, `forward` or
/// `strike` is not positive, `vol` or `expiry` is negative, or a Greek is
/// non-finite.
pub fn black76_greeks(
    forward: f64,
    strike: f64,
    expiry: f64,
    vol: f64,
    option_type: OptionType,
) -> Result<ForwardGreeks> {
    validate_black76(forward, strike, vol, expiry)?;
    let delta = match option_type {
        OptionType::Call => black_delta_call(forward, strike, vol, expiry),
        OptionType::Put => black_delta_put(forward, strike, vol, expiry),
    };
    ForwardGreeks::checked(
        delta,
        black_gamma(forward, strike, vol, expiry),
        black_vega(forward, strike, vol, expiry),
        "Black-76",
    )
}

/// Undiscounted Bachelier (normal-model) price on a unit annuity.
///
/// The canonical unit-annuity twin of
/// [`crate::volatility::normal::bachelier_price_with_annuity`]; multiply by the
/// discount factor or annuity for present value.
///
/// # Arguments
///
/// * `forward` - Forward price or rate at expiry; finite, may be negative.
/// * `strike` - Strike in the same units as `forward`; finite, may be negative.
/// * `normal_vol` - Annualized **absolute** (normal) volatility in the units of
///   `forward` (e.g. `0.0075` for 75 bp on a decimal rate); finite and
///   non-negative.
/// * `expiry` - Time to expiry in years; finite and non-negative.
/// * `option_type` - Call (payer) or put (receiver).
///
/// # Returns
///
/// Undiscounted per-unit premium in the units of `forward`.
///
/// # Errors
///
/// Returns `Error::Validation` when an input is non-finite, `normal_vol` or
/// `expiry` is negative, or the premium is non-finite.
pub fn bachelier_price(
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    option_type: OptionType,
) -> Result<f64> {
    validate_bachelier(forward, strike, normal_vol, expiry)?;
    let value = match option_type {
        OptionType::Call => bachelier_call(forward, strike, normal_vol, expiry),
        OptionType::Put => bachelier_put(forward, strike, normal_vol, expiry),
    };
    checked_closed_form_value(value, "Bachelier price")
}

/// Undiscounted Bachelier (normal-model) forward Greeks.
///
/// # Arguments
///
/// * `forward` - Forward price or rate at expiry; finite, may be negative.
/// * `strike` - Strike in the same units as `forward`; finite, may be negative.
/// * `normal_vol` - Annualized absolute (normal) volatility in the units of
///   `forward`; finite and non-negative. `vega` is per unit (1.0) change in it.
/// * `expiry` - Time to expiry in years; finite and non-negative.
/// * `option_type` - Call or put; only `delta` differs between the two.
///
/// # Returns
///
/// [`ForwardGreeks`] with delta and gamma to the forward and vega per unit
/// normal vol, all undiscounted.
///
/// # Errors
///
/// Returns `Error::Validation` when an input is non-finite, `normal_vol` or
/// `expiry` is negative, or a Greek is non-finite.
pub fn bachelier_greeks(
    forward: f64,
    strike: f64,
    normal_vol: f64,
    expiry: f64,
    option_type: OptionType,
) -> Result<ForwardGreeks> {
    validate_bachelier(forward, strike, normal_vol, expiry)?;
    let delta = match option_type {
        OptionType::Call => bachelier_delta_call(forward, strike, normal_vol, expiry),
        OptionType::Put => bachelier_delta_put(forward, strike, normal_vol, expiry),
    };
    ForwardGreeks::checked(
        delta,
        bachelier_gamma(forward, strike, normal_vol, expiry),
        bachelier_vega(forward, strike, normal_vol, expiry),
        "Bachelier",
    )
}

/// Undiscounted shifted (displaced) Black price for negative-rate markets.
///
/// Prices `Black(forward + shift, strike + shift, vol, expiry)` on a unit
/// annuity.
///
/// # Arguments
///
/// * `forward` - Forward rate at expiry (decimal; may be negative); finite.
/// * `strike` - Strike in the same units as `forward` (may be negative); finite.
/// * `vol` - Annualized shifted-lognormal volatility as a decimal; finite and
///   non-negative.
/// * `expiry` - Time to expiry in years; finite and non-negative.
/// * `shift` - Displacement added to both forward and strike, in the units of
///   `forward` (e.g. `0.03` for 3%); `forward + shift` and `strike + shift` must
///   be strictly positive.
/// * `option_type` - Call or put payoff.
///
/// # Returns
///
/// Undiscounted per-unit premium in the units of `forward`.
///
/// # Errors
///
/// Returns `Error::Validation` when an input is non-finite, a shifted forward
/// or strike is not positive, `vol` or `expiry` is negative, or the premium is
/// non-finite.
pub fn black_shifted_price(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
    option_type: OptionType,
) -> Result<f64> {
    validate_black_shifted(forward, strike, vol, expiry, shift)?;
    let (shifted_forward, shifted_strike) = (forward + shift, strike + shift);
    let value = match option_type {
        OptionType::Call => black_call(shifted_forward, shifted_strike, vol, expiry),
        OptionType::Put => black_put(shifted_forward, shifted_strike, vol, expiry),
    };
    checked_closed_form_value(value, "shifted Black price")
}

/// Undiscounted shifted (displaced) Black vega per unit (1.0) change in `vol`.
///
/// # Arguments
///
/// * `forward` - Forward rate at expiry (decimal; may be negative); finite.
/// * `strike` - Strike in the same units as `forward` (may be negative); finite.
/// * `vol` - Annualized shifted-lognormal volatility as a decimal; finite and
///   non-negative.
/// * `expiry` - Time to expiry in years; finite and non-negative.
/// * `shift` - Displacement added to both forward and strike, in the units of
///   `forward`; `forward + shift` and `strike + shift` must be strictly positive.
///
/// # Returns
///
/// Undiscounted vega in the units of `forward` per unit vol.
///
/// # Errors
///
/// Returns `Error::Validation` when an input is non-finite, a shifted forward
/// or strike is not positive, `vol` or `expiry` is negative, or the vega is
/// non-finite.
pub fn black_shifted_vega(
    forward: f64,
    strike: f64,
    vol: f64,
    expiry: f64,
    shift: f64,
) -> Result<f64> {
    validate_black_shifted(forward, strike, vol, expiry, shift)?;
    checked_closed_form_value(
        black_vega(forward + shift, strike + shift, vol, expiry),
        "shifted Black vega",
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::closed_form::implied_vol::black76_implied_vol;

    #[test]
    fn black76_price_matches_discounted_leaf_formula() {
        let (f, k, df, t, vol) = (100.0, 95.0, 0.97, 1.5, 0.25);
        assert_eq!(
            black76_price(f, k, df, t, vol, OptionType::Call).unwrap(),
            df * black_call(f, k, vol, t)
        );
        assert_eq!(
            black76_price(f, k, df, t, vol, OptionType::Put).unwrap(),
            df * black_put(f, k, vol, t)
        );
    }

    #[test]
    fn black76_price_rejects_non_positive_df_like_its_inverse() {
        for df in [0.0, -0.95, f64::NAN, f64::INFINITY] {
            let err = black76_price(100.0, 100.0, df, 1.0, 0.2, OptionType::Call).unwrap_err();
            assert!(matches!(err, Error::Validation(_)), "df={df}: {err:?}");
            assert!(
                black76_implied_vol(100.0, 100.0, df, 1.0, 7.5, OptionType::Call).is_err(),
                "inverse must reject df={df} too"
            );
        }
        // df > 1 (negative rates) is accepted by both directions.
        let price = black76_price(100.0, 100.0, 1.01, 1.0, 0.2, OptionType::Call).unwrap();
        let vol = black76_implied_vol(100.0, 100.0, 1.01, 1.0, price, OptionType::Call).unwrap();
        assert!((vol - 0.2).abs() < 1e-10);
    }

    #[test]
    fn black76_price_round_trips_through_implied_vol() {
        for option_type in [OptionType::Call, OptionType::Put] {
            let price = black76_price(100.0, 110.0, 0.95, 0.75, 0.3, option_type).unwrap();
            let vol = black76_implied_vol(100.0, 110.0, 0.95, 0.75, price, option_type).unwrap();
            assert!((vol - 0.3).abs() < 1e-10, "{option_type:?}: {vol}");
        }
    }

    #[test]
    fn black76_rejects_negative_vol_and_expiry_and_non_positive_forward() {
        assert!(black76_price(100.0, 100.0, 0.95, 1.0, -0.2, OptionType::Call).is_err());
        assert!(black76_price(100.0, 100.0, 0.95, -1.0, 0.2, OptionType::Call).is_err());
        assert!(black76_price(0.0, 100.0, 0.95, 1.0, 0.2, OptionType::Call).is_err());
        assert!(black76_greeks(100.0, -1.0, 1.0, 0.2, OptionType::Put).is_err());
        // Zero vol and zero expiry are the discounted intrinsic value.
        let intrinsic = black76_price(110.0, 100.0, 0.9, 0.0, 0.2, OptionType::Call).unwrap();
        assert!((intrinsic - 9.0).abs() < 1e-12);
    }

    #[test]
    fn forward_greeks_match_leaf_formulas() {
        let g = black76_greeks(100.0, 100.0, 1.0, 0.2, OptionType::Put).unwrap();
        assert_eq!(g.delta, black_delta_put(100.0, 100.0, 0.2, 1.0));
        assert_eq!(g.gamma, black_gamma(100.0, 100.0, 0.2, 1.0));
        assert_eq!(g.vega, black_vega(100.0, 100.0, 0.2, 1.0));
        let n = bachelier_greeks(-0.001, 0.002, 0.0075, 2.0, OptionType::Call).unwrap();
        assert_eq!(n.delta, bachelier_delta_call(-0.001, 0.002, 0.0075, 2.0));
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(serde_json::from_str::<ForwardGreeks>(&json).unwrap(), n);
    }

    #[test]
    fn bachelier_price_is_unit_annuity_twin_and_checks_domain() {
        let unit = bachelier_price(0.03, 0.025, 0.005, 2.0, OptionType::Put).unwrap();
        let annuity = crate::volatility::normal::bachelier_price_with_annuity(
            OptionType::Put,
            0.03,
            0.025,
            0.005,
            2.0,
            1.0,
        );
        assert_eq!(unit, annuity);
        assert!(bachelier_price(-0.01, -0.02, 0.005, 1.0, OptionType::Call).is_ok());
        assert!(bachelier_price(0.03, 0.03, -0.005, 1.0, OptionType::Call).is_err());
        assert!(bachelier_greeks(0.03, 0.03, 0.005, f64::NAN, OptionType::Call).is_err());
    }

    #[test]
    fn shifted_black_requires_positive_shifted_coordinates() {
        let price = black_shifted_price(-0.005, -0.005, 0.25, 1.0, 0.03, OptionType::Call).unwrap();
        assert_eq!(price, black_call(0.025, 0.025, 0.25, 1.0));
        assert!(black_shifted_price(-0.005, 0.01, 0.25, 1.0, 0.004, OptionType::Put).is_err());
        assert!(black_shifted_vega(0.01, -0.02, 0.25, 1.0, 0.01).is_err());
        assert!(black_shifted_vega(-0.005, -0.005, 0.25, 1.0, 0.03).unwrap() > 0.0);
    }
}
