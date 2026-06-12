//! LIBOR Market Model (LMM/BGM) with displaced diffusion.
//!
//! The LMM models the joint evolution of N forward rates under the terminal
//! measure, enabling consistent pricing of exotic rate derivatives such as
//! Bermudan swaptions. This implementation supports 2–3 factor dynamics with
//! piecewise-constant instantaneous volatilities and displacement shifts for
//! negative-rate environments.
//!
//! # Stochastic Differential Equation
//!
//! Under the terminal measure $T_N$:
//!
//! ```text
//! dF_i(t) = μ_i(t, F) dt + (F_i(t) + d_i) Σ_k λ_{i,k}(t) dW_k(t)
//! ```
//!
//! where:
//! - `F_i(t)` = i-th SOFR forward rate for period `[T_i, T_{i+1}]`
//! - `d_i` = displacement (shift) for negative-rate support
//! - `λ_{i,k}(t)` = factor loading of forward i on Brownian motion k
//! - `μ_i` = drift correction under terminal measure
//!
//! # Drift Under Terminal Measure
//!
//! ```text
//! μ_i(t, F) = -Σ_{j=i+1}^{N-1} [τ_j (F_j + d_j) / (1 + τ_j F_j)] ρ_{ij} σ_i σ_j
//! ```
//!
//! where `ρ_{ij} = λ_i · λ_j` and `σ_i = |λ_i|`.
//!
//! # References
//!
//! - Brace, Gatarek, Musiela (1997). "The Market Model of Interest Rate
//!   Dynamics." *Mathematical Finance*, 7(2), 127-155.
//! - Glasserman (2003). *Monte Carlo Methods in Financial Engineering*,
//!   Ch. 7, Springer.
//! - Rebonato (2002). *Modern Pricing of Interest-Rate Derivatives*,
//!   Ch. 8-9, Princeton University Press.
//! - Andersen & Piterbarg (2010). *Interest Rate Modeling*, Vol. 2,
//!   Ch. 15-16, Atlantic Financial Press.

use super::super::traits::{state_keys, StochasticProcess};

/// Maximum number of LMM factors supported.
const MAX_FACTORS: usize = 3;

/// Parameters for the LMM/BGM model.
///
/// Encapsulates all static configuration needed to evolve forward rates:
/// tenor schedule, displacements, piecewise-constant factor loadings, and
/// initial forward rates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LmmParams {
    /// Number of forward rates (N).
    pub num_forwards: usize,
    /// Number of Brownian factors (2 or 3).
    pub num_factors: usize,
    /// Tenor dates T_0, T_1, ..., T_N (N+1 dates, year fractions).
    pub tenors: Vec<f64>,
    /// Accrual factors τ_i = T_{i+1} − T_i (length N).
    pub accrual_factors: Vec<f64>,
    /// Displacement per forward for negative-rate support (length N).
    pub displacements: Vec<f64>,
    /// Piecewise-constant vol breakpoints (ascending, length M).
    pub vol_times: Vec<f64>,
    /// Factor loadings λ_{i,k}(t) per forward, per time period.
    ///
    /// Outer: M+1 time periods.
    /// Inner: N forwards, each with up to 3 factor loadings.
    pub vol_values: Vec<Vec<[f64; MAX_FACTORS]>>,
    /// Initial forward rates F_i(0) from the curve (length N).
    pub initial_forwards: Vec<f64>,
}

