//! Schwartz-Smith two-factor commodity model.
//!
//! Implements a two-factor model for commodity prices that captures both
//! short-term mean-reverting deviations and long-term trends.
//!
//! # SDE
//!
//! ```text
//! dX_t = (-κ_X X_t - λ_X) dt + σ_X dW_X   // Short-term deviation (mean-reverting)
//! dY_t = μ_Y dt + σ_Y dW_Y                // Long-term trend (arithmetic BM on log-price)
//! S_t = exp(X_t + Y_t)                    // Spot price
//! ```
//!
//! where:
//! - X_t: Short-term deviation from long-term trend (OU process)
//! - Y_t: Long-term trend (arithmetic Brownian motion on the log price)
//! - κ_X: Mean reversion speed for short-term component
//! - λ_X: Constant short-term risk-premium drift shift (0 under the physical
//!   measure; Schwartz & Smith (2000) risk-neutralize the short-term factor
//!   by subtracting the constant λ_χ at **unchanged** κ — not by inflating κ)
//! - σ_X, σ_Y: Volatilities
//! - ρ: Correlation between X and Y
//!
//! # References
//!
//! - Schwartz, E. & Smith, J. E. (2000). "Short-Term Variations and Long-Term
//!   Dynamics in Commodity Prices." *Management Science*, 46(7), 893–911.

use super::super::traits::StochasticProcess;

/// Schwartz-Smith process parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchwartzSmithParams {
    /// Mean reversion speed for short-term deviation (κ_X)
    pub kappa_x: f64,
    /// Volatility of short-term component (σ_X)
    pub sigma_x: f64,
    /// Drift of long-term trend (μ_Y)
    pub mu_y: f64,
    /// Volatility of long-term component (σ_Y)
    pub sigma_y: f64,
    /// Correlation between X and Y (ρ)
    pub rho: f64,
    /// Constant risk-premium drift shift for the short-term factor (λ_X).
    ///
    /// Under the risk-neutral measure the short-term factor follows
    /// `dX = (−κ_X X − λ_X) dt + σ_X dW*` (Schwartz & Smith 2000): a constant
    /// drift shift at unchanged κ_X. Defaults to 0 (physical measure).
    #[serde(default)]
    pub lambda_x: f64,
}

impl SchwartzSmithParams {
    /// Create new Schwartz-Smith parameters.
    ///
    /// # Arguments
    ///
    /// * `kappa_x` - Mean reversion speed (must be > 0)
    /// * `sigma_x` - Short-term volatility (must be > 0)
    /// * `mu_y` - Long-term drift (must be finite)
    /// * `sigma_y` - Long-term volatility (must be > 0)
    /// * `rho` - Correlation between X and Y (must be in [-1, 1])
    ///
    /// # Errors
    ///
    /// Returns an error when any positivity constraint is violated, when
    /// `mu_y` is non-finite, or when `rho` falls outside `[-1, 1]`.
    pub fn new(
        kappa_x: f64,
        sigma_x: f64,
        mu_y: f64,
        sigma_y: f64,
        rho: f64,
    ) -> finstack_core::Result<Self> {
        if !(kappa_x > 0.0 && kappa_x.is_finite()) {
            return Err(finstack_core::Error::Validation(format!(
                "Schwartz-Smith kappa_x must be finite and positive, got {kappa_x}"
            )));
        }
        if !(sigma_x > 0.0 && sigma_x.is_finite()) {
            return Err(finstack_core::Error::Validation(format!(
                "Schwartz-Smith sigma_x must be finite and positive, got {sigma_x}"
            )));
        }
        if !mu_y.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Schwartz-Smith mu_y must be finite, got {mu_y}"
            )));
        }
        if !(sigma_y > 0.0 && sigma_y.is_finite()) {
            return Err(finstack_core::Error::Validation(format!(
                "Schwartz-Smith sigma_y must be finite and positive, got {sigma_y}"
            )));
        }
        if !(rho.is_finite() && (-1.0..=1.0).contains(&rho)) {
            return Err(finstack_core::Error::Validation(format!(
                "Schwartz-Smith correlation rho must be finite and in [-1, 1], got {rho}"
            )));
        }

        Ok(Self {
            kappa_x,
            sigma_x,
            mu_y,
            sigma_y,
            rho,
            lambda_x: 0.0,
        })
    }

    /// Set the constant short-term risk-premium drift shift λ_X.
    ///
    /// Use this to express the Schwartz-Smith risk-neutral dynamics
    /// `dX = (−κ_X X − λ_X) dt + σ_X dW*`. The mean-reversion speed κ_X is
    /// unchanged under the measure change.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] when `lambda_x` is not
    /// finite.
    pub fn with_lambda_x(mut self, lambda_x: f64) -> finstack_core::Result<Self> {
        if !lambda_x.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Schwartz-Smith lambda_x must be finite, got {lambda_x}"
            )));
        }
        self.lambda_x = lambda_x;
        Ok(self)
    }
}

