//! Python bindings for factor-model what-if and factor-stress results
//! (Rust `factor_model::whatif`): `WhatIfResult`, `FactorContributionDelta`,
//! `StressResult` and `StressPnl`.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::tabular_methods;
use crate::bindings::models::factor::risk::PyRiskDecomposition;
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::table_builders::TableBuilder;
use finstack_quant_portfolio::factor_model as fm;
use finstack_quant_portfolio::factor_model::FactorContributionDelta;
use finstack_quant_portfolio::factor_model::StressPnl;
use finstack_quant_portfolio::factor_model::StressResult;
use finstack_quant_portfolio::factor_model::WhatIfResult;
use pyo3::prelude::*;

/// Deserialize what-if position changes straight into the canonical Rust
/// `PositionChange` wire shape (`{"kind": "remove" | "resize", ...}`).
pub(super) fn parse_position_changes(
    py: Python<'_>,
    changes: &Bound<'_, PyAny>,
) -> PyResult<Vec<fm::PositionChange>> {
    py_to_serde(py, changes, "position changes")
}

/// Per-factor contribution change between a baseline and a scenario.
#[pyclass(
    name = "FactorContributionDelta",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyFactorContributionDelta {
    pub(crate) inner: FactorContributionDelta,
}

impl PyFactorContributionDelta {
    fn from_inner(inner: FactorContributionDelta) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorContributionDelta {
    /// Identifier of the factor whose contribution changed.
    #[getter]
    fn factor_id(&self) -> String {
        self.inner.factor_id.as_str().to_owned()
    }

    /// ``after - before`` change in the factor's absolute risk contribution, in
    /// the units of the decomposition's risk measure.
    #[getter]
    fn absolute_change(&self) -> f64 {
        self.inner.absolute_change
    }

    /// ``after - before`` change in the factor's relative (fractional) risk
    /// contribution; dimensionless, not a percentage.
    #[getter]
    fn relative_change(&self) -> f64 {
        self.inner.relative_change
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorContributionDelta(factor_id={:?}, absolute_change={}, relative_change={})",
            self.inner.factor_id.as_str(),
            self.inner.absolute_change,
            self.inner.relative_change,
        )
    }
}
crate::bindings::macros::wire_methods!(
    PyFactorContributionDelta,
    FactorContributionDelta,
    "FactorContributionDelta"
);

/// Result of a position what-if scenario.
#[pyclass(
    name = "WhatIfResult",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyWhatIfResult {
    pub(crate) inner: WhatIfResult,
}

impl PyWhatIfResult {
    pub(super) fn from_inner(inner: WhatIfResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyWhatIfResult {
    /// Baseline risk decomposition used as the comparison point.
    ///
    /// Use its own table accessors (:meth:`RiskDecomposition.to_factor_table`
    /// and friends) for a tabular view of the baseline.
    #[getter]
    fn before(&self) -> PyRiskDecomposition {
        PyRiskDecomposition::from_inner(self.inner.before.clone())
    }

    /// Risk decomposition after applying the requested position changes.
    #[getter]
    fn after(&self) -> PyRiskDecomposition {
        PyRiskDecomposition::from_inner(self.inner.after.clone())
    }

    /// Per-factor ``after - before`` contribution changes.
    #[getter]
    fn delta(&self) -> Vec<PyFactorContributionDelta> {
        self.inner
            .delta
            .iter()
            .cloned()
            .map(PyFactorContributionDelta::from_inner)
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "WhatIfResult(before_total={}, after_total={}, delta_entries={})",
            self.inner.before.total_risk,
            self.inner.after.total_risk,
            self.inner.delta.len(),
        )
    }
}
crate::bindings::macros::wire_methods!(PyWhatIfResult, WhatIfResult, "WhatIfResult");

impl PyWhatIfResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows = &self.inner.delta;
        let factor_ids: Vec<String> = rows.iter().map(|d| d.factor_id.to_string()).collect();
        let absolute_change: Vec<f64> = rows.iter().map(|d| d.absolute_change).collect();
        let relative_change: Vec<f64> = rows.iter().map(|d| d.relative_change).collect();
        TableBuilder::new()
            .utf8("factor_id", factor_ids)?
            .f64("absolute_change", absolute_change)?
            .f64("relative_change", relative_change)?
            .build()
    }
}

