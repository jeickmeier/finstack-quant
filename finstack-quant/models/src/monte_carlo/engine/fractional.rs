//! Fractional noise integration for rough volatility Monte Carlo simulation.
//!
//! Runs one Monte Carlo path with pre-generated fractional Brownian motion
//! increments, for rough-volatility schemes such as rBergomi.
//!
//! The standard [`crate::monte_carlo::engine::McEngine`] simulation loop draws i.i.d.
//! normals at each time step. Rough volatility models instead need
//! correlated noise across *all* steps, generated upfront via an
//! implementor of [`crate::monte_carlo::rng::fbm::FractionalNoiseGenerator`]. The
//! function in this module wraps the per-path simulation loop to handle
//! that pre-generation and injection.
//!
//! # Usage
//!
//! Per path, the caller pre-generates the fractional increments and then hands
//! them to [`simulate_path_fractional`], which injects them at `fbm_z_index`:
//!
//! ```text
//! rng.fill_std_normals(&mut fbm_normals);
//! fbm_gen.generate(&fbm_normals, &mut fbm_increments);
//!
//! let path = FractionalPath {
//!     time_grid, process, disc, initial_state, currency,
//!     fbm_increments: &fbm_increments, fbm_z_index, aux: None,
//! };
//! let pv = simulate_path_fractional(&mut path_rng, &path, &mut payoff, &mut state, &mut z, &mut work)?;
//! ```

use crate::monte_carlo::engine::simulation::{PathBuffers, PathLoop};
use crate::monte_carlo::engine::{McEngine, NoiseHook, run_path_loop};
#[cfg(test)]
use crate::monte_carlo::traits::PathState;
use crate::monte_carlo::traits::{Discretization, Payoff, RandomStream, StochasticProcess};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::math::time_grid::TimeGrid;
use finstack_quant_core::{Error, Result};

/// Model, grid and pre-generated fractional noise of one
/// [`simulate_path_fractional`] path.
pub struct FractionalPath<'a, P, D> {
    /// Simulation time grid.
    pub time_grid: &'a TimeGrid,
    /// Stochastic process driving the state.
    pub process: &'a P,
    /// Discretization scheme compatible with `process`.
    pub disc: &'a D,
    /// Initial state vector; its length must equal `process.dim()`.
    pub initial_state: &'a [f64],
    /// Currency tag assigned to the resulting payoff amount.
    pub currency: Currency,
    /// Pre-generated fractional increments, at least `time_grid.num_steps()`
    /// long; `fbm_increments[step]` is injected at every step.
    pub fbm_increments: &'a [f64],
    /// Index in `z` where the fractional increment is injected.
    pub fbm_z_index: usize,
    /// Optional second injection `(z_index, values)`; rBergomi uses it to
    /// inject the unit-variance driving Brownian normal for the exact
    /// spot–vol correlation. `None` for schemes that need only the single
    /// increment.
    pub aux: Option<(usize, &'a [f64])>,
}

