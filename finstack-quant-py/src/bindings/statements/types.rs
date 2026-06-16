//! Python wrappers for statement model types and enums.

use crate::errors::display_to_py;
use indexmap::IndexMap;
use pyo3::prelude::*;

// ---------------------------------------------------------------------------
// ForecastMethod
// ---------------------------------------------------------------------------

/// Available forecast methods for projecting node values.
#[pyclass(
    name = "ForecastMethod",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyForecastMethod {
    pub(super) inner: finstack_quant_statements::types::ForecastMethod,
}

#[pymethods]
impl PyForecastMethod {
    #[staticmethod]
    fn forward_fill() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::ForwardFill,
        }
    }

    #[staticmethod]
    fn growth_pct() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::GrowthPct,
        }
    }

    #[staticmethod]
    fn curve_pct() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::CurvePct,
        }
    }

    #[staticmethod]
    fn normal() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::Normal,
        }
    }

    #[staticmethod]
    fn log_normal() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::LogNormal,
        }
    }

    #[staticmethod]
    fn override_method() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::Override,
        }
    }

    #[staticmethod]
    fn time_series() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::TimeSeries,
        }
    }

    #[staticmethod]
    fn seasonal() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastMethod::Seasonal,
        }
    }

    fn __repr__(&self) -> String {
        format!("ForecastMethod({:?})", self.inner)
    }
}

// ---------------------------------------------------------------------------
// ForecastSpec
// ---------------------------------------------------------------------------

/// Forecast configuration for a statement model node.
#[pyclass(
    name = "ForecastSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyForecastSpec {
    pub(super) inner: finstack_quant_statements::types::ForecastSpec,
}

#[pymethods]
impl PyForecastSpec {
    #[new]
    #[pyo3(signature = (method, params_json=None), text_signature = "(method, params_json=None)")]
    fn new(method: PyRef<'_, PyForecastMethod>, params_json: Option<&str>) -> PyResult<Self> {
        let params = parse_params_json(params_json)?;
        Ok(Self {
            inner: finstack_quant_statements::types::ForecastSpec {
                method: method.inner,
                params,
            },
        })
    }

    #[staticmethod]
    fn forward_fill() -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastSpec::forward_fill(),
        }
    }

    #[staticmethod]
    fn growth(rate: f64) -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastSpec::growth(rate),
        }
    }

    #[staticmethod]
    fn curve(curve: Vec<f64>) -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastSpec::curve(curve),
        }
    }

    #[staticmethod]
    fn normal(mean: f64, std_dev: f64, seed: u64) -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastSpec::normal(mean, std_dev, seed),
        }
    }

    #[staticmethod]
    fn lognormal(mean: f64, std_dev: f64, seed: u64) -> Self {
        Self {
            inner: finstack_quant_statements::types::ForecastSpec::lognormal(mean, std_dev, seed),
        }
    }

    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "ForecastSpec(method={:?}, params={})",
            self.inner.method,
            self.inner.params.len()
        )
    }
}

fn parse_params_json(params_json: Option<&str>) -> PyResult<IndexMap<String, serde_json::Value>> {
    match params_json {
        Some(json) => serde_json::from_str(json).map_err(display_to_py),
        None => Ok(IndexMap::new()),
    }
}

// ---------------------------------------------------------------------------
// NodeType
// ---------------------------------------------------------------------------

/// Node computation type.
#[pyclass(
    name = "NodeType",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyNodeType {
    pub(super) inner: finstack_quant_statements::types::NodeType,
}

#[pymethods]
impl PyNodeType {
    #[staticmethod]
    fn value() -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeType::Value,
        }
    }

    #[staticmethod]
    fn calculated() -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeType::Calculated,
        }
    }

    #[staticmethod]
    fn mixed() -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeType::Mixed,
        }
    }

    fn __repr__(&self) -> String {
        format!("NodeType({:?})", self.inner)
    }
}

// ---------------------------------------------------------------------------
// NodeId
// ---------------------------------------------------------------------------

/// Type-safe identifier for a node in a financial model.
#[pyclass(
    name = "NodeId",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyNodeId {
    pub(super) inner: finstack_quant_statements::types::NodeId,
}

#[pymethods]
impl PyNodeId {
    #[new]
    #[pyo3(text_signature = "(id)")]
    fn new(id: &str) -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeId::new(id),
        }
    }

    #[pyo3(text_signature = "($self)")]
    fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    fn __repr__(&self) -> String {
        format!("NodeId({:?})", self.inner.as_str())
    }

    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

// ---------------------------------------------------------------------------
// NumericMode
// ---------------------------------------------------------------------------

/// Numeric evaluation mode.
#[pyclass(
    name = "NumericMode",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyNumericMode {
    pub(super) inner: finstack_quant_statements::evaluator::NumericMode,
}

#[pymethods]
impl PyNumericMode {
    #[staticmethod]
    fn float64() -> Self {
        Self {
            inner: finstack_quant_statements::evaluator::NumericMode::Float64,
        }
    }

    #[staticmethod]
    fn decimal() -> Self {
        // Reserved for future Rust fixed-point statement evaluation.
        Self {
            inner: finstack_quant_statements::evaluator::NumericMode::Decimal,
        }
    }

    fn __repr__(&self) -> String {
        format!("NumericMode({:?})", self.inner)
    }
}

// ---------------------------------------------------------------------------
// FinancialModelSpec — JSON round-trip
// ---------------------------------------------------------------------------

/// Top-level financial model specification.
#[pyclass(
    name = "FinancialModelSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFinancialModelSpec {
    pub(crate) inner: finstack_quant_statements::FinancialModelSpec,
}

#[pymethods]
impl PyFinancialModelSpec {
    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let mut inner: finstack_quant_statements::FinancialModelSpec =
            serde_json::from_str(json).map_err(display_to_py)?;
        inner.validate_semantics().map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Model identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Number of periods.
    #[getter]
    fn period_count(&self) -> usize {
        self.inner.periods.len()
    }

    /// Number of nodes.
    #[getter]
    fn node_count(&self) -> usize {
        self.inner.nodes.len()
    }

    /// Node identifiers in declaration order.
    #[pyo3(text_signature = "($self)")]
    fn node_ids(&self) -> Vec<String> {
        self.inner.nodes.keys().map(|k| k.to_string()).collect()
    }

    /// Whether the model has a node with the given ID.
    #[pyo3(text_signature = "($self, node_id)")]
    fn has_node(&self, node_id: &str) -> bool {
        self.inner.has_node(node_id)
    }

    /// Schema version.
    #[getter]
    fn schema_version(&self) -> u32 {
        self.inner.schema_version
    }

    fn __repr__(&self) -> String {
        format!(
            "FinancialModelSpec(id={:?}, periods={}, nodes={})",
            self.inner.id,
            self.inner.periods.len(),
            self.inner.nodes.len()
        )
    }
}

/// Register type classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyForecastMethod>()?;
    m.add_class::<PyForecastSpec>()?;
    m.add_class::<PyNodeType>()?;
    m.add_class::<PyNodeId>()?;
    m.add_class::<PyNumericMode>()?;
    m.add_class::<PyFinancialModelSpec>()?;
    Ok(())
}