tabular_methods!(PyWhatIfResult);

/// Result of a factor-stress scenario.
#[pyclass(
    name = "StressResult",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyStressResult {
    pub(crate) inner: StressResult,
}

impl PyStressResult {
    pub(super) fn from_inner(inner: StressResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyStressResult {
    /// Total portfolio P&L under the stressed market.
    ///
    /// Returns
    /// -------
    /// float
    ///     Portfolio-currency amount; a loss is **negative**.
    #[getter]
    fn total_pnl(&self) -> f64 {
        self.inner.pnl.total_pnl
    }

    /// Per-position ``(position_id, pnl)`` entries.
    #[getter]
    fn position_pnl(&self) -> Vec<(String, f64)> {
        self.inner
            .pnl
            .position_pnl
            .iter()
            .map(|(id, pnl)| (id.as_str().to_owned(), *pnl))
            .collect()
    }

    /// Risk decomposition recomputed under the stressed market.
    #[getter]
    fn stressed_decomposition(&self) -> PyRiskDecomposition {
        PyRiskDecomposition::from_inner(self.inner.stressed_decomposition.clone())
    }

    fn __repr__(&self) -> String {
        format!(
            "StressResult(total_pnl={}, positions={}, stressed_total_risk={})",
            self.inner.pnl.total_pnl,
            self.inner.pnl.position_pnl.len(),
            self.inner.stressed_decomposition.total_risk,
        )
    }
}
crate::bindings::macros::wire_methods!(PyStressResult, StressResult, "StressResult");

impl PyStressResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows = &self.inner.pnl.position_pnl;
        let position_ids: Vec<String> = rows.iter().map(|(id, _)| id.as_str().to_owned()).collect();
        let pnl: Vec<f64> = rows.iter().map(|(_, pnl)| *pnl).collect();
        TableBuilder::new()
            .utf8("position_id", position_ids)?
            .f64("pnl", pnl)?
            .build()
    }
}

tabular_methods!(PyStressResult);

/// P&L-only result of a factor-stress scenario.
///
/// Stressed-minus-base present value without a risk decomposition on the
/// shocked market. Produced by :meth:`FactorModel.factor_stress_pnl`; use
/// :meth:`FactorModel.factor_stress` when the stressed decomposition is also
/// needed.
#[pyclass(
    name = "StressPnl",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyStressPnl {
    pub(crate) inner: StressPnl,
}

impl PyStressPnl {
    pub(super) fn from_inner(inner: StressPnl) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyStressPnl {
    /// Total portfolio P&L under the stressed market.
    ///
    /// Returns
    /// -------
    /// float
    ///     Portfolio base-currency amount; a loss is **negative**.
    #[getter]
    fn total_pnl(&self) -> f64 {
        self.inner.total_pnl
    }

    /// Per-position ``(position_id, pnl)`` entries in portfolio order.
    #[getter]
    fn position_pnl(&self) -> Vec<(String, f64)> {
        self.inner
            .position_pnl
            .iter()
            .map(|(id, pnl)| (id.as_str().to_owned(), *pnl))
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "StressPnl(total_pnl={}, positions={})",
            self.inner.total_pnl,
            self.inner.position_pnl.len(),
        )
    }
}
crate::bindings::macros::wire_methods!(PyStressPnl, StressPnl, "StressPnl");

impl PyStressPnl {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows = &self.inner.position_pnl;
        let position_ids: Vec<String> = rows.iter().map(|(id, _)| id.as_str().to_owned()).collect();
        let pnl: Vec<f64> = rows.iter().map(|(_, pnl)| *pnl).collect();
        TableBuilder::new()
            .utf8("position_id", position_ids)?
            .f64("pnl", pnl)?
            .build()
    }
}

tabular_methods!(PyStressPnl);
