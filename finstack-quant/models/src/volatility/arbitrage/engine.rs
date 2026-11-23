//! Volatility surface and grid arbitrage checking engine.

use super::checks::{ButterflyCheck, CalendarSpreadCheck, LocalVolDensityCheck};
use super::*;
use finstack_quant_core::market_data::surfaces::{VolQuoteType, VolSurface, VolSurfaceAxis};
use std::collections::BTreeMap;

/// Configuration for the arbitrage detection suite.
///
/// Every field is optional on the wire: an omitted field takes its
/// [`Default`] value, so `{"forward_prices": [100.0]}` runs all three checks
/// at [`DEFAULT_ARBITRAGE_TOLERANCE`] and reports every severity. Unknown
/// fields are rejected.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ArbitrageCheckConfig {
    /// Run butterfly (strike convexity) check.
    pub check_butterfly: bool,
    /// Run calendar spread (expiry monotonicity) check.
    pub check_calendar_spread: bool,
    /// Run Dupire local vol density check.
    pub check_local_vol_density: bool,
    /// Optional per-expiry forward prices used by butterfly, calendar, and
    /// local-vol density checks.
    ///
    /// Required when any check is enabled. Supply one finite positive value
    /// to broadcast across expiries or one such value per expiry.
    pub forward_prices: Option<Vec<f64>>,
    /// Tolerance for all checks (total-variance units).
    pub tolerance: f64,
    /// Minimum severity to include in the report.
    /// Violations below this severity are filtered out.
    pub min_severity: ArbitrageSeverity,
}

/// Default violation tolerance of every arbitrage check.
///
/// Used by [`ArbitrageCheckConfig::default`], the per-check `Default`s
/// ([`ButterflyCheck`], [`CalendarSpreadCheck`], [`LocalVolDensityCheck`]),
/// [`check_local_vol_density_grid`], and by the host bindings of the raw-grid
/// checks ([`check_butterfly_grid`], [`check_calendar_spread_grid`],
/// [`check_surface_grid`]) when the caller does not choose one. Expressed in
/// the units of each check's violation magnitude (call-price convexity or total
/// variance); a caller screening noisy market quotes passes a looser value.
pub const DEFAULT_ARBITRAGE_TOLERANCE: f64 = 1e-10;

