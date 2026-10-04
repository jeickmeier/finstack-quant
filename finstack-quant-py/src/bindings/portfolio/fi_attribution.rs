//! Python bindings for Campisi fixed-income benchmark attribution.
//!
//! The typed entry points return `Py*` wrappers over the canonical Rust
//! results; the paired `*_json` functions keep the exact JSON wire strings
//! (same pattern as the Brinson bindings in [`super::brinson`]).

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::pandas_utils::{
    serde_object_to_single_row_dataframe_with_schema, serde_rows_to_dataframe_with_schema,
    serde_to_py, ColumnSchema,
};
use crate::errors::{display_to_py, portfolio_to_py, serde_json_to_py};

/// Column schema for [`PyFiAttributionResult::to_dataframe`] (per-sector
/// effects).
const FI_SECTOR_COLUMNS: &[ColumnSchema<'static>] = &[
    ("sector", "str"),
    ("portfolio_weight", "float64"),
    ("benchmark_weight", "float64"),
    ("portfolio_return", "float64"),
    ("benchmark_return", "float64"),
    ("allocation", "float64"),
    ("active_carry", "float64"),
    ("active_treasury", "float64"),
    ("active_spread", "float64"),
    ("selection", "float64"),
    ("total_active", "float64"),
];

/// Column schema for [`PyFiCarinoLinkedResult::to_dataframe`] (linked
/// per-sector effects).
const FI_LINKED_SECTOR_COLUMNS: &[ColumnSchema<'static>] = &[
    ("sector", "str"),
    ("allocation", "float64"),
    ("active_carry", "float64"),
    ("active_treasury", "float64"),
    ("active_spread", "float64"),
    ("selection", "float64"),
    ("total_active", "float64"),
];

/// Single-period Campisi benchmark-relative attribution result.
///
/// Returned by :func:`campisi_attribution`.
#[pyclass(
    name = "FiAttributionResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFiAttributionResult {
    pub(crate) inner: finstack_quant_portfolio::FiAttributionResult,
}

#[pymethods]
impl PyFiAttributionResult {
    /// Per-sector effects as a list of dicts, in first-seen order.
    #[getter]
    fn sectors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.sectors)
    }

    /// Portfolio-side absolute Campisi split (``carry``, ``treasury``,
    /// ``spread``, ``selection``, ``total``).
    #[getter]
    fn portfolio_components<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.portfolio_components)
    }

    /// Benchmark-side absolute Campisi split.
    #[getter]
    fn benchmark_components<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.benchmark_components)
    }

    /// Portfolio total return.
    #[getter]
    fn portfolio_return(&self) -> f64 {
        self.inner.portfolio_return
    }

    /// Benchmark total return.
    #[getter]
    fn benchmark_return(&self) -> f64 {
        self.inner.benchmark_return
    }

    /// Active return, ``portfolio_return - benchmark_return``.
    #[getter]
    fn active_return(&self) -> f64 {
        self.inner.active_return
    }

    /// Sum of sector allocation effects.
    #[getter]
    fn total_allocation(&self) -> f64 {
        self.inner.total_allocation
    }

    /// Sum of sector active carry effects.
    #[getter]
    fn total_active_carry(&self) -> f64 {
        self.inner.total_active_carry
    }

    /// Sum of sector active treasury effects.
    #[getter]
    fn total_active_treasury(&self) -> f64 {
        self.inner.total_active_treasury
    }

    /// Sum of sector active spread effects.
    #[getter]
    fn total_active_spread(&self) -> f64 {
        self.inner.total_active_spread
    }

    /// Sum of sector selection effects.
    #[getter]
    fn total_selection(&self) -> f64 {
        self.inner.total_selection
    }

    /// Per-sector effects as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``sector``, ``portfolio_weight``, ``benchmark_weight``,
    /// ``portfolio_return``, ``benchmark_return``, ``allocation``,
    /// ``active_carry``, ``active_treasury``, ``active_spread``,
    /// ``selection``, ``total_active``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.sectors, FI_SECTOR_COLUMNS)
    }

    /// Reconcile the five Campisi effect totals against the active return.
    ///
    /// Binds the Rust method ``FiAttributionResult::reconciliation_check``.
    ///
    /// Parameters
    /// ----------
    /// tolerance : float
    ///     Absolute tolerance in return units (``1e-10`` is appropriate for
    ///     return-space values).
    ///
    /// Returns
    /// -------
    /// FiReconciliationReport
    ///     Typed report with ``total_residual``, ``is_reconciled`` and
    ///     ``tolerance``.
    #[pyo3(text_signature = "(self, tolerance)")]
    fn reconciliation_check(&self, tolerance: f64) -> PyFiReconciliationReport {
        PyFiReconciliationReport {
            inner: self.inner.reconciliation_check(tolerance),
        }
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::FiAttributionResult =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "FiAttributionResult(sectors={}, active_return={})",
            self.inner.sectors.len(),
            self.inner.active_return,
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Multi-period Carino-linked Campisi attribution result.
///
/// Returned by :func:`campisi_carino_link` and
/// :func:`campisi_carino_link_from_snapshots`.
#[pyclass(
    name = "FiCarinoLinkedResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFiCarinoLinkedResult {
    pub(crate) inner: finstack_quant_portfolio::FiCarinoLinkedResult,
}