/// Simulate a single Monte Carlo path with fractional noise injection.
///
/// Thin wrapper over the shared per-path loop with a noise injection hook
/// that overwrites `z[path.fbm_z_index]` with `path.fbm_increments[step]`
/// after drawing i.i.d. shocks. Correlation is not applied; rough-volatility
/// schemes encode their factor structure inside `disc.step`.
///
/// # Arguments
///
/// * `rng` — random stream for standard normals and uniform draws
/// * `path` — grid, process, scheme, initial state, currency and the
///   fractional noise to inject; see [`FractionalPath`]
/// * `payoff` — payoff accumulator (must be pre-reset by caller)
/// * `state` — reusable state buffer (length = `process.dim()`)
/// * `z` — reusable noise buffer (length = `process.num_factors()`)
/// * `work` — reusable work buffer (length = `disc.work_size(process)`); the
///   loop zeros it at the start of every path so rough-vol discretizations
///   don't need to detect path boundaries via fragile float comparisons.
///
/// # Errors
///
/// Returns an error if the scheme pairing, grid step count, buffer dimensions,
/// or payoff value computation fails.
pub fn simulate_path_fractional<R, P, D, F>(
    rng: &mut R,
    path: &FractionalPath<'_, P, D>,
    payoff: &mut F,
    state: &mut [f64],
    z: &mut [f64],
    work: &mut [f64],
) -> Result<f64>
where
    R: RandomStream,
    P: StochasticProcess,
    D: Discretization<P>,
    F: Payoff,
{
    let &FractionalPath {
        time_grid,
        process,
        disc,
        initial_state,
        currency,
        fbm_increments,
        fbm_z_index,
        aux,
    } = path;
    McEngine::validate_scheme_pairing(process, disc)?;
    let num_steps = time_grid.num_steps();
    if let Some(expected) = disc.expected_num_steps()
        && expected != num_steps
    {
        return Err(Error::Validation(format!(
            "discretization '{}' was precomputed for {expected} steps but time_grid has {num_steps} steps",
            disc.scheme_id()
        )));
    }
    let dim = process.dim();
    if initial_state.len() != dim || state.len() != dim {
        return Err(Error::Validation(format!(
            "state buffer lengths (initial_state={}, state={}) must equal process.dim() ({dim})",
            initial_state.len(),
            state.len()
        )));
    }
    let num_factors = process.num_factors();
    if z.len() < num_factors || fbm_z_index >= z.len() {
        return Err(Error::Validation(format!(
            "z buffer (len={}) must have at least {num_factors} factors and contain fbm_z_index={fbm_z_index}",
            z.len()
        )));
    }
    if fbm_increments.len() < num_steps {
        return Err(Error::Validation(format!(
            "fbm_increments length ({}) must be at least time_grid.num_steps() ({num_steps})",
            fbm_increments.len()
        )));
    }
    if let Some((aux_index, aux_values)) = aux
        && (aux_index >= z.len() || aux_values.len() < num_steps)
    {
        return Err(Error::Validation(format!(
            "aux buffer (index={aux_index}, len={}) out of bounds for z_len={} or num_steps={num_steps}",
            aux_values.len(),
            z.len()
        )));
    }
    let req_work = disc.work_size(process);
    if work.len() < req_work {
        return Err(Error::Validation(format!(
            "work buffer length ({}) must be at least disc.work_size() ({req_work})",
            work.len()
        )));
    }

    // No engine-side correlation: rough-vol schemes apply factor structure inside `disc.step`.
    // `z_raw` is unused in the InjectFbm path, so a zero-length slice suffices.
    let mut z_raw: [f64; 0] = [];
    let path_loop = PathLoop {
        time_grid,
        process,
        disc,
        initial_state,
        hook: NoiseHook::InjectFbm {
            z_index: fbm_z_index,
            increments: fbm_increments,
            aux,
        },
        currency,
    };
    let buffers = PathBuffers {
        state,
        z,
        z_raw: &mut z_raw,
        work,
    };
    run_path_loop(rng, &path_loop, payoff, buffers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monte_carlo::rng::fbm::{CholeskyFbm, FractionalNoiseGenerator};
    use crate::monte_carlo::rng::philox::PhiloxRng;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    #[derive(Debug, Clone)]
    struct TwoFactorSpot;

    impl StochasticProcess for TwoFactorSpot {
        fn dim(&self) -> usize {
            1
        }

        fn num_factors(&self) -> usize {
            2 // z[0] = spot noise, z[1] = fBM slot
        }

        fn drift(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
            out[0] = 0.0;
        }

        fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
            out[0] = 1.0;
            if out.len() > 1 {
                out[1] = 0.0;
            }
        }

        fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
            state.set(crate::monte_carlo::traits::state_keys::SPOT, x[0]);
        }
    }

    #[derive(Debug, Clone)]
    struct RecordFbmDisc;

    impl Discretization<TwoFactorSpot> for RecordFbmDisc {
        fn step(
            &self,
            _process: &TwoFactorSpot,
            _t: f64,
            _dt: f64,
            x: &mut [f64],
            z: &[f64],
            _work: &mut [f64],
        ) {
            // Accumulate z[1] (the fBM slot) into x[0]
            x[0] += z[1];
        }

        fn work_size(&self, _process: &TwoFactorSpot) -> usize {
            0
        }
    }

    #[derive(Debug, Clone, Default)]
    struct TerminalSpotPayoff {
        terminal: f64,
    }

    impl Payoff for TerminalSpotPayoff {
        fn on_event(&mut self, state: &mut PathState) -> finstack_quant_core::Result<()> {
            if let Some(s) = state.spot() {
                self.terminal = s;
            }
            Ok(())
        }

        fn value(&self, currency: Currency) -> finstack_quant_core::Result<Money> {
            Ok(Money::new(self.terminal, currency).expect("valid money fixture"))
        }

        fn reset(&mut self) {
            self.terminal = 0.0;
        }
    }

    #[test]
    fn test_simulate_path_fractional_injects_fbm() {
        let process = TwoFactorSpot;
        let disc = RecordFbmDisc;
        let time_grid = TimeGrid::uniform(1.0, 4).expect("valid grid");
        let mut rng = PhiloxRng::new(42);

        let fbm_increments = vec![0.1, 0.2, 0.3, 0.4];
        let mut payoff = TerminalSpotPayoff::default();
        let mut state = vec![0.0];
        let mut z = vec![0.0, 0.0];
        let mut work = vec![];

        let path = FractionalPath {
            time_grid: &time_grid,
            process: &process,
            disc: &disc,
            initial_state: &[0.0],
            currency: Currency::USD,
            fbm_increments: &fbm_increments,
            fbm_z_index: 1,
            aux: None,
        };
        let pv =
            simulate_path_fractional(&mut rng, &path, &mut payoff, &mut state, &mut z, &mut work)
                .expect("simulation should succeed");

        // The disc accumulates z[1] into x[0], so final = sum of fBM increments
        let expected: f64 = fbm_increments.iter().sum();
        assert!(
            (pv - expected).abs() < 1e-12,
            "Expected {expected}, got {pv}"
        );
    }

    #[test]
    fn test_simulate_with_fbm_generator_round_trip() {
        let times = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        let hurst = 0.1;
        let fbm_gen = CholeskyFbm::new(&times, hurst).expect("valid fBM generator");
        let num_steps = fbm_gen.num_steps();

        let process = TwoFactorSpot;
        let disc = RecordFbmDisc;
        let time_grid = TimeGrid::uniform(1.0, 4).expect("valid grid");
        let mut rng = PhiloxRng::new(99);

        let mut payoff = TerminalSpotPayoff::default();
        let mut state = vec![0.0];
        let mut z = vec![0.0, 0.0];
        let mut work = vec![];
        let mut fbm_normals = vec![0.0; num_steps];
        let mut fbm_increments = vec![0.0; num_steps];

        // Generate fBM increments then simulate — the pattern callers use
        rng.fill_std_normals(&mut fbm_normals);
        fbm_gen.generate(&fbm_normals, &mut fbm_increments);

        let path = FractionalPath {
            time_grid: &time_grid,
            process: &process,
            disc: &disc,
            initial_state: &[0.0],
            currency: Currency::USD,
            fbm_increments: &fbm_increments,
            fbm_z_index: 1,
            aux: None,
        };
        let pv =
            simulate_path_fractional(&mut rng, &path, &mut payoff, &mut state, &mut z, &mut work)
                .expect("simulation should succeed");

        // pv should be the sum of correlated fBM increments (finite, non-NaN)
        assert!(pv.is_finite(), "payoff must be finite, got {pv}");
    }
}
