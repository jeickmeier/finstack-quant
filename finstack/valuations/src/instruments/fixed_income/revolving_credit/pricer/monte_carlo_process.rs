//! Multi-factor stochastic process for revolving credit facility Monte Carlo pricing.
//!
//! Combines three correlated stochastic factors:
//! - Utilization rate (mean-reverting OU process)
//! - Short interest rate (Hull-White 1F for floating rates, or constant for fixed)
//! - Credit spread/hazard rate (CIR process)
//!
//! # State Variables
//!
//! State vector: [utilization, short_rate, credit_spread]
//!
//! # SDEs
//!
//! ```text
//! dU_t = κ_U (θ_U - U_t) dt + σ_U dW_1
//! dr_t = κ_r [θ_r(t) - r_t] dt + σ_r dW_2  (floating) or dr_t = 0 (fixed)
//! dλ_t = κ_λ (θ_λ - λ_t) dt + σ_λ √λ_t dW_3
//! ```
//!
//! Where W_1, W_2, W_3 are correlated Brownian motions.

use finstack_monte_carlo::paths::ProcessParams;
use finstack_monte_carlo::process::cir::CirParams;
use finstack_monte_carlo::process::metadata::ProcessMetadata;
use finstack_monte_carlo::process::ou::HullWhite1FParams;
use finstack_monte_carlo::traits::StochasticProcess;

/// Parameters for utilization process (mean-reverting OU).
#[derive(Debug, Clone)]
pub struct UtilizationParams {
    /// Mean reversion speed (κ_U)
    pub kappa: f64,
    /// Long-term mean/target utilization (θ_U)
    pub theta: f64,
    /// Volatility (σ_U)
    pub sigma: f64,
}

impl UtilizationParams {
    /// Create new utilization parameters.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] when `kappa` or `sigma` is
    /// not strictly positive, or when `theta` is outside `[0, 1]`. (Previously
    /// these were `assert!`s that panicked in library code on invalid MC
    /// parameters.)
    pub fn new(kappa: f64, theta: f64, sigma: f64) -> finstack_core::Result<Self> {
        // Reject non-finite values explicitly (a `<= 0.0` test would let NaN
        // slip through, since all NaN comparisons are false).
        if !kappa.is_finite() || kappa <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "utilization mean-reversion speed (kappa) must be positive, got {kappa}"
            )));
        }
        if !theta.is_finite() || !(0.0..=1.0).contains(&theta) {
            return Err(finstack_core::Error::Validation(format!(
                "utilization target (theta) must be in [0, 1], got {theta}"
            )));
        }
        if !sigma.is_finite() || sigma <= 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "utilization volatility (sigma) must be positive, got {sigma}"
            )));
        }

        Ok(Self {
            kappa,
            theta,
            sigma,
        })
    }
}

/// Parameters for credit spread process (CIR).
#[derive(Debug, Clone)]
pub struct CreditSpreadParams {
    /// CIR parameters
    pub cir: CirParams,
    /// Initial credit spread
    pub initial: f64,
}

impl CreditSpreadParams {
    /// Create new credit spread parameters.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] when `initial` is
    /// negative, or any error from [`CirParams::new`] for invalid CIR
    /// parameters. (The initial-spread check was previously an `assert!` that
    /// panicked in library code.)
    pub fn new(kappa: f64, theta: f64, sigma: f64, initial: f64) -> finstack_core::Result<Self> {
        // Explicit non-finite rejection: a `< 0.0` test would admit NaN.
        if !initial.is_finite() || initial < 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "initial credit spread must be non-negative, got {initial}"
            )));
        }

        Ok(Self {
            cir: CirParams::new(kappa, theta, sigma)?,
            initial,
        })
    }
}

/// Interest rate process specification.
#[derive(Debug, Clone)]
pub enum InterestRateSpec {
    /// Fixed rate (constant, no dynamics)
    Fixed {
        /// Fixed interest rate
        rate: f64,
    },
    /// Floating rate (Hull-White 1F)
    Floating {
        /// Hull-White parameters
        params: HullWhite1FParams,
        /// Initial short rate
        initial: f64,
    },
    /// Deterministic forward curve (no randomness): short rate = fwd(t)
    /// where fwd(t) is obtained via linear interpolation of provided knots.
    DeterministicForward {
        /// Knot times in years (monotone increasing)
        times: Vec<f64>,
        /// Forward rates at each knot
        rates: Vec<f64>,
    },
}

