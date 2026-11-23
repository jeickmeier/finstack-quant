//! Cheyette rough-volatility process parameters and [`StochasticProcess`] implementation.

use super::base_volatility::BaseVolatility;
use crate::monte_carlo::paths::ProcessParams;
use crate::monte_carlo::process::credit_line::interpolate_rate;
use crate::monte_carlo::traits::{PathState, ProcessMetadata, StateKey, StochasticProcess};
use finstack_quant_core::math::fractional::HurstExponent;

/// Cheyette + rough stochastic volatility model parameters.
///
/// Encapsulates all inputs required to define the Cheyette dynamics with a
/// rough vol driver. Serialization preserves both the base-volatility curve
/// and initial-forward curve; deserialization validates the complete model
/// through the same constructor used for market inputs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawCheyetteRoughVolParams")]
#[cfg_attr(feature = "json-schema", schemars(!try_from))]
pub struct CheyetteRoughVolParams {
    /// Mean reversion of the short rate (κ > 0).
    pub kappa: f64,
    /// Base volatility term structure σ₀(t) for the rate process.
    pub sigma_base: BaseVolatility,
    /// Hurst exponent H ∈ (0, 0.5) for the rough vol driver.
    pub hurst: HurstExponent,
    /// Vol-of-vol scaling (η > 0).
    pub eta: f64,
    /// Correlation between rate and vol innovations ρ ∈ [-1, 1].
    pub rho: f64,
    /// Time knots for the initial forward rate curve φ(t) = f(0, t).
    phi_times: Vec<f64>,
    /// Value knots for the initial forward rate curve φ(t) = f(0, t).
    phi_values: Vec<f64>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCheyetteRoughVolParams {
    kappa: f64,
    sigma_base: BaseVolatility,
    hurst: HurstExponent,
    eta: f64,
    rho: f64,
    phi_times: Vec<f64>,
    phi_values: Vec<f64>,
}

impl TryFrom<RawCheyetteRoughVolParams> for CheyetteRoughVolParams {
    type Error = finstack_quant_core::Error;

    fn try_from(raw: RawCheyetteRoughVolParams) -> finstack_quant_core::Result<Self> {
        if raw.phi_times.len() != raw.phi_values.len() {
            return Err(finstack_quant_core::Error::Validation(
                "CheyetteRoughVol phi times and values must have equal length".to_string(),
            ));
        }
        let phi_points: Vec<_> = raw.phi_times.into_iter().zip(raw.phi_values).collect();
        Self::new(
            raw.kappa,
            raw.sigma_base,
            raw.hurst,
            raw.eta,
            raw.rho,
            &phi_points,
        )
    }
}

impl CheyetteRoughVolParams {
    /// Create new Cheyette + rough vol parameters with full validation.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Mean reversion speed (must be positive and finite)
    /// * `sigma_base` - Deterministic rate volatility σ₀(t), in decimal rate units per square-root model year.
    /// * `hurst` - Hurst exponent; warns if not rough (H ≥ 0.5)
    /// * `eta` - Vol-of-vol (must be positive and finite)
    /// * `rho` - Rate-vol correlation (must be in \[-1, 1\] and finite)
    /// * `phi_points` - Initial forward rate curve as (time, rate) pairs;
    ///   must have at least one point, times non-negative and increasing
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when any parameter is out
    /// of range.
    pub fn new(
        kappa: f64,
        sigma_base: BaseVolatility,
        hurst: HurstExponent,
        eta: f64,
        rho: f64,
        phi_points: &[(f64, f64)],
    ) -> finstack_quant_core::Result<Self> {
        let (phi_times, phi_values): (Vec<f64>, Vec<f64>) = phi_points.iter().copied().unzip();
        let params = Self {
            kappa,
            sigma_base,
            hurst,
            eta,
            rho,
            phi_times,
            phi_values,
        };
        params.validate()?;
        Ok(params)
    }

