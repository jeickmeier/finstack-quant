//! Multi-column panel transform specification, result, and entry points.

use super::parse::{extract_keys, extract_opt_f64_vec};
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::py_to_json_value;
use crate::errors::core_to_py;
use finstack_quant_features::{PanelTransformResult, PanelTransformSpec};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;

/// Specification for a sequential panel transform pipeline.
///
/// Parameters
/// ----------
/// values : list[float | None]
///     Input value column; ``None``, NaN and infinity are missing.
/// operations : list[dict]
///     Ordered operations, each ``{"name", "family" ("timeseries" |
///     "cross_sectional"), "op", "params"?, "input"?}``. ``input`` selects
///     ``"values"`` or an earlier operation name (default: previous column).
/// entity : sequence, optional
///     Row-aligned entity keys (required for time-series ops; aware datetimes normalize to UTC; strings stay opaque).
/// order : sequence, optional
///     Row-aligned sort keys (required for time-series ops).
/// time_key : sequence, optional
///     Row-aligned partition keys (required for cross-sectional ops).
///
/// Raises
/// ------
/// ValueError
///     If an operation mapping is malformed (unknown family, op, or key).
///
/// Examples
/// --------
/// >>> from finstack_quant.features import PanelTransformSpec
/// >>> spec = PanelTransformSpec([1.0, 3.0], [{"name": "r", "family": "cross_sectional", "op": "rank"}], time_key=["d", "d"])
/// >>> spec.operation_names
/// ['r']
#[pyclass(
    name = "PanelTransformSpec",
    module = "finstack_quant.features",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPanelTransformSpec {
    /// Inner typed spec.
    pub(crate) inner: PanelTransformSpec,
}

#[pymethods]
impl PyPanelTransformSpec {
    /// Construct a panel spec; see the class docstring for parameters.
    #[new]
    #[pyo3(
        signature = (values, operations, entity=None, order=None, time_key=None),
        text_signature = "(values, operations, entity=None, order=None, time_key=None)"
    )]
    fn new(
        py: Python<'_>,
        values: &Bound<'_, PyAny>,
        operations: &Bound<'_, PyAny>,
        entity: Option<&Bound<'_, PyAny>>,
        order: Option<&Bound<'_, PyAny>>,
        time_key: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let values = extract_opt_f64_vec(values, "values")?;
        let ops_json = py_to_json_value(py, operations, "panel transform operations")?;
        let operations = serde_json::from_value(ops_json).map_err(|e| {
            crate::errors::serde_json_to_py(e, "invalid panel transform operations")
        })?;
        Ok(Self {
            inner: PanelTransformSpec {
                values: values
                    .into_iter()
                    .map(|value| value.filter(|v| v.is_finite()))
                    .collect(),
                entity: entity.map(|e| extract_keys(e, "entity")).transpose()?,
                order: order.map(|o| extract_keys(o, "order")).transpose()?,
                time_key: time_key.map(|t| extract_keys(t, "time_key")).transpose()?,
                operations,
            },
        })
    }

    /// Output column names in operation order.
    #[getter]
    fn operation_names(&self) -> Vec<String> {
        self.inner
            .operations
            .iter()
            .map(|op| op.name().to_string())
            .collect()
    }

    /// Input value column.
    #[getter]
    fn values(&self) -> Vec<Option<f64>> {
        self.inner.values.clone()
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        format!(
            "PanelTransformSpec(rows={}, operations={:?})",
            self.inner.values.len(),
            self.operation_names()
        )
    }
}

wire_methods!(
    PyPanelTransformSpec,
    PanelTransformSpec,
    "PanelTransformSpec"
);

/// Ordered output columns of a panel transform pipeline.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import transform_panel
/// >>> res = transform_panel({"values": [1.0, 3.0], "time_key": ["d", "d"], "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}]})
/// >>> res.columns
/// ['r']
/// >>> res.get_column("r")
/// [0.0, 1.0]
#[pyclass(
    name = "PanelTransformResult",
    module = "finstack_quant.features",
    frozen,
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyPanelTransformResult {
    /// Inner typed result.
    pub(crate) inner: PanelTransformResult,
}

#[pymethods]
impl PyPanelTransformResult {
    /// Output column names in operation order.
    #[getter]
    fn columns(&self) -> Vec<String> {
        self.inner.columns.iter().map(|c| c.name.clone()).collect()
    }

    /// Values of one output column, aligned to the input rows.
    ///
    /// Parameters
    /// ----------
    /// name : str
    ///     Operation output name (case-sensitive).
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If no column has that name.
    #[pyo3(text_signature = "(self, name)")]
    fn get_column(&self, name: &str) -> PyResult<Vec<Option<f64>>> {
        self.inner
            .get_column(name)
            .map(<[Option<f64>]>::to_vec)
            .ok_or_else(|| {
                pyo3::exceptions::PyKeyError::new_err(format!(
                    "no panel transform column '{name}'; available: {:?}",
                    self.columns()
                ))
            })
    }