/// Revolving credit multi-factor process parameters.
#[derive(Debug, Clone)]
pub struct RevolvingCreditProcessParams {
    /// Utilization process parameters
    pub utilization: UtilizationParams,
    /// Interest rate specification (fixed or floating)
    pub interest_rate: InterestRateSpec,
    /// Credit spread process parameters
    pub credit_spread: CreditSpreadParams,
    /// Correlation matrix (3x3) between [utilization, rate, credit]
    /// Must be symmetric, positive definite, with ones on diagonal
    pub correlation: Option<[[f64; 3]; 3]>,
    /// Time offset applied to MC time when mapping to market curve time axis
    /// (e.g., base_date→commitment_date), so market-t = offset + path-t
    pub time_offset: f64,
}

impl RevolvingCreditProcessParams {
    /// Create new process parameters.
    pub fn new(
        utilization: UtilizationParams,
        interest_rate: InterestRateSpec,
        credit_spread: CreditSpreadParams,
    ) -> Self {
        Self {
            utilization,
            interest_rate,
            credit_spread,
            correlation: None,
            time_offset: 0.0,
        }
    }

    /// Set correlation matrix.
    pub fn with_correlation(mut self, correlation: [[f64; 3]; 3]) -> Self {
        self.correlation = Some(correlation);
        self
    }

    /// Set time offset for mapping MC time to market time axis.
    pub fn with_time_offset(mut self, time_offset: f64) -> Self {
        self.time_offset = time_offset.max(0.0);
        self
    }

    /// Get initial state vector [utilization, short_rate, credit_spread].
    pub fn initial_state(&self, initial_utilization: f64) -> [f64; 3] {
        let rate = match &self.interest_rate {
            InterestRateSpec::Fixed { rate } => *rate,
            InterestRateSpec::Floating { initial, .. } => *initial,
            InterestRateSpec::DeterministicForward { times, rates } => {
                if times.is_empty() {
                    0.0
                } else {
                    rates[0]
                }
            }
        };

        [
            initial_utilization.clamp(0.0, 1.0),
            rate,
            self.credit_spread.initial.max(0.0),
        ]
    }
}

/// Multi-factor stochastic process for revolving credit facilities.
///
/// State dimension: 3 [utilization, short_rate, credit_spread]
/// Factor dimension: 3 (correlated Brownian motions)
///
/// When using correlation, the Cholesky decomposition should be applied
/// in the discretization scheme.
#[derive(Debug, Clone)]
pub struct RevolvingCreditProcess {
    params: RevolvingCreditProcessParams,
}

impl RevolvingCreditProcess {
    /// Create a new revolving credit process.
    pub fn new(params: RevolvingCreditProcessParams) -> Self {
        Self { params }
    }

    /// Get process parameters.
    pub fn params(&self) -> &RevolvingCreditProcessParams {
        &self.params
    }

    /// Get correlation matrix (if specified).
    pub fn correlation(&self) -> Option<&[[f64; 3]; 3]> {
        self.params.correlation.as_ref()
    }
}

impl StochasticProcess for RevolvingCreditProcess {
    fn dim(&self) -> usize {
        3
    }

    fn num_factors(&self) -> usize {
        3
    }

    fn drift(&self, t: f64, x: &[f64], out: &mut [f64]) {
        // x[0] = utilization, x[1] = short_rate, x[2] = credit_spread

        // Utilization: κ_U (θ_U - U_t)
        out[0] =
            self.params.utilization.kappa * (self.params.utilization.theta - x[0].clamp(0.0, 1.0));

        // Short rate: κ_r [θ_r(t) - r_t] for floating, or 0 for fixed
        out[1] = match &self.params.interest_rate {
            InterestRateSpec::Fixed { .. } => 0.0,
            InterestRateSpec::Floating { params, .. } => {
                params.kappa * (params.theta_at_time(t) - x[1])
            }
            InterestRateSpec::DeterministicForward { .. } => 0.0,
        };

        // Credit spread: κ_λ (θ_λ - λ_t)
        out[2] = self.params.credit_spread.cir.kappa
            * (self.params.credit_spread.cir.theta - x[2].max(0.0));
    }

    fn diffusion(&self, t: f64, x: &[f64], out: &mut [f64]) {
        // x[0] = utilization, x[1] = short_rate, x[2] = credit_spread

        // Utilization: σ_U (constant volatility)
        out[0] = self.params.utilization.sigma;

        // Short rate: σ_r (constant) for floating, or 0 for fixed
        out[1] = match &self.params.interest_rate {
            InterestRateSpec::Fixed { .. } => 0.0,
            InterestRateSpec::Floating { params, .. } => {
                // Avoid unused variable warning
                let _ = t;
                params.sigma
            }
            InterestRateSpec::DeterministicForward { .. } => 0.0,
        };

        // Credit spread: σ_λ √λ_t (square-root diffusion)
        let lambda = x[2].max(0.0);
        out[2] = self.params.credit_spread.cir.sigma * lambda.sqrt();
    }

    fn factor_correlation(&self) -> Option<Vec<f64>> {
        self.params.correlation.as_ref().map(|m| {
            let mut out = Vec::with_capacity(9);
            for row in m.iter() {
                out.extend_from_slice(row);
            }
            out
        })
    }

