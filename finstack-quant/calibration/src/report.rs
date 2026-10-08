//! Calibration reporting and diagnostics.

use crate::constants::RESIDUAL_PENALTY_ABS_MIN;
use crate::solver::SolverConfig;
use finstack_quant_core::config::ResultsMeta;
use finstack_quant_core::explain::ExplanationTrace;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_true() -> bool {
    true
}

/// Numerical procedure that produced a [`CalibrationReport`].
///
/// The wire form is the snake_case variant name (for example
/// `"sequential_bootstrap"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SolverMethod {
    /// Knots solved one at a time in maturity order, each by a bracketed
    /// one-dimensional root find on its own quote (curve bootstraps and the
    /// piecewise Hull-White volatility bootstrap).
    SequentialBootstrap,
    /// All parameters solved simultaneously by weighted Levenberg-Marquardt
    /// least squares over every quote.
    GlobalFitLmWeightedLsq,
    /// One independent least-squares smile fit per expiry (or per
    /// expiry/tenor bucket), as used by the SABR and SVI surface calibrators.
    PerSliceLeastSquares,
    /// A single scalar parameter solved by a bracketed one-dimensional root
    /// find (Student-t degrees of freedom).
    ScalarRootFind,
    /// A single scalar parameter solved by one-dimensional minimisation of
    /// the sum of squared pricing errors (Hull-White volatility at a fixed
    /// mean reversion).
    ScalarMinimization,
    /// Aggregation of the step reports of a calibration plan; no solver ran
    /// at this level.
    PlanExecution,
}

impl SolverMethod {
    /// Wire name of this method, identical to its serde representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SequentialBootstrap => "sequential_bootstrap",
            Self::GlobalFitLmWeightedLsq => "global_fit_lm_weighted_lsq",
            Self::PerSliceLeastSquares => "per_slice_least_squares",
            Self::ScalarRootFind => "scalar_root_find",
            Self::ScalarMinimization => "scalar_minimization",
            Self::PlanExecution => "plan_execution",
        }
    }
}

/// Units of the residuals in [`CalibrationReport::residuals`],
/// [`CalibrationReport::max_residual`] and [`CalibrationReport::rmse`].
///
/// The wire form is the snake_case variant name (for example
/// `"pv_per_unit_notional"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ResidualUnits {
    /// Present value of the quote's calibration instrument on the calibrated
    /// curve, divided by the target's residual notional: a dimensionless
    /// fraction of notional (`1e-4` is one basis point of notional). Used by
    /// the discount, forward, hazard, inflation, cross-currency basis and
    /// parametric curve targets.
    PvPerUnitNotional,
    /// Model-implied volatility minus quoted volatility, in the quote's own
    /// volatility convention and decimal units (`0.01` is one volatility
    /// point for lognormal quotes; normal quotes are in rate units).
    QuotedVolatility,
    /// Discounted tranche present value divided by the tranche notional: the
    /// upfront error as a decimal fraction of tranche notional.
    DiscountedUpfrontFraction,
    /// `|residual| / step_tolerance`, dimensionless. Used by the plan-level
    /// report, whose residual map pools steps with different native units.
    AbsoluteResidualOverStepTolerance,
}

impl ResidualUnits {
    /// Wire name of these units, identical to the serde representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PvPerUnitNotional => "pv_per_unit_notional",
            Self::QuotedVolatility => "quoted_volatility",
            Self::DiscountedUpfrontFraction => "discounted_upfront_fraction",
            Self::AbsoluteResidualOverStepTolerance => "absolute_residual_over_step_tolerance",
        }
    }
}

/// Smile-model parameters fitted to one expiry slice of a volatility surface.
///
/// The wire form is externally tagged by model: `{"sabr": {...}}` or
/// `{"svi": {...}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FittedSliceParameters {
    /// SABR parameters (`alpha`, `beta`, `nu`, `rho` and the optional
    /// displacement `shift`).
    Sabr(finstack_quant_models::volatility::sabr::SabrParameters),
    /// Raw SVI total-variance parameters (`a`, `b`, `rho`, `m`, `sigma`) in
    /// log-moneyness `ln(K / F)`.
    Svi(finstack_quant_models::volatility::svi::SviParams),
}

/// Parameters a surface calibrator fitted to the quotes of one expiry.
///
/// The SABR and SVI surface calibrators fit one smile per quoted expiry and
/// then sample those smiles onto the surface's target grid. The gridded
/// surface keeps only the sampled volatilities, so the fitted parameters are
/// recorded here, one entry per quoted expiry in increasing expiry order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct FittedSlice {
    /// Expiry of the quotes this slice was fitted to, as an Act/365F year
    /// fraction from the calibration base date. This is a quoted expiry, not
    /// necessarily a node of the surface's target expiry grid.
    pub expiry: f64,
    /// Fitted model parameters for this expiry.
    pub parameters: FittedSliceParameters,
    /// Solver iterations spent on this slice (all attempted starts for SABR).
    /// The slice iterations sum to [`CalibrationReport::iterations`].
    pub iterations: usize,
    /// Largest `|model − market| / market` volatility error of the fitted
    /// smile over this slice's quotes, as a decimal fraction of the quoted
    /// volatility (`1e-4` is 0.01%). It measures the smile fit itself; the
    /// report's residuals measure the gridded surface instead.
    ///
    /// Present for SABR slices. Absent for SVI slices, whose fit reports no
    /// per-slice error.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_relative_quote_error: Option<f64>,
}

/// Per-quote quality metrics from a calibration run.
///
/// Captures the market quote, the residual left after calibration and a local
/// sensitivity measure for a single calibration instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct QuoteQuality {
    /// Human-readable label identifying this quote (e.g., "USD-1Y-SWAP").
    pub quote_label: String,
    /// Market quote the calibration instrument was built from, in the quote's
    /// native units: decimal rate for deposits, FRAs, swaps and inflation
    /// swaps; price for rate futures; basis points for CDS par spreads and
    /// cross-currency basis; percent for CDS and tranche upfronts.
    ///
    /// Absent when the target's quotes carry no single scalar value or
    /// diagnostics were not computed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote_value: Option<f64>,
    /// Signed calibration residual: the value of the instrument built at
    /// `quote_value`, priced on the calibrated curve and divided by the
    /// target's residual notional. The calibration target is zero.
    pub residual: f64,
    /// Local sensitivity: dOutput/dParam (via finite difference or Jacobian diagonal).
    pub sensitivity: f64,
}

