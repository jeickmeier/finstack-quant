//! Two-factor binomial tree: short rate + credit hazard (intensity).
//!
//! Models the joint evolution of the risk-free short rate and the credit hazard
//! rate using correlated binomial moves. Both factors are **calibrated** to their
//! respective market curves (discount curve for rates, hazard curve for credit)
//! via independent Arrow-Debreu forward induction, analogous to Ho-Lee calibration.
//!
//! # Lattice convention
//!
//! Each factor evolves on an **additive normal** binomial lattice: from a node
//! with value `x`, the up move is `x + σ√Δt` and the down move is `x - σ√Δt`,
//! where `σ` is the factor's annualized **normal** volatility. The node spacing
//! `σ√Δt` is therefore in **absolute rate units** (rate per year), not log-rate
//! units. The lattice recombines: row `k` is uniformly spaced by `2σ√Δt`.
//!
//! # Mean reversion — accuracy limits
//!
//! When a factor's mean-reversion speed `κ` is non-zero, the one-step transition
//! probability is moment-matched to a mean-reverting drift. The drift
//! `μ = −κ·(x − x_ref)` and the lattice spacing `σ√Δt` are **both in absolute
//! rate units**, so the moment-matched up-probability is dimensionally coherent:
//!
//! ```text
//! p_up = ½ + μ·√Δt / (2σ) = ½ − κ·(x − x_ref)·√Δt / (2σ)
//! ```
//!
//! Here `x_ref` is the factor's calibrated initial instantaneous rate `x₀`
//! (the level the input curve implies at `t = 0`). Because **calibration and
//! pricing use the identical per-node probability**, the forward Arrow-Debreu
//! recursion and the backward induction remain exact duals and the tree
//! **reprices the input curves exactly for any κ** — the Ho-Lee theta absorbs
//! the first-moment bias entirely.
//!
//! **However**, an additive binomial lattice has only ONE free parameter (`p`),
//! so it can match either the conditional mean **or** the conditional variance,
//! not both. This implementation matches the mean. The conditional variance of
//! one step is `σ²Δt · 4p(1−p)`, which collapses as `p` moves away from ½.
//! Consequence: **option-value accuracy degrades as κ grows**. Concretely, a
//! node near the reversion reference at `κ = 0.15` retains roughly 80 % of the
//! intended conditional variance; by `κ = 0.3` only ~55 % remains. This tree
//! prices callable bonds and term loans, so option-value accuracy matters.
//!
//! For accurate mean-reverting optionality, use [`HullWhiteTree`] (the
//! Hull-White trinomial tree in the same module), which does not have this
//! variance-collapse limitation.
//!
//! This implementation enforces `κ ≤ 0.15` for each factor via a
//! [`Error::Validation`] returned from `calibrate()`. See
//! [`KAPPA_MAX`] for the threshold and its justification.
//!
//! (The earlier implementation calibrated with `p = ½` but priced with a
//! mean-reversion-dependent probability, so the tree no longer repriced the
//! discount curve; it also mixed a *log-rate* drift with the *absolute-rate*
//! lattice spacing, which was dimensionally incoherent.)
//!
//! # Calibration
//!
//! `calibrate()` must be called before `price()`. The calibration ensures:
//! - Tree-implied zero-coupon bond prices match the discount curve at every step
//! - Tree-implied survival probabilities match the hazard curve at every step
//!
//! # OAS
//!
//! Option-adjusted spread is read from `initial_vars["oas"]` (basis points) and
//! applied as a parallel shift to calibrated short rates during backward induction.
//! This matches the `ShortRateTree` convention.
//!
//! [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree

use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::HazardCurve;
use finstack_core::market_data::traits::Discounting;
use finstack_core::{Error, Result};

use super::state_keys;
use super::tree_framework::{NodeState, StateVariables, TreeModel, TreeValuator};

/// Maximum allowed mean-reversion speed (κ) for either factor.
///
/// Above this threshold the probability shift `p = ½ + μ√Δt/(2σ)` pushes `p`
/// far enough from ½ that the conditional variance `σ²Δt·4p(1−p)` becomes
/// materially understated relative to the target Hull-White variance. At
/// `κ = 0.15` the worst-case unclamped node sitting 2σ√Δt above the reversion
/// reference retains ≈ 80 % of the intended conditional variance — a tolerable
/// discretisation error given the other approximations in a binomial tree. By
/// `κ = 0.20` that figure drops to ≈ 65 %, and by `κ = 0.30` to ≈ 55 %;
/// option-value errors become material for callable bond / term-loan pricing.
///
/// Callers needing accurate mean-reverting optionality above this threshold
/// should use [`HullWhiteTree`], which uses a trinomial branching scheme that
/// preserves the conditional variance exactly.
///
/// [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree
pub const KAPPA_MAX: f64 = 0.15;

