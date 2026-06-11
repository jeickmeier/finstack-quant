//! Merton Monte Carlo engine for PIK bonds with structural credit risk.
//!
//! Orchestrates [`crate::models::credit::MertonModel`],
//! [`crate::models::credit::EndogenousHazardSpec`],
//! [`crate::models::credit::DynamicRecoverySpec`], and
//! [`crate::models::credit::ToggleExerciseModel`] into a
//! unified Monte Carlo simulation for pricing bonds with PIK (payment-in-kind)
//! features.
//!
//! # Algorithm
//!
//! For each Monte Carlo path:
//! 1. Evolve asset value via GBM (or jump-diffusion) time steps.
//! 2. Determine the hazard rate (endogenous or Merton-implied).
//! 3. Check for default via first-passage barrier breach.
//! 4. At coupon dates, apply PIK/cash toggle logic.
//! 5. Compute terminal payment for surviving paths.
//!
//! Aggregate across paths to produce clean price, expected/unexpected loss,
//! expected shortfall, and path statistics.
//!
//! # Runtime Contract
//!
//! This module is part of the standard valuations build and is selected through
//! `ModelKey::MertonMc`.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::pricing::time_basis::{
    bond_cashflow_dfs_on_model_grid, bond_model_maturity_years,
    implied_flat_discount_rate_from_curve,
};
use crate::instruments::fixed_income::bond::types::Bond;
use crate::models::credit::{
    AssetDynamics, BarrierType, CreditState, DynamicRecoverySpec, EndogenousHazardSpec,
    MertonModel, ToggleExerciseModel,
};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::credit::registry::default_market_recovery_rate;
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};
use finstack_core::{InputError, Result};
use indexmap::IndexMap;

// ---------------------------------------------------------------------------
// PIK schedule types
// ---------------------------------------------------------------------------

/// Barrier-crossing detection policy for first-passage default simulation.
///
/// `Discrete` only checks the barrier at grid points (fast but biased for
/// coarse time steps). `BrownianBridge` uses a Brownian-bridge crossing
/// probability between grid points to approximate continuous monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarrierCrossing {
    /// Discrete monitoring: default if `V(t_i) < B(t_i)` at time steps.
    Discrete,
    /// Brownian-bridge correction for continuous monitoring between steps.
    BrownianBridge,
}

/// Which structural parameter to calibrate in the MC engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationParameter {
    /// Calibrate the debt barrier B.
    DebtBarrier,
    /// Calibrate the asset volatility sigma_V.
    AssetVol,
}

/// Calibration settings for MC-to-market matching.
///
/// When set on [`MertonMcConfig::calibration`], the pricer runs a low-path
/// bisection to solve for a structural parameter so that the cash base-case
/// MC price matches the target market quote, then re-prices with full paths.
#[derive(Debug, Clone)]
pub struct MertonMcCalibrationSpec {
    /// Target market quote to match (interpreted at quote/settlement date).
    pub target: crate::instruments::fixed_income::bond::pricing::quote_conversions::BondQuoteInput,
    /// Which structural parameter to solve for.
    pub parameter: CalibrationParameter,
    /// Number of MC paths used during calibration iterations (low paths).
    pub low_paths: usize,
    /// Maximum bisection iterations.
    pub max_iter: usize,
    /// Absolute tolerance on the **PV residual** (currency units at `as_of`).
    pub tolerance_pv: f64,
    /// Search bracket for the calibrated parameter (low, high).
    /// When `None`, auto-brackets based on the calibration parameter type.
    pub bracket: Option<(f64, f64)>,
    /// Optional seed override used for the calibration run.
    pub seed: Option<u64>,
}

impl Default for MertonMcCalibrationSpec {
    fn default() -> Self {
        Self {
            target: crate::instruments::fixed_income::bond::pricing::quote_conversions::BondQuoteInput::ZSpread(0.0),
            parameter: CalibrationParameter::DebtBarrier,
            low_paths: 2_000,
            max_iter: 40,
            tolerance_pv: 1e-4,
            bracket: None,
            seed: None,
        }
    }
}

/// Per-coupon PIK behavior for the MC engine.
///
/// Determines how each coupon payment is handled: paid in cash, accreted
/// to notional (PIK), split between cash and PIK, or decided dynamically
/// by a [`ToggleExerciseModel`].
#[derive(
    Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub enum PikMode {
    /// Coupon paid in cash.
    Cash,
    /// Coupon accreted to notional (payment-in-kind).
    Pik,
    /// Coupon split between cash and PIK.
    Split {
        /// Fraction paid in cash (e.g. 0.5 for 50%).
        cash_fraction: f64,
        /// Fraction accreted to notional.
        pik_fraction: f64,
    },
    /// Deferred to the [`ToggleExerciseModel`] on the config.
    /// Falls back to `Cash` if no toggle model is set.
    Toggle,
}

/// Time-varying PIK schedule for the MC engine.
///
/// Controls per-coupon PIK behavior, either uniformly or as a step
/// function over time.
///
/// # Examples
///
/// ```
/// use finstack_valuations::instruments::fixed_income::bond::pricing::engine::merton_mc::{PikMode, PikSchedule};
///
/// // All coupons PIK
/// let uniform = PikSchedule::Uniform(PikMode::Pik);
///
/// // PIK for first 2 years, then cash
/// let stepped = PikSchedule::Stepped(vec![(0.0, PikMode::Pik), (2.0, PikMode::Cash)]);
///
/// // Toggle for 3 years, then mandatory cash
/// let toggle_window = PikSchedule::Stepped(vec![(0.0, PikMode::Toggle), (3.0, PikMode::Cash)]);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum PikSchedule {
    /// Same mode for all coupon dates.
    Uniform(PikMode),
    /// Step function: each `(t, mode)` entry means `mode` applies from
    /// time `t` onward. Entries must be sorted by time ascending.
    Stepped(Vec<(f64, PikMode)>),
}

impl Default for PikSchedule {
    fn default() -> Self {
        Self::Uniform(PikMode::Cash)
    }
}

