//! CIR (Cox-Ingersoll-Ross) and CIR++ processes.
//!
//! Implements the square-root diffusion process used for modeling:
//! - Short rates (Vasicek extension with mean-reverting variance)
//! - Stochastic volatility (variance component in Heston)
//! - Credit intensities (default hazard rates)
//!
//! # CIR SDE
//!
//! ```text
//! dv_t = κ(θ - v_t)dt + σ√v_t dW_t
//! ```
//!
//! where:
//! - κ = mean reversion speed
//! - θ = long-term mean
//! - σ = volatility of volatility
//! - v_t ≥ 0 (ensured by QE discretization)
//!
//! # Feller Condition
//!
//! For positivity: 2κθ ≥ σ²
//! If violated, zero is attainable (but QE handles gracefully).

use super::super::paths::ProcessParams;
#[cfg(test)]
use super::super::traits::state_keys;
use super::super::traits::{PathState, StateKey, StochasticProcess};
use super::metadata::ProcessMetadata;

/// CIR process parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CirParams {
    /// Mean reversion speed (κ)
    pub kappa: f64,
    /// Long-term mean (θ)
    pub theta: f64,
    /// Volatility of volatility (σ)
    pub sigma: f64,
}

impl CirParams {
    /// Create new CIR parameters with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `kappa <= 0` or non-finite
    /// - `theta < 0` or non-finite
    /// - `sigma <= 0` or non-finite
    ///
    /// # Arguments
    ///
    /// * `kappa` - Mean-reversion speed of the stochastic volatility or short-rate factor
    /// * `theta` - Long-run mean level of the mean-reverting stochastic factor
    /// * `sigma` - Diffusion volatility of the process in decimal annual units
    pub fn new(kappa: f64, theta: f64, sigma: f64) -> finstack_quant_core::Result<Self> {
        if !kappa.is_finite() || kappa <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CIR kappa (mean reversion speed) must be positive, got {kappa}"
            )));
        }
        if !theta.is_finite() || theta < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CIR theta (long-term mean) must be non-negative, got {theta}"
            )));
        }
        if !sigma.is_finite() || sigma <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CIR sigma (volatility) must be positive, got {sigma}"
            )));
        }

        Ok(Self {
            kappa,
            theta,
            sigma,
        })
    }

    /// Check if Feller condition is satisfied.
    ///
    /// Feller condition: 2κθ ≥ σ²
    /// If satisfied, the process stays strictly positive.
    pub fn satisfies_feller(&self) -> bool {
        2.0 * self.kappa * self.theta >= self.sigma * self.sigma
    }
}

/// CIR process for modeling short rates or intensities.
///
/// State dimension: 1 (variance/rate v)
/// Factor dimension: 1 (Brownian motion)
///
/// # SDE
///
/// ```text
/// dv_t = κ(θ - v_t)dt + σ√v_t dW_t
/// ```
///
/// # Discretization
///
/// Use `QeCir` discretization (extracted from Heston QE) for best accuracy
/// and guaranteed positivity.
#[derive(Debug, Clone)]
pub struct CirProcess {
    params: CirParams,
}

impl CirProcess {
    /// Create a new CIR process.
    ///
    /// # Feller Condition Warning
    ///
    /// If the Feller condition (2κθ ≥ σ²) is violated, a warning is logged.
    /// When violated, the process can reach zero with positive probability,
    /// though the QE scheme handles this gracefully via truncation.
    pub fn new(params: CirParams) -> Self {
        if !params.satisfies_feller() {
            let feller_ratio = 2.0 * params.kappa * params.theta / (params.sigma * params.sigma);
            tracing::warn!(
                kappa = params.kappa,
                theta = params.theta,
                sigma = params.sigma,
                feller_ratio = feller_ratio,
                "CIR Feller condition violated (2κθ < σ²): process may reach zero. \
                 Feller ratio = {:.4} (should be ≥ 1.0). QE scheme will truncate at zero.",
                feller_ratio
            );
        }
        Self { params }
    }

    /// Create with explicit parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if any parameter is invalid (see [`CirParams::new`]).
    pub fn with_params(kappa: f64, theta: f64, sigma: f64) -> finstack_quant_core::Result<Self> {
        Ok(Self::new(CirParams::new(kappa, theta, sigma)?))
    }

    /// Get parameters.
    pub fn params(&self) -> &CirParams {
        &self.params
    }
}

