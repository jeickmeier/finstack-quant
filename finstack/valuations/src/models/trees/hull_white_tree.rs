//! Hull-White trinomial tree for Bermudan swaption pricing.
//!
//! Implements the industry-standard Hull-White 1-factor short rate model on a
//! recombining trinomial lattice. The tree is calibrated to the initial yield
//! curve via forward induction on the drift parameter α(t).
//!
//! # Model Dynamics
//!
//! The Hull-White model specifies the following short rate dynamics:
//!
//! ```text
//! dr(t) = [θ(t) - κr(t)]dt + σdW(t)
//! ```
//!
//! where:
//! - κ = mean reversion speed
//! - σ = short rate volatility
//! - θ(t) = time-dependent drift calibrated to fit initial yield curve
//!
//! # Tree Construction
//!
//! The tree uses a two-phase approach:
//! 1. Build tree in auxiliary x-space where x(t) = r(t) - α(t)
//! 2. Calibrate α(t) via forward induction to match discount curve
//!
//! The x-variable follows:
//! ```text
//! dx(t) = -κx(t)dt + σdW(t)
//! ```
//!
//! which has constant transition probabilities at each step.
//!
//! # References
//!
//! - Hull, J. & White, A. (1994). "Numerical Procedures for Implementing
//!   Term Structure Models I: Single-Factor Models", *Journal of Derivatives*
//! - Hull, J. (2018). *Options, Futures, and Other Derivatives*, 10th ed.
//!   Chapter 31: Interest Rate Derivatives: Models of the Short Rate

use super::short_rate_tree::TreeCompounding;
use crate::instruments::common_impl::validation;
use finstack_core::market_data::traits::Discounting;
use finstack_core::{Error, Result};

// ============================================================================
// Configuration
// ============================================================================

/// Hull-White 1-factor trinomial tree configuration.
///
/// # Parameter Guidelines
///
/// | Parameter | Typical Range | Description |
/// |-----------|---------------|-------------|
/// | kappa | 0.01-0.10 | Mean reversion (higher = faster reversion) |
/// | sigma | 0.005-0.015 | Normal volatility (50-150 bps) |
/// | steps | 50-200 | Tree steps (more = accuracy, O(n²) cost) |
#[derive(Debug, Clone)]
pub struct HullWhiteTreeConfig {
    /// Mean reversion speed (κ), annualized.
    ///
    /// Higher values cause rates to revert faster to the mean.
    /// Typical range: 0.01-0.10 (1-10% per year)
    pub kappa: f64,

    /// Short rate volatility (σ), annualized.
    ///
    /// This is normal/absolute volatility in rate units.
    /// Typical range: 0.005-0.015 (50-150 bps per year)
    pub sigma: f64,

    /// Number of time steps in the tree.
    ///
    /// More steps improve accuracy but increase computation time O(n²).
    /// Typical values: 50 (fast), 100 (standard), 200+ (high precision)
    pub steps: usize,

    /// Maximum number of nodes per step (limits tree width).
    ///
    /// For mean-reverting processes, the tree doesn't grow indefinitely.
    /// Default: 2 * steps + 1 (sufficient for most cases)
    pub max_nodes: Option<usize>,

    /// Per-node discount factor convention.
    ///
    /// Controls whether calibration and backward induction use continuous
    /// `exp(-r*dt)` or periodic compounding. Default: `Continuous`.
    pub compounding: TreeCompounding,
}

impl Default for HullWhiteTreeConfig {
    fn default() -> Self {
        Self {
            kappa: 0.03,
            sigma: 0.01,
            steps: 100,
            max_nodes: None,
            compounding: TreeCompounding::default(),
        }
    }
}

impl HullWhiteTreeConfig {
    /// Create a new configuration with specified parameters.
    pub fn new(kappa: f64, sigma: f64, steps: usize) -> Self {
        Self {
            kappa,
            sigma,
            steps,
            max_nodes: None,
            compounding: TreeCompounding::default(),
        }
    }

    /// Set maximum nodes per step.
    pub fn with_max_nodes(mut self, max_nodes: usize) -> Self {
        self.max_nodes = Some(max_nodes);
        self
    }

    /// Validate configuration parameters.
    pub fn validate(&self) -> Result<()> {
        validation::require_with(self.kappa > 0.0, || "kappa must be positive".into())?;
        validation::require_with(self.sigma > 0.0, || "sigma must be positive".into())?;
        validation::require_with(self.steps >= 2, || "steps must be at least 2".into())?;
        Ok(())
    }
}

// ============================================================================
// Hull-White Trinomial Tree
// ============================================================================

/// Calibrated Hull-White trinomial tree.
///
/// The tree is built and calibrated via [`HullWhiteTree::calibrate`] (uniform
/// grid) or [`HullWhiteTree::calibrate_with_times`] (grid refined to pass
/// exactly through caller-supplied mandatory dates such as exercise or coupon
/// times). After calibration, it can compute bond prices, forward swap rates,
/// and annuities at any node.
///
/// # Node Indexing
///
/// Each level `i` covers a contiguous signed-index range
/// `[j_min(i), j_min(i) + width(i) - 1]`. Nodes are addressed by their array
/// index `idx` in `0..width(i)`, with signed index `j = j_min(i) + idx`.
///
/// The x-value at node (i, idx) is: `x = j * dx(i)` where `dx(i) = σ√(3·dt_{i-1})`
/// is the level spacing implied by the preceding step's size.
///
/// The short rate at node (i, idx) is: `r = x + alpha[i]`
///
/// # Branching
///
/// Each node stores its central child (array index at the next level) plus
/// trinomial probabilities matched to the conditional mean `x·(1 − κ·dt_i)`
/// and variance `σ²·dt_i` of the mean-reverting x-process. The central child
/// is chosen as the next-level node closest to the conditional mean, so the
/// lattice self-limits its width once mean reversion pulls strongly enough —
/// no explicit boundary-branching switch is needed.
#[derive(Debug, Clone)]
pub struct HullWhiteTree {
    /// Configuration parameters
    config: HullWhiteTreeConfig,
    /// Time grid (year fractions from t=0), `n+1` entries
    time_grid: Vec<f64>,
    /// Per-step time sizes: `dts[i] = time_grid[i+1] - time_grid[i]`, `n` entries
    dts: Vec<f64>,
    /// Per-level x-spacing: `dxs[i] = σ√(3·dts[i-1])` (`dxs[0] = dxs[1]`), `n+1` entries
    dxs: Vec<f64>,
    /// Lowest signed node index at each level, `n+1` entries
    j_mins: Vec<i32>,
    /// Number of nodes at each level, `n+1` entries
    widths: Vec<usize>,
    /// α(t) drift parameter at each time step (calibrated to yield curve)
    alpha: Vec<f64>,
    /// Per-step, per-node branching geometry, `n` entries
    branches: Vec<Vec<NodeBranch>>,
    /// Arrow-Debreu state prices Q(t,j) for verification
    state_prices: Vec<Vec<f64>>,
}

/// Absolute tolerance used to match mandatory times against grid points.
const GRID_TIME_TOLERANCE: f64 = 1e-9;

/// Branch geometry per node: (central child array index at the next level,
/// (p_up, p_mid, p_down)).
type NodeBranch = (usize, (f64, f64, f64));

