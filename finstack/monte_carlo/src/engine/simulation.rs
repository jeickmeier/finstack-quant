//! Per-path simulation primitives used by the `McEngine` pricing loops.
//!
//! The three methods exposed here (`simulate_path`, `simulate_path_with_capture`,
//! and `simulate_antithetic_pair`) are pure per-path helpers. The outer loops
//! (serial / parallel, with or without capture) live in
//! [`super::pricing`] and call these primitives.

use super::pricing::McEngine;
use crate::paths::{PathPoint, SimulatedPath};
use crate::traits::{Discretization, PathState, Payoff, RandomStream, StochasticProcess};
use finstack_core::currency::Currency;
use finstack_core::math::linalg::CorrelationFactor;
use finstack_core::Result;
use smallvec::SmallVec;

/// Strategy for filling the per-step shock vector `z`.
///
/// - `Correlation` — draw i.i.d. shocks and (optionally) Cholesky-transform
///   them with `factor`. The standard McEngine path loop uses this.
/// - `InjectFbm` — draw i.i.d. shocks then overwrite `z[z_index]` with
///   `increments[step]`, and optionally a second slot `z[aux_index]` with
///   `aux[step]`. The rough-volatility / fractional Monte Carlo path loop uses
///   this to splice pre-generated fractional increments into one factor slot
///   (and, for rBergomi, the unit-variance driving Brownian normal into a
///   second slot). Correlation is always `None` in this mode because rough
///   schemes encode their factor structure inside `disc.step`.
pub(crate) enum NoiseHook<'a> {
    Correlation(Option<&'a CorrelationFactor>),
    InjectFbm {
        z_index: usize,
        increments: &'a [f64],
        /// Optional second injection: `(aux_index, aux_values)`. When present,
        /// `z[aux_index]` is overwritten with `aux_values[step]`. Used by the
        /// rBergomi path to inject the unit-variance driving Brownian normal
        /// alongside the Riemann-Liouville Volterra increment.
        aux: Option<(usize, &'a [f64])>,
    },
}

/// Fill `z` with the shocks chosen by `hook` for time step `step`.
#[inline]
fn fill_shocks<R: RandomStream>(
    rng: &mut R,
    z: &mut [f64],
    z_raw: &mut [f64],
    step: usize,
    hook: &NoiseHook<'_>,
) {
    match hook {
        NoiseHook::Correlation(Some(cf)) => {
            rng.fill_std_normals(z_raw);
            let _ = cf.apply(z_raw, z);
        }
        NoiseHook::Correlation(None) => rng.fill_std_normals(z),
        NoiseHook::InjectFbm {
            z_index,
            increments,
            aux,
        } => {
            rng.fill_std_normals(z);
            debug_assert!(
                step < increments.len(),
                "fBM increments shorter than time grid: step {step} >= len {}",
                increments.len()
            );
            debug_assert!(
                *z_index < z.len(),
                "fbm_z_index {z_index} out of bounds for z len {}",
                z.len()
            );
            z[*z_index] = increments[step];
            if let Some((aux_index, aux_values)) = aux {
                debug_assert!(
                    step < aux_values.len(),
                    "fBM aux values shorter than time grid: step {step} >= len {}",
                    aux_values.len()
                );
                debug_assert!(
                    *aux_index < z.len(),
                    "fbm aux_index {aux_index} out of bounds for z len {}",
                    z.len()
                );
                z[*aux_index] = aux_values[step];
            }
        }
    }
}

/// Shared per-path simulation loop used by [`McEngine::simulate_path`] and the
/// fractional-noise wrapper in [`crate::engine_fractional`].
///
/// Drives a single path through `time_grid`, generating shocks via `hook`,
/// stepping the discretization, and dispatching payoff events. Returns the
/// undiscounted payoff amount in `currency`.
///
/// The work buffer is zero-initialised before every path. This makes
/// path-history-dependent discretizations (e.g. [`crate::discretization::RoughHestonHybrid`])
/// work without fragile float comparisons such as `t < ε` to detect path
/// boundaries — the cost is one memset of `work_size()` doubles per path,
/// which is negligible relative to the path simulation itself.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_path_loop<R, P, D, F>(
    rng: &mut R,
    time_grid: &crate::time_grid::TimeGrid,
    process: &P,
    disc: &D,
    initial_state: &[f64],
    payoff: &mut F,
    state: &mut [f64],
    z: &mut [f64],
    z_raw: &mut [f64],
    work: &mut [f64],
    hook: NoiseHook<'_>,
    currency: Currency,
) -> Result<f64>
where
    R: RandomStream,
    P: StochasticProcess,
    D: Discretization<P>,
    F: Payoff,
{
    state.copy_from_slice(initial_state);

    // Always zero the work buffer at path start so discretizations that
    // accumulate path-history state (rough Heston Volterra integrand,
    // step counters, etc.) start every path from a known clean state without
    // resorting to fragile float-based path-boundary detection.
    work.fill(0.0);

    let mut path_state = PathState::new(0, 0.0);
    process.populate_path_state(state, &mut path_state);
    path_state.set_uniform_random(rng.next_u01());
    payoff.on_event(&mut path_state);

    for step in 0..time_grid.num_steps() {
        let t = time_grid.time(step);
        let dt = time_grid.dt(step);

        fill_shocks(rng, z, z_raw, step, &hook);
        disc.step(process, t, dt, state, z, work);

        path_state.set_step_time(step + 1, t + dt);
        process.populate_path_state(state, &mut path_state);
        path_state.set_uniform_random(rng.next_u01());
        payoff.on_event(&mut path_state);
    }

    Ok(payoff.value(currency).amount())
}

