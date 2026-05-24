//! Python bindings for `finstack_core::market_data::arbitrage`.
//!
//! Exposes a function-based API for model-free volatility surface arbitrage
//! detection. Vol surfaces are constructed internally from flat arrays so
//! callers can work directly with numpy-friendly inputs without first
//! building a `VolSurface` wrapper.

use finstack_core::market_data::arbitrage::{
    check_butterfly_grid, check_calendar_spread_grid, check_local_vol_density_grid,
    check_surface_grid, ArbitrageSeverity, ArbitrageType, ArbitrageViolation,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyModule};

use crate::errors::core_to_py;

/// Stable lowercase string name for an [`ArbitrageType`].
fn arbitrage_type_str(t: ArbitrageType) -> &'static str {
    match t {
        ArbitrageType::Butterfly => "butterfly",
        ArbitrageType::CalendarSpread => "calendar_spread",
        ArbitrageType::LocalVolDensity => "local_vol_density",
        ArbitrageType::SviMomentBound => "svi_moment_bound",
        ArbitrageType::SviButterflyCondition => "svi_butterfly_condition",
        ArbitrageType::SviCalendarSpread => "svi_calendar_spread",
    }
}

/// Stable lowercase string name for an [`ArbitrageSeverity`].
fn severity_str(s: ArbitrageSeverity) -> &'static str {
    match s {
        ArbitrageSeverity::Negligible => "negligible",
        ArbitrageSeverity::Minor => "minor",
        ArbitrageSeverity::Major => "major",
        ArbitrageSeverity::Critical => "critical",
    }
}

/// Convert a single violation into a Python dict.
fn violation_to_dict<'py>(py: Python<'py>, v: &ArbitrageViolation) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    d.set_item("type", arbitrage_type_str(v.violation_type))?;
    d.set_item("severity", severity_str(v.severity))?;
    d.set_item("strike", v.location.strike)?;
    d.set_item("expiry", v.location.expiry)?;
    if let Some(ae) = v.location.adjacent_expiry {
        d.set_item("adjacent_expiry", ae)?;
    } else {
        d.set_item("adjacent_expiry", py.None())?;
    }
    d.set_item("magnitude", v.magnitude)?;
    d.set_item("value", v.magnitude)?;
    d.set_item("message", v.description.as_str())?;
    d.set_item("description", v.description.as_str())?;
    Ok(d)
}

/// Convert a slice of violations into a Python list of dicts.
fn violations_to_pylist<'py>(
    py: Python<'py>,
    violations: &[ArbitrageViolation],
) -> PyResult<Bound<'py, PyList>> {
    let items: Vec<Bound<'py, PyDict>> = violations
        .iter()
        .map(|v| violation_to_dict(py, v))
        .collect::<PyResult<Vec<_>>>()?;
    PyList::new(py, items)
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

/// Check butterfly arbitrage via Durrleman's g(k) density condition.
///
/// Parameters
/// ----------
/// strikes : list[float]
///     Monotonically increasing strike grid.
/// expiries : list[float]
///     Monotonically increasing expiry grid (years).
/// vols : list[list[float]]
///     Implied vols shaped ``[n_expiries][n_strikes]``.
/// forward_prices : list[float]
///     Forward prices. Pass either one scalar-equivalent entry to broadcast
///     across expiries, or one value per expiry.
/// tolerance : float, optional
///     Tolerance in total-variance units. Default ``1e-6``.
///
/// Returns
/// -------
/// list[dict]
///     One dict per violation with keys ``type``, ``severity``, ``strike``,
///     ``expiry``, ``adjacent_expiry``, ``magnitude``, ``value``,
///     ``message``, ``description``.
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices, tolerance = 1e-6))]
fn check_butterfly<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<Bound<'py, PyList>> {
    let violations = check_butterfly_grid(&strikes, &expiries, &vols, forward_prices, tolerance)
        .map_err(core_to_py)?;
    violations_to_pylist(py, &violations)
}

/// Check calendar spread arbitrage (total variance monotonicity in log-moneyness).
///
/// Parameters
/// ----------
/// strikes : list[float]
///     Monotonically increasing strike grid.
/// expiries : list[float]
///     Monotonically increasing expiry grid (years).
/// vols : list[list[float]]
///     Implied vols shaped ``[n_expiries][n_strikes]``.
/// forward_prices : list[float]
///     Forward prices. Pass either one scalar-equivalent entry to broadcast
///     across expiries, or one value per expiry.
/// tolerance : float, optional
///     Tolerance in total-variance units. Default ``1e-6``.
///
/// Returns
/// -------
/// list[dict]
///     One dict per violation.
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices, tolerance = 1e-6))]
fn check_calendar_spread<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<Bound<'py, PyList>> {
    let violations =
        check_calendar_spread_grid(&strikes, &expiries, &vols, forward_prices, tolerance)
            .map_err(core_to_py)?;
    violations_to_pylist(py, &violations)
}

