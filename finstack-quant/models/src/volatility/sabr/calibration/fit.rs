//! Vega-weighted residual preparation and quote-fit diagnostics for SABR calibration.

use super::super::parameters::SabrParameters;
use super::calibrator::SabrCalibrationOutcome;
use crate::closed_form::{bachelier_vega, black_vega};
use finstack_quant_core::math::solver_multi::LevenbergMarquardtSolver;
use finstack_quant_core::{Error, Result, market_data::surfaces::VolQuoteType};

/// Vega weight used by the SABR calibration objectives.
///
/// Standard practitioner choice (Hagan 2002, Bloomberg VCUB): weight each
/// (strike, market_vol) residual by vega. Vega concentrates near ATM
/// and decays into the wings, so an unweighted `Σ(σ_m − σ_*)²` would
/// over-fit the wings (large numbers of low-information quotes) at the
/// expense of the ATM. Vega weighting gives every dollar of premium roughly
/// equal weight, which is what the market actually quotes against.
///
/// The vega convention must match the explicitly selected volatility quotes.
///
/// - Normal quotes use
///   (Bachelier) vols, so weight with Bachelier vega `√T·φ((F−K)/(σ_N√T))`.
///   Feeding a ~1% normal vol to Black vega would collapse all wing weights
///   to the floor and leave the smile uncalibrated.
/// - otherwise: lognormal (Black) quotes, weight with Black-76 vega. Shifted
///   calibrations pass already-shifted forward/strikes, making this the
///   shifted-Black vega.
///
/// Floor at a tiny positive number keeps deep-OTM strikes from getting a
/// strictly-zero weight (which would let the optimizer drift on the wings).
///
/// # Arguments
///
/// * `forward` - Forward price or rate in the same units as `strike`.
/// * `strike` - Option strike in price units or decimal-rate units.
/// * `market_vol` - Market volatility in the selected quote units.
/// * `time_to_expiry` - Time to expiry in years.
/// * `quote_type` - Normal or Black vega basis matching the input volatilities.
#[cfg(test)]
#[inline]
pub(super) fn vega_weight(
    forward: f64,
    strike: f64,
    market_vol: f64,
    time_to_expiry: f64,
    quote_type: VolQuoteType,
) -> f64 {
    const MIN_VEGA: f64 = 1e-10;
    let vega = if quote_type == VolQuoteType::Normal {
        bachelier_vega(forward, strike, market_vol, time_to_expiry)
    } else {
        black_vega(forward, strike, market_vol, time_to_expiry)
    };
    vega.max(MIN_VEGA)
}

pub(super) fn normalized_quote_weights(
    forward: f64,
    strikes: &[f64],
    market_vols: &[f64],
    time_to_expiry: f64,
    beta: f64,
    quote_type: VolQuoteType,
) -> Result<Vec<f64>> {
    if strikes.len() != market_vols.len()
        || strikes.is_empty()
        || market_vols.iter().any(|v| !v.is_finite() || *v <= 0.0)
    {
        return Err(Error::Validation(
            "SABR requires aligned, finite positive volatility quotes".to_owned(),
        ));
    }
    let params = SabrParameters::new(1.0, beta, 0.3, 0.0)?;
    let mut weights = Vec::with_capacity(strikes.len());
    for (&strike, &vol) in strikes.iter().zip(market_vols) {
        params.validate_inputs(forward, strike, time_to_expiry)?;
        if quote_type == VolQuoteType::BlackLognormal && (forward <= 0.0 || strike <= 0.0) {
            return Err(Error::Validation(
                "Black SABR calibration requires positive effective rates".into(),
            ));
        }
        let weight = if quote_type == VolQuoteType::Normal {
            bachelier_vega(forward, strike, vol, time_to_expiry)
        } else {
            black_vega(forward, strike, vol, time_to_expiry)
        };
        if !weight.is_finite() || weight < 0.0 {
            return Err(Error::Validation(
                "SABR quote produced invalid vega weight".to_owned(),
            ));
        }
        weights.push(weight);
    }
    let max_weight = weights.iter().copied().fold(0.0_f64, f64::max);
    if max_weight <= 0.0 {
        return Err(Error::Validation(
            "SABR quotes have no measurable vega".to_owned(),
        ));
    }
    // A relative floor gives every observed quote a nonzero objective weight
    // without introducing a dependence on the forward/volatility unit scale.
    for weight in &mut weights {
        *weight = (*weight / max_weight).max(1e-8);
    }
    let total: f64 = weights.iter().sum();
    let factors = weights
        .iter()
        .zip(market_vols)
        .map(|(&weight, &vol)| (weight / total).sqrt() / vol)
        .collect();
    Ok(factors)
}