/// Configuration for rates + credit two-factor tree.
///
/// Mean-reversion speeds (`rate_mean_reversion` and `hazard_mean_reversion`)
/// are validated by [`RatesCreditTree::calibrate`] against [`KAPPA_MAX`].
/// Values above that threshold return a [`Error::Validation`] — use
/// [`HullWhiteTree`] instead for strong mean reversion.
///
/// [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree
#[derive(Debug, Clone)]
pub struct RatesCreditConfig {
    /// Number of time steps
    pub steps: usize,
    /// Short-rate volatility (annualized, normal / Ho-Lee convention)
    pub rate_vol: f64,
    /// Credit hazard volatility (annualized, normal convention)
    pub hazard_vol: f64,
    /// Instantaneous correlation between rate and hazard shocks
    pub correlation: f64,
    /// Mean reversion speed for the short rate (`κ_r`, annualized, `0.0` = no
    /// reversion). Must be ≤ [`KAPPA_MAX`]; the rate factor reverts toward the
    /// discount-curve-implied `t = 0` instantaneous rate.
    pub rate_mean_reversion: f64,
    /// Mean reversion speed for the hazard rate (`κ_h`, annualized, `0.0` = no
    /// reversion). Must be ≤ [`KAPPA_MAX`]; the hazard factor reverts toward
    /// the hazard-curve-implied `t = 0` instantaneous hazard.
    pub hazard_mean_reversion: f64,
}

impl Default for RatesCreditConfig {
    fn default() -> Self {
        Self {
            steps: 100,
            rate_vol: 0.01,
            hazard_vol: 0.20,
            correlation: 0.0,
            rate_mean_reversion: 0.0,
            hazard_mean_reversion: 0.0,
        }
    }
}

/// Two-factor correlated binomial tree (short rate + hazard rate).
///
/// Both factors are calibrated to market curves via `calibrate()`. Calling
/// `price()` without prior calibration returns an error.
#[derive(Debug, Clone)]
pub struct RatesCreditTree {
    /// Rates-credit tree configuration
    pub config: RatesCreditConfig,
    /// Calibrated short rates: `rates[step][node_i]`.
    /// Populated by `calibrate()`.
    calibrated_rates: Vec<Vec<f64>>,
    /// Calibrated hazard rates: `hazards[step][node_j]`.
    /// Populated by `calibrate()`.
    calibrated_hazards: Vec<Vec<f64>>,
    /// Recovery rate from the hazard curve (populated by `calibrate()`).
    recovery_rate: f64,
    /// Mean-reversion reference level for the rate factor (the calibrated
    /// `t = 0` instantaneous rate `r₀`). Populated by `calibrate()`. Pricing
    /// reverts the rate factor toward this level, matching the level used
    /// during calibration so the tree reprices the discount curve.
    rate_ref: f64,
    /// Mean-reversion reference level for the hazard factor (the calibrated
    /// `t = 0` instantaneous hazard `h₀`). Populated by `calibrate()`.
    hazard_ref: f64,
}

impl RatesCreditTree {
    /// Create a new rates-credit tree with the given configuration.
    ///
    /// `calibrate()` must be called before `price()`.
    pub fn new(config: RatesCreditConfig) -> Self {
        Self {
            config,
            calibrated_rates: Vec::new(),
            calibrated_hazards: Vec::new(),
            recovery_rate: 0.0,
            rate_ref: 0.0,
            hazard_ref: 0.0,
        }
    }

    /// Calibrate both factors to market curves using Arrow-Debreu forward induction.
    ///
    /// - **Rate factor**: calibrated to the discount curve (Ho-Lee style theta adjustment)
    /// - **Hazard factor**: calibrated to the hazard curve's survival probabilities
    ///
    /// After calibration, `price()` uses the stored per-node rates and hazards.
    ///
    /// # Arguments
    ///
    /// * `disc` - Discount curve for risk-free rate calibration
    /// * `hazard` - Hazard curve for credit intensity calibration
    /// * `time_to_maturity` - Total time horizon in years
    pub fn calibrate(
        &mut self,
        disc: &dyn Discounting,
        hazard: &HazardCurve,
        time_to_maturity: f64,
    ) -> Result<()> {
        let steps = self.config.steps;
        if steps == 0 || time_to_maturity <= 0.0 {
            return Err(Error::internal(
                "rates-credit tree calibration requires positive steps and time_to_maturity",
            ));
        }

        // Guard: mean-reversion speeds above KAPPA_MAX cause material
        // conditional-variance collapse on the fixed-geometry binomial lattice.
        // Discount-curve repricing is exact for any κ, but option values
        // (callable bonds, term loans) degrade as κ grows — negligible for
        // κ ≲ 0.10, material by κ ≈ 0.20+. Use HullWhiteTree for κ > KAPPA_MAX.
        let kappa_r = self.config.rate_mean_reversion;
        if kappa_r > KAPPA_MAX {
            return Err(Error::Validation(format!(
                "rate_mean_reversion = {kappa_r:.4} exceeds the binomial-lattice limit \
                 (KAPPA_MAX = {KAPPA_MAX}). At this speed the conditional variance of \
                 the rate factor collapses to a fraction of its intended value, which \
                 degrades option-value accuracy for callable bonds and term loans. \
                 Use HullWhiteTree for mean reversion above this threshold."
            )));
        }
        let kappa_h = self.config.hazard_mean_reversion;
        if kappa_h > KAPPA_MAX {
            return Err(Error::Validation(format!(
                "hazard_mean_reversion = {kappa_h:.4} exceeds the binomial-lattice limit \
                 (KAPPA_MAX = {KAPPA_MAX}). At this speed the conditional variance of \
                 the hazard factor collapses to a fraction of its intended value, which \
                 degrades option-value accuracy for callable bonds and term loans. \
                 Use HullWhiteTree for mean reversion above this threshold."
            )));
        }

        let dt = time_to_maturity / steps as f64;

        // Store recovery rate from hazard curve.
        self.recovery_rate = hazard.recovery_rate();

        // Reference (reversion) levels: the t=0 instantaneous rate / hazard
        // implied by each input curve. Both factors revert toward these levels
        // during pricing; calibration uses the same levels so the tree reprices
        // the input curves with mean reversion active.
        self.rate_ref = Self::initial_instantaneous(|t| disc.df(t), dt);
        self.hazard_ref = Self::initial_instantaneous(|t| hazard.sp(t), dt);

        // --- Rate factor calibration (Ho-Lee style) ---
        let rate_vol = self.config.rate_vol;
        let rate_kappa = self.config.rate_mean_reversion;
        let rate_ref = self.rate_ref;
        self.calibrated_rates = self.calibrate_factor_ho_lee(
            steps,
            dt,
            rate_vol,
            |t| disc.df(t),
            time_to_maturity,
            |r| Self::mean_reverting_up_prob(r, rate_ref, rate_kappa, rate_vol, dt),
        )?;

        // --- Hazard factor calibration (same Ho-Lee approach targeting survival) ---
        let hazard_vol = self.config.hazard_vol;
        let hazard_kappa = self.config.hazard_mean_reversion;
        let hazard_ref = self.hazard_ref;
        self.calibrated_hazards = self.calibrate_factor_ho_lee(
            steps,
            dt,
            hazard_vol,
            |t| hazard.sp(t),
            time_to_maturity,
            |h| Self::mean_reverting_up_prob(h, hazard_ref, hazard_kappa, hazard_vol, dt),
        )?;

        Ok(())
    }

