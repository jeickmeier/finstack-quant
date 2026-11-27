//! Statement node and financial model specification Python bindings.

use super::forecast::PyForecastSpec;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::date_utils::date_to_py;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::{core_to_py, serde_json_to_py, statements_to_py};
use finstack_quant_statements::types::{NodeSpec, NodeValueType};
use indexmap::IndexMap;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Columns emitted by `FinancialModelSpec` tabular exports.
const NODE_COLUMNS: [(&str, &str); 8] = [
    ("node_id", "str"),
    ("node_type", "str"),
    ("name", "str"),
    ("formula_text", "str"),
    ("forecast_method", "str"),
    ("value_type", "str"),
    ("currency", "str"),
    ("where_text", "str"),
];

/// Node computation type.
#[pyclass(
    name = "NodeType",
    module = "finstack_quant.statements",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyNodeType {
    pub(crate) inner: finstack_quant_statements::types::NodeType,
}

#[pymethods]
impl PyNodeType {
    /// Value node: explicit per-period values only (actuals, assumptions).
    ///
    /// The node has no formula and no forecast, so periods without an
    /// explicit value stay unset.
    #[staticmethod]
    fn value() -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeType::Value,
        }
    }

    /// Calculated node: derived from its formula in every period.
    #[staticmethod]
    fn calculated() -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeType::Calculated,
        }
    }

    /// Mixed node: explicit value, else forecast, else formula.
    ///
    /// This is the crate's core precedence invariant — **Value > Forecast >
    /// Formula** — resolved independently for each period, so a node can be
    /// an actual in historical periods, a forecast in projected periods, and
    /// a formula wherever neither is supplied.
    #[staticmethod]
    fn mixed() -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeType::Mixed,
        }
    }

    #[doc = pydoc!("finstack_quant.statements._types.model", "NodeType.kind")]
    #[getter]
    fn kind(&self) -> PyResult<String> {
        crate::bindings::extract::label(&self.inner)
    }

    /// Return the canonical snake_case discriminant (same as :attr:`kind`).
    fn __str__(&self) -> PyResult<String> {
        self.kind()
    }

    /// Return ``NodeType('mixed')`` style representation.
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("NodeType({:?})", self.kind()?))
    }
}

/// Type-safe identifier for a node in a financial model.
#[pyclass(
    name = "NodeId",
    module = "finstack_quant.statements",
    eq,
    hash,
    frozen,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyNodeId {
    pub(crate) inner: finstack_quant_statements::types::NodeId,
}

#[pymethods]
impl PyNodeId {
    /// Wrap a raw node identifier string.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Node identifier as it appears in the model graph and in formulas
    ///     (e.g. ``"revenue"``). The value is stored verbatim — no case
    ///     folding or trimming — so identifiers are matched exactly.
    #[new]
    #[pyo3(text_signature = "(id)")]
    fn new(id: &str) -> Self {
        Self {
            inner: finstack_quant_statements::types::NodeId::new(id),
        }
    }

    /// Return the underlying identifier string.
    ///
    /// Returns
    /// -------
    /// str
    ///     The identifier exactly as supplied at construction.
    #[pyo3(text_signature = "($self)")]
    fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Return the representation, e.g. ``NodeId('revenue')``.
    fn __repr__(&self) -> String {
        format!("NodeId({:?})", self.inner.as_str())
    }

    /// Return the bare identifier string (no quoting or wrapper).
    fn __str__(&self) -> String {
        self.inner.to_string()
    }
}

/// Numeric evaluation mode.
#[pyclass(
    name = "NumericMode",
    module = "finstack_quant.statements",
    frozen,
    eq,
    hash,
    skip_from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PyNumericMode {
    pub(crate) inner: finstack_quant_statements::evaluator::NumericMode,
}

#[pymethods]
impl PyNumericMode {
    /// f64 floating-point evaluation — the mode the evaluator actually emits.
    ///
    /// Statement arithmetic runs in f64, so identity checks carry rounding
    /// proportional to the magnitudes involved; that is why check tolerances
    /// blend an absolute floor with a relative component.
    #[staticmethod]
    fn float64() -> Self {
        Self {
            inner: finstack_quant_statements::evaluator::NumericMode::Float64,
        }
    }

    /// Return ``NumericMode('float64')`` style representation.
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "NumericMode({:?})",
            crate::bindings::extract::label(&self.inner)?
        ))
    }
}

/// Specification of a single node (line item / metric) in a financial model.
///
/// A node is one of three kinds: a **value** node (explicit per-period
/// data), a **calculated** node (formula only), or a **mixed** node that
/// resolves each period as *Value > Forecast > Formula*. Nodes are normally
/// produced by ``ModelBuilder``; construct one directly for
/// ``ModelBuilder.insert_node`` when a template needs full control.
#[pyclass(
    name = "NodeSpec",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyNodeSpec {
    pub(crate) inner: NodeSpec,
}

