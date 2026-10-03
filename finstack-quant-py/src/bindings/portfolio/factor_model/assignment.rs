use pyo3::prelude::*;
use pyo3::types::PyDict;

use finstack_quant_portfolio::factor_model::{
    FactorAssignmentReport, PositionAssignment, UnmatchedEntry,
};

use crate::bindings::pandas_utils::dict_to_dataframe;
use crate::errors::display_to_py;

use crate::bindings::json_bridge::{deserialize_json, serialize_json};

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
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PositionAssignment = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

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
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: UnmatchedEntry = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

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
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: FactorAssignmentReport = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

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
    /// One row per ``(dependency, factor_id, beta)`` mapping, so a position
    /// matched by a hierarchical matcher (e.g. credit) contributes one row per
    /// level. Row count is the sum of ``n_mappings`` across
    /// :attr:`assignments`, not the number of positions.
    ///
    /// Columns: ``position_id``, ``factor_id``, ``beta`` (factor loading),
    /// ``dependency_json`` — the ``MarketDependency`` variant tree is
    /// deliberately kept as a JSON string rather than exploded into columns,
    /// matching :meth:`PositionAssignment.mappings_json`.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a market dependency cannot be serialized to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut position_ids: Vec<&str> = Vec::new();
        let mut factor_ids: Vec<&str> = Vec::new();
        let mut betas: Vec<f64> = Vec::new();
        let mut dependencies: Vec<String> = Vec::new();
        for assignment in &self.inner.assignments {
            for (dependency, factor_id, beta) in &assignment.mappings {
                position_ids.push(assignment.position_id.as_str());
                factor_ids.push(factor_id.as_str());
                betas.push(*beta);
                dependencies.push(serde_json::to_string(dependency).map_err(display_to_py)?);
            }
        }
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("factor_id", factor_ids)?;
        data.set_item("beta", betas)?;
        data.set_item("dependency_json", dependencies)?;
        dict_to_dataframe(py, &data, None)
    }

    /// Export the unmatched dependencies as a pandas ``DataFrame``.
    ///
    /// One row per entry of :attr:`unmatched`; a zero-row frame (columns still
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
    fn to_unmatched_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.unmatched;
        let position_ids: Vec<&str> = rows.iter().map(|e| e.position_id.as_str()).collect();
        let dependencies: Vec<String> = rows
            .iter()
            .map(|e| serde_json::to_string(&e.dependency).map_err(display_to_py))
            .collect::<PyResult<Vec<_>>>()?;
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("dependency_json", dependencies)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorAssignmentReport(assignments={}, unmatched={})",
            self.inner.assignments.len(),
            self.inner.unmatched.len(),
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}