/// Calibration diagnostics providing condition number, residual analysis, and fit quality.
///
/// These diagnostics are only populated when `CalibrationConfig::compute_diagnostics`
/// is set to `true`. They are relatively expensive to compute (requiring Jacobian
/// analysis) and are intended for calibration debugging, auditing, and quality
/// monitoring rather than production hot paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationDiagnostics {
    /// Per-quote quality metrics for each calibration instrument.
    pub per_quote: Vec<QuoteQuality>,
    /// Condition number of the weighted Jacobian's normal equations (J^T * W * J).
    /// `W` contains the configured residual weights; unweighted fits use identity.
    ///
    /// A high condition number (e.g., > 1e10) indicates an ill-conditioned
    /// calibration problem where small changes in market data can produce
    /// large changes in calibrated parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_number: Option<f64>,
    /// Finite-difference Jacobian of the residuals at the solution.
    ///
    /// `jacobian[i][j]` is the derivative of the residual of quote `i` with
    /// respect to solved parameter `j`: rows follow [`Self::per_quote`] order,
    /// columns follow the solved parameters in knot order (for curve targets,
    /// the knot values at increasing knot times). Entries are in the report's
    /// residual units per unit of the solved parameter, and are unweighted;
    /// [`Self::condition_number`] is computed from this matrix with the
    /// residual weights applied. `per_quote[i].sensitivity` equals
    /// `max_j |jacobian[i][j]|`.
    ///
    /// Present only for global (simultaneous) solves in which every parameter
    /// bump could be priced. Absent for sequential bootstraps, which solve one
    /// knot per quote and build no full Jacobian.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jacobian: Option<Vec<Vec<f64>>>,
    /// Maximum absolute residual across all quotes.
    pub max_residual: f64,
    /// Root mean square residual across all quotes.
    pub rms_residual: f64,
}

impl CalibrationDiagnostics {
    /// Compute basic diagnostics from a vector of residuals.
    ///
    /// This is a lightweight computation that does not require the Jacobian.
    /// The condition number and the Jacobian are left as `None`.
    pub fn from_residuals(residuals: &[f64]) -> Self {
        let n = residuals.len();
        let max_residual = residuals.iter().map(|r| r.abs()).fold(0.0_f64, f64::max);
        let rms_residual = if n > 0 {
            (residuals.iter().map(|r| r * r).sum::<f64>() / n as f64).sqrt()
        } else {
            0.0
        };

        Self {
            per_quote: Vec::new(),
            condition_number: None,
            jacobian: None,
            max_residual,
            rms_residual,
        }
    }
}

/// Diagnostics computed from residual values.
///
/// Provides a statistical summary of the instrument fitting errors.
#[derive(Debug)]
struct ResidualDiagnostics {
    /// Maximum absolute residual across all instruments.
    max_residual: f64,
    /// Root mean square error of all residuals.
    rmse: f64,
    /// Whether any residual was a penalty value (solver failure).
    has_penalty: bool,
}