wire_methods!(PyNodeSpec, NodeSpec, "NodeSpec");

#[pymethods]
impl PyNodeSpec {
    /// Build a node specification from its keyword-only fields.
    ///
    /// Rust reads the keywords as one ``NodeSpec`` document: each field takes
    /// its serde wire form (or a typed object the JSON hook writes as that
    /// form), Rust supplies the defaults, and an unknown keyword raises.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Unique node identifier as referenced from formulas.
    /// node_type : NodeType | str
    ///     ``NodeType.value()``, ``NodeType.calculated()`` or
    ///     ``NodeType.mixed()`` (or ``"value"`` / ``"calculated"`` /
    ///     ``"mixed"``).
    /// name : str | None
    ///     Optional human-readable label used in reports.
    /// values : Mapping[str, float | Money] | None
    ///     Explicit per-period values keyed by period id (``"2025Q1"``);
    ///     ``Money`` cells make the node monetary, floats make it scalar.
    /// availability_dates : Mapping[str, datetime.date | str]
    ///     Publication date of each explicit observation. Default ``{}``.
    /// forecast : ForecastSpec | None
    ///     Projection rule for forecast periods (mixed nodes).
    /// formula_text : str | None
    ///     Statements DSL expression (calculated and mixed nodes).
    /// where_text : str | None
    ///     Optional DSL predicate; periods where it is false evaluate to 0.
    /// tags : list[str]
    ///     Free-form labels for grouping and filtering. Default ``[]``.
    /// value_type : dict | None
    ///     Serde ``NodeValueType`` (``{"type": "monetary", "currency": "USD"}``
    ///     or ``{"type": "scalar"}``); ``None`` infers it from the values.
    /// rollup : str | None
    ///     ``"sum"``, ``"period_end"`` or ``"formula"``; ``None`` sums.
    /// meta : Mapping[str, Any]
    ///     Free-form JSON metadata. Default ``{}``.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a field is missing or has the wrong wire form, a keyword is
    ///     unknown, a period id does not parse, or ``rollup`` is not a
    ///     recognized method.
    #[new]
    #[pyo3(
        signature = (**fields),
        text_signature = "(*, node_id, node_type, name=None, values=None, availability_dates={}, forecast=None, formula_text=None, where_text=None, tags=[], value_type=None, rollup=None, meta={})"
    )]
    fn new(py: Python<'_>, fields: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let inner: NodeSpec =
            crate::bindings::module_utils::fields_from_py(py, fields, "NodeSpec")?;
        Ok(Self { inner })
    }

    /// Node identifier.
    #[getter]
    fn node_id(&self) -> &str {
        self.inner.node_id.as_str()
    }

    /// Human-readable name, or ``None``.
    #[getter]
    fn name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }

    /// Computation type (value / calculated / mixed).
    #[getter]
    fn node_type(&self) -> PyNodeType {
        PyNodeType {
            inner: self.inner.node_type,
        }
    }

    /// Explicit per-period values as floats, or ``None`` when the node
    /// carries no explicit data.
    ///
    /// Returns
    /// -------
    /// dict[str, float] | None
    ///     Period id to value in the node's own units; monetary amounts are
    ///     returned as their float amount (see :attr:`currency`).
    #[getter]
    fn values<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyDict>>> {
        let Some(values) = &self.inner.values else {
            return Ok(None);
        };
        let dict = PyDict::new(py);
        for (period, value) in values {
            dict.set_item(period.to_string(), value.value())?;
        }
        Ok(Some(dict))
    }

    /// Point-in-time availability dates for explicit observations.
    ///
    /// Returns
    /// -------
    /// dict[str, datetime.date]
    ///     Period id to the date the observation became available; empty
    ///     when every observation defaults to its period's exclusive end.
    #[getter]
    fn availability_dates<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for (period, date) in &self.inner.availability_dates {
            dict.set_item(period.to_string(), date_to_py(py, *date)?)?;
        }
        Ok(dict)
    }

    /// Forecast specification, or ``None``.
    #[getter]
    fn forecast(&self) -> Option<PyForecastSpec> {
        self.inner
            .forecast
            .clone()
            .map(|inner| PyForecastSpec { inner })
    }

    /// Statements DSL formula, or ``None`` for value-only nodes.
    #[getter]
    fn formula_text(&self) -> Option<&str> {
        self.inner.formula_text.as_deref()
    }

    /// Conditional ``where`` predicate, or ``None``.
    #[getter]
    fn where_text(&self) -> Option<&str> {
        self.inner.where_text.as_deref()
    }

    /// Free-form tags.
    #[getter]
    fn tags(&self) -> Vec<String> {
        self.inner.tags.clone()
    }

    #[doc = pydoc!("finstack_quant.statements._types.model", "NodeSpec.rollup")]
    #[getter]
    fn rollup(&self) -> PyResult<Option<String>> {
        self.inner
            .rollup
            .as_ref()
            .map(finstack_quant_core::wire::serde_label)
            .transpose()
            .map_err(core_to_py)
    }

    /// Declared or inferred value type: ``"monetary"``, ``"scalar"``, or
    /// ``None`` when not yet resolved (resolved at model build).
    #[getter]
    fn value_type(&self) -> Option<&'static str> {
        self.inner.value_type.map(|value_type| match value_type {
            NodeValueType::Monetary { .. } => "monetary",
            NodeValueType::Scalar => "scalar",
        })
    }

    /// ISO-4217 currency code for monetary nodes, else ``None``.
    #[getter]
    fn currency(&self) -> Option<String> {
        match self.inner.value_type {
            Some(NodeValueType::Monetary { currency }) => Some(currency.to_string()),
            _ => None,
        }
    }

    /// Node-level metadata as a plain ``dict``.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Return ``NodeSpec(node_id='revenue', node_type='mixed', ...)``.
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "NodeSpec(node_id={:?}, node_type={:?}, values={}, forecast={}, formula_text={})",
            self.inner.node_id.as_str(),
            crate::bindings::extract::label(&self.inner.node_type)?,
            self.inner.values.as_ref().map_or(0, IndexMap::len),
            self.inner.forecast.is_some(),
            self.inner
                .formula_text
                .as_deref()
                .map_or_else(|| "None".to_string(), |f| format!("{f:?}")),
        ))
    }
}

