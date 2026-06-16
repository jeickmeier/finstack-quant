//! Calibration configuration and solver selection.
//!
//! This module provides unified configuration for calibration processes including:
//! - Solver selection and numerical parameters
//! - Rate bounds for market-regime-aware calibration
//! - Validation and explainability settings

use crate::calibration::solver::SolverConfig;
use crate::calibration::validation::{RateBounds, RateBoundsPolicy, ValidationMode};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::explain::ExplainOpts;
use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
use finstack_quant_core::money::fx::FxConfig;

use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// Calibration method selection (bootstrap vs global solve).
///
/// Defines the numerical approach used to solve for curve/surface parameters.
/// Bootstrap is the traditional sequential approach, while GlobalSolve
/// solves all parameters simultaneously.
///
/// # Variants
/// - `Bootstrap`: Traditional sequential bootstrap where each knot is solved
///   independently based on the previous knots.
/// - `GlobalSolve`: Simultaneous optimization of all knots using Levenberg-Marquardt
///   or Newton-Raphson.
///
/// # Examples
/// ```rust
/// use finstack_quant_valuations::calibration::CalibrationMethod;
///
/// let method = CalibrationMethod::GlobalSolve { use_analytical_jacobian: true };
/// ```
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub enum CalibrationMethod {
    /// Traditional sequential bootstrap (default).
    #[default]
    Bootstrap,
    /// Global solve of all knots simultaneously (Newton/LM).
    GlobalSolve {
        /// Use analytical Jacobian if available (otherwise finite-difference).
        #[serde(default)]
        use_analytical_jacobian: bool,
    },
}

impl std::fmt::Display for CalibrationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bootstrap => write!(f, "bootstrap"),
            Self::GlobalSolve { .. } => write!(f, "global_solve"),
        }
    }
}

impl std::str::FromStr for CalibrationMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "bootstrap" => Ok(Self::Bootstrap),
            "global_solve" | "globalsolve" => Ok(Self::GlobalSolve {
                use_analytical_jacobian: false,
            }),
            other => Err(format!(
                "Unknown calibration method: '{}'. Valid: bootstrap, global_solve",
                other
            )),
        }
    }
}

/// Policy for weighting residuals in global solve calibration.
///
/// Determines how the objective function weights individual instrument fitting
/// errors (residuals) during optimization.
///
/// # Variants
/// - `Equal`: Every instrument contributes equally to the objective.
/// - `LinearTime`: Weights increase linearly with time to maturity.
/// - `SqrtTime`: Weights increase with the square root of time (market-standard).
/// - `InverseDuration`: Weights based on inverse DV01 approximation.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ResidualWeightingScheme {
    /// Equal weighting (1.0 for all quotes).
    Equal,
    /// Weight by time to maturity (t).
    LinearTime,
    /// Weight by square root of time (sqrt(t)).
    #[default]
    SqrtTime,
    /// Weight by inverse duration (1/DV01 approximation).
    InverseDuration,
}

impl std::fmt::Display for ResidualWeightingScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Equal => write!(f, "equal"),
            Self::LinearTime => write!(f, "linear_time"),
            Self::SqrtTime => write!(f, "sqrt_time"),
            Self::InverseDuration => write!(f, "inverse_duration"),
        }
    }
}

impl std::str::FromStr for ResidualWeightingScheme {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', '/', ' '], "_");
        match normalized.as_str() {
            "equal" => Ok(Self::Equal),
            "linear_time" | "lineartime" => Ok(Self::LinearTime),
            "sqrt_time" | "sqrttime" => Ok(Self::SqrtTime),
            "inverse_duration" | "inverseduration" => Ok(Self::InverseDuration),
            other => Err(format!(
                "Unknown residual weighting scheme: '{}'. Valid: equal, linear_time, sqrt_time, inverse_duration",
                other
            )),
        }
    }
}

