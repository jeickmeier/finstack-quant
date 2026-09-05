//! Python bindings for `finstack_quant_models::volatility::arbitrage`.
//!
//! Exposes a function-based API for model-free volatility surface arbitrage
//! detection. Vol surfaces are constructed internally from flat arrays so
//! callers can work directly with numpy-friendly inputs without first
//! building a `VolSurface` wrapper. The per-check functions return the serde
//! rows of the Rust `ArbitrageViolation`; the combined `check_surface_grid`
//! returns a typed `ArbitrageReport`.

use finstack_quant_models::volatility::arbitrage::{
    self as model_arbitrage, ArbitrageReport, ArbitrageSeverity, ArbitrageType, ArbitrageViolation,
};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

use crate::bindings::pandas_utils::{serde_rows_to_dataframe, serde_to_py};
use crate::errors::{core_to_py, serde_json_to_py};

/// Serde name of an arbitrage enum variant (the `snake_case` wire form).
fn label<T: serde::Serialize>(value: &T) -> PyResult<String> {
    finstack_quant_core::wire::serde_label(value).map_err(core_to_py)
}

/// Convert a slice of violations into a Python list of serde dicts.
///
/// Each entry is the wire form of the Rust ``ArbitrageViolation``:
/// ``violation_type``, ``severity``, ``location`` (``strike``, ``expiry``,
/// ``adjacent_expiry``), ``magnitude``, ``description``,
/// ``suggested_fix``.
fn violations_to_pylist<'py>(
    py: Python<'py>,
    violations: &[ArbitrageViolation],
) -> PyResult<Bound<'py, PyAny>> {
    serde_to_py(py, &violations)
}

/// Flatten one violation into a long-format row for ``to_dataframe``.
#[derive(serde::Serialize)]
struct ViolationRow<'a> {
    violation_type: String,
    severity: String,
    strike: f64,
    expiry: f64,
    adjacent_expiry: Option<f64>,
    magnitude: f64,
    suggested_fix: Option<f64>,
    description: &'a str,
}

/// Aggregated model-free arbitrage report for a volatility grid.
///
/// Returned by ``check_surface_grid``. Carries the sorted violation list
/// (critical first), the pass flag, and per-type / per-severity counts.
/// Picklable and JSON round-trippable; ``to_dataframe()`` gives one row per
/// violation.
///
/// Examples
/// --------
/// >>> from finstack_quant.models.volatility import check_surface_grid
/// >>> strikes, expiries = [90.0, 100.0, 110.0], [1.0, 2.0]
/// >>> vols, forwards = [[0.2, 0.2, 0.2], [0.2, 0.2, 0.2]], [100.0, 100.0]
/// >>> report = check_surface_grid(strikes, expiries, vols, forwards)
/// >>> (report.passed, report.total_violations)
/// (True, 0)
#[pyclass(
    name = "ArbitrageReport",
    module = "finstack_quant.models.volatility",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyArbitrageReport {
    pub(crate) inner: ArbitrageReport,
}

#[pymethods]
impl PyArbitrageReport {
    /// Identifier of the checked surface (``"grid"`` for array inputs).
    #[getter]
    fn vol_surface_id(&self) -> &str {
        &self.inner.vol_surface_id
    }

    /// ``True`` when no violation above ``negligible`` severity was found.
    #[getter]
    fn passed(&self) -> bool {
        self.inner.passed
    }

    /// Number of violations of any severity.
    #[getter]
    fn total_violations(&self) -> usize {
        self.inner.violations.len()
    }

    /// Wall-clock microseconds spent on the check suite (non-deterministic).
    #[getter]
    fn elapsed_us(&self) -> u64 {
        self.inner.elapsed_us
    }

    /// Violation rows as serde dicts (``violation_type``, ``location``,
    /// ``severity``, ``magnitude``, ``description``, ``suggested_fix``).
    #[getter]
    fn violations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        violations_to_pylist(py, &self.inner.violations)
    }

    /// Violation counts keyed by severity name, every severity present.
    #[getter]
    fn by_severity<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let by_sev = PyDict::new(py);
        for sev in [
            ArbitrageSeverity::Negligible,
            ArbitrageSeverity::Minor,
            ArbitrageSeverity::Major,
            ArbitrageSeverity::Critical,
        ] {
            by_sev.set_item(
                label(&sev)?,
                self.inner
                    .counts_by_severity
                    .get(&sev)
                    .copied()
                    .unwrap_or(0),
            )?;
        }
        Ok(by_sev)
    }

    /// Violation counts keyed by check type, every ``ArbitrageType`` present.
    #[getter]
    fn by_type<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let by_type = PyDict::new(py);
        for t in [
            ArbitrageType::Butterfly,
            ArbitrageType::CalendarSpread,
            ArbitrageType::LocalVolDensity,
            ArbitrageType::SviMomentBound,
            ArbitrageType::SviButterflyCondition,
            ArbitrageType::SviCalendarSpread,
        ] {
            by_type.set_item(
                label(&t)?,
                self.inner.counts_by_type.get(&t).copied().unwrap_or(0),
            )?;
        }
        Ok(by_type)
    }

    /// One row per violation as a ``pandas.DataFrame``.
    ///
    /// Columns: ``violation_type``, ``severity``, ``strike``, ``expiry``,
    /// ``adjacent_expiry``, ``magnitude``, ``suggested_fix``, ``description``.
    /// Empty (no columns) when the surface passed cleanly.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = self
            .inner
            .violations
            .iter()
            .map(|v| {
                Ok(ViolationRow {
                    violation_type: label(&v.violation_type)?,
                    severity: label(&v.severity)?,
                    strike: v.location.strike,
                    expiry: v.location.expiry,
                    adjacent_expiry: v.location.adjacent_expiry,
                    magnitude: v.magnitude,
                    suggested_fix: v.suggested_fix,
                    description: v.description.as_str(),
                })
            })
            .collect::<PyResult<Vec<_>>>()?;
        serde_rows_to_dataframe(py, &rows)
    }

    /// Serialize to compact JSON (the Rust ``ArbitrageReport`` wire form).
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(|e| serde_json_to_py(e, "ArbitrageReport"))
    }

    /// Deserialize from the JSON produced by ``to_json``.
    ///
    /// Raises ``ValueError`` on malformed JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: ArbitrageReport = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid ArbitrageReport JSON"))?;
        Ok(Self { inner })
    }

    /// Support ``pickle`` (and therefore ``copy.deepcopy``, ``multiprocessing``).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "ArbitrageReport(passed={}, total_violations={}, vol_surface_id='{}')",
            if self.inner.passed { "True" } else { "False" },
            self.inner.violations.len(),
            self.inner.vol_surface_id
        )
    }
}

