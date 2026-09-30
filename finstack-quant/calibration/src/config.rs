//! Calibration configuration and solver selection.
//!
//! This module provides unified configuration for calibration processes including:
//! - Solver selection and numerical parameters
//! - Rate bounds for market-regime-aware calibration
//! - Validation and explainability settings

use crate::solver::SolverConfig;
use crate::validation::{RateBounds, RateBoundsPolicy, ValidationMode};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::explain::ExplainOpts;
use finstack_quant_core::market_data::hierarchy::MarketDataHierarchy;
use finstack_quant_core::money::fx::FxConfig;

use serde::{Deserialize, Serialize};

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
/// use finstack_quant_calibration::CalibrationMethod;
///
/// let method = CalibrationMethod::GlobalSolve { use_analytical_jacobian: true };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CalibrationMethod {
    /// Traditional sequential bootstrap (default).
    #[default]
    Bootstrap,
    /// Global solve of all knots simultaneously (Newton/LM).
    GlobalSolve {
        /// Use analytical Jacobian if available (otherwise finite-difference).
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
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
/// ```
/// use finstack_quant_calibration::HazardCurveSolveConfig;
///
/// // For distressed debt scenarios, increase the max hazard rate
/// let config = HazardCurveSolveConfig {
///     hazard_hard_max: 100.0,  // Allow up to ~100% default probability per year
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
/// ```
/// use finstack_quant_calibration::InflationCurveSolveConfig;
///
/// let config = InflationCurveSolveConfig {
///     validation_tolerance: 1e-6,  // Relax for noisy inflation markets
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

/// Volatility-surface specific numerical solver configuration.
///
/// Controls the success criterion for SABR and SVI surface calibration.
/// Residuals are in **decimal implied-vol units** (for example `0.001` is
/// 0.10 vol points), not PV-per-notional.
///
/// # Examples
/// ```
/// use finstack_quant_calibration::VolSurfaceSolveConfig;
///
/// let config = VolSurfaceSolveConfig {
///     validation_tolerance: 2e-3, // 0.20 vol points
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct VolSurfaceSolveConfig {
    /// Tolerance for determining calibration *success* (applied to vol residuals).
    ///
    /// After each expiry slice is calibrated, the final `|σ_model − σ_mkt|`
    /// residuals are compared against this tolerance. If `max_residual >
    /// validation_tolerance`, the calibration report will have `success = false`
    /// even if the slice solver converged.
    ///
    /// This is distinct from `solver.tolerance()` which controls when the
    /// numerical solver terminates. See [`CalibrationConfig`] for a full
    /// explanation.
    ///
    /// Default: `1e-3` (0.10 vol points in decimal vol).
    pub validation_tolerance: f64,
}

impl Default for VolSurfaceSolveConfig {
    fn default() -> Self {
        Self {
            validation_tolerance: 1e-3,
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
/// ```
/// use finstack_quant_calibration::DiscountCurveSolveConfig;
///
/// let config = DiscountCurveSolveConfig {
///     scan_grid_points: 64,
///     df_hard_min: 1e-10,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    /// Whether to use a sequential bootstrap to seed a global solve.
    pub bootstrap_seed_global_solve: bool,
    /// Override final-curve monotonicity enforcement (None = policy-driven).
    #[serde(default)]
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

/// Forward-curve specific numerical solver configuration.
///
/// Controls residual weighting and post-solve success tolerance for
/// projection-curve global solves. Defaults match the values previously
/// borrowed from [`DiscountCurveSolveConfig`].
///
/// # Examples
/// ```
/// use finstack_quant_calibration::ForwardCurveSolveConfig;
///
/// let config = ForwardCurveSolveConfig {
///     validation_tolerance: 1e-6,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ForwardCurveSolveConfig {
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

impl Default for ForwardCurveSolveConfig {
    fn default() -> Self {
        Self {
            weighting_scheme: ResidualWeightingScheme::default(),
            validation_tolerance: 1e-8,
        }
    }
}

/// Selected side of the market snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum MarketQuoteSide {
    /// Mid-market observations.
    #[default]
    Mid,
    /// Executable bid-side observations.
    Bid,
    /// Executable ask-side observations.
    Ask,
}

/// Audit metadata and freshness policy for calibration inputs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct MarketFreshnessPolicy {
    /// RFC3339 timestamp at which the quote snapshot was captured.
    pub snapshot_timestamp: Option<String>,
    /// Maximum permitted snapshot age in seconds.
    pub max_age_seconds: Option<u64>,
    /// Market side represented by quote values in the envelope.
    pub quote_side: MarketQuoteSide,
}

impl MarketFreshnessPolicy {
    fn validate(&self) -> finstack_quant_core::Result<()> {
        match (&self.snapshot_timestamp, self.max_age_seconds) {
            (None, None) => Ok(()),
            (Some(timestamp), Some(max_age_seconds)) if max_age_seconds > 0 => {
                let captured = time::OffsetDateTime::parse(
                    timestamp,
                    &time::format_description::well_known::Rfc3339,
                )
                .map_err(|error| {
                    finstack_quant_core::Error::Validation(format!(
                        "market snapshot timestamp must be RFC3339: {error}"
                    ))
                })?;
                let now = time::OffsetDateTime::now_utc();
                if captured > now {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "market snapshot timestamp {timestamp} is in the future"
                    )));
                }
                let age_seconds = (now - captured).whole_seconds();
                if age_seconds > max_age_seconds as i64 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "market snapshot is {age_seconds} seconds old, exceeding max_age_seconds={max_age_seconds}"
                    )));
                }
                Ok(())
            }
            (Some(_), Some(0)) => Err(finstack_quant_core::Error::Validation(
                "market freshness max_age_seconds must be positive".to_string(),
            )),
            _ => Err(finstack_quant_core::Error::Validation(
                "market freshness requires snapshot_timestamp and max_age_seconds together"
                    .to_string(),
            )),
        }
    }

    /// Whether the snapshot carries a complete, validated freshness assertion.
    #[must_use]
    pub fn is_verifiable(&self) -> bool {
        self.snapshot_timestamp.is_some() && self.max_age_seconds.is_some()
    }
}