/// Hazard-curve specific numerical solver configuration.
///
/// Controls the search space and numerical stability of hazard curve
/// bootstrapping or global solve for credit curve calibration.
///
/// # Invariants
/// - `hazard_hard_min` >= 0 (hazard rates must be non-negative)
/// - `hazard_hard_max` > `hazard_hard_min`
///
/// # Examples
/// ```ignore
/// use finstack_quant_valuations::calibration::HazardCurveSolveConfig;
///
/// // For distressed debt scenarios, increase the max hazard rate
/// let config = HazardCurveSolveConfig {
///     hazard_hard_max: 100.0,  // Allow up to ~100% default probability per year
///     ..Default::default()
/// };
/// ```
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HazardCurveSolveConfig {
    /// Minimum allowed hazard rate (must be non-negative for survival monotonicity).
    pub hazard_hard_min: f64,
    /// Maximum allowed hazard rate.
    ///
    /// The default of 10.0 corresponds to roughly 99.995% 1Y default probability.
    /// For distressed/distressed sovereign scenarios, increase to 50.0 or 100.0.
    pub hazard_hard_max: f64,
    /// Weighting scheme for global solve residuals.
    #[serde(default)]
    pub weighting_scheme: ResidualWeightingScheme,
    /// Tolerance for determining calibration *success* (applied to residuals).
    ///
    /// After the solver converges, the final residuals are compared against this
    /// tolerance. If `max_residual > validation_tolerance`, the calibration report
    /// will have `success = false` even if the solver converged.
    ///
    /// This is distinct from `solver.tolerance()` which controls when the numerical
    /// solver terminates. See [`CalibrationConfig`] for a full explanation.
    ///
    /// Default: `1e-8` (suitable for per-unit-notional residuals).
    pub validation_tolerance: f64,
}

impl Default for HazardCurveSolveConfig {
    fn default() -> Self {
        Self {
            hazard_hard_min: 0.0,
            hazard_hard_max: 10.0, // ~99.995% 1Y default probability
            weighting_scheme: ResidualWeightingScheme::default(),
            validation_tolerance: 1e-8,
        }
    }
}

/// Inflation-curve specific numerical solver configuration.
///
/// Controls the numerical stability and success criteria of inflation curve
/// bootstrapping or global solve for CPI curve calibration.
///
/// # Examples
/// ```ignore
/// use finstack_quant_valuations::calibration::InflationCurveSolveConfig;
///
/// let config = InflationCurveSolveConfig {
///     validation_tolerance: 1e-6,  // Relax for noisy inflation markets
///     ..Default::default()
/// };
/// ```
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InflationCurveSolveConfig {
    /// Weighting scheme for global solve residuals.
    #[serde(default)]
    pub weighting_scheme: ResidualWeightingScheme,
    /// Tolerance for determining calibration *success* (applied to residuals).
    ///
    /// After the solver converges, the final residuals are compared against this
    /// tolerance. If `max_residual > validation_tolerance`, the calibration report
    /// will have `success = false` even if the solver converged.
    ///
    /// This is distinct from `solver.tolerance()` which controls when the numerical
    /// solver terminates. See [`CalibrationConfig`] for a full explanation.
    ///
    /// Default: `1e-8` (suitable for per-unit-notional residuals).
    pub validation_tolerance: f64,
    /// Minimum allowed CPI level during calibration.
    ///
    /// Default: `1.0`. For indices with different base conventions or rebased
    /// indices, adjust accordingly.
    pub cpi_hard_min: f64,
    /// Maximum allowed CPI level during calibration.
    ///
    /// Default: `10_000.0`. For hyperinflation currencies (TRY, ARS, VES),
    /// increase this bound significantly (e.g. `1e9`).
    pub cpi_hard_max: f64,
}

impl Default for InflationCurveSolveConfig {
    fn default() -> Self {
        Self {
            weighting_scheme: ResidualWeightingScheme::default(),
            validation_tolerance: 1e-8,
            cpi_hard_min: 1.0,
            cpi_hard_max: 10_000.0,
        }
    }
}

