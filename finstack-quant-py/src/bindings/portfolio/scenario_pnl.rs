//! Typed wrapper for scenario profit-and-loss results.

use crate::bindings::pandas_utils::ColumnSchema;
use crate::bindings::pandas_utils::{
    labeled_values_to_series, serde_rows_to_dataframe_with_schema,
};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use serde::Serialize;

/// Column schema for [`PyScenarioPnl::to_dataframe`].
///
/// Pinned so an empty ladder keeps the same dtypes as a populated one.
const PNL_ROW_COLUMNS: &[ColumnSchema<'static>] = &[("position_id", "str"), ("pnl", "f64")];

#[derive(Serialize)]
struct PnlRow<'a> {
    position_id: &'a str,
    pnl: f64,
}

/// Scenario-attributable profit and loss, in the portfolio base currency.
///
/// Returned by :func:`scenario_pnl` alongside the scenario
/// :class:`ApplicationReport`.
#[pyclass(
    name = "ScenarioPnl",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioPnl {
    pub(crate) inner: finstack_quant_portfolio::scenarios::ScenarioPnl,
}

crate::bindings::macros::wire_methods!(
    PyScenarioPnl,
    finstack_quant_portfolio::scenarios::ScenarioPnl,
    "ScenarioPnl"
);

impl PyScenarioPnl {
    fn rows(&self) -> Vec<PnlRow<'_>> {
        self.inner
            .by_position
            .iter()
            .map(|(id, money)| PnlRow {
                position_id: id.as_str(),
                pnl: money.amount(),
            })
            .collect()
    }
}

#[pymethods]
impl PyScenarioPnl {
    /// Total scenario P&L in the portfolio base currency.
    #[getter]
    fn total(&self) -> f64 {
        self.inner.total.amount()
    }

    /// Base currency the P&L is denominated in.
    #[getter]
    fn currency(&self) -> String {
        self.inner.total.currency().to_string()
    }

    /// Per-position scenario P&L, keyed by position id.
    ///
    /// The dict preserves the canonical Rust ``IndexMap`` iteration order
    /// (stressed positions first, then base-only positions), matching
    /// :meth:`to_dataframe`, :meth:`to_series`, and :meth:`to_json`.
    #[getter]
    fn by_position<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let out = PyDict::new(py);
        for (id, money) in &self.inner.by_position {
            out.set_item(id.as_str(), money.amount())?;
        }
        Ok(out)
    }

    /// Per-position P&L ladder as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``position_id``, ``pnl``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.rows(), PNL_ROW_COLUMNS)
    }

    /// Per-position P&L as a :class:`pandas.Series` indexed by position id.
    fn to_series<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let labels: Vec<String> = self
            .inner
            .by_position
            .keys()
            .map(ToString::to_string)
            .collect();
        let values: Vec<f64> = self
            .inner
            .by_position
            .values()
            .map(finstack_quant_core::money::Money::amount)
            .collect();
        labeled_values_to_series(py, &labels, values, "pnl")
    }

    /// Identify this value in notebooks and logs.
    ///
    /// Rendered from the wire representation, so the fields shown are the
    /// fields `to_json()` names. Collections are summarised by length; use
    /// `to_json()` or a DataFrame exit when the contents matter.
    fn __repr__(&self) -> String {
        crate::bindings::repr_support::repr_from_serde("ScenarioPnl", &self.inner)
    }
}

/// Column schema for [`PyScenarioPnlBatchItem::to_dataframe`].
///
/// The per-position ladder columns, prefixed with the owning scenario id so
/// frames from a batch can be concatenated directly.
const BATCH_PNL_ROW_COLUMNS: &[ColumnSchema<'static>] = &[
    ("scenario_id", "str"),
    ("position_id", "str"),
    ("pnl", "float64"),
];

#[derive(Serialize)]
struct BatchPnlRow<'a> {
    scenario_id: &'a str,
    position_id: &'a str,
    pnl: f64,
}

/// One ordered result from :func:`scenario_pnl_batch`.
///
/// Carries the scenario identifier, its typed :class:`ScenarioPnl` ladder,
/// and the scenario :class:`~finstack_quant.scenarios.ApplicationReport`.
#[pyclass(
    name = "ScenarioPnlBatchItem",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyScenarioPnlBatchItem {
    pub(crate) inner: finstack_quant_portfolio::scenarios::ScenarioPnlBatchItem,
}

crate::bindings::macros::wire_methods!(
    PyScenarioPnlBatchItem,
    finstack_quant_portfolio::scenarios::ScenarioPnlBatchItem,
    "ScenarioPnlBatchItem"
);

impl PyScenarioPnlBatchItem {
    fn rows(&self) -> Vec<BatchPnlRow<'_>> {
        self.inner
            .pnl
            .by_position
            .iter()
            .map(|(id, money)| BatchPnlRow {
                scenario_id: &self.inner.scenario_id,
                position_id: id.as_str(),
                pnl: money.amount(),
            })
            .collect()
    }
}

#[pymethods]
impl PyScenarioPnlBatchItem {
    /// Identifier copied from the input scenario.
    #[getter]
    fn scenario_id(&self) -> String {
        self.inner.scenario_id.clone()
    }

    /// Scenario-attributable portfolio P&L ladder.
    #[getter]
    fn pnl(&self) -> PyScenarioPnl {
        PyScenarioPnl {
            inner: self.inner.pnl.clone(),
        }
    }

    /// Application provenance and warnings for this scenario.
    #[getter]
    fn report(&self) -> crate::bindings::scenarios::engine::PyApplicationReport {
        crate::bindings::scenarios::engine::PyApplicationReport {
            inner: self.inner.report.clone(),
        }
    }

    /// Per-position P&L ladder as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``scenario_id``, ``position_id``, ``pnl``.
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.rows(), BATCH_PNL_ROW_COLUMNS)
    }

    fn __repr__(&self) -> String {
        format!(
            "ScenarioPnlBatchItem(scenario_id={:?}, positions={})",
            self.inner.scenario_id,
            self.inner.pnl.by_position.len(),
        )
    }
}
