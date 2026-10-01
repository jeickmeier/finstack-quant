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
//!   Ch. 7, Springer. `docs/REFERENCES.md#glasserman-2004-monte-carlo`
//! - Rebonato (2002). *Modern Pricing of Interest-Rate Derivatives*,
//!   Ch. 8-9, Princeton University Press. `docs/REFERENCES.md#rebonato-2004-volatility-correlation`
//! - Andersen & Piterbarg (2010). *Interest Rate Modeling*, Vol. 2,
//!   Ch. 15-16, Atlantic Financial Press. `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`

#[cfg(test)]
use super::super::traits::state_keys;
use super::super::traits::StochasticProcess;

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
    /// Validate LMM parameters assembled as a single cohesive configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if dimensions are inconsistent, factor count is out of
    /// range, or any input is non-finite.
    #[must_use = "validation returns the validated parameter set"]
    pub fn validate(self) -> finstack_quant_core::Result<Self> {
        let Self {
            num_forwards,
            num_factors,
            tenors,
            accrual_factors,
            displacements,
            vol_times,
            vol_values,
            initial_forwards,
        } = self;
        if num_forwards == 0 {
            return Err(finstack_quant_core::Error::Validation(
                "LMM requires at least one forward rate".to_string(),
            ));
        }
        if !(2..=MAX_FACTORS).contains(&num_factors) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "LMM supports 2-{MAX_FACTORS} factors, got {num_factors}"
            )));
        }
        if tenors.len() != num_forwards + 1 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "tenors length must be num_forwards+1 ({}), got {}",
                num_forwards + 1,
                tenors.len()
            )));
        }
        if accrual_factors.len() != num_forwards {
            return Err(finstack_quant_core::Error::Validation(format!(
                "accrual_factors length must be {num_forwards}, got {}",
                accrual_factors.len()
            )));
        }
        if displacements.len() != num_forwards {
            return Err(finstack_quant_core::Error::Validation(format!(
                "displacements length must be {num_forwards}, got {}",
                displacements.len()
            )));
        }
        if initial_forwards.len() != num_forwards {
            return Err(finstack_quant_core::Error::Validation(format!(
                "initial_forwards length must be {num_forwards}, got {}",
                initial_forwards.len()
            )));
        }
        let num_periods = vol_times.len() + 1;
        if vol_values.len() != num_periods {
            return Err(finstack_quant_core::Error::Validation(format!(
                "vol_values length must be {} (vol_times.len()+1), got {}",
                num_periods,
                vol_values.len()
            )));
        }
        for (p, period) in vol_values.iter().enumerate() {
            if period.len() != num_forwards {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "vol_values[{p}] length must be {num_forwards}, got {}",
                    period.len()
                )));
            }
        }
        // Validate finite values
        for (i, t) in tenors.iter().enumerate() {
            if !t.is_finite() || *t < 0.0 || (i > 0 && *t <= tenors[i - 1]) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "tenors must be finite and strictly increasing, issue at index {i}"
                )));
            }
        }
        for (i, tau) in accrual_factors.iter().enumerate() {
            if !tau.is_finite() || *tau <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "accrual_factors[{i}] must be positive and finite, got {tau}"
                )));
            }
        }
        for (i, &time) in vol_times.iter().enumerate() {
            if !time.is_finite() || time < 0.0 || (i > 0 && time <= vol_times[i - 1]) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "LMM volatility knots must be finite, non-negative and strictly increasing, issue at index {i}"
                )));
            }
        }
        for i in 0..num_forwards {
            let forward = initial_forwards[i];
            let displacement = displacements[i];
            if !forward.is_finite()
                || !displacement.is_finite()
                || !((forward + displacement).is_finite() && forward + displacement > 0.0)
                || !((1.0 + accrual_factors[i] * forward).is_finite()
                    && 1.0 + accrual_factors[i] * forward > 0.0)
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "LMM forward {i} must have finite inputs, positive displaced level and positive accrual discount denominator"
                )));
            }
        }
        if vol_values
            .iter()
            .flatten()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(finstack_quant_core::Error::Validation(
                "LMM factor loadings must be finite".to_string(),
            ));
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
/// `"lmm_numeraire"`, the product `Π_{j ≥ first_period} 1/(1 + τ_j F_j)` —
/// i.e. `P(t, T_N) / P(t, T_{first_period})`, **not** `P(t, T_N)` itself:
/// the stub discount over `[t, T_{first_period})` is omitted (and nothing
/// divides by `P(0, T_N)`). Consumers must either evaluate only at tenor
/// dates where the stub is empty, or — like the Bermudan LSMC pricer —
/// form ratios in which the common `T_{first_period}` reference cancels.
/// A forward fixing exactly at `t` enters this discount product but is frozen
/// for subsequent evolution. Simulation grids must include all fixing dates
/// and volatility knots within the simulation horizon.
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
    /// Forward `i` is alive for evolution only if `T_i > t`.
    #[inline]
    pub(crate) fn first_alive(&self, t: f64) -> usize {
        self.params.tenors[..self.params.num_forwards].partition_point(|&v| v <= t)
    }

    /// First accrual period in the terminal-numeraire ratio. Unlike evolution,
    /// the product includes the forward that has just fixed at this instant.
    fn first_numeraire_period(&self, t: f64) -> usize {
        self.params.tenors[..self.params.num_forwards].partition_point(|&v| v < t)
    }

    /// Whether coefficients and evolution eligibility are constant inside a
    /// step. Only roundoff in reconstructing the step's end from `t+dt` is
    /// allowed; the start must use the exact coefficient regime at `t`.
    pub(crate) fn step_is_aligned(&self, t: f64, dt: f64) -> bool {
        let end = t + dt;
        let tolerance = 32.0 * f64::EPSILON * end.abs().max(t.abs()).max(1.0);
        [
            &self.params.vol_times[..],
            &self.params.tenors[..self.params.num_forwards],
        ]
        .into_iter()
        .all(|knots| {
            let index = knots.partition_point(|&knot| knot <= t);
            knots.get(index).is_none_or(|&knot| knot >= end - tolerance)
        })
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
    fn validate_time_grid(
        &self,
        time_grid: &crate::monte_carlo::TimeGrid,
    ) -> finstack_quant_core::Result<()> {
        self.params.clone().validate()?;
        // Require exact nodes: accepting a near-knot start while selecting
        // loadings at the unadjusted time would use the previous volatility
        // regime for the entire following interval.
        let horizon = time_grid.t_max();
        for &knot in self
            .params
            .vol_times
            .iter()
            .chain(&self.params.tenors[..self.params.num_forwards])
        {
            if knot > 0.0
                && knot < horizon
                && time_grid
                    .times()
                    .binary_search_by(|time| time.total_cmp(&knot))
                    .is_err()
            {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "LMM simulation grid must contain every fixing date and volatility knot exactly; missing model time {knot}"
                )));
            }
        }
        Ok(())
    }

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
            state.set_indexed_spot(i, fwd);
        }

        // Compute numeraire ratio P(t, T_N) from forwards:
        // P(t, T_N) / P(t, T_{first_alive}) = Π_{j=first_alive}^{N-1} 1/(1 + τ_j F_j)
        // We store the product of discount factors from first alive to terminal.
        let first = self.first_numeraire_period(state.time);
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

    /// Only [`LmmPredictorCorrector`](crate::monte_carlo::discretization::lmm_predictor_corrector::LmmPredictorCorrector)
    /// evolves the n×K displaced-diffusion system under the terminal measure.
    /// A generic scheme such as Euler-Maruyama type-checks but treats diffusion
    /// as an n-vector, so the pairing either panics on the n×K slice or
    /// silently simulates the wrong SDE.
    fn dedicated_scheme(&self) -> Option<&'static str> {
        Some("lmm_predictor_corrector")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monte_carlo::traits::PathState;

    fn simple_2f_params() -> LmmParams {
        // 3 forwards, 2 factors, flat 3% with 0.5% displacement
        LmmParams {
            num_forwards: 3,
            num_factors: 2,
            tenors: vec![0.0, 1.0, 2.0, 3.0],
            accrual_factors: vec![1.0, 1.0, 1.0],
            displacements: vec![0.005, 0.005, 0.005],
            vol_times: vec![], // single vol period
            vol_values: vec![vec![
                [0.15, 0.05, 0.0],
                [0.12, 0.08, 0.0],
                [0.10, 0.10, 0.0],
            ]],
            initial_forwards: vec![0.03, 0.03, 0.03],
        }
        .validate()
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
        let result = LmmParams {
            num_forwards: 3,
            num_factors: 5, // invalid
            tenors: vec![0.0, 1.0, 2.0, 3.0],
            accrual_factors: vec![1.0, 1.0, 1.0],
            displacements: vec![0.005, 0.005, 0.005],
            vol_times: vec![],
            vol_values: vec![vec![[0.1, 0.0, 0.0]; 3]],
            initial_forwards: vec![0.03, 0.03, 0.03],
        }
        .validate();
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

        // Forward 0 has fixed; the remaining non-terminal forward has drift.
        assert_eq!(drift[0], 0.0);
        assert!(drift[1].abs() > 1e-10);
    }

    #[test]
    fn test_diffusion_matrix() {
        let p = simple_2f_params();
        let process = LmmProcess::new(p);
        let x = vec![0.03, 0.03, 0.03];
        let mut diff = vec![0.0; 6]; // 3 forwards × 2 factors
        process.diffusion(0.0, &x, &mut diff);

        // The forward fixing at t=0 is frozen; later forwards diffuse.
        let shifted = 0.03 + 0.005; // = 0.035
        assert_eq!(diff[0], 0.0);
        assert_eq!(diff[1], 0.0);
        assert!((diff[2] - shifted * 0.12).abs() < 1e-12);
        assert!((diff[3] - shifted * 0.08).abs() < 1e-12);
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
        assert_eq!(process.first_alive(0.0), 1);
        assert_eq!(process.first_alive(0.5), 1); // T_0=0.0 < 0.5 → forward 0 dead
        assert_eq!(process.first_alive(1.0), 2); // Both forwards 0 and 1 have fixed.
        assert_eq!(process.first_alive(1.5), 2);
    }

    #[test]
    fn grid_must_include_fixing_and_volatility_knots() {
        let mut params = simple_2f_params();
        params.vol_times = vec![0.4];
        params.vol_values.push(params.vol_values[0].clone());
        let process = LmmProcess::new(params);
        let aligned = crate::monte_carlo::TimeGrid::from_times(vec![0.0, 0.4, 1.0, 2.0, 3.0])
            .expect("valid grid");
        assert!(process.validate_time_grid(&aligned).is_ok());
        for times in [vec![0.0, 1.0, 2.0, 3.0], vec![0.0, 0.4, 2.0, 3.0]] {
            let grid = crate::monte_carlo::TimeGrid::from_times(times).expect("valid grid");
            assert!(process.validate_time_grid(&grid).is_err());
        }
    }

    #[test]
    fn near_volatility_knot_cannot_select_stale_coefficients_for_the_next_step() {
        let mut params = simple_2f_params();
        params.vol_times = vec![0.3];
        params.vol_values = vec![vec![[0.0; 3]; 3], vec![[0.2, 0.0, 0.0]; 3]];
        let process = LmmProcess::new(params);
        for offset in [-1e-15, 1e-15] {
            let near = 0.3 + offset;
            let grid =
                crate::monte_carlo::TimeGrid::from_times(vec![0.0, near, 1.0]).expect("valid grid");
            assert!(process.validate_time_grid(&grid).is_err());
        }
        assert!(!process.step_is_aligned(0.3 - 1e-15, 0.7));
        let exact =
            crate::monte_carlo::TimeGrid::from_times(vec![0.0, 0.3, 1.0]).expect("valid grid");
        assert!(process.validate_time_grid(&exact).is_ok());
        assert_eq!(process.params.factor_loadings(1, 0.3)[0], 0.2);
    }

    #[test]
    fn validate_rejects_invalid_lmm_domains() {
        let mut params = simple_2f_params();
        params.initial_forwards[0] = -0.01;
        assert!(params.validate().is_err());
        let mut params = simple_2f_params();
        params.vol_values[0][0][0] = f64::NAN;
        assert!(params.validate().is_err());
        let mut params = simple_2f_params();
        params.displacements[0] = f64::INFINITY;
        assert!(params.validate().is_err());
        let mut params = simple_2f_params();
        params.vol_times = vec![f64::NAN];
        params.vol_values.push(params.vol_values[0].clone());
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_vol_period_selection() {
        let p = LmmParams {
            num_forwards: 2,
            num_factors: 2,
            tenors: vec![0.0, 1.0, 2.0],
            accrual_factors: vec![1.0, 1.0],
            displacements: vec![0.005, 0.005],
            vol_times: vec![0.5], // breakpoint at 0.5y
            vol_values: vec![
                vec![[0.20, 0.05, 0.0], [0.18, 0.06, 0.0]], // period 0: [0, 0.5)
                vec![[0.15, 0.04, 0.0], [0.13, 0.05, 0.0]], // period 1: [0.5, ∞)
            ],
            initial_forwards: vec![0.03, 0.03],
        }
        .validate()
        .expect("valid");
        assert_eq!(p.vol_period(0.25), 0);
        assert_eq!(p.vol_period(0.5), 1);
        assert_eq!(p.vol_period(0.75), 1);
    }
}