/// Discount-curve specific numerical solver configuration.
///
/// Controls the search space and numerical stability of the discount curve
/// bootstrapping or global solve process.
///
/// # Invariants
/// - `df_hard_min` > 0
/// - `scan_grid_points` > 0
///
/// # Examples
/// ```ignore
/// use finstack_quant_valuations::calibration::DiscountCurveSolveConfig;
///
/// let config = DiscountCurveSolveConfig {
///     scan_grid_points: 64,
///     df_hard_min: 1e-10,
///     ..Default::default()
/// };
/// ```
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiscountCurveSolveConfig {
    /// Number of points in the initial geometric scan grid.
    pub scan_grid_points: usize,
    /// Minimum required success points in scan before attempting polish.
    pub min_scan_grid_points: usize,
    /// Initial step size for geometric scan grid.
    pub scan_grid_step: f64,
    /// Absolute minimum allowed discount factor (prevents singularity).
    pub df_hard_min: f64,
    /// Absolute maximum allowed discount factor (prevents divergence).
    pub df_hard_max: f64,
    /// Minimum time threshold for considering a knot at spot (t=0).
    pub min_t_spot: f64,
    /// Interpolation style for the constructed curve.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(skip))]
    pub interp_style: InterpStyle,
    /// Extrapolation policy for the constructed curve.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(skip))]
    pub extrapolation_policy: ExtrapolationPolicy,
    /// Whether to use a sequential bootstrap to seed a global solve.
    pub bootstrap_seed_global_solve: bool,
    /// Override final-curve monotonicity enforcement (None = policy-driven).
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "boolean | null"))]
    pub allow_non_monotonic_final: Option<bool>,
    /// Weighting scheme for global solve residuals.
    #[serde(default)]
    pub weighting_scheme: ResidualWeightingScheme,
    /// Step size (h) for finite-difference Jacobian calculation.
    #[serde(default)]
    pub jacobian_step_size: f64,
    /// Tolerance for determining calibration *success* (applied to residuals).
    ///
    /// After the solver converges, the final residuals are compared against this
    /// tolerance. If `max_residual > validation_tolerance`, the calibration report
    /// will have `success = false` even if the solver converged.
    ///
    /// This is distinct from `solver.tolerance()` which controls when the numerical
    /// solver terminates. See [`CalibrationConfig`] for a full explanation.
    ///
    /// Default: `1e-8` (suitable for per-unit-notional residuals).
    pub validation_tolerance: f64,
}

impl Default for DiscountCurveSolveConfig {
    fn default() -> Self {
        Self {
            scan_grid_points: 48,
            min_scan_grid_points: 20,
            scan_grid_step: 1e-4,
            interp_style: InterpStyle::default(),
            extrapolation_policy: ExtrapolationPolicy::default(),
            df_hard_min: 1e-12,
            df_hard_max: 1e6,
            min_t_spot: 1e-6,
            bootstrap_seed_global_solve: true,
            allow_non_monotonic_final: None,
            weighting_scheme: ResidualWeightingScheme::default(),
            // `jacobian_step_size` is used as a *relative* bump size (h = max(eps, |p|*eps)).
            // For typical discount-curve zero rates (~1-5%), `1e-8` can make PV differences
            // fall into numerical noise and cause GlobalSolve to stall (e.g. StepTooSmall).
            jacobian_step_size: 1e-6,
            // Validation success tolerance for residuals (per notional).
            validation_tolerance: 1e-8,
        }
    }
}