// Check each candidate before selecting the best weighted fit. A small weighted
pub(super) struct PreparedSabrFit<'a> {
    pub(super) forward: f64,
    pub(super) strikes: &'a [f64],
    pub(super) market_vols: &'a [f64],
    pub(super) time_to_expiry: f64,
    pub(super) quote_type: VolQuoteType,
    pub(super) tolerance: f64,
    pub(super) weights: Vec<f64>,
    pub(super) atm_vol: f64,
    pub(super) starts: Vec<[f64; 3]>,
    pub(super) prepared: Vec<super::super::expansion::PreparedSabrStrike>,
    pub(super) solver: LevenbergMarquardtSolver,
}

impl PreparedSabrFit<'_> {
    pub(super) fn evaluate_residuals(
        &self,
        parameters: &SabrParameters,
        output: &mut [f64],
    ) -> finstack_quant_core::Result<()> {
        for (index, (strike, &market_vol)) in self.prepared.iter().zip(self.market_vols).enumerate()
        {
            let model_vol = strike.evaluate(parameters)?;
            output[index] = self.weights[index] * (model_vol - market_vol);
        }
        Ok(())
    }

    // Check every individual quote error against the budget since weighted L2
    // norm alone can conceal a bad quote at a low-vega strike.
    pub(super) fn candidate_fits(&self, parameters: &SabrParameters) -> bool {
        self.strikes
            .iter()
            .zip(self.market_vols)
            .all(|(&strike, &quote)| {
                parameters
                    .implied_vol(self.forward, strike, self.time_to_expiry, self.quote_type)
                    .is_ok_and(|vol| {
                        vol.is_finite() && (vol - quote).abs() <= self.tolerance * quote
                    })
            })
    }

    pub(super) fn validate_quote_fit(
        &self,
        mut outcome: SabrCalibrationOutcome,
    ) -> Result<SabrCalibrationOutcome> {
        outcome.quote_errors = self
            .strikes
            .iter()
            .zip(self.market_vols)
            .map(|(&strike, &market_vol)| {
                outcome
                    .parameters
                    .implied_vol(self.forward, strike, self.time_to_expiry, self.quote_type)
                    .map(|v| v - market_vol)
            })
            .collect::<Result<Vec<_>>>()?;
        outcome.max_relative_quote_error = outcome
            .quote_errors
            .iter()
            .zip(self.market_vols)
            .map(|(&error, &quote)| error.abs() / quote)
            .fold(0.0_f64, f64::max);
        if !outcome.max_relative_quote_error.is_finite()
            || outcome.max_relative_quote_error > self.tolerance
        {
            return Err(Error::Validation(format!(
                "SABR final maximum relative quote error {} exceeds tolerance {}; quote errors {:?}",
                outcome.max_relative_quote_error, self.tolerance, outcome.quote_errors
            )));
        }
        Ok(outcome)
    }
}

/// Leading ATM term of the selected SABR expansion.
pub(super) fn initial_alpha_guess(
    atm_vol: f64,
    forward: f64,
    beta: f64,
    quote_type: VolQuoteType,
) -> f64 {
    match quote_type {
        VolQuoteType::Normal if beta == 0.0 => atm_vol,
        VolQuoteType::Normal => atm_vol / forward.powf(beta),
        _ => atm_vol * forward.powf(1.0 - beta),
    }
}
