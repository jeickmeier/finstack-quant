//! Standalone no-arbitrage checks on a volatility surface.
//!
//! Each function converts its arguments and calls the matching
//! `finstack_quant_calibration::validation` function; a failed check raises
//! ``ValueError`` with the Rust message.

use super::config::PyValidationConfig;
use crate::bindings::core::market_data::curves::PyVolSurface;
use crate::errors::core_to_py;
use pyo3::prelude::*;

use finstack_quant_calibration::validation as rust_validation;

/// Run the calendar-spread, butterfly-spread and volatility-bound checks.
///
/// Parameters
/// ----------
/// surface : VolSurface
///     Implied-volatility surface (expiries in years, absolute strikes).
/// config : ValidationConfig
///     Thresholds; ``check_arbitrage=False`` skips the two arbitrage checks
///     and ``lenient_arbitrage=True`` logs arbitrage violations instead of
///     raising (the volatility bounds are always checked).
///
/// Raises
/// ------
/// ValueError
///     If any check fails, naming the offending expiry and strike.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import ValidationConfig, validate_surface
/// >>> from finstack_quant.core.market_data import VolSurface
/// >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
/// >>> validate_surface(surface, ValidationConfig())
#[pyfunction]
#[pyo3(text_signature = "(surface, config)")]
pub(crate) fn validate_surface(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
) -> PyResult<()> {
    rust_validation::validate_surface(&surface.inner, &config.inner).map_err(core_to_py)
}

/// Run the forward-aware calendar-spread and call-convexity checks plus the volatility bounds.
///
/// Parameters
/// ----------
/// surface : VolSurface
///     Implied-volatility surface (expiries in years, absolute strikes).
/// config : ValidationConfig
///     Thresholds; ``check_arbitrage=False`` skips the arbitrage checks and
///     ``lenient_arbitrage=True`` logs arbitrage violations instead of raising.
/// forwards : list[float]
///     Forward price for each surface expiry, in expiry order (same units as
///     the strikes); one entry per expiry.
///
/// Raises
/// ------
/// ValueError
///     If ``forwards`` does not have one finite positive entry per expiry, or
///     any check fails.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import ValidationConfig, validate_surface_with_forwards
/// >>> from finstack_quant.core.market_data import VolSurface
/// >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
/// >>> validate_surface_with_forwards(surface, ValidationConfig(), [100.0, 100.0])
#[pyfunction]
#[pyo3(text_signature = "(surface, config, forwards)")]
pub(crate) fn validate_surface_with_forwards(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
    forwards: Vec<f64>,
) -> PyResult<()> {
    rust_validation::validate_surface_with_forwards(&surface.inner, &config.inner, &forwards)
        .map_err(core_to_py)
}

/// Check that total variance does not decrease with expiry at each strike.
///
/// Parameters
/// ----------
/// surface : VolSurface
///     Implied-volatility surface (expiries in years, absolute strikes).
/// config : ValidationConfig
///     Thresholds; ``check_arbitrage=False`` skips the check and
///     ``lenient_arbitrage=True`` logs violations instead of raising.
///
/// Raises
/// ------
/// ValueError
///     If total variance decreases between two expiries.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import ValidationConfig, validate_calendar_spread
/// >>> from finstack_quant.core.market_data import VolSurface
/// >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
/// >>> validate_calendar_spread(surface, ValidationConfig())
#[pyfunction]
#[pyo3(text_signature = "(surface, config)")]
pub(crate) fn validate_calendar_spread(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
) -> PyResult<()> {
    rust_validation::validate_calendar_spread(&surface.inner, &config.inner).map_err(core_to_py)
}