/// Global configuration for the calibration subsystem.
///
/// This struct consolidates all settings for solvers, validation, and market-regime
/// specific bounds. It is typically derived from a `FinstackConfig` extension section.
/// Public callers should treat it as the behavioral contract for calibration
/// execution policy: solver choice, convergence settings, validation thresholds,
/// rate-bound policy, and curve-specific numerical guardrails.
///
/// # Tolerance Semantics
///
/// Calibration involves two distinct tolerance concepts:
///
/// 1. **Solver Tolerance** ([`solver.tolerance()`](SolverConfig::tolerance)):
///    Controls when the numerical solver (Brent/Newton) terminates. This is an
///    algorithmic convergence criterion in x-space (parameter space). The solver
///    stops when successive parameter estimates differ by less than this tolerance.
///
/// 2. **Validation Tolerance** (e.g., [`discount_curve.validation_tolerance`](DiscountCurveSolveConfig::validation_tolerance)):
///    Controls whether calibration is considered *successful*. After the solver
///    converges, the final residuals are compared against this tolerance. If any
///    residual exceeds `validation_tolerance`, the calibration is marked as failed
///    even if the solver converged.
///
/// **Why two tolerances?**
/// - Solver tolerance ensures numerical convergence but doesn't guarantee economic fit.
/// - Validation tolerance ensures the calibrated curve actually prices instruments correctly.
/// - For well-behaved problems, solver tolerance of `1e-12` with validation tolerance of
///   `1e-8` works well: the solver finds a precise root, and we verify it prices accurately.
///
/// # Configuration Hierarchy
///
/// Settings can be specified at multiple levels with the following precedence:
///
/// 1. **Step-level** (`CalibrationStep.params.method`): Per-instrument-type overrides
/// 2. **Plan-level** (`CalibrationPlan.settings`): Plan-wide defaults
/// 3. **Finstack config extensions** (`valuations.calibration`): application defaults
/// 4. **Global defaults** (`CalibrationConfig::default()`): fallback values
///
/// Step-level settings always take precedence over plan-level settings.
/// In other words, this struct provides default policy, but explicit plan steps
/// remain authoritative when both are supplied.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_valuations::calibration::CalibrationConfig;
///
/// // Create a default config
/// let config = CalibrationConfig::default();
///
/// // Customize tolerance settings
/// let custom = CalibrationConfig::default()
///     .with_tolerance(1e-14)  // Solver convergence tolerance
///     .with_max_iterations(200);
/// ```
///
/// # References
///
/// - Multi-curve construction context: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
/// - Curve interpolation context: `docs/REFERENCES.md#hagan-west-monotone-convex`
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CalibrationConfig {
    /// Solver configuration including numerical method (e.g., Brent) and parameters (tolerance, iterations).
    #[schemars(with = "serde_json::Value")]
    pub solver: SolverConfig,
    /// Use parallel processing when available (e.g., for independent curves).
    pub use_parallel: bool,
    /// Enable verbose logging of the calibration process.
    pub verbose: bool,
    /// Explanation options (opt-in detailed trace for debugging).
    #[serde(skip)]
    #[cfg_attr(feature = "ts_export", ts(skip))]
    #[schemars(with = "serde_json::Value")]
    pub explain: ExplainOpts,
    /// Runtime validation mode (warnings vs errors).
    #[schemars(with = "serde_json::Value")]
    pub validation_mode: ValidationMode,
    /// Validation configuration with thresholds and quality checks.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "Record<string, unknown>"))]
    #[schemars(with = "serde_json::Value")]
    pub validation: crate::calibration::validation::ValidationConfig,
    /// Policy for selecting rate bounds (explicit vs currency-derived).
    #[serde(default = "crate::calibration::validation::default_rate_bounds_policy_for_serde")]
    #[schemars(with = "serde_json::Value")]
    pub rate_bounds_policy: RateBoundsPolicy,
    /// Rate bounds for forward/zero rate calibration (when policy is `Explicit`).
    #[serde(default)]
    #[schemars(with = "serde_json::Value")]
    pub rate_bounds: RateBounds,
    /// High-level calibration method (bootstrap vs global solve).
    ///
    /// **Note**: When using the plan-driven API, this field is typically overwritten
    /// by the step-level `params.method` for each calibration step. The step-level
    /// method always takes precedence. This field serves as runtime state passed
    /// from calibration targets to the underlying solvers.
    #[serde(default)]
    pub calibration_method: CalibrationMethod,

    /// Whether to compute detailed calibration diagnostics (condition number,
    /// per-quote quality metrics, singular values, R-squared, etc.).
    ///
    /// When `true`, the solver will perform additional post-solve analysis
    /// including Jacobian-based condition number estimation. This adds
    /// computational overhead and should typically be disabled in production
    /// hot paths but enabled for calibration debugging, auditing, or
    /// quality monitoring.
    ///
    /// Default: `false`.
    #[serde(default)]
    pub compute_diagnostics: bool,

    /// Discount-curve specific solver configuration.
    #[serde(default)]
    #[schemars(with = "serde_json::Value")]
    pub discount_curve: DiscountCurveSolveConfig,

    /// Hazard-curve specific solver configuration.
    #[serde(default)]
    #[schemars(with = "serde_json::Value")]
    pub hazard_curve: HazardCurveSolveConfig,

    /// Inflation-curve specific solver configuration.
    #[serde(default)]
    #[schemars(with = "serde_json::Value")]
    pub inflation_curve: InflationCurveSolveConfig,

    /// When `true`, a calibration step whose solver reports
    /// `report.success == false` is propagated as a
    /// `finstack_quant_core::Error::Calibration` and its output is **not**
    /// installed into the market context.
    ///
    /// Defaults to `true` — this is the safe production choice because
    /// a non-converged solver would otherwise silently poison downstream
    /// pricing. Legacy or exploratory workflows that want to inspect the
    /// report without aborting can set this to `false`.
    #[serde(default = "default_fail_on_bad_fit")]
    pub fail_on_bad_fit: bool,

    /// FX matrix runtime config (pivot currency, triangulation, cache capacity).
    /// Hoisted out of `initial_market.fx.config` in v3 envelopes.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "Record<string, unknown>"))]
    #[schemars(with = "serde_json::Value")]
    pub fx: FxConfig,

    /// Optional market-data hierarchy snapshot.
    /// Hoisted out of `initial_market.hierarchy` in v3 envelopes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts_export", ts(type = "Record<string, unknown> | null"))]
    #[schemars(with = "serde_json::Value")]
    pub hierarchy: Option<MarketDataHierarchy>,
}

