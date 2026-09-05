use super::params::HESTON_TAIL_DIAGNOSTIC_THRESHOLD;
use super::quadrature::{
    composite_gauss_legendre_grid, heston_pj_on_grid, heston_pj_with_diagnostics,
};
use super::{HestonFourierSettings, HestonPricingParams, HestonStripPricer};
use crate::closed_form::vanilla::bs_price_unchecked;
use crate::types::OptionType;
use finstack_quant_core::{Error, Result};
use tracing::warn;

fn resolve_heston_settings(
    time: f64,
    params: &HestonPricingParams,
    settings: Option<&HestonFourierSettings>,
) -> HestonFourierSettings {
    settings
        .copied()
        .unwrap_or_else(|| HestonFourierSettings::for_maturity_with_variance(time, params.v0))
}

/// Price a European call option under the Heston model using Fourier inversion.
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option quote currency.
/// * `strike` - Exercise price in the same units as `spot`.
/// * `time` - Remaining time to maturity in years.
/// * `params` - Validated Heston rate, carry, variance, mean-reversion,
///   volatility-of-variance, and correlation parameters.
/// * `settings` - Optional Fourier integration grid, truncation, and damping
///   settings. `None` uses [`HestonFourierSettings::for_maturity_with_variance`]
///   so short-dated and low-variance options get a wider/finer grid.
///
/// # Returns
///
/// Call option price
///
/// # Formula
///
/// C = S * exp(-qT) * P1 - K * exp(-rT) * P2
///
/// where P1 and P2 are risk-neutral probabilities computed via Fourier inversion.
///
/// # Example
///
/// ```text
/// use finstack_quant_models::closed_form::heston::{
///     heston_call_price_fourier, HestonPricingParams,
/// };
///
/// let params = HestonPricingParams::new(
///     0.05,  // risk-free rate
///     0.02,  // dividend yield
///     2.0,   // kappa (mean reversion)
///     0.04,  // theta (long-run variance)
///     0.3,   // sigma_v (vol-of-vol)
///     -0.7,  // rho (correlation)
///     0.04,  // v0 (initial variance)
/// )
/// .unwrap();
///
/// let price = heston_call_price_fourier(100.0, 100.0, 1.0, &params, None).unwrap();
/// assert!(price > 0.0 && price < 100.0);
/// ```
pub fn heston_call_price_fourier(
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonPricingParams,
    settings: Option<&HestonFourierSettings>,
) -> Result<f64> {
    if time <= 0.0 {
        return Ok((spot - strike).max(0.0));
    }

    // This is the exact sigma_v -> 0 Heston limit, not a numerical fallback.
    if params.sigma_v < 1e-10 {
        return Ok(black_scholes_call(
            spot,
            strike,
            time,
            params.r,
            params.q,
            params.deterministic_avg_variance(time).sqrt(),
        ));
    }

    let initial = resolve_heston_settings(time, params, settings);
    let retry = HestonFourierSettings {
        u_max: initial.u_max * 2.0,
        panels: initial.panels.saturating_mul(2),
        ..initial
    };
    for attempt in [initial, retry] {
        if let Some(price) = heston_call_attempt(spot, strike, time, params, attempt) {
            return Ok(price);
        }
    }

    Err(Error::Calibration {
        category: "heston_fourier".to_string(),
        message: format!(
            "Heston Fourier integration failed after 2 attempts for spot={spot}, \
             strike={strike}, time={time}; characteristic-function corruption or \
             a non-finite integral persisted"
        ),
    })
}

fn heston_call_attempt(
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonPricingParams,
    settings: HestonFourierSettings,
) -> Option<f64> {
    let grid =
        composite_gauss_legendre_grid(0.0, settings.u_max, settings.gl_order, settings.panels);
    let (d1, d2) = match &grid {
        Some(grid) => (
            heston_pj_on_grid(1, spot, strike, time, params, &settings, grid),
            heston_pj_on_grid(2, spot, strike, time, params, &settings, grid),
        ),
        None => (
            heston_pj_with_diagnostics(1, spot, strike, time, params, &settings),
            heston_pj_with_diagnostics(2, spot, strike, time, params, &settings),
        ),
    };
    if d1.corrupted || d2.corrupted {
        return None;
    }

    let tail = d1.tail_estimate.max(d2.tail_estimate);
    let raw_p1_excursion = (d1.raw_probability - d1.raw_probability.clamp(0.0, 1.0)).abs();
    let raw_p2_excursion = (d2.raw_probability - d2.raw_probability.clamp(0.0, 1.0)).abs();
    let raw_excursion = raw_p1_excursion.max(raw_p2_excursion);
    if tail > HESTON_TAIL_DIAGNOSTIC_THRESHOLD || raw_excursion > HESTON_TAIL_DIAGNOSTIC_THRESHOLD {
        warn!(
            spot,
            strike,
            time,
            u_max = settings.u_max,
            tail_estimate = tail,
            raw_probability_excursion = raw_excursion,
            "Heston Gil-Pelaez integral has a non-negligible truncation diagnostic"
        );
    }

    let call_price = spot * (-params.q * time).exp() * d1.probability
        - strike * (-params.r * time).exp() * d2.probability;
    call_price.is_finite().then(|| call_price.max(0.0))
}