/// Check that total variance does not decrease with expiry at each forward moneyness.
///
/// Parameters
/// ----------
/// surface : VolSurface
///     Implied-volatility surface (expiries in years, absolute strikes).
/// config : ValidationConfig
///     Thresholds; ``check_arbitrage=False`` skips the check and
///     ``lenient_arbitrage=True`` logs violations instead of raising.
/// forwards : list[float]
///     Forward price for each surface expiry, in expiry order.
///
/// Raises
/// ------
/// ValueError
///     If ``forwards`` does not match the expiries, or total variance
///     decreases between two expiries at fixed forward moneyness.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import ValidationConfig, validate_calendar_spread_with_forwards
/// >>> from finstack_quant.core.market_data import VolSurface
/// >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
/// >>> validate_calendar_spread_with_forwards(surface, ValidationConfig(), [100.0, 100.0])
#[pyfunction]
#[pyo3(text_signature = "(surface, config, forwards)")]
pub(crate) fn validate_calendar_spread_with_forwards(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
    forwards: Vec<f64>,
) -> PyResult<()> {
    rust_validation::validate_calendar_spread_with_forwards(
        &surface.inner,
        &config.inner,
        &forwards,
    )
    .map_err(core_to_py)
}

/// Check that total variance is convex in strike at each expiry (butterfly spread).
///
/// Parameters
/// ----------
/// surface : VolSurface
///     Implied-volatility surface (expiries in years, absolute strikes).
/// config : ValidationConfig
///     Thresholds (``butterfly_upper_ratio``, ``butterfly_lower_ratio``,
///     ``lenient_arbitrage`` logs violations instead of raising);
///     ``check_arbitrage=False`` skips the check.
///
/// Raises
/// ------
/// ValueError
///     If a butterfly violation exceeds the configured tolerance.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import ValidationConfig, validate_butterfly_spread
/// >>> from finstack_quant.core.market_data import VolSurface
/// >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
/// >>> validate_butterfly_spread(surface, ValidationConfig())
#[pyfunction]
#[pyo3(text_signature = "(surface, config)")]
pub(crate) fn validate_butterfly_spread(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
) -> PyResult<()> {
    rust_validation::validate_butterfly_spread(&surface.inner, &config.inner).map_err(core_to_py)
}

/// Check that undiscounted Black call prices are convex in strike at each expiry.
///
/// Every adjacent vertical call spread must also cost between zero and its
/// strike width; the price tolerance is ``config.tolerance`` times the forward.
///
/// Parameters
/// ----------
/// surface : VolSurface
///     Implied-volatility surface (expiries in years, absolute strikes).
/// config : ValidationConfig
///     Thresholds; ``check_arbitrage=False`` skips the check and
///     ``lenient_arbitrage=True`` logs violations instead of raising.
/// forwards : list[float]
///     Forward price for each surface expiry, in expiry order.
///
/// Raises
/// ------
/// ValueError
///     If ``forwards`` does not match the expiries, or call prices are not
///     convex in strike.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import ValidationConfig, validate_butterfly_call_convexity
/// >>> from finstack_quant.core.market_data import VolSurface
/// >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
/// >>> validate_butterfly_call_convexity(surface, ValidationConfig(), [100.0, 100.0])
#[pyfunction]
#[pyo3(text_signature = "(surface, config, forwards)")]
pub(crate) fn validate_butterfly_call_convexity(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
    forwards: Vec<f64>,
) -> PyResult<()> {
    rust_validation::validate_butterfly_call_convexity(&surface.inner, &config.inner, &forwards)
        .map_err(core_to_py)
}

/// Check that every surface volatility is positive and at most ``config.max_volatility``.
///
/// Parameters
/// ----------
/// surface : VolSurface
///     Implied-volatility surface (expiries in years, absolute strikes).
/// config : ValidationConfig
///     Thresholds; ``max_volatility`` is the upper bound (annualized decimal).
///
/// Raises
/// ------
/// ValueError
///     If a volatility is not positive or exceeds ``max_volatility``.
///
/// Examples
/// --------
/// >>> from finstack_quant.calibration import ValidationConfig, validate_vol_bounds
/// >>> from finstack_quant.core.market_data import VolSurface
/// >>> surface = VolSurface("EQ-VOL", [1.0, 2.0], [90.0, 100.0, 110.0], [[0.205, 0.20, 0.205], [0.215, 0.21, 0.215]])
/// >>> validate_vol_bounds(surface, ValidationConfig())
#[pyfunction]
#[pyo3(text_signature = "(surface, config)")]
pub(crate) fn validate_vol_bounds(
    surface: PyRef<'_, PyVolSurface>,
    config: PyRef<'_, PyValidationConfig>,
) -> PyResult<()> {
    rust_validation::validate_vol_bounds(&surface.inner, &config.inner).map_err(core_to_py)
}