fn default_fail_on_bad_fit() -> bool {
    true
}

/// Extension section key for calibration overrides.
pub const CALIBRATION_CONFIG_KEY: &str = "valuations.calibration";

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            solver: SolverConfig::default(),
            use_parallel: false, // Deterministic by default
            verbose: false,
            explain: ExplainOpts::default(), // Disabled by default (zero overhead)
            validation_mode: ValidationMode::Error,
            validation: crate::calibration::validation::ValidationConfig::default(),
            rate_bounds_policy: RateBoundsPolicy::AutoCurrency,
            rate_bounds: RateBounds::default(),
            calibration_method: CalibrationMethod::default(),
            compute_diagnostics: false,
            discount_curve: DiscountCurveSolveConfig::default(),
            hazard_curve: HazardCurveSolveConfig::default(),
            inflation_curve: InflationCurveSolveConfig::default(),
            fail_on_bad_fit: default_fail_on_bad_fit(),
            fx: FxConfig::default(),
            hierarchy: None,
        }
    }
}

impl CalibrationConfig {
    /// Build a calibration config from a `FinstackConfig` extension section.
    ///
    /// If the extension section `valuations.calibration` is present, its
    /// fields override the defaults; otherwise defaults are used.
    ///
    /// # Errors
    ///
    /// Returns an error if the extension section is present but malformed
    /// (e.g., unknown fields when `deny_unknown_fields` is enforced).
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::config::FinstackConfig;
    /// use finstack_quant_valuations::calibration::CalibrationConfig;
    ///
    /// let cfg = FinstackConfig::default();
    /// let calib_cfg = CalibrationConfig::from_finstack_config_or_default(&cfg)
    ///     .expect("valid config");
    /// assert_eq!(calib_cfg.solver.tolerance(), 1e-12); // default
    /// ```
    pub fn from_finstack_config_or_default(
        cfg: &FinstackConfig,
    ) -> finstack_quant_core::Result<Self> {
        let config = if let Some(raw) = cfg.extensions.get(CALIBRATION_CONFIG_KEY) {
            // Deserialize directly into CalibrationConfig; missing fields use defaults via #[serde(default)]
            serde_json::from_value(raw.clone()).map_err(|e| {
                finstack_quant_core::Error::Calibration {
                    message: format!(
                        "Failed to parse extension '{}': {}",
                        CALIBRATION_CONFIG_KEY, e
                    ),
                    category: "config".to_string(),
                }
            })?
        } else {
            Self::default()
        };
        config.validate()?;
        Ok(config)
    }