    fn populate_path_state(&self, x: &[f64], state: &mut finstack_monte_carlo::traits::PathState) {
        use finstack_monte_carlo::traits::state_keys;
        if !x.is_empty() {
            state.set(state_keys::SPOT, x[0]);
        }
        if x.len() >= 2 {
            state.set(state_keys::SHORT_RATE, x[1]);
        }
        if x.len() >= 3 {
            state.set("credit_spread", x[2]);
        }
    }
}

impl ProcessMetadata for RevolvingCreditProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("RevolvingCredit");

        // Utilization parameters
        params.add_param("util_kappa", self.params.utilization.kappa);
        params.add_param("util_theta", self.params.utilization.theta);
        params.add_param("util_sigma", self.params.utilization.sigma);

        // Interest rate parameters
        match &self.params.interest_rate {
            InterestRateSpec::Fixed { rate } => {
                params.add_param("rate_type", 0.0); // 0.0 = fixed
                params.add_param("rate_fixed", *rate);
            }
            InterestRateSpec::Floating {
                params: hw_params,
                initial,
            } => {
                params.add_param("rate_type", 1.0); // 1.0 = floating
                params.add_param("rate_kappa", hw_params.kappa);
                params.add_param("rate_sigma", hw_params.sigma);
                params.add_param("rate_initial", *initial);
            }
            InterestRateSpec::DeterministicForward { times, .. } => {
                params.add_param("rate_type", 2.0); // 2.0 = deterministic forward
                params.add_param("rate_knots", times.len() as f64);
            }
        }

        // Credit spread parameters
        params.add_param("spread_kappa", self.params.credit_spread.cir.kappa);
        params.add_param("spread_theta", self.params.credit_spread.cir.theta);
        params.add_param("spread_sigma", self.params.credit_spread.cir.sigma);
        params.add_param("spread_initial", self.params.credit_spread.initial);

        // Add correlation matrix if present (3x3 matrix)
        let params = if let Some(ref corr_matrix) = self.params.correlation {
            let correlation = vec![
                corr_matrix[0][0],
                corr_matrix[0][1],
                corr_matrix[0][2],
                corr_matrix[1][0],
                corr_matrix[1][1],
                corr_matrix[1][2],
                corr_matrix[2][0],
                corr_matrix[2][1],
                corr_matrix[2][2],
            ];
            params.with_correlation(correlation)
        } else {
            params
        };

        params.with_factors(vec![
            "utilization".to_string(),
            "short_rate".to_string(),
            "credit_spread".to_string(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utilization_params() {
        let params = UtilizationParams::new(0.5, 0.6, 0.1).expect("valid utilization params");
        assert_eq!(params.kappa, 0.5);
        assert_eq!(params.theta, 0.6);
        assert_eq!(params.sigma, 0.1);
    }

    /// Item 16 regression: invalid MC parameters must return
    /// `Error::Validation`, not panic via `assert!` inside library code.
    #[test]
    fn utilization_params_reject_invalid_inputs_without_panicking() {
        // Non-positive mean-reversion speed.
        let bad_kappa = UtilizationParams::new(0.0, 0.5, 0.1);
        assert!(
            matches!(bad_kappa, Err(finstack_core::Error::Validation(_))),
            "kappa=0 should yield a Validation error, got {bad_kappa:?}"
        );
        // theta outside [0, 1].
        let bad_theta = UtilizationParams::new(0.5, 1.5, 0.1);
        assert!(
            matches!(bad_theta, Err(finstack_core::Error::Validation(_))),
            "theta=1.5 should yield a Validation error, got {bad_theta:?}"
        );
        // Non-positive volatility.
        let bad_sigma = UtilizationParams::new(0.5, 0.5, 0.0);
        assert!(
            matches!(bad_sigma, Err(finstack_core::Error::Validation(_))),
            "sigma=0 should yield a Validation error, got {bad_sigma:?}"
        );
    }

    #[test]
    fn test_credit_spread_params() {
        let params = CreditSpreadParams::new(0.3, 0.02, 0.05, 0.015).unwrap();
        assert_eq!(params.initial, 0.015);
        assert_eq!(params.cir.kappa, 0.3);
    }

    /// Item 16 regression: a negative initial credit spread must return
    /// `Error::Validation`, not panic.
    #[test]
    fn credit_spread_params_reject_negative_initial_without_panicking() {
        let bad = CreditSpreadParams::new(0.3, 0.02, 0.05, -0.01);
        assert!(
            matches!(bad, Err(finstack_core::Error::Validation(_))),
            "negative initial spread should yield a Validation error, got {bad:?}"
        );
    }

    #[test]
    fn test_process_params_initial_state() {
        let utilization = UtilizationParams::new(0.5, 0.6, 0.1).expect("valid utilization params");
        let interest_rate = InterestRateSpec::Fixed { rate: 0.05 };
        let credit_spread = CreditSpreadParams::new(0.3, 0.02, 0.05, 0.015).unwrap();

        let params = RevolvingCreditProcessParams::new(utilization, interest_rate, credit_spread);
        let state = params.initial_state(0.5);

        assert_eq!(state[0], 0.5); // utilization
        assert_eq!(state[1], 0.05); // fixed rate
        assert_eq!(state[2], 0.015); // credit spread
    }

    #[test]
    fn test_process_params_floating_rate() {
        let utilization = UtilizationParams::new(0.5, 0.6, 0.1).expect("valid utilization params");
        let hw_params = HullWhite1FParams::new(0.1, 0.01, 0.03);
        let interest_rate = InterestRateSpec::Floating {
            params: hw_params,
            initial: 0.04,
        };
        let credit_spread = CreditSpreadParams::new(0.3, 0.02, 0.05, 0.015).unwrap();

        let params = RevolvingCreditProcessParams::new(utilization, interest_rate, credit_spread);
        let state = params.initial_state(0.5);

        assert_eq!(state[0], 0.5); // utilization
        assert_eq!(state[1], 0.04); // initial floating rate
        assert_eq!(state[2], 0.015); // credit spread
    }

    #[test]
    fn test_process_drift_fixed_rate() {
        let utilization = UtilizationParams::new(0.5, 0.6, 0.1).expect("valid utilization params");
        let interest_rate = InterestRateSpec::Fixed { rate: 0.05 };
        let credit_spread = CreditSpreadParams::new(0.3, 0.02, 0.05, 0.015).unwrap();

        let params = RevolvingCreditProcessParams::new(utilization, interest_rate, credit_spread);
        let process = RevolvingCreditProcess::new(params);

        let x = [0.5, 0.05, 0.015];
        let mut drift = [0.0; 3];

        process.drift(0.0, &x, &mut drift);

        // Utilization drift: 0.5 * (0.6 - 0.5) = 0.05
        assert!((drift[0] - 0.05).abs() < 1e-10);

        // Fixed rate drift: 0
        assert_eq!(drift[1], 0.0);

        // Credit spread drift: 0.3 * (0.02 - 0.015) = 0.0015
        assert!((drift[2] - 0.0015).abs() < 1e-10);
    }

    #[test]
    fn test_process_diffusion() {
        let utilization = UtilizationParams::new(0.5, 0.6, 0.1).expect("valid utilization params");
        let interest_rate = InterestRateSpec::Fixed { rate: 0.05 };
        let credit_spread = CreditSpreadParams::new(0.3, 0.02, 0.05, 0.015).unwrap();

        let params = RevolvingCreditProcessParams::new(utilization, interest_rate, credit_spread);
        let process = RevolvingCreditProcess::new(params);

        let x = [0.5, 0.05, 0.015];
        let mut diffusion = [0.0; 3];

        process.diffusion(0.0, &x, &mut diffusion);

        // Utilization: σ_U = 0.1
        assert_eq!(diffusion[0], 0.1);

        // Fixed rate: 0
        assert_eq!(diffusion[1], 0.0);

        // Credit spread: σ_λ * √λ = 0.05 * √0.015
        assert!((diffusion[2] - 0.05 * 0.015_f64.sqrt()).abs() < 1e-10);
    }

    #[test]
    fn test_factor_correlation() {
        let utilization = UtilizationParams::new(0.5, 0.6, 0.1).expect("valid utilization params");
        let interest_rate = InterestRateSpec::Fixed { rate: 0.05 };
        let credit_spread = CreditSpreadParams::new(0.3, 0.02, 0.05, 0.015).unwrap();

        let params = RevolvingCreditProcessParams::new(utilization, interest_rate, credit_spread);
        let process_no_corr = RevolvingCreditProcess::new(params);
        assert!(process_no_corr.factor_correlation().is_none());

        let utilization2 = UtilizationParams::new(0.5, 0.6, 0.1).expect("valid utilization params");
        let interest_rate2 = InterestRateSpec::Fixed { rate: 0.05 };
        let credit_spread2 = CreditSpreadParams::new(0.3, 0.02, 0.05, 0.015).unwrap();
        let correlation = [[1.0, 0.2, 0.1], [0.2, 1.0, 0.3], [0.1, 0.3, 1.0]];

        let params2 =
            RevolvingCreditProcessParams::new(utilization2, interest_rate2, credit_spread2)
                .with_correlation(correlation);
        let process_corr = RevolvingCreditProcess::new(params2);
        let corr = process_corr.factor_correlation().expect("correlation set");
        assert_eq!(corr.len(), 9);
        assert!((corr[1] - 0.2).abs() < 1e-12);
    }
}
