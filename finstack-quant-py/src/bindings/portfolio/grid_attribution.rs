//! Python bindings for hierarchical duration-cell x sector grid attribution.
//!
//! Binds `finstack_quant_portfolio::grid_attribution::grid_attribution` (Dynkin, Hyman &
//! Vankudre 1998, Appendix A). The typed entry points return `Py*` wrappers;
//! Result wrappers serialize through the canonical Rust wire format.

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::{
    serde_object_to_single_row_arrow_table, serde_rows_to_arrow_table,
};
use crate::errors::{portfolio_to_py, serde_json_to_py};

/// Column schema for [`PyGridAttributionResult::to_arrow_table`] (per-cell
/// curve effects).
const GRID_CURVE_COLUMNS: &[(&str, &str)] = &[
    ("cell", "str"),
    ("portfolio_weight", "float64"),
    ("benchmark_weight", "float64"),
    ("benchmark_cell_return", "float64"),
    ("curve_effect", "float64"),
];

/// Column schema for [`PyGridAttributionResult::to_sector_effects_table`].
const GRID_SECTOR_COLUMNS: &[(&str, &str)] = &[
    ("cell", "str"),
    ("sector", "str"),
    ("allocation_effect", "float64"),
    ("selection_effect", "float64"),
];

/// Single-period hierarchical grid attribution result.
///
/// Returned by :func:`grid_attribution`.
#[pyclass(
    name = "GridAttributionResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyGridAttributionResult {
    pub(crate) inner: finstack_quant_portfolio::grid_attribution::GridAttributionResult,
}

crate::bindings::macros::wire_methods!(
    PyGridAttributionResult,
    finstack_quant_portfolio::grid_attribution::GridAttributionResult,
    "GridAttributionResult"
);

#[pymethods]
impl PyGridAttributionResult {
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

    /// Per-cell curve effects as a list of dicts, in first-appearance order.
    #[getter]
    fn curve_effects<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.curve_effects)
    }

    /// Per-(cell, sector) allocation and selection effects as a list of dicts.
    #[getter]
    fn sector_effects<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.sector_effects)
    }
    /// Sum of the curve effects.
    #[getter]
    fn total_curve(&self) -> f64 {
        self.inner.total_curve
    }

    /// Sum of the sector allocation effects.
    #[getter]
    fn total_sector(&self) -> f64 {
        self.inner.total_sector
    }

    /// Sum of the selection effects.
    #[getter]
    fn total_selection(&self) -> f64 {
        self.inner.total_selection
    }

    /// Per-(cell, sector) allocation and selection effects as an
    /// :class:`~finstack_quant.core.table.ArrowTable`.
    ///
    /// Columns: ``cell``, ``sector``, ``allocation_effect``, ``selection_effect``.
    #[pyo3(text_signature = "($self)")]
    fn to_sector_effects_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.sector_effects, GRID_SECTOR_COLUMNS)
    }

    fn __repr__(&self) -> String {
        format!(
            "GridAttributionResult(cells={}, active_return={})",
            self.inner.curve_effects.len(),
            self.inner.active_return,
        )
    }
}

impl PyGridAttributionResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.curve_effects, GRID_CURVE_COLUMNS)
    }
}

tabular_methods!(PyGridAttributionResult);

/// Multi-period Carino-linked hierarchical grid attribution result.
///
/// Returned by :func:`grid_carino_link`. Only the three top-level effects
/// are linked (see the Rust module docs).
#[pyclass(
    name = "GridCarinoLinkedResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyGridCarinoLinkedResult {
    pub(crate) inner: finstack_quant_portfolio::grid_attribution::GridCarinoLinkedResult,
}

crate::bindings::macros::wire_methods!(
    PyGridCarinoLinkedResult,
    finstack_quant_portfolio::grid_attribution::GridCarinoLinkedResult,
    "GridCarinoLinkedResult"
);

#[pymethods]
impl PyGridCarinoLinkedResult {
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

    /// Sum of per-period Carino-scaled curve effects.
    #[getter]
    fn linked_curve(&self) -> f64 {
        self.inner.linked_curve
    }

