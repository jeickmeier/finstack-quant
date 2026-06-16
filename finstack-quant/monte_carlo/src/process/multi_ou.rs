//! Multi-dimensional Ornstein–Uhlenbeck (OU) process for generic time series.
//!
//! State dynamics for each component i:
//! dX_i = κ_i (θ_i - X_i) dt + σ_i dW_i
//! with optional correlation across the driving Brownian motions.
//!
//! `κ_i` is the mean-reversion speed per year, `θ_i` is the long-run level in
//! state units, and `σ_i` is the diffusion scale per square root year.

use super::super::paths::ProcessParams;
use super::super::traits::StochasticProcess;
use super::metadata::ProcessMetadata;

/// Parameters for a multi-dimensional Ornstein-Uhlenbeck process.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MultiOuParams {
    /// Mean-reversion speeds `κ_i` per year.
    pub kappas: Vec<f64>,
    /// Long-run means `θ_i` in state units.
    pub thetas: Vec<f64>,
    /// Diffusion scales `σ_i` per square root year.
    pub sigmas: Vec<f64>,
    /// Optional row-major `n x n` correlation matrix.
    pub correlation: Option<Vec<f64>>,
}

impl MultiOuParams {
    /// Create OU parameters.
    ///
    /// # Arguments
    ///
    /// * `kappas` - Mean-reversion speeds per year.
    /// * `thetas` - Long-run levels in state units.
    /// * `sigmas` - Diffusion scales per square root year.
    /// * `correlation` - Optional row-major `n x n` correlation matrix.
    pub fn new(
        kappas: Vec<f64>,
        thetas: Vec<f64>,
        sigmas: Vec<f64>,
        correlation: Option<Vec<f64>>,
    ) -> Self {
        let n = kappas.len();
        assert_eq!(thetas.len(), n, "thetas length must match kappas");
        assert_eq!(sigmas.len(), n, "sigmas length must match kappas");
        if let Some(ref corr) = correlation {
            assert_eq!(corr.len(), n * n, "correlation must be n x n");
        }
        Self {
            kappas,
            thetas,
            sigmas,
            correlation,
        }
    }
}

/// Multi-dimensional Ornstein-Uhlenbeck process.
#[derive(Debug, Clone)]
pub struct MultiOuProcess {
    params: MultiOuParams,
}

impl MultiOuProcess {
    /// Create a multi-dimensional Ornstein-Uhlenbeck process.
    pub fn new(params: MultiOuParams) -> Self {
        Self { params }
    }

    /// Return the number of state variables in the process.
    pub fn dim(&self) -> usize {
        self.params.kappas.len()
    }

    /// Borrow the optional row-major correlation matrix.
    pub fn correlation(&self) -> Option<&[f64]> {
        self.params.correlation.as_deref()
    }
}

impl StochasticProcess for MultiOuProcess {
    fn dim(&self) -> usize {
        self.dim()
    }

    fn num_factors(&self) -> usize {
        self.dim()
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        for i in 0..self.dim() {
            out[i] = self.params.kappas[i] * (self.params.thetas[i] - x[i]);
        }
    }

    fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out.copy_from_slice(&self.params.sigmas);
    }

    fn factor_correlation(&self) -> Option<Vec<f64>> {
        self.params.correlation.clone()
    }
}

impl ProcessMetadata for MultiOuProcess {
    fn metadata(&self) -> ProcessParams {
        let mut p = ProcessParams::new("MultiOU");
        for i in 0..self.dim() {
            p.add_param(format!("kappa_{}", i), self.params.kappas[i]);
            p.add_param(format!("theta_{}", i), self.params.thetas[i]);
            p.add_param(format!("sigma_{}", i), self.params.sigmas[i]);
        }
        let p = if let Some(ref corr) = self.params.correlation {
            p.with_correlation(corr.clone())
        } else {
            p
        };
        let factor_names: Vec<String> = (0..self.dim()).map(|i| format!("x_{}", i)).collect();
        p.with_factors(factor_names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_ou_drift_diffusion() {
        let params = MultiOuParams::new(vec![2.0, 1.0], vec![1.0, -1.0], vec![0.3, 0.4], None);
        let proc = MultiOuProcess::new(params);
        let x = [0.0, 0.0];
        let mut mu = [0.0, 0.0];
        let mut sig = [0.0, 0.0];
        proc.drift(0.0, &x, &mut mu);
        proc.diffusion(0.0, &x, &mut sig);
        assert!((mu[0] - 2.0 * (1.0 - 0.0)).abs() < 1e-12);
        assert!((mu[1] - 1.0 * (-1.0 - 0.0)).abs() < 1e-12);
        assert!((sig[0] - 0.3).abs() < 1e-12);
        assert!((sig[1] - 0.4).abs() < 1e-12);
    }
}