impl HullWhiteTree {
    /// Build and calibrate a Hull-White tree to match the discount curve.
    ///
    /// # Arguments
    ///
    /// * `config` - Tree configuration (κ, σ, steps)
    /// * `discount_curve` - Initial yield curve for calibration
    /// * `time_to_maturity` - Total tree horizon in years (must be finite and
    ///   strictly positive)
    ///
    /// # Returns
    ///
    /// Calibrated tree ready for pricing
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] if the
    /// configuration is invalid or `time_to_maturity` is not a finite,
    /// strictly positive number of years.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use finstack_valuations::models::trees::HullWhiteTree;
    /// use finstack_valuations::models::trees::HullWhiteTreeConfig;
    ///
    /// let config = HullWhiteTreeConfig::default();
    /// # let discount_curve: &dyn finstack_core::market_data::traits::Discounting = todo!();
    /// let tree = HullWhiteTree::calibrate(config, discount_curve, 10.0)?;
    /// # Ok::<(), finstack_core::Error>(())
    /// ```
    pub fn calibrate(
        config: HullWhiteTreeConfig,
        discount_curve: &dyn Discounting,
        time_to_maturity: f64,
    ) -> Result<Self> {
        Self::calibrate_with_times(config, discount_curve, time_to_maturity, &[])
    }

    /// Build and calibrate a Hull-White tree whose time grid passes exactly
    /// through caller-supplied mandatory times.
    ///
    /// The grid is built segment-by-segment between consecutive mandatory
    /// times (exercise dates, coupon dates, fixing dates, ...), with each
    /// segment subdivided so the total step count stays close to
    /// `config.steps` while every mandatory time lands exactly on a grid
    /// point. Steps therefore have per-step sizes `dt_i`, and each level
    /// uses spacing `dx_i = σ√(3·dt_{i-1})`.
    ///
    /// With an empty `mandatory_times`, this degenerates to the uniform grid
    /// built by [`HullWhiteTree::calibrate`].
    ///
    /// # Arguments
    ///
    /// * `config` - Tree configuration (κ, σ, target steps)
    /// * `discount_curve` - Initial yield curve for calibration
    /// * `time_to_maturity` - Total tree horizon in years
    /// * `mandatory_times` - Times (year fractions) that must coincide with
    ///   grid points; entries outside `(0, time_to_maturity)` are ignored
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] if the configuration is
    /// invalid, `time_to_maturity` is not a finite positive number, a
    /// mandatory time is not finite, or calibration produces invalid
    /// transition probabilities.
    pub fn calibrate_with_times(
        config: HullWhiteTreeConfig,
        discount_curve: &dyn Discounting,
        time_to_maturity: f64,
        mandatory_times: &[f64],
    ) -> Result<Self> {
        config.validate()?;

        // Validate the tree horizon up front. Without this check a
        // non-positive `time_to_maturity` yields `dt <= 0`, and the failure
        // only surfaces deep inside the probability computation as the
        // misleading "probabilities require finite, positive inputs". The
        // `is_finite` check runs first so the subsequent `> 0.0` cannot see
        // a `NaN`.
        let horizon_ok = time_to_maturity.is_finite() && time_to_maturity > 0.0;
        if !horizon_ok {
            return Err(finstack_core::Error::Validation(format!(
                "Hull-White tree requires a finite, strictly positive \
                 time_to_maturity in years, got {time_to_maturity}"
            )));
        }

        let time_grid = Self::build_time_grid(config.steps, time_to_maturity, mandatory_times)?;
        let n = time_grid.len() - 1;

        let dts: Vec<f64> = time_grid.windows(2).map(|w| w[1] - w[0]).collect();
        // Level spacing: dx_i = σ√(3·dt_{i-1}) matches the variance of the
        // step *arriving* at level i. Level 0 has a single node at x = 0, so
        // its spacing is irrelevant; mirror dx_1 for accessor consistency.
        let mut dxs = Vec::with_capacity(n + 1);
        dxs.push(config.sigma * (3.0 * dts[0]).sqrt());
        for &dt_i in &dts {
            dxs.push(config.sigma * (3.0 * dt_i).sqrt());
        }

        // Optional hard cap on level width from `max_nodes`: signed index
        // |j| ≤ j_cap. Central children are clamped to keep all three
        // branches inside the cap; if the cap is too tight the resulting
        // probabilities go negative and calibration fails loudly below.
        let j_cap: Option<i32> = config.max_nodes.map(|m| ((m.max(3) - 1) / 2) as i32);

        let mut j_mins: Vec<i32> = vec![0];
        let mut widths: Vec<usize> = vec![1];
        let mut alpha = vec![0.0; n + 1];
        let mut branches: Vec<Vec<NodeBranch>> = Vec::with_capacity(n);
        let mut state_prices: Vec<Vec<f64>> = Vec::with_capacity(n + 1);

        // Initial state: single node at t=0 with Q(0,0) = 1
        state_prices.push(vec![1.0]);

        // Forward induction: build branching geometry, calibrate α(t) for
        // each interval [t_step, t_{step+1}], and roll state prices forward.
        //
        // The α solved at iteration `step` is the drift adjustment for the
        // interval [t_step, t_{step+1}] (it matches P(0, t_{step+1}) given
        // the state prices at t_step), so it is stored at `alpha[step]` —
        // the index `backward_induction` uses to discount that interval
        // .
        for step in 0..n {
            let dt_i = dts[step];
            let dx_curr = dxs[step];
            let dx_next = dxs[step + 1];
            let variance = config.sigma * config.sigma * dt_i;

            // Pass 1: central child (signed index at next level) per node.
            // The conditional mean of x over the step is x·(1 − κ·dt_i);
            // branch around the closest next-level node.
            let width = widths[step];
            let j_min = j_mins[step];
            let mut centers = Vec::with_capacity(width);
            for idx in 0..width {
                let j = j_min + idx as i32;
                let x = j as f64 * dx_curr;
                let m_target = x * (1.0 - config.kappa * dt_i);
                let mut k = (m_target / dx_next).round() as i32;
                if let Some(cap) = j_cap {
                    k = k.clamp(-(cap - 1), cap - 1);
                }
                centers.push(k);
            }

            let next_j_min = centers.iter().copied().min().unwrap_or(0) - 1;
            let next_j_max = centers.iter().copied().max().unwrap_or(0) + 1;
            let next_width = (next_j_max - next_j_min + 1) as usize;

            // Pass 2: probabilities matched to the conditional mean and
            // variance around each central child.
            let mut step_branches = Vec::with_capacity(width);
            for (idx, &k) in centers.iter().enumerate() {
                let j = j_min + idx as i32;
                let x = j as f64 * dx_curr;
                let m_target = x * (1.0 - config.kappa * dt_i);
                let eta = m_target - k as f64 * dx_next;
                let probs = Self::branch_probabilities(eta, variance, dx_next, j)?;
                step_branches.push(((k - next_j_min) as usize, probs));
            }

            // Calibrate α for the interval [t_step, t_next] to match the
            // discount factor at t_next.
            let target_df = discount_curve.df(time_grid[step + 1]);
            alpha[step] = Self::calibrate_alpha(
                &state_prices[step],
                j_min,
                dx_curr,
                dt_i,
                target_df,
                config.compounding,
            )?;

            // Roll Arrow-Debreu state prices forward through the branches.
            let mut next_q = vec![0.0; next_width];
            for (idx, &(center, (p_up, p_mid, p_down))) in step_branches.iter().enumerate() {
                let j = j_min + idx as i32;
                let r_j = j as f64 * dx_curr + alpha[step];
                let contribution = state_prices[step][idx] * config.compounding.df(r_j, dt_i);
                next_q[center + 1] += contribution * p_up;
                next_q[center] += contribution * p_mid;
                next_q[center - 1] += contribution * p_down;
            }

            branches.push(step_branches);
            state_prices.push(next_q);
            j_mins.push(next_j_min);
            widths.push(next_width);
        }

        // Terminal row: no interval [t_N, t_{N+1}] exists to calibrate, so
        // extend the last solved drift for accessors that read rates at the
        // final step.
        if n > 0 {
            alpha[n] = alpha[n - 1];
        }

        Ok(Self {
            config,
            time_grid,
            dts,
            dxs,
            j_mins,
            widths,
            alpha,
            branches,
            state_prices,
        })
    }

    /// Build a time grid from `0` to `time_to_maturity` that passes exactly
    /// through every mandatory time, targeting ~`steps` total steps.
    ///
    /// Each segment between consecutive anchors is subdivided uniformly with
    /// the number of substeps chosen so the local step size stays close to
    /// the global target `time_to_maturity / steps`.
    fn build_time_grid(
        steps: usize,
        time_to_maturity: f64,
        mandatory_times: &[f64],
    ) -> Result<Vec<f64>> {
        for &t in mandatory_times {
            if !t.is_finite() {
                return Err(Error::Validation(format!(
                    "Hull-White tree mandatory times must be finite, got {t}"
                )));
            }
        }

        let mut anchors: Vec<f64> = mandatory_times
            .iter()
            .copied()
            .filter(|&t| t > GRID_TIME_TOLERANCE && t < time_to_maturity - GRID_TIME_TOLERANCE)
            .collect();
        anchors.sort_by(|a, b| a.total_cmp(b));
        anchors.dedup_by(|a, b| (*a - *b).abs() <= GRID_TIME_TOLERANCE);
        anchors.push(time_to_maturity);

        let target_dt = time_to_maturity / steps as f64;
        let mut grid = Vec::with_capacity(steps + anchors.len() + 1);
        grid.push(0.0);
        let mut prev = 0.0;
        for &anchor in &anchors {
            let segment = anchor - prev;
            let substeps = ((segment / target_dt).round() as usize).max(1);
            for i in 1..=substeps {
                grid.push(prev + segment * i as f64 / substeps as f64);
            }
            // Force the anchor to land exactly (no floating-point drift).
            if let Some(last) = grid.last_mut() {
                *last = anchor;
            }
            prev = anchor;
        }
        Ok(grid)
    }

    /// Trinomial probabilities matched to a conditional mean offset `eta`
    /// (distance from the central child, in rate units) and conditional
    /// variance `variance`, on a next-level spacing `dx`.
    ///
    /// ```text
    /// p_up   = (variance + eta²)/(2dx²) + eta/(2dx)
    /// p_mid  = 1 − (variance + eta²)/dx²
    /// p_down = (variance + eta²)/(2dx²) − eta/(2dx)
    /// ```
    ///
    /// With `dx² = 3·variance` and `eta = 0` this reduces to the canonical
    /// (1/6, 2/3, 1/6) interior branching.
    fn branch_probabilities(eta: f64, variance: f64, dx: f64, j: i32) -> Result<(f64, f64, f64)> {
        if !eta.is_finite() || !variance.is_finite() || !dx.is_finite() || dx <= 0.0 {
            return Err(Error::Validation(
                "Hull-White probabilities require finite, positive inputs".to_string(),
            ));
        }

        let a = (variance + eta * eta) / (dx * dx);
        let b = eta / dx;
        let mut p_up = (a + b) / 2.0;
        let mut p_mid = 1.0 - a;
        let mut p_down = (a - b) / 2.0;

        // Ensure probabilities are valid (handle numerical edge cases)
        if p_up < 0.0
            || p_mid < 0.0
            || p_down < 0.0
            || !p_up.is_finite()
            || !p_mid.is_finite()
            || !p_down.is_finite()
        {
            return Err(Error::Validation(format!(
                "Hull-White probabilities invalid at j={j} (p_up={p_up}, p_mid={p_mid}, p_down={p_down})"
            )));
        }

        // Normalize to ensure sum = 1
        let sum = p_up + p_mid + p_down;
        if sum > 0.0 && sum.is_finite() {
            p_up /= sum;
            p_mid /= sum;
            p_down /= sum;
        } else {
            return Err(Error::Validation(
                "Hull-White probabilities did not sum to a finite value".to_string(),
            ));
        }

        let normalized_sum = p_up + p_mid + p_down;
        if (normalized_sum - 1.0).abs() > 1.0e-9 {
            return Err(Error::Validation(format!(
                "Hull-White probabilities failed the sum-to-one invariant at \
                 j={j}: p_up={p_up}, p_mid={p_mid}, p_down={p_down} (sum={normalized_sum})"
            )));
        }

        Ok((p_up, p_mid, p_down))
    }

    /// Compute trinomial transition probabilities for node j.
    ///
    /// For the Hull-White model with mean reversion κ:
    /// - p_up = 1/6 + (j²M² - jM)/2
    /// - p_mid = 2/3 - j²M²
    /// - p_down = 1/6 + (j²M² + jM)/2
    ///
    /// where M = κ·dt
    ///
    /// At boundaries (|j| >= j_max), we use drift-adjusted branching that:
    /// 1. Prevents the tree from growing beyond j_max
    /// 2. Accounts for mean reversion to maintain martingale property
    ///
    /// `pub(crate)`: the Black-Karasinski trinomial lattice in
    /// `short_rate_tree.rs` reuses this geometry — its x = ln r process is the
    /// same mean-reverting OU dynamics this branching discretizes.
    pub(crate) fn compute_probabilities(
        kappa: f64,
        dt: f64,
        dx: f64,
        j: i32,
        j_max: usize,
    ) -> finstack_core::Result<(f64, f64, f64)> {
        if !kappa.is_finite() || !dt.is_finite() || !dx.is_finite() || dt <= 0.0 || dx <= 0.0 {
            return Err(finstack_core::Error::Validation(
                "Hull-White probabilities require finite, positive inputs".to_string(),
            ));
        }

        let m = kappa * dt;
        let jf = j as f64;

        // Standard interior node probabilities (Hull-White trinomial).
        // The expected offset is -j*kappa*dt, pulling x back toward zero.
        let mut p_up = 1.0 / 6.0 + (jf * jf * m * m - jf * m) / 2.0;
        let mut p_mid = 2.0 / 3.0 - jf * jf * m * m;
        let mut p_down = 1.0 / 6.0 + (jf * jf * m * m + jf * m) / 2.0;

        // At boundaries (|j| >= j_max), use Hull & White (1994) shifted
        // branching to stay inside the capped lattice while matching the first
        // two moments.
        //
        // The tuple still stores probabilities in the branch order used by
        // transition_offsets(): upper boundary (0, -1, -2), lower boundary
        // (+2, +1, 0), interior (+1, 0, -1).
        let j_abs = j.unsigned_abs() as usize;
        if j_abs >= j_max && j_max > 0 {
            let mean = -jf * m;
            let second_moment = 1.0 / 3.0 + mean * mean;
            if j > 0 {
                // Upper boundary Type B: offsets 0, -1, -2.
                p_down = (second_moment + mean) / 2.0;
                p_mid = -second_moment - 2.0 * mean;
                p_up = 1.0 - p_mid - p_down;
            } else if j < 0 {
                // Lower boundary Type C: offsets +2, +1, 0.
                p_up = (second_moment - mean) / 2.0;
                p_mid = 2.0 * mean - second_moment;
                p_down = 1.0 - p_up - p_mid;
            }
        }

        // Ensure probabilities are valid (handle numerical edge cases)
        if p_up < 0.0
            || p_mid < 0.0
            || p_down < 0.0
            || !p_up.is_finite()
            || !p_mid.is_finite()
            || !p_down.is_finite()
        {
            return Err(finstack_core::Error::Validation(format!(
                "Hull-White probabilities invalid at j={j} (p_up={p_up}, p_mid={p_mid}, p_down={p_down})"
            )));
        }

        // Normalize to ensure sum = 1
        let sum = p_up + p_mid + p_down;
        if sum > 0.0 && sum.is_finite() {
            p_up /= sum;
            p_mid /= sum;
            p_down /= sum;
        } else {
            return Err(finstack_core::Error::Validation(
                "Hull-White probabilities did not sum to a finite value".to_string(),
            ));
        }

        // Enforce the probability-sum invariant in *all* builds. Each branch
        // is already non-negative (checked above) and `sum` is strictly
        // positive, so dividing keeps every branch in `[0, 1]` and the three
        // sum to 1 up to rounding — no redundant final clamp is needed (a
        // clamp here could only re-break the sum it is meant to protect).
        // A release-mode check (rather than `debug_assert!`) guarantees a
        // mispriced lattice can never escape this function silently.
        let normalized_sum = p_up + p_mid + p_down;
        if (normalized_sum - 1.0).abs() > 1.0e-9 {
            return Err(finstack_core::Error::Validation(format!(
                "Hull-White probabilities failed the sum-to-one invariant at \
                 j={j}: p_up={p_up}, p_mid={p_mid}, p_down={p_down} (sum={normalized_sum})"
            )));
        }

        Ok((p_up, p_mid, p_down))
    }

    pub(crate) fn transition_offsets(
        j: i32,
        j_max: usize,
        probs: (f64, f64, f64),
    ) -> [(i32, f64); 3] {
        let (p_up, p_mid, p_down) = probs;
        let j_abs = j.unsigned_abs() as usize;
        if j_abs >= j_max && j_max > 0 {
            if j > 0 {
                // Upper boundary: branches to j, j-1, j-2.
                [(0, p_up), (-1, p_mid), (-2, p_down)]
            } else if j < 0 {
                // Lower boundary: branches to j+2, j+1, j.
                [(2, p_up), (1, p_mid), (0, p_down)]
            } else {
                [(1, p_up), (0, p_mid), (-1, p_down)]
            }
        } else {
            [(1, p_up), (0, p_mid), (-1, p_down)]
        }
    }

    pub(crate) fn transition_index(j: i32, offset: i32, next_j_max: usize) -> Option<usize> {
        let next_j = j + offset;
        let lower = -(next_j_max as i32);
        let upper = next_j_max as i32;
        if (lower..=upper).contains(&next_j) {
            Some((next_j + next_j_max as i32) as usize)
        } else {
            None
        }
    }

    /// Calibrate α for the interval starting at the current level to match
    /// the target discount factor.
    ///
    /// For continuous compounding, the closed-form solution is used:
    ///   `exp(-α*dt) * Σ Q(t,j) * exp(-x(t,j)*dt) = target_df`
    ///   → `α = -ln(target_df / weighted_sum) / dt`
    ///
    /// For periodic compounding, a numerical root-find is used since α
    /// enters nonlinearly into the per-node discount factor
    /// `comp.df(x_j + α, dt)`.
    fn calibrate_alpha(
        curr_state_prices: &[f64],
        j_min: i32,
        dx: f64,
        dt: f64,
        target_df: f64,
        compounding: TreeCompounding,
    ) -> Result<f64> {
        match compounding {
            TreeCompounding::Continuous => {
                let mut weighted_sum = 0.0;
                for (idx, &q) in curr_state_prices.iter().enumerate() {
                    let j = j_min + idx as i32;
                    let x_j = j as f64 * dx;
                    weighted_sum += q * (-x_j * dt).exp();
                }
                if weighted_sum <= 0.0 {
                    return Err(Error::Validation(
                        "Invalid state prices in tree calibration".into(),
                    ));
                }
                Ok(-(target_df / weighted_sum).ln() / dt)
            }
            _ => {
                use finstack_core::math::solver::{BrentSolver, Solver};
                let objective = |alpha: f64| -> f64 {
                    let mut model_df = 0.0;
                    for (idx, &q) in curr_state_prices.iter().enumerate() {
                        let j = j_min + idx as i32;
                        let x_j = j as f64 * dx;
                        model_df += q * compounding.df(x_j + alpha, dt);
                    }
                    model_df - target_df
                };
                let initial_guess = if dt > 0.0 {
                    -(target_df).ln() / dt
                } else {
                    0.03
                };
                BrentSolver::new()
                    .solve(objective, initial_guess)
                    .map_err(|e| Error::Validation(format!("HW alpha calibration failed: {e}")))
            }
        }
    }

    // ========================================================================
    // Accessor Methods
    // ========================================================================

    /// Get configuration.
    pub fn config(&self) -> &HullWhiteTreeConfig {
        &self.config
    }

    /// Get number of time steps.
    pub fn num_steps(&self) -> usize {
        self.time_grid.len() - 1
    }

    /// Get time at step i.
    pub fn time_at_step(&self, step: usize) -> f64 {
        self.time_grid.get(step).copied().unwrap_or(0.0)
    }

    /// Get the full time grid (year fractions from t=0), `num_steps() + 1`
    /// entries.
    pub fn time_grid(&self) -> &[f64] {
        &self.time_grid
    }

    /// Get the time step size of the interval `[t_step, t_{step+1}]`.
    ///
    /// Out-of-range steps return the last interval's size.
    pub fn dt_at_step(&self, step: usize) -> f64 {
        self.dts
            .get(step)
            .or(self.dts.last())
            .copied()
            .unwrap_or(0.0)
    }

    /// Get number of nodes at a given step.
    pub fn num_nodes(&self, step: usize) -> usize {
        self.widths.get(step).copied().unwrap_or(0)
    }

    /// Get short rate r(t,j) at node (step, node_idx).
    pub fn rate_at_node(&self, step: usize, node_idx: usize) -> f64 {
        let j_min = self.j_mins.get(step).copied().unwrap_or(0);
        let dx = self.dxs.get(step).copied().unwrap_or(0.0);
        let x_j = (j_min + node_idx as i32) as f64 * dx;
        x_j + self.alpha.get(step).copied().unwrap_or(0.0)
    }

    /// Get transition probabilities at node (step, node_idx).
    ///
    /// Returns (p_up, p_mid, p_down).
    pub fn probabilities(&self, step: usize, node_idx: usize) -> (f64, f64, f64) {
        self.branches
            .get(step)
            .and_then(|p| p.get(node_idx))
            .map(|&(_, probs)| probs)
            .unwrap_or((1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0))
    }

    /// Get the central child of node (step, node_idx) as an array index at
    /// level `step + 1`.
    ///
    /// The up/down children are `center + 1` / `center - 1`. Out-of-range
    /// queries return 0.
    pub fn branch_center(&self, step: usize, node_idx: usize) -> usize {
        self.branches
            .get(step)
            .and_then(|p| p.get(node_idx))
            .map(|&(center, _)| center)
            .unwrap_or(0)
    }

    /// Get state price Q(t,j) at node (step, node_idx).
    pub fn state_price(&self, step: usize, node_idx: usize) -> f64 {
        self.state_prices
            .get(step)
            .and_then(|p| p.get(node_idx))
            .copied()
            .unwrap_or(0.0)
    }

    // ========================================================================
    // Bond Price Calculations
    // ========================================================================

    /// Compute zero-coupon bond price P(t, T) at node (step, node_idx).
    ///
    /// Uses the Hull-White analytical formula:
    /// ```text
    /// P(t, T) = A(t, T) * exp(-B(t, T) * r(t))
    /// ```
    ///
    /// where:
    /// - B(t, T) = (1 - exp(-κ(T-t))) / κ
    /// - A(t, T) = P(0,T)/P(0,t) * exp(B(t,T)*f(0,t) - σ²(1-e^(-2κt))B²/(4κ))
    ///
    /// # Arguments
    ///
    /// * `step` - Current time step
    /// * `node_idx` - Node index at current step
    /// * `maturity_time` - Bond maturity time T (year fraction from t=0)
    /// * `discount_curve` - Initial yield curve for A(t,T) calculation
    pub fn bond_price(
        &self,
        step: usize,
        node_idx: usize,
        maturity_time: f64,
        discount_curve: &dyn Discounting,
    ) -> f64 {
        let t = self.time_at_step(step);
        let tau = maturity_time - t;

        if tau <= 0.0 {
            return 1.0;
        }

        let r = self.rate_at_node(step, node_idx);
        let kappa = self.config.kappa;
        let sigma = self.config.sigma;

        // B(t, T) factor
        let b = if kappa.abs() < 1e-10 {
            tau // Limit as κ → 0
        } else {
            (1.0 - (-kappa * tau).exp()) / kappa
        };

        // A(t, T) factor using market discount factors
        let p_0_t = discount_curve.df(t);
        let p_0_tt = discount_curve.df(maturity_time);

        if p_0_t <= 0.0 {
            return 0.0;
        }

        // Forward rate at t=0 for maturity t
        let f_0_t = if t > 0.0 {
            discount_curve.instantaneous_forward(t).unwrap_or_else(|e| {
                // Fall back to the average zero rate -ln P(0,t)/t. This keeps
                // the f64 signature (bond_price is called inside f64-returning
                // backward-induction closures) but the substitution is no
                // longer silent.
                tracing::warn!(
                    time = t,
                    error = %e,
                    "HullWhiteTree::bond_price: instantaneous forward unavailable; \
                     falling back to average zero rate -ln P(0,t)/t"
                );
                -p_0_t.ln() / t
            })
        } else {
            self.alpha[0]
        };

        // Variance term
        let var_term = if kappa.abs() < 1e-10 {
            sigma * sigma * t * b * b / 2.0
        } else {
            sigma * sigma * (1.0 - (-2.0 * kappa * t).exp()) * b * b / (4.0 * kappa)
        };

        let ln_a = (p_0_tt / p_0_t).ln() + b * f_0_t - var_term;
        let a = ln_a.exp();

        a * (-b * r).exp()
    }

    /// Compute forward swap rate S(t) at node (step, node_idx).
    ///
    /// The forward swap rate is computed as:
    /// ```text
    /// S(t) = [P(t, T_start) - P(t, T_end)] / A(t)
    /// ```
    ///
    /// where A(t) = Σᵢ τᵢ P(t, Tᵢ) is the annuity.
    ///
    /// # Arguments
    ///
    /// * `step` - Current time step
    /// * `node_idx` - Node index
    /// * `swap_start_time` - Swap start time (year fraction)
    /// * `swap_end_time` - Swap end time (year fraction)
    /// * `payment_times` - Payment date times (year fractions)
    /// * `accrual_fractions` - Accrual fractions for each period
    /// * `discount_curve` - Initial yield curve
    #[allow(clippy::too_many_arguments)]
    pub fn forward_swap_rate(
        &self,
        step: usize,
        node_idx: usize,
        swap_start_time: f64,
        swap_end_time: f64,
        payment_times: &[f64],
        accrual_fractions: &[f64],
        discount_curve: &dyn Discounting,
    ) -> f64 {
        let t = self.time_at_step(step);

        // Filter to remaining payments
        let remaining: Vec<_> = payment_times
            .iter()
            .zip(accrual_fractions.iter())
            .filter(|(&pay_t, _)| pay_t > t)
            .collect();

        if remaining.is_empty() {
            return 0.0;
        }

        // Start discount factor (or 1.0 if already started)
        let p_start = if swap_start_time > t {
            self.bond_price(step, node_idx, swap_start_time, discount_curve)
        } else {
            1.0
        };

        // End discount factor
        let p_end = self.bond_price(step, node_idx, swap_end_time, discount_curve);

        // Annuity
        let annuity = self.annuity(
            step,
            node_idx,
            payment_times,
            accrual_fractions,
            discount_curve,
        );

        if annuity.abs() < 1e-12 {
            return 0.0;
        }

        (p_start - p_end) / annuity
    }

    /// Compute annuity (PV01) at node (step, node_idx).
    ///
    /// ```text
    /// A(t) = Σᵢ τᵢ P(t, Tᵢ)
    /// ```
    ///
    /// Only includes payments occurring after time t.
    pub fn annuity(
        &self,
        step: usize,
        node_idx: usize,
        payment_times: &[f64],
        accrual_fractions: &[f64],
        discount_curve: &dyn Discounting,
    ) -> f64 {
        let t = self.time_at_step(step);

        payment_times
            .iter()
            .zip(accrual_fractions.iter())
            .filter(|(&pay_t, _)| pay_t > t)
            .map(|(&pay_t, &tau)| tau * self.bond_price(step, node_idx, pay_t, discount_curve))
            .sum()
    }

    // ========================================================================
    // Backward Induction
    // ========================================================================

    /// Price an instrument using backward induction.
    ///
    /// # Arguments
    ///
    /// * `terminal_values` - Payoff values at maturity for each node
    /// * `intermediate_value_fn` - Called at each step to adjust for early exercise/coupons
    ///
    /// The `intermediate_value_fn` takes (step, node_idx, continuation_value) and returns
    /// the adjusted value (e.g., max(continuation, exercise_value) for Bermudan options).
    ///
    /// # Errors
    ///
    /// Returns a validation error when `terminal_values.len()` does not match
    /// the number of nodes at the final step (`num_nodes(steps)`).
    pub fn backward_induction<F>(
        &self,
        terminal_values: &[f64],
        intermediate_value_fn: F,
    ) -> finstack_core::Result<f64>
    where
        F: Fn(usize, usize, f64) -> f64,
    {
        let n = self.num_steps();

        let expected = self.num_nodes(n);
        if terminal_values.len() != expected {
            return Err(finstack_core::Error::Validation(format!(
                "HullWhiteTree::backward_induction: terminal_values has {} entries but the \
                 final step has {} nodes",
                terminal_values.len(),
                expected
            )));
        }

        // Reuse two scratch buffers across steps to avoid per-step
        // allocation. Both are sized to the widest level: the buffers swap
        // every step, and interior levels can be wider than the terminal
        // level when mean reversion contracts the lattice near maturity.
        let max_nodes = self.widths.iter().copied().max().unwrap_or(1);
        let mut values = vec![0.0; max_nodes];
        values[..terminal_values.len()].copy_from_slice(terminal_values);
        let mut scratch = vec![0.0; max_nodes];
        let comp = self.config.compounding;

        // Backward induction
        for step in (0..n).rev() {
            let num_nodes = self.num_nodes(step);
            let dt_i = self.dts[step];
            let alpha_step = self.alpha.get(step).copied().unwrap_or(0.0);
            let j_min = self.j_mins[step];
            let dx = self.dxs[step];
            let step_branches = &self.branches[step];

            for (idx, scratch_j) in scratch.iter_mut().enumerate().take(num_nodes) {
                let j = j_min + idx as i32;
                let r_j = j as f64 * dx + alpha_step;
                let (center, (p_up, p_mid, p_down)) = step_branches[idx];

                let expected_value = p_up * values[center + 1]
                    + p_mid * values[center]
                    + p_down * values[center - 1];
                let discounted = expected_value * comp.df(r_j, dt_i);

                *scratch_j = intermediate_value_fn(step, idx, discounted);
            }

            // Swap buffers instead of allocating new Vec
            std::mem::swap(&mut values, &mut scratch);
        }

        // Return value at root node
        Ok(values.first().copied().unwrap_or(0.0))
    }

    /// Map a time (year fraction) to the nearest tree step.
    ///
    /// Mandatory times supplied to [`HullWhiteTree::calibrate_with_times`]
    /// land exactly on grid points, so for those the nearest step is exact.
    /// Use [`HullWhiteTree::step_at_time`] when an exact match is required.
    pub fn time_to_step(&self, time: f64) -> usize {
        if time <= 0.0 {
            return 0;
        }
        let n = self.num_steps();
        if time >= self.time_grid[n] {
            return n;
        }
        // First grid point strictly greater than `time`.
        let upper = self.time_grid.partition_point(|&t| t <= time);
        debug_assert!(upper >= 1 && upper <= n);
        let lower = upper - 1;
        if (time - self.time_grid[lower]).abs() <= (self.time_grid[upper] - time).abs() {
            lower
        } else {
            upper
        }
    }

    /// Map a time (year fraction) to the tree step whose grid point matches
    /// it exactly (within a small absolute tolerance).
    ///
    /// # Errors
    ///
    /// Returns a validation error when no grid point matches — i.e. the time
    /// was not supplied as a mandatory time during calibration and does not
    /// happen to coincide with the grid.
    pub fn step_at_time(&self, time: f64) -> Result<usize> {
        let step = self.time_to_step(time);
        let grid_time = self.time_at_step(step);
        if (grid_time - time).abs() <= GRID_TIME_TOLERANCE.max(1e-12 * time.abs()) {
            Ok(step)
        } else {
            Err(Error::Validation(format!(
                "Hull-White tree has no grid point at t={time} (nearest is t={grid_time}); \
                 pass it as a mandatory time to calibrate_with_times"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;
    use time::Month;

    fn test_discount_curve() -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(
                finstack_core::dates::Date::from_calendar_date(2025, Month::January, 1)
                    .expect("Valid date"),
            )
            .knots([
                (0.0, 1.0),
                (0.5, 0.985),
                (1.0, 0.97),
                (2.0, 0.94),
                (5.0, 0.85),
                (10.0, 0.70),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Valid curve")
    }

    #[test]
    fn test_tree_calibration() {
        // Use 200 steps for production-quality < 1 bp calibration
        let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
        let curve = test_discount_curve();

        let tree =
            HullWhiteTree::calibrate(config, &curve, 5.0).expect("Calibration should succeed");

        // Tree should have correct number of steps
        assert_eq!(tree.num_steps(), 200);

        // State prices should sum to discount factors. With the drift α
        // stored at the correct index (B4) the forward induction reprices
        // the curve essentially exactly; 0.1 bp leaves only solver/float
        // headroom. The pre-fix off-by-one bug produced ~0.8 bp errors on
        // this mild curve, which the old 1 bp tolerance could not see.
        for step in [20, 50, 100, 150, 200] {
            let t = tree.time_at_step(step);
            let target_df = curve.df(t);
            let sum_q: f64 = (0..tree.num_nodes(step))
                .map(|j| tree.state_price(step, j))
                .sum();

            let error = (sum_q - target_df).abs();
            let error_bps = (error / target_df) * 10000.0;

            assert!(
                error_bps < 0.1,
                "State price calibration error {:.6} ({:.4} bps) at step {} (t={:.2})",
                error,
                error_bps,
                step,
                t
            );
        }
    }

    #[test]
    fn test_tree_calibration_steep_curve() {
        // Regression: on a steep curve the off-by-one drift placement
        // produced 20-40 bp of ZCB bias. The calibrated tree must reprice
        // the input curve at every pillar to well under 0.1 bp.
        let steep_curve = DiscountCurve::builder("USD-OIS-STEEP")
            .base_date(
                finstack_core::dates::Date::from_calendar_date(2025, Month::January, 1)
                    .expect("Valid date"),
            )
            // Zero rates rising ~1% -> ~6%: df(t) = exp(-z(t)*t)
            .knots([
                (0.0, 1.0),
                (0.5, (-0.012_f64 * 0.5).exp()),
                (1.0, (-0.018_f64 * 1.0).exp()),
                (2.0, (-0.030_f64 * 2.0).exp()),
                (5.0, (-0.048_f64 * 5.0).exp()),
                (10.0, (-0.060_f64 * 10.0).exp()),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Valid curve");

        let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
        let tree = HullWhiteTree::calibrate(config, &steep_curve, 10.0)
            .expect("Calibration should succeed");

        for step in [10, 20, 40, 100, 160, 200] {
            let t = tree.time_at_step(step);
            let target_df = steep_curve.df(t);
            let sum_q: f64 = (0..tree.num_nodes(step))
                .map(|j| tree.state_price(step, j))
                .sum();

            let error_bps = ((sum_q - target_df) / target_df).abs() * 10000.0;
            assert!(
                error_bps < 0.1,
                "Steep-curve calibration error {:.4} bps at step {} (t={:.2})",
                error_bps,
                step,
                t
            );
        }

        // Backward induction of a unit payoff must also recover the curve.
        let final_step = tree.num_steps();
        let terminal = vec![1.0; tree.num_nodes(final_step)];
        let value = tree
            .backward_induction(&terminal, |_, _, cont| cont)
            .expect("terminal values sized to final step");
        let target_df = steep_curve.df(10.0);
        let error_bps = ((value - target_df) / target_df).abs() * 10000.0;
        assert!(
            error_bps < 0.1,
            "Steep-curve backward induction error {:.4} bps (value={:.8}, df={:.8})",
            error_bps,
            value,
            target_df
        );
    }

    #[test]
    fn test_bond_price_at_maturity() {
        // Use 200 steps for production-quality < 1 bp accuracy
        let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
        let curve = test_discount_curve();

        let tree =
            HullWhiteTree::calibrate(config, &curve, 2.0).expect("Calibration should succeed");
        let final_step = tree.num_steps();
        let mid_node = tree.num_nodes(final_step) / 2;

        // Bond price at maturity should be exactly 1.0
        // Production standard: < 1 bp error
        let bp = tree.bond_price(final_step, mid_node, 2.0, &curve);
        let error_bps = (bp - 1.0).abs() * 10000.0;
        assert!(
            error_bps < 1.0,
            "Bond price at maturity should be 1.0, got {:.8} (error: {:.4} bps)",
            bp,
            error_bps
        );
    }

    #[test]
    fn probabilities_fail_fast_when_invalid() {
        let err =
            HullWhiteTree::compute_probabilities(0.03, 0.25, 0.0, 1, 1).expect_err("should fail");
        match err {
            finstack_core::Error::Validation(msg) => {
                assert!(msg.contains("finite"), "message={msg}");
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn interior_probabilities_match_mean_reversion_moments() {
        let kappa = 0.03;
        let dt = 0.05;
        let dx = 0.01 * (3.0_f64 * dt).sqrt();
        let j = 12;
        let j_max = 50;

        let (p_up, p_mid, p_down) =
            HullWhiteTree::compute_probabilities(kappa, dt, dx, j, j_max).expect("probabilities");

        let m = kappa * dt;
        let expected_mean_offset = -(j as f64) * m;
        let expected_second_moment = 1.0 / 3.0 + expected_mean_offset * expected_mean_offset;

        let actual_mean_offset = p_up - p_down;
        let actual_second_moment = p_up + p_down;

        assert!(
            (actual_mean_offset - expected_mean_offset).abs() < 1e-12,
            "mean offset should pull positive j back toward zero: actual={actual_mean_offset}, expected={expected_mean_offset}"
        );
        assert!(
            (actual_second_moment - expected_second_moment).abs() < 1e-12,
            "second moment mismatch: actual={actual_second_moment}, expected={expected_second_moment}"
        );
        assert!((p_up + p_mid + p_down - 1.0).abs() < 1e-12);
    }

    #[test]
    fn boundary_probabilities_match_shifted_branch_moments() {
        let kappa = 0.15;
        let dt = 0.05;
        let dx = 0.01 * (3.0_f64 * dt).sqrt();
        let j_max = 25;

        let (p_upper_0, p_upper_m1, p_upper_m2) =
            HullWhiteTree::compute_probabilities(kappa, dt, dx, j_max as i32, j_max)
                .expect("upper boundary probabilities");
        let m = kappa * dt;
        let upper_expected_mean = -(j_max as f64) * m;
        let upper_expected_second = 1.0 / 3.0 + upper_expected_mean * upper_expected_mean;
        let upper_mean = -p_upper_m1 - 2.0 * p_upper_m2;
        let upper_second = p_upper_m1 + 4.0 * p_upper_m2;

        assert!((upper_mean - upper_expected_mean).abs() < 1e-12);
        assert!((upper_second - upper_expected_second).abs() < 1e-12);
        assert!((p_upper_0 + p_upper_m1 + p_upper_m2 - 1.0).abs() < 1e-12);

        let (p_lower_p2, p_lower_p1, p_lower_0) =
            HullWhiteTree::compute_probabilities(kappa, dt, dx, -(j_max as i32), j_max)
                .expect("lower boundary probabilities");
        let lower_expected_mean = (j_max as f64) * m;
        let lower_expected_second = 1.0 / 3.0 + lower_expected_mean * lower_expected_mean;
        let lower_mean = 2.0 * p_lower_p2 + p_lower_p1;
        let lower_second = 4.0 * p_lower_p2 + p_lower_p1;

        assert!((lower_mean - lower_expected_mean).abs() < 1e-12);
        assert!((lower_second - lower_expected_second).abs() < 1e-12);
        assert!((p_lower_p2 + p_lower_p1 + p_lower_0 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_backward_induction_zero_payoff() {
        let config = HullWhiteTreeConfig::new(0.03, 0.01, 10);
        let curve = test_discount_curve();

        let tree =
            HullWhiteTree::calibrate(config, &curve, 1.0).expect("Calibration should succeed");

        // Zero payoff should give zero value
        let terminal = vec![0.0; tree.num_nodes(10)];
        let value = tree
            .backward_induction(&terminal, |_, _, cont| cont)
            .expect("terminal values sized to final step");

        assert!(value.abs() < 1e-10, "Zero payoff should give zero value");
    }

    #[test]
    fn test_backward_induction_unit_payoff() {
        // Use 200 steps for production-quality < 1 bp backward induction
        let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
        let curve = test_discount_curve();

        let tree =
            HullWhiteTree::calibrate(config, &curve, 1.0).expect("Calibration should succeed");
        let final_step = tree.num_steps();

        // Unit payoff at all nodes should give approximately the discount factor
        let terminal = vec![1.0; tree.num_nodes(final_step)];
        let value = tree
            .backward_induction(&terminal, |_, _, cont| cont)
            .expect("terminal values sized to final step");

        let target_df = curve.df(1.0);
        let error = (value - target_df).abs();
        let error_bps = (error / target_df) * 10000.0;

        // Production standard: pricing error < 1 basis point
        assert!(
            error_bps < 1.0,
            "Unit payoff value {:.8} should match df {:.8} (error: {:.4} bps)",
            value,
            target_df,
            error_bps
        );
    }

    #[test]
    fn backward_induction_rejects_mis_sized_terminal_values() {
        let config = HullWhiteTreeConfig::new(0.03, 0.01, 50);
        let curve = test_discount_curve();
        let tree =
            HullWhiteTree::calibrate(config, &curve, 1.0).expect("Calibration should succeed");

        let expected = tree.num_nodes(tree.num_steps());
        for bad_len in [0, expected - 1, expected + 1] {
            let terminal = vec![1.0; bad_len];
            let err = tree
                .backward_induction(&terminal, |_, _, cont| cont)
                .expect_err("mis-sized terminal values must be rejected");
            assert!(
                err.to_string().contains("terminal_values"),
                "error should name terminal_values: {err}"
            );
        }
    }

    #[test]
    fn calibrate_rejects_non_positive_time_to_maturity() {
        // `calibrate` previously never validated `time_to_maturity`: a value
        // of `0` or negative produced `dt <= 0`, and the failure only
        // surfaced much later inside `compute_probabilities` with the
        // confusing message "probabilities require finite, positive inputs".
        // It must instead fail up front with a clear maturity error.
        let curve = test_discount_curve();
        for &bad_t in &[0.0_f64, -1.0, -5.0] {
            let config = HullWhiteTreeConfig::new(0.03, 0.01, 100);
            let err = HullWhiteTree::calibrate(config, &curve, bad_t)
                .expect_err("non-positive time_to_maturity must be rejected");
            match err {
                finstack_core::Error::Validation(msg) => {
                    let lower = msg.to_lowercase();
                    assert!(
                        lower.contains("maturity") || lower.contains("time"),
                        "error must clearly name the maturity input, got: {msg}"
                    );
                    assert!(
                        !lower.contains("probabilit"),
                        "error must be the up-front maturity check, not the \
                         downstream probability check: {msg}"
                    );
                }
                other => panic!("expected a validation error, got {other:?}"),
            }
        }
    }

    #[test]
    fn calibrate_accepts_small_positive_time_to_maturity() {
        // A genuinely small-but-positive horizon must still calibrate.
        let curve = test_discount_curve();
        let config = HullWhiteTreeConfig::new(0.03, 0.01, 10);
        let tree = HullWhiteTree::calibrate(config, &curve, 0.25)
            .expect("small positive maturity should calibrate");
        assert_eq!(tree.num_steps(), 10);
    }

    #[test]
    fn computed_probabilities_sum_to_one_in_release_builds() {
        // The probability-sum invariant must hold even when `debug_assert!`
        // is compiled out (release builds). Sweep interior and boundary
        // nodes across a range of mean-reversion regimes.
        for &(kappa, dt) in &[(0.03_f64, 0.05_f64), (0.15, 0.05), (0.50, 0.02)] {
            let dx = 0.01 * (3.0 * dt).sqrt();
            let j_max = (0.184 / (kappa * dt)).ceil() as usize;
            for j in -(j_max as i32)..=(j_max as i32) {
                let (p_up, p_mid, p_down) =
                    HullWhiteTree::compute_probabilities(kappa, dt, dx, j, j_max)
                        .expect("probabilities should be valid");
                let sum = p_up + p_mid + p_down;
                assert!(
                    (sum - 1.0).abs() < 1e-12,
                    "probabilities must sum to 1 (kappa={kappa}, j={j}): sum={sum}"
                );
            }
        }
    }

    #[test]
    fn test_high_kappa_boundary_nodes_hit() {
        // High mean reversion must contain the lattice width: branching
        // centers on the conditional mean x·(1 − κ·dt), so once |j|·κ·dt
        // exceeds 0.5 the central child rounds inward and growth stops at
        // roughly j ≈ 0.5/(κ·dt). With κ=0.15, steps=100, T=5 (dt=0.05),
        // that bound is j ≈ 67 (width ≈ 135) versus the uncontained 201.
        let config = HullWhiteTreeConfig::new(0.15, 0.01, 100);
        let curve = test_discount_curve();

        let tree =
            HullWhiteTree::calibrate(config, &curve, 5.0).expect("Calibration should succeed");

        // Verify mean reversion actually limits the width well below the
        // uncontained 2·steps + 1 node count.
        let final_width = tree.num_nodes(tree.num_steps());
        assert!(
            final_width < 150,
            "final width {} should be mean-reversion-contained (< 150)",
            final_width
        );

        // State prices should still approximately match discount factors
        // Relaxed tolerance to 5bp for boundary-heavy scenarios
        for step in [25, 50, 75, 100] {
            let t = tree.time_at_step(step);
            let target_df = curve.df(t);
            let sum_q: f64 = (0..tree.num_nodes(step))
                .map(|j| tree.state_price(step, j))
                .sum();

            let error_bps = ((sum_q - target_df) / target_df).abs() * 10000.0;
            assert!(
                error_bps < 5.0,
                "Boundary-heavy calibration error {:.2} bps at step {} (t={:.2})",
                error_bps,
                step,
                t
            );
        }

        // Unit payoff backward induction should still approximately recover df
        let final_step = tree.num_steps();
        let terminal = vec![1.0; tree.num_nodes(final_step)];
        let value = tree
            .backward_induction(&terminal, |_, _, cont| cont)
            .expect("terminal values sized to final step");
        let target_df = curve.df(5.0);
        let error_bps = ((value - target_df) / target_df).abs() * 10000.0;
        assert!(
            error_bps < 5.0,
            "Backward induction error {:.2} bps with boundary nodes (value={:.8}, df={:.8})",
            error_bps,
            value,
            target_df
        );
    }

    #[test]
    fn calibrate_with_times_places_mandatory_dates_on_grid() {
        let curve = test_discount_curve();
        let config = HullWhiteTreeConfig::new(0.03, 0.01, 100);
        // Irregular exercise dates that a uniform 100-step grid over 5y
        // (dt = 0.05) cannot represent exactly.
        let mandatory = [0.7123, 1.234, 2.5, 3.99];
        let tree = HullWhiteTree::calibrate_with_times(config, &curve, 5.0, &mandatory)
            .expect("Calibration should succeed");

        for &t in &mandatory {
            let step = tree
                .step_at_time(t)
                .expect("mandatory time must land exactly on a grid point");
            assert!(
                (tree.time_at_step(step) - t).abs() <= 1e-9,
                "grid point {} should equal mandatory time {}",
                tree.time_at_step(step),
                t
            );
        }

        // Total step count stays close to the target.
        let n = tree.num_steps();
        assert!(
            (95..=110).contains(&n),
            "step count {} should stay near the 100-step target",
            n
        );

        // An off-grid time still maps to the nearest step but is rejected by
        // the exact lookup.
        assert!(tree.step_at_time(0.7).is_err());
    }

    #[test]
    fn calibrate_with_times_empty_matches_uniform_calibrate() {
        let curve = test_discount_curve();
        let config = HullWhiteTreeConfig::new(0.03, 0.01, 100);
        let uniform = HullWhiteTree::calibrate(config.clone(), &curve, 5.0).expect("uniform");
        let with_empty =
            HullWhiteTree::calibrate_with_times(config, &curve, 5.0, &[]).expect("empty mandatory");

        assert_eq!(uniform.num_steps(), with_empty.num_steps());
        for step in 0..=uniform.num_steps() {
            assert_eq!(uniform.num_nodes(step), with_empty.num_nodes(step));
            assert!((uniform.time_at_step(step) - with_empty.time_at_step(step)).abs() < 1e-15);
        }

        let final_step = uniform.num_steps();
        let terminal = vec![1.0; uniform.num_nodes(final_step)];
        let v_uniform = uniform
            .backward_induction(&terminal, |_, _, cont| cont)
            .expect("uniform induction");
        let v_empty = with_empty
            .backward_induction(&terminal, |_, _, cont| cont)
            .expect("empty-mandatory induction");
        assert!((v_uniform - v_empty).abs() < 1e-15);
    }

    #[test]
    fn steep_curve_recalibration_with_mandatory_pillars_stays_tight() {
        // Per-step dt regression: a non-uniform grid through every curve
        // pillar must still reprice the steep input curve to < 0.1 bp at
        // every pillar, both via state prices and backward induction.
        let steep_curve = DiscountCurve::builder("USD-OIS-STEEP")
            .base_date(
                finstack_core::dates::Date::from_calendar_date(2025, Month::January, 1)
                    .expect("Valid date"),
            )
            .knots([
                (0.0, 1.0),
                (0.5, (-0.012_f64 * 0.5).exp()),
                (1.0, (-0.018_f64 * 1.0).exp()),
                (2.0, (-0.030_f64 * 2.0).exp()),
                (5.0, (-0.048_f64 * 5.0).exp()),
                (10.0, (-0.060_f64 * 10.0).exp()),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("Valid curve");

        let config = HullWhiteTreeConfig::new(0.03, 0.01, 200);
        let pillars = [0.5, 1.0, 2.0, 5.0];
        let tree = HullWhiteTree::calibrate_with_times(config, &steep_curve, 10.0, &pillars)
            .expect("Calibration should succeed");

        for &t in &pillars {
            let step = tree.step_at_time(t).expect("pillar on grid");
            let target_df = steep_curve.df(t);
            let sum_q: f64 = (0..tree.num_nodes(step))
                .map(|j| tree.state_price(step, j))
                .sum();
            let error_bps = ((sum_q - target_df) / target_df).abs() * 10000.0;
            assert!(
                error_bps < 0.1,
                "Steep-curve per-step-dt calibration error {:.4} bps at pillar t={:.2}",
                error_bps,
                t
            );
        }

        let final_step = tree.num_steps();
        let terminal = vec![1.0; tree.num_nodes(final_step)];
        let value = tree
            .backward_induction(&terminal, |_, _, cont| cont)
            .expect("terminal values sized to final step");
        let target_df = steep_curve.df(10.0);
        let error_bps = ((value - target_df) / target_df).abs() * 10000.0;
        assert!(
            error_bps < 0.1,
            "Steep-curve per-step-dt backward induction error {:.4} bps",
            error_bps
        );
    }
}
