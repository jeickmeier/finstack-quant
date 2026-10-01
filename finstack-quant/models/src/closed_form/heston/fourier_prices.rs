use super::params::HESTON_TAIL_LOG_TARGET;
use super::{HestonFourierSettings, HestonPricingParams, HestonStripPricer};
#[cfg(test)]
use crate::closed_form::vanilla::bs_price_unchecked;
#[cfg(test)]
use crate::types::OptionType;
use finstack_quant_core::{Error, Result};

fn resolve_heston_settings(
    time: f64,
    params: &HestonPricingParams,
    settings: Option<&HestonFourierSettings>,
) -> HestonFourierSettings {
    if let Some(settings) = settings {
        return *settings;
    }
    let mut settings =
        HestonFourierSettings::for_maturity_with_variance(time, params.v0.min(params.theta));
    // Include the large-frequency Heston exponential tail alongside the
    // Gaussian short-time bound. Neither bound alone covers both regimes.
    let c_inf = (1.0 - params.rho * params.rho).sqrt()
        * (params.v0 + params.kappa * params.theta * time)
        / params.sigma_v;
    settings.u_max = settings.u_max.max(HESTON_TAIL_LOG_TARGET / c_inf);
    settings.panels = settings.u_max.ceil() as usize;
    settings
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
    heston_call_prices_fourier(spot, &[strike], time, params, settings)?
        .into_iter()
        .next()
        .ok_or_else(|| Error::Validation("Heston scalar pricing returned no result".to_string()))
}

/// Validate the common market and model boundary before any limiting branch.
pub(super) fn validate_heston_inputs(
    spot: f64,
    time: f64,
    params: &HestonPricingParams,
) -> Result<()> {
    for (name, value) in [
        ("spot", spot),
        ("v0", params.v0),
        ("kappa", params.kappa),
        ("theta", params.theta),
        ("sigma_v", params.sigma_v),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(Error::Validation(format!(
                "Heston {name} must be finite and positive"
            )));
        }
    }
    for (name, value) in [
        ("time", time),
        ("r", params.r),
        ("q", params.q),
        ("rho", params.rho),
    ] {
        if !value.is_finite() {
            return Err(Error::Validation(format!("Heston {name} must be finite")));
        }
    }
    if params.rho <= -1.0 || params.rho >= 1.0 {
        return Err(Error::Validation(
            "Heston rho must be in (-1, 1)".to_string(),
        ));
    }
    if time > 0.0
        && [(-params.r * time).exp(), spot * (-params.q * time).exp()]
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(Error::Validation(
            "Heston discounting must be finite and positive".to_string(),
        ));
    }
    Ok(())
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
    validate_heston_inputs(spot, time, params)?;
    for strike in strikes {
        if !strike.is_finite() || *strike <= 0.0 {
            return Err(Error::Validation(
                "Heston strikes must be finite and positive".to_string(),
            ));
        }
    }
    if let Some(settings) = settings {
        settings.validate()?;
    }
    if time <= 0.0 {
        return Ok(strikes
            .iter()
            .map(|&strike| (spot - strike).max(0.0))
            .collect());
    }
    if strikes.is_empty() {
        return Ok(Vec::new());
    }

    let mut grid = resolve_heston_settings(time, params, settings);
    grid.validate()?;
    let scale = spot * (-params.q * time).exp();
    let mut previous: Option<Vec<f64>> = None;
    // Require two resolved grids to agree. Widening the interval and halving
    // panel width checks both truncation and quadrature resolution.
    for _ in 0..6 {
        if let Some(pricer) = HestonStripPricer::new(spot, time, params, &grid) {
            if let Ok(prices) = pricer.price_calls(strikes) {
                if let Some(previous) = &previous {
                    if prices
                        .iter()
                        .zip(previous)
                        .all(|(price, old)| (price - old).abs() <= 1e-9 * scale)
                    {
                        return Ok(prices);
                    }
                }
                previous = Some(prices);
            } else {
                previous = None;
            }
        }
        grid.u_max *= 2.0;
        let Some(panels) = grid.panels.checked_mul(4) else {
            break;
        };
        grid.panels = panels;
        if grid.validate().is_err() {
            break;
        }
    }
    Err(Error::Calibration {
        category: "heston_fourier".to_string(),
        message: format!("Heston Fourier integration did not converge within the quadrature work budget for spot={spot}, time={time}"),
    })
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
    let call_prices = heston_call_prices_fourier(spot, strikes, time, params, settings)?;
    Ok(call_prices
        .into_iter()
        .zip(strikes.iter())
        .map(|(call_price, strike)| {
            let forward = spot * (-params.q * time.max(0.0)).exp();
            let discount_k = *strike * (-params.r * time.max(0.0)).exp();
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
    let call_price = heston_call_price_fourier(spot, strike, time, params, settings)?;
    let forward = spot * (-params.q * time.max(0.0)).exp();
    let discount_k = strike * (-params.r * time.max(0.0)).exp();
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

/// Black-Scholes reference for deterministic-variance regression tests.
#[cfg(test)]
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