/// Check butterfly arbitrage via Durrleman's g(k) density condition.
///
/// Parameters
/// ----------
/// strikes : list[float]
///     Monotonically increasing strike grid.
/// expiries : list[float]
///     Monotonically increasing expiry grid (years).
/// vols : list[list[float]]
///     Implied vols shaped ``[n_expiries][n_strikes]`` (decimal).
/// forward_prices : list[float]
///     Forward prices. Pass either one value to broadcast across expiries,
///     or one value per expiry.
/// tolerance : float, optional
///     Tolerance in total-variance units. Default ``1e-6``.
///
/// Returns
/// -------
/// list[dict]
///     One serde dict per violation (``violation_type``, ``location``,
///     ``severity``, ``magnitude``, ``description``, ``suggested_fix``).
///
/// Raises
/// ------
/// ValueError
///     If grid dimensions are inconsistent or inputs are non-finite.
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices, tolerance = 1e-6))]
fn check_butterfly_grid<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<Bound<'py, PyAny>> {
    let violations = model_arbitrage::check_butterfly_grid(
        &strikes,
        &expiries,
        &vols,
        forward_prices,
        tolerance,
    )
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
///     Implied vols shaped ``[n_expiries][n_strikes]`` (decimal).
/// forward_prices : list[float]
///     Forward prices. Pass either one value to broadcast across expiries,
///     or one value per expiry.
/// tolerance : float, optional
///     Tolerance in total-variance units. Default ``1e-6``.
///
/// Returns
/// -------
/// list[dict]
///     One serde dict per violation.
///
/// Raises
/// ------
/// ValueError
///     If grid dimensions are inconsistent or inputs are non-finite.
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices, tolerance = 1e-6))]
fn check_calendar_spread_grid<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<Bound<'py, PyAny>> {
    let violations = model_arbitrage::check_calendar_spread_grid(
        &strikes,
        &expiries,
        &vols,
        forward_prices,
        tolerance,
    )
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
///     Implied vols shaped ``[n_expiries][n_strikes]`` (decimal).
/// forward_prices : list[float]
///     One finite positive forward per expiry; length must equal
///     ``len(expiries)``. A single forward is not broadcast.
///
/// Returns
/// -------
/// list[dict]
///     One serde dict per violation.
///
/// Raises
/// ------
/// ValueError
///     If grid dimensions are inconsistent or inputs are non-finite.
///
/// Notes
/// -----
/// The underlying Rust check takes a single forward. When per-expiry forwards
/// are supplied, the check is run once per expiry with that expiry's forward
/// and only the corresponding expiry's violations are kept.
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices))]
fn check_local_vol_density_grid<'py>(
    py: Python<'py>,
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
) -> PyResult<Bound<'py, PyAny>> {
    let violations =
        model_arbitrage::check_local_vol_density_grid(&strikes, &expiries, &vols, forward_prices)
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
///     Implied vols shaped ``[n_expiries][n_strikes]`` (decimal).
/// forward_prices : list[float]
///     Forward prices for every check. Pass either one value to broadcast or
///     one value per expiry.
/// tolerance : float, optional
///     Shared tolerance for all checks. Default ``1e-6``.
///
/// Returns
/// -------
/// ArbitrageReport
///     Typed report with ``passed``, ``total_violations``, ``by_severity``,
///     ``by_type``, ``violations``, ``elapsed_us`` and ``to_dataframe()``.
///
/// Raises
/// ------
/// ValueError
///     If the forward-price shape or grid inputs are invalid.
#[pyfunction]
#[pyo3(signature = (strikes, expiries, vols, forward_prices, tolerance = 1e-6))]
fn check_surface_grid(
    strikes: Vec<f64>,
    expiries: Vec<f64>,
    vols: Vec<Vec<f64>>,
    forward_prices: Vec<f64>,
    tolerance: f64,
) -> PyResult<PyArbitrageReport> {
    model_arbitrage::check_surface_grid(&strikes, &expiries, &vols, forward_prices, tolerance)
        .map(|inner| PyArbitrageReport { inner })
        .map_err(core_to_py)
}

/// Register volatility arbitrage functions on `finstack_quant.models.volatility`.
pub fn register(_py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    parent.add_class::<PyArbitrageReport>()?;
    parent.add_function(wrap_pyfunction!(check_butterfly_grid, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_calendar_spread_grid, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_local_vol_density_grid, parent)?)?;
    parent.add_function(wrap_pyfunction!(check_surface_grid, parent)?)?;

    Ok(())
}