impl LmmParams {
    /// Validate and construct LMM parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if dimensions are inconsistent, factor count is out of
    /// range, or any input is non-finite.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        num_forwards: usize,
        num_factors: usize,
        tenors: Vec<f64>,
        accrual_factors: Vec<f64>,
        displacements: Vec<f64>,
        vol_times: Vec<f64>,
        vol_values: Vec<Vec<[f64; MAX_FACTORS]>>,
        initial_forwards: Vec<f64>,
    ) -> finstack_core::Result<Self> {
        if num_forwards == 0 {
            return Err(finstack_core::Error::Validation(
                "LMM requires at least one forward rate".to_string(),
            ));
        }
        if !(2..=MAX_FACTORS).contains(&num_factors) {
            return Err(finstack_core::Error::Validation(format!(
                "LMM supports 2-{MAX_FACTORS} factors, got {num_factors}"
            )));
        }
        if tenors.len() != num_forwards + 1 {
            return Err(finstack_core::Error::Validation(format!(
                "tenors length must be num_forwards+1 ({}), got {}",
                num_forwards + 1,
                tenors.len()
            )));
        }
        if accrual_factors.len() != num_forwards {
            return Err(finstack_core::Error::Validation(format!(
                "accrual_factors length must be {num_forwards}, got {}",
                accrual_factors.len()
            )));
        }
        if displacements.len() != num_forwards {
            return Err(finstack_core::Error::Validation(format!(
                "displacements length must be {num_forwards}, got {}",
                displacements.len()
            )));
        }
        if initial_forwards.len() != num_forwards {
            return Err(finstack_core::Error::Validation(format!(
                "initial_forwards length must be {num_forwards}, got {}",
                initial_forwards.len()
            )));
        }
        let num_periods = vol_times.len() + 1;
        if vol_values.len() != num_periods {
            return Err(finstack_core::Error::Validation(format!(
                "vol_values length must be {} (vol_times.len()+1), got {}",
                num_periods,
                vol_values.len()
            )));
        }
        for (p, period) in vol_values.iter().enumerate() {
            if period.len() != num_forwards {
                return Err(finstack_core::Error::Validation(format!(
                    "vol_values[{p}] length must be {num_forwards}, got {}",
                    period.len()
                )));
            }
        }
        // Validate finite values
        for (i, t) in tenors.iter().enumerate() {
            if !t.is_finite() || (i > 0 && *t <= tenors[i - 1]) {
                return Err(finstack_core::Error::Validation(format!(
                    "tenors must be finite and strictly increasing, issue at index {i}"
                )));
            }
        }
        for (i, tau) in accrual_factors.iter().enumerate() {
            if !tau.is_finite() || *tau <= 0.0 {
                return Err(finstack_core::Error::Validation(format!(
                    "accrual_factors[{i}] must be positive and finite, got {tau}"
                )));
            }
        }

        Ok(Self {
            num_forwards,
            num_factors,
            tenors,
            accrual_factors,
            displacements,
            vol_times,
            vol_values,
            initial_forwards,
        })
    }

    /// Return the vol period index for time t.
    fn vol_period(&self, t: f64) -> usize {
        match self
            .vol_times
            .binary_search_by(|v| v.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Equal))
        {
            Ok(i) => i + 1, // t exactly matches a breakpoint → next period
            Err(i) => i,    // t falls before breakpoint i
        }
    }

    /// Return factor loadings for forward `i` at time `t`.
    #[inline]
    pub(crate) fn factor_loadings(&self, i: usize, t: f64) -> &[f64; MAX_FACTORS] {
        let p = self.vol_period(t);
        &self.vol_values[p][i]
    }
}

/// LMM/BGM stochastic process with displaced diffusion.
///
/// State vector: `[F_0, F_1, ..., F_{N-1}]` — the N forward rates.
///
/// The process uses the terminal measure (`T_N`) numeraire convention.
/// [`populate_path_state`](StochasticProcess::populate_path_state) stores
/// each forward rate as `indexed_spot(i)` and additionally stores, under
/// `"lmm_numeraire"`, the product `Π_{j ≥ first_alive} 1/(1 + τ_j F_j)` —
/// i.e. `P(t, T_N) / P(t, T_{first_alive})`, **not** `P(t, T_N)` itself:
/// the stub discount over `[t, T_{first_alive})` is omitted (and nothing
/// divides by `P(0, T_N)`). Consumers must either evaluate only at tenor
/// dates where the stub is empty, or — like the Bermudan LSMC pricer —
/// form ratios in which the common `T_{first_alive}` reference cancels.
#[derive(Debug, Clone)]
pub struct LmmProcess {
    /// Model parameters.
    params: LmmParams,
}

impl LmmProcess {
    /// Create a new LMM process from validated parameters.
    pub fn new(params: LmmParams) -> Self {
        Self { params }
    }

    /// Access the underlying parameters.
    pub fn params(&self) -> &LmmParams {
        &self.params
    }

    /// Return index of the first alive forward at time `t`.
    ///
    /// Forward `i` is alive if `T_i >= t` (it fixes at T_i but is still
    /// active for evolution at that instant).
    #[inline]
    pub(crate) fn first_alive(&self, t: f64) -> usize {
        // Find first tenor >= t (alive forwards have T_i >= t).
        self.params.tenors[..self.params.num_forwards].partition_point(|&v| v < t)
    }

