//! Analytical pricing formulas for quanto options.
//!
//! This module provides closed-form solutions for vanilla quanto options under the
//! Black-Scholes framework with quanto drift adjustment.
//!
//! # Conventions
//!
//! | Parameter | Convention | Units |
//! |-----------|-----------|-------|
//! | Rates (r, q) | Continuously compounded | Decimal (0.05 = 5%) |
//! | Volatility (σ_asset, σ_fx) | Annualized | Decimal (0.20 = 20%) |
//! | Correlation (ρ) | Asset–FX correlation | Decimal (-1.0 to 1.0) |
//! | Time (T) | ACT/365-style | Years (1.0 = 1 year) |
//! | Prices | Per unit of underlying | Currency units |
//!
//! # References
//!
//! - Garman, M. B., & Kohlhagen, S. W. (1983), "Foreign Currency Option Values" `docs/REFERENCES.md#garman-kohlhagen-1983`
//! - Brigo, D., & Mercurio, F. (2006), "Interest Rate Models—Theory and Practice" `docs/REFERENCES.md#brigo-mercurio-2006-interest-rate-models`
//! - Hull, J. C., "Options, Futures, and Other Derivatives" (quanto adjustment overview) `docs/REFERENCES.md#hull-options-futures`
//!
//! # Quanto Adjustment
//!
//! A quanto option pays in a different currency than the underlying asset denomination.
//! The drift adjustment accounts for the correlation between the asset and FX rate:
//!
//! μ_quanto = μ_asset - ρ * σ_asset * σ_fx
//!
//! where:
//! - ρ is the correlation between asset and FX rate
//! - σ_asset is the asset volatility
//! - σ_fx is the FX rate volatility

use crate::types::{OptionMarketParams, OptionType};
use finstack_quant_core::math::special_functions::norm_cdf;

/// Compute the quanto drift adjustment.
///
/// # Arguments
///
/// * `correlation` - Correlation between asset and FX rate (ρ)
/// * `vol_asset` - Asset volatility (σ_S)
/// * `vol_fx` - FX rate volatility (σ_X)
///
/// # Returns
///
/// Quanto adjustment: -ρ * σ_S * σ_X
///
/// This adjustment is **subtracted** from the foreign risk-free rate in the BS formula.
#[inline]
fn quanto_drift_adjustment(correlation: f64, vol_asset: f64, vol_fx: f64) -> f64 {
    -correlation * vol_asset * vol_fx
}

