//! Python bindings for Brinson-Fachler attribution.
//!
//! The typed entry points (`brinson_fachler`, `carino_link`) return `Py*`
//! wrappers over the canonical Rust results; the paired `*_json` functions
//! keep the exact JSON wire strings for pipelines that exchange documents.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::errors::{display_to_py, portfolio_to_py, serde_json_to_py};

/// Column schema shared by [`PyBrinsonPeriodResult::to_dataframe`] and
/// [`PyCarinoLinkedAttribution::to_dataframe`] (both frames are sector-effect
/// tables).
const SECTOR_EFFECT_COLUMNS: &[ColumnSchema<'static>] = &[
    ("sector", "str"),
    ("allocation", "float64"),
    ("selection", "float64"),
    ("interaction", "float64"),
    ("total", "float64"),
];

/// Single-period Brinson-Fachler attribution result.
///
/// Returned by :func:`brinson_fachler`. Sector effects preserve input order.
#[pyclass(
    name = "BrinsonPeriodResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyBrinsonPeriodResult {
    pub(crate) inner: finstack_quant_portfolio::BrinsonPeriodResult,
}

#[pymethods]
impl PyBrinsonPeriodResult {
    /// Per-sector effects as a list of dicts, in the order supplied.
    #[getter]
    fn sectors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.sectors)
    }

    /// Sum of allocation effects across sectors.
    #[getter]
    fn total_allocation(&self) -> f64 {
        self.inner.total_allocation
    }

    /// Sum of selection effects across sectors.
    #[getter]
    fn total_selection(&self) -> f64 {
        self.inner.total_selection
    }

    /// Sum of interaction effects across sectors.
    #[getter]
    fn total_interaction(&self) -> f64 {
        self.inner.total_interaction
    }

    /// Portfolio total return for the period.
    #[getter]
    fn portfolio_return(&self) -> f64 {
        self.inner.portfolio_return
    }

    /// Benchmark total return for the period.
    #[getter]
    fn benchmark_return(&self) -> f64 {
        self.inner.benchmark_return
    }

    /// Active return; equals the sum of the three effect totals.
    #[getter]
    fn total_excess_return(&self) -> f64 {
        self.inner.total_excess_return
    }

    /// Per-sector effects as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``sector``, ``allocation``, ``selection``, ``interaction``,
    /// ``total``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.sectors, SECTOR_EFFECT_COLUMNS)
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::BrinsonPeriodResult =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "BrinsonPeriodResult(sectors={}, total_excess_return={})",
            self.inner.sectors.len(),
            self.inner.total_excess_return,
        )
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }
}

/// Carino-linked multi-period Brinson attribution result.
///
/// Returned by :func:`carino_link`.
#[pyclass(
    name = "CarinoLinkedAttribution",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyCarinoLinkedAttribution {
    pub(crate) inner: finstack_quant_portfolio::CarinoLinkedAttribution,
}

#[pymethods]
impl PyCarinoLinkedAttribution {
    /// Per-period decompositions as a list of dicts, in chronological order.
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

    /// Per-sector Carino-smoothed effects summed across periods.
    #[getter]
    fn linked_sectors<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.linked_sectors)
    }

    /// Sum of per-sector linked allocation effects.
    #[getter]
    fn linked_allocation(&self) -> f64 {
        self.inner.linked_allocation
    }

    /// Sum of per-sector linked selection effects.
    #[getter]
    fn linked_selection(&self) -> f64 {
        self.inner.linked_selection
    }

    /// Sum of per-sector linked interaction effects.
    #[getter]
    fn linked_interaction(&self) -> f64 {
        self.inner.linked_interaction
    }

    /// Linked per-sector effects as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``sector``, ``allocation``, ``selection``, ``interaction``,
    /// ``total``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.linked_sectors, SECTOR_EFFECT_COLUMNS)
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::CarinoLinkedAttribution =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "CarinoLinkedAttribution(periods={}, sectors={})",
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

/// Parse sector JSON and run the canonical Rust single-period attribution.
fn run_brinson_fachler(
    py: Python<'_>,
    sectors_json: &str,
) -> PyResult<finstack_quant_portfolio::BrinsonPeriodResult> {
    let sectors_json = sectors_json.to_owned();
    py.detach(move || {
        let sectors: Vec<finstack_quant_portfolio::SectorPeriod> =
            serde_json::from_str(&sectors_json)
                .map_err(|err| serde_json_to_py(err, "invalid Brinson sectors JSON"))?;
        finstack_quant_portfolio::brinson_fachler(&sectors).map_err(portfolio_to_py)
    })
}