    /// Compute terminal-measure drift for all alive forwards.
    ///
    /// Writes drift correction `μ_i` into `out[i]` for alive forwards.
    /// Dead forwards get zero drift.
    #[allow(clippy::needless_range_loop)]
    fn compute_drift(&self, t: f64, x: &[f64], out: &mut [f64]) {
        let n = self.params.num_forwards;
        let nf = self.params.num_factors;
        let first = self.first_alive(t);

        for val in out[..n].iter_mut() {
            *val = 0.0;
        }

        // Under terminal measure T_N, drift for forward i:
        // μ_i = -Σ_{j=i+1}^{N-1} [τ_j (F_j+d_j) / (1+τ_j F_j)] ρ_{ij} σ_i σ_j
        for i in first..n {
            let lam_i = self.params.factor_loadings(i, t);
            let d_i_shifted = x[i] + self.params.displacements[i];

            let mut mu = 0.0;
            for j in (i + 1)..n {
                let tau_j = self.params.accrual_factors[j];
                let f_j = x[j];
                let d_j = self.params.displacements[j];
                let denom = 1.0 + tau_j * f_j;
                if denom.abs() < 1e-15 {
                    continue; // avoid division by near-zero
                }
                let lam_j = self.params.factor_loadings(j, t);

                // ρ_{ij} σ_i σ_j = λ_i · λ_j (dot product)
                let dot: f64 = lam_i
                    .iter()
                    .zip(lam_j.iter())
                    .take(nf)
                    .map(|(a, b)| a * b)
                    .sum();

                mu -= tau_j * (f_j + d_j) / denom * dot;
            }
            // The full drift multiplier is μ_i * (F_i + d_i)
            out[i] = mu * d_i_shifted;
        }
    }
}

impl StochasticProcess for LmmProcess {
    fn dim(&self) -> usize {
        self.params.num_forwards
    }

    fn num_factors(&self) -> usize {
        self.params.num_factors
    }

    fn drift(&self, t: f64, x: &[f64], out: &mut [f64]) {
        self.compute_drift(t, x, out);
    }

    fn diffusion(&self, t: f64, x: &[f64], out: &mut [f64]) {
        let n = self.params.num_forwards;
        let nf = self.params.num_factors;
        let first = self.first_alive(t);

        // out is row-major N × K matrix
        for val in out[..n * nf].iter_mut() {
            *val = 0.0;
        }

        for i in first..n {
            let d_i_shifted = x[i] + self.params.displacements[i];
            let lam = self.params.factor_loadings(i, t);
            for k in 0..nf {
                out[i * nf + k] = d_i_shifted * lam[k];
            }
        }
    }

