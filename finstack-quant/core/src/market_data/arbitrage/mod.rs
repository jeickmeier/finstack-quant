//! Volatility surface arbitrage detection framework.
//!
//! Provides model-free arbitrage detection for any
//! [`VolSurface`](crate::market_data::surfaces::VolSurface) regardless of how
//! it was constructed (market quotes, SABR, SVI, etc.), plus SVI-specific
//! checks that operate on raw calibrated parameters.
//!
//! # Architecture
//!
//! - **Types** ([`types`][crate::market_data::arbitrage::types]): Violation
//!   taxonomy, severity model, and report
//! - **Checks** ([`checks`][crate::market_data::arbitrage::checks]):
//!   Composable [`ArbitrageCheck`][crate::market_data::arbitrage::ArbitrageCheck]
//!   trait with implementations for butterfly, calendar spread, and local vol
//!   density
//! - **SVI checks** ([`checks::svi`][crate::market_data::arbitrage::checks::svi]):
//!   Moment bounds, Gatheral-Jacquier density, and cross-slice calendar spread
//!   for SVI parameterizations
//! - **Orchestrator**
//!   ([`check_surface`][crate::market_data::arbitrage::check_surface]): Runs all
//!   enabled checks and aggregates results into an
//!   [`ArbitrageReport`][crate::market_data::arbitrage::ArbitrageReport]
//!
//! # Usage
//!
//! ```rust
//! use finstack_quant_core::market_data::surfaces::VolSurface;
//! use finstack_quant_core::market_data::arbitrage::{check_surface, ArbitrageCheckConfig};
//!
//! let surface = VolSurface::builder("TEST")
//!     .expiries(&[0.5, 1.0, 2.0])
//!     .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
//!     .row(&[0.30, 0.25, 0.20, 0.22, 0.28])
//!     .row(&[0.28, 0.24, 0.19, 0.21, 0.26])
//!     .row(&[0.26, 0.22, 0.18, 0.20, 0.24])
//!     .build()
//!     .expect("surface should build");
//!
//! let report = check_surface(&surface, &ArbitrageCheckConfig {
//!     forward: Some(100.0),
//!     ..Default::default()
//! })?;
//!
//! if report.passed {
//!     println!("Surface is arbitrage-free");
//! } else {
//!     for v in &report.violations {
//!         println!("{}: {}", v.severity, v.description);
//!     }
//! }
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

pub mod checks;
pub mod types;

pub use checks::{
    ArbitrageCheck, ButterflyCheck, CalendarSpreadCheck, LocalVolDensityCheck, SviArbitrageCheck,
};
pub use types::{
    ArbitrageReport, ArbitrageSeverity, ArbitrageType, ArbitrageViolation, ViolationLocation,
};

use crate::market_data::surfaces::VolSurface;
use crate::HashMap;

/// Configuration for the arbitrage detection suite.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArbitrageCheckConfig {
    /// Run butterfly (strike convexity) check.
    pub check_butterfly: bool,
    /// Run calendar spread (expiry monotonicity) check.
    pub check_calendar_spread: bool,
    /// Run Dupire local vol density check.
    pub check_local_vol_density: bool,
    /// Scalar forward price, broadcast across expiries when `forward_prices`
    /// is absent.
    pub forward: Option<f64>,
    /// Optional per-expiry forward prices used by butterfly, calendar, and
    /// local-vol density checks.
    ///
    /// When supplied, this must have the same length as `surface.expiries()`.
    /// If omitted, `forward` is broadcast across expiries for backward compatibility.
    #[serde(default)]
    pub forward_prices: Option<Vec<f64>>,
    /// Tolerance for all checks (total-variance units).
    pub tolerance: f64,
    /// Minimum severity to include in the report.
    /// Violations below this severity are filtered out.
    pub min_severity: ArbitrageSeverity,
}

impl Default for ArbitrageCheckConfig {
    fn default() -> Self {
        Self {
            check_butterfly: true,
            check_calendar_spread: true,
            check_local_vol_density: true,
            forward: None,
            forward_prices: None,
            tolerance: 1e-10,
            min_severity: ArbitrageSeverity::Negligible,
        }
    }
}