#[pymethods]
impl PyFiCarinoLinkedResult {
    /// Per-period single-period results as a list of dicts, in chronological
    /// order.
    #[getter]
    fn periods<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.periods)
    }

    /// Geometrically compounded portfolio return.
    #[getter]
    fn portfolio_return_compounded(&self) -> f64 {
        self.inner.portfolio_return_compounded
    }

    /// Geometrically compounded benchmark return.
    #[getter]
    fn benchmark_return_compounded(&self) -> f64 {
        self.inner.benchmark_return_compounded
    }

    /// Per-sector linked effects as a list of dicts.
    #[getter]
    fn linked_sectors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.linked_sectors)
    }

    /// Sum of linked allocation effects.
    #[getter]
    fn linked_allocation(&self) -> f64 {
        self.inner.linked_allocation
    }

    /// Sum of linked active carry effects.
    #[getter]
    fn linked_active_carry(&self) -> f64 {
        self.inner.linked_active_carry
    }

    /// Sum of linked active treasury effects.
    #[getter]
    fn linked_active_treasury(&self) -> f64 {
        self.inner.linked_active_treasury
    }

    /// Sum of linked active spread effects.
    #[getter]
    fn linked_active_spread(&self) -> f64 {
        self.inner.linked_active_spread
    }

    /// Sum of linked selection effects.
    #[getter]
    fn linked_selection(&self) -> f64 {
        self.inner.linked_selection
    }

    /// Linked per-sector effects as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``sector``, ``allocation``, ``active_carry``,
    /// ``active_treasury``, ``active_spread``, ``selection``,
    /// ``total_active``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(
            py,
            &self.inner.linked_sectors,
            FI_LINKED_SECTOR_COLUMNS,
        )
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::FiCarinoLinkedResult =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "FiCarinoLinkedResult(periods={}, sectors={})",
            self.inner.periods.len(),
            self.inner.linked_sectors.len(),
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Reconciliation report for the five Campisi effect totals.
///
/// Returned by :func:`campisi_reconciliation_check`.
#[pyclass(
    name = "FiReconciliationReport",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFiReconciliationReport {
    pub(crate) inner: finstack_quant_portfolio::FiReconciliationReport,
}

#[pymethods]
impl PyFiReconciliationReport {
    /// ``active_return - (allocation + carry + treasury + spread + selection)``.
    #[getter]
    fn total_residual(&self) -> f64 {
        self.inner.total_residual
    }

    /// Whether the residual is within tolerance.
    #[getter]
    fn is_reconciled(&self) -> bool {
        self.inner.is_reconciled
    }

    /// Tolerance used for the check.
    #[getter]
    fn tolerance(&self) -> f64 {
        self.inner.tolerance
    }