    fn populate_path_state(&self, x: &[f64], state: &mut super::super::traits::PathState) {
        let n = self.params.num_forwards;
        // Store each forward rate as indexed_spot(i)
        for (i, &fwd) in x.iter().enumerate().take(n) {
            state.set(state_keys::indexed_spot(i), fwd);
        }

        // Compute numeraire ratio P(t, T_N) from forwards:
        // P(t, T_N) / P(t, T_{first_alive}) = Π_{j=first_alive}^{N-1} 1/(1 + τ_j F_j)
        // We store the product of discount factors from first alive to terminal.
        let first = self.first_alive(state.time);
        let mut numeraire = 1.0;
        for (fwd, tau) in x[first..n]
            .iter()
            .zip(&self.params.accrual_factors[first..n])
        {
            numeraire /= 1.0 + tau * fwd;
        }
        state.set("lmm_numeraire", numeraire);

        // Also store the first alive index for downstream payoffs
        state.set("lmm_first_alive", first as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::PathState;

    fn simple_2f_params() -> LmmParams {
        // 3 forwards, 2 factors, flat 3% with 0.5% displacement
        LmmParams::try_new(
            3,
            2,
            vec![0.0, 1.0, 2.0, 3.0],
            vec![1.0, 1.0, 1.0],
            vec![0.005, 0.005, 0.005],
            vec![], // single vol period
            vec![vec![
                [0.15, 0.05, 0.0],
                [0.12, 0.08, 0.0],
                [0.10, 0.10, 0.0],
            ]],
            vec![0.03, 0.03, 0.03],
        )
        .expect("valid params")
    }

    #[test]
    fn test_params_validation() {
        let p = simple_2f_params();
        assert_eq!(p.num_forwards, 3);
        assert_eq!(p.num_factors, 2);
    }

    #[test]
    fn test_invalid_factor_count() {
        let result = LmmParams::try_new(
            3,
            5, // invalid
            vec![0.0, 1.0, 2.0, 3.0],
            vec![1.0, 1.0, 1.0],
            vec![0.005, 0.005, 0.005],
            vec![],
            vec![vec![[0.1, 0.0, 0.0]; 3]],
            vec![0.03, 0.03, 0.03],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_dim_and_factors() {
        let p = simple_2f_params();
        let process = LmmProcess::new(p);
        assert_eq!(process.dim(), 3);
        assert_eq!(process.num_factors(), 2);
    }

    #[test]
    fn test_drift_terminal_forward_zero() {
        // The terminal forward (N-1) has no drift correction under terminal measure
        let p = simple_2f_params();
        let process = LmmProcess::new(p);
        let x = vec![0.03, 0.03, 0.03];
        let mut drift = vec![0.0; 3];
        process.drift(0.0, &x, &mut drift);

        // Terminal forward (index 2) should have zero drift (no j > 2)
        assert!((drift[2]).abs() < 1e-15);

        // Non-terminal forwards should have non-zero drift
        assert!(drift[0].abs() > 1e-10);
        assert!(drift[1].abs() > 1e-10);
    }

    #[test]
    fn test_diffusion_matrix() {
        let p = simple_2f_params();
        let process = LmmProcess::new(p);
        let x = vec![0.03, 0.03, 0.03];
        let mut diff = vec![0.0; 6]; // 3 forwards × 2 factors
        process.diffusion(0.0, &x, &mut diff);

        // diff[i*2+k] = (F_i + d_i) * λ_{i,k}
        let shifted = 0.03 + 0.005; // = 0.035
        assert!((diff[0] - shifted * 0.15).abs() < 1e-12);
        assert!((diff[1] - shifted * 0.05).abs() < 1e-12);
    }

    #[test]
    fn test_populate_path_state() {
        let p = simple_2f_params();
        let process = LmmProcess::new(p);
        let x = vec![0.03, 0.035, 0.04];
        let mut state = PathState::new(0, 0.0);
        process.populate_path_state(&x, &mut state);

        assert!((state.get(state_keys::indexed_spot(0)).expect("set") - 0.03).abs() < 1e-12);
        assert!((state.get(state_keys::indexed_spot(1)).expect("set") - 0.035).abs() < 1e-12);
        assert!((state.get(state_keys::indexed_spot(2)).expect("set") - 0.04).abs() < 1e-12);

        // Numeraire should be product of discount factors
        let num = state.get("lmm_numeraire").expect("set");
        let expected = 1.0 / (1.03 * 1.035 * 1.04);
        assert!((num - expected).abs() < 1e-10);
    }

    #[test]
    fn test_first_alive() {
        let p = simple_2f_params();
        let process = LmmProcess::new(p);
        assert_eq!(process.first_alive(0.0), 0);
        assert_eq!(process.first_alive(0.5), 1); // T_0=0.0 < 0.5 → forward 0 dead
        assert_eq!(process.first_alive(1.0), 1); // T_0=0.0, T_1=1.0 → forward 0 dead, 1 just fixed
        assert_eq!(process.first_alive(1.5), 2);
    }

    #[test]
    fn test_vol_period_selection() {
        let p = LmmParams::try_new(
            2,
            2,
            vec![0.0, 1.0, 2.0],
            vec![1.0, 1.0],
            vec![0.005, 0.005],
            vec![0.5], // breakpoint at 0.5y
            vec![
                vec![[0.20, 0.05, 0.0], [0.18, 0.06, 0.0]], // period 0: [0, 0.5)
                vec![[0.15, 0.04, 0.0], [0.13, 0.05, 0.0]], // period 1: [0.5, ∞)
            ],
            vec![0.03, 0.03],
        )
        .expect("valid");
        assert_eq!(p.vol_period(0.25), 0);
        assert_eq!(p.vol_period(0.5), 1);
        assert_eq!(p.vol_period(0.75), 1);
    }
}
