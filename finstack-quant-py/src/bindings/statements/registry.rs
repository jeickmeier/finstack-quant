//! Python wrappers for the metric registry (`finstack_quant_statements::registry`).

use crate::bindings::macros::wire_methods;
use crate::bindings::pandas_utils::serde_to_py;
use crate::bindings::pydoc::pydoc;
use crate::errors::statements_to_py;
use pyo3::prelude::*;

/// Read-only view of one registry metric definition.
///
/// Returned by ``Registry.get``; carries the formula, documentation, and
/// declared dependencies of a reusable metric such as ``fin.gross_margin``.
#[pyclass(
    name = "MetricDefinition",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMetricDefinition {
    inner: finstack_quant_statements::registry::MetricDefinition,
}

wire_methods!(
    PyMetricDefinition,
    finstack_quant_statements::registry::MetricDefinition,
    "MetricDefinition"
);

#[pymethods]
impl PyMetricDefinition {
    /// Metric identifier within its namespace (``"gross_margin"``).
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Human-readable metric name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Statements DSL formula, with unqualified references to sibling
    /// metrics (``"gross_profit / revenue"``).
    #[getter]
    fn formula(&self) -> &str {
        &self.inner.formula
    }

    /// Description of what the metric represents, or ``None``.
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    /// Grouping category (``"margins"``, ``"leverage"``, ...), or ``None``.
    #[getter]
    fn category(&self) -> Option<&str> {
        self.inner.category.as_deref()
    }

    #[doc = pydoc!("finstack_quant.statements", "MetricDefinition.unit_type")]
    #[getter]
    fn unit_type(&self) -> PyResult<Option<String>> {
        self.inner
            .unit_type
            .as_ref()
            .map(crate::bindings::extract::label)
            .transpose()
    }

    /// Node identifiers the formula requires as inputs.
    #[getter]
    fn requires(&self) -> Vec<String> {
        self.inner.requires.clone()
    }

    /// Free-form tags for filtering.
    #[getter]
    fn tags(&self) -> Vec<String> {
        self.inner.tags.clone()
    }

    /// Additional metadata as a plain ``dict``.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Return ``MetricDefinition(id='gross_margin', formula='...')``.
    fn __repr__(&self) -> String {
        format!(
            "MetricDefinition(id={:?}, name={:?}, formula={:?})",
            self.inner.id, self.inner.name, self.inner.formula
        )
    }
}

/// Metric registry used to add reusable statement metrics to a model.
#[pyclass(
    name = "Registry",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRegistry {
    pub(crate) inner: finstack_quant_statements::registry::Registry,
}

#[pymethods]
impl PyRegistry {
    /// Create an empty metric registry.
    #[new]
    fn new() -> Self {
        Self {
            inner: finstack_quant_statements::registry::Registry::new(),
        }
    }

    /// Create a registry preloaded with built-in metrics.
    #[staticmethod]
    fn with_builtins() -> PyResult<Self> {
        let inner = finstack_quant_statements::registry::Registry::with_builtins()
            .map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Load built-in metrics into this registry.
    fn load_builtins(&mut self) -> PyResult<()> {
        self.inner.load_builtins().map_err(statements_to_py)
    }

    /// Load metrics from a JSON string.
    fn load_from_json_str(&mut self, json: &str) -> PyResult<()> {
        self.inner
            .load_from_json_str(json)
            .map(|_| ())
            .map_err(statements_to_py)
    }

    /// Return whether a fully qualified metric exists.
    fn has(&self, qualified_id: &str) -> bool {
        self.inner.has(qualified_id)
    }

    /// Fully qualified identifiers of every registered metric
    /// (``["fin.gross_profit", "fin.gross_margin", ...]``), in load order.
    #[pyo3(text_signature = "($self)")]
    fn metric_ids(&self) -> Vec<String> {
        self.inner
            .all_metrics()
            .map(|(id, _)| id.to_string())
            .collect()
    }

    /// Look up one metric definition.
    ///
    /// Parameters
    /// ----------
    /// qualified_id : str
    ///     Fully qualified ``namespace.metric_id`` (``"fin.gross_margin"``).
    ///
    /// Returns
    /// -------
    /// MetricDefinition
    ///     The registered definition.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If no metric with that id is registered.
    #[pyo3(text_signature = "($self, qualified_id)")]
    fn get(&self, qualified_id: &str) -> PyResult<PyMetricDefinition> {
        let stored = self.inner.get(qualified_id).map_err(statements_to_py)?;
        Ok(PyMetricDefinition {
            inner: stored.definition.clone(),
        })
    }

    #[doc = pydoc!("finstack_quant.statements", "Registry.get_dependencies")]
    #[pyo3(text_signature = "($self, qualified_id)")]
    fn get_dependencies(&self, qualified_id: &str) -> PyResult<Vec<String>> {
        self.inner
            .get_dependencies(qualified_id)
            .map_err(statements_to_py)
    }

    /// Number of metrics in the registry.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Return ``Registry(metrics=22)``.
    fn __repr__(&self) -> String {
        format!("Registry(metrics={})", self.inner.len())
    }
}

/// Register registry classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyRegistry>()?;
    m.add_class::<PyMetricDefinition>()?;
    Ok(())
}