    /// Columns as a ``pandas.DataFrame`` (one float column per operation).
    ///
    /// Parameters
    /// ----------
    /// index : pandas.Index or sequence, optional
    ///     Row index to attach (e.g. the source frame's index); default
    ///     ``RangeIndex``.
    ///
    /// Returns
    /// -------
    /// pandas.DataFrame
    ///     ``None`` outputs become ``NaN``.
    #[allow(clippy::wrong_self_convention)]
    #[pyo3(signature = (index=None), text_signature = "(self, index=None)")]
    fn to_pandas<'py>(
        &self,
        py: Python<'py>,
        index: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if self.inner.columns.is_empty()
            && let Some(idx) = index
        {
            let kwargs = PyDict::new(py);
            kwargs.set_item("index", idx)?;
            return py
                .import("pandas")?
                .call_method("DataFrame", (), Some(&kwargs));
        }
        let table = Bound::new(py, self.to_arrow_table()?)?;
        let frame = crate::bindings::core::table::PyArrowTable::to_pandas(&table, py)?;
        if let Some(idx) = index {
            frame.setattr("index", idx)?;
        }
        Ok(frame)
    }

    /// Python-style summary.
    fn __repr__(&self) -> String {
        format!(
            "PanelTransformResult(columns={:?}, rows={})",
            self.columns(),
            self.inner.columns.first().map_or(0, |c| c.values.len())
        )
    }
}

impl PyPanelTransformResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<crate::bindings::core::table::PyArrowTable> {
        let mut builder = crate::bindings::table_builders::TableBuilder::new();
        for column in &self.inner.columns {
            builder = builder.nullable_f64(&column.name, column.values.clone())?;
        }
        builder.build()
    }
}

crate::bindings::macros::tabular_methods!(PyPanelTransformResult, custom_pandas);
wire_methods!(
    PyPanelTransformResult,
    PanelTransformResult,
    "PanelTransformResult"
);

/// Apply a named panel transform pipeline (typed twin of ``transform_panel_json``).
///
/// Operations run sequentially; each reads the previous column unless
/// ``input`` selects ``"values"`` or an earlier operation name.
///
/// Parameters
/// ----------
/// spec : PanelTransformSpec or dict
///     Typed spec or an equivalent ``dict`` (``values``, ``operations``,
///     optional ``entity`` / ``order`` / ``time_key``). Load JSON text with
///     ``PanelTransformSpec.from_json``.
///
/// Returns
/// -------
/// PanelTransformResult
///     Ordered output columns with ``get_column`` and ``to_pandas`` / ``to_polars`` / ``to_arrow``.
///
/// Raises
/// ------
/// ValueError
///     If the spec is malformed, an operation name is duplicated or
///     reserved, ``input`` is unknown, or an operation fails.
///
/// Examples
/// --------
/// >>> from finstack_quant.features import transform_panel
/// >>> spec = {"values": [1.0, 3.0], "time_key": ["d", "d"], "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}]}
/// >>> transform_panel(spec).get_column("r")
/// [0.0, 1.0]
#[pyfunction]
#[pyo3(text_signature = "(spec)")]
pub(super) fn transform_panel(
    py: Python<'_>,
    spec: &Bound<'_, PyAny>,
) -> PyResult<PyPanelTransformResult> {
    let spec: PanelTransformSpec = match spec.extract::<PyRef<'_, PyPanelTransformSpec>>() {
        Ok(typed) => typed.inner.clone(),
        _ => {
            {
                // Host conversion only: numeric/NaN values, date-like keys and other
                // fields become JSON; Rust serde owns the field set, the
                // required fields and the unknown-field rejection.
                let mapping = spec.cast::<PyDict>()?;
                let mut fields = serde_json::Map::new();
                for (key, value) in mapping.iter() {
                    let key: String = key.extract()?;
                    let json = match key.as_str() {
                        _ if value.is_none() => Value::Null,
                        // Non-finite numbers (NaN marks missing) become JSON null.
                        "values" => Value::from(extract_opt_f64_vec(&value, "values")?),
                        "entity" | "order" | "time_key" => Value::from(extract_keys(&value, &key)?),
                        _ => py_to_json_value(py, &value, "panel transform spec field")?,
                    };
                    fields.insert(key, json);
                }
                serde_json::from_value(Value::Object(fields)).map_err(|error| {
                    crate::errors::serde_json_to_py(error, "invalid panel transform spec")
                })?
            }
        }
    };
    py.detach(move || finstack_quant_features::transform_panel(&spec))
        .map(|inner| PyPanelTransformResult { inner })
        .map_err(core_to_py)
}

/// Apply a JSON panel transform pipeline (JSON twin of ``transform_panel``).
///
/// Parameters
/// ----------
/// spec_json : str
///     JSON-encoded ``PanelTransformSpec``.
///
/// Returns
/// -------
/// str
///     JSON ``{"columns": [{"name", "values"}, ...]}`` in operation order.
///
/// Raises
/// ------
/// ValueError
///     If the JSON is malformed or an operation fails.
///
/// Examples
/// --------
/// >>> import json
/// >>> from finstack_quant.features import transform_panel_json
/// >>> spec = {"values": [1.0, 3.0], "time_key": ["d", "d"], "operations": [{"name": "r", "family": "cross_sectional", "op": "rank"}]}
/// >>> json.loads(transform_panel_json(json.dumps(spec)))["columns"][0]["name"]
/// 'r'
#[pyfunction]
#[pyo3(text_signature = "(spec_json)")]
pub(super) fn transform_panel_json(py: Python<'_>, spec_json: String) -> PyResult<String> {
    py.detach(move || finstack_quant_features::transform_panel_json(&spec_json))
        .map_err(core_to_py)
}