/// Check Dupire local-vol density positivity.
///
/// Parameters
/// ----------
/// strikes : list[float]
///     Monotonically increasing strike grid.
/// expiries : list[float]
///     Monotonically increasing expiry grid (years).
/// vols : list[list[float]]
///     Implied vols shaped ``[n_expiries][n_strikes]``.
/// forward_prices : list[float]
///     Forward price per expiry (length must equal ``len(expiries)``).
///
/// Notes
/// -----
/// The underlying Rust check takes a single forward. When per-expiry forwards
/// are supplied, the check is run once per expiry with that expiry's forward
/// and only the corresponding expiry's violations are kept. This is
/// equivalent to the scalar case when all forwards are identical.
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices))]
fn check_local_vol_density<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
) -> PyResult<Bound<'py, PyList>> {
    let violations = check_local_vol_density_grid(&strikes, &expiries, &vols, forward_prices)
        .map_err(core_to_py)?;
    violations_to_pylist(py, &violations)
}

/// Run butterfly, calendar-spread, and local-vol density checks together.
///
/// Parameters
/// ----------
/// strikes : list[float]
///     Monotonically increasing strike grid.
/// expiries : list[float]
///     Monotonically increasing expiry grid (years).
/// vols : list[list[float]]
///     Implied vols shaped ``[n_expiries][n_strikes]``.
/// forward : float, optional
///     Scalar forward used by local-vol density. Also broadcast to the other
///     checks when ``forward_prices`` is omitted.
/// forward_prices : list[float], optional
///     Forward prices for butterfly and calendar-spread checks. Pass either
///     one value to broadcast, or one value per expiry.
/// tolerance : float, optional
///     Shared tolerance for all checks. Default ``1e-6``.
///
/// Returns
/// -------
/// dict
///     Aggregated report with keys ``total_violations``, ``passed``,
///     ``by_severity`` (dict ``severity -> count``), ``by_type``
///     (dict ``type -> count``), and ``violations`` (list[dict]).
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward = None, forward_prices = None, tolerance = 1e-6))]
fn check_all<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward: Option<f64>,
    forward_prices: Option<Vec<f64>>,
    tolerance: f64,
) -> PyResult<Bound<'py, PyDict>> {
    let report = check_surface_grid(
        &strikes,
        &expiries,
        &vols,
        forward,
        forward_prices,
        tolerance,
    )
    .map_err(core_to_py)?;

    let out = PyDict::new(py);
    out.set_item("total_violations", report.violations.len())?;
    out.set_item("passed", report.passed)?;

    let by_sev = PyDict::new(py);
    for sev in [
        ArbitrageSeverity::Negligible,
        ArbitrageSeverity::Minor,
        ArbitrageSeverity::Major,
        ArbitrageSeverity::Critical,
    ] {
        by_sev.set_item(
            severity_str(sev),
            report.counts_by_severity.get(&sev).copied().unwrap_or(0),
        )?;
    }
    out.set_item("by_severity", by_sev)?;

    let by_type = PyDict::new(py);
    for t in [
        ArbitrageType::Butterfly,
        ArbitrageType::CalendarSpread,
        ArbitrageType::LocalVolDensity,
    ] {
        by_type.set_item(
            arbitrage_type_str(t),
            report.counts_by_type.get(&t).copied().unwrap_or(0),
        )?;
    }
    out.set_item("by_type", by_type)?;

    out.set_item("violations", violations_to_pylist(py, &report.violations)?)?;
    out.set_item("elapsed_us", report.elapsed_us)?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the `finstack.core.market_data.arbitrage` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "arbitrage")?;
    m.setattr(
        "__doc__",
        "Volatility surface arbitrage detection: butterfly, calendar spread, local-vol density.",
    )?;

    m.add_function(wrap_pyfunction!(check_butterfly, &m)?)?;
    m.add_function(wrap_pyfunction!(check_calendar_spread, &m)?)?;
    m.add_function(wrap_pyfunction!(check_local_vol_density, &m)?)?;
    m.add_function(wrap_pyfunction!(check_all, &m)?)?;

    let all = PyList::new(
        py,
        [
            "check_butterfly",
            "check_calendar_spread",
            "check_local_vol_density",
            "check_all",
        ],
    )?;
    m.setattr("__all__", all)?;

    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "arbitrage",
        "finstack.core.market_data",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
