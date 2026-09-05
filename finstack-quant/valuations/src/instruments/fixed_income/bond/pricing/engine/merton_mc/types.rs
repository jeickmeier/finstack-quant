use finstack_quant_models::credit::{
    BarrierType, DynamicRecoverySpec, EndogenousHazardSpec, MertonModel, ToggleExerciseModel,
};

// PIK schedule types

/// Barrier-crossing detection policy for first-passage default simulation.
///
/// `Discrete` only checks the barrier at grid points (fast but biased for
/// coarse time steps). `BrownianBridge` uses a Brownian-bridge crossing
/// probability between grid points to approximate continuous monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum BarrierCrossing {
    /// Discrete monitoring: default if `V(t_i) < B(t_i)` at time steps.
    Discrete,
    /// Brownian-bridge correction for continuous monitoring between steps.
    BrownianBridge,
}

/// Which structural parameter to calibrate in the MC engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
/// use finstack_quant_valuations::instruments::fixed_income::bond::pricing::engine::merton_mc::{PikMode, PikSchedule};
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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

/// Configuration for Monte Carlo PIK bond pricing.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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

impl MertonMcConfig {
    /// Create a new configuration with default simulation parameters.
    ///
    /// Simulation defaults are sourced from the embedded Monte Carlo registry;
    /// recovery is always supplied explicitly by the caller.
    ///
    /// # Arguments
    ///
    /// * `merton` - Structural credit model driving the simulated asset value
    ///   and default boundary.
    /// * `recovery_rate` - Recovery on default as a decimal fraction in the
    ///   inclusive range `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `recovery_rate` is non-finite or lies
    /// outside `[0, 1]`.
    pub fn new(merton: MertonModel, recovery_rate: f64) -> finstack_quant_core::Result<Self> {
        validate_recovery_rate(recovery_rate)?;
        let defaults = &finstack_quant_models::monte_carlo::registry::embedded_defaults()?
            .rust
            .merton_pik_bond;
        let barrier_crossing = match merton.barrier_type() {
            BarrierType::FirstPassage { .. } => BarrierCrossing::BrownianBridge,
            BarrierType::Terminal => BarrierCrossing::Discrete,
        };
        Ok(Self {
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
            default_recovery_rate: recovery_rate,
            calibration: None,
            cashflow_dfs: None,
        })
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
    ///
    /// # Arguments
    ///
    /// * `c` - Market-quote calibration that the MC engine uses to solve for barrier
    ///   or asset vol before the full-path reprice.
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

    /// Set the flat recovery used when no dynamic recovery model is configured.
    ///
    /// # Arguments
    ///
    /// * `recovery_rate` - Recovery on default as a decimal fraction in the
    ///   inclusive range `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns a validation error when `recovery_rate` is non-finite or lies
    /// outside `[0, 1]`.
    pub fn default_recovery_rate(
        mut self,
        recovery_rate: f64,
    ) -> finstack_quant_core::Result<Self> {
        validate_recovery_rate(recovery_rate)?;
        self.default_recovery_rate = recovery_rate;
        Ok(self)
    }

    /// Set the toggle exercise model.
    #[must_use]
    pub fn toggle_model(mut self, t: ToggleExerciseModel) -> Self {
        self.toggle_model = Some(t);
        self
    }
}

fn validate_recovery_rate(recovery_rate: f64) -> finstack_quant_core::Result<()> {
    finstack_quant_core::validation::require_with(
        recovery_rate.is_finite() && (0.0..=1.0).contains(&recovery_rate),
        || {
            format!(
                "MertonMcConfig recovery_rate must be finite and in [0, 1], got {recovery_rate}"
            )
        },
    )
}

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