impl StochasticProcess for CirProcess {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        1
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // μ(v) = κ(θ − v⁺): full-truncation Euler (Lord, Koekkoek & van Dijk
        // 2010) — both drift and diffusion read the truncated state v⁺ while
        // the state itself may go negative. This matches the Heston variance
        // leg; mixing truncated diffusion with raw-state drift ("partial
        // truncation") is a different scheme with a different bias.
        let v = x[0].max(0.0);
        out[0] = self.params.kappa * (self.params.theta - v);
    }

    fn diffusion(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // σ(v) = σ√(v⁺) — full truncation, see `drift`.
        let v = x[0].max(0.0);
        out[0] = self.params.sigma * v.sqrt();
    }

    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        if !x.is_empty() {
            state.set_key(StateKey::ShortRate, x[0]);
            state.set_key(StateKey::Spot, x[0]);
        }
    }
}

impl ProcessMetadata for CirProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("CIR");
        params.add_param("kappa", self.params.kappa);
        params.add_param("theta", self.params.theta);
        params.add_param("sigma", self.params.sigma);
        params.with_factors(vec!["short_rate".to_string()])
    }
}

/// CIR++ process (shifted CIR for yield curve fitting).
///
/// The CIR++ model adds a deterministic shift φ(t) to CIR:
///
/// ```text
/// r_t = x_t + φ(t)
/// dx_t = κ(θ - x_t)dt + σ√x_t dW_t
/// ```
///
/// where φ(t) is chosen to fit the initial yield curve.
///
/// State dimension: 1 (shifted rate x, actual rate is x + φ(t))
#[derive(Debug, Clone)]
pub struct CirPlusPlusProcess {
    /// Base CIR process
    cir: CirProcess,
    /// Deterministic shift curve φ(t) (piecewise constant)
    shift_curve: Vec<f64>,
    /// Time breakpoints for φ(t)
    shift_times: Vec<f64>,
}

impl CirPlusPlusProcess {
    /// Create a new CIR++ process.
    ///
    /// # Arguments
    ///
    /// * `cir` - Base CIR process
    /// * `shift_curve` - Deterministic shift values φ as decimal rates, one
    ///   per breakpoint; each applies from its breakpoint until the next
    /// * `shift_times` - Strictly increasing finite time breakpoints in years
    ///
    /// # Errors
    ///
    /// Returns a validation error when the two vectors are empty or differ in
    /// length, any value is non-finite, or the times are not strictly
    /// increasing.
    pub fn new(
        cir: CirProcess,
        shift_curve: Vec<f64>,
        shift_times: Vec<f64>,
    ) -> finstack_quant_core::Result<Self> {
        if shift_times.is_empty() || shift_curve.len() != shift_times.len() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CIR++ shift curve and times must be non-empty and equally sized, got {} and {}",
                shift_curve.len(),
                shift_times.len()
            )));
        }
        if shift_curve
            .iter()
            .chain(&shift_times)
            .any(|value| !value.is_finite())
        {
            return Err(finstack_quant_core::Error::Validation(
                "CIR++ shift values and times must be finite".to_string(),
            ));
        }
        if shift_times.windows(2).any(|pair| pair[1] <= pair[0]) {
            return Err(finstack_quant_core::Error::Validation(
                "CIR++ shift times must increase strictly".to_string(),
            ));
        }

        Ok(Self {
            cir,
            shift_curve,
            shift_times,
        })
    }

    /// Create with constant shift.
    ///
    /// # Arguments
    ///
    /// * `cir` - Base CIR process
    /// * `shift` - Finite constant shift φ as a decimal rate
    ///
    /// # Errors
    ///
    /// Returns a validation error when `shift` is non-finite.
    pub fn with_constant_shift(cir: CirProcess, shift: f64) -> finstack_quant_core::Result<Self> {
        Self::new(cir, vec![shift], vec![0.0])
    }

    /// Get φ(t) at a given time.
    pub fn shift_at_time(&self, t: f64) -> f64 {
        // Piecewise-constant interpolation
        for i in (0..self.shift_times.len()).rev() {
            if t >= self.shift_times[i] {
                return self.shift_curve[i];
            }
        }

        self.shift_curve[0]
    }

    /// Get actual short rate r_t = x_t + φ(t).
    pub fn actual_rate(&self, x: f64, t: f64) -> f64 {
        x + self.shift_at_time(t)
    }

    /// Get base CIR process.
    pub fn cir(&self) -> &CirProcess {
        &self.cir
    }
}

impl StochasticProcess for CirPlusPlusProcess {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        1
    }

    fn drift(&self, t: f64, x: &[f64], out: &mut [f64]) {
        // The state x follows base CIR dynamics (shift doesn't affect SDE)
        self.cir.drift(t, x, out);
    }

    fn diffusion(&self, t: f64, x: &[f64], out: &mut [f64]) {
        // Diffusion is same as base CIR
        self.cir.diffusion(t, x, out);
    }

    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        if !x.is_empty() {
            let rate = self.actual_rate(x[0], state.time);
            state.set_key(StateKey::ShortRate, rate);
            state.set_key(StateKey::Spot, rate);
        }
    }
}