/// Price a quanto option (closed-form).
///
/// # Arguments
///
/// * `market` - The foreign asset: `spot` and `strike` in foreign currency,
///   `rate` = the **domestic** risk-free rate and `dividend_yield` = the
///   asset's dividend yield in foreign terms (continuously compounded
///   decimals), `volatility` = the annualized asset volatility (decimal),
///   `time_to_expiry` in years and the call/put `option_type`.
/// * `rate_foreign` - Foreign risk-free rate, continuously compounded decimal
/// * `vol_fx` - Annualized FX rate volatility as a decimal
/// * `correlation` - Correlation between asset and FX rate, normally in `[-1, 1]`
///
/// # Returns
///
/// Option price in domestic currency (per unit of foreign notional); `NaN`
/// when an input or the result is non-finite.
///
/// # Formula
///
/// The quanto option is priced like a vanilla option with adjusted drift:
/// C_quanto = exp(-r_dom * T) * [F_adjusted * N(d1) - K * N(d2)]
/// P_quanto = exp(-r_dom * T) * [K * N(-d2) - F_adjusted * N(-d1)]
///
/// where:
/// - F_adjusted = S * exp((r_for - q - ρ*σ_S*σ_X) * T)
/// - d1 = [ln(S/K) + (r_for - q - ρ*σ_S*σ_X + σ_S²/2) * T] / (σ_S√T)
/// - d2 = d1 - σ_S√T
pub fn quanto_price(
    market: &OptionMarketParams,
    rate_foreign: f64,
    vol_fx: f64,
    correlation: f64,
) -> f64 {
    let &OptionMarketParams {
        spot,
        strike,
        rate: rate_domestic,
        volatility: vol_asset,
        time_to_expiry: time,
        dividend_yield: div_yield,
        option_type,
    } = market;
    if !spot.is_finite()
        || !strike.is_finite()
        || !time.is_finite()
        || !rate_domestic.is_finite()
        || !rate_foreign.is_finite()
        || !div_yield.is_finite()
        || !vol_asset.is_finite()
        || !vol_fx.is_finite()
        || !correlation.is_finite()
    {
        return f64::NAN;
    }
    if time <= 0.0 {
        return option_type.intrinsic(spot, strike);
    }
    if vol_asset <= 0.0 {
        let forward_adj = spot
            * ((rate_foreign - div_yield
                + quanto_drift_adjustment(correlation, vol_asset, vol_fx))
                * time)
                .exp();
        let forward_payoff = match option_type {
            OptionType::Call => forward_adj - strike,
            OptionType::Put => strike - forward_adj,
        };
        let price = forward_payoff * (-rate_domestic * time).exp();
        return if price.is_finite() {
            price.max(0.0)
        } else {
            f64::NAN
        };
    }

    let sqrt_t = time.sqrt();
    let vol_sqrt_t = vol_asset * sqrt_t;

    // Quanto-adjusted drift
    let quanto_adj = quanto_drift_adjustment(correlation, vol_asset, vol_fx);

    // Effective drift for the quanto measure
    let drift_adj = rate_foreign - div_yield + quanto_adj;

    // d1/d2 intentionally inline: Quanto-adjusted drift doesn't map cleanly to r, q params
    let d1 = ((spot / strike).ln() + (drift_adj + 0.5 * vol_asset * vol_asset) * time) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;

    let forward_adj = spot * (drift_adj * time).exp();

    // Discount at domestic rate
    let discount = (-rate_domestic * time).exp();

    let price = match option_type {
        OptionType::Call => discount * (forward_adj * norm_cdf(d1) - strike * norm_cdf(d2)),
        OptionType::Put => discount * (strike * norm_cdf(-d2) - forward_adj * norm_cdf(-d1)),
    };
    if price.is_finite() {
        price.max(0.0)
    } else {
        f64::NAN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quanto_drift_adjustment() {
        let adj = quanto_drift_adjustment(0.5, 0.2, 0.1);
        assert!((adj - (-0.01)).abs() < 1e-10);

        let adj_neg = quanto_drift_adjustment(-0.5, 0.2, 0.1);
        assert!((adj_neg - 0.01).abs() < 1e-10);
    }

    #[test]
    fn test_quanto_call_positive() {
        let price = quanto_price(
            &OptionMarketParams {
                spot: 100.0,
                strike: 100.0,
                rate: 0.05,
                volatility: 0.2,
                time_to_expiry: 1.0,
                dividend_yield: 0.01,
                option_type: OptionType::Call,
            },
            0.03,
            0.1,
            0.5,
        );
        assert!(price > 0.0);
        assert!(price < 100.0);
    }

    #[test]
    fn test_quanto_put_positive() {
        let price = quanto_price(
            &OptionMarketParams {
                spot: 100.0,
                strike: 100.0,
                rate: 0.05,
                volatility: 0.2,
                time_to_expiry: 1.0,
                dividend_yield: 0.01,
                option_type: OptionType::Put,
            },
            0.03,
            0.1,
            0.5,
        );
        assert!(price > 0.0);
        assert!(price < 100.0);
    }

    #[test]
    fn test_quanto_zero_correlation_vs_vanilla() {
        // With zero correlation and r_dom = r_for, should match vanilla BS
        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let rate = 0.05;
        let div_yield = 0.02;
        let vol = 0.2;

        let quanto_call_price = quanto_price(
            &OptionMarketParams {
                spot,
                strike,
                rate,
                volatility: vol,
                time_to_expiry: time,
                dividend_yield: div_yield,
                option_type: OptionType::Call,
            },
            rate,
            0.1,
            0.0,
        );

        let sqrt_t = time.sqrt();
        let d1 =
            ((spot / strike).ln() + (rate - div_yield + 0.5 * vol * vol) * time) / (vol * sqrt_t);
        let d2 = d1 - vol * sqrt_t;
        let vanilla = spot * (-div_yield * time).exp() * norm_cdf(d1)
            - strike * (-rate * time).exp() * norm_cdf(d2);

        assert!(
            (quanto_call_price - vanilla).abs() < 0.01,
            "Quanto with zero corr {} should match vanilla {}",
            quanto_call_price,
            vanilla
        );
    }

    #[test]
    fn test_quanto_put_call_parity() {
        // Modified put-call parity for quanto:
        // C - P = exp(-r_dom*T) * [F_adjusted - K]
        // where F_adjusted = S * exp((r_for - q + quanto_adj) * T)

        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let r_dom = 0.05;
        let r_for = 0.03;
        let div_yield = 0.01;
        let vol_asset = 0.2;
        let vol_fx = 0.1;
        let correlation = 0.5;

        let call = quanto_price(
            &OptionMarketParams {
                spot,
                strike,
                rate: r_dom,
                volatility: vol_asset,
                time_to_expiry: time,
                dividend_yield: div_yield,
                option_type: OptionType::Call,
            },
            r_for,
            vol_fx,
            correlation,
        );
        let put = quanto_price(
            &OptionMarketParams {
                spot,
                strike,
                rate: r_dom,
                volatility: vol_asset,
                time_to_expiry: time,
                dividend_yield: div_yield,
                option_type: OptionType::Put,
            },
            r_for,
            vol_fx,
            correlation,
        );

        let quanto_adj = quanto_drift_adjustment(correlation, vol_asset, vol_fx);
        let forward_adj = spot * ((r_for - div_yield + quanto_adj) * time).exp();
        let discount = (-r_dom * time).exp();

        let lhs = call - put;
        let rhs = discount * (forward_adj - strike);

        assert!(
            (lhs - rhs).abs() < 0.01,
            "Quanto put-call parity failed: {} vs {}",
            lhs,
            rhs
        );
    }

    #[test]
    fn test_quanto_negative_correlation_higher_call() {
        // Negative correlation means when asset goes up, FX goes down
        // This increases the quanto-adjusted drift, making calls more valuable
        let spot = 100.0;
        let strike = 100.0;
        let time = 1.0;
        let r_dom = 0.05;
        let r_for = 0.03;
        let div_yield = 0.01;
        let vol_asset = 0.2;
        let vol_fx = 0.1;

        let call_neg_corr = quanto_price(
            &OptionMarketParams {
                spot,
                strike,
                rate: r_dom,
                volatility: vol_asset,
                time_to_expiry: time,
                dividend_yield: div_yield,
                option_type: OptionType::Call,
            },
            r_for,
            vol_fx,
            -0.5,
        );
        let call_pos_corr = quanto_price(
            &OptionMarketParams {
                spot,
                strike,
                rate: r_dom,
                volatility: vol_asset,
                time_to_expiry: time,
                dividend_yield: div_yield,
                option_type: OptionType::Call,
            },
            r_for,
            vol_fx,
            0.5,
        );

        assert!(
            call_neg_corr > call_pos_corr,
            "Negative correlation call {} should be > positive correlation call {}",
            call_neg_corr,
            call_pos_corr
        );
    }
}
