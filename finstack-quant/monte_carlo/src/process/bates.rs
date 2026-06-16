//! Bates model (Heston stochastic volatility + Merton jumps).
//!
//! Combines stochastic volatility (Heston) with Poisson jumps (Merton)
//! for modeling equity derivatives with:
//! - Stochastic volatility smile
//! - Fat tails from jumps
//! - Volatility clustering
//!
//! # Bates SDE
//!
//! ```text
//! dS_t/S_t = (r - q - λk)dt + √v_t dW_t^S + (J-1)dN_t
//! dv_t = κ(θ - v_t)dt + σ_v√v_t dW_t^v
//! ```
//!
//! where:
//! - v_t = stochastic variance (CIR process)
//! - Corr(W^S, W^v) = ρ
//! - λ = jump intensity, N_t a Poisson process with rate λ
//! - ln J ~ Normal(μ_J, σ_J²), compensator k = E[J − 1] = e^{μ_J + σ_J²/2} − 1
//!
//! # Supported discretization
//!
//! Use [`crate::discretization::qe_bates::QeBates`] — it is the **only**
//! scheme that applies the jump leg. Pairing this process with the generic
//! Euler/log-Euler schemes type-checks but silently simulates the diffusion
//! only: the jumps and the spot–vol correlation are dropped while the drift
//! still subtracts the jump compensator `λk`, breaking the martingale
//! `E[S_T] = S₀·e^{(r−q)T}` by exactly the compensator.
//!
//! # References
//!
//! - Bates, D. S. (1996). "Jumps and Stochastic Volatility: Exchange Rate
//!   Processes Implicit in Deutsche Mark Options." *Review of Financial
//!   Studies*, 9(1), 69–107.

use super::super::traits::StochasticProcess;
use super::heston::{HestonParams, HestonProcess};
use super::jump_diffusion::MertonJumpParams;

/// Bates model parameters (Heston + jumps).
///
/// The diffusive volatility comes from the Heston block; the `jump.gbm.sigma`
/// field of the Merton block is ignored by the Bates dynamics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatesParams {
    /// Heston parameters (spot dynamics + variance)
    pub heston: HestonParams,
    /// Jump parameters (intensity, distribution)
    pub jump: MertonJumpParams,
}

impl BatesParams {
    /// Create new Bates parameters.
    ///
    /// # Errors
    ///
    /// Returns an error when the Heston and jump parameter blocks disagree on
    /// the risk-free rate or dividend yield (mismatches more than `1e-12`).
    /// Both blocks must reference the same risk-neutral measure.
    pub fn new(heston: HestonParams, jump: MertonJumpParams) -> finstack_quant_core::Result<Self> {
        if (heston.r - jump.gbm.r).abs() >= 1e-12 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Risk-free rate mismatch between Heston (r={}) and jump (r={}) params",
                heston.r, jump.gbm.r
            )));
        }
        if (heston.q - jump.gbm.q).abs() >= 1e-12 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Dividend yield mismatch between Heston (q={}) and jump (q={}) params",
                heston.q, jump.gbm.q
            )));
        }

        Ok(Self { heston, jump })
    }

    /// Compensated drift for the risk-neutral measure: `r − q − λk` with
    /// `k = e^{μ_J + σ_J²/2} − 1`.
    pub fn compensated_drift(&self) -> f64 {
        self.heston.r - self.heston.q - self.jump.lambda * self.jump.jump_compensation()
    }
}

/// Bates process (Heston + Merton jumps).
///
/// State dimension: 2 (spot S, variance v).
/// Factor dimension: 4 — `z[0]` spot diffusion, `z[1]` variance shock,
/// `z[2]` Poisson jump-count shock, `z[3]` aggregate jump-size shock
/// (the layout consumed by [`crate::discretization::qe_bates::QeBates`]).
///
/// # Usage
///
/// Price with [`crate::discretization::qe_bates::QeBates`] only — see the
/// module documentation for why the generic Euler schemes must not be used
/// with this process.
#[derive(Debug, Clone)]
pub struct BatesProcess {
    params: BatesParams,
    /// Heston leg with the jump compensator absorbed into the rate
    /// (`r_eff = r − λk`), so the diffusive K0* step of `QeBates` carries
    /// the compensator exactly once.
    compensated_heston: HestonProcess,
}

impl BatesProcess {
    /// Create a new Bates process.
    pub fn new(params: BatesParams) -> Self {
        // `r − λk` preserves every HestonParams validity constraint (only
        // the rate changes), so reconstructing the params cannot fail.
        let mut heston_params = params.heston.clone();
        heston_params.r -= params.jump.lambda * params.jump.jump_compensation();
        let compensated_heston = HestonProcess::new(heston_params);
        Self {
            params,
            compensated_heston,
        }
    }

    /// Get parameters.
    pub fn params(&self) -> &BatesParams {
        &self.params
    }

    /// Get the Heston component (uncompensated rate).
    pub fn heston(&self) -> HestonProcess {
        HestonProcess::new(self.params.heston.clone())
    }

    /// The Heston leg with the jump compensator absorbed into the rate
    /// (`r_eff = r − λk`). Used by
    /// [`crate::discretization::qe_bates::QeBates`] so the compensator
    /// appears in the spot drift exactly once.
    pub(crate) fn compensated_heston(&self) -> &HestonProcess {
        &self.compensated_heston
    }
}

impl StochasticProcess for BatesProcess {
    fn dim(&self) -> usize {
        2 // Spot and variance
    }

