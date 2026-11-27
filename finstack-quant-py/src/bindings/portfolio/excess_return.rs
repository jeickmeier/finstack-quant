//! Python bindings for duration-matched credit excess returns.
//!
//! Binds `finstack_quant_portfolio::excess_return`: duration-cell base-return
//! tables built either from a reference universe (Dynkin, Hyman & Vankudre
//! 1998, Appendix B) or from discount-curve snapshots, plus position-level
//! duration-matched excess returns measured against either table. The typed
//! entry points return `Py*` wrappers; the paired `*_json` functions keep the
//! exact JSON wire strings. The curve-snapshot path takes typed
//! `DiscountCurve` objects directly (same pattern as the XVA bindings in
//! `crate::bindings::margin::xva`).

use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

use crate::bindings::core::market_data::curves::PyDiscountCurve;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::{portfolio_to_py, serde_json_to_py};

/// Column schema for [`PyDurationCellTable::to_arrow_table`].
const CELL_COLUMNS: &[(&str, &str)] = &[
    ("label", "str"),
    ("lower", "float64"),
    ("upper", "float64"),
    ("base_return", "float64"),
    ("observed", "bool"),
];

/// Column schema for [`PyExcessReturnResult::to_arrow_table`].
const POSITION_EXCESS_COLUMNS: &[(&str, &str)] = &[
    ("id", "str"),
    ("cell", "str"),
    ("base_return", "float64"),
    ("excess_return", "float64"),
];

/// Duration-cell base-return table.
///
/// Returned by :func:`cell_returns_from_reference` and
/// :func:`cell_returns_from_curves`; consumed by :func:`excess_returns`
/// (pass ``table.to_json()``).
#[pyclass(
    name = "DurationCellTable",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyDurationCellTable {
    pub(crate) inner: finstack_quant_portfolio::excess_return::DurationCellTable,
}

crate::bindings::macros::wire_methods!(
    PyDurationCellTable,
    finstack_quant_portfolio::excess_return::DurationCellTable,
    "DurationCellTable"
);

#[pymethods]
impl PyDurationCellTable {
    /// Label identifying the reference universe (e.g. ``"UST"``).
    #[getter]
    fn base_label(&self) -> String {
        self.inner.base_label.clone()
    }

    /// Cells as a list of dicts, in ascending duration order.
    #[getter]
    fn cells<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.cells)
    }

    fn __repr__(&self) -> String {
        format!(
            "DurationCellTable(base_label={:?}, cells={})",
            self.inner.base_label,
            self.inner.cells.len(),
        )
    }
}

impl PyDurationCellTable {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.cells, CELL_COLUMNS)
    }
}

tabular_methods!(PyDurationCellTable);

/// Per-position and portfolio-level duration-matched credit excess returns.
///
/// Returned by :func:`excess_returns`.
#[pyclass(
    name = "ExcessReturnResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyExcessReturnResult {
    pub(crate) inner: finstack_quant_portfolio::excess_return::ExcessReturnResult,
}

crate::bindings::macros::wire_methods!(
    PyExcessReturnResult,
    finstack_quant_portfolio::excess_return::ExcessReturnResult,
    "ExcessReturnResult"
);

#[pymethods]
impl PyExcessReturnResult {
    /// Label of the base curve the excess returns were measured against.
    #[getter]
    fn base_label(&self) -> String {
        self.inner.base_label.clone()
    }

    /// Per-position results as a list of dicts, in input order.
    #[getter]
    fn positions<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.positions)
    }

    /// Weight-weighted portfolio total return.
    #[getter]
    fn portfolio_total_return(&self) -> f64 {
        self.inner.portfolio_total_return
    }

    /// Weight-weighted portfolio base return.
    #[getter]
    fn portfolio_base_return(&self) -> f64 {
        self.inner.portfolio_base_return
    }

    /// Weight-weighted portfolio excess return.
    #[getter]
    fn portfolio_excess_return(&self) -> f64 {
        self.inner.portfolio_excess_return
    }

    fn __repr__(&self) -> String {
        format!(
            "ExcessReturnResult(positions={}, portfolio_excess_return={})",
            self.inner.positions.len(),
            self.inner.portfolio_excess_return,
        )
    }
}

impl PyExcessReturnResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.positions, POSITION_EXCESS_COLUMNS)
    }
}

