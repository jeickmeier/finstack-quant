use super::params::HESTON_TAIL_LOG_TARGET;
use super::{HestonFourierSettings, HestonStripPricer};
#[cfg(test)]
use crate::closed_form::bs_price_unchecked;
use crate::monte_carlo::process::heston::HestonProcessParams;
use crate::types::{OptionContractParams, OptionType};
use crate::volatility::heston::HestonParams;
use finstack_quant_core::{Error, Result};

fn resolve_heston_settings(
    time: f64,
    params: &HestonProcessParams,
    settings: Option<&HestonFourierSettings>,
) -> HestonFourierSettings {
    if let Some(settings) = settings {
        return *settings;
    }
    let mut settings =
        HestonFourierSettings::for_maturity(time, params.model.v0.min(params.model.theta));
    // Include the large-frequency Heston exponential tail alongside the
    // Gaussian short-time bound. Neither bound alone covers both regimes.
    let c_inf = (1.0 - params.model.rho * params.model.rho).sqrt()
        * (params.model.v0 + params.model.kappa * params.model.theta * time)
        / params.model.vol_of_vol;
    settings.u_max = settings.u_max.max(HESTON_TAIL_LOG_TARGET / c_inf);
    settings.panels = settings.u_max.ceil() as usize;
    settings
}

pub(super) fn call_strike(
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonProcessParams,
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
    params: &HestonProcessParams,
) -> Result<()> {
    for (name, value) in [
        ("spot", spot),
        ("v0", params.model.v0),
        ("kappa", params.model.kappa),
        ("theta", params.model.theta),
        ("vol_of_vol", params.model.vol_of_vol),
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
        ("rho", params.model.rho),
    ] {
        if !value.is_finite() {
            return Err(Error::Validation(format!("Heston {name} must be finite")));
        }
    }
    if params.model.rho <= -1.0 || params.model.rho >= 1.0 {
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
///   [`HestonFourierSettings::for_maturity`].
pub(crate) fn heston_call_prices_fourier(
    spot: f64,
    strikes: &[f64],
    time: f64,
    params: &HestonProcessParams,
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
                if let Some(previous) = &previous
                    && prices
                        .iter()
                        .zip(previous)
                        .all(|(price, old)| (price - old).abs() <= 1e-9 * scale)
                {
                    return Ok(prices);
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
        message: format!(
            "Heston Fourier integration did not converge within the quadrature work budget for spot={spot}, time={time}"
        ),
    })
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
///   [`HestonFourierSettings::for_maturity`].
///
/// # Returns
///
/// Put option price
///
/// # Formula
///
/// Uses put-call parity: P = C - S*exp(-qT) + K*exp(-rT)
pub(crate) fn heston_put_price_fourier(
    spot: f64,
    strike: f64,
    time: f64,
    params: &HestonProcessParams,
    settings: Option<&HestonFourierSettings>,
) -> Result<f64> {
    let call_price = call_strike(spot, strike, time, params, settings)?;
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

/// Price a European option under the Heston model using Fourier inversion.
/// # Arguments
///
/// * `contract` - Spot and strike in quote currency, annual continuous
///   rate and carry, nonnegative expiry in years, and the payoff direction.
/// * `heston` - Instantaneous opening variance and canonical annual variance
///   dynamics, including the spot/variance correlation in `(-1, 1)`.
/// * `settings` - Optional Fourier integration limits and quadrature
///   settings. Omitted settings are chosen from maturity and variance.
///
/// # Errors
///
/// Returns a validation error for invalid inputs or unrepresentable
/// discounting. Returns a calibration error when integration or put-call
/// parity cannot produce a finite resolved price.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_models::closed_form::heston_price;
/// use finstack_quant_models::types::{OptionContractParams, OptionType};
/// use finstack_quant_models::volatility::heston::HestonParams;
///
/// let contract = OptionContractParams {
///     spot: 100.0, strike: 100.0, rate: 0.05, dividend_yield: 0.02,
///     time_to_expiry: 1.0, option_type: OptionType::Call,
/// };
/// let model = HestonParams::new(0.04, 2.0, 0.04, 0.3, -0.7)?;
/// let price = heston_price(&contract, &model, None)?;
/// assert!(price > 5.0 && price < 15.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn heston_price(
    contract: &OptionContractParams,
    heston: &HestonParams,
    settings: Option<&HestonFourierSettings>,
) -> Result<f64> {
    contract.validate()?;
    let OptionContractParams {
        spot,
        strike,
        rate,
        dividend_yield,
        time_to_expiry,
        option_type,
    } = *contract;
    heston.validate()?;
    let dynamics = HestonProcessParams {
        r: rate,
        q: dividend_yield,
        model: *heston,
    };
    match option_type {
        OptionType::Call => call_strike(spot, strike, time_to_expiry, &dynamics, settings),
        OptionType::Put => {
            heston_put_price_fourier(spot, strike, time_to_expiry, &dynamics, settings)
        }
    }
}

/// Price one European call under the Heston model using Fourier inversion.
///
/// # Arguments
///
/// * `contract` - Call payoff, positive spot and strike in quote units, annual
///   continuous carry rates and nonnegative time to expiry in years.
/// * `heston` - Validated annual Heston variance dynamics and spot/variance correlation.
/// * `settings` - Optional integration settings. Omit to use the maturity-based defaults.
///
/// # Errors
/// Rejects non-call contracts, invalid input domains, or unresolved integration.
pub fn heston_call_price_fourier(
    contract: &OptionContractParams,
    heston: &HestonParams,
    settings: Option<&HestonFourierSettings>,
) -> Result<f64> {
    if contract.option_type != OptionType::Call {
        return Err(Error::Validation(
            "Heston call pricing requires a call payoff".into(),
        ));
    }
    heston_price(contract, heston, settings)
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