/// Schwartz-Smith two-factor commodity process.
///
/// State: [X_t, Y_t] where X_t is short-term deviation and Y_t is long-term trend
/// Spot price: S_t = exp(X_t + Y_t)
///
/// # State Variables
///
/// - `state[0]` = X_t (short-term deviation)
/// - `state[1]` = Y_t (long-term trend)
///
/// # Factors
///
/// Two correlated Brownian motions with correlation ρ.
#[derive(Debug, Clone)]
pub struct SchwartzSmithProcess {
    params: SchwartzSmithParams,
    /// Initial values [X_0, Y_0]
    initial: [f64; 2],
}

impl SchwartzSmithProcess {
    /// Create a new Schwartz-Smith process.
    ///
    /// # Arguments
    ///
    /// * `params` - Process parameters
    /// * `initial_x` - Initial short-term deviation X_0
    /// * `initial_y` - Initial long-term trend Y_0
    pub fn new(params: SchwartzSmithParams, initial_x: f64, initial_y: f64) -> Self {
        Self {
            params,
            initial: [initial_x, initial_y],
        }
    }

    /// Create from spot price and initial state.
    ///
    /// If X_0 = 0 and Y_0 = ln(S_0), then S_0 = exp(X_0 + Y_0) = exp(ln(S_0)) = S_0.
    ///
    /// # Arguments
    ///
    /// * `params` - Process parameters
    /// * `initial_spot` - Initial spot price S_0
    /// * `initial_x` - Initial short-term deviation (default 0.0 if None)
    pub fn from_spot(
        params: SchwartzSmithParams,
        initial_spot: f64,
        initial_x: Option<f64>,
    ) -> Self {
        let x_0 = initial_x.unwrap_or(0.0);
        let y_0 = initial_spot.ln() - x_0; // Ensure S_0 = exp(X_0 + Y_0)
        Self::new(params, x_0, y_0)
    }

    /// Get parameters.
    pub fn params(&self) -> &SchwartzSmithParams {
        &self.params
    }

    /// Get initial state.
    pub fn initial_state(&self) -> [f64; 2] {
        self.initial
    }

    /// Compute spot price from state [X, Y].
    pub fn spot_from_state(&self, state: &[f64]) -> f64 {
        assert_eq!(state.len(), 2, "State must have dimension 2");
        (state[0] + state[1]).exp()
    }