tabular_methods!(PyExcessReturnResult);

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._excess_return", "cell_returns_from_reference")]
#[pyfunction]
#[pyo3(text_signature = "(reference, base_label, config)")]
fn cell_returns_from_reference(
    py: Python<'_>,
    reference: &Bound<'_, PyAny>,
    base_label: &str,
    config: &Bound<'_, PyAny>,
) -> PyResult<PyDurationCellTable> {
    let reference = crate::bindings::extract::extract_records_json(py, reference, "reference")?;
    let config = crate::bindings::extract::extract_records_json(py, config, "config")?;
    Ok(PyDurationCellTable {
        inner: py.detach(move || {
            let reference: Vec<finstack_quant_portfolio::excess_return::ReferenceReturn> =
                serde_json::from_str(&reference)
                    .map_err(|err| serde_json_to_py(err, "invalid reference JSON"))?;
            let config: finstack_quant_portfolio::excess_return::CellConfig =
                serde_json::from_str(&config)
                    .map_err(|err| serde_json_to_py(err, "invalid config JSON"))?;
            finstack_quant_portfolio::excess_return::cell_returns_from_reference(
                &reference, base_label, &config,
            )
            .map_err(portfolio_to_py)
        })?,
    })
}
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._excess_return", "cell_returns_from_curves")]
#[pyfunction]
#[pyo3(text_signature = "(start, end, horizon_years, max_duration, base_label, config)")]
fn cell_returns_from_curves(
    py: Python<'_>,
    start: &PyDiscountCurve,
    end: &PyDiscountCurve,
    horizon_years: f64,
    max_duration: f64,
    base_label: &str,
    config: &Bound<'_, PyAny>,
) -> PyResult<PyDurationCellTable> {
    let config = crate::bindings::extract::extract_records_json(py, config, "config")?;
    let start_curve = std::sync::Arc::clone(&start.inner);
    let end_curve = std::sync::Arc::clone(&end.inner);
    Ok(PyDurationCellTable {
        inner: py.detach(move || {
            let config: finstack_quant_portfolio::excess_return::CellConfig =
                serde_json::from_str(&config)
                    .map_err(|err| serde_json_to_py(err, "invalid config JSON"))?;
            finstack_quant_portfolio::excess_return::cell_returns_from_curves(
                &start_curve,
                &end_curve,
                horizon_years,
                max_duration,
                base_label,
                &config,
            )
            .map_err(portfolio_to_py)
        })?,
    })
}
#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._excess_return", "excess_returns")]
#[pyfunction]
#[pyo3(text_signature = "(positions, table)")]
fn excess_returns(
    py: Python<'_>,
    positions: &Bound<'_, PyAny>,
    table: &Bound<'_, PyAny>,
) -> PyResult<PyExcessReturnResult> {
    let positions = crate::bindings::extract::extract_records_json(py, positions, "positions")?;
    let typed_table = table
        .cast::<PyDurationCellTable>()
        .ok()
        .map(|tbl| tbl.borrow().inner.clone());
    let table_json = if typed_table.is_none() {
        Some(crate::bindings::extract::extract_records_json(
            py, table, "table",
        )?)
    } else {
        None
    };
    Ok(PyExcessReturnResult {
        inner: py.detach(move || {
            let positions: Vec<finstack_quant_portfolio::excess_return::ExcessReturnPosition> =
                serde_json::from_str(&positions)
                    .map_err(|err| serde_json_to_py(err, "invalid positions JSON"))?;
            let table: finstack_quant_portfolio::excess_return::DurationCellTable =
                match typed_table {
                    Some(tbl) => tbl,
                    None => serde_json::from_str(table_json.as_deref().unwrap_or_default())
                        .map_err(|err| serde_json_to_py(err, "invalid table JSON"))?,
                };
            finstack_quant_portfolio::excess_return::excess_returns(&positions, &table)
                .map_err(portfolio_to_py)
        })?,
    })
}
/// Register duration-matched credit excess return functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDurationCellTable>()?;
    m.add_class::<PyExcessReturnResult>()?;
    m.add_function(wrap_pyfunction!(cell_returns_from_reference, m)?)?;
    m.add_function(wrap_pyfunction!(cell_returns_from_curves, m)?)?;
    m.add_function(wrap_pyfunction!(excess_returns, m)?)?;
    Ok(())
}