    /// Sum of per-period Carino-scaled sector allocation effects.
    #[getter]
    fn linked_sector(&self) -> f64 {
        self.inner.linked_sector
    }

    /// Sum of per-period Carino-scaled selection effects.
    #[getter]
    fn linked_selection(&self) -> f64 {
        self.inner.linked_selection
    }

    fn __repr__(&self) -> String {
        format!(
            "GridCarinoLinkedResult(periods={}, linked_selection={})",
            self.inner.periods.len(),
            self.inner.linked_selection,
        )
    }
}

impl PyGridCarinoLinkedResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let row = serde_json::json!({
            "portfolio_return_compounded": self.inner.portfolio_return_compounded,
            "benchmark_return_compounded": self.inner.benchmark_return_compounded,
            "linked_curve": self.inner.linked_curve,
            "linked_sector": self.inner.linked_sector,
            "linked_selection": self.inner.linked_selection,
        });
        serde_object_to_single_row_arrow_table(
            &row,
            &[
                ("portfolio_return_compounded", "float64"),
                ("benchmark_return_compounded", "float64"),
                ("linked_curve", "float64"),
                ("linked_sector", "float64"),
                ("linked_selection", "float64"),
            ],
        )
    }
}

tabular_methods!(PyGridCarinoLinkedResult);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._grid_attribution", "grid_attribution")]
#[pyfunction]
#[pyo3(text_signature = "(portfolio, benchmark)")]
fn grid_attribution(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    benchmark: &Bound<'_, PyAny>,
) -> PyResult<PyGridAttributionResult> {
    let portfolio = crate::bindings::extract::extract_records_json(py, portfolio, "portfolio")?;
    let benchmark = crate::bindings::extract::extract_records_json(py, benchmark, "benchmark")?;
    Ok(PyGridAttributionResult {
        inner: py.detach(move || {
            let portfolio: Vec<finstack_quant_portfolio::grid_attribution::GridPosition> =
                serde_json::from_str(&portfolio)
                    .map_err(|err| serde_json_to_py(err, "invalid grid portfolio JSON"))?;
            let benchmark: Vec<finstack_quant_portfolio::grid_attribution::GridPosition> =
                serde_json::from_str(&benchmark)
                    .map_err(|err| serde_json_to_py(err, "invalid grid benchmark JSON"))?;
            finstack_quant_portfolio::grid_attribution::grid_attribution(&portfolio, &benchmark)
                .map_err(portfolio_to_py)
        })?,
    })
}
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._grid_attribution", "grid_carino_link")]
#[pyfunction]
#[pyo3(text_signature = "(periods)")]
fn grid_carino_link(
    py: Python<'_>,
    periods: &Bound<'_, PyAny>,
) -> PyResult<PyGridCarinoLinkedResult> {
    if let Ok(typed) = periods.extract::<Vec<PyRef<'_, PyGridAttributionResult>>>() {
        let items: Vec<_> = typed.into_iter().map(|item| item.inner.clone()).collect();
        return Ok(PyGridCarinoLinkedResult {
            inner: py
                .detach(move || {
                    finstack_quant_portfolio::grid_attribution::grid_carino_link(&items)
                })
                .map_err(portfolio_to_py)?,
        });
    }
    let periods = crate::bindings::extract::extract_records_json(py, periods, "periods")?;
    Ok(PyGridCarinoLinkedResult {
        inner: py.detach(move || {
            let periods: Vec<finstack_quant_portfolio::grid_attribution::GridAttributionResult> =
                serde_json::from_str(&periods)
                    .map_err(|err| serde_json_to_py(err, "invalid grid periods JSON"))?;
            finstack_quant_portfolio::grid_attribution::grid_carino_link(&periods)
                .map_err(portfolio_to_py)
        })?,
    })
}
/// Register grid attribution functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGridAttributionResult>()?;
    m.add_class::<PyGridCarinoLinkedResult>()?;
    m.add_function(wrap_pyfunction!(grid_attribution, m)?)?;
    m.add_function(wrap_pyfunction!(grid_carino_link, m)?)?;
    Ok(())
}
