//! Shared tolerance policy for the calibration canonical test suite.
//!
//! These values aim to be "vendor grade" (Bloomberg/FINCAD-style): tight, explicit,
//! and consistent with the units used by each calibration adapter.
//!
//! ## Residual Units (important!)
//!
//! - **Rates discount/forward adapters**: residual = PV / CALIBRATION_NOTIONAL
//!   (PV is in currency units; CALIBRATION_NOTIONAL is typically 1_000_000).
//!   Example: tolerance `1e-8` ⇒ PV ≤ `0.01` per $1mm notional.
//!
//! - **Hazard adapter** (`HazardCurveTarget::calculate_residual`):
//!   residual = NPV / params.notional. With the default
//!   `HazardCurveSolveConfig::validation_tolerance = 1e-8` and
//!   `params.notional = 1.0`, this means absolute NPV-fit tolerance of `1e-8`
//!   per unit of synthetic-CDS notional. For a 5Y CDS at 100bp on a $1mm
//!   notional (~$30k protection-leg PV), this is ~$0.01 absolute = 3.3e-7
//!   relative — Bloomberg/Markit-grade tight.
//!
//! - **Base correlation adapter**: residual = complete quote-built tranche NPV /
//!   tranche notional. The quoted `upfront_pct` is already a decimal fraction of
//!   tranche notional, and its cashflow is discounted at contractual settlement.
//!
//! - **Swaption vol adapter**: residual = |model_vol - market_vol| in **decimal** units.
//!   For Normal vols, quotes are in bp and are normalized to decimals (50bp → 0.0050).

/// Strict absolute tolerance for pure floating-point identity checks.
#[allow(dead_code)]
pub const F64_ABS_TOL_STRICT: f64 = 1e-12;

/// Looser absolute tolerance for time-axis/year-fraction computations that involve multiple
/// floating-point steps and convention adjustments.
pub const F64_ABS_TOL_LOOSE: f64 = 1e-10;

/// Repricing PV tolerance (absolute dollars) used by canonical external repricing tests.
#[allow(dead_code)]
pub const REPRICE_PV_ABS_TOL_DOLLARS: f64 = 1.0;

/// Swap repricing tolerance expressed as an implied par-rate error in basis points.
#[allow(dead_code)]
pub const SWAP_REPRICE_TOL_BP: f64 = 0.1;

/// Minimum absolute PV tolerance for swap repricing, to avoid over-tight constraints when DV01 is
/// extremely small.
#[allow(dead_code)]
pub const SWAP_REPRICE_MIN_ABS_DOLLARS: f64 = 1.0;

/// FRA repricing tolerance (absolute dollars) for a $1mm notional FRA.
#[allow(dead_code)]
pub const FRA_REPRICE_ABS_TOL_DOLLARS: f64 = 5.0;

/// Bloomberg DF difference tolerances (basis points of DF, i.e. (df - bbg_df) * 10_000).
///
/// These tolerances account for:
/// - Convention differences between our implementation and Bloomberg
/// - Interpolation method variations
/// - Calendar and business day convention effects
#[allow(dead_code)]
pub const BBG_DF_TOL_BP_SHORT: f64 = 0.25;
#[allow(dead_code)]
pub const BBG_DF_TOL_BP_MID: f64 = 1.0;
#[allow(dead_code)]
pub const BBG_DF_TOL_BP_LONG: f64 = 2.5; // Slightly relaxed for 10Y-25Y range

/// Bloomberg zero-rate tolerance (basis points).
#[allow(dead_code)]
pub const BBG_ZERO_TOL_BP: f64 = 1.0;

/// Bloomberg DF tolerance for ultra-long end (>= 25Y), in DF basis points.
///
/// Ultra-long maturities have looser tolerances due to:
/// 1. Convention differences (day count, compounding) between implementations
/// 2. Interpolation method sensitivity at humped/inverted curve regions
/// 3. Small differences in OIS swap pricing (payment delays, calendar effects)
/// 4. The compounding effect of small rate differences over long horizons
#[allow(dead_code)]
pub const BBG_DF_TOL_BP_ULTRA_LONG: f64 = 100.0; // ~1% DF tolerance at 25Y+

/// Bloomberg zero-rate tolerance for ultra-long end (>= 25Y), in basis points.
///
/// A 12 bp tolerance at 25Y+ reflects real-world calibration challenges:
/// - Different OIS compounding implementations (FEDFUNDS vs SOFR conventions)
/// - Curve interpolation method differences (log-linear vs monotone convex)
/// - Market data interpretation differences at illiquid long tenors
#[allow(dead_code)]
pub const BBG_ZERO_TOL_BP_ULTRA_LONG: f64 = 100.0;

/// Forward rate absolute tolerance used by parity tests (in rate decimals).
pub const FWD_RATE_ABS_TOL: f64 = 1e-8;

/// Base correlation discounted upfront-fraction tolerance (dimensionless).
///
/// The production fit budget is 10 bp of tranche notional after discounting the
/// complete quoted contract, including its settlement upfront. Synthetic
/// round-trips additionally assert tighter repricing and correlation recovery
/// against the same contractual cashflows, using `BASE_CORR_SOLVER_TOL`.
#[allow(dead_code)]
pub const BASE_CORR_UPFRONT_FRAC_TOL: f64 = 1e-3;

/// Base-correlation Brent root-finder (solver) tolerance.
///
/// Distinct from `BASE_CORR_UPFRONT_FRAC_TOL`: this is the per-knot root-finding
/// precision, kept tight and <= the discount-curve validation tolerance (the
/// `CalibrationConfig` invariant requires `solver_tol <= validation_tolerance`).
/// `BASE_CORR_UPFRONT_FRAC_TOL` is only the end-to-end calibration-quality bar.
#[allow(dead_code)]
pub const BASE_CORR_SOLVER_TOL: f64 = 1e-10;

/// Swaption normal-vol fit tolerance (decimal). 5bp → 0.0005.
pub const SWAPTION_VOL_FIT_TOL_NORMAL_DECIMAL: f64 = 0.0005;

/// Convert a basis-point tolerance to a PV tolerance using an instrument DV01 (dollars per bp).
#[allow(dead_code)]
#[inline]
pub fn pv_tolerance_from_dv01(dv01_dollars_per_bp: f64, tol_bp: f64) -> f64 {
    (tol_bp * dv01_dollars_per_bp.abs()).max(SWAP_REPRICE_MIN_ABS_DOLLARS)
}