    /// Return the recovery rate from the most recent `calibrate()` call.
    pub fn recovery_rate(&self) -> f64 {
        self.recovery_rate
    }

    /// Calibrated short rate at node `(step, node_i)`.
    ///
    /// Returns an error if the tree has not been calibrated or the indices are
    /// out of bounds. Node `0` is the lowest rate, node `step` the highest
    /// (additive Ho-Lee lattice).
    pub fn rate_at_node(&self, step: usize, node: usize) -> Result<f64> {
        self.calibrated_rates
            .get(step)
            .and_then(|row| row.get(node))
            .copied()
            .ok_or_else(|| {
                Error::internal(format!(
                    "rates-credit tree rate node out of bounds: step={step}, node={node}"
                ))
            })
    }

    /// Calibrated hazard rate at node `(step, node_j)`.
    ///
    /// Returns an error if the tree has not been calibrated or the indices are
    /// out of bounds.
    pub fn hazard_at_node(&self, step: usize, node: usize) -> Result<f64> {
        self.calibrated_hazards
            .get(step)
            .and_then(|row| row.get(node))
            .copied()
            .ok_or_else(|| {
                Error::internal(format!(
                    "rates-credit tree hazard node out of bounds: step={step}, node={node}"
                ))
            })
    }

    /// Initial instantaneous rate implied by a target curve: `−ln(target(Δt))/Δt`.
    ///
    /// Falls back to `0.03` if the target is non-positive (degenerate curve).
    fn initial_instantaneous(target_fn: impl Fn(f64) -> f64, dt: f64) -> f64 {
        let target_dt = target_fn(dt);
        if target_dt > 0.0 && dt > 0.0 {
            -target_dt.ln() / dt
        } else {
            0.03
        }
    }

    /// Moment-matched up-probability for a mean-reverting factor on the additive
    /// normal lattice.
    ///
    /// # Units
    ///
    /// Every quantity below is in **absolute rate units** (rate per year), so
    /// the formula is dimensionally coherent:
    /// - `x`, `x_ref`: rate level (per year)
    /// - drift `μ = −κ·(x − x_ref)`: rate per year
    /// - lattice up/down step `σ√Δt`: rate (per year, integrated over `√Δt`)
    ///
    /// The standard moment-matched binomial up-probability for a drift `μ` on a
    /// lattice with step `±σ√Δt` is
    ///
    /// ```text
    /// p_up = ½ + (μ·Δt) / (2·σ√Δt) = ½ + μ·√Δt / (2σ)
    /// ```
    ///
    /// which matches the conditional **mean** `E[Δx] = μ·Δt`. With `κ = 0`
    /// this reduces to `p_up = ½` (plain Ho-Lee).
    ///
    /// # Mean vs variance trade-off
    ///
    /// An additive binomial lattice has a single free parameter (`p`). This
    /// function uses it to match the conditional mean, leaving the conditional
    /// variance `σ²Δt · 4p(1−p)` understated once `p ≠ ½`. Discount-curve
    /// repricing is **exact for any κ** because calibration and pricing apply
    /// the **identical** clamped probability, so the forward Arrow-Debreu
    /// recursion and backward induction remain exact duals. However,
    /// **option-value accuracy degrades as κ grows** — a limitation that
    /// matters when pricing callable bonds and term loans. For κ beyond
    /// [`KAPPA_MAX`] the degradation becomes material; use [`HullWhiteTree`]
    /// instead.
    ///
    /// [`HullWhiteTree`]: super::hull_white_tree::HullWhiteTree
    #[inline]
    fn mean_reverting_up_prob(x: f64, x_ref: f64, kappa: f64, sigma: f64, dt: f64) -> f64 {
        if kappa <= 0.0 || dt <= 0.0 {
            return 0.5;
        }
        let sigma = sigma.max(1e-12);
        // Drift in absolute rate units (rate / year).
        let drift = -kappa * (x - x_ref);
        (0.5 + drift * dt.sqrt() / (2.0 * sigma)).clamp(0.0, 1.0)
    }