/// Top-level financial model specification.
#[pyclass(
    name = "FinancialModelSpec",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyFinancialModelSpec {
    pub(crate) inner: finstack_quant_statements::types::FinancialModelSpec,
}

impl PyFinancialModelSpec {
    pub(crate) fn from_inner(inner: finstack_quant_statements::types::FinancialModelSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFinancialModelSpec {
    #[doc = pydoc!("finstack_quant.statements._types.model", "FinancialModelSpec.to_builder")]
    fn to_builder(&self) -> PyResult<super::super::builder::PyModelBuilder> {
        self.inner
            .to_builder()
            .map(super::super::builder::PyModelBuilder::from_inner)
            .map_err(statements_to_py)
    }

    #[doc = pydoc!("finstack_quant.statements._types.model", "FinancialModelSpec.with_meta")]
    fn with_meta(&self, key: &str, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        let value = crate::bindings::module_utils::py_to_serde(value.py(), value, "value")?;
        Ok(Self::from_inner(self.inner.with_meta(key, value)))
    }

    #[doc = pydoc!("finstack_quant.statements._types.model", "FinancialModelSpec.with_currency")]
    fn with_currency(&self, currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        let currency = crate::bindings::core::currency::extract_currency(currency)?;
        Ok(Self::from_inner(self.inner.with_currency(currency)))
    }

    #[doc = pydoc!("finstack_quant.statements._types.model", "FinancialModelSpec.with_reporting_currency")]
    fn with_reporting_currency(&self, currency: &Bound<'_, PyAny>) -> PyResult<Self> {
        let currency = crate::bindings::core::currency::extract_currency(currency)?;
        self.inner
            .with_reporting_currency(currency)
            .map(Self::from_inner)
            .map_err(statements_to_py)
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from a JSON string and run semantic validation.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed, or the model violates a semantic
    ///     invariant (empty timeline, reserved node id, invalid formula,
    ///     mixed currencies, invalid waterfall).
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(py: Python<'_>, json: &str) -> PyResult<Self> {
        let inner = finstack_quant_statements::types::FinancialModelSpec::from_json(json)
            .map_err(|error| crate::errors::contract_to_py(py, error))?;
        Ok(Self { inner })
    }

    /// Serialize to a JSON string.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize FinancialModelSpec"))
    }

    /// Unique model identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[doc = pydoc!("finstack_quant.statements._types.model", "FinancialModelSpec.source_snapshot")]
    #[getter]
    fn source_snapshot(&self) -> Option<&str> {
        self.inner.source_snapshot.as_deref()
    }

    #[doc = pydoc!("finstack_quant.statements._types.model", "FinancialModelSpec.apply_edits")]
    #[pyo3(text_signature = "($self, edits)")]
    fn apply_edits(&self, py: Python<'_>, edits: &Bound<'_, PyAny>) -> PyResult<Self> {
        let edits: Vec<finstack_quant_statements::types::ModelEdit> =
            crate::bindings::module_utils::py_to_serde(py, edits, "model edits")?;
        let mut inner = self.inner.clone();
        inner.apply_edits(&edits).map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Number of periods in the model timeline.
    ///
    /// Counted in **periods** on the model's own cadence (quarters, months,
    /// years), not months. Their declared order *is* the evaluation timeline.
    #[getter]
    fn period_count(&self) -> usize {
        self.inner.periods.len()
    }

    /// Period identifiers in timeline order (``["2025Q1", "2025Q2", ...]``).
    #[getter]
    fn periods(&self) -> Vec<String> {
        self.inner
            .periods
            .iter()
            .map(|p| p.id.to_string())
            .collect()
    }

    /// Period identifiers flagged as actuals (historical), in timeline order.
    #[getter]
    fn actual_periods(&self) -> Vec<String> {
        self.inner
            .periods
            .iter()
            .filter(|p| p.is_actual)
            .map(|p| p.id.to_string())
            .collect()
    }

    /// Period identifiers flagged as forecasts, in timeline order.
    #[getter]
    fn forecast_periods(&self) -> Vec<String> {
        self.inner
            .periods
            .iter()
            .filter(|p| !p.is_actual)
            .map(|p| p.id.to_string())
            .collect()
    }

    /// Number of nodes (line items / metrics) declared in the model.
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

    /// Look up one node specification.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier.
    ///
    /// Returns
    /// -------
    /// NodeSpec | None
    ///     The node's specification (type, values, forecast, formula, ...),
    ///     or ``None`` when the model has no such node.
    #[pyo3(text_signature = "($self, node_id)")]
    fn get_node(&self, node_id: &str) -> Option<PyNodeSpec> {
        self.inner.get_node(node_id).map(|node| PyNodeSpec {
            inner: node.clone(),
        })
    }

    /// All node specifications in declaration order.
    #[getter]
    fn nodes(&self) -> Vec<PyNodeSpec> {
        self.inner
            .nodes
            .values()
            .map(|node| PyNodeSpec {
                inner: node.clone(),
            })
            .collect()
    }

    /// Model-level metadata as a plain ``dict`` (e.g. ``{"currency": "USD"}``).
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Capital-structure specification in its JSON shape, or ``None``.
    ///
    /// Returns
    /// -------
    /// dict | None
    ///     ``{"debt_instruments": [{"id", "spec": {"type", "spec"}}],
    ///     "reporting_currency", "fx_policy", "waterfall", "meta"}`` as
    ///     plain Python containers, matching ``to_json()``.
    #[getter]
    fn capital_structure<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .capital_structure
            .as_ref()
            .map(|cs| serde_to_py(py, cs))
            .transpose()
    }

    #[doc = pydoc!("finstack_quant.statements._types.model", "FinancialModelSpec.content_hash")]
    #[pyo3(text_signature = "($self)")]
    fn content_hash(&self) -> PyResult<String> {
        self.inner.content_hash().map_err(core_to_py)
    }

    /// Wire-format schema version of this model spec.
    ///
    /// Only version ``1`` is accepted today; the field exists so persisted
    /// models can be migrated rather than silently misread.
    #[getter]
    fn schema_version(&self) -> u32 {
        self.inner.schema_version.into()
    }

    /// Return the representation with the id, period and node counts.
    fn __repr__(&self) -> String {
        format!(
            "FinancialModelSpec(id={:?}, periods={}, nodes={})",
            self.inner.id,
            self.inner.periods.len(),
            self.inner.nodes.len()
        )
    }
}

impl PyFinancialModelSpec {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows: Vec<serde_json::Value> = self
            .inner
            .nodes
            .values()
            .map(|node| {
                let (value_type, currency) = match node.value_type {
                    Some(NodeValueType::Monetary { currency }) => {
                        (Some("monetary"), Some(currency.to_string()))
                    }
                    Some(NodeValueType::Scalar) => (Some("scalar"), None),
                    None => (None, None),
                };
                Ok(serde_json::json!({
                    "node_id": node.node_id.as_str(),
                    "node_type": crate::bindings::extract::label(&node.node_type)?,
                    "name": node.name,
                    "formula_text": node.formula_text,
                    "forecast_method": node.forecast.as_ref().map(|f| crate::bindings::extract::label(&f.method())).transpose()?,
                    "value_type": value_type,
                    "currency": currency,
                    "where_text": node.where_text,
                }))
            })
            .collect::<PyResult<_>>()?;
        serde_rows_to_arrow_table(&rows, &NODE_COLUMNS)
    }
}

tabular_methods!(
    PyFinancialModelSpec,
    "FinancialModelSpec",
    module = "finstack_quant.statements._types.model"
);
