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
//! - Garman, M. B., & Kohlhagen, S. W. (1983), "Foreign Currency Option Values"
//! - Brigo, D., & Mercurio, F. (2006), "Interest Rate Models—Theory and Practice"
//! - Hull, J. C., "Options, Futures, and Other Derivatives" (quanto adjustment overview)
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

use finstack_core::math::special_functions::norm_cdf;

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
#[must_use]
pub fn quanto_drift_adjustment(correlation: f64, vol_asset: f64, vol_fx: f64) -> f64 {
    -correlation * vol_asset * vol_fx
}

/// Price a quanto call option (closed-form).
///
/// # Arguments
///
/// * `spot` - Current spot price (in foreign currency)
/// * `strike` - Strike price (in foreign currency)
/// * `time` - Time to maturity (years)
/// * `rate_domestic` - Domestic risk-free rate
/// * `rate_foreign` - Foreign risk-free rate
/// * `div_yield` - Dividend yield (in foreign terms)
/// * `vol_asset` - Asset volatility
/// * `vol_fx` - FX rate volatility
/// * `correlation` - Correlation between asset and FX rate
///
/// # Returns
///
/// Call option price in domestic currency (per unit of foreign notional)
///
/// # Formula
///
/// The quanto call is priced like a vanilla call with adjusted drift:
/// C_quanto = exp(-r_dom * T) * [F_adjusted * N(d1) - K * N(d2)]
///
/// where:
/// - F_adjusted = S * exp((r_for - q - ρ*σ_S*σ_X) * T)
/// - d1 = [ln(S/K) + (r_for - q - ρ*σ_S*σ_X + σ_S²/2) * T] / (σ_S√T)
/// - d2 = d1 - σ_S√T
#[allow(clippy::too_many_arguments)]
pub fn quanto_call(
    spot: f64,
    strike: f64,
    time: f64,
    rate_domestic: f64,
    rate_foreign: f64,
    div_yield: f64,
    vol_asset: f64,
    vol_fx: f64,
    correlation: f64,
) -> f64 {
    if time <= 0.0 {
        return (spot - strike).max(0.0);
    }
    if vol_asset <= 0.0 {
        let forward_adj = spot
            * ((rate_foreign - div_yield
                + quanto_drift_adjustment(correlation, vol_asset, vol_fx))
                * time)
                .exp();
        return ((forward_adj - strike) * (-rate_domestic * time).exp()).max(0.0);
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

    // Forward adjusted for quanto
    let forward_adj = spot * (drift_adj * time).exp();

    // Discount at domestic rate
    let discount = (-rate_domestic * time).exp();

    discount * (forward_adj * norm_cdf(d1) - strike * norm_cdf(d2))
}

/// Price a quanto put option (closed-form).
///
/// # Arguments
///
/// Same as `quanto_call`.
///
/// # Returns
///
/// Put option price in domestic currency (per unit of foreign notional)
#[allow(clippy::too_many_arguments)]
pub fn quanto_put(
    spot: f64,
    strike: f64,
    time: f64,
    rate_domestic: f64,
    rate_foreign: f64,
    div_yield: f64,
    vol_asset: f64,
    vol_fx: f64,
    correlation: f64,
) -> f64 {
    if time <= 0.0 {
        return (strike - spot).max(0.0);
    }
    if vol_asset <= 0.0 {
        let forward_adj = spot
            * ((rate_foreign - div_yield
                + quanto_drift_adjustment(correlation, vol_asset, vol_fx))
                * time)
                .exp();
        return ((strike - forward_adj) * (-rate_domestic * time).exp()).max(0.0);
    }

    let sqrt_t = time.sqrt();
    let vol_sqrt_t = vol_asset * sqrt_t;

    let quanto_adj = quanto_drift_adjustment(correlation, vol_asset, vol_fx);
    let drift_adj = rate_foreign - div_yield + quanto_adj;

    let d1 = ((spot / strike).ln() + (drift_adj + 0.5 * vol_asset * vol_asset) * time) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;

    let forward_adj = spot * (drift_adj * time).exp();
    let discount = (-rate_domestic * time).exp();

    discount * (strike * norm_cdf(-d2) - forward_adj * norm_cdf(-d1))
}

/// Price a quanto call with simplified parameters (FX vol and correlation defaulted).
///
/// This is a convenience wrapper when FX volatility and correlation are not explicitly provided.
/// Uses reasonable defaults or zero correlation.
pub fn quanto_call_simple(
    spot: f64,
    strike: f64,
    time: f64,
    rate_domestic: f64,
    rate_foreign: f64,
    div_yield: f64,
    vol_asset: f64,
) -> f64 {
    // Zero correlation assumption (no quanto adjustment)
    quanto_call(
        spot,
        strike,
        time,
        rate_domestic,
        rate_foreign,
        div_yield,
        vol_asset,
        0.0,
        0.0,
    )
}

/// Price a quanto put with simplified parameters.
pub fn quanto_put_simple(
    spot: f64,
    strike: f64,
    time: f64,
    rate_domestic: f64,
    rate_foreign: f64,
    div_yield: f64,
    vol_asset: f64,
) -> f64 {
    quanto_put(
        spot,
        strike,
        time,
        rate_domestic,
        rate_foreign,
        div_yield,
        vol_asset,
        0.0,
        0.0,
    )
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
        let price = quanto_call(100.0, 100.0, 1.0, 0.05, 0.03, 0.01, 0.2, 0.1, 0.5);
        assert!(price > 0.0);
        assert!(price < 100.0);
    }

    #[test]
    fn test_quanto_put_positive() {
        let price = quanto_put(100.0, 100.0, 1.0, 0.05, 0.03, 0.01, 0.2, 0.1, 0.5);
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

        let quanto_call_price =
            quanto_call(spot, strike, time, rate, rate, div_yield, vol, 0.1, 0.0);

        // Vanilla BS call
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

        let call = quanto_call(
            spot,
            strike,
            time,
            r_dom,
            r_for,
            div_yield,
            vol_asset,
            vol_fx,
            correlation,
        );
        let put = quanto_put(
            spot,
            strike,
            time,
            r_dom,
            r_for,
            div_yield,
            vol_asset,
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

        let call_neg_corr = quanto_call(
            spot, strike, time, r_dom, r_for, div_yield, vol_asset, vol_fx, -0.5,
        );
        let call_pos_corr = quanto_call(
            spot, strike, time, r_dom, r_for, div_yield, vol_asset, vol_fx, 0.5,
        );

        assert!(
            call_neg_corr > call_pos_corr,
            "Negative correlation call {} should be > positive correlation call {}",
            call_neg_corr,
            call_pos_corr
        );
    }
}
