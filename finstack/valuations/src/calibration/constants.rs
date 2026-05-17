//! Shared constants for calibration logic.
//!
//! Centralizes numerical tolerances and magic numbers to ensure consistency
//! across solvers, adapters, and validation.
//!
//! # Constants
//! - [`TOLERANCE_DUP_KNOTS`]: Tolerance for detecting duplicate knot times.
//! - [`PENALTY`]: Finite penalty value for objective functions.
//! - [`DF_MIN_HARD`]: Lower bound for discount factors.

/// Tolerance for detecting duplicate knot times or unsorted knots.
///
/// Used to ensure numerical stability in spline interpolation and sequential bootstrapping.
/// Knots closer than this value are treated as occurring at the same time.
pub(crate) const TOLERANCE_DUP_KNOTS: f64 = 1e-10;

/// Relative tolerance for deduping grid points in scan grids.
///
/// Ensures scan grids don't have clusters of points that could cause
/// redundant objective function evaluations.
pub(crate) const TOLERANCE_GRID_DEDUP: f64 = 0.001;

/// Minimum spacing between scan grid points to avoid numerical instability.
///
/// Expressed in year-fraction units. Spacing below this threshold can lead
/// to poorly conditioned matrices in global optimization.
pub(crate) const MIN_GRID_SPACING: f64 = 1e-8;

/// Hard minimum for discount factors during solving (to prevent log(0) or negative DFs).
///
/// Prevents the solver from exploring regions where interest rates become physically
/// impossible or result in non-finite logarithms.
pub(crate) const DF_MIN_HARD: f64 = 1e-12;

/// Minimum weight floor to avoid division by zero or effectively ignoring valid quotes.
///
/// Used when weighting residuals by inverse duration or other dynamic schemes.
pub(crate) const WEIGHT_MIN_FLOOR: f64 = 1e-3;

/// Finite penalty value used in objective functions instead of infinity.
///
/// Using a moderate large finite value (1e6) helps solvers behave more predictably
/// than extremely large values like 1e12, which can cause numerical instability
/// with gradient-based methods. The bootstrap-scan filter
/// `OBJECTIVE_VALID_ABS_MAX = PENALTY / 10.0` is calibrated against this value
/// — lowering PENALTY without raising the filter ratio causes legitimately
/// large feasible objective values to be treated as infeasible. Audit item #26
/// proposed lowering this to 1e3 to avoid LM trust-region stagnation; the
/// bootstrap-target test suite (xccy_basis, f-space tolerance) demonstrates
/// that the scan filter currently relies on the larger separation, so any
/// future reduction must come with a coordinated retune of the scan threshold.
pub(crate) const PENALTY: f64 = 1e6;

/// Maximum absolute objective value treated as "valid" during bracketing scans.
///
/// Values with `|f(x)| >= OBJECTIVE_VALID_ABS_MAX` are treated as penalized/infeasible
/// during the scan phase (but are still counted toward total evaluations).
pub(crate) const OBJECTIVE_VALID_ABS_MAX: f64 = PENALTY / 10.0;

/// Minimum absolute residual value treated as a "penalty" for reporting/diagnostics.
///
/// If the final residual exceeds this value, the calibration is considered to have
/// hit a hard constraint or failed significantly.
///
/// # W-41: Known threshold gap
///
/// The global solver's `fill_penalty` computes `PENALTY * (d / (1 + d))` for bound
/// violations, where `d = sqrt(sum_squared_outside)`. For very small violations
/// (`d ≪ 1`), this penalty value is proportional to `d * PENALTY`, which can be
/// far below this threshold. Magnitude-based detection alone cannot catch these
/// "small penalty" residuals.
///
/// Callers that KNOW bound violations occurred (e.g., because they track clamp
/// counts) MUST call [`CalibrationReport::with_has_penalty_residuals`] to explicitly
/// propagate the penalty flag, bypassing the magnitude threshold.
pub(crate) const RESIDUAL_PENALTY_ABS_MIN: f64 = PENALTY * 0.5;

/// Newtype wrapper for `f64` that implements `Ord` for use as `BTreeMap` keys.
///
/// Uses `f64::total_cmp` so NaN values sort consistently (greater than all finite values).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OrderedF64(pub f64);

impl Eq for OrderedF64 {}

impl PartialOrd for OrderedF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl From<f64> for OrderedF64 {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

impl OrderedF64 {
    pub(crate) fn into_inner(self) -> f64 {
        self.0
    }
}
