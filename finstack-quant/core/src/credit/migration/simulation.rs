//! CTMC path simulation via Gillespie's competing exponentials algorithm.
//!
//! Simulates individual obligor rating trajectories as continuous-time Markov
//! chains and supports batch simulation with empirical transition matrix
//! estimation.
//!
//! # Algorithm
//!
//! Gillespie's algorithm (competing exponentials):
//!
//! 1. Let λ = −q_ss (exit rate from current state s).
//! 2. If λ ≈ 0 (absorbing state): record and stop.
//! 3. Draw holding time τ ~ Exp(λ), i.e. τ = −ln(U₁) / λ.
//! 4. If t + τ > T: terminate.
//! 5. Draw next state j from Categorical(q_sj / λ) using U₂.
//! 6. Record transition (t + τ, j); advance t.
//!
//! # References
//!
//! - Gillespie, D. T. (1977). "Exact Stochastic Simulation of Coupled
//!   Chemical Reactions." *Journal of Physical Chemistry*, 81(25), 2340-2361.
//! - Lando, D., & Skodeberg, T. M. (2002). "Analyzing Rating Transitions and
//!   Rating Drift with Continuous Observations." *Journal of Banking & Finance*,
//!   26(2-3), 423-444.

use std::sync::Arc;

use rand::Rng;
use serde::{Deserialize, Serialize};

use super::{
    error::MigrationError, generator::GeneratorMatrix, matrix::TransitionMatrix, scale::RatingScale,
};

// ---------------------------------------------------------------------------
// RatingPath
// ---------------------------------------------------------------------------

/// A simulated rating trajectory: sequence of (time, state_index) pairs.
///
/// The path is piecewise-constant and right-continuous: at any time `t`, the
/// state is the most recent transition that occurred at or before `t`.
///
/// The first entry always records the initial state at time 0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingPath {
    /// Transition events as (time, state_index) pairs, starting with (0.0, s₀).
    transitions: Vec<(f64, usize)>,
    /// Total simulation horizon.
    horizon: f64,
    /// Rating scale used for the simulation. Shared via `Arc` so the
    /// hot batch-simulation loops bump a refcount per path instead of
    /// re-allocating the labels `Vec` and rebuilding the index map. The serde
    /// "rc" feature keeps the wire format identical to an inline `RatingScale`.
    scale: Arc<RatingScale>,
}

impl RatingPath {
    /// State index at time t (piecewise-constant, right-continuous at jumps).
    ///
    /// Returns the initial state if `t < 0` and the terminal state if `t > horizon`.
    #[must_use]
    pub fn state_at(&self, t: f64) -> usize {
        // Find the last transition at or before t.
        let mut state = self.transitions[0].1;
        for &(time, s) in &self.transitions {
            if time <= t {
                state = s;
            } else {
                break;
            }
        }
        state
    }

    /// State label at time t.
    ///
    /// Returns `"UNKNOWN"` if the index is out of range (should never happen
    /// for well-formed paths).
    #[must_use]
    pub fn label_at(&self, t: f64) -> &str {
        let idx = self.state_at(t);
        self.scale.label_of(idx).unwrap_or("UNKNOWN")
    }

    /// Whether the obligor defaulted during the simulation horizon.
    #[must_use]
    pub fn defaulted(&self) -> bool {
        self.default_time().is_some()
    }

    /// Time of default, if the obligor reached the default state.
    ///
    /// Returns `None` if no default occurred or if no default state is defined.
    #[must_use]
    pub fn default_time(&self) -> Option<f64> {
        let d = self.scale.default_state()?;
        self.transitions
            .iter()
            .find(|&&(_, s)| s == d)
            .map(|&(t, _)| t)
    }

    /// Number of transitions (excluding the initial state recording at t=0).
    #[must_use]
    pub fn n_transitions(&self) -> usize {
        self.transitions.len().saturating_sub(1)
    }

    /// All transition events as (time, state_index) pairs.
    ///
    /// The first element is always `(0.0, initial_state)`.
    #[must_use]
    pub fn transitions(&self) -> &[(f64, usize)] {
        &self.transitions
    }

    /// The rating scale associated with this path.
    #[must_use]
    pub fn scale(&self) -> &RatingScale {
        self.scale.as_ref()
    }

    /// The simulation horizon.
    #[must_use]
    pub fn horizon(&self) -> f64 {
        self.horizon
    }
}

// ---------------------------------------------------------------------------
// MigrationSimulator
// ---------------------------------------------------------------------------

/// Simulator for generating rating paths from a generator matrix.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::credit::migration::{RatingScale, GeneratorMatrix, simulation::MigrationSimulator};
/// use rand::SeedableRng;
/// use rand_pcg::Pcg64;
///
/// let scale = RatingScale::custom(vec!["AAA".to_string(), "D".to_string()])
///     .expect("valid scale");
/// let gen = GeneratorMatrix::new(scale, &[-0.1, 0.1, 0.0, 0.0])
///     .expect("valid generator");
/// let sim = MigrationSimulator::new(gen, 5.0).expect("valid simulator");
///
/// let mut rng = Pcg64::seed_from_u64(42);
/// let paths = sim.simulate(0, 1000, &mut rng).expect("valid simulation inputs");
/// assert_eq!(paths.len(), 1000);
/// ```
#[derive(Debug, Clone)]
pub struct MigrationSimulator {
    /// The generator matrix.
    generator: GeneratorMatrix,
    /// Simulation horizon in years.
    horizon: f64,
}

