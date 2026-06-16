//! Quadratic-Exponential (QE) scheme for CIR process.
//!
//! The QE scheme (Andersen, 2008) ensures positive variance while maintaining
//! accuracy for the square-root diffusion process.
//!
//! This is extracted from the Heston QE implementation and can be reused
//! for standalone CIR processes (short rates, intensities).
//!
//! Reference: Andersen (2008) - "Simple and efficient simulation of the Heston stochastic volatility model"

use super::super::process::cir::CirProcess;
use super::super::traits::Discretization;
use super::qe_common::qe_step_variance;

/// QE discretization for CIR process.
///
/// Handles the square-root diffusion while ensuring positivity.
///
/// # Algorithm
///
/// For variance:
/// - Compute ψ = s²/m² (scaled variance)
/// - If ψ ≤ ψ_c: use power/gamma approximation
/// - If ψ > ψ_c: use exponential/uniform mixture
#[derive(Debug, Clone)]
pub struct QeCir {
    /// Critical value for ψ (default 1.5)
    psi_c: f64,
}

impl QeCir {
    /// Create a new QE CIR discretization.
    pub fn new() -> Self {
        Self { psi_c: 1.5 }
    }

    /// Create with custom ψ_c threshold.
    ///
    /// Andersen (2008, §3.2.4) requires ψ_c ∈ \[1, 2\]; see
    /// [`super::qe_heston::QeHeston::with_psi_c`].
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when `psi_c` is not in
    /// \[1, 2\].
    pub fn with_psi_c(psi_c: f64) -> finstack_quant_core::Result<Self> {
        super::qe_common::validate_psi_c(psi_c)?;
        Ok(Self { psi_c })
    }

    /// One QE step of the CIR process.
    ///
    /// Thin wrapper around [`qe_step_variance`]; see
    /// `qe_common` for the algorithm, references, and numerical
    /// safeguards shared with `QeHeston`.
    #[inline]
    fn step_variance(&self, v_t: f64, kappa: f64, theta: f64, sigma: f64, dt: f64, z: f64) -> f64 {
        qe_step_variance(v_t, kappa, theta, sigma, dt, z, self.psi_c)
    }
}

impl Default for QeCir {
    fn default() -> Self {
        Self::new()
    }
}

impl Discretization<CirProcess> for QeCir {
    fn step(
        &self,
        process: &CirProcess,
        _t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        _work: &mut [f64],
    ) {
        let params = process.params();
        let v_t = x[0].max(0.0);

        // Apply QE scheme
        let v_next = self.step_variance(v_t, params.kappa, params.theta, params.sigma, dt, z[0]);

        x[0] = v_next;
    }

    fn work_size(&self, _process: &CirProcess) -> usize {
        0 // No workspace needed
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::process::cir::{CirParams, CirProcess};
    use super::*;

    #[test]
    fn test_qe_cir_positivity() {
        let params = CirParams::new(0.5, 0.04, 0.1).unwrap();
        let process = CirProcess::new(params);
        let disc = QeCir::new();

        let t: f64 = 0.0;
        let dt: f64 = 0.01;
        let mut x = vec![0.04];
        let mut work = vec![0.0; disc.work_size(&process)];

        // Test many shocks - variance should stay non-negative
        for shock in [-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0] {
            let z = vec![shock];
            disc.step(&process, t, dt, &mut x, &z, &mut work);

            assert!(x[0] >= 0.0, "Variance should be non-negative");
        }
    }

    #[test]
    fn test_qe_cir_mean_reversion() {
        let params = CirParams::new(0.5, 0.04, 0.1).unwrap();
        let process = CirProcess::new(params);
        let disc = QeCir::new();

        let t: f64 = 0.0;
        let dt: f64 = 0.1;
        let mut work = vec![0.0; disc.work_size(&process)];

        // Start above mean with no shock
        let mut x = vec![0.06];
        let z = vec![0.0];
        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // Should move toward mean
        assert!(x[0] < 0.06);
        assert!(x[0] > 0.04);
    }

    #[test]
    fn test_qe_cir_feller_violation() {
        // Test case where Feller condition is violated
        let params = CirParams::new(0.1, 0.01, 0.2).unwrap();
        assert!(!params.satisfies_feller());

        let process = CirProcess::new(params);
        let disc = QeCir::new();

        let t: f64 = 0.0;
        let dt: f64 = 0.05;
        let mut x = vec![0.005];
        let z = vec![-2.0]; // Large negative shock
        let mut work = vec![0.0; disc.work_size(&process)];

        disc.step(&process, t, dt, &mut x, &z, &mut work);

        // QE should handle this gracefully and maintain non-negativity
        assert!(x[0] >= 0.0);
    }
}