    /// Check the parameter domains that [`Self::new`] enforces, for a value
    /// whose public fields may have been edited after construction; logs the
    /// non-rough Hurst warning as [`Self::new`] does.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when any parameter is
    /// out of range.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        let Self {
            kappa,
            hurst,
            eta,
            rho,
            ..
        } = *self;
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CheyetteRoughVol parameter kappa must be positive and finite, got {kappa}"
            )));
        }
        if eta <= 0.0 || !eta.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CheyetteRoughVol parameter eta must be positive and finite, got {eta}"
            )));
        }
        if !(-1.0..=1.0).contains(&rho) || !rho.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CheyetteRoughVol parameter rho must be in [-1, 1], got {rho}"
            )));
        }
        super::super::validate_piecewise_knots(
            &self.phi_times,
            &self.phi_values,
            true,
            "CheyetteRoughVol phi_points",
        )?;

        if !hurst.is_rough() {
            tracing::warn!(
                h = hurst.get_value(),
                "CheyetteRoughVol Hurst exponent H = {:.4} is not rough (H >= 0.5). \
                 The model is designed for rough volatility; results may not \
                 match empirical behavior.",
                hurst.get_value()
            );
        }

        Ok(())
    }

    /// Interpolate the initial forward rate φ(t) = f(0, t) at time `t`.
    ///
    /// Uses linear interpolation between knots and flat extrapolation at the
    /// boundaries.
    pub fn phi(&self, t: f64) -> f64 {
        debug_assert!(!self.phi_times.is_empty());
        interpolate_rate(t, &self.phi_times, &self.phi_values)
    }
}

/// Cheyette + rough stochastic volatility process.
///
/// State: \[x, y\] where x is the rate deviation from the initial forward
/// curve and y is the accumulated variance state.
///
/// Factors: 3 — one independent standard normal for the uncorrelated rate
/// component, one Riemann-Liouville Volterra increment `ΔỸ`, and one
/// unit-variance driving Brownian normal `Z̃` (the rate–vol correlation
/// target), both supplied by the engine's fractional noise integration.
///
/// The short rate is reconstructed as r(t) = x(t) + φ(t) and written to
/// [`crate::monte_carlo::traits::state_keys::SHORT_RATE`] in [`populate_path_state`](StochasticProcess::populate_path_state).
#[derive(Debug, Clone)]
pub struct CheyetteRoughVolProcess {
    /// Model parameters.
    params: CheyetteRoughVolParams,
}

impl CheyetteRoughVolProcess {
    /// Create a new Cheyette + rough vol process from validated parameters.
    pub fn new(params: CheyetteRoughVolParams) -> Self {
        Self { params }
    }

    /// Get a reference to the process parameters.
    pub fn params(&self) -> &CheyetteRoughVolParams {
        &self.params
    }
}

impl StochasticProcess for CheyetteRoughVolProcess {
    fn dim(&self) -> usize {
        2 // x (rate state) and y (variance state)
    }

    fn num_factors(&self) -> usize {
        // z[0] = independent normal (uncorrelated rate),
        // z[1] = RL Volterra increment ΔỸ,
        // z[2] = unit-variance driving Brownian normal Z̃
        3
    }

    fn requires_injected_noise(&self) -> bool {
        // z[1]/z[2] must be Volterra increments and their driving normals,
        // generated jointly on the full grid; i.i.d. engine noise is invalid.
        true
    }

    fn dedicated_scheme(&self) -> Option<&'static str> {
        Some("cheyette_rough_euler")
    }

    /// Formal no-op: actual drift is applied by the Cheyette rough vol discretization.
    fn drift(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out[0] = 0.0;
        out[1] = 0.0;
    }

    /// Formal no-op: actual diffusion is applied by the Cheyette rough vol discretization.
    fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) -> finstack_quant_core::Result<()> {
        out[0] = 0.0;
        out[1] = 0.0;
        Ok(())
    }

    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        let t = state.time;
        let phi_t = self.params.phi(t);
        state.set_key(StateKey::ShortRate, x[0] + phi_t);
    }
}

impl ProcessMetadata for CheyetteRoughVolProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("CheyetteRoughVol");
        params.add_param("kappa", self.params.kappa);
        params.add_param("H", self.params.hurst.get_value());
        params.add_param("eta", self.params.eta);
        params.add_param("rho", self.params.rho);

        params.with_factors(vec!["rate".to_string(), "variance".to_string()])
    }
}
