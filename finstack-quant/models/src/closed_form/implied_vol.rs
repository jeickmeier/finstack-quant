//! Implied volatility solvers for vanilla option models.
//!
//! Provides economic adapters to the shared Black implied-volatility solver:
//! - Black–Scholes / Garman–Kohlhagen (spot-based, with `r` and `q`)
//! - Black-76 (forward-based, with discount factor `df`)
//!
//! These are intended as shared utilities used by instrument-specific `implied_vol` methods
//! (e.g., equity and FX options) to avoid duplicated solvers and inconsistent edge handling.

use finstack_quant_core::Result;

use crate::types::OptionType;
use crate::volatility::implied_vol_black;

/// Error returned when implied volatility inputs contain non-finite values.
const NON_FINITE_MSG: &str = "Implied volatility solver received non-finite input parameters";

fn check_positive_implied_vol_inputs(
    all_finite: bool,
    expiry: f64,
    positive_inputs: &[f64],
    positive_msg: &str,
) -> Result<()> {
    if !all_finite {
        return Err(finstack_quant_core::Error::Validation(
            NON_FINITE_MSG.into(),
        ));
    }
    if expiry <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "implied vol requires a positive time to expiry, got {expiry}; an expired option \
             has no implied volatility"
        )));
    }
    if positive_inputs.iter().any(|&v| v <= 0.0) {
        return Err(finstack_quant_core::Error::Validation(positive_msg.into()));
    }
    Ok(())
}

fn invert_discounted_black(
    discounted_forward: f64,
    discounted_strike: f64,
    expiry: f64,
    price: f64,
    option_type: OptionType,
    sub_intrinsic_msg: impl FnOnce(f64) -> String,
) -> Result<f64> {
    if !discounted_forward.is_finite() || !discounted_strike.is_finite() {
        return Err(finstack_quant_core::Error::Validation(
            NON_FINITE_MSG.into(),
        ));
    }
    let intrinsic = option_type.intrinsic(discounted_forward, discounted_strike);
    if price <= intrinsic {
        return Err(finstack_quant_core::Error::Validation(sub_intrinsic_msg(
            intrinsic,
        )));
    }
    implied_vol_black(
        price,
        discounted_forward,
        discounted_strike,
        expiry,
        option_type == OptionType::Call,
    )
}

/// Solve for Black–Scholes / Garman–Kohlhagen implied volatility.
///
/// Finds \(\sigma\) such that `bs_price(&market_with_volatility) == price`.
///
/// - `price` is the **per-unit** option price (not contract-scaled).
/// - Returns `Err` for non-finite inputs, non-positive `expiry` (an expired
///   option has no implied volatility), non-positive `spot`/`strike`/`price`,
///   or when the target cannot be bracketed.
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option's price units.
/// * `strike` - Exercise price in the same units as `spot`.
/// * `rate` - Continuously compounded domestic risk-free rate as a decimal.
/// * `div_yield` - Continuously compounded dividend yield or foreign-rate carry as a
///   decimal.
/// * `expiry` - Remaining time to expiry in years; must be strictly positive.
/// * `price` - Observed per-unit option premium to match, excluding any
///   contract multiplier.
/// * `option_type` - Call or put payoff convention to invert.
pub fn bs_implied_vol(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    expiry: f64,
    price: f64,
    option_type: OptionType,
) -> Result<f64> {
    check_positive_implied_vol_inputs(
        spot.is_finite()
            && strike.is_finite()
            && rate.is_finite()
            && div_yield.is_finite()
            && expiry.is_finite()
            && price.is_finite(),
        expiry,
        &[spot, strike, price],
        "implied vol requires positive spot, strike, and price",
    )?;

    // Black prices are homogeneous in forward and strike. Discounting both
    // coordinates keeps the target in its original premium units and avoids
    // constructing a potentially overflowing forward S * exp((r - q) * T).
    invert_discounted_black(
        spot * (-div_yield * expiry).exp(),
        strike * (-rate * expiry).exp(),
        expiry,
        price,
        option_type,
        |intrinsic| {
            format!(
                "Implied vol: target price {price:.6} is at or below intrinsic value \
                 {intrinsic:.6} for spot={spot}, strike={strike}, rate={rate}, div_yield={div_yield}, expiry={expiry}. \
                 A positive-volatility inversion requires a premium strictly above discounted intrinsic."
            )
        },
    )
}

