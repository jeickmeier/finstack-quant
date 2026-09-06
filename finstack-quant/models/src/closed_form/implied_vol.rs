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

/// Solve for Black–Scholes / Garman–Kohlhagen implied volatility.
///
/// Finds \(\sigma\) such that `bs_price(spot, strike, rate, div_yield, vol, expiry, option_type) == target_price`.
///
/// - `target_price` is the **per-unit** option price (not contract-scaled).
/// - Returns `Err` for non-finite inputs, non-positive `expiry` (an expired
///   option has no implied volatility), non-positive `spot`/`strike`/`target_price`,
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
/// * `option_type` - Call or put payoff convention to invert.
/// * `target_price` - Observed per-unit option premium to match, excluding
///   any contract multiplier.
#[allow(clippy::too_many_arguments)]
pub fn bs_implied_vol(
    spot: f64,
    strike: f64,
    rate: f64,
    div_yield: f64,
    expiry: f64,
    option_type: OptionType,
    target_price: f64,
) -> Result<f64> {
    if !spot.is_finite()
        || !strike.is_finite()
        || !rate.is_finite()
        || !div_yield.is_finite()
        || !expiry.is_finite()
        || !target_price.is_finite()
    {
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
    if target_price <= 0.0 || spot <= 0.0 || strike <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "implied vol requires positive spot, strike, and target_price".into(),
        ));
    }

    // Intrinsic lower bound (per unit) for continuous compounding.
    let intrinsic = match option_type {
        OptionType::Call => {
            (spot * (-div_yield * expiry).exp() - strike * (-rate * expiry).exp()).max(0.0)
        }
        OptionType::Put => {
            (strike * (-rate * expiry).exp() - spot * (-div_yield * expiry).exp()).max(0.0)
        }
    };
    if target_price <= intrinsic {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Implied vol: target price {target_price:.6} is at or below intrinsic value \
             {intrinsic:.6} for spot={spot}, strike={strike}, rate={rate}, div_yield={div_yield}, expiry={expiry}. \
             A positive-volatility inversion requires a premium strictly above discounted intrinsic."
        )));
    }

    // Black prices are homogeneous in forward and strike. Discounting both
    // coordinates keeps the target in its original premium units and avoids
    // constructing a potentially overflowing forward S * exp((r - q) * T).
    implied_vol_black(
        target_price,
        spot * (-div_yield * expiry).exp(),
        strike * (-rate * expiry).exp(),
        expiry,
        option_type == OptionType::Call,
    )
}

/// Solve for Black-76 implied volatility (forward-based).
///
/// Finds \(\sigma\) such that:
/// `df * bs_price(forward, strike, 0, 0, sigma, expiry, option_type) == target_price`.
///
/// - `target_price` is the **per-unit** option price (not contract-scaled).
/// - Returns `Err` for non-finite inputs, non-positive `expiry` (an expired
///   option has no implied volatility), non-positive `forward`/`strike`/`df`/`target_price`,
///   or when the target cannot be bracketed.
///
/// # Arguments
///
/// * `forward` - Forward price or rate at expiry in the option's quote units.
/// * `strike` - Exercise price or rate in the same units as `forward`.
/// * `df` - Discount factor from valuation date to expiry.
/// * `expiry` - Remaining time to expiry in years; must be strictly positive.
/// * `option_type` - Call or put payoff convention to invert.
/// * `target_price` - Observed discounted per-unit option premium to match.
pub fn black76_implied_vol(
    forward: f64,
    strike: f64,
    df: f64,
    expiry: f64,
    option_type: OptionType,
    target_price: f64,
) -> Result<f64> {
    if !forward.is_finite()
        || !strike.is_finite()
        || !df.is_finite()
        || !expiry.is_finite()
        || !target_price.is_finite()
    {
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
    if target_price <= 0.0 || forward <= 0.0 || strike <= 0.0 || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "implied vol requires positive forward, strike, df, and target_price".into(),
        ));
    }

    let intrinsic = match option_type {
        OptionType::Call => (forward - strike).max(0.0) * df,
        OptionType::Put => (strike - forward).max(0.0) * df,
    };
    if target_price <= intrinsic {
        return Err(finstack_quant_core::Error::Validation(
            "Implied vol requires a premium strictly above discounted intrinsic".into(),
        ));
    }

    implied_vol_black(
        target_price,
        df * forward,
        df * strike,
        expiry,
        option_type == OptionType::Call,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closed_form::vanilla::bs_price_unchecked;

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
                let solved = bs_implied_vol(spot, strike, r, q, t, option_type, target)
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
            OptionType::Call,
            intrinsic * 0.5,
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
                    let bs = bs_implied_vol(spot, strike, rate, carry, expiry, option, price)
                        .expect("BS inverse");
                    let df = (-rate * expiry).exp();
                    let forward = spot * ((rate - carry) * expiry).exp();
                    let black = black76_implied_vol(forward, strike, df, expiry, option, price)
                        .expect("Black inverse");
                    assert!((bs - vol).abs() < 1e-10, "{bs} vs {vol}");
                    assert!((black - vol).abs() < 1e-10, "{black} vs {vol}");
                }
            }
        }
    }

    #[test]
    fn adapters_reject_nonfinite_transformed_coordinates() {
        assert!(bs_implied_vol(100.0, 100.0, -1000.0, 0.0, 1.0, OptionType::Put, 10.0).is_err());
        assert!(black76_implied_vol(1e308, 1e308, 10.0, 1.0, OptionType::Call, 1.0).is_err());
    }
}