impl PikSchedule {
    /// Look up the active [`PikMode`] at time `t`.
    pub fn mode_at(&self, t: f64) -> PikMode {
        match self {
            Self::Uniform(mode) => *mode,
            Self::Stepped(steps) => {
                let mut active = PikMode::Cash;
                for &(step_t, mode) in steps {
                    if t >= step_t {
                        active = mode;
                    } else {
                        break;
                    }
                }
                active
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for Monte Carlo PIK bond pricing.
#[derive(Debug, Clone)]
pub struct MertonMcConfig {
    /// Merton structural credit model.
    pub merton: MertonModel,
    /// PIK schedule controlling per-coupon cash/PIK/toggle behavior.
    pub pik_schedule: PikSchedule,
    /// Optional endogenous (leverage-dependent) hazard rate model.
    pub endogenous_hazard: Option<EndogenousHazardSpec>,
    /// Optional dynamic (notional-dependent) recovery rate model.
    ///
    /// Recovery on default is evaluated pathwise as
    /// `DynamicRecoverySpec::recovery_at_notional(N(τ))`, which hard-clamps
    /// the result to `[0, base_recovery]`. The clamp introduces a small kink
    /// in recovery as a function of the accreted notional; paths far into
    /// the clamped region all contribute the same (floored/capped) recovery.
    /// No smoothed (e.g. logistic) recovery rule is applied.
    pub dynamic_recovery: Option<DynamicRecoverySpec>,
    /// Optional toggle exercise model for PIK/cash coupon decisions.
    /// Active only for coupon dates where [`PikSchedule`] resolves to
    /// [`PikMode::Toggle`].
    pub toggle_model: Option<ToggleExerciseModel>,
    /// Number of Monte Carlo paths.
    pub num_paths: usize,
    /// RNG seed for reproducibility.
    pub seed: u64,
    /// Whether to use antithetic variates for variance reduction.
    pub antithetic: bool,
    /// Time steps per year for the simulation grid.
    pub time_steps_per_year: usize,
    /// Barrier-crossing policy used for `BarrierType::FirstPassage`.
    ///
    /// Default: `BrownianBridge` when the Merton model uses `FirstPassage`,
    /// otherwise `Discrete`.
    pub barrier_crossing: BarrierCrossing,
    /// Default recovery rate used when no `dynamic_recovery` model is set.
    pub default_recovery_rate: f64,
    /// Optional market-calibration specification.
    ///
    /// When set, the pricer first calibrates a structural parameter
    /// (barrier or asset vol) to match a market quote using low-path MC
    /// with common random numbers, then re-prices with full paths.
    pub calibration: Option<MertonMcCalibrationSpec>,
    /// Pre-computed discount factors for term-structure cashflow discounting.
    ///
    /// Each entry is `(year_fraction, discount_factor)`, sorted by time.
    /// When set, cashflows are discounted using log-linear interpolation of
    /// these factors instead of the flat `discount_rate`. The flat rate is
    /// still used for the Merton risk-neutral drift.
    pub cashflow_dfs: Option<Vec<(f64, f64)>>,
}

fn default_market_recovery_rate_for_mc() -> f64 {
    match default_market_recovery_rate() {
        Ok(rate) => rate,
        Err(err) => {
            tracing::warn!(
                error = %err,
                "falling back to 40% market recovery rate for Merton MC defaults"
            );
            0.40
        }
    }
}

impl MertonMcConfig {
    /// Create a new configuration with default simulation parameters.
    ///
    /// Defaults are sourced from the embedded Monte Carlo and credit assumptions registries.
    #[must_use]
    pub fn new(merton: MertonModel) -> Self {
        let defaults = &finstack_monte_carlo::registry::embedded_defaults_or_panic()
            .rust
            .merton_pik_bond;
        let barrier_crossing = match merton.barrier_type() {
            BarrierType::FirstPassage { .. } => BarrierCrossing::BrownianBridge,
            BarrierType::Terminal => BarrierCrossing::Discrete,
        };
        Self {
            merton,
            pik_schedule: PikSchedule::default(),
            endogenous_hazard: None,
            dynamic_recovery: None,
            toggle_model: None,
            num_paths: defaults.num_paths,
            seed: defaults.seed,
            antithetic: defaults.antithetic,
            time_steps_per_year: defaults.time_steps_per_year,
            barrier_crossing,
            default_recovery_rate: default_market_recovery_rate_for_mc(),
            calibration: None,
            cashflow_dfs: None,
        }
    }

    /// Set the PIK schedule.
    #[must_use]
    pub fn pik_schedule(mut self, s: PikSchedule) -> Self {
        self.pik_schedule = s;
        self
    }

    /// Set the number of Monte Carlo paths.
    #[must_use]
    pub fn num_paths(mut self, n: usize) -> Self {
        self.num_paths = n;
        self
    }

    /// Set the RNG seed.
    #[must_use]
    pub fn seed(mut self, s: u64) -> Self {
        self.seed = s;
        self
    }

    /// Enable or disable antithetic variates.
    #[must_use]
    pub fn antithetic(mut self, a: bool) -> Self {
        self.antithetic = a;
        self
    }

    /// Set time steps per year.
    #[must_use]
    pub fn time_steps_per_year(mut self, n: usize) -> Self {
        self.time_steps_per_year = n;
        self
    }

    /// Set barrier-crossing policy for first-passage default monitoring.
    #[must_use]
    pub fn barrier_crossing(mut self, p: BarrierCrossing) -> Self {
        self.barrier_crossing = p;
        self
    }

    /// Set the market-calibration specification.
    #[must_use]
    pub fn calibration(mut self, c: MertonMcCalibrationSpec) -> Self {
        self.calibration = Some(c);
        self
    }

    /// Set pre-computed discount factors for term-structure cashflow discounting.
    #[must_use]
    pub fn cashflow_dfs(mut self, dfs: Vec<(f64, f64)>) -> Self {
        self.cashflow_dfs = Some(dfs);
        self
    }

    /// Set the endogenous hazard model.
    #[must_use]
    pub fn endogenous_hazard(mut self, h: EndogenousHazardSpec) -> Self {
        self.endogenous_hazard = Some(h);
        self
    }

    /// Set the dynamic recovery model.
    #[must_use]
    pub fn dynamic_recovery(mut self, r: DynamicRecoverySpec) -> Self {
        self.dynamic_recovery = Some(r);
        self
    }

    /// Set the toggle exercise model.
    #[must_use]
    pub fn toggle_model(mut self, t: ToggleExerciseModel) -> Self {
        self.toggle_model = Some(t);
        self
    }
}

// ---------------------------------------------------------------------------
// Results
// ---------------------------------------------------------------------------

/// Result from Monte Carlo PIK pricing.
#[derive(Debug, Clone)]
pub struct MertonMcResult {
    /// Clean price as percentage of par.
    pub clean_price_pct: f64,
    /// Dirty price as percentage of par.
    ///
    /// Equal to `clean_price_pct` because the MC engine works in continuous
    /// time and does not model accrued interest separately. Use the pricer's
    /// metrics pipeline for clean/dirty decomposition.
    pub dirty_price_pct: f64,
    /// Expected loss as fraction of PIK-aware risk-free PV.
    ///
    /// Defined as `1 - mean_mc_pv / risk_free_pv` where the risk-free PV
    /// accounts for the PIK schedule (accreted notional in the no-default
    /// scenario). For Toggle periods, the risk-free scenario assumes cash
    /// (zero hazard implies no PIK trigger).
    pub expected_loss: f64,
    /// Unexpected loss (standard deviation of path PVs / notional).
    pub unexpected_loss: f64,
    /// Expected shortfall at the 95% confidence level.
    pub expected_shortfall_95: f64,
    /// Average PIK fraction across all coupon dates and paths.
    pub average_pik_fraction: f64,
    /// Effective spread in basis points implied by MC price vs risk-free.
    pub effective_spread_bp: f64,
    /// Path-level statistics.
    pub path_statistics: PathStatistics,
    /// Number of paths used.
    pub num_paths: usize,
    /// Standard error of the clean price estimate (percentage of par).
    pub standard_error: f64,
}

/// Path-level statistics from the Monte Carlo simulation.
#[derive(Debug, Clone)]
pub struct PathStatistics {
    /// Fraction of paths that defaulted.
    pub default_rate: f64,
    /// Average default time (in years) among defaulted paths.
    pub avg_default_time: f64,
    /// Average terminal notional (reflects PIK accrual).
    pub avg_terminal_notional: f64,
    /// Average recovery percentage among defaulted paths.
    pub avg_recovery_pct: f64,
    /// Fraction of coupon dates where PIK was elected.
    pub pik_exercise_rate: f64,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Bond parameters for the PIK-aware risk-free leg (no-default scenario).
struct RiskFreeLeg {
    notional: f64,
    coupon_rate: f64,
    maturity_years: f64,
    coupon_frequency: usize,
    discount_rate: f64,
}

/// Merton Monte Carlo pricing engine for PIK bonds.
pub struct MertonMcEngine;

impl MertonMcEngine {
    /// Price a bond with structural credit model via Monte Carlo.
    ///
    /// Asset paths evolve under the risk-neutral measure using the Merton
    /// model's `risk_free_rate` as drift (GBM only). Cashflows are discounted
    /// using `config.cashflow_dfs` when available, falling back to
    /// `exp(-discount_rate * t)`. The flat `discount_rate` is always used for
    /// the Merton risk-neutral drift.
    ///
    /// Per-coupon PIK behavior is controlled by `config.pik_schedule`:
    /// - `PikMode::Cash` → coupon paid in cash
    /// - `PikMode::Pik` → coupon accreted to notional
    /// - `PikMode::Split` → coupon split between cash and PIK
    /// - `PikMode::Toggle` → deferred to `config.toggle_model`
    ///
    /// # Arguments
    ///
    /// * `notional` - Bond face value
    /// * `coupon_rate` - Annual coupon rate (e.g., 0.08 for 8%)
    /// * `maturity_years` - Time to maturity in years
    /// * `coupon_frequency` - Coupons per year (e.g., 2 for semi-annual)
    /// * `config` - Monte Carlo configuration (includes PIK schedule)
    /// * `discount_rate` - Discount rate for the Merton drift and flat-rate
    ///   fallback when `cashflow_dfs` is not set
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid (e.g., non-GBM
    /// dynamics, invalid PIK split fractions).
    pub fn price(
        notional: f64,
        coupon_rate: f64,
        maturity_years: f64,
        coupon_frequency: usize,
        config: &MertonMcConfig,
        discount_rate: f64,
    ) -> Result<MertonMcResult> {
        // --- Validation ---
        if !matches!(config.merton.dynamics(), AssetDynamics::GeometricBrownian) {
            return Err(InputError::Invalid.into());
        }
        Self::validate_pik_schedule(&config.pik_schedule)?;
        // A single path has no variance estimate (division by n-1 = 0) and
        // no meaningful MC statistics.
        if config.num_paths < 2 {
            return Err(finstack_core::Error::Validation(format!(
                "Merton MC requires num_paths >= 2 (got {}); a single path has \
                 no variance estimate",
                config.num_paths
            )));
        }

        let num_paths = config.num_paths;
        // Size the grid so it ends EXACTLY at maturity: rounding the step
        // count against a fixed dt = 1/steps_per_year left stub maturities
        // with a horizon ≠ maturity (coupons placed beyond the simulated
        // horizon, first-passage defaults triggering after maturity).
        let total_steps = (maturity_years * config.time_steps_per_year as f64)
            .ceil()
            .max(1.0) as usize;
        let dt = maturity_years / total_steps as f64;
        let sqrt_dt = dt.sqrt();
        let accrual_factor = coupon_rate / coupon_frequency as f64;
        let sigma = config.merton.asset_vol();
        let r = config.merton.risk_free_rate();
        let mu = r - config.merton.payout_rate() - 0.5 * sigma * sigma;

        // Coupon schedule anchored backward from maturity (floor + stub),
        // shared with the risk-free leg.
        let coupon_schedule = Self::coupon_schedule(maturity_years, coupon_frequency);
        let num_coupons = coupon_schedule.len();

        let dfs_ref = config.cashflow_dfs.as_deref();

        // Barrier parameters
        let debt_barrier = config.merton.debt_barrier();
        let (barrier_type, barrier_growth_rate) = match config.merton.barrier_type() {
            BarrierType::FirstPassage {
                barrier_growth_rate,
            } => (
                BarrierType::FirstPassage {
                    barrier_growth_rate: *barrier_growth_rate,
                },
                *barrier_growth_rate,
            ),
            BarrierType::Terminal => (BarrierType::Terminal, 0.0),
        };

        // Determine how many base paths (for antithetic)
        let n_base = if config.antithetic {
            num_paths.div_ceil(2)
        } else {
            num_paths
        };

        let mut path_pvs: Vec<f64> = Vec::with_capacity(num_paths);

        // Accumulators for statistics
        let mut total_defaults: usize = 0;
        let mut total_default_time: f64 = 0.0;
        let mut total_terminal_notional: f64 = 0.0;
        let mut total_recovery_pct: f64 = 0.0;
        let mut total_pik_elections: usize = 0;
        let mut total_coupon_periods: usize = 0;
        let mut surviving_paths: usize = 0;

        // Pre-allocate random draw buffers outside the loop to avoid
        // 3 heap allocations per base path (150K+ allocations for 50K paths).
        let mut normals = vec![0.0f64; total_steps];
        let mut uniforms = vec![0.0f64; total_steps];
        let mut toggle_uniforms = vec![0.0f64; num_coupons];

        // First-passage barriers depend only on the time step (deterministic in
        // `t`), so precompute them once rather than evaluating two `exp()` per step
        // per path. Empty for `Terminal`, which uses the flat `debt_barrier`.
        let first_passage_barriers: Vec<f64> = match barrier_type {
            BarrierType::FirstPassage { .. } => (0..=total_steps)
                .map(|i| debt_barrier * (barrier_growth_rate * (i as f64 * dt)).exp())
                .collect(),
            BarrierType::Terminal => Vec::new(),
        };

        for path_idx in 0..n_base {
            // Per-path RNG for determinism
            let mut rng = Pcg64Rng::new_with_stream(config.seed, path_idx as u64);

            // Fill pre-allocated buffers with random draws so that antithetic
            // pairs share identical randomness (normals are sign-flipped;
            // uniforms are reused).
            for n in normals.iter_mut() {
                *n = rng.normal(0.0, 1.0);
            }
            // Brownian-bridge crossing checks
            for u in uniforms.iter_mut() {
                *u = rng.uniform();
            }
            // Toggle decision draws — one per coupon date, shared across the
            // antithetic pair to preserve variance-reduction symmetry.
            for tu in toggle_uniforms.iter_mut() {
                *tu = rng.uniform();
            }

            // Simulate base path (and optionally antithetic)
            let signs: &[f64] = if config.antithetic && path_pvs.len() + 1 < num_paths {
                &[1.0, -1.0]
            } else {
                &[1.0]
            };

            for &sign in signs {
                let mut v = config.merton.asset_value();
                let mut n_current = notional;
                let mut defaulted = false;
                let mut path_pv = 0.0;
                let mut path_pik_elections: usize = 0;
                let mut path_coupon_periods: usize = 0;
                let mut coupon_idx: usize = 0;

                for (step, &normal_draw) in normals.iter().enumerate().take(total_steps) {
                    let t = (step + 1) as f64 * dt;
                    let z = normal_draw * sign;
                    // For the antithetic path (sign = -1) the trajectory is
                    // sign-flipped, so the crossing probability `p` is
                    // different.  Proper antithetic variance reduction requires
                    // the complementary uniform `1 - u` so that the two path
                    // legs remain negatively correlated and the variance
                    // reduction is not partially defeated by correlated
                    // default decisions.
                    let u = if sign < 0.0 {
                        1.0 - uniforms[step]
                    } else {
                        uniforms[step]
                    };

                    let v_prev = v;
                    let barrier_prev = match barrier_type {
                        BarrierType::FirstPassage { .. } => first_passage_barriers[step],
                        BarrierType::Terminal => debt_barrier,
                    };

                    // 1. Evolve asset value (GBM)
                    v *= (mu * dt + sigma * sqrt_dt * z).exp();

                    // 2. Check default
                    match barrier_type {
                        BarrierType::Terminal => {
                            let is_final_step = step + 1 == total_steps;
                            if is_final_step && v < debt_barrier {
                                let recovery_rate = config
                                    .dynamic_recovery
                                    .as_ref()
                                    .map_or(config.default_recovery_rate, |dr| {
                                        dr.recovery_at_notional(n_current)
                                    });
                                let recovery_cashflow = recovery_rate * n_current;
                                let df = Self::df_at_time(dfs_ref, t, discount_rate);
                                path_pv += recovery_cashflow * df;
                                defaulted = true;
                                total_defaults += 1;
                                total_default_time += t;
                                total_recovery_pct += recovery_rate;
                                break;
                            }
                        }
                        BarrierType::FirstPassage { .. } => {
                            let barrier = first_passage_barriers[step + 1];
                            let crossed = if v < barrier {
                                true
                            } else if matches!(
                                config.barrier_crossing,
                                BarrierCrossing::BrownianBridge
                            ) && sigma > 0.0
                                && dt > 0.0
                            {
                                let barrier_now = barrier;
                                let x0 = (v_prev / barrier_prev).ln();
                                let x1 = (v / barrier_now).ln();
                                if x0 > 0.0 && x1 > 0.0 {
                                    let denom = sigma * sigma * dt;
                                    let p = (-2.0 * x0 * x1 / denom).exp();
                                    u < p
                                } else {
                                    true
                                }
                            } else {
                                false
                            };

                            if crossed {
                                let recovery_rate = config
                                    .dynamic_recovery
                                    .as_ref()
                                    .map_or(config.default_recovery_rate, |dr| {
                                        dr.recovery_at_notional(n_current)
                                    });
                                let recovery_cashflow = recovery_rate * n_current;
                                let df = Self::df_at_time(dfs_ref, t, discount_rate);
                                path_pv += recovery_cashflow * df;
                                defaulted = true;
                                total_defaults += 1;
                                total_default_time += t;
                                total_recovery_pct += recovery_rate;
                                break;
                            }
                        }
                    }

                    // 3. At coupon dates (using pre-computed schedule)
                    while coupon_idx < coupon_schedule.len()
                        && t >= coupon_schedule[coupon_idx].0 - dt * 0.5
                    {
                        let (coupon_t, period_frac) = coupon_schedule[coupon_idx];
                        let coupon_amount = n_current * accrual_factor * period_frac;
                        path_coupon_periods += 1;
                        let df = Self::df_at_time(dfs_ref, coupon_t, discount_rate);

                        match config.pik_schedule.mode_at(coupon_t) {
                            PikMode::Cash => {
                                path_pv += coupon_amount * df;
                            }
                            PikMode::Pik => {
                                n_current += coupon_amount;
                                path_pik_elections += 1;
                            }
                            PikMode::Split {
                                cash_fraction,
                                pik_fraction,
                            } => {
                                path_pv += coupon_amount * cash_fraction * df;
                                n_current += coupon_amount * pik_fraction;
                                if pik_fraction > 0.0 {
                                    path_pik_elections += 1;
                                }
                            }
                            PikMode::Toggle => {
                                if let Some(ref toggle) = config.toggle_model {
                                    let leverage = n_current / v;
                                    let hazard_rate =
                                        config.endogenous_hazard.as_ref().map_or_else(
                                            || {
                                                let pd =
                                                    config.merton.default_probability(coupon_t);
                                                if coupon_t > 0.0 {
                                                    -(1.0 - pd).ln() / coupon_t
                                                } else {
                                                    0.0
                                                }
                                            },
                                            |eh| eh.hazard_at_leverage(leverage),
                                        );
                                    let remaining = maturity_years - coupon_t;
                                    let dd = if sigma > 0.0 && remaining > 0.0 {
                                        let sqrt_remaining = remaining.sqrt();
                                        ((v / n_current).ln()
                                            + (r - config.merton.payout_rate()
                                                - 0.5 * sigma * sigma)
                                                * remaining)
                                            / (sigma * sqrt_remaining)
                                    } else {
                                        0.0
                                    };

                                    let state = CreditState {
                                        hazard_rate,
                                        distance_to_default: Some(dd),
                                        leverage,
                                        accreted_notional: n_current,
                                        coupon_due: coupon_amount,
                                        asset_value: Some(v),
                                    };

                                    let tu =
                                        toggle_uniforms.get(coupon_idx).copied().unwrap_or(0.5);
                                    if toggle.should_pik_with_uniform(&state, tu) {
                                        n_current += coupon_amount;
                                        path_pik_elections += 1;
                                    } else {
                                        path_pv += coupon_amount * df;
                                    }
                                } else {
                                    path_pv += coupon_amount * df;
                                }
                            }
                        }

                        coupon_idx += 1;
                    }
                }

                // 4. Terminal payment (if survived)
                if !defaulted {
                    let df = Self::df_at_time(dfs_ref, maturity_years, discount_rate);
                    path_pv += n_current * df;
                    surviving_paths += 1;
                    total_terminal_notional += n_current;
                }

                total_pik_elections += path_pik_elections;
                total_coupon_periods += path_coupon_periods;

                path_pvs.push(path_pv);
            }
        }

        // Trim to exact num_paths in case antithetic generated extras
        path_pvs.truncate(num_paths);

        // Aggregate statistics
        let actual_paths = path_pvs.len() as f64;
        let mean_pv = path_pvs.iter().sum::<f64>() / actual_paths;
        let clean_price_pct = mean_pv / notional * 100.0;

        // PIK-aware risk-free PV for expected loss calculation
        let risk_free_pv = Self::risk_free_pv_pik_aware(
            notional,
            coupon_rate,
            maturity_years,
            coupon_frequency,
            discount_rate,
            &config.pik_schedule,
            dfs_ref,
        );
        let expected_loss = if risk_free_pv > 0.0 {
            1.0 - mean_pv / risk_free_pv
        } else {
            0.0
        };

        // Effective spread: solve for the constant spread s such that the
        // risk-free PV with every cashflow's discount factor bumped by
        // e^{-s·t} equals mean_pv. Solving on the SAME discount basis as
        // mean_pv (term-structure DFs when set) keeps curve shape out of the
        // spread.
        let effective_spread_bp = Self::solve_effective_spread(
            &RiskFreeLeg {
                notional,
                coupon_rate,
                maturity_years,
                coupon_frequency,
                discount_rate,
            },
            mean_pv,
            &config.pik_schedule,
            dfs_ref,
        );

        // Unexpected loss (std dev of path PVs / notional) — a distribution
        // measure over individual paths, so it always uses the raw PVs.
        let variance = path_pvs
            .iter()
            .map(|&pv| (pv - mean_pv).powi(2))
            .sum::<f64>()
            / (actual_paths - 1.0);
        let std_dev = variance.sqrt();
        let unexpected_loss = std_dev / notional;

        // Standard error of the MEAN estimate. Antithetic legs are
        // negatively correlated by construction, so the i.i.d. samples are
        // the PAIR AVERAGES (adjacent in path order), not the individual
        // legs — treating 2N legs as independent misstates the SE.
        let standard_error = if config.antithetic {
            let pair_means: Vec<f64> = path_pvs
                .chunks(2)
                .map(|pair| pair.iter().sum::<f64>() / pair.len() as f64)
                .collect();
            let m = pair_means.len() as f64;
            let pair_mean = pair_means.iter().sum::<f64>() / m;
            let pair_var = if pair_means.len() > 1 {
                pair_means
                    .iter()
                    .map(|&x| (x - pair_mean).powi(2))
                    .sum::<f64>()
                    / (m - 1.0)
            } else {
                0.0
            };
            (pair_var / m).sqrt() / notional * 100.0
        } else {
            std_dev / actual_paths.sqrt() / notional * 100.0
        };

        // Expected shortfall at 95% (average of worst 5% of paths)
        // Sort in place — mean_pv and variance are already computed above.
        path_pvs.sort_by(|a, b| a.total_cmp(b));
        let cutoff = (0.05 * actual_paths).ceil() as usize;
        let cutoff = cutoff.max(1);
        let es_sum: f64 = path_pvs.iter().take(cutoff).sum();
        let expected_shortfall_95 = es_sum / cutoff as f64 / notional * 100.0;

        // Average PIK fraction
        let average_pik_fraction = if total_coupon_periods > 0 {
            total_pik_elections as f64 / total_coupon_periods as f64
        } else {
            0.0
        };

        // Path statistics
        let default_rate = total_defaults as f64 / actual_paths;
        let avg_default_time = if total_defaults > 0 {
            total_default_time / total_defaults as f64
        } else {
            0.0
        };
        let avg_terminal_notional = if surviving_paths > 0 {
            total_terminal_notional / surviving_paths as f64
        } else {
            notional
        };
        let avg_recovery_pct = if total_defaults > 0 {
            total_recovery_pct / total_defaults as f64
        } else {
            0.0
        };
        let pik_exercise_rate = average_pik_fraction;

        Ok(MertonMcResult {
            clean_price_pct,
            dirty_price_pct: clean_price_pct,
            expected_loss,
            unexpected_loss,
            expected_shortfall_95,
            average_pik_fraction,
            effective_spread_bp,
            path_statistics: PathStatistics {
                default_rate,
                avg_default_time,
                avg_terminal_notional,
                avg_recovery_pct,
                pik_exercise_rate,
            },
            num_paths: path_pvs.len(),
            standard_error,
        })
    }

    // -----------------------------------------------------------------------
    // Helper methods
    // -----------------------------------------------------------------------

    /// Validate PIK split fractions (non-negative, summing to 1) and, for
    /// `Stepped` schedules, that step times are finite and strictly
    /// ascending — `mode_at` does a linear scan that silently returns the
    /// wrong mode for out-of-order entries.
    fn validate_pik_schedule(schedule: &PikSchedule) -> Result<()> {
        let check = |mode: &PikMode| -> Result<()> {
            if let PikMode::Split {
                cash_fraction,
                pik_fraction,
            } = mode
            {
                if *cash_fraction < 0.0
                    || *pik_fraction < 0.0
                    || (*cash_fraction + *pik_fraction - 1.0).abs() > 1e-10
                {
                    return Err(InputError::Invalid.into());
                }
            }
            Ok(())
        };
        match schedule {
            PikSchedule::Uniform(mode) => check(mode)?,
            PikSchedule::Stepped(steps) => {
                let mut prev_t = f64::NEG_INFINITY;
                for &(t, ref mode) in steps {
                    if !t.is_finite() || t <= prev_t {
                        return Err(finstack_core::Error::Validation(format!(
                            "PikSchedule::Stepped times must be finite and strictly \
                             ascending; entry at t = {t} follows t = {prev_t}"
                        )));
                    }
                    prev_t = t;
                    check(mode)?;
                }
            }
        }
        Ok(())
    }

    /// Log-linear interpolation of discount factors with flat-rate fallback.
    fn df_at_time(cashflow_dfs: Option<&[(f64, f64)]>, t: f64, flat_rate: f64) -> f64 {
        let Some(dfs) = cashflow_dfs.filter(|d| !d.is_empty()) else {
            return (-flat_rate * t).exp();
        };
        if t <= 0.0 {
            return 1.0;
        }
        if t <= dfs[0].0 {
            let (t0, df0) = dfs[0];
            if t0 > 0.0 {
                let r0 = -df0.ln() / t0;
                return (-r0 * t).exp();
            }
            return (-flat_rate * t).exp();
        }
        let last = dfs.len() - 1;
        if t >= dfs[last].0 {
            let (tn, dfn) = dfs[last];
            if tn > 0.0 {
                let rn = -dfn.ln() / tn;
                return (-rn * t).exp();
            }
            return (-flat_rate * t).exp();
        }
        let idx = dfs.partition_point(|&(ti, _)| ti < t);
        let (t0, df0) = dfs[idx - 1];
        let (t1, df1) = dfs[idx];
        if (t1 - t0).abs() < 1e-15 {
            return df0;
        }
        let w = (t - t0) / (t1 - t0);
        (df0.ln() * (1.0 - w) + df1.ln() * w).exp()
    }

    /// Coupon times anchored backward from maturity (standard back-generated
    /// schedule), with a short front stub when the maturity is not an integer
    /// number of periods.
    ///
    /// Returns `(time, period_fraction)` pairs where `period_fraction` is the
    /// coupon's accrual as a fraction of a full period (1.0 for regular
    /// coupons, < 1.0 for the stub). Anchoring at maturity keeps every coupon
    /// inside the simulated horizon and gives seasoned bonds the correct
    /// remaining-coupon timing; `round(maturity·freq)` from the valuation
    /// date (the previous behavior) dropped stub coupons and could place the
    /// final coupon beyond the simulated horizon.
    ///
    /// Shared by the MC pricing loop and the risk-free leg so expected loss
    /// is a like-for-like difference.
    fn coupon_schedule(maturity_years: f64, coupon_frequency: usize) -> Vec<(f64, f64)> {
        let period = 1.0 / coupon_frequency as f64;
        // floor + stub, with an FP tolerance so aligned maturities (e.g.
        // exactly 10 semi-annual periods) do not gain a zero-length stub.
        let n = ((maturity_years / period) - 1e-9).ceil().max(1.0) as usize;
        (1..=n)
            .map(|k| {
                let t = maturity_years - (n - k) as f64 * period;
                let t_prev = (t - period).max(0.0);
                let frac = ((t - t_prev) / period).clamp(0.0, 1.0);
                (t, frac)
            })
            .collect()
    }

    /// PIK-aware risk-free PV (no-default scenario).
    ///
    /// For Toggle periods the risk-free scenario assumes cash payment
    /// (zero hazard implies no PIK trigger).
    fn risk_free_pv_pik_aware(
        notional: f64,
        coupon_rate: f64,
        maturity_years: f64,
        coupon_frequency: usize,
        discount_rate: f64,
        pik_schedule: &PikSchedule,
        cashflow_dfs: Option<&[(f64, f64)]>,
    ) -> f64 {
        Self::risk_free_pv_spread(
            &RiskFreeLeg {
                notional,
                coupon_rate,
                maturity_years,
                coupon_frequency,
                discount_rate,
            },
            pik_schedule,
            cashflow_dfs,
            0.0,
        )
    }

    /// PIK-aware risk-free PV with each cashflow's discount factor bumped by
    /// `e^{-spread·t}` on top of the base discount basis (term-structure DFs
    /// when set, flat rate otherwise).
    fn risk_free_pv_spread(
        leg: &RiskFreeLeg,
        pik_schedule: &PikSchedule,
        cashflow_dfs: Option<&[(f64, f64)]>,
        spread: f64,
    ) -> f64 {
        let accrual_factor = leg.coupon_rate / leg.coupon_frequency as f64;
        let mut pv = 0.0;
        let mut n = leg.notional;

        for &(t, period_frac) in &Self::coupon_schedule(leg.maturity_years, leg.coupon_frequency) {
            let df = Self::df_at_time(cashflow_dfs, t, leg.discount_rate) * (-spread * t).exp();
            let coupon = n * accrual_factor * period_frac;

            match pik_schedule.mode_at(t) {
                PikMode::Cash | PikMode::Toggle => {
                    pv += coupon * df;
                }
                PikMode::Pik => {
                    n += coupon;
                }
                PikMode::Split {
                    cash_fraction,
                    pik_fraction,
                } => {
                    pv += coupon * cash_fraction * df;
                    n += coupon * pik_fraction;
                }
            }
        }
        pv += n
            * Self::df_at_time(cashflow_dfs, leg.maturity_years, leg.discount_rate)
            * (-spread * leg.maturity_years).exp();
        pv
    }

    /// Solve for the constant spread `s` (in bp) such that the PIK-aware
    /// risk-free PV with every cashflow's discount factor bumped by
    /// `e^{-s·t}` equals `target_pv`.
    ///
    /// The base discount basis (term-structure `cashflow_dfs` when set) is
    /// the SAME one used to compute `target_pv`, so curve shape cancels out
    /// of the spread instead of leaking into it.
    fn solve_effective_spread(
        leg: &RiskFreeLeg,
        target_pv: f64,
        pik_schedule: &PikSchedule,
        cashflow_dfs: Option<&[(f64, f64)]>,
    ) -> f64 {
        let (notional, maturity_years) = (leg.notional, leg.maturity_years);
        if target_pv <= 0.0 || maturity_years <= 0.0 {
            return 0.0;
        }
        let pv_at = |s: f64| Self::risk_free_pv_spread(leg, pik_schedule, cashflow_dfs, s);
        let rf_pv = pv_at(0.0);
        if target_pv >= rf_pv {
            return 0.0;
        }

        // Newton-Raphson with ZCB-approximation initial guess
        let mut s = (rf_pv / target_pv).ln() / maturity_years;
        let bump = 1e-4;
        for _ in 0..50 {
            let pv = pv_at(s);
            let f = pv - target_pv;
            if f.abs() < 1e-10 * notional {
                break;
            }
            let pv_up = pv_at(s + bump);
            let dpv = (pv_up - pv) / bump;
            if dpv.abs() < 1e-15 {
                break;
            }
            s -= f / dpv;
            if s < 0.0 {
                s = 0.0;
            }
        }
        s * 10_000.0
    }
}

// ---------------------------------------------------------------------------
// Calibration helpers
// ---------------------------------------------------------------------------

pub mod calibration {
    //! Low-path MC calibration loop with common random numbers.

    use super::{
        CalibrationParameter, MertonMcCalibrationSpec, MertonMcConfig, PikMode, PikSchedule,
    };
    use crate::cashflow::builder::specs::CouponType;
    use crate::instruments::fixed_income::bond::pricing::quote_conversions::{
        price_from_ytm, price_from_z_spread, BondQuoteInput,
    };
    use crate::instruments::fixed_income::bond::pricing::settlement::QuoteDateContext;
    use crate::instruments::fixed_income::bond::types::Bond;
    use crate::instruments::fixed_income::bond::CashflowSpec;
    use crate::models::credit::{AssetDynamics, BarrierType, MertonModel};
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::{InputError, Result};

    /// Output from MC calibration.
    #[derive(Debug, Clone)]
    pub struct MertonMcCalibrationOutput {
        /// Merton model with the calibrated parameter.
        pub calibrated_merton: MertonModel,
        /// PV at `as_of` produced by the calibration (low-path).
        pub calibrated_pv: f64,
        /// PV at `as_of` implied by the market quote (target).
        pub target_pv: f64,
        /// PV residual = calibrated_pv - target_pv.
        pub residual_pv: f64,
        /// Number of bisection iterations used.
        pub iterations: usize,
        /// Value of the calibrated parameter (barrier or asset vol).
        pub solved_parameter: f64,
    }

    /// Create a cash-equivalent bond for calibration.
    pub fn cash_equivalent_bond(bond: &Bond) -> Result<Bond> {
        fn cashify_spec(spec: &CashflowSpec) -> Result<CashflowSpec> {
            Ok(match spec {
                CashflowSpec::Fixed(fixed) => {
                    let mut f = fixed.clone();
                    f.coupon_type = CouponType::Cash;
                    CashflowSpec::Fixed(f)
                }
                CashflowSpec::Floating(_) => return Err(InputError::Invalid.into()),
                CashflowSpec::StepUp(s) => {
                    let mut s = s.clone();
                    s.coupon_type = CouponType::Cash;
                    CashflowSpec::StepUp(s)
                }
                CashflowSpec::Amortizing { base, schedule } => CashflowSpec::Amortizing {
                    base: Box::new(cashify_spec(base.as_ref())?),
                    schedule: schedule.clone(),
                },
            })
        }

        let mut b = bond.clone();
        b.cashflow_spec = cashify_spec(&b.cashflow_spec)?;
        Ok(b)
    }

    fn with_parameter(
        base: &MertonModel,
        parameter: CalibrationParameter,
        x: f64,
    ) -> Result<MertonModel> {
        let barrier_type: BarrierType = *base.barrier_type();
        let dynamics: AssetDynamics = *base.dynamics();
        let (asset_value, mut asset_vol, mut debt_barrier) =
            (base.asset_value(), base.asset_vol(), base.debt_barrier());

        match parameter {
            CalibrationParameter::DebtBarrier => debt_barrier = x,
            CalibrationParameter::AssetVol => asset_vol = x,
        }

        MertonModel::new_with_dynamics(
            asset_value,
            asset_vol,
            debt_barrier,
            base.risk_free_rate(),
            base.payout_rate(),
            barrier_type,
            dynamics,
        )
    }

    fn target_pv_from_quote(
        bond: &Bond,
        market: &MarketContext,
        as_of: Date,
        target: &BondQuoteInput,
    ) -> Result<f64> {
        let quote_ctx = QuoteDateContext::new(bond, market, as_of)?;
        let quote_date = quote_ctx.quote_date;

        let dirty_quote_ccy = match *target {
            BondQuoteInput::CleanPricePct(clean_pct) => {
                quote_ctx.dirty_from_clean_pct(clean_pct, bond.notional.amount())
            }
            BondQuoteInput::DirtyPriceCcy(dirty_ccy) => dirty_ccy,
            BondQuoteInput::Ytm(ytm) | BondQuoteInput::Ytw(ytm) => {
                // YTW inversion uses maturity flows (same convention as
                // `Bond::base_value`'s `quoted_ytw` path); for callable bonds,
                // prefer `Oas` for exercise-aware pricing.
                let flows = bond.pricing_dated_cashflows(market, as_of)?;
                price_from_ytm(bond, &flows, quote_date, ytm)?
            }
            // `price_from_z_spread` derives the settlement origin internally,
            // so it takes the valuation `as_of` (not the pre-computed quote_date).
            BondQuoteInput::ZSpread(z) => price_from_z_spread(bond, market, as_of, z)?,
            BondQuoteInput::DiscountMargin(_)
            | BondQuoteInput::Oas(_)
            | BondQuoteInput::AswMarket(_)
            | BondQuoteInput::ISpread(_) => return Err(InputError::Invalid.into()),
        };

        let disc = market.get_discount(&bond.discount_curve_id)?;
        let df_settle = if quote_date > as_of {
            disc.df_between_dates(as_of, quote_date)?
        } else {
            1.0
        };
        Ok(dirty_quote_ccy * df_settle)
    }

    fn mc_cash_pv(
        bond_cash: &Bond,
        as_of: Date,
        discount_rate: f64,
        base_config: &MertonMcConfig,
        low_paths: usize,
        seed_override: Option<u64>,
        merton: MertonModel,
    ) -> Result<f64> {
        let cash_schedule = PikSchedule::Stepped(vec![(0.0, PikMode::Cash)]);

        let mut cfg = base_config.clone();
        cfg.merton = merton;
        cfg.num_paths = low_paths;
        cfg.pik_schedule = cash_schedule;
        cfg.calibration = None;
        if let Some(seed) = seed_override {
            cfg.seed = seed;
        }

        let result = bond_cash.price_merton_mc(&cfg, discount_rate, as_of)?;
        Ok(result.clean_price_pct / 100.0 * bond_cash.notional.amount())
    }

    /// Calibrate a structural parameter to a market quote using the same MC engine.
    ///
    /// Uses bisection with common random numbers (deterministic per-path RNG streams)
    /// by reusing the same seed and simulation settings across iterations.
    pub fn calibrate_parameter_to_market(
        bond: &Bond,
        market: &MarketContext,
        as_of: Date,
        discount_rate: f64,
        base_config: &MertonMcConfig,
        spec: &MertonMcCalibrationSpec,
    ) -> Result<MertonMcCalibrationOutput> {
        let bond_cash = cash_equivalent_bond(bond)?;
        let target_pv = target_pv_from_quote(&bond_cash, market, as_of, &spec.target)?;

        let base_merton = &base_config.merton;
        let asset_value = base_merton.asset_value();
        if asset_value <= 0.0 {
            return Err(InputError::NonPositiveValue.into());
        }

        let (mut lo, mut hi) = spec.bracket.unwrap_or(match spec.parameter {
            CalibrationParameter::DebtBarrier => (0.001 * asset_value, 0.999 * asset_value),
            CalibrationParameter::AssetVol => (0.01, 2.0),
        });
        if !(lo.is_finite() && hi.is_finite() && lo > 0.0 && hi > lo) {
            return Err(InputError::Invalid.into());
        }

        let eval = |x: f64| -> Result<(f64, f64)> {
            let m = with_parameter(base_merton, spec.parameter, x)?;
            let pv = mc_cash_pv(
                &bond_cash,
                as_of,
                discount_rate,
                base_config,
                spec.low_paths.max(2),
                spec.seed,
                m,
            )?;
            Ok((pv, pv - target_pv))
        };

        let (pv_lo, mut f_lo) = eval(lo)?;
        let (pv_hi, f_hi) = eval(hi)?;
        if f_lo == 0.0 {
            return Ok(MertonMcCalibrationOutput {
                calibrated_merton: with_parameter(base_merton, spec.parameter, lo)?,
                calibrated_pv: pv_lo,
                target_pv,
                residual_pv: 0.0,
                iterations: 0,
                solved_parameter: lo,
            });
        }
        if f_hi == 0.0 {
            return Ok(MertonMcCalibrationOutput {
                calibrated_merton: with_parameter(base_merton, spec.parameter, hi)?,
                calibrated_pv: pv_hi,
                target_pv,
                residual_pv: 0.0,
                iterations: 0,
                solved_parameter: hi,
            });
        }

        if f_lo.signum() == f_hi.signum() {
            return Err(InputError::SolverConvergenceFailed {
                iterations: 0,
                residual: f_hi.abs().min(f_lo.abs()),
                last_x: hi,
                reason: format!(
                    "Calibration bracket does not straddle root: f(lo)={f_lo:.6e}, f(hi)={f_hi:.6e}"
                ),
            }
            .into());
        }

        let mut iterations = 0usize;
        let mut mid = 0.5 * (lo + hi);
        let mut pv_mid = 0.0;
        let mut f_mid = 0.0;
        let mut converged = false;

        for i in 0..spec.max_iter.max(1) {
            iterations = i + 1;
            mid = 0.5 * (lo + hi);
            let (pv, f) = eval(mid)?;
            pv_mid = pv;
            f_mid = f;

            if f_mid.abs() <= spec.tolerance_pv.max(0.0) {
                converged = true;
                break;
            }

            if f_lo.signum() == f_mid.signum() {
                lo = mid;
                f_lo = f_mid;
            } else {
                hi = mid;
            }
        }

        if !converged {
            return Err(InputError::SolverConvergenceFailed {
                iterations,
                residual: f_mid.abs(),
                last_x: mid,
                reason: format!(
                    "Merton MC calibration did not meet tolerance_pv = {} after {} \
                     iterations: residual_pv = {f_mid:.6e}",
                    spec.tolerance_pv, iterations
                ),
            }
            .into());
        }

        let calibrated_merton = with_parameter(base_merton, spec.parameter, mid)?;
        Ok(MertonMcCalibrationOutput {
            calibrated_merton,
            calibrated_pv: pv_mid,
            target_pv,
            residual_pv: f_mid,
            iterations,
            solved_parameter: mid,
        })
    }
}

// ---------------------------------------------------------------------------
// Pricer Registry Adapter
// ---------------------------------------------------------------------------

/// Model maturity and flat discount rate for Merton MC, on consistent time bases.
struct MertonMcTimeBasis {
    mat_years: f64,
    discount_rate: f64,
}

fn merton_mc_time_basis(
    bond: &Bond,
    disc: &DiscountCurve,
    as_of: finstack_core::dates::Date,
    ctx: &PricingErrorContext,
) -> std::result::Result<MertonMcTimeBasis, PricingError> {
    let mat_years = bond_model_maturity_years(bond, as_of)
        .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;
    let discount_rate =
        implied_flat_discount_rate_from_curve(disc, as_of, bond.maturity, mat_years)
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;
    Ok(MertonMcTimeBasis {
        mat_years,
        discount_rate,
    })
}

fn populate_cashflow_dfs_if_needed(
    config: &mut MertonMcConfig,
    disc: &DiscountCurve,
    bond: &Bond,
    as_of: finstack_core::dates::Date,
    mat_years: f64,
    ctx: &PricingErrorContext,
) -> std::result::Result<(), PricingError> {
    if config.cashflow_dfs.is_none() && mat_years > 0.0 {
        let dfs = bond_cashflow_dfs_on_model_grid(
            disc,
            as_of,
            bond.maturity,
            mat_years,
            config.time_steps_per_year,
        )
        .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;
        config.cashflow_dfs = Some(dfs);
    }
    Ok(())
}

/// Merton structural Monte Carlo pricer for (PIK) bonds.
///
/// Registered under `PricerKey::new(InstrumentType::Bond, ModelKey::MertonMc)`;
/// see the module-level docs for configuration details and metric outputs.
pub(crate) struct SimpleBondMertonMcPricer;

impl Pricer for SimpleBondMertonMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Bond, ModelKey::MertonMc)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        use finstack_core::money::Money;

        let bond = instrument
            .as_any()
            .downcast_ref::<Bond>()
            .ok_or_else(|| PricingError::type_mismatch(InstrumentType::Bond, instrument.key()))?;

        let ctx = PricingErrorContext::new()
            .instrument_id(bond.id())
            .instrument_type(InstrumentType::Bond)
            .model(ModelKey::MertonMc)
            .curve_id(bond.discount_curve_id.as_str());

        let mc_override = bond
            .pricing_overrides
            .model_config
            .merton_mc_config
            .as_ref()
            .ok_or_else(|| {
                PricingError::invalid_input_with_context(
                    "MertonMc pricer requires merton_mc_config on pricing_overrides",
                    ctx.clone(),
                )
            })?;

        let disc = market
            .get_discount(bond.discount_curve_id.as_str())
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;

        let MertonMcTimeBasis {
            mat_years,
            discount_rate,
        } = merton_mc_time_basis(bond, &disc, as_of, &ctx)?;

        // ---- Calibration pass (opt-in) ---------------------------------
        let mut calibration_measures: IndexMap<crate::metrics::MetricId, f64> = IndexMap::new();
        let mut effective_config = if let Some(ref cal_spec) = mc_override.0.calibration {
            use super::merton_mc::calibration::calibrate_parameter_to_market;
            let cal_output = calibrate_parameter_to_market(
                bond,
                market,
                as_of,
                discount_rate,
                &mc_override.0,
                cal_spec,
            )
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;

            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibrated_debt_barrier"),
                cal_output.calibrated_merton.debt_barrier(),
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibrated_asset_vol"),
                cal_output.calibrated_merton.asset_vol(),
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibration_residual_pv"),
                cal_output.residual_pv,
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibration_iterations"),
                cal_output.iterations as f64,
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibration_target_pv"),
                cal_output.target_pv,
            );
            calibration_measures.insert(
                crate::metrics::MetricId::custom("calibration_solved_parameter"),
                cal_output.solved_parameter,
            );

            let mut cfg = mc_override.0.clone();
            cfg.merton = cal_output.calibrated_merton;
            cfg.calibration = None;
            cfg
        } else {
            mc_override.0.clone()
        };

        // Build term-structure discount factors from the curve for cashflow
        // discounting. The flat `discount_rate` is still used for the Merton
        // risk-neutral drift.
        populate_cashflow_dfs_if_needed(
            &mut effective_config,
            &disc,
            bond,
            as_of,
            mat_years,
            &ctx,
        )?;

        // ---- Full pricing pass -----------------------------------------
        let mc_result = bond
            .price_merton_mc(&effective_config, discount_rate, as_of)
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx))?;

        let mc_clean_pct = mc_result.clean_price_pct;
        let pv_amount = mc_clean_pct / 100.0 * bond.notional.amount();
        let pv = Money::new(pv_amount, bond.notional.currency());

        let mut measures = IndexMap::new();
        measures.insert(
            crate::metrics::MetricId::custom("expected_loss"),
            mc_result.expected_loss,
        );
        measures.insert(
            crate::metrics::MetricId::custom("default_rate"),
            mc_result.path_statistics.default_rate,
        );
        measures.insert(
            crate::metrics::MetricId::custom("avg_terminal_notional"),
            mc_result.path_statistics.avg_terminal_notional,
        );
        measures.insert(
            crate::metrics::MetricId::custom("pik_fraction"),
            mc_result.average_pik_fraction,
        );
        measures.insert(
            crate::metrics::MetricId::custom("mc_stderr"),
            mc_result.standard_error,
        );
        measures.insert(
            crate::metrics::MetricId::custom("unexpected_loss"),
            mc_result.unexpected_loss,
        );
        measures.insert(
            crate::metrics::MetricId::custom("expected_shortfall_95"),
            mc_result.expected_shortfall_95,
        );

        for (k, v) in calibration_measures {
            measures.insert(k, v);
        }

        let result = ValuationResult::stamped(bond.id(), as_of, pv);
        Ok(result.with_measures(measures))
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let bond = instrument
            .as_any()
            .downcast_ref::<Bond>()
            .ok_or_else(|| PricingError::type_mismatch(InstrumentType::Bond, instrument.key()))?;

        let ctx = PricingErrorContext::new()
            .instrument_id(bond.id())
            .instrument_type(InstrumentType::Bond)
            .model(ModelKey::MertonMc)
            .curve_id(bond.discount_curve_id.as_str());

        let mc_override = bond
            .pricing_overrides
            .model_config
            .merton_mc_config
            .as_ref()
            .ok_or_else(|| {
                PricingError::invalid_input_with_context(
                    "MertonMc pricer requires merton_mc_config on pricing_overrides",
                    ctx.clone(),
                )
            })?;

        let disc = market
            .get_discount(bond.discount_curve_id.as_str())
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;

        let MertonMcTimeBasis {
            mat_years,
            discount_rate,
        } = merton_mc_time_basis(bond, &disc, as_of, &ctx)?;

        let mut effective_config = if let Some(ref cal_spec) = mc_override.0.calibration {
            use super::merton_mc::calibration::calibrate_parameter_to_market;
            let cal_output = calibrate_parameter_to_market(
                bond,
                market,
                as_of,
                discount_rate,
                &mc_override.0,
                cal_spec,
            )
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx.clone()))?;