impl McEngine {
    /// Simulate a single Monte Carlo path.
    ///
    /// Visibility is `pub(crate)` so pricer modules (e.g. the Sobol loop in
    /// [`crate::pricer::path_dependent`]) can drive one path at a time with a
    /// custom RNG while still reusing the engine's canonical stepping logic.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn simulate_path<R, P, D, F>(
        &self,
        rng: &mut R,
        process: &P,
        disc: &D,
        initial_state: &[f64],
        payoff: &mut F,
        state: &mut [f64],
        z: &mut [f64],
        z_raw: &mut [f64],
        work: &mut [f64],
        correlation: Option<&CorrelationFactor>,
        currency: Currency,
    ) -> Result<f64>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P>,
        F: Payoff,
    {
        run_path_loop(
            rng,
            &self.config.time_grid,
            process,
            disc,
            initial_state,
            payoff,
            state,
            z,
            z_raw,
            work,
            NoiseHook::Correlation(correlation),
            currency,
        )
    }

    /// Simulate a single Monte Carlo path with full capture.
    ///
    /// Returns the payoff value and the captured path data. Visibility is
    /// `pub(crate)` for the same reason as [`Self::simulate_path`]: specialised
    /// pricer loops (e.g. Sobol path-dependent pricing) drive per-path RNGs
    /// themselves while reusing the engine's capture bookkeeping.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn simulate_path_with_capture<R, P, D, F>(
        &self,
        rng: &mut R,
        process: &P,
        disc: &D,
        initial_state: &[f64],
        payoff: &mut F,
        state: &mut [f64],
        z: &mut [f64],
        z_raw: &mut [f64],
        work: &mut [f64],
        correlation: Option<&CorrelationFactor>,
        path_id: usize,
        discount_factor: f64,
        currency: Currency,
    ) -> Result<(f64, SimulatedPath)>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P>,
        F: Payoff,
    {
        // Initialize state
        state.copy_from_slice(initial_state);

        // Create initial path state for payoff
        let mut path_state = PathState::new(0, 0.0);
        process.populate_path_state(state, &mut path_state);
        path_state.set_uniform_random(rng.next_u01());
        payoff.on_event(&mut path_state);

        // Initialize simulated path after the initial event so step-0 payoff and
        // cashflow state are captured consistently.
        let num_steps = self.config.time_grid.num_steps() + 1; // +1 for initial point
        let mut simulated_path = SimulatedPath::with_capacity(path_id, num_steps);
        let initial_state_vec = SmallVec::from_slice(state);
        let mut initial_point = PathPoint::with_state(0, 0.0, initial_state_vec);
        path_state.drain_cashflows(|time, amount, cf_type| {
            initial_point.add_typed_cashflow(time, amount, cf_type);
        });
        if self.config.path_capture.capture_payoffs {
            let payoff_money = payoff.value(currency);
            initial_point.set_payoff(payoff_money.amount());
        }
        simulated_path.add_point(initial_point);

        // Simulate path through time steps
        let hook = NoiseHook::Correlation(correlation);
        for step in 0..self.config.time_grid.num_steps() {
            let t = self.config.time_grid.time(step);
            let dt = self.config.time_grid.dt(step);

            fill_shocks(rng, z, z_raw, step, &hook);
            disc.step(process, t, dt, state, z, work);

            path_state.set_step_time(step + 1, t + dt);
            process.populate_path_state(state, &mut path_state);
            path_state.set_uniform_random(rng.next_u01());

            // Process payoff event (payoff may add cashflows to path_state)
            payoff.on_event(&mut path_state);

            // Capture this point with state vector
            let state_vec = SmallVec::from_slice(state);
            let mut point = PathPoint::with_state(step + 1, t + dt, state_vec);

            // Transfer cashflows from PathState to PathPoint
            path_state.drain_cashflows(|time, amount, cf_type| {
                point.add_typed_cashflow(time, amount, cf_type);
            });

            if self.config.path_capture.capture_payoffs {
                // Capture intermediate payoff value (undiscounted)
                let payoff_money = payoff.value(currency);
                point.set_payoff(payoff_money.amount());
            }
            simulated_path.add_point(point);
        }

        // Extract final payoff value
        let payoff_money = payoff.value(currency);
        let payoff_value = payoff_money.amount();

        // Set final discounted value
        simulated_path.set_final_value(payoff_value * discount_factor);

        // Calculate IRR from cashflows (if available)
        let cashflow_amounts = simulated_path.extract_cashflow_amounts();
        if cashflow_amounts.len() >= 2 {
            // Use periodic IRR approximation (assumes roughly equal spacing)
            if let Ok(rate) = finstack_core::cashflow::irr(&cashflow_amounts, None) {
                simulated_path.set_irr(rate);
            }
        }

        Ok((payoff_value, simulated_path))
    }

    /// Simulate one antithetic pair and return the average payoff (in amount).
    ///
    /// Uses separate work buffers for primary and antithetic paths to prevent
    /// state corruption in discretizations with stateful work buffers (e.g.,
    /// rough Heston, rBergomi, Cheyette rough-vol).
    ///
    /// `payoff_p` and `payoff_a` are caller-owned per-pair scratch payoffs
    /// that this method `reset()`s in place — the caller hoists the two
    /// clones outside the per-path loop so we don't allocate-and-discard a
    /// fresh payoff on every pair.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn simulate_antithetic_pair<R, P, D, F>(
        &self,
        rng: &mut R,
        process: &P,
        disc: &D,
        initial_state: &[f64],
        payoff_p: &mut F,
        payoff_a: &mut F,
        state_p: &mut [f64],
        state_a: &mut [f64],
        z: &mut [f64],
        z_anti: &mut [f64],
        z_raw: &mut [f64],
        work: &mut [f64],
        work_anti: &mut [f64],
        correlation: Option<&CorrelationFactor>,
        currency: Currency,
    ) -> Result<f64>
    where
        R: RandomStream,
        P: StochasticProcess,
        D: Discretization<P>,
        F: Payoff,
    {
        // Both payoffs are reset by the caller (so on_path_start has already
        // run and any per-path RNG draws are stable across the pair).
        // Primary path
        state_p.copy_from_slice(initial_state);
        let mut path_state_p = PathState::new(0, 0.0);
        process.populate_path_state(state_p, &mut path_state_p);
        let u_init = rng.next_u01();
        path_state_p.set_uniform_random(u_init);
        payoff_p.on_event(&mut path_state_p);

        // Antithetic path
        state_a.copy_from_slice(initial_state);
        let mut path_state_a = PathState::new(0, 0.0);
        process.populate_path_state(state_a, &mut path_state_a);
        path_state_a.set_uniform_random(1.0 - u_init);
        payoff_a.on_event(&mut path_state_a);

        let hook = NoiseHook::Correlation(correlation);
        for step in 0..self.config.time_grid.num_steps() {
            let t = self.config.time_grid.time(step);
            let dt = self.config.time_grid.dt(step);

            fill_shocks(rng, z, z_raw, step, &hook);
            for i in 0..z.len() {
                z_anti[i] = -z[i];
            }

            disc.step(process, t, dt, state_p, z, work);
            disc.step(process, t, dt, state_a, z_anti, work_anti);

            let u_step = rng.next_u01();

            path_state_p.set_step_time(step + 1, t + dt);
            process.populate_path_state(state_p, &mut path_state_p);
            path_state_p.set_uniform_random(u_step);
            payoff_p.on_event(&mut path_state_p);

            path_state_a.set_step_time(step + 1, t + dt);
            process.populate_path_state(state_a, &mut path_state_a);
            path_state_a.set_uniform_random(1.0 - u_step);
            payoff_a.on_event(&mut path_state_a);
        }

        let v_p = payoff_p.value(currency).amount();
        let v_a = payoff_a.value(currency).amount();
        Ok(0.5 * (v_p + v_a))
    }
}