impl Default for ArbitrageCheckConfig {
    fn default() -> Self {
        Self {
            check_butterfly: true,
            check_calendar_spread: true,
            check_local_vol_density: true,
            forward_prices: None,
            tolerance: DEFAULT_ARBITRAGE_TOLERANCE,
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
/// * `forward_prices` - One finite positive forward to broadcast, or one such forward
///   price per expiry in surface order.
///
/// # Errors
/// Returns a validation error when a forward is non-finite or non-positive,
/// or the supplied vector has neither length one nor `expiry_count`.
pub(super) fn expand_forward_prices(
    expiry_count: usize,
    forward_prices: Vec<f64>,
) -> finstack_quant_core::Result<Vec<f64>> {
    validate_forward_prices(&forward_prices)?;
    match forward_prices.len() {
        len if len == expiry_count => Ok(forward_prices),
        1 => Ok(vec![forward_prices[0]; expiry_count]),
        len => Err(finstack_quant_core::Error::Validation(format!(
            "forward_prices has {len} entries but expiries has {expiry_count}; expected 1 or {expiry_count}"
        ))),
    }
}

pub(super) fn validate_forward_prices(forwards: &[f64]) -> finstack_quant_core::Result<()> {
    if forwards.iter().any(|f| !f.is_finite() || *f <= 0.0) {
        return Err(finstack_quant_core::Error::Validation(
            "forward prices must be finite and positive".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_tolerance(tolerance: f64) -> finstack_quant_core::Result<()> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "arbitrage tolerance must be finite and non-negative".into(),
        ));
    }
    Ok(())
}

pub(super) fn local_vol_density_violations(
    surface: &VolSurface,
    forward_prices: &[f64],
    tolerance: f64,
) -> Vec<ArbitrageViolation> {
    LocalVolDensityCheck {
        forwards: forward_prices.to_vec(),
        tolerance,
    }
    .check(surface)
}

/// Run butterfly, calendar-spread, and local-vol density checks on a surface.
///
/// # Arguments
///
/// * `surface` - Volatility surface whose strike and expiry grid is checked.
/// * `config` - Check-selection, forward-price, tolerance, and severity
///   settings applied without mutating `surface`.
///
/// Butterfly, calendar-spread, and local-vol-density checks require forward
/// prices. `config.forward_prices` accepts one value to broadcast or one value
/// for every surface expiry. Omitting forwards while any check is enabled
/// returns a validation error. Results below `config.min_severity` are
/// filtered before the report is sorted from highest to lowest severity.
///
/// The tolerance is interpreted in total-variance units by the individual
/// checks. This detector reports violations; it does not repair the surface or
/// prove the absence of arbitrage between the sampled checks.
///
/// # Errors
///
/// Returns an error if required forwards are missing, non-finite, non-positive,
/// or have the wrong length, or if tolerance is non-finite or negative.
/// Only unshifted Black quotes on a strike axis are supported; normal,
/// displaced-Black, and tenor-axis surfaces return a convention error.
pub fn check_surface(
    surface: &VolSurface,
    config: &ArbitrageCheckConfig,
) -> finstack_quant_core::Result<ArbitrageReport> {
    surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
    surface.require_quote_type(VolQuoteType::BlackLognormal)?;
    validate_tolerance(config.tolerance)?;
    if config.forward_prices.is_none()
        && (config.check_butterfly
            || config.check_calendar_spread
            || config.check_local_vol_density)
    {
        return Err(finstack_quant_core::Error::Validation(
            "forward prices are required when arbitrage checks are enabled".into(),
        ));
    }
    let mut all_violations: Vec<ArbitrageViolation> = Vec::new();

    let forwards = if let Some(forwards) = &config.forward_prices {
        expand_forward_prices(surface.expiries().len(), forwards.clone())?
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

    all_violations.retain(|v| v.severity >= config.min_severity);
    all_violations.sort_by_key(|violation| std::cmp::Reverse(violation.severity));

    let mut counts_by_type: BTreeMap<ArbitrageType, usize> = BTreeMap::new();
    let mut counts_by_severity: BTreeMap<ArbitrageSeverity, usize> = BTreeMap::new();
    for v in &all_violations {
        *counts_by_type.entry(v.violation_type).or_insert(0) += 1;
        *counts_by_severity.entry(v.severity).or_insert(0) += 1;
    }

    let passed = !all_violations
        .iter()
        .any(|v| v.severity > ArbitrageSeverity::Negligible);

    Ok(ArbitrageReport {
        vol_surface_id: surface.id().to_string(),
        violations: all_violations,
        passed,
        counts_by_type,
        counts_by_severity,
    })
}

pub(super) fn surface_from_rows(
    expiries: &[f64],
    strikes: &[f64],
    vols: &[Vec<f64>],
) -> finstack_quant_core::Result<VolSurface> {
    VolSurface::builder("surface-grid")
        .expiries(expiries)
        .secondary_values(strikes)
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(finstack_quant_core::market_data::surfaces::VolInterpolationMode::Vol)
        .rows(vols)
        .build()
}

/// Run a butterfly arbitrage check on volatility rows.
///
/// # Arguments
///
/// * `expiries` - Strictly ordered expiry times in years, one per row in `vols`.
/// * `strikes` - Strictly ordered strike grid shared by every volatility row.
/// * `vols` - Implied-volatility rows aligned with `expiries` and `strikes`.
/// * `forward_prices` - One forward price to broadcast or one price per expiry.
/// * `tolerance` - Non-negative numerical tolerance in total-variance units
///   used when classifying violations.
///
/// # Errors
/// Returns an error if the grid or forward vector is invalid, any forward is
/// non-finite or non-positive, or tolerance is non-finite or negative.
pub fn check_butterfly_grid(
    expiries: &[f64],
    strikes: &[f64],
    vols: &[Vec<f64>],
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> finstack_quant_core::Result<Vec<ArbitrageViolation>> {
    validate_tolerance(tolerance)?;
    let surface = surface_from_rows(expiries, strikes, vols)?;
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
/// * `expiries` - Strictly ordered expiry times in years, one per row in `vols`.
/// * `strikes` - Strictly ordered strike grid shared by every volatility row.
/// * `vols` - Implied-volatility rows aligned with `expiries` and `strikes`.
/// * `forward_prices` - One forward price to broadcast or one price per expiry.
/// * `tolerance` - Non-negative numerical tolerance in total-variance units
///   used when classifying violations.
///
/// # Errors
/// Returns an error if the grid or forward vector is invalid, any forward is
/// non-finite or non-positive, or tolerance is non-finite or negative.
pub fn check_calendar_spread_grid(
    expiries: &[f64],
    strikes: &[f64],
    vols: &[Vec<f64>],
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> finstack_quant_core::Result<Vec<ArbitrageViolation>> {
    validate_tolerance(tolerance)?;
    let surface = surface_from_rows(expiries, strikes, vols)?;
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
/// * `expiries` - Strictly ordered expiry times in years, one per row in `vols`.
/// * `strikes` - Strictly ordered strike grid shared by every volatility row.
/// * `vols` - Implied-volatility rows aligned with `expiries` and `strikes`.
/// * `forward_prices` - One finite positive forward price for every expiry, in the same
///   order as `expiries`.
///
/// # Errors
/// Returns an error if the volatility grid is invalid or the forward vector
/// length does not match the expiry count, or any forward is non-finite or non-positive.
pub fn check_local_vol_density_grid(
    expiries: &[f64],
    strikes: &[f64],
    vols: &[Vec<f64>],
    forward_prices: Vec<f64>,
) -> finstack_quant_core::Result<Vec<ArbitrageViolation>> {
    validate_forward_prices(&forward_prices)?;
    if forward_prices.len() != expiries.len() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "forward_prices has {} entries but expiries has {}",
            forward_prices.len(),
            expiries.len()
        )));
    }
    let surface = surface_from_rows(expiries, strikes, vols)?;
    Ok(local_vol_density_violations(
        &surface,
        &forward_prices,
        DEFAULT_ARBITRAGE_TOLERANCE,
    ))
}

/// Run all standard volatility-surface arbitrage checks on volatility rows.
///
/// `forward_prices`, when provided, may contain either one value to broadcast
/// or one value per expiry.
///
/// # Arguments
///
/// * `expiries` - Strictly ordered expiry times in years, one per row in `vols`.
/// * `strikes` - Strictly ordered strike grid shared by every volatility row.
/// * `vols` - Implied-volatility rows aligned with `expiries` and `strikes`.
/// * `forward_prices` - One-value broadcast or one forward price per expiry.
/// * `tolerance` - Non-negative numerical tolerance in total-variance units
///   used when classifying violations.
///
/// # Errors
/// Returns an error if the grid or forward vector is invalid, any forward is
/// non-finite or non-positive, or tolerance is non-finite or negative.
pub fn check_surface_grid(
    expiries: &[f64],
    strikes: &[f64],
    vols: &[Vec<f64>],
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> finstack_quant_core::Result<ArbitrageReport> {
    let surface = surface_from_rows(expiries, strikes, vols)?;
    let config = ArbitrageCheckConfig {
        check_butterfly: true,
        check_calendar_spread: true,
        check_local_vol_density: true,
        forward_prices: Some(forward_prices),
        tolerance,
        min_severity: ArbitrageSeverity::Negligible,
    };
    check_surface(&surface, &config)
}