// The state vector holds the unshifted CIR factor `x_t`; the short rate is
// `x_t + φ(t)` (see `actual_rate`).
impl ProcessMetadata for CirPlusPlusProcess {
    fn metadata(&self) -> ProcessParams {
        let cir = self.cir.params();
        let mut params = ProcessParams::new("CIR++");
        params.add_param("kappa", cir.kappa);
        params.add_param("theta", cir.theta);
        params.add_param("sigma", cir.sigma);
        params.with_factors(vec!["x".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cir_params_feller_condition() {
        // Satisfies Feller: 2κθ ≥ σ²
        let params1 = CirParams::new(0.5, 0.04, 0.1).unwrap();
        assert!(params1.satisfies_feller());
        // 2 * 0.5 * 0.04 = 0.04 >= 0.01 ✓

        // Violates Feller
        let params2 = CirParams::new(0.1, 0.01, 0.2).unwrap();
        assert!(!params2.satisfies_feller());
        // 2 * 0.1 * 0.01 = 0.002 < 0.04 ✗
    }

    #[test]
    fn test_cir_drift() {
        let params = CirParams::new(0.3, 0.04, 0.1).unwrap();
        let process = CirProcess::new(params);

        // Above mean
        let x = vec![0.05];
        let mut drift = vec![0.0];
        process.drift(0.0, &x, &mut drift);
        assert!(drift[0] < 0.0); // Pull down
        assert_eq!(drift[0], 0.3 * (0.04 - 0.05));

        // Below mean
        let x2 = vec![0.03];
        process.drift(0.0, &x2, &mut drift);
        assert!(drift[0] > 0.0); // Pull up
        assert_eq!(drift[0], 0.3 * (0.04 - 0.03));
    }

    #[test]
    fn test_cir_diffusion() {
        let params = CirParams::new(0.3, 0.04, 0.1).unwrap();
        let process = CirProcess::new(params);

        let x = vec![0.04];
        let mut diffusion = vec![0.0];
        process.diffusion(0.0, &x, &mut diffusion);

        // σ√v = 0.1 * √0.04 = 0.1 * 0.2 = 0.02
        assert_eq!(diffusion[0], 0.1 * 0.04_f64.sqrt());
    }

    #[test]
    fn test_cir_plus_plus_shift() {
        let cir = CirProcess::with_params(0.1, 0.03, 0.05).unwrap();
        let shift_curve = vec![0.01, 0.02];
        let shift_times = vec![0.0, 1.0];

        let cir_pp = CirPlusPlusProcess::new(cir, shift_curve, shift_times).unwrap();

        assert_eq!(cir_pp.shift_at_time(0.0), 0.01);
        assert_eq!(cir_pp.shift_at_time(0.5), 0.01);
        assert_eq!(cir_pp.shift_at_time(1.0), 0.02);
        assert_eq!(cir_pp.shift_at_time(2.0), 0.02);

        // Actual rate
        assert_eq!(cir_pp.actual_rate(0.03, 0.0), 0.04); // x + φ(0)
        assert_eq!(cir_pp.actual_rate(0.03, 1.5), 0.05); // x + φ(1.5)
    }

    #[test]
    fn test_cir_plus_plus_populates_shifted_short_rate() {
        let cir = CirProcess::with_params(0.1, 0.03, 0.05).unwrap();
        let cir_pp = CirPlusPlusProcess::new(cir, vec![0.01, 0.02], vec![0.0, 1.0]).unwrap();
        let mut state = PathState::new(1, 1.5);

        cir_pp.populate_path_state(&[0.03], &mut state);

        assert_eq!(state.get(state_keys::SHORT_RATE), Some(0.05));
        assert_eq!(state.spot(), Some(0.05));
    }

    #[test]
    fn test_cir_plus_plus_dynamics() {
        let cir = CirProcess::with_params(0.1, 0.03, 0.05).unwrap();
        let cir_pp = CirPlusPlusProcess::with_constant_shift(cir, 0.02).unwrap();

        // The state x follows base CIR dynamics
        let x = vec![0.04];
        let mut drift = vec![0.0];
        cir_pp.drift(0.0, &x, &mut drift);

        // Drift should be same as base CIR
        assert_eq!(drift[0], 0.1 * (0.03 - 0.04));
    }

    #[test]
    fn invalid_cir_plus_plus_shift_schedules_are_errors() {
        let cir = || CirProcess::with_params(0.1, 0.03, 0.05).unwrap();
        assert!(CirPlusPlusProcess::new(cir(), vec![], vec![]).is_err());
        assert!(CirPlusPlusProcess::new(cir(), vec![0.01], vec![0.0, 1.0]).is_err());
        assert!(CirPlusPlusProcess::new(cir(), vec![0.01, 0.02], vec![1.0, 1.0]).is_err());
        assert!(CirPlusPlusProcess::with_constant_shift(cir(), f64::NAN).is_err());
    }
}