impl MigrationSimulator {
    /// Create a new simulator.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationError::InvalidHorizon`] if `horizon <= 0`.
    pub fn new(generator: GeneratorMatrix, horizon: f64) -> Result<Self, MigrationError> {
        if !horizon.is_finite() || horizon <= 0.0 {
            return Err(MigrationError::InvalidHorizon(horizon));
        }
        Ok(Self { generator, horizon })
    }

    /// Simulate `n_paths` independent rating paths from `initial_state`.
    ///
    /// # Arguments
    ///
    /// * `initial_state` — Starting state index.
    /// * `n_paths` — Number of paths to generate.
    /// * `rng` — Any `rand::Rng` source.
    pub fn simulate<R: Rng>(
        &self,
        initial_state: usize,
        n_paths: usize,
        rng: &mut R,
    ) -> Result<Vec<RatingPath>, MigrationError> {
        let n_states = self.generator.n_states();
        if initial_state >= n_states {
            return Err(MigrationError::InvalidState {
                state: initial_state,
                n_states,
            });
        }
        let scale = Arc::new(self.generator.scale.clone());
        Ok((0..n_paths)
            .map(|_| simulate_path(&self.generator, &scale, initial_state, self.horizon, rng))
            .collect())
    }

    /// Estimate the transition matrix from batch simulation.
    ///
    /// Runs `n_paths_per_state` paths from every state and records the terminal
    /// state at `self.horizon` to build the empirical transition matrix.
    ///
    /// # Arguments
    ///
    /// * `n_paths_per_state` — Paths per starting state.
    /// * `rng` — Any `rand::Rng` source.
    pub fn empirical_matrix<R: Rng>(
        &self,
        n_paths_per_state: usize,
        rng: &mut R,
    ) -> Result<TransitionMatrix, MigrationError> {
        if n_paths_per_state == 0 {
            return Err(MigrationError::InvalidPathCount);
        }
        let n = self.generator.n_states();
        // Flat row-major counts (index `from * n + to`) for cache-friendly
        // accumulation and a single allocation.
        let mut counts = vec![0usize; n * n];

        for from in 0..n {
            for _ in 0..n_paths_per_state {
                let to = simulate_terminal_state(&self.generator, from, self.horizon, rng);
                counts[from * n + to] += 1;
            }
        }

        // Normalize each row to sum to 1.0 in place.
        let mut data = vec![0.0f64; n * n];
        for from in 0..n {
            let row = &counts[from * n..(from + 1) * n];
            let row_sum: usize = row.iter().sum();
            let divisor = row_sum as f64;
            for to in 0..n {
                data[from * n + to] = row[to] as f64 / divisor;
            }
        }

        TransitionMatrix::new(self.generator.scale.clone(), &data, self.horizon)
    }

    /// The generator matrix.
    #[must_use]
    pub fn generator(&self) -> &GeneratorMatrix {
        &self.generator
    }

    /// The simulation horizon.
    #[must_use]
    pub fn horizon(&self) -> f64 {
        self.horizon
    }
}

// ---------------------------------------------------------------------------
// Core Gillespie algorithm
// ---------------------------------------------------------------------------

/// Run Gillespie's competing-exponentials scheme and return the terminal state.
///
/// `on_transition` receives each `(time, new_state)` transition. Both path
/// recording and terminal-only simulation use this loop and RNG draw order.
fn simulate_gillespie<R: Rng>(
    gen: &GeneratorMatrix,
    initial_state: usize,
    horizon: f64,
    rng: &mut R,
    mut on_transition: impl FnMut(f64, usize),
) -> usize {
    let n = gen.n_states();
    let mut t = 0.0;
    let mut state = initial_state;

    loop {
        let lambda = -gen.data[(state, state)]; // exit rate = -q_ss

        // Absorbing state: stop.
        if lambda < 1e-15 {
            break;
        }

        // Draw holding time from Exp(lambda).
        let u1: f64 = rng.random();
        let holding = -u1.ln() / lambda;

        if t + holding > horizon {
            break;
        }

        t += holding;

        // Draw next state from Categorical(q_sj / lambda for j != s).
        let u2: f64 = rng.random();
        let mut cumulative = 0.0;
        let mut next_state = state; // fallback — should always be reassigned below

        for j in 0..n {
            if j == state {
                continue;
            }
            cumulative += gen.data[(state, j)] / lambda;
            if u2 < cumulative {
                next_state = j;
                break;
            }
        }
        // If floating-point rounding left u2 >= total, assign the last valid state.
        if next_state == state {
            // Find last non-zero off-diagonal.
            for j in (0..n).rev() {
                if j != state && gen.data[(state, j)] > 0.0 {
                    next_state = j;
                    break;
                }
            }
        }

        state = next_state;
        on_transition(t, state);

        // Absorbing state (e.g., default): stop immediately.
        if -gen.data[(state, state)] < 1e-15 {
            break;
        }
    }

    state
}

/// Simulate a single rating path using Gillespie's competing exponentials.
fn simulate_path<R: Rng>(
    gen: &GeneratorMatrix,
    scale: &Arc<RatingScale>,
    initial_state: usize,
    horizon: f64,
    rng: &mut R,
) -> RatingPath {
    let mut transitions = Vec::with_capacity(8);
    transitions.push((0.0, initial_state));
    simulate_gillespie(gen, initial_state, horizon, rng, |t, state| {
        transitions.push((t, state));
    });

    RatingPath {
        transitions,
        horizon,
        scale: Arc::clone(scale),
    }
}

/// Simulate one path without allocating a transition history.
fn simulate_terminal_state<R: Rng>(
    gen: &GeneratorMatrix,
    initial_state: usize,
    horizon: f64,
    rng: &mut R,
) -> usize {
    simulate_gillespie(gen, initial_state, horizon, rng, |_, _| {})
}
