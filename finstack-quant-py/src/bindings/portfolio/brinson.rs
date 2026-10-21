//! Python bindings for Brinson-Fachler attribution.
//!
//! The entry points (`brinson_fachler`, `carino_link`) return `Py*` wrappers
//! over the canonical Rust results; each wrapper's `to_json()` gives the exact
//! JSON wire string for pipelines that exchange documents.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::errors::{portfolio_to_py, serde_json_to_py};

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
    pub(crate) inner: finstack_quant_portfolio::brinson::BrinsonPeriodResult,
}

crate::bindings::macros::wire_methods!(
    PyBrinsonPeriodResult,
    finstack_quant_portfolio::brinson::BrinsonPeriodResult,
    "BrinsonPeriodResult"
);

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

    fn __repr__(&self) -> String {
        format!(
            "BrinsonPeriodResult(sectors={}, total_excess_return={})",
            self.inner.sectors.len(),
            self.inner.total_excess_return,
        )
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
    pub(crate) inner: finstack_quant_portfolio::brinson::CarinoLinkedAttribution,
}

crate::bindings::macros::wire_methods!(
    PyCarinoLinkedAttribution,
    finstack_quant_portfolio::brinson::CarinoLinkedAttribution,
    "CarinoLinkedAttribution"
);

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

    fn __repr__(&self) -> String {
        format!(
            "CarinoLinkedAttribution(periods={}, sectors={})",
            self.inner.periods.len(),
            self.inner.linked_sectors.len(),
        )
    }
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
///     :meth:`to_json` for the raw wire string.
#[pyfunction]
#[pyo3(text_signature = "(sectors_json)")]
fn brinson_fachler(
    py: Python<'_>,
    sectors_json: &Bound<'_, PyAny>,
) -> PyResult<PyBrinsonPeriodResult> {
    let sectors_json = crate::bindings::extract::extract_records_json(py, sectors_json, "sectors")?;
    Ok(PyBrinsonPeriodResult {
        inner: py.detach(move || {
            let sectors: Vec<finstack_quant_portfolio::brinson::SectorPeriod> =
                serde_json::from_str(&sectors_json)
                    .map_err(|err| serde_json_to_py(err, "invalid Brinson sectors JSON"))?;
            finstack_quant_portfolio::brinson::brinson_fachler(&sectors).map_err(portfolio_to_py)
        })?,
    })
}
/// Carino-link already-computed single-period Brinson-Fachler results.
///
/// Binds Rust ``carino_link``, like :func:`campisi_carino_link` and
/// :func:`grid_carino_link`. To link raw sector inputs, attribute each period
/// first: ``carino_link([brinson_fachler(p) for p in periods])``.
///
/// Parameters
/// ----------
/// periods_json : list[BrinsonPeriodResult] | str | list[dict]
///     Chronological ``BrinsonPeriodResult`` values (typed results from
///     :func:`brinson_fachler`, or their JSON) with identical sector ordering
///     in every period.
///
/// Returns
/// -------
/// CarinoLinkedAttribution
///     Typed result with linked sector effects and compounded returns. Use
///     :meth:`to_json` for the raw wire string.
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
    Ok(PyCarinoLinkedAttribution {
        inner: py.detach(move || {
            let periods: Vec<finstack_quant_portfolio::brinson::BrinsonPeriodResult> =
                serde_json::from_str(&periods_json)
                    .map_err(|err| serde_json_to_py(err, "invalid Brinson period results JSON"))?;
            finstack_quant_portfolio::brinson::carino_link(&periods).map_err(portfolio_to_py)
        })?,
    })
}
/// Register Brinson attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBrinsonPeriodResult>()?;
    m.add_class::<PyCarinoLinkedAttribution>()?;
    m.add_function(wrap_pyfunction!(brinson_fachler, m)?)?;
    m.add_function(wrap_pyfunction!(carino_link, m)?)?;
    Ok(())
}
