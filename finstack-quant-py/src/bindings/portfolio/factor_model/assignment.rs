use pyo3::prelude::*;

use finstack_quant_portfolio::factor_model::{
    FactorAssignmentReport, PositionAssignment, UnmatchedEntry,
};

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::display_to_py;

use crate::bindings::json_bridge::serialize_json;

/// Matched factor assignments for a single portfolio position.
///
/// The `mappings` field carries ``(MarketDependency, FactorId, beta)`` triples whose
/// dependency variant tree is wide enough that the binding exposes it as a
/// JSON-serialized vector via :meth:`mappings_json` rather than a fully
/// structured Python type.
#[pyclass(
    name = "PositionAssignment",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionAssignment {
    pub(crate) inner: PositionAssignment,
}

impl PyPositionAssignment {
    fn from_inner(inner: PositionAssignment) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionAssignment {
    /// Portfolio position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Number of matched ``(dependency, factor_id, beta)`` triples.
    #[getter]
    fn n_mappings(&self) -> usize {
        self.inner.mappings.len()
    }

    /// Matched ``(dependency, factor_id, beta)`` triples as a JSON string.
    #[pyo3(text_signature = "(self)")]
    fn mappings_json(&self) -> PyResult<String> {
        serialize_json(&self.inner.mappings)
    }

    /// Matched factor identifiers (in mapping order).
    #[getter]
    fn factor_ids(&self) -> Vec<String> {
        self.inner
            .mappings
            .iter()
            .map(|(_, fid, _)| fid.as_str().to_owned())
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionAssignment(position_id={:?}, n_mappings={})",
            self.inner.position_id.as_str(),
            self.inner.mappings.len(),
        )
    }
}
crate::bindings::macros::wire_methods!(
    PyPositionAssignment,
    PositionAssignment,
    "PositionAssignment"
);

/// Single unmatched dependency surfaced during assignment.
#[pyclass(
    name = "UnmatchedEntry",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyUnmatchedEntry {
    pub(crate) inner: UnmatchedEntry,
}

impl PyUnmatchedEntry {
    fn from_inner(inner: UnmatchedEntry) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyUnmatchedEntry {
    /// Portfolio position identifier that owns the unmatched dependency.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Unmatched dependency as a JSON string.
    #[pyo3(text_signature = "(self)")]
    fn dependency_json(&self) -> PyResult<String> {
        serialize_json(&self.inner.dependency)
    }

    fn __repr__(&self) -> String {
        format!(
            "UnmatchedEntry(position_id={:?})",
            self.inner.position_id.as_str(),
        )
    }
}
crate::bindings::macros::wire_methods!(PyUnmatchedEntry, UnmatchedEntry, "UnmatchedEntry");

/// Assignment results for a portfolio-level factor mapping pass.
#[pyclass(
    name = "FactorAssignmentReport",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyFactorAssignmentReport {
    pub(crate) inner: FactorAssignmentReport,
}

impl PyFactorAssignmentReport {
    pub(crate) fn from_inner(inner: FactorAssignmentReport) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorAssignmentReport {
    /// Per-position matched dependencies and factor identifiers.
    #[getter]
    fn assignments(&self) -> Vec<PyPositionAssignment> {
        self.inner
            .assignments
            .iter()
            .cloned()
            .map(PyPositionAssignment::from_inner)
            .collect()
    }

    /// Dependencies that did not match any configured factor.
    #[getter]
    fn unmatched(&self) -> Vec<PyUnmatchedEntry> {
        self.inner
            .unmatched
            .iter()
            .cloned()
            .map(PyUnmatchedEntry::from_inner)
            .collect()
    }

    /// Export the matched assignments as a long-format pandas ``DataFrame``.
    ///
    /// Export the unmatched dependencies as an
    /// :class:`~finstack_quant.core.table.ArrowTable`.
    ///
    /// One row per entry of :attr:`unmatched`; a zero-row table (columns still
    /// present) means every dependency matched a configured factor.
    ///
    /// Columns: ``position_id``, ``dependency_json`` (the unmatched
    /// ``MarketDependency`` as a JSON string).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a market dependency cannot be serialized to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_unmatched_table(&self) -> PyResult<PyArrowTable> {
        let rows = &self.inner.unmatched;
        let position_ids: Vec<String> = rows
            .iter()
            .map(|e| e.position_id.as_str().to_owned())
            .collect();
        let dependencies: Vec<String> = rows
            .iter()
            .map(|e| serde_json::to_string(&e.dependency).map_err(display_to_py))
            .collect::<PyResult<Vec<_>>>()?;
        TableBuilder::new()
            .utf8("position_id", position_ids)?
            .utf8("dependency_json", dependencies)?
            .build()
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorAssignmentReport(assignments={}, unmatched={})",
            self.inner.assignments.len(),
            self.inner.unmatched.len(),
        )
    }
}
crate::bindings::macros::wire_methods!(
    PyFactorAssignmentReport,
    FactorAssignmentReport,
    "FactorAssignmentReport"
);

impl PyFactorAssignmentReport {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let mut position_ids: Vec<String> = Vec::new();
        let mut factor_ids: Vec<String> = Vec::new();
        let mut betas: Vec<f64> = Vec::new();
        let mut dependencies: Vec<String> = Vec::new();
        for assignment in &self.inner.assignments {
            for (dependency, factor_id, beta) in &assignment.mappings {
                position_ids.push(assignment.position_id.as_str().to_owned());
                factor_ids.push(factor_id.as_str().to_owned());
                betas.push(*beta);
                dependencies.push(serde_json::to_string(dependency).map_err(display_to_py)?);
            }
        }
        TableBuilder::new()
            .utf8("position_id", position_ids)?
            .utf8("factor_id", factor_ids)?
            .f64("beta", betas)?
            .utf8("dependency_json", dependencies)?
            .build()
    }
}

tabular_methods!(PyFactorAssignmentReport);