    fn num_factors(&self) -> usize {
        4 // S diffusion, v diffusion, Poisson count, aggregate jump size
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        let s = x[0];
        let v = x[1].max(0.0);

        // Spot drift: (r - q - λk) S. Correct only when paired with a scheme
        // that also applies the jump leg (QeBates) — see the module docs.
        out[0] = self.params.compensated_drift() * s;

        // Variance drift: κ(θ - v) (full truncation, matching Heston)
        out[1] = self.params.heston.kappa * (self.params.heston.theta - v);
    }

    fn diffusion(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        let s = x[0];
        let v = x[1].max(0.0);

        // Spot diffusion: √v S (stochastic vol; jumps are not a diffusion term)
        out[0] = v.sqrt() * s;

        // Variance diffusion: σ_v √v
        out[1] = self.params.heston.sigma_v * v.sqrt();
    }

    fn dedicated_scheme(&self) -> Option<&'static str> {
        // Only QeBates applies the jump leg; a generic scheme would simulate
        // the diffusion alone while the drift still subtracts the jump
        // compensator, biasing the forward.
        Some("qe_bates")
    }

    fn factor_correlation(&self) -> Option<Vec<f64>> {
        // 4×4: spot/variance correlated by ρ; jump factors independent.
        // QeBates declares `applies_correlation_internally`, so this matrix
        // is consulted only by generic schemes (which must not be used with
        // this process — see the module docs).
        let rho = self.params.heston.rho;
        #[rustfmt::skip]
        let corr = vec![
            1.0, rho, 0.0, 0.0,
            rho, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,
        ];
        Some(corr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bates_params() {
        let heston = HestonParams::new(0.05, 0.02, 0.5, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let jump = MertonJumpParams::new(0.05, 0.02, 0.0, 1.0, -0.05, 0.1).unwrap();

        let bates = BatesParams::new(heston, jump).expect("matching r/q");

        assert_eq!(bates.heston.r, 0.05);
        assert_eq!(bates.jump.lambda, 1.0);
    }

    #[test]
    fn test_bates_params_rejects_measure_mismatch() {
        let heston = HestonParams::new(0.05, 0.02, 0.5, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let jump = MertonJumpParams::new(0.04, 0.02, 0.0, 1.0, -0.05, 0.1).unwrap();
        assert!(BatesParams::new(heston, jump).is_err());
    }

    #[test]
    fn test_bates_compensated_drift() {
        let heston = HestonParams::new(0.05, 0.02, 0.5, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let jump = MertonJumpParams::new(0.05, 0.02, 0.0, 2.0, 0.0, 0.05).unwrap();

        let bates = BatesParams::new(heston, jump).expect("matching r/q");

        let drift = bates.compensated_drift();

        // Should be r - q - λk
        let expected = 0.05 - 0.02 - bates.jump.lambda * bates.jump.jump_compensation();
        assert!((drift - expected).abs() < 1e-10);
    }

    #[test]
    fn test_bates_process_drift_and_compensated_heston() {
        let heston = HestonParams::new(0.05, 0.02, 0.5, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let jump = MertonJumpParams::new(0.05, 0.02, 0.0, 1.0, -0.02, 0.08).unwrap();
        let bates_params = BatesParams::new(heston, jump).expect("matching r/q");

        let process = BatesProcess::new(bates_params);

        let x = vec![100.0, 0.04]; // S=100, v=0.04
        let mut drift = vec![0.0, 0.0];
        process.drift(0.0, &x, &mut drift);

        // Spot drift should be compensated
        let expected_spot_drift = process.params().compensated_drift() * 100.0;
        assert!((drift[0] - expected_spot_drift).abs() < 1e-6);

        // Variance drift
        assert_eq!(drift[1], 0.5 * (0.04 - 0.04));

        // The compensated Heston leg absorbs the compensator into the rate
        // and changes nothing else.
        let leg = process.compensated_heston().params();
        let lambda_k = process.params().jump.lambda * process.params().jump.jump_compensation();
        assert!((leg.r - (0.05 - lambda_k)).abs() < 1e-15);
        assert_eq!(leg.q, 0.02);
        assert_eq!(leg.kappa, 0.5);
        assert_eq!(leg.rho, -0.7);
    }

    #[test]
    fn test_bates_process_diffusion() {
        let heston = HestonParams::new(0.05, 0.02, 0.5, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let jump = MertonJumpParams::new(0.05, 0.02, 0.0, 1.0, 0.0, 0.1).unwrap();
        let bates_params = BatesParams::new(heston, jump).expect("matching r/q");

        let process = BatesProcess::new(bates_params);

        let x = vec![100.0, 0.04]; // S=100, v=0.04
        let mut diffusion = vec![0.0, 0.0];

        process.diffusion(0.0, &x, &mut diffusion);

        // Spot diffusion: √v * S = √0.04 * 100 = 0.2 * 100 = 20
        assert_eq!(diffusion[0], 0.04_f64.sqrt() * 100.0);

        // Variance diffusion: σ_v * √v = 0.3 * 0.2 = 0.06
        assert_eq!(diffusion[1], 0.3 * 0.04_f64.sqrt());
    }

    #[test]
    fn test_bates_factor_layout() {
        let heston = HestonParams::new(0.05, 0.02, 0.5, 0.04, 0.3, -0.7, 0.04).expect("valid");
        let jump = MertonJumpParams::new(0.05, 0.02, 0.0, 1.0, 0.0, 0.1).unwrap();
        let process = BatesProcess::new(BatesParams::new(heston, jump).expect("matching r/q"));

        assert_eq!(process.dim(), 2);
        assert_eq!(process.num_factors(), 4);
        let corr = process.factor_correlation().expect("correlation matrix");
        assert_eq!(corr.len(), 16);
        assert_eq!(corr[1], -0.7); // spot/variance ρ
        assert_eq!(corr[10], 1.0); // jump factors independent
    }
}