    /// Ho-Lee style calibration for a single factor.
    ///
    /// Builds a 1D binomial lattice with additive normal volatility (`sigma * sqrt(dt)`)
    /// and solves for a theta (drift) at each step so that the lattice-implied
    /// "discount factor" matches a target curve.
    ///
    /// - For the rate factor: `target_fn(t) = disc.df(t)` (discount factor)
    /// - For the hazard factor: `target_fn(t) = hazard.sp(t)` (survival probability)
    ///
    /// Both share the same mathematical structure: the product `exp(-x * dt)` over
    /// path nodes must match the target curve value at each maturity.
    ///
    /// `up_prob_fn` returns the up-transition probability for a node given its
    /// (post-theta) rate. It **must** be the identical function the pricing pass
    /// uses for backward induction: the forward Arrow-Debreu recursion below and
    /// the backward induction in `price()` are exact duals only when they share
    /// the same per-node probability, which is what makes the calibrated tree
    /// reprice the input curve when mean reversion is active.
    fn calibrate_factor_ho_lee(
        &self,
        steps: usize,
        dt: f64,
        sigma: f64,
        target_fn: impl Fn(f64) -> f64,
        time_to_maturity: f64,
        up_prob_fn: impl Fn(f64) -> f64,
    ) -> Result<Vec<Vec<f64>>> {
        let mut rates = vec![Vec::new(); steps + 1];

        // Initial rate: r0 = -ln(target(dt)) / dt
        let r0 = Self::initial_instantaneous(&target_fn, dt);
        rates[0] = vec![r0];

        // Arrow-Debreu state prices
        let mut state_prices = vec![1.0];

        let sqrt_dt = dt.sqrt();

        for step in 0..steps {
            let next_nodes = step + 2;
            let mut next_rates_base = vec![0.0; next_nodes];
            let mut next_state_prices = vec![0.0; next_nodes];

            // Propagate state prices and compute base rates (without theta).
            //
            // The transition probability uses the SAME mean-reversion-aware
            // function the pricing pass applies, evaluated on the (calibrated)
            // current-row rate. Forward induction here is the exact dual of the
            // backward induction in `price()` because both use this probability.
            for (i, &current_rate) in rates[step].iter().enumerate() {
                let q = state_prices[i];
                let df_i = (-current_rate * dt).exp();
                let p_up = up_prob_fn(current_rate);

                // Up move (to node i+1)
                let r_up_base = current_rate + sigma * sqrt_dt;
                if i + 1 < next_nodes {
                    next_rates_base[i + 1] = r_up_base;
                    next_state_prices[i + 1] += q * df_i * p_up;
                }

                // Down move (to node i)
                let r_down_base = current_rate - sigma * sqrt_dt;
                if i < next_nodes {
                    next_rates_base[i] = r_down_base;
                    next_state_prices[i] += q * df_i * (1.0 - p_up);
                }
            }

            // Solve for theta: target = exp(-theta*dt) * sum_j(Q_next[j] * exp(-r_base[j]*dt))
            let next_next_time = (step + 2) as f64 * dt;
            let theta = if next_next_time <= time_to_maturity + dt * 0.5 {
                let p_target = target_fn(next_next_time);
                let mut p_model_base = 0.0;
                for (j, &q_next) in next_state_prices.iter().enumerate() {
                    p_model_base += q_next * (-next_rates_base[j] * dt).exp();
                }
                if p_model_base > 0.0 && p_target > 0.0 {
                    -(p_target / p_model_base).ln() / dt
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // Apply theta to get final calibrated rates
            let mut next_rates = vec![0.0; next_nodes];
            for j in 0..next_nodes {
                next_rates[j] = next_rates_base[j] + theta;
            }
            rates[step + 1] = next_rates;
            state_prices = next_state_prices;
        }

        Ok(rates)
    }

    #[inline]
    fn joint_probabilities(&self, p_r: f64, p_h: f64) -> (f64, f64, f64, f64) {
        // Correlated Bernoulli coupling
        let var_r = p_r * (1.0 - p_r);
        let var_h = p_h * (1.0 - p_h);
        let cov = self.config.correlation * (var_r * var_h).sqrt();

        let mut p_uu = (p_r * p_h + cov).clamp(0.0, 1.0);
        let mut p_ud = (p_r * (1.0 - p_h) - cov).clamp(0.0, 1.0);
        let mut p_du = ((1.0 - p_r) * p_h - cov).clamp(0.0, 1.0);
        let mut p_dd = ((1.0 - p_r) * (1.0 - p_h) + cov).clamp(0.0, 1.0);

        let sum = p_uu + p_ud + p_du + p_dd;
        if sum > 0.0 {
            p_uu /= sum;
            p_ud /= sum;
            p_du /= sum;
            p_dd /= sum;
        } else {
            // fallback to independent
            p_uu = p_r * p_h;
            p_ud = p_r * (1.0 - p_h);
            p_du = (1.0 - p_r) * p_h;
            p_dd = (1.0 - p_r) * (1.0 - p_h);
        }
        (p_uu, p_ud, p_du, p_dd)
    }
}

impl TreeModel for RatesCreditTree {
    fn price<V: TreeValuator>(
        &self,
        initial_vars: StateVariables,
        time_to_maturity: f64,
        market_context: &MarketContext,
        valuator: &V,
    ) -> Result<f64> {
        if self.calibrated_rates.is_empty() || self.calibrated_hazards.is_empty() {
            return Err(Error::internal(
                "rates-credit tree must be calibrated before pricing",
            ));
        }
        if self.config.steps == 0 || time_to_maturity <= 0.0 {
            return Err(Error::internal(
                "rates-credit tree pricing requires positive steps and time_to_maturity",
            ));
        }

        let steps = self.config.steps;
        let dt = time_to_maturity / steps as f64;

        // OAS from initial variables (bp units, same convention as ShortRateTree)
        let oas_decimal = initial_vars.get("oas").copied().unwrap_or(0.0) / 10_000.0;

        // Pre-allocate double buffers for backward induction (zero allocations in loop)
        let max_nodes = steps + 1;
        let mut curr_values: Vec<Vec<f64>> = vec![vec![0.0; max_nodes]; max_nodes];
        let mut next_values: Vec<Vec<f64>> = vec![vec![0.0; max_nodes]; max_nodes];
        let mut vars = initial_vars.clone();

        // Initialize terminal values
        #[allow(clippy::needless_range_loop)]
        for i in 0..=steps {
            let r_t = self.calibrated_rates[steps][i];
            #[allow(clippy::needless_range_loop)]
            for j in 0..=steps {
                let h_t = self.calibrated_hazards[steps][j];

                vars.insert(state_keys::INTEREST_RATE, r_t.max(1e-8));
                vars.insert(state_keys::HAZARD_RATE, h_t.max(0.0));
                vars.insert("step", steps as f64);
                vars.insert("node_i", i as f64);
                vars.insert("node_j", j as f64);
                vars.insert("time", time_to_maturity);

                let state = NodeState::new(steps, time_to_maturity, &vars, market_context);
                curr_values[i][j] = valuator.value_at_maturity(&state)?;
            }
        }

        // Backward induction with double-buffering
        for k in (0..steps).rev() {
            for i in 0..=k {
                let r_t = self.calibrated_rates[k][i];

                // Rate transition probability with mean reversion. This is the
                // SAME function used during calibration; using an identical
                // per-node probability is what makes the tree reprice the
                // discount curve when mean reversion is non-zero.
                let p_r = Self::mean_reverting_up_prob(
                    r_t,
                    self.rate_ref,
                    self.config.rate_mean_reversion,
                    self.config.rate_vol,
                    dt,
                );

                for j in 0..=k {
                    let h_t = self.calibrated_hazards[k][j];

                    // Hazard transition probability with mean reversion (same
                    // function and reference level used during calibration).
                    let p_h = Self::mean_reverting_up_prob(
                        h_t,
                        self.hazard_ref,
                        self.config.hazard_mean_reversion,
                        self.config.hazard_vol,
                        dt,
                    );

                    // Joint probabilities
                    let (p_uu, p_ud, p_du, p_dd) = self.joint_probabilities(p_r, p_h);

                    // Continuation from four children at step k+1
                    let v_uu = curr_values[i + 1][j + 1];
                    let v_ud = curr_values[i + 1][j];
                    let v_du = curr_values[i][j + 1];
                    let v_dd = curr_values[i][j];

                    // Risk-free discounting with calibrated rate + OAS.
                    //
                    // The rate is NOT floored here: `calibrate_factor_ho_lee`
                    // discounts with the raw (un-floored) calibrated rate, so
                    // backward induction must do the same or the tree will not
                    // reprice the discount curve once a wide lattice produces
                    // negative node rates. The `1e-8` floor is still applied
                    // below to the INTEREST_RATE / HAZARD_RATE *state
                    // variables*, which shields valuators that cannot accept
                    // non-positive rates.
                    let df = (-(r_t + oas_decimal) * dt).exp();
                    let cont = df * (p_uu * v_uu + p_ud * v_ud + p_du * v_du + p_dd * v_dd);

                    vars.insert(state_keys::INTEREST_RATE, r_t.max(1e-8));
                    vars.insert(state_keys::HAZARD_RATE, h_t.max(0.0));
                    vars.insert(state_keys::DF, df);
                    vars.insert("step", k as f64);
                    vars.insert("node_i", i as f64);
                    vars.insert("node_j", j as f64);
                    vars.insert("time", k as f64 * dt);

                    let state = NodeState::new(k, k as f64 * dt, &vars, market_context);
                    next_values[i][j] = valuator.value_at_node(&state, cont, dt)?;
                }
            }
            // Swap buffers (O(1) pointer swap, no data copy)
            std::mem::swap(&mut curr_values, &mut next_values);
        }

        Ok(curr_values[0][0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::math::interp::InterpStyle;

    struct DummyValuator;

    impl TreeValuator for DummyValuator {
        fn value_at_maturity(&self, _state: &NodeState) -> Result<f64> {
            Ok(1.0)
        }
        fn value_at_node(
            &self,
            _state: &NodeState,
            continuation_value: f64,
            _dt: f64,
        ) -> Result<f64> {
            Ok(continuation_value)
        }
    }

    fn test_base_date() -> finstack_core::dates::Date {
        finstack_core::dates::Date::from_calendar_date(2025, time::Month::January, 1)
            .expect("valid date")
    }

    fn sloped_discount_curve() -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(test_base_date())
            .knots([
                (0.0, 1.0),
                (1.0, 0.96),
                (2.0, 0.91),
                (3.0, 0.86),
                (5.0, 0.78),
                (10.0, 0.60),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("curve should build")
    }

    fn test_hazard_curve() -> HazardCurve {
        use finstack_core::market_data::term_structures::ParInterp;
        HazardCurve::builder("TEST-HAZ")
            .base_date(test_base_date())
            .recovery_rate(0.4)
            .knots([(0.0, 0.02), (2.0, 0.025), (5.0, 0.03), (10.0, 0.035)])
            .par_interp(ParInterp::Linear)
            .build()
            .expect("hazard curve should build")
    }

    fn near_zero_discount_curve() -> DiscountCurve {
        DiscountCurve::builder("USD-OIS")
            .base_date(test_base_date())
            .knots([
                (0.0, 1.0),
                (1.0, (-0.000001_f64).exp()),
                (2.0, (-0.000002_f64).exp()),
                (5.0, (-0.000005_f64).exp()),
            ])
            .interp(InterpStyle::LogLinear)
            .build()
            .expect("curve should build")
    }

    fn near_zero_hazard_curve() -> HazardCurve {
        use finstack_core::market_data::term_structures::ParInterp;
        HazardCurve::builder("LOW-HAZ")
            .base_date(test_base_date())
            .recovery_rate(0.4)
            .knots([(0.0, 1e-8), (2.0, 1e-8), (5.0, 1e-8)])
            .par_interp(ParInterp::Linear)
            .build()
            .expect("hazard curve should build")
    }

    #[test]
    fn rates_credit_calibrated_prices_positive() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 40,
            ..Default::default()
        });
        tree.calibrate(&disc, &haz, 5.0).expect("calibration");

        let ctx = MarketContext::new();
        let vars = StateVariables::default();
        let val = DummyValuator;
        let price = tree.price(vars, 5.0, &ctx, &val).expect("should succeed");
        assert!(price.is_finite() && price > 0.0);
    }

    #[test]
    fn uncalibrated_tree_returns_error() {
        let tree = RatesCreditTree::new(RatesCreditConfig::default());
        let ctx = MarketContext::new();
        let vars = StateVariables::default();
        let val = DummyValuator;
        let result = tree.price(vars, 1.0, &ctx, &val);
        assert!(result.is_err(), "price() without calibrate() must fail");
    }

    /// Verify that tree-implied ZCB prices at each step match `disc.df(t)` within 1e-6.
    ///
    /// The DummyValuator passes continuation through unchanged and pays 1.0 at
    /// maturity, so tree price = ZCB price ≈ disc.df(T) for any number of steps.
    #[test]
    fn calibration_quality_zcb_repricing() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 60;
        let ttm = 5.0;

        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: 0.01,
            hazard_vol: 0.0, // no hazard vol → pure rate test
            ..Default::default()
        });
        tree.calibrate(&disc, &haz, ttm).expect("calibrate");

        let ctx = MarketContext::new();
        let vars = StateVariables::default();
        let val = DummyValuator;
        let tree_price = tree.price(vars, ttm, &ctx, &val).expect("price");
        let market_df = disc.df(ttm);

        let error_bps = (tree_price - market_df).abs() * 10_000.0;
        assert!(
            error_bps < 1.0, // within 1 bp
            "ZCB repricing error = {:.4} bps (tree={:.8}, market={:.8})",
            error_bps,
            tree_price,
            market_df
        );
    }