/// Price a strip of European call options under the Heston model using shared
/// characteristic-function precomputation.
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option quote currency.
/// * `strikes` - Exercise prices in result order, each in the same units as
///   `spot`; the returned vector has the same length and order.
/// * `time` - Common remaining time to expiry in years.
/// * `params` - Validated Heston rate, carry, variance, mean-reversion,
///   volatility-of-variance, and correlation parameters.
/// * `settings` - Optional Fourier integration settings applied consistently
///   to the whole strike strip. `None` uses
///   [`HestonFourierSettings::for_maturity_with_variance`].
pub fn heston_call_prices_fourier(
    spot: f64,
    strikes: &[f64],
    time: f64,
    params: &HestonPricingParams,
    settings: Option<&HestonFourierSettings>,
) -> Result<Vec<f64>> {
    if time <= 0.0 {
        return Ok(strikes
            .iter()
            .map(|&strike| (spot - strike).max(0.0))
            .collect());
    }

    if params.sigma_v < 1e-10 {
        let avg_vol = params.deterministic_avg_variance(time).sqrt();
        return Ok(strikes
            .iter()
            .map(|&strike| black_scholes_call(spot, strike, time, params.r, params.q, avg_vol))
            .collect());
    }

    let initial = resolve_heston_settings(time, params, settings);
    let retry = HestonFourierSettings {
        u_max: initial.u_max * 2.0,
        panels: initial.panels.saturating_mul(2),
        ..initial
    };
    for attempt in [initial, retry] {
        let Some(pricer) = HestonStripPricer::new(spot, time, params, &attempt) else {
            continue;
        };
        if let Ok(prices) = pricer.price_calls(strikes) {
            return Ok(prices);
        }
    }

    strikes
        .iter()
        .map(|&strike| heston_call_price_fourier(spot, strike, time, params, Some(&retry)))
        .collect()
}

/// Price a strip of European put options under the Heston model using shared
/// characteristic-function precomputation.
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option quote currency.
/// * `strikes` - Exercise prices in result order, each in the same units as
///   `spot`; the returned vector has the same length and order.
/// * `time` - Common remaining time to expiry in years.
/// * `params` - Validated Heston rate, carry, variance, mean-reversion,
///   volatility-of-variance, and correlation parameters.
/// * `settings` - Optional Fourier integration settings applied consistently
///   to the whole strike strip. `None` uses
///   [`HestonFourierSettings::for_maturity_with_variance`].
pub fn heston_put_prices_fourier(
    spot: f64,
    strikes: &[f64],
    time: f64,
    params: &HestonPricingParams,
    settings: Option<&HestonFourierSettings>,
) -> Result<Vec<f64>> {
    if time <= 0.0 {
        return Ok(strikes
            .iter()
            .map(|&strike| (strike - spot).max(0.0))
            .collect());
    }

    let call_prices = heston_call_prices_fourier(spot, strikes, time, params, settings)?;
    Ok(call_prices
        .into_iter()
        .zip(strikes.iter())
        .map(|(call_price, strike)| {
            let forward = spot * (-params.q * time).exp();
            let discount_k = *strike * (-params.r * time).exp();
            (call_price - forward + discount_k).max(0.0)
        })
        .collect())
}

/// Price a European put option under the Heston model using Fourier inversion.
///
/// # Arguments
///
/// * `spot` - Current underlying spot price in the option quote currency.
/// * `strike` - Exercise price in the same units as `spot`.
/// * `time` - Remaining time to maturity in years.
/// * `params` - Validated Heston rate, carry, variance, mean-reversion,
///   volatility-of-variance, and correlation parameters.
/// * `settings` - Optional Fourier integration settings. `None` uses
///   [`HestonFourierSettings::for_maturity_with_variance`].
///
/// # Returns
///
/// Put option price
///
/// # Formula
///
/// Uses put-call parity: P = C - S*exp(-qT) + K*exp(-rT)
pub fn heston_put_price_fourier(
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonPricingParams,
    settings: Option<&HestonFourierSettings>,
) -> Result<f64> {
    if time <= 0.0 {
        return Ok((strike - spot).max(0.0));
    }

    let call_price = heston_call_price_fourier(spot, strike, time, params, settings)?;
    let forward = spot * (-params.q * time).exp();
    let discount_k = strike * (-params.r * time).exp();
    let put_price = call_price - forward + discount_k;
    if !put_price.is_finite() {
        return Err(Error::Calibration {
            category: "heston_fourier".to_string(),
            message: format!(
                "Heston put-call parity produced a non-finite price for spot={spot}, \
                 strike={strike}, time={time}"
            ),
        });
    }
    Ok(put_price.max(0.0))
}

/// Black-Scholes call price for the exact deterministic-variance limit.
pub(super) fn black_scholes_call(
    spot: f64,
    strike: f64,
    time: f64,
    r: f64,
    q: f64,
    vol: f64,
) -> f64 {
    bs_price_unchecked(spot, strike, r, q, vol, time, OptionType::Call)
}