    /// Validate cross-field calibration configuration invariants.
    ///
    /// # Errors
    ///
    /// Returns an error when nested validation/rate-bound settings are invalid or
    /// when solver tolerance is looser than residual success tolerances.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.validation.validate()?;
        self.rate_bounds.validate()?;
        self.validate_solver_vs_success_tolerance(
            "discount_curve.validation_tolerance",
            self.discount_curve.validation_tolerance,
        )?;
        self.validate_solver_vs_success_tolerance(
            "hazard_curve.validation_tolerance",
            self.hazard_curve.validation_tolerance,
        )?;
        self.validate_solver_vs_success_tolerance(
            "inflation_curve.validation_tolerance",
            self.inflation_curve.validation_tolerance,
        )?;
        self.validate_hazard_bounds()?;
        self.validate_inflation_bounds()
    }

    fn validate_solver_vs_success_tolerance(
        &self,
        label: &str,
        validation_tolerance: f64,
    ) -> finstack_quant_core::Result<()> {
        let solver_tolerance = self.solver.tolerance();
        if !validation_tolerance.is_finite() || validation_tolerance <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CalibrationConfig invalid: {label} must be finite and positive, got {validation_tolerance}"
            )));
        }
        if solver_tolerance > validation_tolerance {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CalibrationConfig invalid: solver tolerance ({solver_tolerance}) must be <= {label} ({validation_tolerance})"
            )));
        }
        Ok(())
    }

    fn validate_hazard_bounds(&self) -> finstack_quant_core::Result<()> {
        if !self.hazard_curve.hazard_hard_min.is_finite()
            || !self.hazard_curve.hazard_hard_max.is_finite()
            || self.hazard_curve.hazard_hard_min < 0.0
            || self.hazard_curve.hazard_hard_min >= self.hazard_curve.hazard_hard_max
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CalibrationConfig invalid: hazard bounds must satisfy 0 <= hazard_hard_min < hazard_hard_max; got ({}, {})",
                self.hazard_curve.hazard_hard_min, self.hazard_curve.hazard_hard_max
            )));
        }
        Ok(())
    }

    fn validate_inflation_bounds(&self) -> finstack_quant_core::Result<()> {
        if !self.inflation_curve.cpi_hard_min.is_finite()
            || !self.inflation_curve.cpi_hard_max.is_finite()
            || self.inflation_curve.cpi_hard_min <= 0.0
            || self.inflation_curve.cpi_hard_min >= self.inflation_curve.cpi_hard_max
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CalibrationConfig invalid: CPI bounds must satisfy 0 < cpi_hard_min < cpi_hard_max; got ({}, {})",
                self.inflation_curve.cpi_hard_min, self.inflation_curve.cpi_hard_max
            )));
        }
        Ok(())
    }

    /// Resolve effective rate bounds for a given currency based on `rate_bounds_policy`.
    pub fn effective_rate_bounds(&self, currency: Currency) -> RateBounds {
        match self.rate_bounds_policy {
            RateBoundsPolicy::AutoCurrency => RateBounds::for_currency(currency),
            RateBoundsPolicy::Explicit => self.rate_bounds.clone(),
        }
    }

    /// Set custom rate bounds for calibration.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_valuations::calibration::{CalibrationConfig, RateBounds};
    ///
    /// let config = CalibrationConfig::default()
    ///     .with_rate_bounds(RateBounds::emerging_markets());
    /// ```
    #[must_use]
    pub fn with_rate_bounds(mut self, bounds: RateBounds) -> Self {
        self.rate_bounds_policy = RateBoundsPolicy::Explicit;
        self.rate_bounds = bounds;
        self
    }

    /// Set the calibration method (bootstrap vs global solve).
    #[must_use]
    pub fn with_calibration_method(mut self, method: CalibrationMethod) -> Self {
        self.calibration_method = method;
        self
    }

    /// Set the solver tolerance.
    #[must_use]
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.solver = self.solver.with_tolerance(tolerance);
        self
    }

    /// Set the maximum number of iterations.
    #[must_use]
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.solver = self.solver.with_max_iterations(max_iterations);
        self
    }

    /// Enable or disable detailed calibration diagnostics.
    ///
    /// When enabled, the solver will compute condition number, per-quote
    /// quality metrics, and other diagnostic information after calibration.
    #[must_use]
    pub fn with_compute_diagnostics(mut self, enabled: bool) -> Self {
        self.compute_diagnostics = enabled;
        self
    }

    /// Create a Levenberg-Marquardt solver with current config settings.
    ///
    /// Returns the concrete solver for multi-dimensional optimization.
    pub fn create_lm_solver(
        &self,
    ) -> finstack_quant_core::math::solver_multi::LevenbergMarquardtSolver {
        use finstack_quant_core::math::solver_multi::LevenbergMarquardtSolver;

        LevenbergMarquardtSolver::new()
            .with_tolerance(self.solver.tolerance())
            .with_max_iterations(self.solver.max_iterations())
    }
}