    /// Single-row :class:`pandas.DataFrame` view of the report.
    ///
    /// Columns: ``total_residual``, ``is_reconciled``, ``tolerance``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe_with_schema(
            py,
            &self.inner,
            &["total_residual", "is_reconciled", "tolerance"],
        )
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::FiReconciliationReport =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "FiReconciliationReport(total_residual={}, is_reconciled={})",
            self.inner.total_residual, self.inner.is_reconciled,
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Parse inputs and run the canonical single-period Campisi attribution.
fn run_campisi_attribution(
    py: Python<'_>,
    portfolio_json: &str,
    benchmark_json: &str,
    config_json: &str,
) -> PyResult<finstack_quant_portfolio::FiAttributionResult> {
    let portfolio_json = portfolio_json.to_owned();
    let benchmark_json = benchmark_json.to_owned();
    let config_json = config_json.to_owned();
    py.detach(move || {
        let portfolio: Vec<finstack_quant_portfolio::FiPositionSnapshot> =
            serde_json::from_str(&portfolio_json)
                .map_err(|err| serde_json_to_py(err, "invalid Campisi portfolio JSON"))?;
        let benchmark: Vec<finstack_quant_portfolio::FiPositionSnapshot> =
            serde_json::from_str(&benchmark_json)
                .map_err(|err| serde_json_to_py(err, "invalid Campisi benchmark JSON"))?;
        let config: finstack_quant_portfolio::FiAttributionConfig =
            serde_json::from_str(&config_json)
                .map_err(|err| serde_json_to_py(err, "invalid Campisi config JSON"))?;
        finstack_quant_portfolio::campisi_attribution(&portfolio, &benchmark, &config)
            .map_err(portfolio_to_py)
    })
}

/// Parse period-result JSON and run the canonical Campisi Carino linking.
fn run_campisi_carino_link(
    py: Python<'_>,
    periods_json: &str,
) -> PyResult<finstack_quant_portfolio::FiCarinoLinkedResult> {
    let periods_json = periods_json.to_owned();
    py.detach(move || {
        let periods: Vec<finstack_quant_portfolio::FiAttributionResult> =
            serde_json::from_str(&periods_json)
                .map_err(|err| serde_json_to_py(err, "invalid Campisi period results JSON"))?;
        finstack_quant_portfolio::campisi_carino_link(&periods).map_err(portfolio_to_py)
    })
}

/// Parse snapshot JSON and run the canonical snapshot-level Carino linking.
fn run_campisi_carino_link_from_snapshots(
    py: Python<'_>,
    periods_json: &str,
    config_json: &str,
) -> PyResult<finstack_quant_portfolio::FiCarinoLinkedResult> {
    let periods_json = periods_json.to_owned();
    let config_json = config_json.to_owned();
    py.detach(move || {
        let periods: Vec<finstack_quant_portfolio::FiPeriodInput> =
            serde_json::from_str(&periods_json)
                .map_err(|err| serde_json_to_py(err, "invalid Campisi periods JSON"))?;
        let config: finstack_quant_portfolio::FiAttributionConfig =
            serde_json::from_str(&config_json)
                .map_err(|err| serde_json_to_py(err, "invalid Campisi config JSON"))?;
        finstack_quant_portfolio::campisi_carino_link_from_snapshots(&periods, &config)
            .map_err(portfolio_to_py)
    })
}

/// Parse a result and run the canonical reconciliation check.
fn run_campisi_reconciliation_check(
    py: Python<'_>,
    result_json: &str,
    tolerance: f64,
) -> PyResult<finstack_quant_portfolio::FiReconciliationReport> {
    let result_json = result_json.to_owned();
    py.detach(move || {
        let result: finstack_quant_portfolio::FiAttributionResult =
            serde_json::from_str(&result_json)
                .map_err(|err| serde_json_to_py(err, "invalid Campisi result JSON"))?;
        Ok(result.reconciliation_check(tolerance))
    })
}