    /// Verify that calibrated hazard rates reproduce the hazard curve's survival
    /// probabilities at each step, using Arrow-Debreu forward induction on the
    /// 1D hazard lattice.
    #[test]
    fn calibration_quality_survival_matching() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 50;
        let ttm = 5.0;
        let dt = ttm / steps as f64;

        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps,
            hazard_vol: 0.20,
            ..Default::default()
        });
        tree.calibrate(&disc, &haz, ttm).expect("calibrate");

        // Forward-propagate Arrow-Debreu state prices through the calibrated
        // hazard lattice to compute model survival probability at each step.
        // No floor applied — must exactly mirror the calibration logic.
        let mut state_prices = vec![1.0_f64]; // Q_h[0] = 1.0

        for k in 0..steps {
            let next_nodes = k + 2;
            let mut next_sp = vec![0.0_f64; next_nodes];
            for j in 0..=k {
                let h_j = tree.calibrated_hazards[k][j];
                let surv_df = (-h_j * dt).exp();
                let q = state_prices[j];
                // Up move to j+1, down move to j — p = 0.5 each (no mean reversion)
                if j + 1 < next_nodes {
                    next_sp[j + 1] += q * surv_df * 0.5;
                }
                next_sp[j] += q * surv_df * 0.5;
            }
            state_prices = next_sp;

            // Model survival probability at step k+1 = sum of state prices
            let model_sp: f64 = state_prices.iter().sum();
            let t = (k + 1) as f64 * dt;
            let market_sp = haz.sp(t);

            let error = (model_sp - market_sp).abs();
            assert!(
                error < 1e-6,
                "Survival mismatch at step {} (t={:.3}): model={:.8}, market={:.8}, err={:.2e}",
                k + 1,
                t,
                model_sp,
                market_sp,
                error
            );
        }
    }

    #[test]
    fn near_zero_rates_with_mean_reversion_price_finitely() {
        let disc = near_zero_discount_curve();
        let haz = near_zero_hazard_curve();
        let mut tree = RatesCreditTree::new(RatesCreditConfig {
            steps: 20,
            rate_vol: 0.20,
            hazard_vol: 0.20,
            rate_mean_reversion: 0.001,
            hazard_mean_reversion: 0.001,
            ..Default::default()
        });

        tree.calibrate(&disc, &haz, 2.0).expect("calibration");

        let price = tree
            .price(
                StateVariables::default(),
                2.0,
                &MarketContext::new(),
                &DummyValuator,
            )
            .expect("pricing should succeed");

        assert!(price.is_finite() && price > 0.0, "price={price}");
    }

    /// The tree must reprice the input discount curve **with mean reversion
    /// active**. The `DummyValuator` pays 1.0 at maturity and passes
    /// continuation through unchanged, so the tree price equals the implied
    /// ZCB price, which must match `disc.df(T)`.
    ///
    /// On the parent (`e7dd696da`) this test fails by hundreds of bps: the
    /// calibration assumed `p = 0.5` while pricing used a different
    /// mean-reversion-dependent probability, so the tree no longer repriced
    /// the curve once `rate_mean_reversion != 0`.
    ///
    /// κ values are capped at `KAPPA_MAX` (= 0.15) because above that threshold
    /// `calibrate()` returns a `Validation` error. The fix is still demonstrated
    /// by these values — the parent was off by > 1000 bps even at κ = 0.05.
    #[test]
    fn calibration_reprices_disc_curve_with_rate_mean_reversion() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let ttm = 5.0;

        for &kappa in &[0.05_f64, 0.10, 0.15] {
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps: 60,
                rate_vol: 0.012,
                hazard_vol: 0.0, // isolate the rate factor
                rate_mean_reversion: kappa,
                ..Default::default()
            });
            tree.calibrate(&disc, &haz, ttm).expect("calibrate");

            let ctx = MarketContext::new();
            let price = tree
                .price(StateVariables::default(), ttm, &ctx, &DummyValuator)
                .expect("price");
            let market_df = disc.df(ttm);
            let error_bps = (price - market_df).abs() * 10_000.0;
            assert!(
                error_bps < 1.0,
                "kappa={kappa}: ZCB repricing error {error_bps:.4} bps \
                 (tree={price:.8}, market={market_df:.8})",
            );
        }
    }

    /// The hazard factor must likewise reprice the survival curve when its own
    /// mean reversion is active.
    #[test]
    fn calibration_reprices_survival_with_hazard_mean_reversion() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 60;
        let ttm = 5.0;
        let dt = ttm / steps as f64;

        // κ values capped at KAPPA_MAX (0.15); above that calibrate() returns
        // a Validation error (see `mean_reversion_above_kappa_max_returns_validation_error`).
        for &kappa in &[0.05_f64, 0.10, 0.15] {
            let mut tree = RatesCreditTree::new(RatesCreditConfig {
                steps,
                hazard_vol: 0.20,
                hazard_mean_reversion: kappa,
                ..Default::default()
            });
            tree.calibrate(&disc, &haz, ttm).expect("calibrate");

            // Forward-propagate Arrow-Debreu state prices through the
            // calibrated hazard lattice using the SAME mean-reversion
            // probability the calibration applied. The summed state prices at
            // step k must equal the market survival probability at t_k.
            let mut state_prices = vec![1.0_f64];
            for k in 0..steps {
                let next_nodes = k + 2;
                let mut next_sp = vec![0.0_f64; next_nodes];
                for j in 0..=k {
                    let h_j = tree.hazard_at_node(k, j).expect("hazard node");
                    let surv_df = (-h_j * dt).exp();
                    let q = state_prices[j];
                    let p_up = RatesCreditTree::mean_reverting_up_prob(
                        h_j,
                        tree.hazard_ref,
                        kappa,
                        0.20,
                        dt,
                    );
                    if j + 1 < next_nodes {
                        next_sp[j + 1] += q * surv_df * p_up;
                    }
                    next_sp[j] += q * surv_df * (1.0 - p_up);
                }
                state_prices = next_sp;

                let model_sp: f64 = state_prices.iter().sum();
                let t = (k + 1) as f64 * dt;
                let market_sp = haz.sp(t);
                let error = (model_sp - market_sp).abs();
                assert!(
                    error < 1e-6,
                    "kappa={kappa}: survival mismatch at step {} (t={t:.3}): \
                     model={model_sp:.8}, market={market_sp:.8}, err={error:.2e}",
                    k + 1,
                );
            }
        }
    }

    /// At **zero** mean reversion the two-factor tree's rate factor must
    /// reproduce a standalone Ho-Lee `ShortRateTree` node-for-node: both use
    /// the identical additive lattice (`r ± σ√Δt`, `p = ½`, theta calibration).
    #[test]
    fn rate_factor_matches_short_rate_tree_at_zero_mean_reversion() {
        use super::super::short_rate_tree::{ShortRateTree, ShortRateTreeConfig};
        use finstack_core::types::CurveId;

        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();
        let steps = 50;
        let ttm = 5.0;
        let vol = 0.011;

        let mut two_factor = RatesCreditTree::new(RatesCreditConfig {
            steps,
            rate_vol: vol,
            hazard_vol: 0.0,
            rate_mean_reversion: 0.0,
            ..Default::default()
        });
        two_factor
            .calibrate(&disc, &haz, ttm)
            .expect("calibrate 2F");

        let mut short_rate = ShortRateTree::new(ShortRateTreeConfig::ho_lee(steps, vol));
        short_rate
            .calibrate(&CurveId::new("USD-OIS"), &disc, ttm)
            .expect("calibrate SR");

        // Compare every calibrated rate node. The terminal row (step == steps)
        // is geometry-only and excluded from both trees' discounting, so the
        // comparison covers the discounting rows 0..steps.
        for step in 0..steps {
            for node in 0..=step {
                let r_2f = two_factor.rate_at_node(step, node).expect("2F node");
                let r_sr = short_rate.rate_at_node(step, node).expect("SR node");
                assert!(
                    (r_2f - r_sr).abs() < 1e-10,
                    "rate mismatch at (step={step}, node={node}): \
                     two_factor={r_2f:.12}, short_rate={r_sr:.12}",
                );
            }
        }
    }

    /// Calibration must reject mean-reversion speeds above `KAPPA_MAX` with a
    /// `Validation` error that names the offending value, the threshold, and
    /// the variance-degradation reason. Values at or below the threshold pass.
    #[test]
    fn mean_reversion_above_kappa_max_returns_validation_error() {
        let disc = sloped_discount_curve();
        let haz = test_hazard_curve();

        // Exactly at the threshold: must succeed.
        let mut tree_at_limit = RatesCreditTree::new(RatesCreditConfig {
            steps: 20,
            rate_mean_reversion: KAPPA_MAX,
            ..Default::default()
        });
        tree_at_limit
            .calibrate(&disc, &haz, 5.0)
            .expect("kappa == KAPPA_MAX must succeed");

        // Just above the threshold: must fail with Validation.
        let over_rate = KAPPA_MAX + 0.01;
        let mut tree_over_rate = RatesCreditTree::new(RatesCreditConfig {
            steps: 20,
            rate_mean_reversion: over_rate,
            ..Default::default()
        });
        match tree_over_rate.calibrate(&disc, &haz, 5.0) {
            Err(Error::Validation(msg)) => {
                assert!(
                    msg.contains("rate_mean_reversion"),
                    "error message must name the field; got: {msg}"
                );
                assert!(
                    msg.contains("HullWhiteTree"),
                    "error message must point to HullWhiteTree; got: {msg}"
                );
            }
            other => panic!("expected Validation error for rate κ={over_rate}, got: {other:?}"),
        }

        // Hazard factor guard: just above the threshold.
        let over_hazard = KAPPA_MAX + 0.01;
        let mut tree_over_hazard = RatesCreditTree::new(RatesCreditConfig {
            steps: 20,
            hazard_mean_reversion: over_hazard,
            ..Default::default()
        });
        match tree_over_hazard.calibrate(&disc, &haz, 5.0) {
            Err(Error::Validation(msg)) => {
                assert!(
                    msg.contains("hazard_mean_reversion"),
                    "error message must name the field; got: {msg}"
                );
                assert!(
                    msg.contains("HullWhiteTree"),
                    "error message must point to HullWhiteTree; got: {msg}"
                );
            }
            other => panic!("expected Validation error for hazard κ={over_hazard}, got: {other:?}"),
        }
    }

    /// The moment-matched up-probability is dimensionally coherent and reduces
    /// to `½` at zero mean reversion / at the reference level.
    #[test]
    fn mean_reverting_up_prob_is_coherent() {
        let sigma = 0.01_f64;
        let dt = 0.05_f64;
        let kappa = 0.10_f64;
        let r_ref = 0.03_f64;

        // At the reference level the drift is zero -> p = 1/2.
        let p_at_ref = RatesCreditTree::mean_reverting_up_prob(r_ref, r_ref, kappa, sigma, dt);
        assert!((p_at_ref - 0.5).abs() < 1e-15, "p at ref should be 1/2");

        // Zero mean reversion -> p = 1/2 everywhere.
        let p_no_mr = RatesCreditTree::mean_reverting_up_prob(0.07, r_ref, 0.0, sigma, dt);
        assert!((p_no_mr - 0.5).abs() < 1e-15, "p without MR should be 1/2");

        // Above the reference level the drift is negative -> p < 1/2 (pull
        // down); below -> p > 1/2 (pull up). Symmetric about 1/2.
        let p_high = RatesCreditTree::mean_reverting_up_prob(r_ref + 0.02, r_ref, kappa, sigma, dt);
        let p_low = RatesCreditTree::mean_reverting_up_prob(r_ref - 0.02, r_ref, kappa, sigma, dt);
        assert!(
            p_high < 0.5 && p_low > 0.5,
            "mean reversion must pull toward ref"
        );
        assert!(
            ((p_high - 0.5) + (p_low - 0.5)).abs() < 1e-15,
            "probability must be symmetric about the reference level",
        );

        // Magnitude check: p = 1/2 + mu*sqrt(dt)/(2*sigma), mu = -kappa*(r-r_ref),
        // all in absolute rate units. With the inputs above:
        //   mu = -0.10 * 0.02 = -0.002 (rate/yr)
        //   p  = 0.5 + (-0.002)*sqrt(0.05)/(2*0.01) = 0.5 - 0.0223607...
        let expected = 0.5 + (-kappa * 0.02) * dt.sqrt() / (2.0 * sigma);
        assert!(
            (p_high - expected).abs() < 1e-14,
            "p_high={p_high}, expected={expected}"
        );

        // Always in [0, 1], even for an extreme node.
        let p_extreme =
            RatesCreditTree::mean_reverting_up_prob(r_ref + 100.0, r_ref, kappa, sigma, dt);
        assert!(
            (0.0..=1.0).contains(&p_extreme),
            "probability must stay in [0,1]"
        );
    }
}