/// Parse precomputed period results and run the canonical Rust Carino linking.
fn run_carino_link(
    py: Python<'_>,
    periods_json: &str,
) -> PyResult<finstack_quant_portfolio::CarinoLinkedAttribution> {
    let periods_json = periods_json.to_owned();
    py.detach(move || {
        let periods: Vec<finstack_quant_portfolio::BrinsonPeriodResult> =
            serde_json::from_str(&periods_json)
                .map_err(|err| serde_json_to_py(err, "invalid Brinson period results JSON"))?;
        finstack_quant_portfolio::carino_link(&periods).map_err(portfolio_to_py)
    })
}

/// Parse raw sector periods and run the canonical Rust attribute-then-link.
fn run_carino_link_from_sector_periods(
    py: Python<'_>,
    periods_json: &str,
) -> PyResult<finstack_quant_portfolio::CarinoLinkedAttribution> {
    let periods_json = periods_json.to_owned();
    py.detach(move || {
        let periods: Vec<Vec<finstack_quant_portfolio::SectorPeriod>> =
            serde_json::from_str(&periods_json)
                .map_err(|err| serde_json_to_py(err, "invalid Carino sector periods JSON"))?;
        finstack_quant_portfolio::carino_link_from_sector_periods(&periods).map_err(portfolio_to_py)
    })
}

/// Compute a single-period Brinson-Fachler attribution from sector JSON.
///
/// Parameters
/// ----------
/// sectors_json : str | dict | list | pandas.DataFrame
///     JSON array of ``SectorPeriod`` objects with ``sector``,
///     ``portfolio_weight``, ``benchmark_weight``, ``portfolio_return``, and
///     ``benchmark_return`` fields.
///
/// Returns
/// -------
/// BrinsonPeriodResult
///     Typed result with per-sector effects, effect totals, and
///     ``to_dataframe()`` / ``to_json()`` exits. Use
///     :func:`brinson_fachler_json` for the raw wire string.
#[pyfunction]
#[pyo3(text_signature = "(sectors_json)")]
fn brinson_fachler(
    py: Python<'_>,
    sectors_json: &Bound<'_, PyAny>,
) -> PyResult<PyBrinsonPeriodResult> {
    let sectors_json = crate::bindings::extract::extract_records_json(py, sectors_json, "sectors")?;
    let sectors_json: &str = &sectors_json;
    Ok(PyBrinsonPeriodResult {
        inner: run_brinson_fachler(py, sectors_json)?,
    })
}

/// Compute a single-period Brinson-Fachler attribution and return wire JSON.
///
/// Wire twin of :func:`brinson_fachler`; same inputs, JSON-string output.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``BrinsonPeriodResult``.
#[pyfunction]
#[pyo3(text_signature = "(sectors_json)")]
fn brinson_fachler_json(py: Python<'_>, sectors_json: &Bound<'_, PyAny>) -> PyResult<String> {
    let sectors_json = crate::bindings::extract::extract_records_json(py, sectors_json, "sectors")?;
    let sectors_json: &str = &sectors_json;
    let result = run_brinson_fachler(py, sectors_json)?;
    serde_json::to_string(&result).map_err(|err| serde_json_to_py(err, "serialize Brinson result"))
}

/// Carino-link already-computed single-period Brinson-Fachler results.
///
/// Binds Rust ``carino_link``, like :func:`campisi_carino_link` and
/// :func:`grid_carino_link`; use :func:`carino_link_from_sector_periods` to
/// link raw sector inputs instead.
///
/// Parameters
/// ----------
/// periods_json : str | list[dict]
///     Chronological JSON array of ``BrinsonPeriodResult`` objects (for
///     example ``json.loads(brinson_fachler_json(...))`` per period) with
///     identical sector ordering in every period.
///
/// Returns
/// -------
/// CarinoLinkedAttribution
///     Typed result with linked sector effects and compounded returns. Use
///     :func:`carino_link_json` for the raw wire string.
///
/// Raises
/// ------
/// ValueError
///     If the JSON is malformed.
/// PortfolioError
///     If the sequence is empty or changes sector ordering, or a period
///     return is non-finite or at most ``-1``.
#[pyfunction]
#[pyo3(text_signature = "(periods_json)")]
fn carino_link(
    py: Python<'_>,
    periods_json: &Bound<'_, PyAny>,
) -> PyResult<PyCarinoLinkedAttribution> {
    let periods_json = crate::bindings::extract::extract_records_json(py, periods_json, "periods")?;
    let periods_json: &str = &periods_json;
    Ok(PyCarinoLinkedAttribution {
        inner: run_carino_link(py, periods_json)?,
    })
}