/// Solve for Black-76 implied volatility (forward-based).
///
/// Finds \(\sigma\) such that:
/// `df * bs_price(&market_with_zero_rates) == price`.
///
/// - `price` is the **per-unit** option price (not contract-scaled).
/// - Returns `Err` for non-finite inputs, non-positive `expiry` (an expired
///   option has no implied volatility), non-positive `forward`/`strike`/`df`/`price`,
///   or when the target cannot be bracketed.
///
/// # Arguments
///
/// * `forward` - Forward price or rate at expiry in the option's quote units.
/// * `strike` - Exercise price or rate in the same units as `forward`.
/// * `df` - Discount factor from valuation date to expiry.
/// * `expiry` - Remaining time to expiry in years; must be strictly positive.
/// * `price` - Observed discounted per-unit option premium to match.
/// * `option_type` - Call or put payoff convention to invert.
pub fn black76_implied_vol(
    forward: f64,
    strike: f64,
    df: f64,
    expiry: f64,
    price: f64,
    option_type: OptionType,
) -> Result<f64> {
    check_positive_implied_vol_inputs(
        forward.is_finite()
            && strike.is_finite()
            && df.is_finite()
            && expiry.is_finite()
            && price.is_finite(),
        expiry,
        &[forward, strike, df, price],
        "implied vol requires positive forward, strike, df, and price",
    )?;

    invert_discounted_black(
        df * forward,
        df * strike,
        expiry,
        price,
        option_type,
        |_| "Implied vol requires a premium strictly above discounted intrinsic".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closed_form::bs_price_unchecked;

    #[test]
    fn solver_only_returns_ok_for_genuinely_converged_solutions() {
        let cases = [
            // (spot, strike, r, q, t, vol_used_to_make_target)
            (100.0, 100.0, 0.05, 0.02, 1.0, 0.20),
            (100.0, 80.0, 0.03, 0.0, 0.5, 0.45),
            (100.0, 130.0, 0.06, 0.01, 2.0, 0.65),
            (100.0, 100.0, 0.0, 0.0, 0.1, 0.10),
            (50.0, 55.0, 0.08, 0.0, 0.25, 0.80),
        ];
        for &(spot, strike, r, q, t, vol) in &cases {
            for option_type in [OptionType::Call, OptionType::Put] {
                let target = bs_price_unchecked(spot, strike, r, q, vol, t, option_type);
                let solved = bs_implied_vol(spot, strike, r, q, t, target, option_type)
                    .expect("a price generated from a real vol must invert");
                let repriced = bs_price_unchecked(spot, strike, r, q, solved, t, option_type);
                assert!(
                    (repriced - target).abs() <= 1e-6 * target.max(1.0),
                    "solver returned a non-converged Ok: vol={vol} solved={solved} \
                     target={target} repriced={repriced}"
                );
            }
        }
    }

    #[test]
    fn sub_intrinsic_prices_are_rejected() {
        let intrinsic =
            (100.0_f64 * (-0.0_f64 * 1.0).exp() - 80.0_f64 * (-0.05_f64 * 1.0).exp()).max(0.0);
        let err = bs_implied_vol(
            100.0,
            80.0,
            0.05,
            0.0,
            1.0,
            intrinsic * 0.5,
            OptionType::Call,
        )
        .expect_err("sub-intrinsic target must not yield a silent Ok");
        assert!(
            matches!(err, finstack_quant_core::Error::Validation(_)),
            "non-solvable implied-vol request must be a Validation error, got {err:?}"
        );
    }

    #[test]
    fn economic_adapters_share_the_black_kernel_across_scales() {
        for scale in [1e-12, 1.0, 1e12] {
            for (rate, carry, expiry, vol) in [
                (-0.05, 0.02, 0.01, 0.4),
                (0.08, -0.02, 1.0, 0.2),
                (0.02, 0.03, 2.0, 1.5),
            ] {
                for option in [OptionType::Call, OptionType::Put] {
                    let spot = 100.0 * scale;
                    let strike = 105.0 * scale;
                    let price = bs_price_unchecked(spot, strike, rate, carry, vol, expiry, option);
                    let bs = bs_implied_vol(spot, strike, rate, carry, expiry, price, option)
                        .expect("BS inverse");
                    let df = (-rate * expiry).exp();
                    let forward = spot * ((rate - carry) * expiry).exp();
                    let black = black76_implied_vol(forward, strike, df, expiry, price, option)
                        .expect("Black inverse");
                    assert!((bs - vol).abs() < 1e-10, "{bs} vs {vol}");
                    assert!((black - vol).abs() < 1e-10, "{black} vs {vol}");
                }
            }
        }
    }

    #[test]
    fn adapters_reject_nonfinite_transformed_coordinates() {
        assert!(bs_implied_vol(100.0, 100.0, -1000.0, 0.0, 1.0, 10.0, OptionType::Put).is_err());
        assert!(black76_implied_vol(1e308, 1e308, 10.0, 1.0, 1.0, OptionType::Call).is_err());
    }
}