            let mut cfg = mc_override.0.clone();
            cfg.merton = cal_output.calibrated_merton;
            cfg.calibration = None;
            cfg
        } else {
            mc_override.0.clone()
        };

        populate_cashflow_dfs_if_needed(
            &mut effective_config,
            &disc,
            bond,
            as_of,
            mat_years,
            &ctx,
        )?;

        let mc_result = bond
            .price_merton_mc(&effective_config, discount_rate, as_of)
            .map_err(|e| PricingError::model_failure_with_context(e.to_string(), ctx))?;

        Ok(mc_result.clean_price_pct / 100.0 * bond.notional.amount())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::credit::toggle_exercise::ThresholdDirection;
    use crate::models::credit::{
        AssetDynamics, BarrierType, CreditStateVariable, DynamicRecoverySpec, EndogenousHazardSpec,
        MertonModel, ToggleExerciseModel,
    };

    fn test_merton() -> MertonModel {
        MertonModel::new_with_dynamics(
            200.0,
            0.25,
            100.0,
            0.04,
            0.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid merton")
    }

    #[test]
    fn cash_bond_produces_positive_price() {
        let config = MertonMcConfig::new(test_merton()).num_paths(5000).seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            result.clean_price_pct > 50.0 && result.clean_price_pct < 150.0,
            "Price should be reasonable: got {}",
            result.clean_price_pct
        );
    }

    #[test]
    fn pik_bond_produces_positive_price() {
        let config = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(5000)
            .seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            result.clean_price_pct > 50.0 && result.clean_price_pct < 150.0,
            "Price should be reasonable: got {}",
            result.clean_price_pct
        );
    }

    #[test]
    fn endogenous_hazard_lowers_pik_price() {
        let endo = EndogenousHazardSpec::power_law(0.06, 0.5, 2.5).expect("valid");
        let config_no = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(10_000)
            .seed(42);
        let config_yes = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(10_000)
            .seed(42)
            .endogenous_hazard(endo);
        let result_no = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_no, 0.04).expect("ok");
        let result_yes = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_yes, 0.04).expect("ok");
        assert!(
            result_yes.clean_price_pct <= result_no.clean_price_pct,
            "Endogenous hazard should lower or maintain PIK price: no={}, yes={}",
            result_no.clean_price_pct,
            result_yes.clean_price_pct
        );
    }

    #[test]
    fn dynamic_recovery_lowers_pik_price() {
        let dyn_rec = DynamicRecoverySpec::floored_inverse(0.40, 100.0, 0.10).expect("valid");
        let config_no = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(10_000)
            .seed(42);
        let config_yes = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(10_000)
            .seed(42)
            .dynamic_recovery(dyn_rec);
        let result_no = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_no, 0.04).expect("ok");
        let result_yes = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_yes, 0.04).expect("ok");
        assert!(
            result_yes.clean_price_pct <= result_no.clean_price_pct,
            "Dynamic recovery should lower or maintain PIK price: no={}, yes={}",
            result_no.clean_price_pct,
            result_yes.clean_price_pct
        );
    }

    #[test]
    fn toggle_price_between_cash_and_pik() {
        let toggle = ToggleExerciseModel::threshold(
            CreditStateVariable::HazardRate,
            0.10,
            ThresholdDirection::Above,
        );
        let config_cash = MertonMcConfig::new(test_merton())
            .num_paths(10_000)
            .seed(42);
        let config_pik = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(10_000)
            .seed(42);
        let config_toggle = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Toggle))
            .num_paths(10_000)
            .seed(42)
            .toggle_model(toggle);
        let cash = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_cash, 0.04).expect("ok");
        let pik = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_pik, 0.04).expect("ok");
        let toggle_result =
            MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_toggle, 0.04).expect("ok");
        let min_price = pik.clean_price_pct.min(cash.clean_price_pct) - 5.0;
        let max_price = pik.clean_price_pct.max(cash.clean_price_pct) + 5.0;
        assert!(
            toggle_result.clean_price_pct >= min_price
                && toggle_result.clean_price_pct <= max_price,
            "Toggle should be between cash and PIK: cash={}, pik={}, toggle={}",
            cash.clean_price_pct,
            pik.clean_price_pct,
            toggle_result.clean_price_pct
        );
    }

    /// M2.12: the coupon schedule is anchored backward from maturity with a
    /// floor + stub count, so stub maturities keep every coupon inside the
    /// simulated horizon and aligned maturities stay regular.
    #[test]
    fn coupon_schedule_handles_stub_and_aligned_maturities() {
        // 4.6y semi-annual: 10 coupons, first is a 0.1y stub (fraction 0.2),
        // last lands exactly at maturity.
        let sched = MertonMcEngine::coupon_schedule(4.6, 2);
        assert_eq!(sched.len(), 10);
        let (t_first, frac_first) = sched[0];
        assert!((t_first - 0.1).abs() < 1e-9, "stub time: {t_first}");
        assert!(
            (frac_first - 0.2).abs() < 1e-9,
            "stub fraction: {frac_first}"
        );
        let (t_last, frac_last) = sched[sched.len() - 1];
        assert!(
            (t_last - 4.6).abs() < 1e-12,
            "final coupon at maturity: {t_last}"
        );
        assert!((frac_last - 1.0).abs() < 1e-9);

        // Aligned 5.0y semi-annual: 10 full coupons at 0.5, 1.0, …, 5.0.
        let aligned = MertonMcEngine::coupon_schedule(5.0, 2);
        assert_eq!(aligned.len(), 10);
        for (i, &(t, frac)) in aligned.iter().enumerate() {
            assert!((t - 0.5 * (i + 1) as f64).abs() < 1e-9);
            assert!((frac - 1.0).abs() < 1e-9);
        }
    }

    /// M2.12: with default risk switched off (asset value far above the
    /// barrier, negligible vol) the MC price of a stub-maturity bond must
    /// reproduce the risk-free PV — i.e. every coupon in the risk-free leg is
    /// reachable on the simulation grid and the grid ends exactly at
    /// maturity. Before the fix, `round(maturity·freq)` and a fixed
    /// `dt = 1/steps_per_year` let the legs disagree on stub maturities.
    #[test]
    fn stub_maturity_mc_matches_risk_free_pv_without_default_risk() {
        let merton = MertonModel::new_with_dynamics(
            1.0e9, // asset value far above the barrier: no defaults
            1e-8,  // negligible asset vol: deterministic paths
            100.0,
            0.04,
            0.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid merton");
        let config = MertonMcConfig::new(merton).num_paths(64).seed(42);

        for maturity in [4.6, 4.8, 5.0] {
            let result = MertonMcEngine::price(100.0, 0.08, maturity, 2, &config, 0.04)
                .expect("price succeeds");
            assert_eq!(
                result.path_statistics.default_rate, 0.0,
                "no defaults expected at maturity {maturity}"
            );
            // expected_loss = 1 − mean_mc_pv / risk_free_pv: with default
            // risk off, the two legs must agree (shared coupon schedule,
            // grid ending exactly at maturity).
            assert!(
                result.expected_loss.abs() < 1e-6,
                "default-free MC price must equal the risk-free PV at maturity \
                 {maturity}: expected_loss = {}",
                result.expected_loss
            );
        }
    }

    #[test]
    fn mc_is_deterministic_with_seed() {
        let config = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(1000)
            .seed(42);
        let r1 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        let r2 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            (r1.clean_price_pct - r2.clean_price_pct).abs() < 1e-10,
            "Same seed should give same result"
        );
    }

    #[test]
    fn path_statistics_reasonable() {
        let config = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(5000)
            .seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            result.path_statistics.default_rate >= 0.0
                && result.path_statistics.default_rate <= 1.0
        );
        assert!(
            result.path_statistics.avg_terminal_notional >= 100.0,
            "PIK should accrete notional, got {}",
            result.path_statistics.avg_terminal_notional
        );
        assert!(result.standard_error > 0.0);
    }

    // -----------------------------------------------------------------------
    // PikSchedule tests
    // -----------------------------------------------------------------------

    #[test]
    fn pik_schedule_mode_at_uniform() {
        let s = PikSchedule::Uniform(PikMode::Pik);
        assert_eq!(s.mode_at(0.0), PikMode::Pik);
        assert_eq!(s.mode_at(5.0), PikMode::Pik);
    }

    #[test]
    fn pik_schedule_mode_at_stepped() {
        let s = PikSchedule::Stepped(vec![(0.0, PikMode::Pik), (2.0, PikMode::Cash)]);
        assert_eq!(s.mode_at(0.5), PikMode::Pik);
        assert_eq!(s.mode_at(1.9), PikMode::Pik);
        assert_eq!(s.mode_at(2.0), PikMode::Cash);
        assert_eq!(s.mode_at(5.0), PikMode::Cash);
    }

    #[test]
    fn pik_schedule_stepped_toggle_then_cash() {
        let s = PikSchedule::Stepped(vec![(0.0, PikMode::Toggle), (3.0, PikMode::Cash)]);
        assert_eq!(s.mode_at(1.0), PikMode::Toggle);
        assert_eq!(s.mode_at(2.9), PikMode::Toggle);
        assert_eq!(s.mode_at(3.0), PikMode::Cash);
    }

    #[test]
    fn split_schedule_prices_between_cash_and_pik() {
        let config_cash = MertonMcConfig::new(test_merton()).num_paths(5000).seed(42);
        let config_pik = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(5000)
            .seed(42);
        let config_split = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Split {
                cash_fraction: 0.5,
                pik_fraction: 0.5,
            }))
            .num_paths(5000)
            .seed(42);

        let cash = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_cash, 0.04).expect("ok");
        let pik = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_pik, 0.04).expect("ok");
        let split = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_split, 0.04).expect("ok");

        let lo = cash.clean_price_pct.min(pik.clean_price_pct) - 2.0;
        let hi = cash.clean_price_pct.max(pik.clean_price_pct) + 2.0;
        assert!(
            split.clean_price_pct >= lo && split.clean_price_pct <= hi,
            "50/50 split should be between cash ({}) and PIK ({}), got {}",
            cash.clean_price_pct,
            pik.clean_price_pct,
            split.clean_price_pct
        );
    }

    #[test]
    fn stepped_schedule_pik_then_cash() {
        // PIK for first 2 years, then cash for remaining 3 years.
        // Should be between full cash and full PIK.
        let config_cash = MertonMcConfig::new(test_merton()).num_paths(5000).seed(42);
        let config_pik = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Pik))
            .num_paths(5000)
            .seed(42);
        let config_step = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Stepped(vec![
                (0.0, PikMode::Pik),
                (2.0, PikMode::Cash),
            ]))
            .num_paths(5000)
            .seed(42);

        let cash = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_cash, 0.04).expect("ok");
        let pik = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_pik, 0.04).expect("ok");
        let step = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_step, 0.04).expect("ok");

        let lo = cash.clean_price_pct.min(pik.clean_price_pct) - 2.0;
        let hi = cash.clean_price_pct.max(pik.clean_price_pct) + 2.0;
        assert!(
            step.clean_price_pct >= lo && step.clean_price_pct <= hi,
            "Stepped PIK→cash should be between full cash ({}) and full PIK ({}), got {}",
            cash.clean_price_pct,
            pik.clean_price_pct,
            step.clean_price_pct
        );
        assert!(
            step.average_pik_fraction > 0.0 && step.average_pik_fraction < 1.0,
            "Stepped schedule should have partial PIK fraction, got {}",
            step.average_pik_fraction
        );
    }

    #[test]
    fn toggle_window_then_cash() {
        // Toggle for first 3 years, mandatory cash after.
        let toggle = ToggleExerciseModel::threshold(
            CreditStateVariable::HazardRate,
            0.10,
            ThresholdDirection::Above,
        );
        let config = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Stepped(vec![
                (0.0, PikMode::Toggle),
                (3.0, PikMode::Cash),
            ]))
            .toggle_model(toggle)
            .num_paths(5000)
            .seed(42);

        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            result.clean_price_pct > 50.0 && result.clean_price_pct < 150.0,
            "Toggle window price should be reasonable: {}",
            result.clean_price_pct
        );
    }

    #[test]
    fn toggle_without_model_falls_back_to_cash() {
        // PikMode::Toggle but no toggle_model → should behave like cash
        let config_toggle_no_model = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Toggle))
            .num_paths(5000)
            .seed(42);
        let config_cash = MertonMcConfig::new(test_merton()).num_paths(5000).seed(42);

        let toggle_result =
            MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_toggle_no_model, 0.04).expect("ok");
        let cash_result =
            MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_cash, 0.04).expect("ok");

        assert!(
            (toggle_result.clean_price_pct - cash_result.clean_price_pct).abs() < 1e-10,
            "Toggle without model should equal cash: toggle={}, cash={}",
            toggle_result.clean_price_pct,
            cash_result.clean_price_pct,
        );
    }

    #[test]
    fn default_pik_schedule_is_cash() {
        let config = MertonMcConfig::new(test_merton());
        assert!(
            matches!(config.pik_schedule, PikSchedule::Uniform(PikMode::Cash)),
            "Default pik_schedule should be Uniform(Cash)"
        );
    }

    // -----------------------------------------------------------------------
    // Brownian-bridge crossing tests
    // -----------------------------------------------------------------------

    #[test]
    fn brownian_bridge_increases_default_rate() {
        let config_discrete = MertonMcConfig::new(test_merton())
            .num_paths(10_000)
            .seed(42)
            .barrier_crossing(BarrierCrossing::Discrete);
        let config_bridge = MertonMcConfig::new(test_merton())
            .num_paths(10_000)
            .seed(42)
            .barrier_crossing(BarrierCrossing::BrownianBridge);

        let result_discrete =
            MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_discrete, 0.04).expect("ok");
        let result_bridge =
            MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config_bridge, 0.04).expect("ok");

        assert!(
            result_bridge.path_statistics.default_rate >= result_discrete.path_statistics.default_rate,
            "Brownian-bridge should detect at least as many defaults as discrete: bb={}, discrete={}",
            result_bridge.path_statistics.default_rate,
            result_discrete.path_statistics.default_rate
        );
    }

    #[test]
    fn brownian_bridge_is_deterministic() {
        let config = MertonMcConfig::new(test_merton())
            .num_paths(2000)
            .seed(99)
            .barrier_crossing(BarrierCrossing::BrownianBridge);
        let r1 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        let r2 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            (r1.clean_price_pct - r2.clean_price_pct).abs() < 1e-10,
            "Same seed + bridge should give same result"
        );
    }

    #[test]
    fn terminal_barrier_only_defaults_at_maturity() {
        let merton_terminal = MertonModel::new(200.0, 0.25, 100.0, 0.04).expect("valid");
        let config = MertonMcConfig::new(merton_terminal)
            .num_paths(5000)
            .seed(42);
        assert_eq!(config.barrier_crossing, BarrierCrossing::Discrete);

        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        if result.path_statistics.default_rate > 0.0 {
            let expected_default_time = 5.0;
            assert!(
                (result.path_statistics.avg_default_time - expected_default_time).abs() < 0.5,
                "Terminal barrier defaults should only occur near maturity, got avg_default_time={}",
                result.path_statistics.avg_default_time
            );
        }
    }

    #[test]
    fn first_passage_default_config_uses_brownian_bridge() {
        let config = MertonMcConfig::new(test_merton());
        assert_eq!(
            config.barrier_crossing,
            BarrierCrossing::BrownianBridge,
            "FirstPassage should default to BrownianBridge"
        );
    }

    // -----------------------------------------------------------------------
    // Validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn non_gbm_dynamics_rejected() {
        let merton_jd = MertonModel::new_with_dynamics(
            200.0,
            0.25,
            100.0,
            0.04,
            0.0,
            BarrierType::Terminal,
            AssetDynamics::JumpDiffusion {
                jump_intensity: 0.5,
                jump_mean: -0.05,
                jump_vol: 0.10,
            },
        )
        .expect("valid");
        let config = MertonMcConfig::new(merton_jd).num_paths(100).seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
        assert!(result.is_err(), "JumpDiffusion should be rejected");
    }

    #[test]
    fn invalid_split_fractions_rejected() {
        let config = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Split {
                cash_fraction: 0.6,
                pik_fraction: 0.6,
            }))
            .num_paths(100)
            .seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
        assert!(result.is_err(), "Split fractions > 1.0 should be rejected");
    }

    #[test]
    fn negative_split_fractions_rejected() {
        let config = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Split {
                cash_fraction: -0.1,
                pik_fraction: 1.1,
            }))
            .num_paths(100)
            .seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
        assert!(
            result.is_err(),
            "Negative split fractions should be rejected"
        );
    }

    #[test]
    fn unsorted_stepped_schedule_rejected() {
        let config = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Stepped(vec![
                (2.0, PikMode::Cash),
                (0.0, PikMode::Pik),
            ]))
            .num_paths(100)
            .seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
        assert!(
            result.is_err(),
            "Out-of-order Stepped times must be rejected"
        );

        let dup = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Stepped(vec![
                (1.0, PikMode::Pik),
                (1.0, PikMode::Cash),
            ]))
            .num_paths(100)
            .seed(42);
        assert!(
            MertonMcEngine::price(100.0, 0.08, 5.0, 2, &dup, 0.04).is_err(),
            "Duplicate Stepped times must be rejected"
        );
    }

    #[test]
    fn single_path_rejected() {
        let config = MertonMcConfig::new(test_merton()).num_paths(1).seed(42);
        assert!(
            MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).is_err(),
            "num_paths < 2 must be rejected"
        );
    }

    /// Antithetic SE must be computed over pair averages, not individual
    /// legs: pairs are negatively correlated, so the pair-based SE differs
    /// from the naive 2N-independent-legs SE (and is typically smaller).
    #[test]
    fn antithetic_se_uses_pair_averages() {
        let config = MertonMcConfig::new(test_merton())
            .num_paths(10_000)
            .antithetic(true)
            .seed(7);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");

        // Naive SE treating all legs as i.i.d.
        let naive_se = result.unexpected_loss * 100.0 / (result.num_paths as f64).sqrt();
        assert!(
            result.standard_error > 0.0 && (result.standard_error - naive_se).abs() > 1e-12,
            "pair-aware SE ({}) should differ from naive per-leg SE ({naive_se})",
            result.standard_error
        );
    }

    /// The effective spread must be solved on the same discount basis as the
    /// MC PV: with term-structure DFs set, a default-free bond must imply a
    /// ~zero spread even when the curve shape differs from the flat rate.
    #[test]
    fn effective_spread_zero_for_default_free_bond_on_term_structure_basis() {
        let merton = MertonModel::new_with_dynamics(
            1.0e9,
            1e-8,
            100.0,
            0.04,
            0.0,
            BarrierType::FirstPassage {
                barrier_growth_rate: 0.0,
            },
            AssetDynamics::GeometricBrownian,
        )
        .expect("valid merton");
        let steep_dfs: Vec<(f64, f64)> = (1..=60)
            .map(|i| {
                let t = i as f64 / 12.0;
                let r = 0.05 - 0.002 * t;
                (t, (-r * t).exp())
            })
            .collect();
        let config = MertonMcConfig::new(merton)
            .num_paths(64)
            .seed(42)
            .cashflow_dfs(steep_dfs);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            result.effective_spread_bp.abs() < 0.1,
            "default-free bond must imply ~zero spread on the curve basis, got {} bp",
            result.effective_spread_bp
        );
    }

    #[test]
    fn valid_split_fractions_accepted() {
        let config = MertonMcConfig::new(test_merton())
            .pik_schedule(PikSchedule::Uniform(PikMode::Split {
                cash_fraction: 0.3,
                pik_fraction: 0.7,
            }))
            .num_paths(1000)
            .seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04);
        assert!(result.is_ok(), "Valid 30/70 split should be accepted");
    }

    // -----------------------------------------------------------------------
    // Term-structure discounting tests
    // -----------------------------------------------------------------------

    #[test]
    fn cashflow_dfs_overrides_flat_rate() {
        let flat_config = MertonMcConfig::new(test_merton()).num_paths(2000).seed(42);
        let flat = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &flat_config, 0.04).expect("ok");

        // Build steeper curve DFs (higher short rates, lower long rates)
        let steep_dfs: Vec<(f64, f64)> = (1..=60)
            .map(|i| {
                let t = i as f64 / 12.0;
                let r = 0.05 - 0.002 * t; // inverted for visible difference
                (t, (-r * t).exp())
            })
            .collect();
        let ts_config = MertonMcConfig::new(test_merton())
            .num_paths(2000)
            .seed(42)
            .cashflow_dfs(steep_dfs);
        let ts = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &ts_config, 0.04).expect("ok");

        assert!(
            (flat.clean_price_pct - ts.clean_price_pct).abs() > 0.01,
            "Term-structure DFs should produce a different price: flat={}, ts={}",
            flat.clean_price_pct,
            ts.clean_price_pct
        );
    }

    // -----------------------------------------------------------------------
    // Spread solver test
    // -----------------------------------------------------------------------

    #[test]
    fn effective_spread_positive_for_risky_bond() {
        let config = MertonMcConfig::new(test_merton()).num_paths(5000).seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            result.effective_spread_bp > 0.0,
            "Risky bond should have positive effective spread, got {}",
            result.effective_spread_bp
        );
    }

    #[test]
    fn standard_error_in_pct_of_par() {
        let config = MertonMcConfig::new(test_merton()).num_paths(5000).seed(42);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            result.standard_error > 0.001 && result.standard_error < 10.0,
            "SE in pct-of-par should be small but positive: got {}",
            result.standard_error
        );
    }

    // -----------------------------------------------------------------------
    // Antithetic complementary-uniform regression guard (W15)
    // -----------------------------------------------------------------------
    //
    // The antithetic path uses sign-flipped normals.  Its Brownian-bridge
    // barrier-crossing test must use the *complementary* uniform `1 - u`
    // (where the base path uses `u`) so that the two legs remain negatively
    // correlated in their default decisions.  Before this fix both legs used
    // the same raw `uniforms[step]`, which partially defeated variance
    // reduction and produced a subtly biased `default_rate`.
    //
    // This test pins the *corrected* `default_rate` and `clean_price_pct` for
    // a fixed seed.  It is a deterministic value-pinning guard: if the
    // complementary-uniform logic is regressed the output changes measurably
    // and this test will fail.
    #[test]
    fn antithetic_bridge_uses_complementary_uniform() {
        let config = MertonMcConfig::new(test_merton())
            .num_paths(10_000)
            .antithetic(true)
            .seed(777)
            .barrier_crossing(BarrierCrossing::BrownianBridge);
        let result = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");

        // Pinned post-fix values (seed=777, 10_000 paths, antithetic=true).
        // The pre-fix (biased) values were default_rate=0.1889 and
        // clean_price_pct≈106.197, confirming the fix has an observable effect.
        let expected_default_rate = 0.1882_f64;
        let expected_price = 106.109_28_f64;

        assert!(
            (result.path_statistics.default_rate - expected_default_rate).abs() < 1e-9,
            "Antithetic default_rate should be {expected_default_rate} (complementary uniform), \
             got {}",
            result.path_statistics.default_rate
        );
        assert!(
            (result.clean_price_pct - expected_price).abs() < 1e-3,
            "clean_price_pct should be ≈{expected_price} (complementary uniform), got {}",
            result.clean_price_pct
        );

        // Also verify determinism is preserved with the fixed seed.
        let result2 = MertonMcEngine::price(100.0, 0.08, 5.0, 2, &config, 0.04).expect("ok");
        assert!(
            (result.clean_price_pct - result2.clean_price_pct).abs() < 1e-10,
            "Same seed must give identical results after fix"
        );
    }
}