    /// Model futures price `F(0, τ) = E[S_τ]` under these dynamics.
    ///
    /// Schwartz & Smith (2000), eq. (9), adapted to this parameterization
    /// (X mean-reverting with constant drift shift −λ_X, Y arithmetic BM on
    /// the log price):
    ///
    /// ```text
    /// ln F(0, τ) = e^{−κτ}·X₀ + Y₀ + A(τ)
    /// A(τ) = μ_Y·τ − (1 − e^{−κτ})·λ_X/κ
    ///        + ½[ (1 − e^{−2κτ})·σ_X²/(2κ) + σ_Y²·τ
    ///             + 2(1 − e^{−κτ})·ρ·σ_X·σ_Y/κ ]
    /// ```
    ///
    /// When the parameters carry the risk-neutral drift (λ_X set, μ_Y = μ*),
    /// this is the arbitrage-free futures curve and the simulated spot
    /// satisfies `E[S_τ] = F(0, τ)` exactly under the exact discretization.
    ///
    /// # References
    ///
    /// - Schwartz, E. & Smith, J. E. (2000). "Short-Term Variations and
    ///   Long-Term Dynamics in Commodity Prices." *Management Science*,
    ///   46(7), 893–911 (eq. 9).
    #[must_use]
    pub fn futures_price(&self, tau: f64) -> f64 {
        let p = &self.params;
        let kappa = p.kappa_x;
        // (1 − e^{−κτ})/κ via exp_m1, stable as κτ → 0.
        let one_minus_exp_over_kappa = -(-kappa * tau).exp_m1() / kappa;
        let exp_kappa_tau = (-kappa * tau).exp();

        let var_x = p.sigma_x * p.sigma_x * (-(-2.0 * kappa * tau).exp_m1()) / (2.0 * kappa);
        let var_y = p.sigma_y * p.sigma_y * tau;
        let cov_xy = 2.0 * p.rho * p.sigma_x * p.sigma_y * one_minus_exp_over_kappa;

        let a_tau =
            p.mu_y * tau - p.lambda_x * one_minus_exp_over_kappa + 0.5 * (var_x + var_y + cov_xy);

        (exp_kappa_tau * self.initial[0] + self.initial[1] + a_tau).exp()
    }
}

impl StochasticProcess for SchwartzSmithProcess {
    fn dim(&self) -> usize {
        2
    }

    fn num_factors(&self) -> usize {
        2
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // dX: -κ_X·X − λ_X (mean reversion plus constant risk-premium shift)
        out[0] = -self.params.kappa_x * x[0] - self.params.lambda_x;
        // dY: μ_Y (constant drift)
        out[1] = self.params.mu_y;
    }

    fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        // Diffusion matrix (diagonal elements)
        // For correlated factors, the discretization will apply Cholesky
        out[0] = self.params.sigma_x;
        out[1] = self.params.sigma_y;
    }

    fn factor_correlation(&self) -> Option<Vec<f64>> {
        let rho = self.params.rho;
        Some(vec![1.0, rho, rho, 1.0])
    }

    fn populate_path_state(&self, x: &[f64], state: &mut super::super::traits::PathState) {
        // For Schwartz-Smith: state is [X, Y] but spot = exp(X + Y)
        if x.len() >= 2 {
            let spot = (x[0] + x[1]).exp();
            state.set(super::super::traits::state_keys::SPOT, spot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Near-zero vols: ln F(0,τ) reduces to the deterministic
    /// e^{−κτ}X₀ + Y₀ + μτ − (1 − e^{−κτ})λ/κ.
    #[test]
    fn test_futures_price_deterministic_limit() {
        let (kappa, mu, lambda, tau) = (1.5, 0.03, 0.02, 2.0);
        let params = SchwartzSmithParams::new(kappa, 1e-9, mu, 1e-9, 0.0)
            .unwrap()
            .with_lambda_x(lambda)
            .unwrap();
        let (x0, y0) = (0.1, 4.5);
        let process = SchwartzSmithProcess::new(params, x0, y0);

        let one_minus = 1.0 - (-kappa * tau).exp();
        let expected =
            ((-kappa * tau).exp() * x0 + y0 + mu * tau - lambda * one_minus / kappa).exp();
        let f = process.futures_price(tau);
        assert!(
            (f / expected - 1.0).abs() < 1e-10,
            "futures {f} should match deterministic limit {expected}"
        );
    }

    /// Engine-level pin: under the exact discretization, E[S_τ] must equal
    /// the closed-form futures price within MC stderr — this is the
    /// arbitrage consistency the futures reconstruction exists to provide.
    #[test]
    fn test_simulated_terminal_spot_matches_futures_price() {
        use super::super::super::discretization::schwartz_smith::ExactSchwartzSmith;
        use super::super::super::engine::McEngine;
        use super::super::super::payoff::vanilla::Forward;
        use super::super::super::rng::philox::PhiloxRng;
        use finstack_core::currency::Currency;

        let params = SchwartzSmithParams::new(1.5, 0.30, 0.02, 0.15, -0.4)
            .unwrap()
            .with_lambda_x(0.05)
            .unwrap();
        let process = SchwartzSmithProcess::from_spot(params, 90.0, Some(0.1));
        let disc = ExactSchwartzSmith::from_process(&process).expect("disc");

        let t = 1.0;
        let steps = 12usize;
        let engine = McEngine::builder()
            .num_paths(100_000)
            .uniform_grid(t, steps)
            .parallel(false)
            .build()
            .expect("engine");
        let payoff = Forward::long(0.0, 1.0, steps);
        let result = engine
            .price(
                &PhiloxRng::new(7),
                &process,
                &disc,
                &process.initial_state(),
                &payoff,
                Currency::USD,
                1.0,
            )
            .expect("pricing");

        let futures = process.futures_price(t);
        let mean = result.mean.amount();
        let tol = 4.0 * result.stderr;
        assert!(
            (mean - futures).abs() < tol,
            "E[S_T] = {mean:.4} should match futures {futures:.4} within {tol:.4}"
        );
    }

    #[test]
    fn test_schwartz_smith_creation() {
        let params = SchwartzSmithParams::new(2.0, 0.30, 0.02, 0.15, -0.5).unwrap();
        let process = SchwartzSmithProcess::new(params, 0.0, 4.5);

        assert_eq!(process.dim(), 2);
        assert_eq!(process.num_factors(), 2);
        assert!(process.factor_correlation().is_some());
    }

    #[test]
    fn test_schwartz_smith_from_spot() {
        let params = SchwartzSmithParams::new(2.0, 0.30, 0.02, 0.15, -0.5).unwrap();
        let spot_0 = 90.0;
        let process = SchwartzSmithProcess::from_spot(params, spot_0, None);

        let state = process.initial_state();
        let computed_spot = process.spot_from_state(&state);

        // S_0 = exp(X_0 + Y_0) = exp(0 + ln(90)) = 90
        assert!((computed_spot - spot_0).abs() < 1e-10);
    }

    #[test]
    fn test_schwartz_smith_drift() {
        let params = SchwartzSmithParams::new(2.0, 0.30, 0.02, 0.15, -0.5).unwrap();
        let process = SchwartzSmithProcess::new(params, 0.0, 4.5);

        let x = [0.1, 4.5];
        let mut drift = [0.0; 2];

        process.drift(0.0, &x, &mut drift);

        // dX/dt = -2.0 * 0.1 = -0.2
        assert!((drift[0] - (-0.2)).abs() < 1e-10);
        // dY/dt = 0.02
        assert!((drift[1] - 0.02).abs() < 1e-10);
    }

    #[test]
    fn test_schwartz_smith_diffusion() {
        let params = SchwartzSmithParams::new(2.0, 0.30, 0.02, 0.15, -0.5).unwrap();
        let process = SchwartzSmithProcess::new(params, 0.0, 4.5);

        let x = [0.1, 4.5];
        let mut diffusion = [0.0; 2];

        process.diffusion(0.0, &x, &mut diffusion);

        assert!((diffusion[0] - 0.30).abs() < 1e-10);
        assert!((diffusion[1] - 0.15).abs() < 1e-10);
    }

    #[test]
    fn test_spot_from_state() {
        let params = SchwartzSmithParams::new(2.0, 0.30, 0.02, 0.15, -0.5).unwrap();
        let process = SchwartzSmithProcess::new(params, 0.0, 4.5);

        let state = [0.0, 4.5]; // X=0, Y=ln(90)≈4.5
        let spot = process.spot_from_state(&state);

        // S = exp(0 + 4.5) ≈ 90
        assert!((spot - 90.0).abs() < 1.0); // Allow small tolerance
    }
}