fn market_freshness_is_default(policy: &MarketFreshnessPolicy) -> bool {
    policy.snapshot_timestamp.is_none()
        && policy.max_age_seconds.is_none()
        && policy.quote_side == MarketQuoteSide::Mid
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
/// 2. **Validation Tolerance** (e.g., [`discount_curve.validation_tolerance`](DiscountCurveSolveConfig::validation_tolerance)
///    for PV-per-notional curve residuals, or [`vol_surface.validation_tolerance`](VolSurfaceSolveConfig::validation_tolerance)
///    for decimal implied-vol residuals):
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
/// 3. **Finstack config extensions** (`calibration.config.v1`): application defaults
/// 4. **Global defaults** (`CalibrationConfig::default()`): fallback values
///
/// Step-level settings always take precedence over plan-level settings.
/// In other words, this struct provides default policy, but explicit plan steps
/// remain authoritative when both are supplied.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_calibration::CalibrationConfig;
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct CalibrationConfig {
    /// Solver configuration including numerical method (e.g., Brent) and parameters (tolerance, iterations).
    pub solver: SolverConfig,
    /// Use parallel processing when available (e.g., for independent curves).
    pub use_parallel: bool,
    /// Enable verbose logging of the calibration process.
    pub verbose: bool,
    /// Explanation options (opt-in detailed trace for debugging).
    #[serde(skip)]
    pub explain: ExplainOpts,
    /// Runtime validation mode (warnings vs errors).
    pub validation_mode: ValidationMode,
    /// Validation configuration with thresholds and quality checks.
    #[serde(default)]
    pub validation: crate::validation::ValidationConfig,
    /// Policy for selecting rate bounds (explicit vs currency-derived).
    #[serde(default = "crate::validation::default_rate_bounds_policy_for_serde")]
    pub rate_bounds_policy: RateBoundsPolicy,
    /// Rate bounds for forward/zero rate calibration (when policy is `Explicit`).
    #[serde(default)]
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
    pub discount_curve: DiscountCurveSolveConfig,

    /// Forward-curve specific solver configuration.
    #[serde(default)]
    pub forward_curve: ForwardCurveSolveConfig,

    /// Hazard-curve specific solver configuration.
    #[serde(default)]
    pub hazard_curve: HazardCurveSolveConfig,

    /// Inflation-curve specific solver configuration.
    #[serde(default)]
    pub inflation_curve: InflationCurveSolveConfig,

    /// Volatility-surface specific solver configuration (SABR and SVI).
    #[serde(default)]
    pub vol_surface: VolSurfaceSolveConfig,

    /// When `true`, a calibration step whose solver reports
    /// `report.success == false` is propagated as a
    /// `finstack_quant_core::Error::Calibration` and its output is **not**
    /// installed into the market context.
    ///
    /// Defaults to `true` — this is the safe production choice because
    /// a non-converged solver would otherwise silently poison downstream
    /// pricing. Diagnostic workflows that want to inspect the
    /// report without aborting can set this to `false`.
    #[serde(default = "default_fail_on_bad_fit")]
    pub fail_on_bad_fit: bool,

    /// FX matrix runtime config (pivot currency, triangulation, cache capacity).
    #[serde(default)]
    pub fx: FxConfig,
    /// Snapshot timestamp, maximum age, and selected quote side.
    #[serde(default, skip_serializing_if = "market_freshness_is_default")]
    pub market_freshness: MarketFreshnessPolicy,

    /// Optional market-data hierarchy snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hierarchy: Option<MarketDataHierarchy>,
}

fn default_fail_on_bad_fit() -> bool {
    true
}

/// Extension section key for calibration overrides.
pub const CALIBRATION_CONFIG_KEY: &str = "calibration.config.v1";

/// Recursively overlay `overlay` onto `base` (objects merge, everything else replaces).
fn merge_json(base: &mut serde_json::Value, overlay: serde_json::Value) {
    match (base, overlay) {
        (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) => {
            for (key, value) in overlay_map {
                match base_map.get_mut(&key) {
                    Some(existing) => merge_json(existing, value),
                    None => {
                        base_map.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        Self {
            solver: SolverConfig::default(),
            use_parallel: false, // Deterministic by default
            verbose: false,
            explain: ExplainOpts::default(), // Disabled by default (zero overhead)
            validation_mode: ValidationMode::Error,
            validation: crate::validation::ValidationConfig::default(),
            rate_bounds_policy: RateBoundsPolicy::AutoCurrency,
            rate_bounds: RateBounds::default(),
            calibration_method: CalibrationMethod::default(),
            compute_diagnostics: false,
            discount_curve: DiscountCurveSolveConfig::default(),
            forward_curve: ForwardCurveSolveConfig::default(),
            hazard_curve: HazardCurveSolveConfig::default(),
            inflation_curve: InflationCurveSolveConfig::default(),
            vol_surface: VolSurfaceSolveConfig::default(),
            fail_on_bad_fit: default_fail_on_bad_fit(),
            fx: FxConfig::default(),
            market_freshness: MarketFreshnessPolicy::default(),
            hierarchy: None,
        }
    }
}

impl CalibrationConfig {
    /// Build a calibration config from a `FinstackConfig` extension section.
    ///
    /// If the extension section `calibration.config.v1` is present, its
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
    /// use finstack_quant_calibration::CalibrationConfig;
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
    /// Returns an error when the solver settings are unusable
    /// ([`SolverConfig::validate`](crate::SolverConfig::validate)), nested
    /// validation/rate-bound settings are invalid, or the solver tolerance is
    /// looser than residual success tolerances.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        self.solver.validate()?;
        self.validation.validate()?;
        self.rate_bounds.validate()?;
        self.market_freshness.validate()?;
        self.validate_solver_vs_success_tolerance(
            "discount_curve.validation_tolerance",
            self.discount_curve.validation_tolerance,
        )?;
        self.validate_solver_vs_success_tolerance(
            "forward_curve.validation_tolerance",
            self.forward_curve.validation_tolerance,
        )?;
        self.validate_solver_vs_success_tolerance(
            "hazard_curve.validation_tolerance",
            self.hazard_curve.validation_tolerance,
        )?;
        self.validate_solver_vs_success_tolerance(
            "inflation_curve.validation_tolerance",
            self.inflation_curve.validation_tolerance,
        )?;
        self.validate_solver_vs_success_tolerance(
            "vol_surface.validation_tolerance",
            self.vol_surface.validation_tolerance,
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
    /// use finstack_quant_calibration::{CalibrationConfig, RateBounds};
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

    /// Overlay a partial JSON object onto this configuration.
    ///
    /// Objects merge recursively (only the supplied leaves change); arrays
    /// and scalars replace the existing value. Unknown keys are rejected by
    /// the strict deserialization of the merged document.
    ///
    /// # Arguments
    ///
    /// * `overrides` - JSON object whose keys mirror the serialized
    ///   `CalibrationConfig` field names (e.g. `{"solver": {"tolerance": 1e-10},
    ///   "use_parallel": false}`).
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when `overrides` is
    /// not a JSON object, names an unknown field, or the merged document fails
    /// [`Self::validate`].
    pub fn with_json_overrides(
        &self,
        overrides: serde_json::Value,
    ) -> finstack_quant_core::Result<Self> {
        if !overrides.is_object() {
            return Err(finstack_quant_core::Error::Validation(
                "calibration config overrides must be a JSON object".to_string(),
            ));
        }
        let mut base = serde_json::to_value(self).map_err(|e| {
            finstack_quant_core::Error::Validation(format!(
                "failed to serialize calibration config: {e}"
            ))
        })?;
        merge_json(&mut base, overrides);
        let merged: Self = serde_json::from_value(base).map_err(|e| {
            finstack_quant_core::Error::Validation(format!("invalid calibration config: {e}"))
        })?;
        merged.validate()?;
        Ok(merged)
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RatesStepConventions {
    /// Day count used to map dates to year fractions for curve knot times.
    #[serde(default)]
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
    pub ois_compounding:
        Option<finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_solve_json_requires_explicit_jacobian_policy() {
        let json = serde_json::json!({ "global_solve": {} });
        assert!(serde_json::from_value::<CalibrationMethod>(json).is_err());
    }

    #[test]
    fn discount_solve_config_has_no_curve_shape_knobs() {
        // Curve interpolation and extrapolation belong to the discount step
        // (`DiscountCurveParams`); the solve config never read them.
        let serialized =
            serde_json::to_value(DiscountCurveSolveConfig::default()).expect("serializes");
        assert!(serialized.get("interp_style").is_none());
        assert!(serialized.get("extrapolation_policy").is_none());
        for key in ["interp_style", "extrapolation_policy"] {
            let json = serde_json::json!({ key: "linear" });
            assert!(
                serde_json::from_value::<DiscountCurveSolveConfig>(json).is_err(),
                "{key} must be rejected as an unknown field"
            );
        }
    }

    #[test]
    fn freshness_age_is_a_plain_json_number() {
        let policy: MarketFreshnessPolicy =
            serde_json::from_value(serde_json::json!({ "max_age_seconds": 3600 }))
                .expect("integer age parses");
        assert_eq!(policy.max_age_seconds, Some(3600));
        assert_eq!(
            serde_json::to_value(&policy).expect("serializes")["max_age_seconds"],
            serde_json::json!(3600)
        );
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

    #[test]
    fn vol_surface_default_validation_tolerance_is_one_tenth_vol_point() {
        let cfg = CalibrationConfig::default();
        assert!((cfg.vol_surface.validation_tolerance - 1e-3).abs() < 1e-15);
        cfg.validate()
            .expect("default vol-surface tolerance must be compatible with the solver");
    }

    #[test]
    fn omitted_vol_surface_section_uses_defaults() {
        let json = r#"{}"#;
        let cfg: CalibrationConfig = serde_json::from_str(json).expect("empty JSON");
        assert!((cfg.vol_surface.validation_tolerance - 1e-3).abs() < 1e-15);
    }

    #[test]
    fn omitted_forward_curve_section_uses_defaults() {
        let json = r#"{}"#;
        let cfg: CalibrationConfig = serde_json::from_str(json).expect("empty JSON");
        assert!((cfg.forward_curve.validation_tolerance - 1e-8).abs() < 1e-15);
        assert_eq!(
            cfg.forward_curve.weighting_scheme,
            ResidualWeightingScheme::default()
        );
    }

    #[test]
    fn forward_curve_validation_tolerance_must_dominate_solver() {
        let mut cfg = CalibrationConfig::default();
        cfg.forward_curve.validation_tolerance = 1e-14;
        let err = cfg
            .validate()
            .expect_err("solver tolerance looser than forward-curve success tolerance should fail");
        assert!(
            err.to_string()
                .contains("forward_curve.validation_tolerance"),
            "unexpected validation error: {err}"
        );
    }

    #[test]
    fn vol_surface_validation_tolerance_must_dominate_solver() {
        let mut cfg = CalibrationConfig::default();
        cfg.vol_surface.validation_tolerance = 1e-14;
        let err = cfg
            .validate()
            .expect_err("solver tolerance looser than vol-surface success tolerance should fail");
        assert!(
            err.to_string().contains("vol_surface.validation_tolerance"),
            "unexpected validation error: {err}"
        );
    }
    #[test]
    fn market_freshness_requires_complete_valid_policy() {
        let mut incomplete = CalibrationConfig::default();
        incomplete.market_freshness.snapshot_timestamp = Some("2026-01-01T00:00:00Z".to_string());
        assert!(incomplete
            .validate()
            .expect_err("timestamp without max age")
            .to_string()
            .contains("together"));

        let mut malformed = CalibrationConfig::default();
        malformed.market_freshness.snapshot_timestamp = Some("not-a-timestamp".to_string());
        malformed.market_freshness.max_age_seconds = Some(60);
        assert!(malformed
            .validate()
            .expect_err("malformed timestamp")
            .to_string()
            .contains("RFC3339"));
    }

    #[test]
    fn validate_rejects_inverted_hazard_bounds() {
        let mut config = CalibrationConfig::default();
        config.hazard_curve.hazard_hard_min = 0.05;
        config.hazard_curve.hazard_hard_max = 0.01;

        let err = config
            .validate()
            .expect_err("inverted hazard bounds should be rejected");
        assert!(err.to_string().to_lowercase().contains("hazard"));
    }
}
