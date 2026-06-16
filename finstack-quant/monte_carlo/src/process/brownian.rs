//! Brownian motion (Wiener) processes for generic time series modeling.
//!
//! Provides additive Gaussian processes that are useful beyond financial pricing,
//! e.g., generic diffusion dynamics for continuous time series.
//!
//! # Dynamics
//!
//! The one-dimensional process follows
//!
//! ```text
//! dX_t = μ dt + σ dW_t
//! ```
//!
//! where `μ` is the drift per year and `σ` is the diffusion scale per square
//! root year. The multi-dimensional variant applies the same form componentwise
//! and can attach an optional row-major correlation matrix for downstream
//! metadata consumers.

use super::super::paths::ProcessParams;
use super::super::traits::StochasticProcess;
use super::metadata::ProcessMetadata;

/// Parameters for one-dimensional Brownian motion with drift.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BrownianParams {
    /// Constant drift per year.
    pub mu: f64,
    /// Constant diffusion scale per square root year.
    pub sigma: f64,
}

impl BrownianParams {
    /// Create Brownian-motion parameters.
    ///
    /// # Arguments
    ///
    /// * `mu` - Constant drift per year.
    /// * `sigma` - Constant diffusion scale per square root year.
    pub fn new(mu: f64, sigma: f64) -> Self {
        Self { mu, sigma }
    }
}

/// One-dimensional additive Brownian motion with constant drift and diffusion.
#[derive(Debug, Clone)]
pub struct BrownianProcess {
    params: BrownianParams,
}

impl BrownianProcess {
    /// Create a Brownian process from explicit parameters.
    pub fn new(params: BrownianParams) -> Self {
        Self { params }
    }

    /// Create a Brownian process from `mu` and `sigma`.
    pub fn with_params(mu: f64, sigma: f64) -> Self {
        Self::new(BrownianParams::new(mu, sigma))
    }

    /// Drift parameter μ.
    pub fn mu(&self) -> f64 {
        self.params.mu
    }

    /// Diffusion parameter σ.
    pub fn sigma(&self) -> f64 {
        self.params.sigma
    }
}

impl StochasticProcess for BrownianProcess {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        1
    }

    fn drift(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        // dX = μ dt + σ dW
        out[0] = self.params.mu;
    }

    fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out[0] = self.params.sigma;
    }
}

impl ProcessMetadata for BrownianProcess {
    fn metadata(&self) -> ProcessParams {
        let mut p = ProcessParams::new("Brownian");
        p.add_param("mu", self.params.mu);
        p.add_param("sigma", self.params.sigma);
        p.with_factors(vec!["x".to_string()])
    }
}

/// Multi-dimensional Brownian motion.
///
/// Correlation, when present, is metadata describing the relationship between
/// the components. Simulation code that needs correlated shocks should apply the
/// appropriate transformation before calling a diagonal discretization scheme.
#[derive(Debug, Clone)]
pub struct MultiBrownianProcess {
    mus: Vec<f64>,
    sigmas: Vec<f64>,
    /// Optional row-major `n x n` correlation matrix.
    correlation: Option<Vec<f64>>,
}

impl MultiBrownianProcess {
    /// Create a multi-dimensional Brownian motion.
    ///
    /// # Arguments
    ///
    /// * `mus` - Per-component drifts per year.
    /// * `sigmas` - Per-component diffusion scales per square root year.
    /// * `correlation` - Optional row-major `n x n` correlation matrix.
    pub fn new(mus: Vec<f64>, sigmas: Vec<f64>, correlation: Option<Vec<f64>>) -> Self {
        assert_eq!(
            mus.len(),
            sigmas.len(),
            "mus and sigmas must have same length"
        );
        if let Some(ref corr) = correlation {
            let n = mus.len();
            assert_eq!(corr.len(), n * n, "Correlation matrix must be n x n");
        }
        Self {
            mus,
            sigmas,
            correlation,
        }
    }

    /// Dimension.
    pub fn dim(&self) -> usize {
        self.mus.len()
    }

    /// Correlation matrix if present.
    pub fn correlation(&self) -> Option<&[f64]> {
        self.correlation.as_deref()
    }
}

impl StochasticProcess for MultiBrownianProcess {
    fn dim(&self) -> usize {
        self.mus.len()
    }

    fn num_factors(&self) -> usize {
        self.mus.len()
    }

    fn drift(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out.copy_from_slice(&self.mus);
    }

    fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out.copy_from_slice(&self.sigmas);
    }

    fn factor_correlation(&self) -> Option<Vec<f64>> {
        self.correlation.clone()
    }
}

impl ProcessMetadata for MultiBrownianProcess {
    fn metadata(&self) -> ProcessParams {
        let mut p = ProcessParams::new("MultiBrownian");
        for (i, (&mu, &sigma)) in self.mus.iter().zip(self.sigmas.iter()).enumerate() {
            p.add_param(format!("mu_{}", i), mu);
            p.add_param(format!("sigma_{}", i), sigma);
        }
        let p = if let Some(ref corr) = self.correlation {
            p.with_correlation(corr.clone())
        } else {
            p
        };
        let factor_names: Vec<String> = (0..self.mus.len()).map(|i| format!("x_{}", i)).collect();
        p.with_factors(factor_names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brownian_drift_diffusion() {
        let proc = BrownianProcess::with_params(0.1, 0.3);
        let mut mu = [0.0];
        let mut sig = [0.0];
        proc.drift(0.0, &[0.0], &mut mu);
        proc.diffusion(0.0, &[0.0], &mut sig);
        assert!((mu[0] - 0.1).abs() < 1e-12);
        assert!((sig[0] - 0.3).abs() < 1e-12);
        assert!(proc.factor_correlation().is_none());
    }

    #[test]
    fn test_multi_brownian_metadata() {
        let mu = vec![0.1, -0.2];
        let sig = vec![0.3, 0.5];
        let corr = vec![1.0, 0.2, 0.2, 1.0];
        let proc = MultiBrownianProcess::new(mu, sig, Some(corr));
        assert_eq!(proc.dim(), 2);
        let md = proc.metadata();
        assert_eq!(md.process_type, "MultiBrownian");
        assert_eq!(md.parameters.get("mu_0"), Some(&0.1));
        assert_eq!(md.parameters.get("sigma_1"), Some(&0.5));
        assert!(md.correlation.is_some());
        assert_eq!(md.factor_names, vec!["x_0".to_string(), "x_1".to_string()]);
    }
}