/// Expand a forward-price vector to one value per expiry.
///
/// A single supplied value is broadcast across all expiries. Otherwise the
/// length must match `expiry_count`.
///
/// # Arguments
///
/// * `expiry_count` - Number of volatility-surface expiries requiring a
///   forward price.
/// * `forward_prices` - One forward price to broadcast, or one finite forward
///   price per expiry in surface order.
///
/// # Errors
/// Returns a validation error when the supplied vector has neither length one
/// nor `expiry_count`.
pub fn expand_forward_prices(
    expiry_count: usize,
    forward_prices: Vec<f64>,
) -> crate::Result<Vec<f64>> {
    match forward_prices.len() {
        len if len == expiry_count => Ok(forward_prices),
        1 => Ok(vec![forward_prices[0]; expiry_count]),
        len => Err(crate::Error::Validation(format!(
            "forward_prices has {len} entries but expiries has {expiry_count}; expected 1 or {expiry_count}"
        ))),
    }
}

fn local_vol_density_violations(
    surface: &VolSurface,
    forward_prices: &[f64],
    tolerance: f64,
) -> Vec<ArbitrageViolation> {
    let all_equal = forward_prices
        .windows(2)
        .all(|w| (w[0] - w[1]).abs() < 1e-14);

    if all_equal {
        return LocalVolDensityCheck {
            forward: forward_prices[0],
            tolerance,
        }
        .check(surface);
    }

    let mut violations = Vec::new();
    for (i, &expiry) in surface.expiries().iter().enumerate() {
        let checker = LocalVolDensityCheck {
            forward: forward_prices[i],
            tolerance,
        };
        violations.extend(
            checker
                .check(surface)
                .into_iter()
                .filter(|v| (v.location.expiry - expiry).abs() < 1e-12),
        );
    }
    violations
}

/// Run the full arbitrage detection suite on a volatility surface.
///
/// This is the primary entry point for standalone arbitrage checking.
/// Returns a report aggregating all violations found.
///
/// # Arguments
///
/// * `surface` - Volatility surface whose strike and expiry grid is checked.
/// * `config` - Check-selection, forward-price, tolerance, and severity
///   settings applied without mutating `surface`.
///
/// Butterfly, calendar-spread, and local-vol-density checks require a forward
/// price. `config.forward_prices` supplies one value for every surface expiry
/// and takes precedence over `config.forward`; otherwise the scalar forward is
/// broadcast. If neither is supplied, the enabled checks are skipped and the
/// report contains no violations. Results below `config.min_severity` are
/// filtered before the report is sorted from highest to lowest severity.
///
/// The tolerance is interpreted in total-variance units by the individual
/// checks. This detector reports violations; it does not repair the surface or
/// prove the absence of arbitrage between the sampled checks.
///
/// # Errors
///
/// Returns an error only if `config.forward_prices` is supplied with a length
/// different from `surface.expiries().len()`. It does not validate the
/// finiteness or positivity of forward prices or the tolerance.
pub fn check_surface(
    surface: &VolSurface,
    config: &ArbitrageCheckConfig,
) -> crate::Result<ArbitrageReport> {
    let start = std::time::Instant::now();
    let mut all_violations: Vec<ArbitrageViolation> = Vec::new();

    let forwards = if let Some(forwards) = &config.forward_prices {
        if forwards.len() != surface.expiries().len() {
            return Err(crate::Error::Validation(format!(
                "arbitrage forward_prices length {} does not match expiry count {}",
                forwards.len(),
                surface.expiries().len()
            )));
        }
        forwards.clone()
    } else if let Some(fwd) = config.forward {
        vec![fwd; surface.expiries().len()]
    } else {
        Vec::new()
    };

    if config.check_butterfly && !forwards.is_empty() {
        let checker = ButterflyCheck {
            forwards: forwards.clone(),
            tolerance: config.tolerance,
        };
        all_violations.extend(checker.check(surface));
    }

    if config.check_calendar_spread && !forwards.is_empty() {
        let checker = CalendarSpreadCheck {
            forwards: forwards.clone(),
            tolerance: config.tolerance,
        };
        all_violations.extend(checker.check(surface));
    }

    if config.check_local_vol_density && !forwards.is_empty() {
        all_violations.extend(local_vol_density_violations(
            surface,
            &forwards,
            config.tolerance,
        ));
    }

    // Filter by minimum severity
    all_violations.retain(|v| v.severity >= config.min_severity);

    // Sort: critical first
    all_violations.sort_by(|a, b| b.severity.cmp(&a.severity));

    // Build aggregation maps
    let mut counts_by_type: HashMap<ArbitrageType, usize> = HashMap::default();
    let mut counts_by_severity: HashMap<ArbitrageSeverity, usize> = HashMap::default();
    for v in &all_violations {
        *counts_by_type.entry(v.violation_type).or_insert(0) += 1;
        *counts_by_severity.entry(v.severity).or_insert(0) += 1;
    }

    let passed = !all_violations
        .iter()
        .any(|v| v.severity > ArbitrageSeverity::Negligible);

    Ok(ArbitrageReport {
        surface_id: surface.id().to_string(),
        violations: all_violations,
        passed,
        counts_by_type,
        counts_by_severity,
        elapsed_us: start.elapsed().as_micros() as u64,
    })
}