/// Compute a single-period Campisi fixed-income attribution from JSON.
///
/// Parameters
/// ----------
/// portfolio_json : str | dict | list | pandas.DataFrame
///     JSON array of ``FiPositionSnapshot`` objects (``sector``, ``weight``,
///     ``total_return``, ``yield_annual``, ``modified_duration``,
///     ``spread_duration``, ``spread``, ``delta_treasury_yield``,
///     ``delta_spread``). ``spread_duration`` must be the canonical
///     quote-reproducing Z-spread duration, while ``spread`` and
///     ``delta_spread`` must use the matching ``z_spread`` basis. OAS,
///     G-spread, and discount-margin values must not be mixed into these
///     fields. Because the direct JSON shape carries numeric values but no
///     metric IDs, the binding cannot detect mislabeled spread provenance.
/// benchmark_json : str | dict | list | pandas.DataFrame
///     JSON array of ``FiPositionSnapshot`` objects for the benchmark, subject
///     to the same quote-reproducing Z-spread basis contract.
/// config_json : str | dict | list | pandas.DataFrame
///     JSON ``FiAttributionConfig``; ``period_years`` is its only field and is
///     required (no default). Unknown keys are rejected.
///
/// Returns
/// -------
/// FiAttributionResult
///     Typed result with per-sector effects and the five effect totals. Use
///     :meth:`to_json` for the raw wire string.
///
/// Raises
/// ------
/// PortfolioError
///     If canonical Rust validation rejects empty sides, non-finite values,
///     invalid weights, period length, or a sector present on either side has
///     ``|net sector weight| <= 1e-6 * gross absolute sector weight``.
///     Spread-basis provenance cannot be validated from the numeric JSON shape.
/// ValueError
///     If an input JSON string is malformed or does not match its schema.
#[pyfunction]
#[pyo3(text_signature = "(portfolio_json, benchmark_json, config_json)")]
fn campisi_attribution(
    py: Python<'_>,
    portfolio_json: &Bound<'_, PyAny>,
    benchmark_json: &Bound<'_, PyAny>,
    config_json: &Bound<'_, PyAny>,
) -> PyResult<PyFiAttributionResult> {
    let portfolio_json =
        crate::bindings::extract::extract_records_json(py, portfolio_json, "portfolio")?;
    let portfolio_json: &str = &portfolio_json;
    let benchmark_json =
        crate::bindings::extract::extract_records_json(py, benchmark_json, "benchmark")?;
    let benchmark_json: &str = &benchmark_json;
    let config_json = crate::bindings::extract::extract_records_json(py, config_json, "config")?;
    let config_json: &str = &config_json;
    Ok(PyFiAttributionResult {
        inner: run_campisi_attribution(py, portfolio_json, benchmark_json, config_json)?,
    })
}
/// Carino-link already-computed single-period Campisi attribution results.
///
/// Binds Rust `finstack_quant_portfolio::campisi_carino_link`. Because each
/// period carries its own already-applied `period_years`, periods of
/// *different* lengths (e.g. act/365 calendar months) link correctly here;
/// use this entry point whenever the periods are not all the same length.
///
/// Parameters
/// ----------
/// periods_json : str | dict | list | pandas.DataFrame
///     JSON array of ``FiAttributionResult`` objects, in chronological order,
///     as returned by :meth:`to_json` (or
///     ``FiAttributionResult.to_json()``).
///
/// Returns
/// -------
/// FiCarinoLinkedResult
///     Typed result with linked per-sector effects and compounded returns.
///     Use :meth:`to_json` for the raw wire string.
///
/// Raises
/// ------
/// PortfolioError
///     If no periods are supplied, sector ordering differs, a consumed
///     top-level return/effect, per-sector linked effect, or sector
///     ``total_active`` is non-finite; ``active_return`` disagrees with the
///     portfolio-minus-benchmark return; a sector ``total_active`` disagrees
///     with its five effects; sector effects do not reconcile to their
///     declared top-level totals; the five totals do not reconcile to
///     ``active_return`` within the overflow-safe scaled-L1 tolerance; a
///     reconciliation residual is non-finite; or a return is outside the
///     Carino domain.
/// ValueError
///     If ``periods_json`` is malformed or does not match the
///     ``FiAttributionResult`` schema.
#[pyfunction]
#[pyo3(text_signature = "(periods_json)")]
fn campisi_carino_link(
    py: Python<'_>,
    periods_json: &Bound<'_, PyAny>,
) -> PyResult<PyFiCarinoLinkedResult> {
    let periods_json = crate::bindings::extract::extract_records_json(py, periods_json, "periods")?;
    let periods_json: &str = &periods_json;
    Ok(PyFiCarinoLinkedResult {
        inner: run_campisi_carino_link(py, periods_json)?,
    })
}
/// Compute Carino-linked multi-period Campisi attribution from period JSON.
///
/// Binds Rust `finstack_quant_portfolio::campisi_carino_link_from_snapshots`.
/// One shared config — hence one shared ``period_years`` — is applied to every
/// period, so this entry point is only correct for equal-length periods.
///
/// Parameters
/// ----------
/// periods_json : str | dict | list | pandas.DataFrame
///     JSON array of ``FiPeriodInput`` objects, each with ``portfolio`` and
///     ``benchmark`` arrays of ``FiPositionSnapshot``.
/// config_json : str | dict | list | pandas.DataFrame
///     JSON ``FiAttributionConfig`` shared across periods.
///
/// Returns
/// -------
/// FiCarinoLinkedResult
///     Typed result with linked per-sector effects and compounded returns.
///     Use :meth:`to_json` for the raw wire
///     string.
#[pyfunction]
#[pyo3(text_signature = "(periods_json, config_json)")]
fn campisi_carino_link_from_snapshots(
    py: Python<'_>,
    periods_json: &Bound<'_, PyAny>,
    config_json: &Bound<'_, PyAny>,
) -> PyResult<PyFiCarinoLinkedResult> {
    let periods_json = crate::bindings::extract::extract_records_json(py, periods_json, "periods")?;
    let periods_json: &str = &periods_json;
    let config_json = crate::bindings::extract::extract_records_json(py, config_json, "config")?;
    let config_json: &str = &config_json;
    Ok(PyFiCarinoLinkedResult {
        inner: run_campisi_carino_link_from_snapshots(py, periods_json, config_json)?,
    })
}
/// Reconcile the five Campisi effect totals against the active return.
///
/// Binds the Rust method
/// `finstack_quant_portfolio::FiAttributionResult::reconciliation_check`.
/// The decomposition reconciles by construction (selection is the residual),
/// so this is a floating-point sanity gate rather than a model check; without
/// it Python and JavaScript callers must re-sum the five totals by hand.
///
/// Parameters
/// ----------
/// result_json : FiAttributionResult | str | dict | list | pandas.DataFrame
///     Typed ``FiAttributionResult`` (as returned by
///     :func:`campisi_attribution`) or its JSON form, as returned by
///     :meth:`to_json` (or ``FiAttributionResult.to_json()``).
///     ``FiAttributionResult.reconciliation_check`` is the method form.
/// tolerance : float
///     Absolute tolerance in return units (``1e-10`` is appropriate for
///     return-space values).
///
/// Returns
/// -------
/// FiReconciliationReport
///     Typed report with ``total_residual``, ``is_reconciled`` and
///     ``tolerance``. Use :meth:`to_json` for the
///     raw wire string.
#[pyfunction]
#[pyo3(text_signature = "(result_json, tolerance)")]
fn campisi_reconciliation_check(
    py: Python<'_>,
    result_json: &Bound<'_, PyAny>,
    tolerance: f64,
) -> PyResult<PyFiReconciliationReport> {
    let result_json = crate::bindings::extract::extract_records_json(py, result_json, "result")?;
    let result_json: &str = &result_json;
    Ok(PyFiReconciliationReport {
        inner: run_campisi_reconciliation_check(py, result_json, tolerance)?,
    })
}
/// Register Campisi attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFiAttributionResult>()?;
    m.add_class::<PyFiCarinoLinkedResult>()?;
    m.add_class::<PyFiReconciliationReport>()?;
    m.add_function(wrap_pyfunction!(campisi_attribution, m)?)?;
    m.add_function(wrap_pyfunction!(campisi_carino_link, m)?)?;
    m.add_function(wrap_pyfunction!(campisi_carino_link_from_snapshots, m)?)?;
    m.add_function(wrap_pyfunction!(campisi_reconciliation_check, m)?)?;
    Ok(())
}