/// Step-level conventions for rates calibration (discount and forward curves).
///
/// This is a Bloomberg/FinCad-style design: curve construction uses a small set of
/// *step-level* conventions (e.g., curve time-axis day count).
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RatesStepConventions {
    /// Day count used to map dates to year fractions for curve knot times.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub curve_day_count: Option<finstack_quant_core::dates::DayCount>,

    /// Optional override for the OIS floating-leg compounding mode used by
    /// the calibration's bootstrap-internal swaps.
    ///
    /// When unset, the bootstrap uses the registered per-index default
    /// (e.g. SOFR → `CompoundedInArrears { lookback_days: 0 }`, the cleared
    /// OIS plain in-arrears convention).
    /// Set this to match a vendor convention that differs from the registry
    /// default — e.g. Bloomberg SWPM SOFR uses
    /// `CompoundedWithRateCutoff { cutoff_days: 1 }` for the daily-compounded
    /// float leg, and a curve calibrated against SWPM screen rates needs to
    /// price its bootstrap swaps with the same compounding to bit-match
    /// Bloomberg's resulting DFs.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "unknown | null"))]
    pub ois_compounding: Option<crate::instruments::rates::irs::FloatingLegCompounding>,
}

// =============================================================================
// End of configuration module

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn residual_weighting_scheme_fromstr_display_roundtrip() {
        fn assert_residual_weighting_scheme(label: &str, expected: ResidualWeightingScheme) {
            assert!(
                matches!(ResidualWeightingScheme::from_str(label), Ok(value) if value == expected)
            );
        }

        let variants = [
            ResidualWeightingScheme::Equal,
            ResidualWeightingScheme::LinearTime,
            ResidualWeightingScheme::SqrtTime,
            ResidualWeightingScheme::InverseDuration,
        ];
        for v in variants {
            let s = v.to_string();
            let parsed =
                ResidualWeightingScheme::from_str(&s).expect("roundtrip parse should succeed");
            assert_eq!(v, parsed, "roundtrip failed for {s}");
        }
        // Test aliases
        assert_residual_weighting_scheme("lineartime", ResidualWeightingScheme::LinearTime);
        assert_residual_weighting_scheme("sqrttime", ResidualWeightingScheme::SqrtTime);
        assert!(ResidualWeightingScheme::from_str("invalid").is_err());
    }

    #[test]
    fn calibration_method_fromstr_display_roundtrip() {
        fn assert_calibration_method(label: &str, matcher: fn(&CalibrationMethod) -> bool) {
            let parsed = CalibrationMethod::from_str(label);
            assert!(matches!(parsed.as_ref(), Ok(value) if matcher(value)));
        }

        // Bootstrap roundtrips exactly
        let bootstrap = CalibrationMethod::Bootstrap;
        let s = bootstrap.to_string();
        let parsed = CalibrationMethod::from_str(&s).expect("roundtrip parse should succeed");
        assert_eq!(bootstrap, parsed);

        // GlobalSolve parses to default (use_analytical_jacobian = false)
        assert_calibration_method("global_solve", |value| {
            matches!(
                value,
                CalibrationMethod::GlobalSolve {
                    use_analytical_jacobian: false
                }
            )
        });

        // Alias
        assert_calibration_method("globalsolve", |value| {
            matches!(value, CalibrationMethod::GlobalSolve { .. })
        });

        assert!(CalibrationMethod::from_str("invalid").is_err());
    }
}

#[cfg(test)]
mod fx_and_hierarchy_settings_tests {
    use super::*;
    use finstack_quant_core::money::fx::FxConfig;

    #[test]
    fn config_defaults_carry_fx_subsection() {
        let cfg = CalibrationConfig::default();
        assert_eq!(cfg.fx, FxConfig::default());
        assert!(cfg.hierarchy.is_none());
    }

    #[test]
    fn fx_settings_round_trip_via_serde() {
        let json = r#"{
            "fx": { "pivot_currency": "EUR", "enable_triangulation": false, "cache_capacity": 8 }
        }"#;
        let cfg: CalibrationConfig =
            serde_json::from_str(json).expect("valid CalibrationConfig JSON");
        assert_eq!(
            cfg.fx.pivot_currency,
            finstack_quant_core::currency::Currency::EUR
        );
        assert!(!cfg.fx.enable_triangulation);
        assert_eq!(cfg.fx.cache_capacity, 8);
    }

    #[test]
    fn omitted_fx_section_uses_defaults() {
        let json = r#"{}"#;
        let cfg: CalibrationConfig = serde_json::from_str(json).expect("empty JSON");
        assert_eq!(cfg.fx, FxConfig::default());
        assert!(cfg.hierarchy.is_none());
    }
}