/// Run a butterfly arbitrage check on volatility rows.
///
/// # Arguments
///
/// * `strikes` - Strictly ordered strike grid shared by every volatility row.
/// * `expiries` - Strictly ordered expiry times in years, one per row in `vols`.
/// * `vols` - Implied-volatility rows aligned with `expiries` and `strikes`.
/// * `forward_prices` - One forward price to broadcast or one price per expiry.
/// * `tolerance` - Non-negative numerical tolerance in total-variance units
///   used when classifying violations.
///
/// # Errors
/// Returns an error if the volatility grid or forward vector is invalid.
pub fn check_butterfly_grid(
    strikes: &[f64],
    expiries: &[f64],
    vols: &[Vec<f64>],
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> crate::Result<Vec<ArbitrageViolation>> {
    let surface = VolSurface::from_rows("surface-grid", expiries, strikes, vols)?;
    let forwards = expand_forward_prices(expiries.len(), forward_prices)?;
    Ok(ButterflyCheck {
        forwards,
        tolerance,
    }
    .check(&surface))
}

/// Run a calendar-spread arbitrage check on volatility rows.
///
/// # Arguments
///
/// * `strikes` - Strictly ordered strike grid shared by every volatility row.
/// * `expiries` - Strictly ordered expiry times in years, one per row in `vols`.
/// * `vols` - Implied-volatility rows aligned with `expiries` and `strikes`.
/// * `forward_prices` - One forward price to broadcast or one price per expiry.
/// * `tolerance` - Non-negative numerical tolerance in total-variance units
///   used when classifying violations.
///
/// # Errors
/// Returns an error if the volatility grid or forward vector is invalid.
pub fn check_calendar_spread_grid(
    strikes: &[f64],
    expiries: &[f64],
    vols: &[Vec<f64>],
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> crate::Result<Vec<ArbitrageViolation>> {
    let surface = VolSurface::from_rows("surface-grid", expiries, strikes, vols)?;
    let forwards = expand_forward_prices(expiries.len(), forward_prices)?;
    Ok(CalendarSpreadCheck {
        forwards,
        tolerance,
    }
    .check(&surface))
}

/// Run local-vol density checks on volatility rows.
///
/// `forward_prices` must contain one value per expiry because the local-vol
/// check is evaluated expiry by expiry when forwards differ.
///
/// # Arguments
///
/// * `strikes` - Strictly ordered strike grid shared by every volatility row.
/// * `expiries` - Strictly ordered expiry times in years, one per row in `vols`.
/// * `vols` - Implied-volatility rows aligned with `expiries` and `strikes`.
/// * `forward_prices` - One finite forward price for every expiry, in the same
///   order as `expiries`.
///
/// # Errors
/// Returns an error if the volatility grid is invalid or the forward vector
/// length does not match the expiry count.
pub fn check_local_vol_density_grid(
    strikes: &[f64],
    expiries: &[f64],
    vols: &[Vec<f64>],
    forward_prices: Vec<f64>,
) -> crate::Result<Vec<ArbitrageViolation>> {
    if forward_prices.len() != expiries.len() {
        return Err(crate::Error::Validation(format!(
            "forward_prices has {} entries but expiries has {}",
            forward_prices.len(),
            expiries.len()
        )));
    }
    let surface = VolSurface::from_rows("surface-grid", expiries, strikes, vols)?;
    Ok(local_vol_density_violations(
        &surface,
        &forward_prices,
        1e-10,
    ))
}

/// Run all standard volatility-surface arbitrage checks on volatility rows.
///
/// `forward_prices`, when provided, may contain either one value to broadcast
/// or one value per expiry.
///
/// # Arguments
///
/// * `strikes` - Strictly ordered strike grid shared by every volatility row.
/// * `expiries` - Strictly ordered expiry times in years, one per row in `vols`.
/// * `vols` - Implied-volatility rows aligned with `expiries` and `strikes`.
/// * `forward` - Optional scalar forward price broadcast across expiries when
///   `forward_prices` is absent or constant.
/// * `forward_prices` - Optional one-value broadcast or one forward price per
///   expiry. A non-constant vector enables per-expiry local-vol checks.
/// * `tolerance` - Non-negative numerical tolerance in total-variance units
///   used when classifying violations.
///
/// # Errors
/// Returns an error if the volatility grid or forward-vector shape is invalid.
pub fn check_surface_grid(
    strikes: &[f64],
    expiries: &[f64],
    vols: &[Vec<f64>],
    forward: Option<f64>,
    forward_prices: Option<Vec<f64>>,
    tolerance: f64,
) -> crate::Result<ArbitrageReport> {
    let surface = VolSurface::from_rows("surface-grid", expiries, strikes, vols)?;
    let normalized_forward_prices = forward_prices
        .map(|values| expand_forward_prices(expiries.len(), values))
        .transpose()?;
    let local_forward = forward.or_else(|| {
        normalized_forward_prices.as_ref().and_then(|values| {
            values
                .first()
                .copied()
                .filter(|first| values.iter().all(|v| (v - first).abs() < 1e-14))
        })
    });
    let per_expiry_local_vol_forwards = if local_forward.is_none() {
        normalized_forward_prices.clone()
    } else {
        None
    };
    let config = ArbitrageCheckConfig {
        check_butterfly: true,
        check_calendar_spread: true,
        check_local_vol_density: local_forward.is_some(),
        forward: local_forward,
        forward_prices: normalized_forward_prices,
        tolerance,
        min_severity: ArbitrageSeverity::Negligible,
    };
    let mut report = check_surface(&surface, &config)?;

    if let Some(forwards) = per_expiry_local_vol_forwards {
        report
            .violations
            .extend(local_vol_density_violations(&surface, &forwards, tolerance));
        refresh_report_summary(&mut report);
    }

    Ok(report)
}

fn refresh_report_summary(report: &mut ArbitrageReport) {
    report
        .violations
        .sort_by(|a, b| b.severity.cmp(&a.severity));

    report.counts_by_type.clear();
    report.counts_by_severity.clear();
    for violation in &report.violations {
        *report
            .counts_by_type
            .entry(violation.violation_type)
            .or_insert(0) += 1;
        *report
            .counts_by_severity
            .entry(violation.severity)
            .or_insert(0) += 1;
    }

    report.passed = !report
        .violations
        .iter()
        .any(|v| v.severity > ArbitrageSeverity::Negligible);
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::volatility::svi::SviParams;

    // ---- Helper surface constructors ----

    /// Flat vol surface: constant 20% vol everywhere. Must pass all checks.
    fn flat_surface() -> VolSurface {
        VolSurface::builder("FLAT-20")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[0.20, 0.20, 0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20, 0.20, 0.20])
            .row(&[0.20, 0.20, 0.20, 0.20, 0.20])
            .build()
            .expect("flat surface should build")
    }

    /// Clean smile surface: strongly convex in strike (total variance is
    /// quadratic), term structure with total variance strictly increasing.
    /// Should pass butterfly and calendar spread checks.
    fn clean_smile_surface() -> VolSurface {
        // Build a smile where total variance w = vol^2 * T is strongly convex
        // in strike. Use a wider strike spacing and ensure d2w/dk2 > 0 by
        // making wing vols significantly higher.
        VolSurface::builder("CLEAN-SMILE")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[0.35, 0.27, 0.20, 0.27, 0.35])
            .row(&[0.33, 0.26, 0.20, 0.26, 0.33])
            .row(&[0.31, 0.25, 0.20, 0.25, 0.31])
            .build()
            .expect("clean smile should build")
    }

    /// Surface with butterfly violation: non-convex strike profile at T=1.0.
    /// The middle strike has vol lower than what convexity allows.
    fn butterfly_violation_surface() -> VolSurface {
        // Create a concave kink at K=100 for T=1.0:
        // Total variance w = vol^2 * T should be concave at K=100.
        // Normal convex smile: 0.30, 0.25, 0.20, 0.25, 0.30
        // Violation smile:     0.22, 0.25, 0.30, 0.25, 0.22
        //   (this is concave: center is higher than neighbors)
        VolSurface::builder("BUTTERFLY-BAD")
            .expiries(&[0.5, 1.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[0.30, 0.25, 0.20, 0.25, 0.30]) // T=0.5 clean
            .row(&[0.22, 0.25, 0.30, 0.25, 0.22]) // T=1.0 concave in total var
            .build()
            .expect("butterfly violation surface should build")
    }

    /// Surface with calendar spread violation: T=2 has lower total variance
    /// than T=1 at certain strikes.
    fn calendar_spread_violation_surface() -> VolSurface {
        // T=1.0: vol = 0.25 -> total var = 0.0625
        // T=2.0: vol = 0.15 -> total var = 0.045 < 0.0625 => violation
        VolSurface::builder("CALENDAR-BAD")
            .expiries(&[0.5, 1.0, 2.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[0.30, 0.25, 0.20, 0.25, 0.30])
            .row(&[0.30, 0.25, 0.25, 0.25, 0.30]) // T=1.0 higher vol
            .row(&[0.20, 0.15, 0.15, 0.15, 0.20]) // T=2.0 much lower vol
            .build()
            .expect("calendar spread violation surface should build")
    }

    // ---- Tests ----

    #[test]
    fn expand_forward_prices_broadcasts_single_value() {
        assert_eq!(
            expand_forward_prices(3, vec![100.0]).expect("broadcast forwards"),
            vec![100.0, 100.0, 100.0]
        );
        assert!(expand_forward_prices(3, vec![100.0, 101.0]).is_err());
    }

    #[test]
    fn check_surface_grid_runs_standard_suite() {
        let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
        let expiries = vec![0.25, 0.5, 1.0, 2.0];
        let vols = vec![
            vec![0.20, 0.20, 0.20, 0.20, 0.20],
            vec![0.20, 0.20, 0.20, 0.20, 0.20],
            vec![0.20, 0.20, 0.20, 0.20, 0.20],
            vec![0.20, 0.20, 0.20, 0.20, 0.20],
        ];

        let report = check_surface_grid(
            &strikes,
            &expiries,
            &vols,
            None,
            Some(vec![100.0, 101.0, 102.0, 103.0]),
            1e-6,
        )
        .expect("grid arbitrage check");

        assert!(report.passed);
        assert_eq!(report.surface_id, "surface-grid");
    }

    #[test]
    fn check_surface_uses_forward_prices_for_local_vol_check() {
        let surface = calendar_spread_violation_surface();
        let forwards = vec![100.0, 101.0, 102.0];
        let direct_local_vol_violations = local_vol_density_violations(&surface, &forwards, 1e-10);
        assert!(!direct_local_vol_violations.is_empty());

        let config = ArbitrageCheckConfig {
            check_butterfly: false,
            check_calendar_spread: false,
            check_local_vol_density: true,
            forward: None,
            forward_prices: Some(forwards),
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        assert_eq!(report.violations.len(), direct_local_vol_violations.len());
        assert!(!report.passed);
    }

    #[test]
    fn check_surface_grid_preserves_per_expiry_local_vol_behavior() {
        let strikes = vec![80.0, 90.0, 100.0, 110.0, 120.0];
        let expiries = vec![0.5, 1.0, 2.0];
        let vols = vec![
            vec![0.30, 0.25, 0.20, 0.25, 0.30],
            vec![0.30, 0.25, 0.25, 0.25, 0.30],
            vec![0.20, 0.15, 0.15, 0.15, 0.20],
        ];

        let report = check_surface_grid(
            &strikes,
            &expiries,
            &vols,
            None,
            Some(vec![100.0, 101.0, 102.0]),
            1e-10,
        )
        .expect("grid arbitrage check");

        let local_vol_count = report
            .violations
            .iter()
            .filter(|v| v.violation_type == ArbitrageType::LocalVolDensity)
            .count();
        assert!(local_vol_count > 0);
        assert_eq!(
            report
                .counts_by_type
                .get(&ArbitrageType::LocalVolDensity)
                .copied()
                .unwrap_or(0),
            local_vol_count
        );
    }

    #[test]
    fn flat_surface_passes_all_checks() {
        let surface = flat_surface();
        let config = ArbitrageCheckConfig {
            forward: Some(100.0),
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        assert!(
            report.passed,
            "Flat surface should pass all checks. Violations: {:?}",
            report
                .violations
                .iter()
                .map(|v| &v.description)
                .collect::<Vec<_>>()
        );
        assert!(
            report.violations.is_empty()
                || report
                    .violations
                    .iter()
                    .all(|v| v.severity == ArbitrageSeverity::Negligible),
            "Flat surface should have no violations above Negligible"
        );
    }

    #[test]
    fn clean_smile_passes_butterfly_and_calendar() {
        let surface = clean_smile_surface();
        let config = ArbitrageCheckConfig {
            forward: Some(100.0),
            min_severity: ArbitrageSeverity::Minor,
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        // Filter to non-density checks (butterfly and calendar only)
        let butterfly_violations: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.violation_type == ArbitrageType::Butterfly)
            .collect();
        let calendar_violations: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.violation_type == ArbitrageType::CalendarSpread)
            .collect();

        assert!(
            butterfly_violations.is_empty(),
            "Clean smile should pass butterfly check. Violations: {:?}",
            butterfly_violations
                .iter()
                .map(|v| &v.description)
                .collect::<Vec<_>>()
        );
        assert!(
            calendar_violations.is_empty(),
            "Clean smile should pass calendar spread check. Violations: {:?}",
            calendar_violations
                .iter()
                .map(|v| &v.description)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn butterfly_violation_detected() {
        let surface = butterfly_violation_surface();
        let config = ArbitrageCheckConfig {
            check_butterfly: true,
            check_calendar_spread: false,
            check_local_vol_density: false,
            forward: Some(100.0),
            min_severity: ArbitrageSeverity::Minor,
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        let butterfly_violations: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.violation_type == ArbitrageType::Butterfly)
            .collect();

        assert!(
            !butterfly_violations.is_empty(),
            "Should detect butterfly violation on concave smile"
        );

        // Violation should be at T=1.0 (the concave slice)
        assert!(
            butterfly_violations
                .iter()
                .any(|v| (v.location.expiry - 1.0).abs() < 0.01),
            "Butterfly violation should be at T=1.0"
        );
    }

    #[test]
    fn calendar_spread_violation_detected() {
        let surface = calendar_spread_violation_surface();
        let config = ArbitrageCheckConfig {
            check_butterfly: false,
            check_calendar_spread: true,
            check_local_vol_density: false,
            forward: Some(100.0),
            min_severity: ArbitrageSeverity::Negligible,
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        let calendar_violations: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.violation_type == ArbitrageType::CalendarSpread)
            .collect();

        assert!(
            !calendar_violations.is_empty(),
            "Should detect calendar spread violation"
        );

        // Violation should reference T=2.0
        assert!(
            calendar_violations
                .iter()
                .any(|v| (v.location.expiry - 2.0).abs() < 0.01),
            "Calendar violation should be at T=2.0"
        );

        // Should have adjacent_expiry pointing back to T=1.0
        assert!(
            calendar_violations.iter().any(|v| v
                .location
                .adjacent_expiry
                .is_some_and(|ae| (ae - 1.0).abs() < 0.01)),
            "Calendar violation should reference T=1.0 as adjacent expiry"
        );
    }

    #[test]
    fn single_expiry_surface_no_calendar_violations() {
        let surface = VolSurface::builder("SINGLE-EXP")
            .expiries(&[1.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[0.30, 0.25, 0.20, 0.25, 0.30])
            .build()
            .expect("single expiry surface should build");

        let config = ArbitrageCheckConfig {
            check_calendar_spread: true,
            check_butterfly: false,
            check_local_vol_density: false,
            forward: Some(100.0),
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        assert!(
            report.violations.is_empty(),
            "Single expiry surface should have no calendar spread violations"
        );
    }

    #[test]
    fn two_strikes_no_butterfly_violations() {
        let surface = VolSurface::builder("TWO-STRIKES")
            .expiries(&[1.0, 2.0])
            .strikes(&[90.0, 110.0])
            .row(&[0.25, 0.25])
            .row(&[0.24, 0.24])
            .build()
            .expect("two strikes surface should build");

        let config = ArbitrageCheckConfig {
            check_butterfly: true,
            check_calendar_spread: false,
            check_local_vol_density: false,
            forward: Some(100.0),
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        let butterfly_violations: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.violation_type == ArbitrageType::Butterfly)
            .collect();
        assert!(
            butterfly_violations.is_empty(),
            "Two-strike surface should skip butterfly check (needs 3+ strikes)"
        );
    }

    #[test]
    fn report_severity_filtering() {
        let surface = calendar_spread_violation_surface();
        let config_all = ArbitrageCheckConfig {
            min_severity: ArbitrageSeverity::Negligible,
            forward: Some(100.0),
            ..Default::default()
        };
        let report = check_surface(&surface, &config_all).expect("arbitrage check should succeed");

        // Should have some violations
        assert!(
            !report.violations.is_empty(),
            "Should have violations on dirty surface"
        );

        // Filtering at Critical should return fewer or equal
        let critical_only = report.above_severity(ArbitrageSeverity::Critical);
        assert!(
            critical_only.len() <= report.violations.len(),
            "Critical filter should return subset"
        );
    }

    #[test]
    fn report_has_correct_counts() {
        let surface = calendar_spread_violation_surface();
        let config = ArbitrageCheckConfig {
            check_butterfly: false,
            check_calendar_spread: true,
            check_local_vol_density: false,
            forward: Some(100.0),
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        let type_count = report.counts_by_type.get(&ArbitrageType::CalendarSpread);
        let total_calendar: usize = report
            .violations
            .iter()
            .filter(|v| v.violation_type == ArbitrageType::CalendarSpread)
            .count();

        assert_eq!(
            type_count.copied().unwrap_or(0),
            total_calendar,
            "counts_by_type should match actual violation count"
        );
    }

    #[test]
    fn config_skips_checks_when_no_forward() {
        let surface = flat_surface();
        let config = ArbitrageCheckConfig {
            check_butterfly: true,
            check_calendar_spread: true,
            check_local_vol_density: true,
            forward: None,
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        assert!(
            report.violations.is_empty(),
            "All checks requiring forward should be skipped when forward is None"
        );
    }

    // ---- SVI-specific tests ----

    #[test]
    fn svi_moment_bound_violation_detected() {
        // b*(1+rho) = 1.5 * (1 + 0.5) = 2.25 > 2 => violation
        let bad_params = SviParams {
            a: 0.04,
            b: 1.5,
            rho: 0.5,
            m: 0.0,
            sigma: 0.1,
        };

        let check = SviArbitrageCheck {
            expiries: vec![1.0],
            params: vec![bad_params],
            k_range: (-2.0, 2.0),
            n_samples: 100,
        };

        let violations = check.check_moment_bounds();
        assert!(
            !violations.is_empty(),
            "Should detect moment bound violation for b*(1+rho) > 2"
        );
        assert!(
            violations
                .iter()
                .any(|v| v.violation_type == ArbitrageType::SviMomentBound),
            "Violation should be SviMomentBound type"
        );
    }

    #[test]
    fn svi_clean_params_pass_all_checks() {
        let clean_params = SviParams {
            a: 0.04,
            b: 0.3,
            rho: -0.3,
            m: 0.0,
            sigma: 0.15,
        };
        clean_params.validate().expect("params should be valid");

        let check = SviArbitrageCheck {
            expiries: vec![1.0],
            params: vec![clean_params],
            k_range: (-1.0, 1.0),
            n_samples: 200,
        };

        let violations = check.check_all();
        assert!(
            violations.is_empty(),
            "Clean SVI params should pass all checks. Violations: {:?}",
            violations
                .iter()
                .map(|v| &v.description)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn svi_calendar_spread_violation_detected() {
        // First slice has higher total variance than second at some k
        let slice1 = SviParams {
            a: 0.10,
            b: 0.3,
            rho: -0.2,
            m: 0.0,
            sigma: 0.15,
        };
        let slice2 = SviParams {
            a: 0.02, // much lower a => lower total variance
            b: 0.2,
            rho: -0.2,
            m: 0.0,
            sigma: 0.15,
        };

        let check = SviArbitrageCheck {
            expiries: vec![1.0, 2.0],
            params: vec![slice1, slice2],
            k_range: (-1.0, 1.0),
            n_samples: 100,
        };

        let violations = check.check_calendar_spread();
        assert!(
            !violations.is_empty(),
            "Should detect SVI calendar spread violation when T2 slice has lower total variance"
        );
        assert!(
            violations
                .iter()
                .all(|v| v.violation_type == ArbitrageType::SviCalendarSpread),
            "All violations should be SviCalendarSpread type"
        );
    }

    #[test]
    fn report_serialization_round_trip() {
        let surface = calendar_spread_violation_surface();
        let config = ArbitrageCheckConfig {
            forward: Some(100.0),
            ..Default::default()
        };
        let report = check_surface(&surface, &config).expect("arbitrage check should succeed");

        // Serialize to JSON
        let json = serde_json::to_string(&report).expect("report should serialize");

        // Deserialize back
        let deserialized: ArbitrageReport =
            serde_json::from_str(&json).expect("report should deserialize");

        assert_eq!(report.surface_id, deserialized.surface_id);
        assert_eq!(report.passed, deserialized.passed);
        assert_eq!(report.violations.len(), deserialized.violations.len());
        assert_eq!(report.elapsed_us, deserialized.elapsed_us);
    }

    #[test]
    fn config_serialization_round_trip() {
        let config = ArbitrageCheckConfig {
            check_butterfly: true,
            check_calendar_spread: false,
            check_local_vol_density: true,
            forward: Some(100.0),
            forward_prices: Some(vec![100.0, 101.0]),
            tolerance: 1e-8,
            min_severity: ArbitrageSeverity::Minor,
        };

        let json = serde_json::to_string(&config).expect("config should serialize");
        let deserialized: ArbitrageCheckConfig =
            serde_json::from_str(&json).expect("config should deserialize");

        assert_eq!(config.check_butterfly, deserialized.check_butterfly);
        assert_eq!(
            config.check_calendar_spread,
            deserialized.check_calendar_spread
        );
        assert_eq!(config.forward, deserialized.forward);
        assert_eq!(config.forward_prices, deserialized.forward_prices);
        assert!((config.tolerance - deserialized.tolerance).abs() < 1e-14);
        assert_eq!(config.min_severity, deserialized.min_severity);
    }
}