/// Identifier and signed residual of the worst-fitting quote in a residual map.
///
/// "Worst" is the largest absolute value; penalty sentinels are preferred
/// over normal residuals so that a step that drove a single quote to a
/// penalty (e.g. a CDS leg that returned NaN) surfaces *that* quote as the
/// worst-fitter rather than the next-largest finite residual.
fn worst_quote(residuals: &BTreeMap<String, f64>) -> Option<(String, f64)> {
    residuals
        .iter()
        .max_by(|(_, a), (_, b)| {
            let ord = |v: f64| {
                if !v.is_finite() {
                    f64::INFINITY
                } else {
                    v.abs()
                }
            };
            ord(**a)
                .partial_cmp(&ord(**b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(id, r)| (id.clone(), *r))
}

/// Filter out penalty sentinel values and compute common diagnostics.
///
/// Penalties (INFINITY or values >= `PENALTY` * 0.5) are
/// excluded from max/RMSE unless ALL values are penalties.
///
/// # Arguments
/// * `residuals` - Map of instrument identifiers to their final residual values.
///
/// # Returns
/// A [`ResidualDiagnostics`] struct containing max, RMSE, and penalty status.
fn compute_residual_diagnostics(residuals: &BTreeMap<String, f64>) -> ResidualDiagnostics {
    let penalty_abs_min = RESIDUAL_PENALTY_ABS_MIN;

    // PERF: single pass, no allocation. Track both:
    // - "valid" residuals: finite and non-penalty
    // - fallbacks: all residuals (including penalties) if no valid exist
    let mut has_penalty = false;

    let mut max_abs_all = 0.0_f64;
    let mut sum_sq_all = 0.0_f64;
    let mut n_all = 0usize;

    let mut max_abs_valid = 0.0_f64;
    let mut sum_sq_valid = 0.0_f64;
    let mut n_valid = 0usize;

    for &r in residuals.values() {
        n_all += 1;
        let abs = r.abs();
        if abs > max_abs_all {
            max_abs_all = abs;
        }
        sum_sq_all += r * r;

        let is_penalty = !r.is_finite() || abs >= penalty_abs_min;
        has_penalty |= is_penalty;

        if !is_penalty {
            n_valid += 1;
            if abs > max_abs_valid {
                max_abs_valid = abs;
            }
            sum_sq_valid += r * r;
        }
    }

    let max_residual = if n_valid > 0 {
        max_abs_valid
    } else {
        max_abs_all
    };

    let rmse = if n_all == 0 {
        0.0
    } else if n_valid > 0 {
        (sum_sq_valid / n_valid as f64).sqrt()
    } else {
        (sum_sq_all / n_all as f64).sqrt()
    };

    ResidualDiagnostics {
        max_residual,
        rmse,
        has_penalty,
    }
}

/// Detailed report of a calibration exercise.
///
/// Consolidates success status, residuals, convergence diagnostics, and optional
/// tracing information. Used by the calibration engine to return results and
/// by risk systems to audit calibration quality.
///
/// # Examples
/// ```rust
/// use finstack_quant_calibration::CalibrationReport;
/// use std::collections::BTreeMap;
///
/// let mut residuals = BTreeMap::new();
/// residuals.insert("1Y".to_string(), 1e-12);
///
/// let report = CalibrationReport::new(residuals, 10, true, "Converged");
/// assert!(report.success);
/// assert!(report.max_residual <= 1e-12);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationReport {
    /// User-facing success flag. True only if both fitting and validation passed.
    pub success: bool,
    /// Final residuals (fitting errors) by instrument identifier.
    pub residuals: BTreeMap<String, f64>,
    /// Number of solver iterations or function evaluations.
    pub iterations: usize,
    /// Final objective function value (usually RMSE).
    pub objective_value: f64,
    /// Maximum absolute residual across all instruments, in the residual
    /// units of the calibrator that produced this report (raw units for a
    /// step report; the largest step `max_residual` for a plan report).
    pub max_residual: f64,
    /// Root mean square error of all residuals, in the same units as
    /// [`Self::max_residual`].
    pub rmse: f64,
    /// Plan-level only: maximum `|residual| / step_tolerance` ratio across
    /// every quote of every step. `None` on per-step reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_residual_ratio: Option<f64>,
    /// Plan-level only: root mean square of `|residual| / step_tolerance`
    /// across every quote of every step. `None` on per-step reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rmse_ratio: Option<f64>,
    /// Whether the calibrated market object passed all validation checks.
    #[serde(default = "default_true")]
    pub validation_passed: bool,
    /// Optional details on validation failures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_error: Option<String>,
    /// Human-readable reason for convergence or failure.
    pub convergence_reason: String,
    /// Domain-specific metadata (e.g., "type": "yield_curve").
    pub metadata: BTreeMap<String, String>,
    /// Solver configuration used during this calibration run.
    #[serde(default)]
    pub solver_config: SolverConfig,
    /// Results metadata (timestamp, software version, etc.).
    #[serde(default)]
    pub results_meta: ResultsMeta,
    /// Optional detailed trace of the calibration steps (enabled via config).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<ExplanationTrace>,
    /// Optional model/methodology version used for this calibration.
    ///
    /// Used for audit trails and regulatory compliance. Examples:
    /// - "ISDA Standard Model v1.8.2" for CDS hazard curve calibration
    /// - "Multi-curve OIS discounting" for discount curve calibration
    /// - "SABR v1.0" for volatility surface calibration
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub model_version: Option<String>,
    /// Optional calibration diagnostics (condition number, per-quote quality, etc.).
    ///
    /// Only populated when `CalibrationConfig::compute_diagnostics` is `true`.
    /// Provides detailed information about the quality and stability of the
    /// calibration for debugging, auditing, and monitoring purposes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub diagnostics: Option<CalibrationDiagnostics>,

    /// Identifier of the quote with the largest absolute residual.
    ///
    /// Derived from `residuals`. `None` only when `residuals` is empty. This
    /// is the quote a user should look at first when a step fails to
    /// converge — the input most likely to fix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst_quote_id: Option<String>,

    /// Signed residual of [`Self::worst_quote_id`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worst_quote_residual: Option<f64>,

    /// Configured tolerance used to determine [`Self::success`] from residuals.
    ///
    /// Set by [`Self::for_type_with_tolerance`]; `None` for reports built via [`Self::new`]
    /// directly (no tolerance gate applied). This is the typed source of truth for the
    /// success-gate tolerance; it is distinct from `solver_config.tolerance()`, which controls
    /// when the numerical root-finder stops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub success_tolerance: Option<f64>,

    /// Numerical procedure that produced this report.
    ///
    /// Set by every calibrator in this crate. `None` only on reports built
    /// directly through [`Self::new`] or [`Self::for_type_with_tolerance`]
    /// without a solver, and on JSON written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub solver_method: Option<SolverMethod>,

    /// Units of [`Self::residuals`], [`Self::max_residual`] and [`Self::rmse`].
    ///
    /// Set by every calibrator in this crate and mirrored as the
    /// `residual_units` entry of [`Self::metadata`]. `None` only on reports
    /// built directly through [`Self::new`] or
    /// [`Self::for_type_with_tolerance`] without a solver, and on JSON
    /// written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_units: Option<ResidualUnits>,

    /// Smile parameters fitted per quoted expiry, in increasing expiry order.
    ///
    /// Populated by the equity SABR (`vol_surface`) and SVI (`svi_surface`)
    /// surface calibrators, whose gridded output surface does not retain the
    /// fitted parameters. Empty for every other calibrator, including the
    /// swaption SABR cube, which stores its node parameters on the cube.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fitted_slices: Vec<FittedSlice>,
}

impl CalibrationReport {
    /// Per-quote fit rows of this report.
    ///
    /// Returns the diagnostic rows (`quote_value`, `residual`, `sensitivity`)
    /// when `CalibrationConfig::compute_diagnostics` populated them. Otherwise
    /// it returns one row per entry of [`Self::residuals`], ordered by quote
    /// id, carrying the signed residual with no quote value and `NaN` for the
    /// sensitivity that was not recorded.
    pub fn quote_rows(&self) -> Vec<QuoteQuality> {
        if let Some(diagnostics) = &self.diagnostics {
            if !diagnostics.per_quote.is_empty() {
                return diagnostics.per_quote.clone();
            }
        }
        self.residuals
            .iter()
            .map(|(id, residual)| QuoteQuality {
                quote_label: id.clone(),
                quote_value: None,
                residual: *residual,
                sensitivity: f64::NAN,
            })
            .collect()
    }

    /// Convenience constructor covering the common case of a completed calibration.
    ///
    /// # Arguments
    ///
    /// * `residuals` - Quote identifiers mapped to signed calibration residuals in each quote's native units; consumed to compute residual diagnostics.
    /// * `iterations` - Number of solver iterations performed before this report was created.
    /// * `success` - Whether the caller's calibration success criteria were met; this constructor does not rerun the solver or apply a tolerance gate.
    /// * `convergence_reason` - Human-readable explanation of convergence or failure stored in the report.
    pub fn new(
        residuals: BTreeMap<String, f64>,
        iterations: usize,
        success: bool,
        convergence_reason: impl Into<String>,
    ) -> Self {
        let diag = compute_residual_diagnostics(&residuals);
        let (worst_quote_id, worst_quote_residual) = match worst_quote(&residuals) {
            Some((id, r)) => (Some(id), Some(r)),
            None => (None, None),
        };

        let results_meta = finstack_quant_core::config::results_meta(
            &finstack_quant_core::config::FinstackConfig::default(),
        );

        Self {
            success,
            residuals,
            iterations,
            // Default objective value is RMSE of residuals (penalty residuals excluded).
            // This is a generic, comparable scalar objective across calibrators. Individual
            // calibrators may overwrite this with a domain-specific objective via
            // `with_metadata(...)` or a future explicit objective setter.
            objective_value: diag.rmse,
            max_residual: diag.max_residual,
            rmse: diag.rmse,
            max_residual_ratio: None,
            rmse_ratio: None,
            validation_passed: true,
            validation_error: None,
            convergence_reason: convergence_reason.into(),
            metadata: BTreeMap::new(),
            solver_config: SolverConfig::default(),
            results_meta,
            explanation: None,
            model_version: None,
            diagnostics: None,
            worst_quote_id,
            worst_quote_residual,
            success_tolerance: None,
            solver_method: None,
            residual_units: None,
            fitted_slices: Vec::new(),
        }
    }

    /// Mark this report as a plan-level aggregation over `step_reports`.
    ///
    /// The residual map of a plan report is expressed as
    /// `|residual| / step_tolerance`, so the max/RMSE computed by
    /// [`Self::new`] are ratios. This method moves them to
    /// [`Self::max_residual_ratio`] / [`Self::rmse_ratio`] and recomputes
    /// [`Self::max_residual`] / [`Self::rmse`] from the raw per-step
    /// reports: the maximum step `max_residual` and the residual-count
    /// weighted RMSE of the step RMSEs.
    ///
    /// # Arguments
    ///
    /// * `step_reports` - Per-step reports (raw residual units) whose raw
    ///   statistics replace the ratio statistics on this report.
    #[must_use]
    pub fn with_plan_aggregation(
        mut self,
        step_reports: &BTreeMap<String, CalibrationReport>,
    ) -> Self {
        self.max_residual_ratio = Some(self.max_residual);
        self.rmse_ratio = Some(self.rmse);
        let mut max_raw = 0.0_f64;
        let mut sum_sq = 0.0_f64;
        let mut n_total = 0usize;
        for step in step_reports.values() {
            max_raw = max_raw.max(step.max_residual);
            let n = step.residuals.len();
            sum_sq += step.rmse * step.rmse * n as f64;
            n_total += n;
        }
        self.max_residual = max_raw;
        self.rmse = if n_total == 0 {
            0.0
        } else {
            (sum_sq / n_total as f64).sqrt()
        };
        self
    }

    /// Attach an explanation trace to this report.
    #[must_use]
    pub fn with_explanation(mut self, trace: ExplanationTrace) -> Self {
        self.explanation = Some(trace);
        self
    }

    /// Attach model/methodology version to this report.
    ///
    /// Used for audit trails and regulatory compliance. Examples:
    /// - "ISDA Standard Model v1.8.2" for CDS hazard curve calibration
    /// - "Multi-curve OIS discounting" for discount curve calibration
    #[must_use]
    pub fn with_model_version(mut self, version: impl Into<String>) -> Self {
        self.model_version = Some(version.into());
        self
    }

    /// Attach calibration diagnostics to this report.
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: CalibrationDiagnostics) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }

    /// Add metadata key-value pair to the report (builder pattern).
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Update metadata key-value pair on an existing report.
    pub fn update_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Attach the smile parameters fitted per quoted expiry.
    ///
    /// # Arguments
    ///
    /// * `fitted_slices` - One entry per quoted expiry, in increasing expiry
    ///   order; stored in [`Self::fitted_slices`].
    #[must_use]
    pub fn with_fitted_slices(mut self, fitted_slices: Vec<FittedSlice>) -> Self {
        self.fitted_slices = fitted_slices;
        self
    }

    /// Record the numerical procedure that produced this report.
    ///
    /// # Arguments
    ///
    /// * `method` - Solver procedure stored in [`Self::solver_method`].
    #[must_use]
    pub fn with_solver_method(mut self, method: SolverMethod) -> Self {
        self.solver_method = Some(method);
        self
    }

    /// Record the units of this report's residuals.
    ///
    /// Sets [`Self::residual_units`] and mirrors the wire name into the
    /// `residual_units` metadata entry so the typed field and the metadata
    /// cannot disagree.
    ///
    /// # Arguments
    ///
    /// * `units` - Units of [`Self::residuals`], [`Self::max_residual`] and
    ///   [`Self::rmse`].
    #[must_use]
    pub fn with_residual_units(mut self, units: ResidualUnits) -> Self {
        self.residual_units = Some(units);
        self.metadata
            .insert("residual_units".to_string(), units.as_str().to_string());
        self
    }

    /// Attach the typed success-gate tolerance used to determine [`Self::success`].
    #[must_use]
    pub fn with_success_tolerance(mut self, tolerance: f64) -> Self {
        self.success_tolerance = Some(tolerance);
        self
    }

    /// Attach validation outcome. If validation fails, the report is marked unsuccessful.
    #[must_use]
    pub fn with_validation_result(mut self, passed: bool, error: Option<String>) -> Self {
        self.validation_passed = passed;
        self.validation_error = error;

        if !self.validation_passed {
            self.success = false;
            if let Some(err) = &self.validation_error {
                if self.convergence_reason.contains("converged") {
                    self.convergence_reason =
                        format!("Converged to tolerance but failed validation: {err}");
                } else if !self.convergence_reason.contains("validation failed") {
                    self.convergence_reason =
                        format!("{}; validation failed: {err}", self.convergence_reason);
                }
            } else if self.convergence_reason.contains("converged") {
                self.convergence_reason =
                    "Converged to tolerance but failed validation".to_string();
            } else if !self.convergence_reason.contains("validation failed") {
                self.convergence_reason = format!("{}; validation failed", self.convergence_reason);
            }
        }

        self
    }

    /// Explicitly mark the report as having encountered penalty residuals during
    /// the solve (provenance-based penalty detection).
    ///
    /// The magnitude-based threshold in `compute_residual_diagnostics` cannot
    /// detect small penalty values produced by `fill_penalty` in the optimizer
    /// when bound violations are tiny (e.g., `PENALTY * d/(1+d) ≪ RESIDUAL_PENALTY_ABS_MIN`).
    /// Callers that KNOW bound violations occurred during solving (e.g., because their
    /// `clamp_to_bounds` count was non-zero) MUST call `with_has_penalty_residuals(true)` so
    /// the report is correctly flagged.
    ///
    /// When `has_penalty` is `true`:
    /// - `success` is set to `false`.
    /// - `convergence_reason` is updated to mention "penalty residuals encountered".
    ///
    /// When `has_penalty` is `false`, this is a no-op (does not reset a previously
    /// detected penalty).
    ///
    /// # Example
    /// ```
    /// use finstack_quant_calibration::CalibrationReport;
    /// use std::collections::BTreeMap;
    ///
    /// # let residuals = BTreeMap::new();
    /// # let n_clamped = 1usize;
    /// // If the solver clamped any params to bounds, flag the report.
    /// let report = CalibrationReport::for_type_with_tolerance("global_fit", residuals, 10, 1e-6)
    ///     .with_has_penalty_residuals(n_clamped > 0);
    /// ```
    #[must_use]
    pub fn with_has_penalty_residuals(mut self, has_penalty: bool) -> Self {
        if !has_penalty {
            return self;
        }
        self.success = false;
        if !self.convergence_reason.contains("penalty") {
            if self.convergence_reason.contains("converged") {
                self.convergence_reason = format!(
                    "{}: penalty residuals encountered during solve (bound violations detected)",
                    self.convergence_reason
                );
            } else {
                self.convergence_reason = format!(
                    "{}; penalty residuals encountered during solve (bound violations detected)",
                    self.convergence_reason
                );
            }
        }
        self
    }

    /// Update solver configuration on an existing report.
    pub fn update_solver_config(&mut self, config: SolverConfig) {
        self.solver_config = config;
        self.metadata.insert(
            "solver_tolerance".to_string(),
            format!("{:.2e}", self.solver_config.tolerance()),
        );
        self.metadata.insert(
            "solver_max_iterations".to_string(),
            self.solver_config.max_iterations().to_string(),
        );
    }

    /// Create a calibration report for a specific calibration type with tolerance checking.
    ///
    /// This method properly determines success/failure based on:
    /// - Whether any residuals contain PENALTY values (indicating hard failures)
    /// - Whether max_residual exceeds the configured tolerance
    ///
    /// # Arguments
    /// * `calibration_type` - Type identifier for the calibration (e.g., "yield_curve")
    /// * `residuals` - Map of instrument labels to their calibration residuals
    /// * `iterations` - Number of solver iterations performed
    /// * `tolerance` - Configured tolerance threshold for success determination
    ///
    /// # Example
    /// ```
    /// use finstack_quant_calibration::CalibrationReport;
    /// use std::collections::BTreeMap;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let residuals = BTreeMap::from([("DEP-1D".to_string(), 1e-6)]);
    /// let iterations = 5;
    /// let tolerance = 1e-4;
    ///
    /// let report = CalibrationReport::for_type_with_tolerance(
    ///     "yield_curve",
    ///     residuals,
    ///     iterations,
    ///     tolerance,
    /// );
    /// if !report.success {
    ///     return Err(finstack_quant_core::Error::Calibration {
    ///         message: report.convergence_reason.clone(),
    ///         category: "calibration".to_string(),
    ///     });
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn for_type_with_tolerance(
        calibration_type: impl Into<String>,
        residuals: BTreeMap<String, f64>,
        iterations: usize,
        tolerance: f64,
    ) -> Self {
        let type_str = calibration_type.into();

        if residuals.is_empty() {
            return Self::new(
                residuals,
                iterations,
                false,
                format!(
                    "{} calibration failed: no residuals were produced",
                    type_str.replace('_', " ")
                ),
            )
            .with_metadata("type", type_str)
            .with_success_tolerance(tolerance);
        }

        let diag = compute_residual_diagnostics(&residuals);

        let (success, convergence_reason) = if diag.has_penalty {
            let penalty_abs_min = RESIDUAL_PENALTY_ABS_MIN;
            let penalty_instruments: Vec<&str> = residuals
                .iter()
                .filter(|(_, r)| !r.is_finite() || r.abs() >= penalty_abs_min)
                .map(|(k, _)| k.as_str())
                .collect();
            (
                false,
                format!(
                    "{} calibration failed: penalty values detected for instruments: {:?}",
                    type_str.replace('_', " "),
                    penalty_instruments
                ),
            )
        } else if diag.max_residual > tolerance {
            (
                false,
                format!(
                    "{} calibration failed: max_residual ({:.2e}) exceeds tolerance ({:.2e})",
                    type_str.replace('_', " "),
                    diag.max_residual,
                    tolerance
                ),
            )
        } else {
            (
                true,
                format!(
                    "{} calibration converged: max_residual ({:.2e}) within tolerance ({:.2e})",
                    type_str.replace('_', " "),
                    diag.max_residual,
                    tolerance
                ),
            )
        };

        Self::new(residuals, iterations, success, convergence_reason)
            .with_metadata("type", type_str)
            .with_success_tolerance(tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::PENALTY;

    #[test]
    fn quote_rows_fall_back_to_residual_only_rows() {
        let residuals: BTreeMap<String, f64> =
            [("B".to_string(), -2e-6), ("A".to_string(), 1e-6)].into();
        let mut report = CalibrationReport::new(residuals, 3, true, "converged");
        let rows = report.quote_rows();
        assert_eq!(
            rows.iter()
                .map(|row| row.quote_label.as_str())
                .collect::<Vec<_>>(),
            ["A", "B"]
        );
        assert_eq!(rows[0].residual, 1e-6);
        assert!(rows[0].quote_value.is_none() && rows[0].sensitivity.is_nan());

        let measured = QuoteQuality {
            quote_label: "A".to_string(),
            quote_value: Some(0.05),
            residual: 1e-6,
            sensitivity: 2.0,
        };
        report.diagnostics = Some(CalibrationDiagnostics {
            per_quote: vec![measured],
            condition_number: None,
            jacobian: None,
            max_residual: 1e-6,
            rms_residual: 1e-6,
        });
        let rows = report.quote_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].quote_value, Some(0.05));
    }

    #[test]
    fn omitted_optional_report_fields_round_trip_as_absent_keys() {
        let report = CalibrationReport::new(BTreeMap::new(), 1, true, "converged");
        let value = serde_json::to_value(&report).expect("report serializes");
        for key in ["validation_error", "explanation"] {
            assert!(value.get(key).is_none(), "{key} is omitted, not null");
        }
        let back: CalibrationReport = serde_json::from_value(value).expect("absent keys default");
        assert!(back.validation_error.is_none());
        assert!(back.explanation.is_none());
    }

    #[test]
    fn test_for_type_with_tolerance_success() {
        // All residuals within tolerance
        let mut residuals = BTreeMap::new();
        residuals.insert("quote_1Y".to_string(), 1e-10);
        residuals.insert("quote_2Y".to_string(), 5e-11);
        residuals.insert("quote_5Y".to_string(), 2e-10);

        let report = CalibrationReport::for_type_with_tolerance("yield_curve", residuals, 10, 1e-8);

        assert!(
            report.success,
            "Should succeed when all residuals within tolerance"
        );
        assert!(
            report.convergence_reason.contains("converged"),
            "Reason should indicate convergence: {}",
            report.convergence_reason
        );
        assert!(
            report.max_residual < 1e-8,
            "Max residual should be computed correctly"
        );
    }

    #[test]
    fn test_for_type_with_tolerance_fails_exceeds_tolerance() {
        // One residual exceeds tolerance
        let mut residuals = BTreeMap::new();
        residuals.insert("quote_1Y".to_string(), 1e-10);
        residuals.insert("quote_2Y".to_string(), 1e-6); // Exceeds 1e-8 tolerance
        residuals.insert("quote_5Y".to_string(), 2e-10);

        let report = CalibrationReport::for_type_with_tolerance("yield_curve", residuals, 10, 1e-8);

        assert!(
            !report.success,
            "Should fail when residual exceeds tolerance"
        );
        assert!(
            report.convergence_reason.contains("failed"),
            "Reason should indicate failure: {}",
            report.convergence_reason
        );
        assert!(
            report.convergence_reason.contains("exceeds tolerance"),
            "Reason should explain the tolerance breach: {}",
            report.convergence_reason
        );
    }

    #[test]
    fn test_for_type_with_tolerance_fails_penalty_values() {
        // One residual contains PENALTY value indicating solver failure
        let penalty = PENALTY;
        let mut residuals = BTreeMap::new();
        residuals.insert("quote_1Y".to_string(), 1e-10);
        residuals.insert("quote_2Y_failed".to_string(), penalty); // PENALTY value
        residuals.insert("quote_5Y".to_string(), 2e-10);

        let report = CalibrationReport::for_type_with_tolerance("yield_curve", residuals, 10, 1e-8);

        assert!(!report.success, "Should fail when PENALTY value present");
        assert!(
            report.convergence_reason.contains("failed"),
            "Reason should indicate failure: {}",
            report.convergence_reason
        );
        assert!(
            report.convergence_reason.contains("penalty"),
            "Reason should mention penalty values: {}",
            report.convergence_reason
        );
        assert!(
            report.convergence_reason.contains("quote_2Y_failed"),
            "Reason should identify the failing instrument: {}",
            report.convergence_reason
        );
    }

    #[test]
    fn test_for_type_with_tolerance_fails_non_finite() {
        // Non-finite residual (infinity)
        let mut residuals = BTreeMap::new();
        residuals.insert("quote_1Y".to_string(), 1e-10);
        residuals.insert("quote_2Y_inf".to_string(), f64::INFINITY);
        residuals.insert("quote_5Y".to_string(), 2e-10);

        let report = CalibrationReport::for_type_with_tolerance("yield_curve", residuals, 10, 1e-8);

        assert!(!report.success, "Should fail when infinity present");
        assert!(
            report.convergence_reason.contains("penalty"),
            "Non-finite values should be treated as penalty failures: {}",
            report.convergence_reason
        );
    }

    #[test]
    fn test_for_type_with_tolerance_serialization() {
        let mut residuals = BTreeMap::new();
        residuals.insert("quote_1Y".to_string(), 1e-10);

        let report = CalibrationReport::for_type_with_tolerance("yield_curve", residuals, 10, 1e-8);

        // Test JSON round-trip
        let json = serde_json::to_string(&report).expect("Serialization should succeed");
        let deserialized: CalibrationReport =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(report.success, deserialized.success);
        assert_eq!(report.convergence_reason, deserialized.convergence_reason);
        assert_eq!(report.success_tolerance, deserialized.success_tolerance);
    }

    #[test]
    fn diagnostics_serde_roundtrip() {
        let diagnostics = CalibrationDiagnostics {
            per_quote: vec![
                QuoteQuality {
                    quote_label: "USD-1Y-SWAP".to_string(),
                    quote_value: Some(0.05),
                    residual: 1e-7,
                    sensitivity: 12.5,
                },
                QuoteQuality {
                    quote_label: "USD-5Y-SWAP".to_string(),
                    quote_value: Some(0.06),
                    residual: -2e-7,
                    sensitivity: 8.3,
                },
            ],
            condition_number: Some(1234.5),
            jacobian: Some(vec![vec![5.2, 0.1], vec![0.0, 8.3]]),
            max_residual: 2e-7,
            rms_residual: 1.58e-7,
        };

        let json = serde_json::to_string(&diagnostics).expect("Serialization should succeed");
        let deser: CalibrationDiagnostics =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(deser.per_quote.len(), 2);
        assert_eq!(deser.per_quote[0].quote_label, "USD-1Y-SWAP");
        assert_eq!(deser.per_quote[1].quote_label, "USD-5Y-SWAP");
        assert!((deser.per_quote[0].residual - 1e-7).abs() < 1e-15);
        assert!((deser.per_quote[1].sensitivity - 8.3).abs() < 1e-10);
        assert!((deser.condition_number.expect("condition_number") - 1234.5).abs() < 1e-10);
        assert_eq!(
            deser.jacobian.as_ref().expect("jacobian"),
            &vec![vec![5.2, 0.1], vec![0.0, 8.3]]
        );
        assert!((deser.max_residual - 2e-7).abs() < 1e-15);
        assert!((deser.rms_residual - 1.58e-7).abs() < 1e-15);
    }

    #[test]
    fn diagnostics_serde_roundtrip_with_none_fields() {
        let diagnostics = CalibrationDiagnostics {
            per_quote: vec![],
            condition_number: None,
            jacobian: None,
            max_residual: 0.0,
            rms_residual: 0.0,
        };

        let json = serde_json::to_string(&diagnostics).expect("Serialization should succeed");
        // Verify that None fields are skipped in JSON.
        assert!(!json.contains("condition_number"));
        assert!(!json.contains("jacobian"));

        let deser: CalibrationDiagnostics =
            serde_json::from_str(&json).expect("Deserialization should succeed");
        assert!(deser.condition_number.is_none());
        assert!(deser.jacobian.is_none());
    }

    #[test]
    fn rms_residual_computation_is_correct() {
        let residuals = vec![3.0, 4.0];
        let diag = CalibrationDiagnostics::from_residuals(&residuals);

        // RMS of [3.0, 4.0] = sqrt((9 + 16) / 2) = sqrt(12.5) = 3.5355...
        let expected_rms = (12.5_f64).sqrt();
        assert!(
            (diag.rms_residual - expected_rms).abs() < 1e-12,
            "Expected RMS {expected_rms}, got {}",
            diag.rms_residual
        );
        assert!((diag.max_residual - 4.0).abs() < 1e-12);
    }

    #[test]
    fn rms_residual_computation_single_value() {
        let residuals = vec![5.0];
        let diag = CalibrationDiagnostics::from_residuals(&residuals);
        assert!((diag.rms_residual - 5.0).abs() < 1e-12);
        assert!((diag.max_residual - 5.0).abs() < 1e-12);
    }

    #[test]
    fn rms_residual_computation_empty() {
        let residuals: Vec<f64> = vec![];
        let diag = CalibrationDiagnostics::from_residuals(&residuals);
        assert!((diag.rms_residual - 0.0).abs() < 1e-12);
        assert!((diag.max_residual - 0.0).abs() < 1e-12);
    }

    #[test]
    fn diagnostics_none_when_compute_diagnostics_false() {
        // Default CalibrationReport should have diagnostics = None.
        let mut residuals = BTreeMap::new();
        residuals.insert("1Y".to_string(), 1e-12);
        let report = CalibrationReport::new(residuals, 10, true, "Converged");
        assert!(
            report.diagnostics.is_none(),
            "Diagnostics should be None by default (compute_diagnostics = false)"
        );
    }

    #[test]
    fn diagnostics_none_when_compute_diagnostics_false_for_type() {
        let mut residuals = BTreeMap::new();
        residuals.insert("quote_1Y".to_string(), 1e-10);
        let report = CalibrationReport::for_type_with_tolerance("yield_curve", residuals, 10, 1e-8);
        assert!(
            report.diagnostics.is_none(),
            "for_type_with_tolerance should not produce diagnostics"
        );
    }

    #[test]
    fn report_with_diagnostics_roundtrip() {
        let mut residuals = BTreeMap::new();
        residuals.insert("1Y".to_string(), 1e-10);

        let diagnostics = CalibrationDiagnostics {
            per_quote: vec![QuoteQuality {
                quote_label: "1Y".to_string(),
                quote_value: Some(0.0),
                residual: 1e-10,
                sensitivity: 1.0,
            }],
            condition_number: Some(42.0),
            jacobian: None,
            max_residual: 1e-10,
            rms_residual: 1e-10,
        };

        let report =
            CalibrationReport::new(residuals, 10, true, "Converged").with_diagnostics(diagnostics);

        let json = serde_json::to_string(&report).expect("Serialization should succeed");
        let deser: CalibrationReport =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert!(deser.diagnostics.is_some());
        let d = deser.diagnostics.expect("diagnostics");
        assert_eq!(d.per_quote.len(), 1);
        assert_eq!(d.per_quote[0].quote_label, "1Y");
        assert!((d.condition_number.expect("condition_number") - 42.0).abs() < 1e-10);
    }

    #[test]
    fn quote_quality_struct_construction_and_access() {
        let qq = QuoteQuality {
            quote_label: "EUR-3M-DEPOSIT".to_string(),
            quote_value: Some(0.025),
            residual: 3e-7,
            sensitivity: 15.2,
        };

        assert_eq!(qq.quote_label, "EUR-3M-DEPOSIT");
        assert_eq!(qq.quote_value, Some(0.025));
        assert!((qq.residual - 3e-7).abs() < 1e-15);
        assert!((qq.sensitivity - 15.2).abs() < 1e-10);

        // Verify Clone works.
        let cloned = qq.clone();
        assert_eq!(cloned.quote_label, qq.quote_label);
        assert!((cloned.residual - qq.residual).abs() < 1e-15);
    }

    // Penalty-flag regression tests.
    //
    // `fill_penalty` in `global.rs` computes `PENALTY * (d/(1+d))` for a small
    // bound violation `d`. For tiny `d`, this value can be much less than
    // `RESIDUAL_PENALTY_ABS_MIN = PENALTY * 0.5 = 5e5`. The threshold-only
    // detection in `compute_residual_diagnostics` then misses these "small penalty"
    // residuals and reports a successful calibration with a misleading RMSE.
    //
    // The fix adds `CalibrationReport::with_has_penalty_residuals(bool)` so that
    // callers who KNOW bounds were violated can mark the report explicitly, without
    // relying on magnitude alone. The report honors this flag over the threshold.

    /// A residual smaller than `RESIDUAL_PENALTY_ABS_MIN` but produced by
    /// `fill_penalty` is not recognized by the threshold check alone.
    #[test]
    fn small_penalty_residual_not_recognized_without_explicit_flag() {
        // Simulate a tiny bound violation: d=1e-4, penalty = PENALTY * d/(1+d) ≈ 100.
        let d = 1e-4_f64;
        let small_penalty = PENALTY * (d / (1.0 + d));

        // Verify this value is below the threshold (confirming the gap exists).
        assert!(
            small_penalty < crate::constants::RESIDUAL_PENALTY_ABS_MIN,
            "test setup: small_penalty={small_penalty} should be below RESIDUAL_PENALTY_ABS_MIN={}",
            crate::constants::RESIDUAL_PENALTY_ABS_MIN
        );

        let mut residuals = BTreeMap::new();
        residuals.insert("quote_1Y".to_string(), 1e-10); // normal
        residuals.insert("quote_2Y_penalty".to_string(), small_penalty); // small penalty

        // Without the fix: report does NOT flag this as a penalty; it shows a
        // "large residual" failure but the convergence_reason says "exceeds tolerance",
        // not "penalty values detected". The user cannot distinguish a genuine bad fit
        // from a bound violation.
        let report = CalibrationReport::for_type_with_tolerance(
            "global_fit",
            residuals,
            10,
            1e-8, // small_penalty (≈100) >> tolerance (1e-8)
        );

        // Post-fix (threshold path): still fails (residual > tolerance), but the
        // convergence_reason should NOT say "penalty" for the threshold-only path.
        // This confirms the threshold gap still exists (it requires explicit flagging).
        assert!(
            !report.success,
            "report should fail since small_penalty >> tolerance"
        );
        // The gap: without an explicit flag, the report doesn't say "penalty".
        assert!(
            !report.convergence_reason.contains("penalty"),
            "threshold-based detection missed small penalty residual; \
             convergence_reason says: {}",
            report.convergence_reason
        );
    }

    /// `with_has_penalty_residuals(true)` must override threshold detection
    /// and flag the calibration as having penalty residuals.
    #[test]
    fn explicit_has_penalty_flag_overrides_threshold_detection() {
        let d = 1e-4_f64;
        let small_penalty = PENALTY * (d / (1.0 + d));

        let mut residuals = BTreeMap::new();
        residuals.insert("quote_1Y".to_string(), 1e-10);
        residuals.insert("quote_2Y_penalty".to_string(), small_penalty);

        // With explicit flag: report must mark success=false and explain penalty.
        let report = CalibrationReport::for_type_with_tolerance("global_fit", residuals, 10, 1e-8)
            .with_has_penalty_residuals(true);

        assert!(
            !report.success,
            "with_has_penalty_residuals(true) should mark calibration as failed"
        );
        assert!(
            report.convergence_reason.contains("penalty"),
            "with_has_penalty_residuals(true) should produce a penalty-mentioning reason; got: {}",
            report.convergence_reason
        );
    }

    /// `with_has_penalty_residuals(false)` must not change the outcome for
    /// a normal calibration (no regression on clean path).
    #[test]
    fn explicit_no_penalty_flag_does_not_regress_clean_calibration() {
        let mut residuals = BTreeMap::new();
        residuals.insert("quote_1Y".to_string(), 1e-10);
        residuals.insert("quote_5Y".to_string(), 2e-10);

        let report = CalibrationReport::for_type_with_tolerance("yield_curve", residuals, 10, 1e-8)
            .with_has_penalty_residuals(false);

        assert!(
            report.success,
            "with_has_penalty_residuals(false) should not break a clean calibration"
        );
        assert!(
            report.convergence_reason.contains("converged"),
            "clean calibration should still converge: {}",
            report.convergence_reason
        );
    }

    #[test]
    fn test_for_type_with_tolerance_preserves_typed_tolerance() {
        let residuals = BTreeMap::new();
        let tolerance = 1e-8;

        let report =
            CalibrationReport::for_type_with_tolerance("yield_curve", residuals, 0, tolerance);

        assert_eq!(report.success_tolerance, Some(tolerance));
        assert!(!report.metadata.contains_key("tolerance"));
        assert!(!report.metadata.contains_key("success_tolerance"));
        assert!(
            report.metadata.contains_key("type"),
            "Metadata should include type"
        );
        assert_eq!(
            report.metadata.get("type"),
            Some(&"yield_curve".to_string())
        );
    }

    #[test]
    fn solver_method_and_residual_units_wire_names_match_serde() {
        for method in [
            SolverMethod::SequentialBootstrap,
            SolverMethod::GlobalFitLmWeightedLsq,
            SolverMethod::PerSliceLeastSquares,
            SolverMethod::ScalarRootFind,
            SolverMethod::ScalarMinimization,
            SolverMethod::PlanExecution,
        ] {
            let json = serde_json::to_string(&method).expect("serialize method");
            assert_eq!(json, format!("\"{}\"", method.as_str()));
        }
        for units in [
            ResidualUnits::PvPerUnitNotional,
            ResidualUnits::QuotedVolatility,
            ResidualUnits::DiscountedUpfrontFraction,
            ResidualUnits::AbsoluteResidualOverStepTolerance,
        ] {
            let json = serde_json::to_string(&units).expect("serialize units");
            assert_eq!(json, format!("\"{}\"", units.as_str()));
        }
    }

    #[test]
    fn typed_method_and_units_round_trip_and_mirror_metadata() {
        let residuals = BTreeMap::from([("Q1".to_string(), 1e-10)]);
        let report = CalibrationReport::for_type_with_tolerance("yield_curve", residuals, 3, 1e-8)
            .with_solver_method(SolverMethod::SequentialBootstrap)
            .with_residual_units(ResidualUnits::PvPerUnitNotional);
        assert_eq!(
            report.metadata.get("residual_units").map(String::as_str),
            Some("pv_per_unit_notional")
        );

        let json = serde_json::to_value(&report).expect("serialize report");
        assert_eq!(json["solver_method"], "sequential_bootstrap");
        assert_eq!(json["residual_units"], "pv_per_unit_notional");
        let restored: CalibrationReport = serde_json::from_value(json).expect("deserialize");
        assert_eq!(
            restored.solver_method,
            Some(SolverMethod::SequentialBootstrap)
        );
        assert_eq!(
            restored.residual_units,
            Some(ResidualUnits::PvPerUnitNotional)
        );

        // Reports written before the typed fields existed still deserialize.
        let mut legacy = serde_json::to_value(&report).expect("serialize report");
        let object = legacy.as_object_mut().expect("report object");
        object.remove("solver_method");
        object.remove("residual_units");
        let restored: CalibrationReport = serde_json::from_value(legacy).expect("legacy report");
        assert!(restored.solver_method.is_none());
        assert!(restored.residual_units.is_none());
    }
}