/// Carino-link precomputed Brinson period results and return wire JSON.
///
/// Wire twin of :func:`carino_link`; same inputs, JSON-string output.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``CarinoLinkedAttribution``.
#[pyfunction]
#[pyo3(text_signature = "(periods_json)")]
fn carino_link_json(py: Python<'_>, periods_json: &Bound<'_, PyAny>) -> PyResult<String> {
    let periods_json = crate::bindings::extract::extract_records_json(py, periods_json, "periods")?;
    let periods_json: &str = &periods_json;
    let result = run_carino_link(py, periods_json)?;
    serde_json::to_string(&result).map_err(|err| serde_json_to_py(err, "serialize Carino result"))
}

/// Compute Carino-linked multi-period Brinson attribution from raw sector
/// periods.
///
/// Binds Rust ``carino_link_from_sector_periods``: runs
/// :func:`brinson_fachler` on each period, then Carino-links the results.
///
/// Parameters
/// ----------
/// periods_json : str | list[list[dict]]
///     Chronological JSON array of periods, each an array of ``SectorPeriod``
///     objects (``sector``, ``portfolio_weight``, ``benchmark_weight``,
///     ``portfolio_return``, ``benchmark_return``).
///
/// Returns
/// -------
/// CarinoLinkedAttribution
///     Typed result with linked sector effects and compounded returns. Use
///     :func:`carino_link_from_sector_periods_json` for the raw wire string.
///
/// Raises
/// ------
/// ValueError
///     If the JSON is malformed.
/// PortfolioError
///     If any period fails Brinson validation, the sequence is empty or
///     changes sector ordering, or a period return is non-finite or at most
///     ``-1``.
#[pyfunction]
#[pyo3(text_signature = "(periods_json)")]
fn carino_link_from_sector_periods(
    py: Python<'_>,
    periods_json: &Bound<'_, PyAny>,
) -> PyResult<PyCarinoLinkedAttribution> {
    let periods_json = crate::bindings::extract::extract_records_json(py, periods_json, "periods")?;
    let periods_json: &str = &periods_json;
    Ok(PyCarinoLinkedAttribution {
        inner: run_carino_link_from_sector_periods(py, periods_json)?,
    })
}

/// Carino-link raw sector periods and return wire JSON.
///
/// Wire twin of :func:`carino_link_from_sector_periods`; same inputs,
/// JSON-string output.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``CarinoLinkedAttribution``.
#[pyfunction]
#[pyo3(text_signature = "(periods_json)")]
fn carino_link_from_sector_periods_json(
    py: Python<'_>,
    periods_json: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let periods_json = crate::bindings::extract::extract_records_json(py, periods_json, "periods")?;
    let periods_json: &str = &periods_json;
    let result = run_carino_link_from_sector_periods(py, periods_json)?;
    serde_json::to_string(&result).map_err(|err| serde_json_to_py(err, "serialize Carino result"))
}

/// Register Brinson attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBrinsonPeriodResult>()?;
    m.add_class::<PyCarinoLinkedAttribution>()?;
    m.add_function(wrap_pyfunction!(brinson_fachler, m)?)?;
    m.add_function(wrap_pyfunction!(brinson_fachler_json, m)?)?;
    m.add_function(wrap_pyfunction!(carino_link, m)?)?;
    m.add_function(wrap_pyfunction!(carino_link_json, m)?)?;
    m.add_function(wrap_pyfunction!(carino_link_from_sector_periods, m)?)?;
    m.add_function(wrap_pyfunction!(carino_link_from_sector_periods_json, m)?)?;
    Ok(())
}
